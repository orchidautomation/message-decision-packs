use crate::artifact_hash::{
    canonical_json_sha256, canonical_json_sha256_for_domain, pack_content_snapshot, sha256_hex,
};
use crate::authority::{ProjectionFidelity, SourceAuthority};
use crate::commands::prompt_output::{
    validate_prompt_output_file_with_inputs, validate_prompt_output_file_with_lineage_inputs,
};
use crate::commands::requirements::requirements;
use crate::commands::routing::{
    fit, fit_normalized, fit_prospect_with_governed_authority, resolve_job_ingress,
};
use crate::commands::schemas::prompt_output_schema_for_ref;
use crate::constants::{
    COLLECTED_ATTEMPT_RESULTS_CONTRACT_V2, GENERATED_PACK_DIRECTORIES,
    NORMALIZED_DECISION_INPUT_CONTRACT, NORMALIZED_DECISION_INPUT_CONTRACT_V2,
    ROUTED_CONTEXT_CONTRACT, SOURCE_ATTEMPT_REQUEST_CONTRACT_V2, SOURCE_BINDING_CONTRACT_V2,
};
use crate::model_steps::{CompiledModelStepV1, ModelStepPhase, resolve_selected_model_step};
use crate::pack_io::{read_manifest, resolve_pack_path};
use crate::run_contracts::{
    ArtifactAuthority, AssuranceDimension, AssuranceEvidenceState,
    DRIVER_CONFIGURATION_PROJECTION_V1, DRIVER_REQUEST_V2, DRIVER_RESULT_V2, DecisionAuthority,
    DriverArtifactV2, DriverConfigurationProjectionV1, DriverOutputV2, DriverProviderObservationV2,
    DriverProviderPolicyV2, DriverRequestV2, DriverResultV2, EvidenceProvenance,
    IdentityObservationV1, MDP_RUNTIME_VERSION, MODEL_PARAMETERS_PROJECTION_V1,
    ModelParametersFactsV1, ModelParametersProjectionV1, OPENAI_PROVIDER_REQUEST_SCHEMA_ID,
    PROVIDER_REQUEST_NOT_OBSERVED_V1, PROVIDER_REQUEST_RELATION_V1, PackAuthority,
    ProviderRequestObservationV1, RUN_BUNDLE_V1, RUN_RECEIPT_V1, RUN_REQUEST_V1, RUNNER_AUDIT_V1,
    RunBundleV1, RunMode, RunReceiptV1, RunRequestV1, RunnerAuditV1, TerminalState,
};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PROPOSAL_PROFILE: &str = "proposal";
const VALIDATE_EXISTING_OUTPUT: &str = "validate-existing-output";
const GTM_PROFILE: &str = "gtm";
const QUALIFY: &str = "qualify";
const MAX_PACK_FILES: usize = 10_000;
const MAX_PACK_BYTES: u64 = 100 * 1024 * 1024;
const MAX_EXECUTION_ID_BYTES: usize = 128;
const MAX_OUTPUT_LEAF_BYTES: usize = 120;
const MAX_RECOVERY_CLAIM_BYTES: usize = 512;
const MAX_POLICY_INPUT_BYTES: u64 = 100 * 1024 * 1024;
// Native requests also contain the prompt envelope and projected provider
// schema. Keep the public generative input budget well below the driver's
// 2 MiB serialized-request ceiling so requests cannot pass preflight and then
// fail only after the immutable bundle has been published.
const MAX_NATIVE_DECLARED_INPUT_BYTES: u64 = 128 * 1024;
const MAX_NATIVE_SERIALIZED_REQUEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_POLICY_OUTPUT_BYTES: u64 = 1024 * 1024;
const DRIVER_RESULT_ENVELOPE_BYTES: u64 = 64 * 1024;
const MAX_FINALIZATION_RESERVE_MS: u64 = 250;
const OFFICIAL_OPENAI_RESPONSES_ENDPOINT: &str = "https://api.openai.com/v1/responses";
const NATIVE_SUBPROCESS_REQUEST_V1: &str = "mdp.native-model-subprocess-request.v1";
const NATIVE_SUBPROCESS_RESULT_V1: &str = "mdp.native-model-subprocess-result.v1";
const BUNDLED_NATIVE_DRIVER_ID: &str = "bundled:mdp-native-model-openai";
const BUNDLED_NATIVE_DRIVER_SOURCE: &str =
    include_str!("../../scripts/mdp-native-model-openai.mjs");

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunExecution {
    pub(crate) contract: String,
    pub(crate) valid: bool,
    pub(crate) execution_id: String,
    pub(crate) terminal_state: TerminalState,
    pub(crate) authority: SourceAuthority,
    pub(crate) run_dir: String,
    pub(crate) bundle_sha256: String,
    pub(crate) receipt_sha256: String,
    pub(crate) authority_block: Value,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RunFailureKind {
    Preflight,
    PolicyBlocked,
    RunnerFailed,
}

#[derive(Debug)]
pub(crate) struct RunFailure {
    kind: RunFailureKind,
    code: &'static str,
}

impl RunFailure {
    fn new(kind: RunFailureKind, code: &'static str) -> Self {
        Self { kind, code }
    }

    pub(crate) fn kind(&self) -> RunFailureKind {
        self.kind
    }

    pub(crate) fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for RunFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}

impl std::error::Error for RunFailure {}

fn run_failure(kind: RunFailureKind, code: &'static str) -> anyhow::Error {
    anyhow::Error::new(RunFailure::new(kind, code))
}

struct RunDeadline {
    started_at: Instant,
    budget: Duration,
}

impl RunDeadline {
    fn new(timeout_ms: u64) -> Self {
        Self {
            started_at: Instant::now(),
            budget: Duration::from_millis(timeout_ms),
        }
    }

    fn check(&self) -> Result<()> {
        if self.started_at.elapsed() >= self.budget {
            return Err(run_failure(
                RunFailureKind::RunnerFailed,
                "execution-timeout",
            ));
        }
        Ok(())
    }

    fn expired(&self) -> bool {
        self.started_at.elapsed() >= self.budget
    }

    fn driver_timeout_ms(&self) -> Option<u64> {
        let elapsed = self.started_at.elapsed();
        let remaining = self.budget.checked_sub(elapsed)?;
        let reserve_ms =
            MAX_FINALIZATION_RESERVE_MS.min((self.budget.as_millis() as u64 / 10).max(1));
        let driver_budget = remaining.checked_sub(Duration::from_millis(reserve_ms))?;
        u64::try_from(driver_budget.as_millis())
            .ok()
            .filter(|value| *value > 0)
    }
}

struct TransactionGuard {
    transaction_dir: PathBuf,
    claim_path: PathBuf,
}

#[derive(Serialize)]
struct RunRecoveryClaim<'a> {
    contract: &'static str,
    execution_id: &'a str,
    transaction_leaf: &'a str,
}

impl Drop for TransactionGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.transaction_dir);
        let _ = fs::remove_file(&self.claim_path);
    }
}

#[derive(Clone)]
struct StagedInput {
    logical_name: String,
    authority: ArtifactAuthority,
    source_path: PathBuf,
    staged_path: PathBuf,
    initial_sha256: String,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct NativeSubprocessRequestV1<'a> {
    contract: &'static str,
    execution_id: &'a str,
    provider: &'a str,
    model: &'a str,
    prompt_id: &'a str,
    declared_inputs_only: bool,
    input: String,
    output_schema: &'a Value,
    output_schema_sha256: &'a str,
    schema_name: String,
    timeout_ms: u64,
    max_output_tokens: u64,
}

struct PreparedNativeRequest {
    step: CompiledModelStepV1,
    invocation_value: Value,
    invocation_bytes: Vec<u8>,
    invocation_sha256: String,
    visible_input: String,
    canonical_output_schema: Value,
    canonical_output_schema_sha256: String,
    provider_output_schema: Value,
    provider_output_schema_sha256: String,
    schema_name: String,
}

fn driver_configuration_projection(
    identity: &crate::run_contracts::DriverIdentity,
    source_sha256: String,
    node_sha256: String,
) -> DriverConfigurationProjectionV1 {
    DriverConfigurationProjectionV1 {
        contract: DRIVER_CONFIGURATION_PROJECTION_V1.into(),
        driver_id: identity.driver_id.clone(),
        implementation: identity.implementation.clone(),
        runtime_version: MDP_RUNTIME_VERSION.into(),
        bundled_source_sha256: source_sha256,
        node_executable_sha256: node_sha256,
        native_request_contract: NATIVE_SUBPROCESS_REQUEST_V1.into(),
        native_result_contract: NATIVE_SUBPROCESS_RESULT_V1.into(),
        clear_env: true,
        allowlisted_environment_names: vec![
            "MDP_ALLOW_NATIVE_MODEL_CALLS".into(),
            "OPENAI_API_KEY".into(),
        ],
        filesystem_mode: "private-staging".into(),
        stdin_mode: "bounded-json".into(),
        stdout_mode: "bounded-json-result".into(),
        max_request_bytes: MAX_NATIVE_SERIALIZED_REQUEST_BYTES as u64,
        max_response_bytes: MAX_POLICY_OUTPUT_BYTES
            .saturating_mul(6)
            .saturating_add(DRIVER_RESULT_ENVELOPE_BYTES),
        timeout_enforced: true,
        authorized_endpoint: OFFICIAL_OPENAI_RESPONSES_ENDPOINT.into(),
        redirect_policy: "reject".into(),
        proxy_policy: "excluded".into(),
        storage_policy: "store-false".into(),
        tool_policy: "none".into(),
    }
}

fn model_parameters_facts(
    model: &crate::run_contracts::ModelIdentity,
    prepared: &PreparedNativeRequest,
    declared_timeout_ms: u64,
    max_output_bytes: u64,
) -> ModelParametersFactsV1 {
    ModelParametersFactsV1::from_runtime_inputs(
        model.provider.clone(),
        model.requested_model.clone(),
        model.authorized_endpoint.clone(),
        declared_timeout_ms,
        provider_max_output_tokens(max_output_bytes),
        prepared.schema_name.clone(),
        prepared.provider_output_schema_sha256.clone(),
        sha256_hex(prepared.visible_input.as_bytes()),
    )
}

fn projection_hash<T: Serialize>(domain: &str, projection: &T) -> Result<String> {
    canonical_json_sha256_for_domain(domain, &serde_json::to_value(projection)?)
}

fn prepare_native_request(
    request: &RunRequestV1,
    manifest: &crate::models::Manifest,
    staged_pack: &Path,
    staged_prompt: &StagedInput,
    staged_inputs: &[StagedInput],
) -> Result<PreparedNativeRequest> {
    let identity = request
        .job_identity
        .as_ref()
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "job-identity-required"))?;
    let step =
        resolve_selected_model_step(staged_pack, manifest, &identity.job_id, &request.operation)
            .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "model-step-not-declared"))?;
    validate_generative_job_gates(staged_pack, &identity.job_id, step.phase)?;
    validate_selected_prompt(staged_pack, staged_prompt, &step)?;
    validate_step_inputs(&step, staged_inputs)?;
    validate_generative_input_gates(staged_pack, manifest, staged_inputs, &identity.job_id)?;

    let invocation_value = json!({
        "contract": "mdp.prompt-invocation.v1",
        "job_id": identity.job_id,
        "prompt": {"id": step.prompt_id, "version": step.prompt_version, "sha256": step.prompt_sha256},
        "inputs": staged_inputs.iter().map(|input| json!({
            "name": input.logical_name,
            "sha256": input.authority.sha256,
        })).collect::<Vec<_>>(),
    });
    let mut invocation_bytes = serde_json::to_vec_pretty(&invocation_value)?;
    invocation_bytes.push(b'\n');
    let invocation_content = std::str::from_utf8(&invocation_bytes)
        .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "invocation-not-utf8"))?;
    let invocation_sha256 = sha256_hex(&invocation_bytes);
    let visible_inputs = staged_inputs
        .iter()
        .map(|input| {
            Ok((
                input.authority.logical_name.clone(),
                input.authority.sha256.clone(),
                utf8_staged_content(input, "declared-input")?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let visible_input = native_visible_input(
        &step.prompt_id,
        &step.prompt_version,
        &step.prompt_sha256,
        &utf8_staged_content(staged_prompt, "prompt")?,
        &invocation_sha256,
        invocation_content,
        &visible_inputs,
    );
    let canonical_output_schema =
        canonical_output_schema_for_step(staged_pack, &identity.job_id, &step)?;
    let canonical_output_schema_sha256 = canonical_json_sha256(&canonical_output_schema)?;
    let provider_schema_source =
        provider_schema_source_for_contract(&canonical_output_schema, &step.output_contract)?;
    let provider_output_schema = project_output_schema_for_openai(&provider_schema_source)?;
    let provider_output_schema_sha256 = canonical_json_sha256(&provider_output_schema)?;
    let schema_name = format!("mdp_{}", request.operation.replace([':', '/', '-'], "_"));
    let model = request
        .model
        .as_ref()
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "model-identity-required"))?;
    let native = NativeSubprocessRequestV1 {
        contract: NATIVE_SUBPROCESS_REQUEST_V1,
        execution_id: &request.execution_id,
        provider: &model.provider,
        model: &model.requested_model,
        prompt_id: &step.prompt_id,
        declared_inputs_only: true,
        input: visible_input.clone(),
        output_schema: &provider_output_schema,
        output_schema_sha256: &provider_output_schema_sha256,
        schema_name: schema_name.clone(),
        timeout_ms: request.execution_policy.timeout_ms,
        max_output_tokens: provider_max_output_tokens(request.execution_policy.max_output_bytes),
    };
    if serde_json::to_vec(&native)?.len() > MAX_NATIVE_SERIALIZED_REQUEST_BYTES {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "native-request-too-large",
        ));
    }
    Ok(PreparedNativeRequest {
        step,
        invocation_value,
        invocation_bytes,
        invocation_sha256,
        visible_input,
        canonical_output_schema,
        canonical_output_schema_sha256,
        provider_output_schema,
        provider_output_schema_sha256,
        schema_name,
    })
}

fn bind_native_identity(
    request: &RunRequestV1,
    prepared: &PreparedNativeRequest,
) -> Result<(
    crate::run_contracts::DriverIdentity,
    crate::run_contracts::ModelIdentity,
    IdentityObservationV1,
    ModelParametersFactsV1,
)> {
    let declared_driver = request
        .driver
        .as_ref()
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "driver-identity-required"))?;
    let declared_model = request
        .model
        .as_ref()
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "model-identity-required"))?;
    let source_sha256 = sha256_hex(BUNDLED_NATIVE_DRIVER_SOURCE.as_bytes());
    let node = resolve_node_executable()?;
    let node_sha256 = sha256_hex(&read_bounded(&node, 200 * 1024 * 1024, "node executable")?);
    if declared_driver.executable_sha256.as_deref() != Some(source_sha256.as_str()) {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "driver-hash-mismatch",
        ));
    }
    if declared_driver.dependency_lock_sha256.as_deref() != Some(node_sha256.as_str()) {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "node-hash-mismatch",
        ));
    }
    if declared_driver.version != MDP_RUNTIME_VERSION {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "driver-version-mismatch",
        ));
    }
    let driver_projection =
        driver_configuration_projection(declared_driver, source_sha256, node_sha256);
    let driver_facts = (&driver_projection).into();
    let driver_observed_sha256 =
        projection_hash(DRIVER_CONFIGURATION_PROJECTION_V1, &driver_projection)?;
    let model_facts = model_parameters_facts(
        declared_model,
        prepared,
        request.execution_policy.timeout_ms,
        request.execution_policy.max_output_bytes,
    );
    let model_projection: ModelParametersProjectionV1 = (&model_facts).into();
    let model_observed_sha256 = projection_hash(MODEL_PARAMETERS_PROJECTION_V1, &model_projection)?;
    if declared_driver.configuration_sha256 != driver_observed_sha256 {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "driver-configuration-identity-mismatch",
        ));
    }
    if declared_model.parameters_sha256 != model_observed_sha256 {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "model-parameters-identity-mismatch",
        ));
    }
    let mut observed_driver = declared_driver.clone();
    observed_driver.configuration_sha256 = driver_observed_sha256.clone();
    observed_driver.identity_provenance = EvidenceProvenance::MdpObserved;
    let mut observed_model = declared_model.clone();
    observed_model.parameters_sha256 = model_observed_sha256.clone();
    let identity_observations = IdentityObservationV1 {
        driver_declaration_sha256: declared_driver.configuration_sha256.clone(),
        driver_observed_sha256,
        driver_projection,
        driver_facts,
        model_declaration_sha256: declared_model.parameters_sha256.clone(),
        model_observed_sha256,
        model_projection,
        provider_request: ProviderRequestObservationV1 {
            provider_request_body_sha256: None,
            provider_request_schema_id: None,
            relation: PROVIDER_REQUEST_NOT_OBSERVED_V1.into(),
        },
    };
    Ok((
        observed_driver,
        observed_model,
        identity_observations,
        model_facts,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeSubprocessOutputV1 {
    media_type: String,
    encoding: String,
    content: String,
    byte_count: u64,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeSubprocessObservationV1 {
    provider: String,
    response_id: Option<String>,
    model: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeSubprocessResultV1 {
    contract: String,
    execution_id: String,
    terminal_state: TerminalState,
    output: Option<NativeSubprocessOutputV1>,
    provider_request_body_sha256: Option<String>,
    provider_request_schema_id: Option<String>,
    provider_response_body_sha256: Option<String>,
    provider_output_schema_sha256: Option<String>,
    provider_observation: Option<NativeSubprocessObservationV1>,
    diagnostic_code: Option<String>,
}

fn invoke_native_driver(
    request: &DriverRequestV2,
    identity: &crate::run_contracts::DriverIdentity,
) -> Result<DriverResultV2> {
    if identity.driver_id != "mdp-native-openai" {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "driver-not-authorized",
        ));
    }
    let expected_script_hash = identity
        .executable_sha256
        .as_deref()
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "driver-hash-required"))?;
    if identity.implementation != BUNDLED_NATIVE_DRIVER_ID {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "driver-implementation-not-bundled",
        ));
    }
    if sha256_hex(BUNDLED_NATIVE_DRIVER_SOURCE.as_bytes()) != expected_script_hash {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "driver-hash-mismatch",
        ));
    }
    let node = resolve_node_executable()?;
    let expected_node_hash = identity
        .dependency_lock_sha256
        .as_deref()
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "node-hash-required"))?;
    if sha256_hex(&read_bounded(&node, 200 * 1024 * 1024, "node executable")?) != expected_node_hash
    {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "node-hash-mismatch",
        ));
    }

    let visible_inputs = request
        .inputs
        .iter()
        .map(|input| {
            (
                input.authority.logical_name.clone(),
                input.authority.sha256.clone(),
                input.content_utf8.clone(),
            )
        })
        .collect::<Vec<_>>();
    let visible_input = native_visible_input(
        &request.prompt_id,
        &request.prompt_version,
        &request.prompt_canonical_sha256,
        &request.prompt.content_utf8,
        &request.prompt_invocation.authority.sha256,
        &request.prompt_invocation.content_utf8,
        &visible_inputs,
    );
    let native = NativeSubprocessRequestV1 {
        contract: NATIVE_SUBPROCESS_REQUEST_V1,
        execution_id: &request.execution_id,
        provider: &request.provider_policy.provider,
        model: &request.provider_policy.requested_model,
        prompt_id: &request.prompt_id,
        declared_inputs_only: true,
        input: visible_input,
        output_schema: &request.provider_output_schema,
        output_schema_sha256: &request.provider_output_schema_sha256,
        schema_name: format!("mdp_{}", request.operation.replace([':', '/', '-'], "_")),
        timeout_ms: request.provider_policy.timeout_ms,
        max_output_tokens: provider_max_output_tokens(request.provider_policy.max_output_bytes),
    };
    let request_bytes = serde_json::to_vec(&native)?;
    if request_bytes.len() > MAX_NATIVE_SERIALIZED_REQUEST_BYTES {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "native-request-too-large",
        ));
    }
    let source = format!("{BUNDLED_NATIVE_DRIVER_SOURCE}\nawait main()\n");
    let mut command = Command::new(node);
    command
        .args(["--input-type=module", "--eval", &source])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env_clear();
    for name in ["OPENAI_API_KEY", "MDP_ALLOW_NATIVE_MODEL_CALLS"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let mut child = command
        .spawn()
        .map_err(|_| run_failure(RunFailureKind::RunnerFailed, "driver-start-failed"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| run_failure(RunFailureKind::RunnerFailed, "driver-stdin-failed"))?
        .write_all(&request_bytes)
        .map_err(|_| run_failure(RunFailureKind::RunnerFailed, "driver-stdin-failed"))?;
    let driver_stdout_limit = driver_stdout_limit(request.provider_policy.max_output_bytes)?;
    let (status, stdout_bytes) = supervise_child(
        &mut child,
        request.provider_policy.timeout_ms,
        driver_stdout_limit,
    )?;
    if !status.success() {
        return Err(run_failure(
            RunFailureKind::RunnerFailed,
            "driver-result-invalid",
        ));
    }
    let native_result: NativeSubprocessResultV1 = serde_json::from_slice(&stdout_bytes)
        .map_err(|_| run_failure(RunFailureKind::RunnerFailed, "driver-result-invalid"))?;
    if native_result.contract != NATIVE_SUBPROCESS_RESULT_V1
        || native_result.execution_id != request.execution_id
    {
        return Err(run_failure(
            RunFailureKind::RunnerFailed,
            "driver-result-invalid",
        ));
    }
    let result_output = native_result.output.map(|output| DriverOutputV2 {
        schema_id: "mdp.prompt-output.v0".into(),
        media_type: output.media_type,
        content_utf8: if output.encoding == "utf-8" {
            output.content
        } else {
            String::new()
        },
        byte_count: output.byte_count,
        sha256: output.sha256,
    });
    let observation =
        native_result
            .provider_observation
            .map(|observation| DriverProviderObservationV2 {
                provider: observation.provider,
                response_id: observation.response_id,
                resolved_model: observation.model,
            });
    let mut result = DriverResultV2 {
        contract: DRIVER_RESULT_V2.into(),
        execution_id: request.execution_id.clone(),
        operation: request.operation.clone(),
        terminal_state: native_result.terminal_state,
        output: result_output,
        provider_request_body_sha256: native_result.provider_request_body_sha256,
        provider_request_schema_id: native_result.provider_request_schema_id,
        provider_response_body_sha256: native_result.provider_response_body_sha256,
        provider_output_schema_sha256: native_result.provider_output_schema_sha256,
        provider_observation: observation,
        diagnostic_code: native_result.diagnostic_code,
        result_sha256: String::new(),
    };
    seal_driver_result(&mut result)?;
    Ok(result)
}

fn native_visible_input(
    prompt_id: &str,
    prompt_version: &str,
    prompt_sha256: &str,
    prompt_content: &str,
    invocation_sha256: &str,
    invocation_content: &str,
    inputs: &[(String, String, String)],
) -> String {
    let mut visible_input = String::new();
    visible_input.push_str("<mdp-prompt id=\"");
    visible_input.push_str(prompt_id);
    visible_input.push_str("\" version=\"");
    visible_input.push_str(prompt_version);
    visible_input.push_str("\" canonical_sha256=\"");
    visible_input.push_str(prompt_sha256);
    visible_input.push_str("\">\n");
    visible_input.push_str(prompt_content);
    visible_input.push_str("\n</mdp-prompt>\n<mdp-invocation sha256=\"");
    visible_input.push_str(invocation_sha256);
    visible_input.push_str("\">\n");
    visible_input.push_str(invocation_content);
    visible_input.push_str("\n</mdp-invocation>\n<mdp-host-input name=\"prompt_receipt\">\n");
    visible_input.push_str(invocation_content);
    visible_input
        .push_str("\n</mdp-host-input>\n<mdp-host-input name=\"invocation_receipt_sha256\">\n");
    visible_input.push_str(invocation_sha256);
    visible_input.push_str("\n</mdp-host-input>\n");
    for (logical_name, sha256, content) in inputs {
        visible_input.push_str("<mdp-declared-input name=\"");
        let visible_name = logical_name
            .strip_prefix("declared/")
            .and_then(|name| name.split_once('-'))
            .map_or(logical_name.as_str(), |(_, name)| name);
        visible_input.push_str(visible_name);
        visible_input.push_str("\" sha256=\"");
        visible_input.push_str(sha256);
        visible_input.push_str("\">\n");
        visible_input.push_str(content);
        visible_input.push_str("\n</mdp-declared-input>\n");
    }
    visible_input
}

fn provider_max_output_tokens(max_output_bytes: u64) -> u64 {
    // Four UTF-8 bytes per token is a conservative provider-side budget for
    // ordinary JSON. The local decoded-byte validation remains authoritative.
    (max_output_bytes / 4).clamp(1, 100_000)
}

fn driver_stdout_limit(max_output_bytes: u64) -> Result<u64> {
    // JSON may encode each decoded byte as a six-byte `\u00xx` escape. The
    // decoded DriverOutputV2 byte_count is still checked against the smaller
    // policy limit after parsing.
    max_output_bytes
        .checked_mul(6)
        .and_then(|limit| limit.checked_add(DRIVER_RESULT_ENVELOPE_BYTES))
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "driver-output-limit-invalid"))
}

fn supervise_child(
    child: &mut Child,
    timeout_ms: u64,
    stdout_limit: u64,
) -> Result<(ExitStatus, Vec<u8>)> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| run_failure(RunFailureKind::RunnerFailed, "driver-stdout-failed"))?;
    let (stdout_tx, stdout_rx) = mpsc::sync_channel(1);
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(stdout_limit.saturating_add(1))
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = stdout_tx.send(result);
    });
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(timeout_ms))
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "driver-timeout-invalid"))?;
    let (status, stdout_bytes) = loop {
        if let Ok(result) = stdout_rx.try_recv() {
            let bytes = result
                .map_err(|_| run_failure(RunFailureKind::RunnerFailed, "driver-stdout-failed"))?;
            if bytes.len() as u64 > stdout_limit {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(run_failure(
                    RunFailureKind::RunnerFailed,
                    "driver-result-too-large",
                ));
            }
            let status = child
                .wait()
                .map_err(|_| run_failure(RunFailureKind::RunnerFailed, "driver-wait-failed"))?;
            break (status, bytes);
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|_| run_failure(RunFailureKind::RunnerFailed, "driver-wait-failed"))?
        {
            let bytes = stdout_rx
                .recv_timeout(Duration::from_millis(100))
                .map_err(|_| run_failure(RunFailureKind::RunnerFailed, "driver-stdout-failed"))?
                .map_err(|_| run_failure(RunFailureKind::RunnerFailed, "driver-stdout-failed"))?;
            break (status, bytes);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(run_failure(RunFailureKind::RunnerFailed, "driver-timeout"));
        }
        thread::sleep(Duration::from_millis(5));
    };
    let _ = reader.join();
    Ok((status, stdout_bytes))
}

fn resolve_node_executable() -> Result<PathBuf> {
    let path = std::env::var_os("PATH")
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "node-runtime-not-found"))?;
    for directory in std::env::split_paths(&path) {
        let candidate = directory.join("node");
        let Ok(resolved) = fs::canonicalize(&candidate) else {
            continue;
        };
        let Ok(metadata) = fs::symlink_metadata(&resolved) else {
            continue;
        };
        if metadata.is_file() && !metadata.file_type().is_symlink() {
            return Ok(resolved);
        }
    }
    Err(run_failure(
        RunFailureKind::PolicyBlocked,
        "node-runtime-not-found",
    ))
}

pub(crate) fn execute_run(request: &RunRequestV1, output_root: &Path) -> Result<RunExecution> {
    execute_run_inner(request, output_root, || Ok(()))
}

fn execute_run_inner<F>(
    request: &RunRequestV1,
    output_root: &Path,
    before_post_check: F,
) -> Result<RunExecution>
where
    F: FnOnce() -> Result<()>,
{
    execute_run_inner_with_driver(
        request,
        output_root,
        before_post_check,
        invoke_native_driver,
    )
}

fn execute_run_inner_with_driver<F, D>(
    request: &RunRequestV1,
    output_root: &Path,
    before_post_check: F,
    driver: D,
) -> Result<RunExecution>
where
    F: FnOnce() -> Result<()>,
    D: FnOnce(&DriverRequestV2, &crate::run_contracts::DriverIdentity) -> Result<DriverResultV2>,
{
    validate_request(request)
        .map_err(|_| run_failure(RunFailureKind::Preflight, "request-policy-invalid"))?;
    let deadline = RunDeadline::new(request.execution_policy.timeout_ms);
    let final_dir = output_root;
    validate_output_outside_pack(request, final_dir)?;
    if final_dir.exists() {
        return Err(run_failure(
            RunFailureKind::Preflight,
            "output-directory-reused",
        ));
    }
    let parent = final_dir
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("creating run output parent {}", parent.display()))?;
    validate_output_outside_pack(request, final_dir)?;
    let leaf = final_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("run output directory must have a UTF-8 leaf name"))?;
    validate_output_leaf(leaf)?;
    let transaction_leaf = format!(".{leaf}.tmp-{:032x}", unique_suffix());
    let transaction_dir = parent.join(&transaction_leaf);
    let claim_path = parent.join(format!(".{leaf}.mdp-run.claim"));
    let claim_value = RunRecoveryClaim {
        contract: "mdp.run-recovery-claim.v1",
        execution_id: &request.execution_id,
        transaction_leaf: &transaction_leaf,
    };
    let mut claim_bytes = serde_json::to_vec(&claim_value)?;
    claim_bytes.push(b'\n');
    if claim_bytes.len() > MAX_RECOVERY_CLAIM_BYTES {
        return Err(run_failure(
            RunFailureKind::Preflight,
            "output-claim-invalid",
        ));
    }
    let mut claim = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&claim_path)
        .map_err(|_| run_failure(RunFailureKind::Preflight, "output-directory-claimed"))?;
    if claim
        .write_all(&claim_bytes)
        .and_then(|_| claim.sync_all())
        .is_err()
    {
        drop(claim);
        let _ = fs::remove_file(&claim_path);
        return Err(run_failure(
            RunFailureKind::RunnerFailed,
            "output-claim-failed",
        ));
    }
    drop(claim);
    let transaction_guard = TransactionGuard {
        transaction_dir: transaction_dir.clone(),
        claim_path,
    };
    fs::create_dir(&transaction_dir).with_context(|| {
        format!(
            "creating transaction directory {}",
            transaction_dir.display()
        )
    })?;
    set_private_directory(&transaction_dir)?;
    deadline.check()?;

    let (bundle_sha256, receipt) = match execute_transaction(
        request,
        &transaction_dir,
        &deadline,
        before_post_check,
        driver,
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            cleanup_failed_transaction(&transaction_dir)?;
            return Err(classify_execution_error(error));
        }
    };
    if request.mode == RunMode::Deterministic
        && let Err(error) = deadline.check()
    {
        cleanup_failed_transaction(&transaction_dir)?;
        return Err(error);
    }
    if let Err(error) = validate_output_outside_pack(request, final_dir) {
        let _ = cleanup_failed_transaction(&transaction_dir);
        return Err(error);
    }
    fs::remove_dir_all(transaction_dir.join("private"))
        .map_err(|_| run_failure(RunFailureKind::RunnerFailed, "private-cleanup-failed"))?;
    if let Err(error) = validate_output_outside_pack(request, final_dir) {
        let _ = cleanup_failed_transaction(&transaction_dir);
        return Err(error);
    }
    if fs::symlink_metadata(final_dir).is_ok() {
        return Err(run_failure(
            RunFailureKind::Preflight,
            "output-directory-reused",
        ));
    }
    fs::rename(&transaction_dir, &final_dir).with_context(|| {
        format!(
            "atomically committing run directory {}",
            final_dir.display()
        )
    })?;
    drop(transaction_guard);

    let authority_block = json!({
        "contract": "mdp.canonical-authority-block.v1",
        "execution_id": request.execution_id,
        "terminal_state": receipt.terminal_state,
        "decision": receipt.decision,
        "assurance": receipt.assurance,
        "limitations": receipt.limitations,
        "bundle_sha256": bundle_sha256,
        "receipt_sha256": receipt.receipt_sha256,
        "verification": {
            "bundle": output_root.join("run-bundle.json"),
            "receipt": output_root.join("run-receipt.json"),
            "artifact_root": output_root
        },
        "authority_notice": "Only this block and its hash-bound artifacts are authoritative; surrounding conversation commentary is outside the receipt."
    });
    let authority = SourceAuthority::from_run(
        receipt.terminal_state,
        receipt
            .decision
            .as_ref()
            .map(|decision| decision.decision.as_str()),
        receipt.output.is_some(),
    );
    debug_assert!(authority.permits_projection(
        authority.authority_level,
        authority.disposition,
        authority.governed_generation,
        ProjectionFidelity::Faithful,
    ));
    Ok(RunExecution {
        contract: "mdp.run-execution.v1".into(),
        valid: authority.disposition == crate::authority::DecisionDisposition::Allow,
        execution_id: request.execution_id.clone(),
        terminal_state: receipt.terminal_state,
        authority,
        run_dir: output_root.display().to_string(),
        bundle_sha256,
        receipt_sha256: receipt.receipt_sha256,
        authority_block,
    })
}

fn classify_execution_error(error: anyhow::Error) -> anyhow::Error {
    if error.downcast_ref::<RunFailure>().is_some() {
        error
    } else {
        run_failure(RunFailureKind::RunnerFailed, "run-execution-failed")
    }
}

fn cleanup_failed_transaction(transaction_dir: &Path) -> Result<()> {
    match fs::remove_dir_all(transaction_dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(run_failure(
            RunFailureKind::RunnerFailed,
            "private-cleanup-failed",
        )),
    }
}

fn execute_transaction<F, D>(
    request: &RunRequestV1,
    transaction_dir: &Path,
    deadline: &RunDeadline,
    before_post_check: F,
    driver: D,
) -> Result<(String, RunReceiptV1)>
where
    F: FnOnce() -> Result<()>,
    D: FnOnce(&DriverRequestV2, &crate::run_contracts::DriverIdentity) -> Result<DriverResultV2>,
{
    let private_dir = transaction_dir.join("private");
    let staged_pack = private_dir.join("pack");
    let staged_inputs = private_dir.join("inputs");
    let staged_prompt_dir = private_dir.join("prompt");
    let artifacts_dir = transaction_dir.join("artifacts");
    for directory in [
        &private_dir,
        &staged_pack,
        &staged_inputs,
        &staged_prompt_dir,
        &artifacts_dir,
    ] {
        fs::create_dir_all(directory)?;
        set_private_directory(directory)?;
    }

    let source_pack = Path::new(&request.pack_dir);
    validate_pack_source_bounds(source_pack)
        .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "pack-boundary-refused"))?;
    deadline.check()?;
    let source_snapshot = pack_content_snapshot(source_pack)?;
    validate_pack_snapshot_bounds(&source_snapshot)?;
    copy_pack(source_pack, &staged_pack)?;
    deadline.check()?;
    let staged_snapshot = pack_content_snapshot(&staged_pack)?;
    if source_snapshot != staged_snapshot {
        return Err(anyhow!("pack changed while it was being staged"));
    }
    let manifest = read_manifest(&staged_pack)?;
    let profile_id = manifest
        .profile
        .as_ref()
        .map(|profile| profile.id.as_str())
        .unwrap_or("gtm");
    if request.profile != profile_id {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "pack-profile-mismatch",
        ));
    }

    let staged = stage_inputs(request, &staged_inputs)
        .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "declared-input-refused"))?;
    deadline.check()?;
    let staged_prompt = match request.prompt.as_ref() {
        Some(prompt) => Some(stage_local_artifact(
            prompt,
            &staged_prompt_dir,
            0,
            request.execution_policy.max_input_bytes,
            "prompt",
        )?),
        None => None,
    };
    let total_staged_bytes = staged
        .iter()
        .try_fold(
            staged_prompt
                .as_ref()
                .map_or(0, |prompt| prompt.authority.byte_count),
            |total, input| total.checked_add(input.authority.byte_count),
        )
        .ok_or_else(|| anyhow!("declared input byte count overflow"))?;
    if total_staged_bytes > request.execution_policy.max_input_bytes {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "declared-input-refused",
        ));
    }
    verify_sources_unchanged(&staged)?;
    if let Some(prompt) = &staged_prompt {
        verify_sources_unchanged(std::slice::from_ref(prompt))?;
    }
    if pack_content_snapshot(source_pack)? != source_snapshot {
        return Err(anyhow!("pack changed while declared inputs were staged"));
    }

    let policy_hash = canonical_json_sha256_for_domain(
        "mdp.execution-policy.v1",
        &serde_json::to_value(&request.execution_policy)?,
    )?;
    let (prepared_native, bound_driver, bound_model, mut identity_observations, bound_model_facts) =
        if request.mode == RunMode::Generative {
            let prepared = prepare_native_request(
                request,
                &manifest,
                &staged_pack,
                staged_prompt.as_ref().expect("generative prompt validated"),
                &staged,
            )?;
            let (driver, model, observations, model_facts) =
                bind_native_identity(request, &prepared)?;
            (
                Some(prepared),
                Some(driver),
                Some(model),
                Some(observations),
                Some(model_facts),
            )
        } else {
            (None, None, None, None, None)
        };
    let bundle = RunBundleV1 {
        contract: RUN_BUNDLE_V1.into(),
        execution_id: request.execution_id.clone(),
        created_at: request.created_at.clone(),
        profile: request.profile.clone(),
        operation: request.operation.clone(),
        mode: request.mode,
        job_identity: request.job_identity.clone(),
        pack: PackAuthority {
            release_id: request.pack_release_id.clone(),
            pack_id: manifest.id.clone(),
            version: manifest.version.clone(),
            profile_id: profile_id.to_string(),
            portable_digest: staged_snapshot.sha256.clone(),
            files: staged_snapshot.files.clone(),
        },
        prompt: staged_prompt
            .as_ref()
            .map(|prompt| prompt.authority.clone()),
        inputs: staged.iter().map(|input| input.authority.clone()).collect(),
        execution_policy_sha256: policy_hash,
        driver: bound_driver.clone().or_else(|| request.driver.clone()),
        model: bound_model.clone().or_else(|| request.model.clone()),
        model_facts: bound_model_facts.clone(),
    };
    let bundle_value = serde_json::to_value(&bundle)?;
    let bundle_sha256 = canonical_json_sha256_for_domain(RUN_BUNDLE_V1, &bundle_value)?;
    write_json_create_new(&transaction_dir.join("run-bundle.json"), &bundle)?;

    let mut validation = None;
    let mut driver_request_sha256 = None;
    let mut driver_result_sha256 = None;
    let mut provider_request_body_sha256 = None;
    let mut provider_request_schema_id = None;
    let mut provider_response_body_sha256 = None;
    let mut provider_observation = None;
    let mut diagnostic_code = None;
    let (mut terminal_state, mut success_values) = if request.mode == RunMode::Generative {
        let prompt = staged_prompt.as_ref().ok_or_else(|| {
            run_failure(RunFailureKind::PolicyBlocked, "generative-prompt-missing")
        })?;
        let outcome = execute_generative_step(
            request,
            &staged_pack,
            prompt,
            &staged,
            &private_dir,
            &bundle,
            &bundle_sha256,
            prepared_native
                .as_ref()
                .expect("generative preparation exists"),
            bound_driver.as_ref().expect("bound driver identity exists"),
            deadline,
            driver,
        )?;
        provider_request_body_sha256 = outcome.provider_request_body_sha256.clone();
        provider_request_schema_id = outcome.provider_request_schema_id.clone();
        provider_response_body_sha256 = outcome.provider_response_body_sha256.clone();
        provider_observation = outcome.provider_observation.clone();
        diagnostic_code = outcome.diagnostic_code.clone();
        driver_request_sha256 = Some(outcome.driver_request_sha256);
        driver_result_sha256 = Some(outcome.driver_result_sha256);
        validation = outcome.validation;
        if let Some(observations) = identity_observations.as_mut() {
            observations.provider_request = ProviderRequestObservationV1 {
                provider_request_body_sha256: outcome.provider_request_body_sha256.clone(),
                provider_request_schema_id: outcome.provider_request_schema_id.clone(),
                relation: if outcome.provider_request_body_sha256.is_some()
                    && outcome.provider_request_schema_id.is_some()
                {
                    PROVIDER_REQUEST_RELATION_V1.into()
                } else {
                    PROVIDER_REQUEST_NOT_OBSERVED_V1.into()
                },
            };
        }
        (outcome.terminal_state, outcome.success)
    } else if request.profile == PROPOSAL_PROFILE && request.operation == VALIDATE_EXISTING_OUTPUT {
        let prompt_output = required_typed_input(
            &staged,
            "prompt-output",
            "mdp.prompt-output.v0",
            "application/json",
        )?;
        let source_audit = optional_input(&staged, "source-audit");
        if let Some(input) = source_audit {
            validate_input_type(input, "mdp.source-audit.v0", "application/json")?;
        }
        let source_attempt = optional_input(&staged, "source-attempt-request");
        let attempt_results = optional_input(&staged, "collected-attempt-results");
        let result = validate_prompt_output_file_with_inputs(
            &staged_pack,
            &prompt_output.staged_path,
            None,
            Some("normalize-opportunity"),
            source_audit.map(|input| input.staged_path.as_path()),
            source_attempt.map(|input| input.staged_path.as_path()),
            attempt_results.map(|input| input.staged_path.as_path()),
            None,
            None,
        )?;
        let valid = result["valid"].as_bool() == Some(true);
        validation = Some(result.clone());
        if valid {
            (
                TerminalState::Success,
                Some(success_artifacts(
                    request,
                    &bundle,
                    &bundle_sha256,
                    &prompt_output.staged_path,
                    result,
                )?),
            )
        } else {
            (TerminalState::NoDraftOutputInvalid, None)
        }
    } else if request.profile == GTM_PROFILE && request.operation == QUALIFY {
        let normalized = required_input(&staged, "normalized-decision-input")?;
        if normalized.authority.media_type != "application/json"
            || !matches!(
                normalized.authority.schema_id.as_str(),
                NORMALIZED_DECISION_INPUT_CONTRACT | NORMALIZED_DECISION_INPUT_CONTRACT_V2
            )
        {
            return Err(anyhow!("declared input schema or media type mismatch"));
        }
        let signal_aware = normalized.authority.schema_id == NORMALIZED_DECISION_INPUT_CONTRACT_V2;
        let (source_attempt_schema, collected_results_schema) =
            gtm_lineage_schema_ids(signal_aware);
        let source_attempt = required_typed_input(
            &staged,
            "source-attempt-request",
            source_attempt_schema,
            "application/json",
        )?;
        let attempt_results = required_typed_input(
            &staged,
            "collected-attempt-results",
            collected_results_schema,
            "application/json",
        )?;
        let bound_prompt =
            required_typed_input(&staged, "bound-prompt", "mdp.prompt.v0", "application/yaml")?;
        let normalized_value: Value = serde_json::from_slice(&fs::read(&normalized.staged_path)?)?;
        let normalized_job_id = normalized_value["job_id"]
            .as_str()
            .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "normalized-job-missing"))?;
        let ingress = resolve_job_ingress(&manifest, Some(normalized_job_id))
            .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "job-ingress-invalid"))?;
        let prompt_manifest_path = normalized_value["normalization"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item["prompt"].as_str())
            .ok_or_else(|| anyhow!("normalized decision input omits its bound prompt path"))?;
        let staged_bound_prompt = resolve_pack_path(&staged_pack, prompt_manifest_path)?;
        if sha256_hex(&fs::read(&staged_bound_prompt)?) != bound_prompt.initial_sha256 {
            return Err(anyhow!(
                "declared bound prompt does not match the prompt in the immutable pack snapshot"
            ));
        }
        let source_binding = if signal_aware {
            Some(required_typed_input(
                &staged,
                "source-binding",
                SOURCE_BINDING_CONTRACT_V2,
                "application/json",
            )?)
        } else {
            None
        };
        let result = if signal_aware {
            validate_prompt_output_file_with_lineage_inputs(
                &staged_pack,
                &normalized.staged_path,
                Some(&staged_bound_prompt),
                None,
                None,
                source_binding.map(|input| input.staged_path.as_path()),
                Some(&source_attempt.staged_path),
                Some(&attempt_results.staged_path),
                None,
                None,
            )?
        } else {
            validate_prompt_output_file_with_inputs(
                &staged_pack,
                &normalized.staged_path,
                Some(&staged_bound_prompt),
                None,
                None,
                Some(&source_attempt.staged_path),
                Some(&attempt_results.staged_path),
                None,
                None,
            )?
        };
        let ready = governed_normalization_outcome(
            result["valid"].as_bool() == Some(true),
            signal_aware,
            normalized_value["outcome"].as_str(),
        );
        validation = Some(result);
        if ready {
            let prospect = &normalized_value["normalized_prospect"];
            if !prospect.is_object() {
                (TerminalState::NoDraftDecisionInvalid, None)
            } else {
                let prospect_path = private_dir.join("projected-prospect.json");
                write_json_create_new(&prospect_path, prospect)?;
                let fit_result = if signal_aware {
                    fit_normalized(
                        &staged_pack,
                        &normalized.staged_path,
                        &staged_bound_prompt,
                        &source_binding.expect("v2 source binding").staged_path,
                        &source_attempt.staged_path,
                        &attempt_results.staged_path,
                        Some(normalized_job_id),
                    )?
                } else if ingress
                    .as_ref()
                    .is_some_and(|ingress| ingress.is_governed())
                {
                    let prospect: crate::models::Prospect =
                        serde_json::from_value(prospect.clone()).map_err(|_| {
                            run_failure(
                                RunFailureKind::PolicyBlocked,
                                "normalized-prospect-invalid",
                            )
                        })?;
                    fit_prospect_with_governed_authority(
                        &staged_pack,
                        prospect,
                        normalized_job_id,
                        json!({
                            "normalized_output_sha256": normalized.initial_sha256,
                            "source_attempt_request_sha256": source_attempt.initial_sha256,
                            "collected_attempt_results_sha256": attempt_results.initial_sha256
                        }),
                    )?
                } else {
                    fit(&staged_pack, &prospect_path)?
                };
                (
                    TerminalState::Success,
                    Some(gtm_success_artifacts(
                        request,
                        &bundle,
                        &bundle_sha256,
                        fit_result,
                    )?),
                )
            }
        } else {
            (TerminalState::NoDraftOutputInvalid, None)
        }
    } else {
        (TerminalState::NoDraftPolicyBlocked, None)
    };
    if request.mode == RunMode::Deterministic {
        deadline.check()?;
    } else if deadline.expired() {
        terminal_state = TerminalState::NoDraftRunnerFailed;
        success_values = None;
        validation = None;
    }

    before_post_check()?;
    if request.mode == RunMode::Deterministic {
        deadline.check()?;
    } else if deadline.expired() {
        terminal_state = TerminalState::NoDraftRunnerFailed;
        success_values = None;
        validation = None;
    }
    let staged_pack_after = pack_content_snapshot(&staged_pack)?;
    let source_pack_after = pack_content_snapshot(source_pack)?;
    let sources_unchanged = verify_sources_unchanged(&staged).is_ok()
        && staged_prompt
            .as_ref()
            .is_none_or(|prompt| verify_sources_unchanged(std::slice::from_ref(prompt)).is_ok());
    if staged_pack_after != staged_snapshot
        || source_pack_after != source_snapshot
        || !sources_unchanged
    {
        terminal_state = TerminalState::NoDraftAuditIncomplete;
        success_values = None;
    }

    let validation_authority = if let Some(value) = validation {
        let path = artifacts_dir.join("validation.json");
        write_json_create_new(&path, &value)?;
        Some(authority_for_file(
            "artifacts/validation.json",
            "mdp.validate-prompt-output.v0",
            "application/json",
            &path,
            EvidenceProvenance::MdpObserved,
            vec![bundle_sha256.clone()],
        )?)
    } else {
        None
    };

    let assurance = assurance_dimensions(
        request.mode,
        terminal_state,
        &bundle_sha256,
        staged_pack_after == staged_snapshot
            && source_pack_after == source_snapshot
            && sources_unchanged,
    );
    let audit = RunnerAuditV1 {
        contract: RUNNER_AUDIT_V1.into(),
        execution_id: request.execution_id.clone(),
        runner_version: env!("CARGO_PKG_VERSION").into(),
        runner_build_sha256: option_env!("MDP_BUILD_SHA256").map(str::to_string),
        platform: std::env::consts::OS.into(),
        snapshot_sha256: bundle_sha256.clone(),
        driver_request_sha256,
        driver_result_sha256,
        provider_request_body_sha256,
        provider_request_schema_id,
        provider_response_body_sha256,
        provider_observation,
        identity_observations,
        diagnostic_code,
        terminal_state,
        assurance: assurance.clone(),
        limitations: vec![
            if request.mode == RunMode::Deterministic {
                "local deterministic validation does not attest to authoring-context provenance".into()
            } else {
                "model output authority is granted only after local deterministic contract validation".into()
            },
            "host-level filesystem and process isolation remain operator-owned".into(),
            "timeout_ms is enforced at bounded runtime phase boundaries; blocking filesystem calls are not preempted"
                .into(),
            "pack_release_id is caller-supplied; MDP observes and binds the portable pack digest"
                .into(),
            "local receipt hashes provide integrity, not signer identity or non-repudiation".into(),
        ],
    };
    let audit_path = transaction_dir.join("runner-audit.json");
    write_json_create_new(&audit_path, &audit)?;
    let audit_authority = authority_for_file(
        "runner-audit.json",
        RUNNER_AUDIT_V1,
        "application/json",
        &audit_path,
        EvidenceProvenance::MdpObserved,
        vec![bundle_sha256.clone()],
    )?;

    let (output, decision, compiled_context) = match success_values {
        Some(values) if terminal_state.is_success() => {
            if values.output_bytes.len() as u64 > request.execution_policy.max_output_bytes {
                return Err(anyhow!("run output exceeds execution policy byte limit"));
            }
            let output_path = artifacts_dir.join("output.json");
            write_bytes_create_new(&output_path, &values.output_bytes)?;
            let output = authority_for_file(
                "artifacts/output.json",
                &values.output_schema_id,
                "application/json",
                &output_path,
                EvidenceProvenance::MdpObserved,
                vec![bundle_sha256.clone()],
            )?;
            let context_path = artifacts_dir.join("compiled-context.json");
            write_json_create_new(&context_path, &values.compiled_context)?;
            let compiled = authority_for_file(
                "artifacts/compiled-context.json",
                "mdp.compiled-run-context.v1",
                "application/json",
                &context_path,
                EvidenceProvenance::MdpObserved,
                vec![bundle_sha256.clone()],
            )?;
            (Some(output), Some(values.decision), Some(compiled))
        }
        _ => (None, None, None),
    };

    let mut receipt = RunReceiptV1 {
        contract: RUN_RECEIPT_V1.into(),
        execution_id: request.execution_id.clone(),
        created_at: request.created_at.clone(),
        profile: request.profile.clone(),
        operation: request.operation.clone(),
        job_identity: request.job_identity.clone(),
        bundle_sha256: bundle_sha256.clone(),
        terminal_state,
        output,
        decision,
        compiled_context,
        validation: validation_authority,
        runner_audit: audit_authority,
        assurance,
        limitations: audit.limitations,
        receipt_sha256: String::new(),
    };
    receipt.receipt_sha256 =
        canonical_json_sha256_for_domain(RUN_RECEIPT_V1, &serde_json::to_value(&receipt)?)?;
    write_json_create_new(&transaction_dir.join("run-receipt.json"), &receipt)?;
    let verification = crate::commands::run_verification::verify_run_files(
        Some(&transaction_dir.join("run-bundle.json")),
        &transaction_dir.join("run-receipt.json"),
        Some(transaction_dir),
    )?;
    if verification["valid"].as_bool() != Some(true) {
        return Err(anyhow!(
            "internal run verification failed before artifact publication"
        ));
    }
    Ok((bundle_sha256, receipt))
}

fn gtm_lineage_schema_ids(signal_aware: bool) -> (&'static str, &'static str) {
    if signal_aware {
        (
            SOURCE_ATTEMPT_REQUEST_CONTRACT_V2,
            COLLECTED_ATTEMPT_RESULTS_CONTRACT_V2,
        )
    } else {
        (
            "mdp.source-attempt-request.v1",
            "mdp.collected-attempt-results.v1",
        )
    }
}

fn governed_normalization_outcome(valid: bool, signal_aware: bool, outcome: Option<&str>) -> bool {
    valid && (outcome == Some("ready") || (signal_aware && outcome == Some("disqualified")))
}

struct GenerativeOutcome {
    terminal_state: TerminalState,
    success: Option<SuccessArtifacts>,
    validation: Option<Value>,
    provider_request_body_sha256: Option<String>,
    provider_request_schema_id: Option<String>,
    provider_response_body_sha256: Option<String>,
    provider_observation: Option<DriverProviderObservationV2>,
    diagnostic_code: Option<String>,
    driver_request_sha256: String,
    driver_result_sha256: String,
}

#[allow(clippy::too_many_arguments)]
fn execute_generative_step<D>(
    request: &RunRequestV1,
    staged_pack: &Path,
    staged_prompt: &StagedInput,
    staged_inputs: &[StagedInput],
    private_dir: &Path,
    bundle: &RunBundleV1,
    bundle_sha256: &str,
    prepared: &PreparedNativeRequest,
    driver_identity: &crate::run_contracts::DriverIdentity,
    deadline: &RunDeadline,
    driver: D,
) -> Result<GenerativeOutcome>
where
    D: FnOnce(&DriverRequestV2, &crate::run_contracts::DriverIdentity) -> Result<DriverResultV2>,
{
    let identity = request
        .job_identity
        .as_ref()
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "job-identity-required"))?;
    let invocation_path = private_dir.join("prompt-invocation.json");
    write_json_create_new(&invocation_path, &prepared.invocation_value)?;
    let invocation_bytes = prepared.invocation_bytes.clone();
    let invocation_authority = ArtifactAuthority {
        logical_name: "private/prompt-invocation.json".into(),
        schema_id: "mdp.prompt-invocation.v1".into(),
        media_type: "application/json".into(),
        byte_count: invocation_bytes.len() as u64,
        sha256: prepared.invocation_sha256.clone(),
        provenance: EvidenceProvenance::MdpObserved,
        provenance_refs: vec![bundle_sha256.into()],
    };

    let model = request
        .model
        .as_ref()
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "model-identity-required"))?;
    let driver_timeout_ms = deadline.driver_timeout_ms();
    let policy_hash = bundle.execution_policy_sha256.clone();
    let prompt_content = utf8_staged_content(staged_prompt, "prompt")?;
    let mut driver_request = DriverRequestV2 {
        contract: DRIVER_REQUEST_V2.into(),
        execution_id: request.execution_id.clone(),
        profile: request.profile.clone(),
        operation: request.operation.clone(),
        job_identity: identity.clone(),
        phase: prepared.step.phase.as_str().into(),
        prompt_id: prepared.step.prompt_id.clone(),
        prompt_version: prepared.step.prompt_version.clone(),
        prompt_canonical_sha256: prepared.step.prompt_sha256.clone(),
        prompt: DriverArtifactV2 {
            authority: staged_prompt.authority.clone(),
            content_utf8: prompt_content,
        },
        prompt_invocation: DriverArtifactV2 {
            authority: invocation_authority,
            content_utf8: std::str::from_utf8(&invocation_bytes)
                .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "invocation-not-utf8"))?
                .to_string(),
        },
        inputs: staged_inputs
            .iter()
            .map(|input| {
                Ok(DriverArtifactV2 {
                    authority: input.authority.clone(),
                    content_utf8: utf8_staged_content(input, "declared-input")?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        canonical_output_schema: prepared.canonical_output_schema.clone(),
        canonical_output_schema_sha256: prepared.canonical_output_schema_sha256.clone(),
        provider_output_schema: prepared.provider_output_schema.clone(),
        provider_output_schema_sha256: prepared.provider_output_schema_sha256.clone(),
        provider_policy: DriverProviderPolicyV2 {
            provider: model.provider.clone(),
            requested_model: model.requested_model.clone(),
            authorized_endpoint: model.authorized_endpoint.clone(),
            timeout_ms: driver_timeout_ms.unwrap_or(1),
            max_output_bytes: request.execution_policy.max_output_bytes,
        },
        execution_policy_sha256: policy_hash,
        request_sha256: String::new(),
    };
    seal_driver_request(&mut driver_request)?;

    if driver_timeout_ms.is_none() {
        return failed_generative_outcome(driver_request, "driver_budget_exhausted");
    }
    let result = match driver(&driver_request, driver_identity) {
        Ok(result) => result,
        Err(_) => {
            return failed_generative_outcome(driver_request, "driver_invocation_failed");
        }
    };
    if deadline.expired() {
        return failed_generative_outcome(driver_request, "driver_deadline_exhausted");
    }
    if validate_driver_result(&driver_request, &result).is_err() {
        return Ok(GenerativeOutcome {
            terminal_state: TerminalState::NoDraftRunnerFailed,
            success: None,
            validation: None,
            provider_request_body_sha256: None,
            provider_request_schema_id: None,
            provider_response_body_sha256: None,
            provider_observation: None,
            diagnostic_code: None,
            driver_request_sha256: driver_request.request_sha256,
            driver_result_sha256: result.result_sha256,
        });
    }
    if !result.terminal_state.is_success() {
        return Ok(GenerativeOutcome {
            terminal_state: result.terminal_state,
            success: None,
            validation: None,
            provider_request_body_sha256: result.provider_request_body_sha256,
            provider_request_schema_id: result.provider_request_schema_id,
            provider_response_body_sha256: result.provider_response_body_sha256,
            provider_observation: result.provider_observation,
            diagnostic_code: None,
            driver_request_sha256: driver_request.request_sha256,
            driver_result_sha256: result.result_sha256,
        });
    }
    let output = result.output.as_ref().expect("validated success output");
    let output_path = private_dir.join("driver-output.json");
    let output_bytes = if prepared.step.output_contract.host_envelope.is_some() {
        match host_wrap_governed_output(
            &prepared.step,
            staged_inputs,
            &prepared.invocation_value,
            &prepared.invocation_bytes,
            &output.content_utf8,
            &prepared.canonical_output_schema,
        ) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Ok(host_envelope_failure_outcome(
                    &driver_request,
                    result,
                    sanitized_host_envelope_diagnostic(&error),
                ));
            }
        }
    } else {
        output.content_utf8.as_bytes().to_vec()
    };
    write_bytes_create_new(&output_path, &output_bytes)?;
    let routed_context = optional_input(staged_inputs, "routed_context")
        .or_else(|| optional_input(staged_inputs, "routed-context"));
    let validation = validate_prompt_output_file_with_lineage_inputs(
        staged_pack,
        &output_path,
        Some(&staged_prompt.staged_path),
        None,
        optional_input(staged_inputs, "source-audit").map(|item| item.staged_path.as_path()),
        optional_input(staged_inputs, "source-binding")
            .or_else(|| optional_input(staged_inputs, "source_binding"))
            .map(|item| item.staged_path.as_path()),
        optional_input(staged_inputs, "source-attempt-request")
            .map(|item| item.staged_path.as_path()),
        optional_input(staged_inputs, "collected-attempt-results")
            .map(|item| item.staged_path.as_path()),
        Some(&invocation_path),
        routed_context.map(|item| item.staged_path.as_path()),
    )?;
    let valid = validation["valid"].as_bool() == Some(true);
    Ok(GenerativeOutcome {
        terminal_state: if valid {
            TerminalState::Success
        } else {
            TerminalState::NoDraftOutputInvalid
        },
        success: if valid {
            Some(generative_success_artifacts(
                request,
                bundle,
                bundle_sha256,
                &output_path,
                validation.clone(),
            )?)
        } else {
            None
        },
        validation: if valid { Some(validation) } else { None },
        provider_request_body_sha256: result.provider_request_body_sha256,
        provider_request_schema_id: result.provider_request_schema_id,
        provider_response_body_sha256: result.provider_response_body_sha256,
        provider_observation: result.provider_observation,
        diagnostic_code: None,
        driver_request_sha256: driver_request.request_sha256,
        driver_result_sha256: result.result_sha256,
    })
}

fn failed_generative_outcome(
    driver_request: DriverRequestV2,
    diagnostic_code: &str,
) -> Result<GenerativeOutcome> {
    let mut failed_result = DriverResultV2 {
        contract: DRIVER_RESULT_V2.into(),
        execution_id: driver_request.execution_id.clone(),
        operation: driver_request.operation.clone(),
        terminal_state: TerminalState::NoDraftRunnerFailed,
        output: None,
        provider_request_body_sha256: None,
        provider_request_schema_id: None,
        provider_response_body_sha256: None,
        provider_output_schema_sha256: Some(driver_request.provider_output_schema_sha256.clone()),
        provider_observation: None,
        diagnostic_code: Some(diagnostic_code.into()),
        result_sha256: String::new(),
    };
    seal_driver_result(&mut failed_result)?;
    Ok(GenerativeOutcome {
        terminal_state: TerminalState::NoDraftRunnerFailed,
        success: None,
        validation: None,
        provider_request_body_sha256: None,
        provider_request_schema_id: None,
        provider_response_body_sha256: None,
        provider_observation: None,
        diagnostic_code: Some(diagnostic_code.into()),
        driver_request_sha256: driver_request.request_sha256,
        driver_result_sha256: failed_result.result_sha256,
    })
}

fn host_envelope_failure_outcome(
    driver_request: &DriverRequestV2,
    result: DriverResultV2,
    diagnostic_code: &'static str,
) -> GenerativeOutcome {
    GenerativeOutcome {
        terminal_state: TerminalState::NoDraftOutputInvalid,
        success: None,
        validation: None,
        provider_request_body_sha256: result.provider_request_body_sha256,
        provider_request_schema_id: result.provider_request_schema_id,
        provider_response_body_sha256: result.provider_response_body_sha256,
        provider_observation: result.provider_observation,
        diagnostic_code: Some(diagnostic_code.into()),
        driver_request_sha256: driver_request.request_sha256.clone(),
        driver_result_sha256: result.result_sha256,
    }
}

fn sanitized_host_envelope_diagnostic(error: &anyhow::Error) -> &'static str {
    let code = error
        .downcast_ref::<RunFailure>()
        .map(RunFailure::code)
        .unwrap_or("host-envelope-failed");
    match code {
        "host-envelope-metadata-missing"
        | "host-envelope-metadata-invalid"
        | "semantic-output-malformed"
        | "semantic-output-not-object"
        | "host-owned-field-injection"
        | "semantic-output-invalid"
        | "host-context-source-missing"
        | "semantic-output-missing" => code,
        _ => "host-envelope-failed",
    }
}

fn generative_success_artifacts(
    request: &RunRequestV1,
    bundle: &RunBundleV1,
    bundle_sha256: &str,
    output_path: &Path,
    validation: Value,
) -> Result<SuccessArtifacts> {
    let output_bytes = fs::read(output_path)?;
    let compiled_context = json!({
        "contract": "mdp.compiled-run-context.v1",
        "execution_id": request.execution_id,
        "profile": request.profile,
        "operation": request.operation,
        "job_identity": request.job_identity,
        "bundle_sha256": bundle_sha256,
        "pack_portable_digest": bundle.pack.portable_digest,
        "prompt_sha256": bundle.prompt.as_ref().map(|prompt| prompt.sha256.as_str()),
        "declared_input_sha256": bundle.inputs.iter().map(|input| json!({
            "logical_name": input.logical_name,
            "sha256": input.sha256
        })).collect::<Vec<_>>(),
        "validation_contract": validation["contract"].as_str().unwrap_or("mdp.validate-prompt-output.v0"),
        "drafting_authority": "governed-output-only",
    });
    let mut decision = DecisionAuthority {
        schema_id: "mdp.model-step-decision.v1".into(),
        decision: "governed-output".into(),
        reason_codes: vec!["validation-passed".into()],
        sha256: String::new(),
    };
    decision.sha256 =
        canonical_json_sha256_for_domain(&decision.schema_id, &serde_json::to_value(&decision)?)?;
    Ok(SuccessArtifacts {
        output_bytes,
        output_schema_id: "mdp.prompt-output.v0".into(),
        compiled_context,
        decision,
    })
}

fn validate_selected_prompt(
    staged_pack: &Path,
    staged_prompt: &StagedInput,
    step: &CompiledModelStepV1,
) -> Result<()> {
    let canonical_path = staged_pack.join(".mdp").join(&step.prompt_path);
    let canonical_bytes = fs::read(canonical_path)
        .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "selected-prompt-missing"))?;
    let staged_bytes = fs::read(&staged_prompt.staged_path)?;
    if canonical_bytes != staged_bytes {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "selected-prompt-mismatch",
        ));
    }
    Ok(())
}

fn canonical_output_schema_for_step(
    staged_pack: &Path,
    job_id: &str,
    step: &CompiledModelStepV1,
) -> Result<Value> {
    if let Some(schema) = &step.output_contract.schema {
        return Ok(schema.clone());
    }
    let schema_ref = step
        .output_contract
        .schema_ref
        .as_deref()
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "output-schema-missing"))?;
    if matches!(
        schema_ref,
        NORMALIZED_DECISION_INPUT_CONTRACT | NORMALIZED_DECISION_INPUT_CONTRACT_V2
    ) {
        let compiled = requirements(staged_pack, job_id)
            .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "job-readiness-unavailable"))?;
        let schema = compiled
            .get("normalized_output_schema")
            .filter(|value| value.is_object())
            .cloned()
            .ok_or_else(|| {
                run_failure(
                    RunFailureKind::PolicyBlocked,
                    "output-schema-ref-unsupported",
                )
            })?;
        if schema["properties"]["contract"]["const"] != schema_ref {
            return Err(run_failure(
                RunFailureKind::PolicyBlocked,
                "output-schema-ref-unsupported",
            ));
        }
        return Ok(schema);
    }
    prompt_output_schema_for_ref(schema_ref).ok_or_else(|| {
        run_failure(
            RunFailureKind::PolicyBlocked,
            "output-schema-ref-unsupported",
        )
    })
}

fn validate_generative_job_gates(
    staged_pack: &Path,
    job_id: &str,
    phase: ModelStepPhase,
) -> Result<()> {
    let compiled = requirements(staged_pack, job_id)
        .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "job-readiness-unavailable"))?;
    if compiled["valid"] != true
        || compiled["profile_activation"]["status"] == "blocked"
        || compiled["product_foundation"]["status"] == "blocked"
        || compiled["model_steps"]["status"] != "ready"
        || (phase != ModelStepPhase::Normalization
            && (compiled["status"] != "ready" || compiled["draft_allowed"] == false))
    {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "job-readiness-blocked",
        ));
    }
    Ok(())
}

fn validate_generative_input_gates(
    staged_pack: &Path,
    manifest: &crate::models::Manifest,
    staged: &[StagedInput],
    job: &str,
) -> Result<()> {
    for input in staged.iter().filter(|input| {
        matches!(
            input.logical_name.as_str(),
            "routed_context" | "routed-context"
        )
    }) {
        validate_input_type(input, ROUTED_CONTEXT_CONTRACT, "application/json")
            .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "routed-context-invalid"))?;
        let bytes = fs::read(&input.staged_path)
            .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "routed-context-invalid"))?;
        let validation = crate::routing::validate_routed_context_bytes_for_job(
            staged_pack,
            manifest,
            &bytes,
            job,
        )
        .map_err(|error| {
            let code = match error.kind() {
                crate::routing::RoutedContextValidationKind::ReadinessBlocked => {
                    "draft-readiness-blocked"
                }
                _ => "routed-context-invalid",
            };
            run_failure(RunFailureKind::PolicyBlocked, code)
        })?;
        if validation.sha256 != input.authority.sha256 {
            return Err(run_failure(
                RunFailureKind::PolicyBlocked,
                "routed-context-invalid",
            ));
        }
    }
    Ok(())
}

fn validate_step_inputs(step: &CompiledModelStepV1, staged: &[StagedInput]) -> Result<()> {
    let declared = step
        .declared_inputs
        .iter()
        .map(|input| input.name.as_str())
        .collect::<HashSet<_>>();
    if staged.iter().any(|input| {
        is_host_invocation_metadata(&input.logical_name)
            || !declared.contains(input.logical_name.as_str())
    }) {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "undeclared-model-input",
        ));
    }
    if step.declared_inputs.iter().any(|input| {
        input.required
            && !is_host_invocation_metadata(&input.name)
            && !staged.iter().any(|item| item.logical_name == input.name)
    }) {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "required-model-input-missing",
        ));
    }
    Ok(())
}

fn is_host_invocation_metadata(name: &str) -> bool {
    matches!(
        name,
        "prompt_receipt"
            | "prompt-receipt"
            | "invocation_receipt_sha256"
            | "invocation-receipt-sha256"
    )
}

fn utf8_staged_content(input: &StagedInput, code: &'static str) -> Result<String> {
    String::from_utf8(fs::read(&input.staged_path)?)
        .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, code))
}

fn seal_driver_request(request: &mut DriverRequestV2) -> Result<()> {
    request.request_sha256.clear();
    request.request_sha256 =
        canonical_json_sha256_for_domain(DRIVER_REQUEST_V2, &serde_json::to_value(&*request)?)?;
    Ok(())
}

fn seal_driver_result(result: &mut DriverResultV2) -> Result<()> {
    result.result_sha256.clear();
    result.result_sha256 =
        canonical_json_sha256_for_domain(DRIVER_RESULT_V2, &serde_json::to_value(&*result)?)?;
    Ok(())
}

fn is_canonical_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn validate_driver_result(request: &DriverRequestV2, result: &DriverResultV2) -> Result<()> {
    if result.contract != DRIVER_RESULT_V2
        || result.execution_id != request.execution_id
        || result.operation != request.operation
    {
        return Err(anyhow!("driver result authority mismatch"));
    }
    let mut sealed = result.clone();
    let expected_hash = sealed.result_sha256.clone();
    seal_driver_result(&mut sealed)?;
    if sealed.result_sha256 != expected_hash {
        return Err(anyhow!("driver result hash mismatch"));
    }
    if result.provider_output_schema_sha256.as_deref()
        != Some(request.provider_output_schema_sha256.as_str())
    {
        return Err(anyhow!("driver provider schema hash mismatch"));
    }
    if let Some(observation) = &result.provider_observation
        && (observation.provider != request.provider_policy.provider
            || observation
                .resolved_model
                .as_deref()
                .is_some_and(|model| model.trim().is_empty()))
    {
        return Err(anyhow!("driver provider observation mismatch"));
    }
    match (&result.terminal_state, &result.output) {
        (TerminalState::Success, Some(output)) => {
            if !result
                .provider_request_body_sha256
                .as_deref()
                .is_some_and(is_canonical_sha256)
                || result.provider_request_schema_id.as_deref()
                    != Some(OPENAI_PROVIDER_REQUEST_SCHEMA_ID)
                || !result
                    .provider_response_body_sha256
                    .as_deref()
                    .is_some_and(is_canonical_sha256)
                || result
                    .provider_observation
                    .as_ref()
                    .is_none_or(|observation| {
                        observation
                            .resolved_model
                            .as_deref()
                            .is_none_or(|model| model.trim().is_empty())
                    })
                || output.media_type != "application/json"
                || output.byte_count != output.content_utf8.len() as u64
                || output.sha256 != sha256_hex(output.content_utf8.as_bytes())
                || output.byte_count > request.provider_policy.max_output_bytes
            {
                return Err(anyhow!("driver output authority mismatch"));
            }
        }
        (TerminalState::Success, None) => return Err(anyhow!("driver success output missing")),
        (_, Some(_)) => return Err(anyhow!("failed driver result carries output")),
        (_, None) => {}
    }
    Ok(())
}

fn project_output_schema_for_openai(schema: &Value) -> Result<Value> {
    let projected = project_schema_node(schema)?;
    if projected.as_object().is_none_or(|object| object.is_empty()) {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "provider-schema-projection-empty",
        ));
    }
    Ok(projected)
}

fn provider_schema_source(schema: &Value, required_top_level: &[String]) -> Result<Value> {
    let mut source = schema.clone();
    let object = source.as_object_mut().ok_or_else(|| {
        run_failure(
            RunFailureKind::PolicyBlocked,
            "canonical-output-schema-not-object",
        )
    })?;
    let properties = object
        .get_mut("properties")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            run_failure(
                RunFailureKind::PolicyBlocked,
                "canonical-output-schema-properties-missing",
            )
        })?;
    if required_top_level
        .iter()
        .any(|field| !properties.contains_key(field))
    {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "prompt-required-output-field-missing-from-schema",
        ));
    }
    properties.retain(|field, _| required_top_level.contains(field));
    object.insert("required".into(), json!(required_top_level));
    Ok(source)
}

fn provider_schema_source_for_contract(
    schema: &Value,
    contract: &crate::models::PromptOutputContract,
) -> Result<Value> {
    if let Some(host_envelope) = contract.host_envelope.as_ref() {
        host_envelope
            .validate(
                Some("governed-artifact"),
                true,
                &contract.required_top_level,
            )
            .map_err(|_| {
                run_failure(
                    RunFailureKind::PolicyBlocked,
                    "host-envelope-contract-invalid",
                )
            })?;
        return provider_schema_source(schema, &host_envelope.semantic_required_top_level);
    }
    if contract.schema_ref.is_some() {
        let example = required_output_example(&contract.example, &contract.required_top_level)?;
        Ok(schema_for_example_shape(schema, &example))
    } else {
        provider_schema_source(schema, &contract.required_top_level)
    }
}

fn host_wrap_governed_output(
    step: &CompiledModelStepV1,
    staged_inputs: &[StagedInput],
    invocation_value: &Value,
    invocation_bytes: &[u8],
    model_output: &str,
    canonical_schema: &Value,
) -> Result<Vec<u8>> {
    let envelope = step.output_contract.host_envelope.as_ref().ok_or_else(|| {
        run_failure(
            RunFailureKind::PolicyBlocked,
            "host-envelope-metadata-missing",
        )
    })?;
    let has_routed_context = staged_inputs.iter().any(|input| {
        matches!(
            input.logical_name.as_str(),
            "routed_context" | "routed-context"
        )
    });
    envelope
        .validate(
            step.output_contract.output_kind.as_deref(),
            has_routed_context,
            &step.output_contract.required_top_level,
        )
        .map_err(|_| {
            run_failure(
                RunFailureKind::PolicyBlocked,
                "host-envelope-metadata-invalid",
            )
        })?;

    let semantic = serde_json::from_str::<Value>(model_output)
        .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "semantic-output-malformed"))?;
    let semantic_object = semantic
        .as_object()
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "semantic-output-not-object"))?;
    if envelope
        .owned_top_level
        .iter()
        .any(|field| semantic_object.contains_key(field))
    {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "host-owned-field-injection",
        ));
    }
    let semantic_schema =
        provider_schema_source(canonical_schema, &envelope.semantic_required_top_level)?;
    if jsonschema::draft202012::validate(&semantic_schema, &semantic).is_err() {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "semantic-output-invalid",
        ));
    }
    let context_sha256 = staged_inputs
        .iter()
        .find(|input| {
            matches!(
                input.logical_name.as_str(),
                "routed_context" | "routed-context"
            )
        })
        .map(|input| input.authority.sha256.clone())
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "host-context-source-missing"))?;
    let receipt_sha256 = sha256_hex(invocation_bytes);
    let mut inputs_used = invocation_value["inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|input| input["name"].as_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    inputs_used.push("prompt_receipt".into());
    inputs_used.push("invocation_receipt_sha256".into());

    let mut wrapped = serde_json::Map::new();
    for field in &step.output_contract.required_top_level {
        let value = match field.as_str() {
            "contract" => json!(crate::constants::PROMPT_OUTPUT_CONTRACT),
            "prompt_id" => json!(step.prompt_id),
            "job_id" => json!(step.job_id),
            "prompt_version" => json!(step.prompt_version),
            "prompt_sha256" => json!(step.prompt_sha256),
            "context_sha256" => json!(context_sha256),
            "invocation_receipt_sha256" => json!(receipt_sha256),
            "source_summary" => json!({"inputs_used": inputs_used}),
            _ => semantic_object.get(field).cloned().ok_or_else(|| {
                run_failure(RunFailureKind::PolicyBlocked, "semantic-output-missing")
            })?,
        };
        wrapped.insert(field.clone(), value);
    }
    let mut bytes = serde_json::to_vec_pretty(&Value::Object(wrapped))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn required_output_example(example: &Value, required_top_level: &[String]) -> Result<Value> {
    let object = example.as_object().ok_or_else(|| {
        run_failure(
            RunFailureKind::PolicyBlocked,
            "prompt-output-example-not-object",
        )
    })?;
    if required_top_level
        .iter()
        .any(|field| !object.contains_key(field))
    {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "prompt-required-output-field-missing-from-example",
        ));
    }
    Ok(Value::Object(
        object
            .iter()
            .filter(|(field, _)| required_top_level.contains(field))
            .map(|(field, value)| (field.clone(), value.clone()))
            .collect(),
    ))
}

fn schema_from_example(value: &Value) -> Value {
    match value {
        Value::Null => json!({"type": "null"}),
        Value::Bool(_) => json!({"type": "boolean"}),
        Value::Number(number) if number.is_i64() || number.is_u64() => json!({"type": "integer"}),
        Value::Number(_) => json!({"type": "number"}),
        Value::String(_) => json!({"type": "string"}),
        Value::Array(items) => json!({
            "type": "array",
            "items": items.first().map(schema_from_example).unwrap_or_else(|| json!({}))
        }),
        Value::Object(object) => json!({
            "type": "object",
            "properties": object.iter().map(|(key, value)| (key.clone(), schema_from_example(value))).collect::<serde_json::Map<_, _>>(),
            "required": object.keys().cloned().collect::<Vec<_>>(),
            "additionalProperties": false
        }),
    }
}

fn schema_for_example_shape(schema: &Value, example: &Value) -> Value {
    match (schema.as_object(), example.as_object()) {
        (Some(schema_object), Some(example_object)) => {
            let Some(schema_properties) =
                schema_object.get("properties").and_then(Value::as_object)
            else {
                return schema_from_example(example);
            };
            if example_object
                .keys()
                .any(|field| !schema_properties.contains_key(field))
            {
                return schema_from_example(example);
            }
            let mut shaped = schema_object.clone();
            shaped.insert(
                "properties".into(),
                Value::Object(
                    example_object
                        .iter()
                        .map(|(field, value)| {
                            (
                                field.clone(),
                                schema_for_example_shape(&schema_properties[field], value),
                            )
                        })
                        .collect(),
                ),
            );
            shaped.insert(
                "required".into(),
                json!(example_object.keys().cloned().collect::<Vec<_>>()),
            );
            Value::Object(shaped)
        }
        _ if schema.get("type") == Some(&json!("array")) && example.is_array() => {
            let mut shaped = schema.clone();
            if let (Some(first), Some(items)) = (
                example.as_array().and_then(|items| items.first()),
                schema.get("items"),
            ) {
                shaped["items"] = schema_for_example_shape(items, first);
            }
            shaped
        }
        _ => schema.clone(),
    }
}

fn project_schema_node(schema: &Value) -> Result<Value> {
    if schema == &Value::Bool(true) {
        return Ok(json!({}));
    }
    if schema == &Value::Bool(false) {
        return Err(anyhow!("false schema cannot be projected"));
    }
    let object = schema
        .as_object()
        .ok_or_else(|| anyhow!("schema node must be an object"))?;
    let allowed = [
        "$defs",
        "$ref",
        "type",
        "properties",
        "items",
        "enum",
        "const",
        "description",
        "title",
        "anyOf",
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "multipleOf",
        "minLength",
        "maxLength",
        "pattern",
        "minItems",
        "maxItems",
    ];
    let mut projected = serde_json::Map::new();
    for (key, value) in object {
        if !allowed.contains(&key.as_str()) {
            continue;
        }
        let projected_value = match key.as_str() {
            "properties" | "$defs" => {
                let children = value
                    .as_object()
                    .ok_or_else(|| anyhow!("schema {key} must be an object"))?;
                let mapped = children
                    .iter()
                    .map(|(name, child)| Ok((name.clone(), project_schema_node(child)?)))
                    .collect::<Result<serde_json::Map<_, _>>>()?;
                Value::Object(mapped)
            }
            "items" => project_schema_node(value)?,
            "anyOf" => Value::Array(
                value
                    .as_array()
                    .filter(|items| !items.is_empty())
                    .ok_or_else(|| anyhow!("schema anyOf must be non-empty"))?
                    .iter()
                    .map(project_schema_node)
                    .collect::<Result<Vec<_>>>()?,
            ),
            _ => value.clone(),
        };
        projected.insert(key.clone(), projected_value);
    }
    if let Some(branches) = object.get("allOf").and_then(Value::as_array) {
        for branch in branches {
            projected = merge_projected_schemas(projected, project_schema_node(branch)?)?;
        }
    }
    let is_object = projected.get("type").and_then(Value::as_str) == Some("object")
        || projected.contains_key("properties");
    if is_object {
        let properties = projected.remove("properties").unwrap_or_else(|| json!({}));
        let required = properties
            .as_object()
            .map(|properties| properties.keys().cloned().map(Value::String).collect())
            .unwrap_or_default();
        projected.insert("type".into(), Value::String("object".into()));
        projected.insert("properties".into(), properties);
        projected.insert("required".into(), Value::Array(required));
        projected.insert("additionalProperties".into(), Value::Bool(false));
    }
    Ok(Value::Object(projected))
}

fn merge_projected_schemas(
    mut left: serde_json::Map<String, Value>,
    right: Value,
) -> Result<serde_json::Map<String, Value>> {
    let right = right
        .as_object()
        .ok_or_else(|| anyhow!("projected schema must be an object"))?;
    if left.is_empty() {
        return Ok(right.clone());
    }
    if right.is_empty() {
        return Ok(left);
    }
    let both_objects = left.get("type").and_then(Value::as_str) == Some("object")
        && right.get("type").and_then(Value::as_str) == Some("object");
    if both_objects {
        let mut properties = left
            .remove("properties")
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        properties.extend(
            right
                .get("properties")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default(),
        );
        for (key, value) in right {
            left.insert(key.clone(), value.clone());
        }
        let required = properties.keys().cloned().map(Value::String).collect();
        left.insert("type".into(), Value::String("object".into()));
        left.insert("properties".into(), Value::Object(properties));
        left.insert("required".into(), Value::Array(required));
        left.insert("additionalProperties".into(), Value::Bool(false));
        return Ok(left);
    }
    Ok(serde_json::Map::from_iter([(
        "anyOf".into(),
        Value::Array(vec![Value::Object(left), Value::Object(right.clone())]),
    )]))
}

fn gtm_success_artifacts(
    request: &RunRequestV1,
    bundle: &RunBundleV1,
    bundle_sha256: &str,
    fit_result: Value,
) -> Result<SuccessArtifacts> {
    let fit_status = fit_result["status"]
        .as_str()
        .ok_or_else(|| anyhow!("fit result omits status"))?;
    let (decision_name, reason_codes) = match fit_status {
        "fit" => ("qualified", vec!["ready".to_string()]),
        "disqualified" => ("no-draft", vec!["disqualified".to_string()]),
        _ => ("no-draft", vec!["insufficient-context".to_string()]),
    };
    let compiled_context = json!({
        "contract": "mdp.compiled-run-context.v1",
        "execution_id": request.execution_id,
        "profile": request.profile,
        "operation": request.operation,
        "bundle_sha256": bundle_sha256,
        "pack_portable_digest": bundle.pack.portable_digest,
        "declared_input_sha256": bundle.inputs.iter().map(|input| json!({
            "logical_name": input.logical_name,
            "sha256": input.sha256
        })).collect::<Vec<_>>(),
        "qualification": {
            "status": fit_status,
            "context": fit_result["context"],
            "matches": fit_result["matches"],
            "disqualifiers": fit_result["disqualifiers"],
            "signal_authority": fit_result["signal_authority"]
        },
        "drafting_authority": "not-granted"
    });
    let mut output_bytes = serde_json::to_vec_pretty(&fit_result)?;
    output_bytes.push(b'\n');
    let mut decision = DecisionAuthority {
        schema_id: "mdp.gtm-qualification-decision.v1".into(),
        decision: decision_name.into(),
        reason_codes,
        sha256: String::new(),
    };
    decision.sha256 =
        canonical_json_sha256_for_domain(&decision.schema_id, &serde_json::to_value(&decision)?)?;
    Ok(SuccessArtifacts {
        output_bytes,
        output_schema_id: "mdp.fit.v0".into(),
        compiled_context,
        decision,
    })
}

struct SuccessArtifacts {
    output_bytes: Vec<u8>,
    output_schema_id: String,
    compiled_context: Value,
    decision: DecisionAuthority,
}

fn success_artifacts(
    request: &RunRequestV1,
    bundle: &RunBundleV1,
    bundle_sha256: &str,
    output_path: &Path,
    validation: Value,
) -> Result<SuccessArtifacts> {
    let output_bytes = fs::read(output_path)?;
    let schema_id = bundle
        .inputs
        .iter()
        .find(|input| staged_authority_name_is_exact(&input.logical_name, "prompt-output"))
        .map(|input| input.schema_id.clone())
        .unwrap_or_else(|| "mdp.prompt-output.v0".into());
    let compiled_context = json!({
        "contract": "mdp.compiled-run-context.v1",
        "execution_id": request.execution_id,
        "profile": request.profile,
        "operation": request.operation,
        "bundle_sha256": bundle_sha256,
        "pack_portable_digest": bundle.pack.portable_digest,
        "declared_input_sha256": bundle.inputs.iter().map(|input| json!({
            "logical_name": input.logical_name,
            "sha256": input.sha256
        })).collect::<Vec<_>>(),
        "validation_contract": validation["contract"].as_str().unwrap_or("mdp.validate-prompt-output.v0")
    });
    let mut decision = DecisionAuthority {
        schema_id: "mdp.proposal-validation-decision.v1".into(),
        decision: "valid-existing-output".into(),
        reason_codes: vec!["validation-passed".into()],
        sha256: String::new(),
    };
    decision.sha256 =
        canonical_json_sha256_for_domain(&decision.schema_id, &serde_json::to_value(&decision)?)?;
    Ok(SuccessArtifacts {
        output_bytes,
        output_schema_id: schema_id,
        compiled_context,
        decision,
    })
}

fn staged_authority_name_is_exact(authority_name: &str, logical_name: &str) -> bool {
    authority_name
        .strip_prefix("declared/")
        .and_then(|name| name.split_once('-'))
        .is_some_and(|(_, name)| name == logical_name)
}

fn validate_request(request: &RunRequestV1) -> Result<()> {
    if request.contract != RUN_REQUEST_V1 {
        return Err(anyhow!("unsupported run request contract"));
    }
    if request.execution_id.is_empty()
        || request.execution_id.len() > MAX_EXECUTION_ID_BYTES
        || !request
            .execution_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(anyhow!("execution_id must be portable ASCII"));
    }
    if request.pack_release_id.trim().is_empty() {
        return Err(anyhow!("pack_release_id is required"));
    }
    match request.mode {
        RunMode::Deterministic => {
            if request.prompt.is_some() || request.driver.is_some() || request.model.is_some() {
                return Err(anyhow!(
                    "deterministic requests must not declare prompt, driver, or model authority"
                ));
            }
            if request.inputs.is_empty() {
                return Err(anyhow!("at least one declared input is required"));
            }
            if request.execution_policy.network_mode != "none"
                || !request.execution_policy.authorized_endpoints.is_empty()
            {
                return Err(anyhow!(
                    "deterministic runs require network_mode=none and no endpoints"
                ));
            }
            if request.execution_policy.filesystem_mode != "private-staging"
                || request.execution_policy.tool_mode != "none"
                || !request.execution_policy.environment_allowlist.is_empty()
            {
                return Err(anyhow!(
                    "deterministic runs require private-staging, no tools, and an empty environment allowlist"
                ));
            }
        }
        RunMode::Generative => {
            let prompt = request
                .prompt
                .as_ref()
                .ok_or_else(|| anyhow!("generative requests require prompt authority"))?;
            validate_logical_name(&prompt.logical_name)?;
            let driver = request
                .driver
                .as_ref()
                .ok_or_else(|| anyhow!("generative requests require driver authority"))?;
            let model = request
                .model
                .as_ref()
                .ok_or_else(|| anyhow!("generative requests require model authority"))?;
            if request.job_identity.is_none() {
                return Err(anyhow!("generative requests require job_identity"));
            }
            if driver.driver_id != "mdp-native-openai"
                || driver.implementation != BUNDLED_NATIVE_DRIVER_ID
                || driver
                    .executable_sha256
                    .as_deref()
                    .is_none_or(|hash| !is_sha256(hash))
                || !is_sha256(&driver.configuration_sha256)
                || driver
                    .dependency_lock_sha256
                    .as_deref()
                    .is_none_or(|hash| !is_sha256(hash))
            {
                return Err(anyhow!("generative driver identity is incomplete"));
            }
            if model.provider != "openai"
                || model.authorized_endpoint != OFFICIAL_OPENAI_RESPONSES_ENDPOINT
                || model.resolved_model.is_some()
            {
                return Err(anyhow!("generative model authority is unsupported"));
            }
            if request.execution_policy.network_mode != "authorized-endpoints-only"
                || request.execution_policy.authorized_endpoints
                    != [OFFICIAL_OPENAI_RESPONSES_ENDPOINT.to_string()]
                || request.execution_policy.environment_allowlist != ["OPENAI_API_KEY".to_string()]
            {
                return Err(anyhow!("generative execution boundary is unsupported"));
            }
            if request.execution_policy.filesystem_mode != "private-staging"
                || request.execution_policy.tool_mode != "none"
            {
                return Err(anyhow!(
                    "generative execution must use private staging and no tools"
                ));
            }
            if request.execution_policy.max_input_bytes > MAX_NATIVE_DECLARED_INPUT_BYTES {
                return Err(anyhow!(
                    "generative max_input_bytes exceeds the native request boundary"
                ));
            }
        }
    }
    if request.execution_policy.max_input_bytes == 0
        || request.execution_policy.max_input_bytes > MAX_POLICY_INPUT_BYTES
        || request.execution_policy.max_output_bytes == 0
        || request.execution_policy.max_output_bytes > MAX_POLICY_OUTPUT_BYTES
        || request.execution_policy.timeout_ms == 0
        || request.execution_policy.timeout_ms > 60_000
    {
        return Err(anyhow!("execution policy limits must be positive"));
    }
    if !matches!(
        request.execution_policy.retention_policy.as_str(),
        "receipt-only" | "customer-controlled-workdir"
    ) {
        return Err(anyhow!("unsupported retention policy"));
    }
    let mut names = HashSet::new();
    for input in &request.inputs {
        validate_logical_name(&input.logical_name)?;
        if !names.insert(input.logical_name.as_str()) {
            return Err(anyhow!("duplicate declared input logical_name"));
        }
    }
    Ok(())
}

fn validate_output_leaf(leaf: &str) -> Result<()> {
    if leaf.is_empty()
        || leaf.len() > MAX_OUTPUT_LEAF_BYTES
        || !leaf.is_ascii()
        || matches!(leaf, "." | "..")
        || !leaf
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(run_failure(
            RunFailureKind::Preflight,
            "output-directory-name-invalid",
        ));
    }
    Ok(())
}

fn validate_output_outside_pack(request: &RunRequestV1, output_root: &Path) -> Result<()> {
    let pack_root = fs::canonicalize(&request.pack_dir)
        .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "pack-boundary-refused"))?;
    let output_root = canonicalize_new_output_path(output_root)
        .map_err(|_| run_failure(RunFailureKind::Preflight, "output-directory-path-invalid"))?;
    if output_root == pack_root || output_root.starts_with(&pack_root) {
        return Err(run_failure(
            RunFailureKind::Preflight,
            "output-directory-inside-pack",
        ));
    }
    Ok(())
}

fn canonicalize_new_output_path(path: &Path) -> Result<PathBuf> {
    match canonicalize_new_output_path_inner(path) {
        Ok(path) => Ok(path),
        Err(error)
            if path
                .components()
                .any(|component| matches!(component, Component::ParentDir)) =>
        {
            canonicalize_new_output_path_inner(&lexically_normalize_path(path)).map_err(|_| error)
        }
        Err(error) => Err(error),
    }
}

fn canonicalize_new_output_path_inner(path: &Path) -> Result<PathBuf> {
    let mut existing = path.to_path_buf();
    let mut missing_components = Vec::new();
    loop {
        match fs::symlink_metadata(&existing) {
            Ok(metadata) if metadata.is_dir() || metadata.file_type().is_symlink() => break,
            Ok(_) => {
                return Err(anyhow!(
                    "output directory ancestor is not a directory: {}",
                    existing.display()
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let component = existing
                    .file_name()
                    .ok_or_else(|| anyhow!("output directory has no canonical ancestor"))?;
                missing_components.push(component.to_owned());
                existing = existing
                    .parent()
                    .ok_or_else(|| anyhow!("output directory has no canonical ancestor"))?
                    .to_path_buf();
            }
            Err(error) => return Err(error.into()),
        }
    }
    let mut canonical = fs::canonicalize(existing)?;
    for component in missing_components.iter().rev() {
        if component == ".." {
            canonical.pop();
        } else if component != "." {
            canonical.push(component);
        }
    }
    Ok(canonical)
}

fn lexically_normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized.file_name().is_some_and(|name| name != "..") {
                    normalized.pop();
                } else {
                    normalized.push(component.as_os_str());
                }
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn validate_pack_snapshot_bounds(
    snapshot: &crate::artifact_hash::PortablePackSnapshot,
) -> Result<()> {
    if snapshot.files.len() > MAX_PACK_FILES {
        return Err(anyhow!("pack exceeds fixed file-count limit"));
    }
    let byte_count = snapshot.files.iter().try_fold(0u64, |total, file| {
        total
            .checked_add(file.byte_count)
            .ok_or_else(|| anyhow!("pack byte count overflow"))
    })?;
    if byte_count > MAX_PACK_BYTES {
        return Err(anyhow!("pack exceeds fixed byte limit"));
    }
    Ok(())
}

fn validate_pack_source_bounds(root: &Path) -> Result<()> {
    let pack_root = root.join(".mdp");
    let metadata = fs::symlink_metadata(&pack_root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(anyhow!("pack root must be a real directory"));
    }
    let mut file_count = 0usize;
    let mut byte_count = 0u64;
    validate_pack_directory_bounds(&pack_root, true, &mut file_count, &mut byte_count)
}

fn validate_pack_directory_bounds(
    directory: &Path,
    pack_root: bool,
    file_count: &mut usize,
    byte_count: &mut u64,
) -> Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if pack_root
            && GENERATED_PACK_DIRECTORIES
                .iter()
                .any(|name| entry.file_name() == *name)
        {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            return Err(anyhow!("pack staging rejects symlinks"));
        }
        if metadata.is_dir() {
            validate_pack_directory_bounds(&entry.path(), false, file_count, byte_count)?;
        } else if metadata.is_file() {
            reject_hard_link(&metadata, "pack staging")?;
            *file_count = file_count
                .checked_add(1)
                .ok_or_else(|| anyhow!("pack file count overflow"))?;
            if *file_count > MAX_PACK_FILES {
                return Err(anyhow!("pack exceeds fixed file-count limit"));
            }
            *byte_count = byte_count
                .checked_add(metadata.len())
                .ok_or_else(|| anyhow!("pack byte count overflow"))?;
            if *byte_count > MAX_PACK_BYTES {
                return Err(anyhow!("pack exceeds fixed byte limit"));
            }
        } else {
            return Err(anyhow!("pack staging accepts only regular files"));
        }
    }
    Ok(())
}

fn read_bounded(path: &Path, max_bytes: u64, label: &str) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(anyhow!("{label} exceeds byte limit"));
    }
    Ok(bytes)
}

fn stage_inputs(request: &RunRequestV1, target: &Path) -> Result<Vec<StagedInput>> {
    let mut total_bytes = 0u64;
    request
        .inputs
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let metadata = fs::symlink_metadata(&input.source_path)
                .with_context(|| format!("reading declared input {}", input.source_path))?;
            total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or_else(|| anyhow!("declared input byte count overflow"))?;
            if total_bytes > request.execution_policy.max_input_bytes {
                return Err(anyhow!(
                    "declared inputs exceed execution policy byte limit"
                ));
            }
            stage_local_artifact(
                input,
                target,
                index,
                request.execution_policy.max_input_bytes - (total_bytes - metadata.len()),
                "declared",
            )
        })
        .collect()
}

fn stage_local_artifact(
    input: &crate::run_contracts::LocalArtifactInput,
    target: &Path,
    index: usize,
    max_bytes: u64,
    authority_prefix: &str,
) -> Result<StagedInput> {
    let source = Path::new(&input.source_path);
    let metadata = fs::symlink_metadata(source)
        .with_context(|| format!("reading declared input {}", source.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(anyhow!("declared inputs must be regular non-symlink files"));
    }
    reject_hard_link(&metadata, "declared inputs")?;
    let bytes = read_bounded(source, max_bytes, "declared input")?;
    if bytes.len() as u64 != metadata.len() {
        return Err(anyhow!("declared input changed while it was staged"));
    }
    let initial_sha256 = sha256_hex(&bytes);
    let staged_path = target.join(format!("{index:03}-{}", input.logical_name));
    write_bytes_create_new(&staged_path, &bytes)?;
    if sha256_hex(&fs::read(&staged_path)?) != initial_sha256 {
        return Err(anyhow!("declared input changed while it was staged"));
    }
    Ok(StagedInput {
        logical_name: input.logical_name.clone(),
        authority: ArtifactAuthority {
            logical_name: format!("{authority_prefix}/{index:03}-{}", input.logical_name),
            schema_id: input.schema_id.clone(),
            media_type: input.media_type.clone(),
            byte_count: bytes.len() as u64,
            sha256: initial_sha256.clone(),
            provenance: EvidenceProvenance::MdpObserved,
            provenance_refs: input.provenance_refs.clone(),
        },
        source_path: source.to_path_buf(),
        staged_path,
        initial_sha256,
    })
}

fn verify_sources_unchanged(inputs: &[StagedInput]) -> Result<()> {
    for input in inputs {
        let metadata = fs::symlink_metadata(&input.source_path)?;
        let source_bytes = read_bounded(
            &input.source_path,
            input.authority.byte_count,
            "declared input",
        )?;
        let staged_bytes = read_bounded(
            &input.staged_path,
            input.authority.byte_count,
            "staged input",
        )?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() != input.authority.byte_count
            || sha256_hex(&source_bytes) != input.initial_sha256
            || sha256_hex(&staged_bytes) != input.initial_sha256
        {
            return Err(anyhow!("declared input mutated during execution"));
        }
    }
    Ok(())
}

fn required_input<'a>(inputs: &'a [StagedInput], name: &str) -> Result<&'a StagedInput> {
    optional_input(inputs, name).ok_or_else(|| anyhow!("required declared input missing: {name}"))
}

fn required_typed_input<'a>(
    inputs: &'a [StagedInput],
    name: &str,
    schema_id: &str,
    media_type: &str,
) -> Result<&'a StagedInput> {
    let input = required_input(inputs, name)?;
    validate_input_type(input, schema_id, media_type)?;
    Ok(input)
}

fn validate_input_type(input: &StagedInput, schema_id: &str, media_type: &str) -> Result<()> {
    if input.authority.schema_id != schema_id || input.authority.media_type != media_type {
        return Err(anyhow!("declared input schema or media type mismatch"));
    }
    Ok(())
}

fn optional_input<'a>(inputs: &'a [StagedInput], name: &str) -> Option<&'a StagedInput> {
    inputs.iter().find(|input| input.logical_name == name)
}

fn assurance_dimensions(
    mode: RunMode,
    terminal_state: TerminalState,
    bundle_sha256: &str,
    mutation_check_passed: bool,
) -> Vec<AssuranceDimension> {
    let mutation_state = if mutation_check_passed {
        AssuranceEvidenceState::Verified
    } else {
        AssuranceEvidenceState::Unknown
    };
    vec![
        AssuranceDimension {
            dimension: "declared-input-isolation".into(),
            state: AssuranceEvidenceState::Observed,
            provenance: EvidenceProvenance::MdpObserved,
            evidence_refs: vec![bundle_sha256.into()],
            limitations: vec![
                "OS-level access outside the private staging tree is not attested".into(),
            ],
        },
        AssuranceDimension {
            dimension: "declared-input-byte-binding".into(),
            state: AssuranceEvidenceState::Verified,
            provenance: EvidenceProvenance::MdpObserved,
            evidence_refs: vec![bundle_sha256.into()],
            limitations: vec![
                "exact source and staged bytes were re-read and matched during this local invocation"
                    .into(),
            ],
        },
        AssuranceDimension {
            dimension: "source-mutation-resistance".into(),
            state: mutation_state,
            provenance: EvidenceProvenance::VerifierRecomputed,
            evidence_refs: vec![bundle_sha256.into()],
            limitations: vec![],
        },
        AssuranceDimension {
            dimension: "stateless-inference".into(),
            state: if mode == RunMode::Deterministic {
                AssuranceEvidenceState::NotApplicable
            } else {
                AssuranceEvidenceState::Declared
            },
            provenance: if mode == RunMode::Deterministic {
                EvidenceProvenance::MdpObserved
            } else {
                EvidenceProvenance::DriverAttested
            },
            evidence_refs: if mode == RunMode::Deterministic {
                vec![]
            } else {
                vec![bundle_sha256.into()]
            },
            limitations: if mode == RunMode::Deterministic {
                vec!["this operation performs no model inference".into()]
            } else {
                vec![
                    "store:false and fresh-request behavior are driver-declared; provider-side retention remains provider-controlled"
                        .into(),
                ]
            },
        },
        AssuranceDimension {
            dimension: "audit-evidence".into(),
            state: if terminal_state == TerminalState::NoDraftAuditIncomplete {
                AssuranceEvidenceState::Unknown
            } else {
                AssuranceEvidenceState::Observed
            },
            provenance: EvidenceProvenance::MdpObserved,
            evidence_refs: vec![bundle_sha256.into()],
            limitations: vec![
                "receipt integrity is locally recomputable; host durability is not attested".into(),
            ],
        },
    ]
}

fn authority_for_file(
    logical_name: &str,
    schema_id: &str,
    media_type: &str,
    path: &Path,
    provenance: EvidenceProvenance,
    provenance_refs: Vec<String>,
) -> Result<ArtifactAuthority> {
    let bytes = fs::read(path)?;
    Ok(ArtifactAuthority {
        logical_name: logical_name.into(),
        schema_id: schema_id.into(),
        media_type: media_type.into(),
        byte_count: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
        provenance,
        provenance_refs,
    })
}

fn copy_pack(source_root: &Path, target_root: &Path) -> Result<()> {
    let source = source_root.join(".mdp");
    let target = target_root.join(".mdp");
    fs::create_dir(&target)?;
    set_private_directory(&target)?;
    let mut remaining_bytes = MAX_PACK_BYTES;
    let mut remaining_files = MAX_PACK_FILES;
    copy_pack_directory(
        &source,
        &target,
        true,
        &mut remaining_bytes,
        &mut remaining_files,
    )
}

fn copy_pack_directory(
    source: &Path,
    target: &Path,
    pack_root: bool,
    remaining_bytes: &mut u64,
    remaining_files: &mut usize,
) -> Result<()> {
    let mut entries = fs::read_dir(source)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if pack_root
            && GENERATED_PACK_DIRECTORIES
                .iter()
                .any(|name| entry.file_name() == *name)
        {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            return Err(anyhow!("pack staging rejects symlinks"));
        }
        let destination = target.join(entry.file_name());
        if metadata.is_dir() {
            fs::create_dir(&destination)?;
            set_private_directory(&destination)?;
            copy_pack_directory(
                &entry.path(),
                &destination,
                false,
                remaining_bytes,
                remaining_files,
            )?;
        } else if metadata.is_file() {
            reject_hard_link(&metadata, "pack staging")?;
            if *remaining_files == 0 || metadata.len() > *remaining_bytes {
                return Err(anyhow!("pack exceeds fixed staging limit"));
            }
            let bytes = read_bounded(&entry.path(), *remaining_bytes, "pack")?;
            if bytes.len() as u64 != metadata.len() {
                return Err(anyhow!("pack changed while it was staged"));
            }
            *remaining_files -= 1;
            *remaining_bytes -= bytes.len() as u64;
            write_bytes_create_new(&destination, &bytes)?;
        } else {
            return Err(anyhow!("pack staging accepts only regular files"));
        }
    }
    Ok(())
}

fn validate_logical_name(name: &str) -> Result<()> {
    let path = Path::new(name);
    if name.is_empty()
        || !name.is_ascii()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || name.contains(['/', '\\'])
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(anyhow!(
            "declared input logical_name must be portable ASCII"
        ));
    }
    Ok(())
}

fn write_json_create_new(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write_bytes_create_new(path, &bytes)
}

fn write_bytes_create_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("creating {}", path.display()))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn set_private_directory(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_directory(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn reject_hard_link(metadata: &fs::Metadata, label: &str) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if metadata.nlink() != 1 {
        return Err(anyhow!("{label} rejects hard-linked files"));
    }
    Ok(())
}

#[cfg(not(unix))]
fn reject_hard_link(_metadata: &fs::Metadata, _label: &str) -> Result<()> {
    Ok(())
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::{
        RunFailure, execute_generative_step, execute_run_inner, execute_run_inner_with_driver,
        governed_normalization_outcome, gtm_lineage_schema_ids, gtm_success_artifacts,
        host_wrap_governed_output, project_output_schema_for_openai, provider_max_output_tokens,
        provider_schema_source, provider_schema_source_for_contract, seal_driver_request,
        seal_driver_result, validate_driver_result, validate_request,
    };
    use crate::commands::init::init_pack;
    use crate::models::{PromptEntryDefaults, PromptHostEnvelope, PromptOutputContract};
    use crate::run_contracts::{
        ArtifactAuthority, AssuranceEvidenceState, DRIVER_REQUEST_V2, DRIVER_RESULT_V2,
        DriverArtifactV2, DriverIdentity, DriverOutputV2, DriverProviderObservationV2,
        DriverProviderPolicyV2, DriverRequestV2, DriverResultV2, EvidenceProvenance,
        ExecutionPolicy, JobIdentity, LocalArtifactInput, ModelIdentity, PackAuthority,
        RunBundleV1, RunMode, RunRequestV1, TerminalState,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rejects_ambient_authority_for_deterministic_run() {
        let mut request = request_fixture("not-used", "not-used");
        request.execution_policy.network_mode = "allow".into();
        assert!(validate_request(&request).is_err());
    }

    #[test]
    fn generative_preflight_requires_the_fixed_provider_boundary() {
        let mut request = request_fixture("not-used", "not-used");
        request.mode = RunMode::Generative;
        request.operation = "model:proposal-draft/review".into();
        request.job_identity = Some(JobIdentity {
            job_id: "proposal-draft".into(),
            idempotency_key: "idem-1".into(),
        });
        request.prompt = Some(request.inputs[0].clone());
        request.driver = Some(DriverIdentity {
            driver_id: "mdp-native-openai".into(),
            implementation: super::BUNDLED_NATIVE_DRIVER_ID.into(),
            version: "1".into(),
            build_sha256: None,
            executable_sha256: Some("a".repeat(64)),
            image_digest: None,
            configuration_sha256: "b".repeat(64),
            dependency_lock_sha256: Some("d".repeat(64)),
            identity_provenance: EvidenceProvenance::MdpObserved,
        });
        request.model = Some(ModelIdentity {
            provider: "openai".into(),
            requested_model: "gpt-5-mini".into(),
            resolved_model: None,
            authorized_endpoint: super::OFFICIAL_OPENAI_RESPONSES_ENDPOINT.into(),
            parameters_sha256: "c".repeat(64),
            session_behavior: AssuranceEvidenceState::NotApplicable,
            cache_behavior: AssuranceEvidenceState::Unknown,
            storage_behavior: AssuranceEvidenceState::Declared,
        });
        request.execution_policy.network_mode = "authorized-endpoints-only".into();
        request.execution_policy.authorized_endpoints =
            vec![super::OFFICIAL_OPENAI_RESPONSES_ENDPOINT.into()];
        request.execution_policy.environment_allowlist = vec!["OPENAI_API_KEY".into()];
        request.execution_policy.max_input_bytes = super::MAX_NATIVE_DECLARED_INPUT_BYTES;
        assert!(validate_request(&request).is_ok());

        request.execution_policy.max_input_bytes = super::MAX_NATIVE_DECLARED_INPUT_BYTES + 1;
        assert!(validate_request(&request).is_err());
        request.execution_policy.max_input_bytes = super::MAX_NATIVE_DECLARED_INPUT_BYTES;

        request.execution_policy.authorized_endpoints = vec!["https://example.test".into()];
        assert!(validate_request(&request).is_err());
    }

    #[test]
    fn native_identity_declarations_fail_before_driver_and_run_publication() {
        let root = temp_path("native-identity-mismatch");
        let pack = root.join("pack");
        let raw = root.join("raw-row.json");
        fs::create_dir_all(&root).unwrap();
        crate::commands::init::init_pack(&pack, "Identity Pack", "gtm", true, false).unwrap();
        fs::write(&raw, "{\"company\":\"Synthetic Co\"}\n").unwrap();
        let request = generative_request_fixture(&pack, &raw);
        for label in ["driver", "model", "version"] {
            let mut altered = request.clone();
            if label == "driver" {
                altered.driver.as_mut().unwrap().configuration_sha256 = "b".repeat(64);
            } else if label == "model" {
                altered.model.as_mut().unwrap().parameters_sha256 = "c".repeat(64);
            } else {
                altered.driver.as_mut().unwrap().version = "caller-forged-version".into();
            }
            let run = root.join(format!("run-{label}"));
            let error = execute_run_inner_with_driver(
                &altered,
                &run,
                || Ok(()),
                |_, _| panic!("identity mismatch must not invoke the driver"),
            )
            .unwrap_err();
            assert!(
                matches!(
                    error.downcast_ref::<RunFailure>().map(RunFailure::code),
                    Some("driver-configuration-identity-mismatch")
                        | Some("model-parameters-identity-mismatch")
                        | Some("driver-version-mismatch")
                ),
                "unexpected mismatch code for {label}: {error}"
            );
            assert!(!run.exists());
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn native_output_limits_bind_provider_tokens_and_json_encoding_headroom() {
        assert_eq!(provider_max_output_tokens(1), 1);
        assert_eq!(provider_max_output_tokens(1_024), 256);
        assert_eq!(provider_max_output_tokens(1_048_576), 100_000);
        assert_eq!(super::driver_stdout_limit(1).unwrap(), 65_542);
        assert_eq!(
            super::driver_stdout_limit(1_048_576).unwrap(),
            6 * 1_048_576 + 65_536
        );
    }

    #[test]
    fn provider_schema_projection_is_structural_and_strict() {
        let projected = project_output_schema_for_openai(&serde_json::json!({
            "type": "object",
            "properties": {
                "status": {"type": "string", "enum": ["ready", "gap"]},
                "detail": {"type": "string", "not": {"const": "secret"}}
            },
            "required": ["status"],
            "additionalProperties": true,
            "allOf": [{"properties": {"count": {"type": "integer", "minimum": 0}}}]
        }))
        .unwrap();
        assert_eq!(projected["additionalProperties"], false);
        assert_eq!(
            projected["required"],
            serde_json::json!(["count", "detail", "status"])
        );
        assert!(projected["properties"]["detail"].get("not").is_none());
    }

    #[test]
    fn provider_schema_uses_only_prompt_required_top_level_fields() {
        let canonical = serde_json::json!({
            "type": "object",
            "properties": {
                "contract": {"type": "string"},
                "normalized_prospect": {"type": "object", "properties": {}, "additionalProperties": false},
                "normalized_opportunity": {"type": "object", "properties": {}, "additionalProperties": false}
            },
            "required": ["contract"]
        });
        let source = provider_schema_source(
            &canonical,
            &["contract".into(), "normalized_prospect".into()],
        )
        .unwrap();
        assert_eq!(
            source["required"],
            serde_json::json!(["contract", "normalized_prospect"])
        );
        assert!(source["properties"].get("normalized_opportunity").is_none());
    }

    #[test]
    fn host_envelope_provider_schema_excludes_host_owned_fields() {
        let canonical = serde_json::json!({
            "type": "object",
            "properties": {
                "contract": {"type": "string"},
                "prompt_id": {"type": "string"},
                "job_id": {"type": "string"},
                "prompt_version": {"type": "string"},
                "prompt_sha256": {"type": "string"},
                "context_sha256": {"type": "string"},
                "invocation_receipt_sha256": {"type": "string"},
                "source_summary": {"type": "object"},
                "selected_authority": {"type": "object"},
                "artifact": {"type": "object"},
                "gaps": {"type": "array"},
                "rejected_claims": {"type": "array"}
            },
            "required": [
                "contract", "prompt_id", "job_id", "prompt_version", "prompt_sha256",
                "context_sha256", "invocation_receipt_sha256", "source_summary",
                "selected_authority", "artifact", "gaps", "rejected_claims"
            ],
            "additionalProperties": false
        });
        let contract = PromptOutputContract {
            contract: crate::constants::PROMPT_OUTPUT_CONTRACT.into(),
            output_kind: Some("governed-artifact".into()),
            strict_json_only: true,
            required_top_level: vec![
                "contract".into(),
                "prompt_id".into(),
                "job_id".into(),
                "prompt_version".into(),
                "prompt_sha256".into(),
                "context_sha256".into(),
                "invocation_receipt_sha256".into(),
                "source_summary".into(),
                "selected_authority".into(),
                "artifact".into(),
                "gaps".into(),
                "rejected_claims".into(),
            ],
            entry_defaults: PromptEntryDefaults {
                body: "".into(),
                applies_to: vec![],
                evidence: vec![],
                avoid: vec![],
                confidence: "unknown".into(),
                provenance: vec![],
            },
            schema_ref: None,
            schema: None,
            host_envelope: Some(PromptHostEnvelope {
                contract: crate::constants::GOVERNED_HOST_ENVELOPE_CONTRACT.into(),
                owned_top_level: crate::models::GOVERNED_HOST_ENVELOPE_OWNED_FIELDS
                    .iter()
                    .map(|field| (*field).into())
                    .collect(),
                semantic_required_top_level: crate::models::GOVERNED_HOST_ENVELOPE_SEMANTIC_FIELDS
                    .iter()
                    .map(|field| (*field).into())
                    .collect(),
            }),
            example: serde_json::json!({}),
        };
        let source = provider_schema_source_for_contract(&canonical, &contract).unwrap();
        assert_eq!(
            source["required"],
            serde_json::json!(["selected_authority", "artifact", "gaps", "rejected_claims"])
        );
        assert!(source["properties"].get("contract").is_none());
        assert!(source["properties"].get("prompt_id").is_none());
        assert!(source["properties"].get("artifact").is_some());

        let mut invalid_contract = contract;
        invalid_contract.required_top_level.push("extra".into());
        let error = provider_schema_source_for_contract(&canonical, &invalid_contract).unwrap_err();
        assert_eq!(
            error.downcast_ref::<RunFailure>().unwrap().code(),
            "host-envelope-contract-invalid"
        );
    }

    fn host_envelope_test_schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "contract": {"const": crate::constants::PROMPT_OUTPUT_CONTRACT},
                "prompt_id": {"type": "string"},
                "job_id": {"type": "string"},
                "prompt_version": {"type": "string"},
                "prompt_sha256": {"type": "string"},
                "context_sha256": {"type": "string"},
                "invocation_receipt_sha256": {"type": "string"},
                "source_summary": {"type": "object"},
                "selected_authority": {"type": "object"},
                "artifact": {"type": "object"},
                "gaps": {"type": "array"},
                "rejected_claims": {"type": "array"}
            },
            "required": [
                "contract", "prompt_id", "job_id", "prompt_version", "prompt_sha256",
                "context_sha256", "invocation_receipt_sha256", "source_summary",
                "selected_authority", "artifact", "gaps", "rejected_claims"
            ],
            "additionalProperties": false
        })
    }

    fn host_envelope_test_step() -> crate::model_steps::CompiledModelStepV1 {
        let owned = crate::models::GOVERNED_HOST_ENVELOPE_OWNED_FIELDS
            .iter()
            .map(|field| (*field).to_string())
            .collect::<Vec<_>>();
        let semantic = crate::models::GOVERNED_HOST_ENVELOPE_SEMANTIC_FIELDS
            .iter()
            .map(|field| (*field).to_string())
            .collect::<Vec<_>>();
        let mut required_top_level = owned.clone();
        required_top_level.extend(semantic.iter().cloned());
        crate::model_steps::CompiledModelStepV1 {
            contract: crate::model_steps::COMPILED_MODEL_STEP_V1.into(),
            step_id: "model:outbound-copy-review/review".into(),
            job_id: "outbound-copy-review".into(),
            skill_id: "mdp-gtm-brief".into(),
            phase: crate::model_steps::ModelStepPhase::Review,
            authority: crate::model_steps::ModelStepAuthorityV1 {
                kind: "job".into(),
                ids: vec!["outbound-copy-review".into()],
            },
            prompt_id: "review-outbound-copy-v1".into(),
            prompt_version: "3".into(),
            prompt_path: "prompts/review-outbound-copy.yaml".into(),
            prompt_sha256: "a".repeat(64),
            declared_inputs: vec![],
            routed_context_required: true,
            output_contract: PromptOutputContract {
                contract: crate::constants::PROMPT_OUTPUT_CONTRACT.into(),
                output_kind: Some("governed-artifact".into()),
                strict_json_only: true,
                required_top_level,
                entry_defaults: PromptEntryDefaults {
                    body: "".into(),
                    applies_to: vec![],
                    evidence: vec![],
                    avoid: vec![],
                    confidence: "unknown".into(),
                    provenance: vec![],
                },
                schema_ref: None,
                schema: Some(host_envelope_test_schema()),
                host_envelope: Some(PromptHostEnvelope {
                    contract: crate::constants::GOVERNED_HOST_ENVELOPE_CONTRACT.into(),
                    owned_top_level: owned,
                    semantic_required_top_level: semantic,
                }),
                example: serde_json::json!({}),
            },
            output_contract_sha256: "b".repeat(64),
        }
    }

    fn host_staged_context(context_sha256: &str) -> Vec<super::StagedInput> {
        vec![super::StagedInput {
            logical_name: "routed_context".into(),
            authority: ArtifactAuthority {
                logical_name: "routed_context".into(),
                schema_id: "mdp.routed-context.v1".into(),
                media_type: "application/json".into(),
                byte_count: 1,
                sha256: context_sha256.into(),
                provenance: EvidenceProvenance::MdpObserved,
                provenance_refs: vec![],
            },
            source_path: PathBuf::new(),
            staged_path: PathBuf::new(),
            initial_sha256: context_sha256.into(),
        }]
    }

    fn host_semantic_output() -> serde_json::Value {
        serde_json::json!({
            "selected_authority": {},
            "artifact": {},
            "gaps": [],
            "rejected_claims": []
        })
    }

    #[test]
    fn host_wraps_valid_semantics_and_recomputes_receipt_and_context() {
        let step = host_envelope_test_step();
        let invocation = serde_json::json!({"inputs": [{"name": "routed_context"}]});
        let invocation_bytes = serde_json::to_vec(&invocation).unwrap();
        let wrapped = host_wrap_governed_output(
            &step,
            &host_staged_context(&"c".repeat(64)),
            &invocation,
            &invocation_bytes,
            &serde_json::to_string(&host_semantic_output()).unwrap(),
            &host_envelope_test_schema(),
        )
        .unwrap();
        let wrapped: serde_json::Value = serde_json::from_slice(&wrapped).unwrap();
        assert_eq!(wrapped["prompt_id"], "review-outbound-copy-v1");
        assert_eq!(wrapped["context_sha256"], "c".repeat(64));
        assert_eq!(
            wrapped["invocation_receipt_sha256"],
            crate::artifact_hash::sha256_hex(&invocation_bytes)
        );
        assert_eq!(wrapped["artifact"], serde_json::json!({}));

        let altered_invocation = b"{\"inputs\":[]}\n";
        let altered_receipt = host_wrap_governed_output(
            &step,
            &host_staged_context(&"c".repeat(64)),
            &invocation,
            altered_invocation,
            &serde_json::to_string(&host_semantic_output()).unwrap(),
            &host_envelope_test_schema(),
        )
        .unwrap();
        let altered_receipt: serde_json::Value = serde_json::from_slice(&altered_receipt).unwrap();
        assert_ne!(
            wrapped["invocation_receipt_sha256"],
            altered_receipt["invocation_receipt_sha256"]
        );

        let altered_context = host_wrap_governed_output(
            &step,
            &host_staged_context(&"d".repeat(64)),
            &invocation,
            &invocation_bytes,
            &serde_json::to_string(&host_semantic_output()).unwrap(),
            &host_envelope_test_schema(),
        )
        .unwrap();
        let altered_context: serde_json::Value = serde_json::from_slice(&altered_context).unwrap();
        assert_ne!(wrapped["context_sha256"], altered_context["context_sha256"]);
    }

    #[test]
    fn host_wrapper_rejects_malformed_semantics_and_owned_injection() {
        let step = host_envelope_test_step();
        let staged = host_staged_context(&"c".repeat(64));
        let invocation = serde_json::json!({"inputs": [{"name": "routed_context"}]});
        let invocation_bytes = serde_json::to_vec(&invocation).unwrap();
        for (output, expected) in [
            ("{", "semantic-output-malformed"),
            (
                &serde_json::json!({
                    "contract": "mdp.prompt-output.v0",
                    "selected_authority": {}, "artifact": {}, "gaps": [], "rejected_claims": []
                })
                .to_string(),
                "host-owned-field-injection",
            ),
        ] {
            let error = host_wrap_governed_output(
                &step,
                &staged,
                &invocation,
                &invocation_bytes,
                output,
                &host_envelope_test_schema(),
            )
            .unwrap_err();
            assert_eq!(error.downcast_ref::<RunFailure>().unwrap().code(), expected);
        }
    }

    #[test]
    fn host_wrapper_failure_outcome_is_safe_and_preserves_observed_hashes() {
        let mut request = sample_driver_request();
        seal_driver_request(&mut request).unwrap();
        let mut result = DriverResultV2 {
            contract: DRIVER_RESULT_V2.into(),
            execution_id: request.execution_id.clone(),
            operation: request.operation.clone(),
            terminal_state: TerminalState::Success,
            output: Some(DriverOutputV2 {
                schema_id: "mdp.prompt-output.v0".into(),
                media_type: "application/json".into(),
                content_utf8: "{}".into(),
                byte_count: 2,
                sha256: crate::artifact_hash::sha256_hex(b"{}"),
            }),
            provider_request_body_sha256: Some("d".repeat(64)),
            provider_request_schema_id: Some("openai.responses.json-schema-request.v1".into()),
            provider_response_body_sha256: Some("e".repeat(64)),
            provider_output_schema_sha256: Some(request.provider_output_schema_sha256.clone()),
            provider_observation: Some(DriverProviderObservationV2 {
                provider: "openai".into(),
                response_id: Some("resp_host_wrap".into()),
                resolved_model: Some("gpt-5-mini".into()),
            }),
            diagnostic_code: None,
            result_sha256: String::new(),
        };
        seal_driver_result(&mut result).unwrap();
        let result_sha256 = result.result_sha256.clone();
        let outcome =
            super::host_envelope_failure_outcome(&request, result, "host-owned-field-injection");
        assert_eq!(outcome.terminal_state, TerminalState::NoDraftOutputInvalid);
        assert_eq!(
            outcome.diagnostic_code.as_deref(),
            Some("host-owned-field-injection")
        );
        assert_eq!(outcome.driver_request_sha256, request.request_sha256);
        assert_eq!(outcome.driver_result_sha256, result_sha256);
        assert_eq!(outcome.provider_response_body_sha256, Some("e".repeat(64)));
        assert_eq!(
            outcome
                .provider_observation
                .unwrap()
                .resolved_model
                .as_deref(),
            Some("gpt-5-mini")
        );
    }

    fn execute_host_model_case(model_output: &str) -> super::GenerativeOutcome {
        let root = temp_path("host-envelope-model-case");
        let prompt_path = root.join("prompt.yaml");
        let context_path = root.join("routed-context.json");
        let private_dir = root.join("private");
        fs::create_dir_all(&private_dir).unwrap();
        fs::write(&prompt_path, "synthetic prompt\n").unwrap();
        fs::write(&context_path, "{}\n").unwrap();

        let mut request = request_fixture("synthetic-pack", "synthetic-output");
        request.profile = "gtm".into();
        request.operation = "model:outbound-copy-review/review".into();
        request.mode = RunMode::Generative;
        request.job_identity = Some(JobIdentity {
            job_id: "outbound-copy-review".into(),
            idempotency_key: "host-envelope-test".into(),
        });
        request.driver = Some(DriverIdentity {
            driver_id: "mdp-native-openai".into(),
            implementation: super::BUNDLED_NATIVE_DRIVER_ID.into(),
            version: super::MDP_RUNTIME_VERSION.into(),
            build_sha256: None,
            executable_sha256: Some("a".repeat(64)),
            image_digest: None,
            configuration_sha256: "b".repeat(64),
            dependency_lock_sha256: Some("c".repeat(64)),
            identity_provenance: EvidenceProvenance::MdpObserved,
        });
        request.model = Some(ModelIdentity {
            provider: "openai".into(),
            requested_model: "gpt-5-mini".into(),
            resolved_model: None,
            authorized_endpoint: super::OFFICIAL_OPENAI_RESPONSES_ENDPOINT.into(),
            parameters_sha256: "d".repeat(64),
            session_behavior: AssuranceEvidenceState::NotApplicable,
            cache_behavior: AssuranceEvidenceState::Unknown,
            storage_behavior: AssuranceEvidenceState::Declared,
        });

        let staged_prompt = super::StagedInput {
            logical_name: "review-outbound-copy-v1".into(),
            authority: ArtifactAuthority {
                logical_name: "review-outbound-copy-v1".into(),
                schema_id: "mdp.prompt.v1".into(),
                media_type: "application/yaml".into(),
                byte_count: 17,
                sha256: "e".repeat(64),
                provenance: EvidenceProvenance::MdpObserved,
                provenance_refs: vec![],
            },
            source_path: prompt_path.clone(),
            staged_path: prompt_path,
            initial_sha256: "e".repeat(64),
        };
        let mut staged_inputs = host_staged_context(&"f".repeat(64));
        staged_inputs[0].source_path = context_path.clone();
        staged_inputs[0].staged_path = context_path;

        let step = host_envelope_test_step();
        let invocation_value = serde_json::json!({
            "contract": "mdp.prompt-invocation.v1",
            "job_id": "outbound-copy-review",
            "inputs": [{"name": "routed_context", "sha256": "f".repeat(64)}]
        });
        let mut invocation_bytes = serde_json::to_vec_pretty(&invocation_value).unwrap();
        invocation_bytes.push(b'\n');
        let schema = host_envelope_test_schema();
        let schema_sha256 = crate::artifact_hash::canonical_json_sha256(&schema).unwrap();
        let prepared = super::PreparedNativeRequest {
            step,
            invocation_value,
            invocation_bytes: invocation_bytes.clone(),
            invocation_sha256: crate::artifact_hash::sha256_hex(&invocation_bytes),
            visible_input: String::new(),
            canonical_output_schema: schema.clone(),
            canonical_output_schema_sha256: schema_sha256.clone(),
            provider_output_schema: schema,
            provider_output_schema_sha256: schema_sha256,
            schema_name: "mdp_host_envelope_test".into(),
        };
        let bundle = RunBundleV1 {
            contract: "mdp.run-bundle.v1".into(),
            execution_id: request.execution_id.clone(),
            created_at: request.created_at.clone(),
            profile: request.profile.clone(),
            operation: request.operation.clone(),
            mode: RunMode::Generative,
            job_identity: request.job_identity.clone(),
            pack: PackAuthority {
                release_id: "synthetic-release".into(),
                pack_id: "synthetic-pack".into(),
                version: "0.1.73".into(),
                profile_id: "gtm".into(),
                portable_digest: "1".repeat(64),
                files: vec![],
            },
            prompt: Some(staged_prompt.authority.clone()),
            inputs: staged_inputs
                .iter()
                .map(|input| input.authority.clone())
                .collect(),
            execution_policy_sha256: "2".repeat(64),
            driver: request.driver.clone(),
            model: request.model.clone(),
            model_facts: None,
        };
        let driver_identity = request.driver.clone().unwrap();
        let output = model_output.to_string();
        let outcome = execute_generative_step(
            &request,
            &root,
            &staged_prompt,
            &staged_inputs,
            &private_dir,
            &bundle,
            &"3".repeat(64),
            &prepared,
            &driver_identity,
            &super::RunDeadline::new(30_000),
            move |driver_request, _| {
                let mut result = DriverResultV2 {
                    contract: DRIVER_RESULT_V2.into(),
                    execution_id: driver_request.execution_id.clone(),
                    operation: driver_request.operation.clone(),
                    terminal_state: TerminalState::Success,
                    output: Some(DriverOutputV2 {
                        schema_id: "mdp.prompt-output.v0".into(),
                        media_type: "application/json".into(),
                        byte_count: output.len() as u64,
                        sha256: crate::artifact_hash::sha256_hex(output.as_bytes()),
                        content_utf8: output,
                    }),
                    provider_request_body_sha256: Some("4".repeat(64)),
                    provider_request_schema_id: Some(
                        "openai.responses.json-schema-request.v1".into(),
                    ),
                    provider_response_body_sha256: Some("5".repeat(64)),
                    provider_output_schema_sha256: Some(
                        driver_request.provider_output_schema_sha256.clone(),
                    ),
                    provider_observation: Some(DriverProviderObservationV2 {
                        provider: "openai".into(),
                        response_id: Some("resp_host_case".into()),
                        resolved_model: Some("gpt-5-mini".into()),
                    }),
                    diagnostic_code: None,
                    result_sha256: String::new(),
                };
                seal_driver_result(&mut result)?;
                Ok(result)
            },
        )
        .unwrap();
        let _ = fs::remove_dir_all(root);
        outcome
    }

    #[test]
    fn model_call_wrapper_failures_return_safe_no_draft_outcomes() {
        for (output, code) in [
            ("{", "semantic-output-malformed"),
            (
                r#"{"contract":"mdp.prompt-output.v0","selected_authority":{},"artifact":{},"gaps":[],"rejected_claims":[]}"#,
                "host-owned-field-injection",
            ),
        ] {
            let outcome = execute_host_model_case(output);
            assert_eq!(outcome.terminal_state, TerminalState::NoDraftOutputInvalid);
            assert_eq!(outcome.diagnostic_code.as_deref(), Some(code));
            assert_eq!(outcome.provider_request_body_sha256, Some("4".repeat(64)));
            assert_eq!(outcome.provider_response_body_sha256, Some("5".repeat(64)));
            assert_eq!(outcome.driver_result_sha256.len(), 64);
        }
    }

    #[test]
    fn executable_transaction_publishes_safe_no_draft_for_wrapper_rejections() {
        let root = temp_path("host-envelope-transaction");
        let pack = root.join("pack");
        let routed_context = root.join("routed-context.json");
        let normalized_prospect = root.join("normalized-prospect.json");
        let supplied_material = root.join("supplied-material.json");
        fs::create_dir_all(&root).unwrap();
        init_pack(&pack, "Host Envelope Pack", "proposal", true, false).unwrap();

        let brief =
            crate::commands::briefs::emit_brief(&pack, "Proposal Lead", None, Some("proof-review"))
                .unwrap();
        let routed_context_bytes =
            crate::artifact_hash::canonical_json_bytes(&brief["context"]["model_context"]).unwrap();
        fs::write(&routed_context, &routed_context_bytes).unwrap();
        fs::write(&normalized_prospect, b"{}\n").unwrap();
        fs::write(&supplied_material, b"{}\n").unwrap();

        let driver_sha =
            crate::artifact_hash::sha256_hex(super::BUNDLED_NATIVE_DRIVER_SOURCE.as_bytes());
        let mut request = RunRequestV1 {
            contract: "mdp.run-request.v1".into(),
            execution_id: "host-envelope-transaction".into(),
            created_at: "2026-08-22T00:00:00Z".into(),
            profile: "proposal".into(),
            operation: "model:proof-review/review".into(),
            mode: RunMode::Generative,
            job_identity: Some(JobIdentity {
                job_id: "proof-review".into(),
                idempotency_key: "host-envelope-transaction".into(),
            }),
            pack_dir: pack.display().to_string(),
            pack_release_id: "host-envelope-test-release".into(),
            prompt: Some(LocalArtifactInput {
                logical_name: "review-proposal-proof-v1".into(),
                source_path: pack
                    .join(".mdp/prompts/review-proposal-proof.yaml")
                    .display()
                    .to_string(),
                schema_id: "mdp.prompt.v1".into(),
                media_type: "application/yaml".into(),
                provenance_refs: vec![],
            }),
            inputs: vec![
                LocalArtifactInput {
                    logical_name: "routed_context".into(),
                    source_path: routed_context.display().to_string(),
                    schema_id: "mdp.routed-context.v1".into(),
                    media_type: "application/json".into(),
                    provenance_refs: vec![],
                },
                LocalArtifactInput {
                    logical_name: "normalized_prospect".into(),
                    source_path: normalized_prospect.display().to_string(),
                    schema_id: "mdp.synthetic-normalized-prospect.v1".into(),
                    media_type: "application/json".into(),
                    provenance_refs: vec![],
                },
                LocalArtifactInput {
                    logical_name: "supplied_material".into(),
                    source_path: supplied_material.display().to_string(),
                    schema_id: "mdp.synthetic-supplied-material.v1".into(),
                    media_type: "application/json".into(),
                    provenance_refs: vec![],
                },
            ],
            execution_policy: ExecutionPolicy {
                environment_allowlist: vec!["OPENAI_API_KEY".into()],
                filesystem_mode: "private-staging".into(),
                tool_mode: "none".into(),
                network_mode: "authorized-endpoints-only".into(),
                authorized_endpoints: vec![super::OFFICIAL_OPENAI_RESPONSES_ENDPOINT.into()],
                max_input_bytes: 131_072,
                max_output_bytes: 1_048_576,
                timeout_ms: 30_000,
                retention_policy: "receipt-only".into(),
            },
            driver: Some(DriverIdentity {
                driver_id: "mdp-native-openai".into(),
                implementation: super::BUNDLED_NATIVE_DRIVER_ID.into(),
                version: super::MDP_RUNTIME_VERSION.into(),
                build_sha256: None,
                executable_sha256: Some(driver_sha),
                image_digest: None,
                configuration_sha256: "0".repeat(64),
                dependency_lock_sha256: Some("1".repeat(64)),
                identity_provenance: EvidenceProvenance::MdpObserved,
            }),
            model: Some(ModelIdentity {
                provider: "openai".into(),
                requested_model: "gpt-5-mini".into(),
                resolved_model: None,
                authorized_endpoint: super::OFFICIAL_OPENAI_RESPONSES_ENDPOINT.into(),
                parameters_sha256: "2".repeat(64),
                session_behavior: AssuranceEvidenceState::NotApplicable,
                cache_behavior: AssuranceEvidenceState::Unknown,
                storage_behavior: AssuranceEvidenceState::Declared,
            }),
        };
        refresh_test_native_declarations(&mut request);

        for (index, (model_output, diagnostic_code)) in [
            ("{", "semantic-output-malformed"),
            (
                r#"{"contract":"mdp.prompt-output.v0","selected_authority":[],"artifact":{},"gaps":[],"rejected_claims":[]}"#,
                "host-owned-field-injection",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let run = root.join(format!("published-run-{index}"));
            let model_output = model_output.to_string();
            let result = execute_run_inner_with_driver(
                &request,
                &run,
                || Ok(()),
                move |driver_request, _| {
                    let mut result = DriverResultV2 {
                        contract: DRIVER_RESULT_V2.into(),
                        execution_id: driver_request.execution_id.clone(),
                        operation: driver_request.operation.clone(),
                        terminal_state: TerminalState::Success,
                        output: Some(DriverOutputV2 {
                            schema_id: "mdp.prompt-output.v0".into(),
                            media_type: "application/json".into(),
                            byte_count: model_output.len() as u64,
                            sha256: crate::artifact_hash::sha256_hex(model_output.as_bytes()),
                            content_utf8: model_output,
                        }),
                        provider_request_body_sha256: Some("3".repeat(64)),
                        provider_request_schema_id: Some(
                            "openai.responses.json-schema-request.v1".into(),
                        ),
                        provider_response_body_sha256: Some("4".repeat(64)),
                        provider_output_schema_sha256: Some(
                            driver_request.provider_output_schema_sha256.clone(),
                        ),
                        provider_observation: Some(DriverProviderObservationV2 {
                            provider: "openai".into(),
                            response_id: Some("resp_host_transaction".into()),
                            resolved_model: Some("gpt-5-mini".into()),
                        }),
                        diagnostic_code: None,
                        result_sha256: String::new(),
                    };
                    seal_driver_result(&mut result)?;
                    Ok(result)
                },
            )
            .unwrap();

            assert_eq!(result.terminal_state, TerminalState::NoDraftOutputInvalid);
            assert!(run.join("run-bundle.json").is_file());
            assert!(run.join("runner-audit.json").is_file());
            assert!(run.join("run-receipt.json").is_file());
            assert!(!run.join("artifacts/output.json").exists());
            assert!(!run.join("private").exists());

            let receipt: serde_json::Value =
                serde_json::from_slice(&fs::read(run.join("run-receipt.json")).unwrap())
                    .unwrap();
            assert!(receipt["output"].is_null());
            assert!(receipt["decision"].is_null());
            let audit: crate::run_contracts::RunnerAuditV1 =
                serde_json::from_slice(&fs::read(run.join("runner-audit.json")).unwrap())
                    .unwrap();
            assert_eq!(audit.diagnostic_code.as_deref(), Some(diagnostic_code));
            assert_eq!(audit.provider_response_body_sha256, Some("4".repeat(64)));
            assert_eq!(
                crate::commands::run_verification::verify_run_files(
                    Some(&run.join("run-bundle.json")),
                    &run.join("run-receipt.json"),
                    Some(&run),
                )
                .unwrap()["valid"],
                true
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn driver_result_binds_projected_schema_and_exact_output_bytes() {
        let mut request = sample_driver_request();
        seal_driver_request(&mut request).unwrap();
        let content = "{\"status\":\"ready\"}";
        let mut result = DriverResultV2 {
            contract: DRIVER_RESULT_V2.into(),
            execution_id: request.execution_id.clone(),
            operation: request.operation.clone(),
            terminal_state: TerminalState::Success,
            output: Some(DriverOutputV2 {
                schema_id: "mdp.prompt-output.v0".into(),
                media_type: "application/json".into(),
                content_utf8: content.into(),
                byte_count: content.len() as u64,
                sha256: crate::artifact_hash::sha256_hex(content.as_bytes()),
            }),
            provider_request_body_sha256: Some("d".repeat(64)),
            provider_request_schema_id: Some("openai.responses.json-schema-request.v1".into()),
            provider_response_body_sha256: Some("f".repeat(64)),
            provider_output_schema_sha256: Some(request.provider_output_schema_sha256.clone()),
            provider_observation: Some(DriverProviderObservationV2 {
                provider: "openai".into(),
                response_id: Some("resp_synthetic".into()),
                resolved_model: Some("gpt-5-mini-2026-08-01".into()),
            }),
            diagnostic_code: None,
            result_sha256: String::new(),
        };
        seal_driver_result(&mut result).unwrap();
        assert!(validate_driver_result(&request, &result).is_ok());

        let valid = result.clone();
        result.provider_request_body_sha256 = None;
        seal_driver_result(&mut result).unwrap();
        assert!(validate_driver_result(&request, &result).is_err());

        result = valid.clone();
        result.provider_request_schema_id = None;
        seal_driver_result(&mut result).unwrap();
        assert!(validate_driver_result(&request, &result).is_err());

        result = valid.clone();
        result.provider_request_schema_id = Some("caller-selected-schema".into());
        seal_driver_result(&mut result).unwrap();
        assert!(validate_driver_result(&request, &result).is_err());

        result = valid.clone();
        result.provider_response_body_sha256 = None;
        seal_driver_result(&mut result).unwrap();
        assert!(validate_driver_result(&request, &result).is_err());

        result = valid.clone();
        result.provider_observation.as_mut().unwrap().provider = "other".into();
        seal_driver_result(&mut result).unwrap();
        assert!(validate_driver_result(&request, &result).is_err());

        result = valid.clone();
        result.provider_observation.as_mut().unwrap().resolved_model = None;
        seal_driver_result(&mut result).unwrap();
        assert!(validate_driver_result(&request, &result).is_err());

        result = valid;
        result.provider_output_schema_sha256 = Some("e".repeat(64));
        seal_driver_result(&mut result).unwrap();
        assert!(validate_driver_result(&request, &result).is_err());
    }

    #[test]
    fn post_bundle_driver_failure_publishes_a_safe_no_draft_receipt() {
        let root = temp_path("generative-driver-failure");
        let pack = root.join("pack");
        let raw = root.join("raw-row.json");
        fs::create_dir_all(&root).unwrap();
        crate::commands::init::init_pack(&pack, "Driver Failure Pack", "gtm", true, false).unwrap();
        fs::write(&raw, "{\"company\":\"Synthetic Co\"}\n").unwrap();
        let request = generative_request_fixture(&pack, &raw);
        let run = root.join("published-run");
        let result = execute_run_inner_with_driver(
            &request,
            &run,
            || Ok(()),
            |driver_request, _| {
                assert_eq!(
                    driver_request.operation,
                    "model:prospect-fit-or-brief/normalization"
                );
                assert_eq!(
                    driver_request.canonical_output_schema,
                    crate::commands::schemas::schema(crate::cli::SchemaTarget::PromptOutput)
                );
                assert!(
                    driver_request.provider_output_schema["properties"]["normalized_prospect"]
                        ["properties"]
                        .get("persona")
                        .is_some()
                );
                assert!(
                    driver_request.provider_output_schema["properties"]["normalized_prospect"]
                        ["properties"]
                        .get("linkedin_url")
                        .is_none()
                );
                assert_eq!(
                    driver_request.provider_output_schema["properties"]["gaps"]["items"]["type"],
                    "string"
                );
                let mut result = DriverResultV2 {
                    contract: DRIVER_RESULT_V2.into(),
                    execution_id: driver_request.execution_id.clone(),
                    operation: driver_request.operation.clone(),
                    terminal_state: TerminalState::NoDraftRunnerFailed,
                    output: None,
                    provider_request_body_sha256: Some("d".repeat(64)),
                    provider_request_schema_id: Some(
                        "openai.responses.json-schema-request.v1".into(),
                    ),
                    provider_response_body_sha256: None,
                    provider_output_schema_sha256: Some(
                        driver_request.provider_output_schema_sha256.clone(),
                    ),
                    provider_observation: None,
                    diagnostic_code: Some("provider_timeout".into()),
                    result_sha256: String::new(),
                };
                seal_driver_result(&mut result)?;
                Ok(result)
            },
        )
        .unwrap();
        assert_eq!(result.terminal_state, TerminalState::NoDraftRunnerFailed);
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(run.join("run-receipt.json")).unwrap()).unwrap();
        assert!(receipt["output"].is_null());
        assert!(receipt["decision"].is_null());
        assert!(!run.join("private").exists());
        assert_eq!(
            crate::commands::run_verification::verify_run_files(
                Some(&run.join("run-bundle.json")),
                &run.join("run-receipt.json"),
                Some(&run),
            )
            .unwrap()["valid"],
            true
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn successful_provider_observation_is_bound_even_when_local_output_validation_blocks() {
        let root = temp_path("generative-provider-observation");
        let pack = root.join("pack");
        let raw = root.join("raw-row.json");
        fs::create_dir_all(&root).unwrap();
        crate::commands::init::init_pack(&pack, "Provider Observation Pack", "gtm", true, false)
            .unwrap();
        fs::write(&raw, "{\"company\":\"Synthetic Co\"}\n").unwrap();
        let request = generative_request_fixture(&pack, &raw);
        let run = root.join("published-run");
        let result = execute_run_inner_with_driver(
            &request,
            &run,
            || Ok(()),
            |driver_request, _| {
                let content = "{\"status\":\"ready\"}";
                let mut result = DriverResultV2 {
                    contract: DRIVER_RESULT_V2.into(),
                    execution_id: driver_request.execution_id.clone(),
                    operation: driver_request.operation.clone(),
                    terminal_state: TerminalState::Success,
                    output: Some(DriverOutputV2 {
                        schema_id: "mdp.prompt-output.v0".into(),
                        media_type: "application/json".into(),
                        content_utf8: content.into(),
                        byte_count: content.len() as u64,
                        sha256: crate::artifact_hash::sha256_hex(content.as_bytes()),
                    }),
                    provider_request_body_sha256: Some("d".repeat(64)),
                    provider_request_schema_id: Some(
                        "openai.responses.json-schema-request.v1".into(),
                    ),
                    provider_response_body_sha256: Some("f".repeat(64)),
                    provider_output_schema_sha256: Some(
                        driver_request.provider_output_schema_sha256.clone(),
                    ),
                    provider_observation: Some(DriverProviderObservationV2 {
                        provider: "openai".into(),
                        response_id: Some("resp_synthetic".into()),
                        resolved_model: Some("gpt-5-mini-2026-08-01".into()),
                    }),
                    diagnostic_code: None,
                    result_sha256: String::new(),
                };
                seal_driver_result(&mut result)?;
                Ok(result)
            },
        )
        .unwrap();

        assert_eq!(result.terminal_state, TerminalState::NoDraftOutputInvalid);
        let audit: crate::run_contracts::RunnerAuditV1 =
            serde_json::from_slice(&fs::read(run.join("runner-audit.json")).unwrap()).unwrap();
        assert_eq!(audit.provider_response_body_sha256, Some("f".repeat(64)));
        let observation = audit.provider_observation.unwrap();
        assert_eq!(observation.provider, "openai");
        assert_eq!(
            observation.resolved_model.as_deref(),
            Some("gpt-5-mini-2026-08-01")
        );
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(run.join("run-receipt.json")).unwrap()).unwrap();
        assert!(receipt["validation"].is_null());
        assert!(!run.join("artifacts/validation.json").exists());
        assert!(!run.join("private").exists());

        let bundle_schema = crate::commands::schemas::schema(crate::cli::SchemaTarget::RunBundleV1);
        let mut bundle: serde_json::Value =
            serde_json::from_slice(&fs::read(run.join("run-bundle.json")).unwrap()).unwrap();
        jsonschema::draft202012::validate(&bundle_schema, &bundle).unwrap();
        bundle["mode"] = serde_json::json!("deterministic");
        assert!(jsonschema::draft202012::validate(&bundle_schema, &bundle).is_err());
        assert_eq!(
            crate::commands::run_verification::verify_run_files(
                Some(&run.join("run-bundle.json")),
                &run.join("run-receipt.json"),
                Some(&run),
            )
            .unwrap()["valid"],
            true
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn computed_pack_readiness_block_never_invokes_the_generative_driver() {
        let root = temp_path("computed-generative-readiness-block");
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source_pack = repository.join("plugin/assets/templates/basic");
        let pack = root.join("pack");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir(&pack).unwrap();
        super::copy_pack(&source_pack, &pack).unwrap();
        for entry in fs::read_dir(pack.join(".mdp/evals")).expect("evals should be readable") {
            let path = entry.expect("eval entry should load").path();
            let raw = fs::read_to_string(&path).expect("eval should be readable");
            fs::write(
                path,
                raw.replace("category: prompt-output-validation", "category: proceed"),
            )
            .expect("eval should be writable");
        }
        let raw = root.join("raw-row.json");
        fs::write(&raw, "{\"company\":\"Synthetic Co\"}\n").unwrap();
        let request = generative_request_fixture(&pack, &raw);
        let run = root.join("published-run");

        let error = execute_run_inner_with_driver(
            &request,
            &run,
            || Ok(()),
            |_, _| panic!("blocked readiness must not invoke the model driver"),
        )
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<super::RunFailure>().unwrap().code(),
            "job-readiness-blocked"
        );
        assert!(!run.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn post_bundle_deadline_exhaustion_still_publishes_a_safe_no_draft_receipt() {
        let root = temp_path("generative-deadline-receipt");
        let pack = root.join("pack");
        let raw = root.join("raw-row.json");
        fs::create_dir_all(&root).unwrap();
        crate::commands::init::init_pack(&pack, "Deadline Pack", "gtm", true, false).unwrap();
        fs::write(&raw, "{\"company\":\"Synthetic Co\"}\n").unwrap();
        let mut request = generative_request_fixture(&pack, &raw);
        request.execution_policy.timeout_ms = 5_000;
        refresh_test_native_declarations(&mut request);
        let run = root.join("published-run");
        let result = execute_run_inner_with_driver(
            &request,
            &run,
            || Ok(()),
            |driver_request, _| {
                assert!(driver_request.provider_policy.timeout_ms < 5_000);
                std::thread::sleep(std::time::Duration::from_millis(
                    driver_request.provider_policy.timeout_ms
                        + super::MAX_FINALIZATION_RESERVE_MS
                        + 25,
                ));
                let mut result = DriverResultV2 {
                    contract: DRIVER_RESULT_V2.into(),
                    execution_id: driver_request.execution_id.clone(),
                    operation: driver_request.operation.clone(),
                    terminal_state: TerminalState::NoDraftRunnerFailed,
                    output: None,
                    provider_request_body_sha256: None,
                    provider_request_schema_id: None,
                    provider_response_body_sha256: None,
                    provider_output_schema_sha256: Some(
                        driver_request.provider_output_schema_sha256.clone(),
                    ),
                    provider_observation: None,
                    diagnostic_code: Some("provider_timeout".into()),
                    result_sha256: String::new(),
                };
                seal_driver_result(&mut result)?;
                Ok(result)
            },
        )
        .unwrap();

        assert_eq!(result.terminal_state, TerminalState::NoDraftRunnerFailed);
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(run.join("run-receipt.json")).unwrap()).unwrap();
        assert!(receipt["output"].is_null());
        assert!(receipt["decision"].is_null());
        assert!(!run.join("private").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn native_driver_never_executes_a_request_selected_path() {
        let mut request = sample_driver_request();
        seal_driver_request(&mut request).unwrap();
        let identity = DriverIdentity {
            driver_id: "mdp-native-openai".into(),
            implementation: "/tmp/attacker-controlled.mjs".into(),
            version: "1".into(),
            build_sha256: None,
            executable_sha256: Some(crate::artifact_hash::sha256_hex(
                super::BUNDLED_NATIVE_DRIVER_SOURCE.as_bytes(),
            )),
            image_digest: None,
            configuration_sha256: "b".repeat(64),
            dependency_lock_sha256: Some("c".repeat(64)),
            identity_provenance: EvidenceProvenance::MdpObserved,
        };
        let error = super::invoke_native_driver(&request, &identity).unwrap_err();
        assert_eq!(
            error.downcast_ref::<super::RunFailure>().unwrap().code(),
            "driver-implementation-not-bundled"
        );
    }

    #[test]
    fn subprocess_supervisor_kills_a_hung_driver_at_the_declared_deadline() {
        let node = super::resolve_node_executable().unwrap();
        let mut child = std::process::Command::new(node)
            .args(["--eval", "setTimeout(() => {}, 10000)"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let started = std::time::Instant::now();
        let error = super::supervise_child(&mut child, 25, 1024).unwrap_err();
        assert_eq!(
            error.downcast_ref::<super::RunFailure>().unwrap().code(),
            "driver-timeout"
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }

    #[test]
    fn subprocess_supervisor_stops_oversized_stdout_before_publication() {
        let node = super::resolve_node_executable().unwrap();
        let mut child = std::process::Command::new(node)
            .args(["--eval", "process.stdout.write('x'.repeat(10000))"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        // Cold Node startup can exceed two seconds on a contended CI runner.
        // Keep the deadline bounded while giving the child enough time to
        // exercise the stdout limit instead of racing the timeout branch.
        let error = super::supervise_child(&mut child, 10_000, 128).unwrap_err();
        assert_eq!(
            error.downcast_ref::<super::RunFailure>().unwrap().code(),
            "driver-result-too-large"
        );
    }

    #[test]
    fn invalid_proposal_output_commits_no_draft_receipt_without_output_authority() {
        let root = temp_path("invalid");
        let pack = root.join("pack");
        let runs = root.join("runs");
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        let output = root.join("invalid.json");
        fs::write(&output, "{}\n").unwrap();
        let request = request_fixture(pack.to_str().unwrap(), output.to_str().unwrap());

        let result = execute_run_inner(&request, &runs, || Ok(())).unwrap();
        assert_eq!(result.terminal_state, TerminalState::NoDraftOutputInvalid);
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(runs.join("run-receipt.json")).unwrap()).unwrap();
        assert!(receipt["output"].is_null());
        assert!(receipt["decision"].is_null());
        assert!(receipt["compiled_context"].is_null());
        assert!(receipt["validation"].is_object());
        assert!(!runs.join("private").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn valid_proposal_output_publishes_a_self_verifying_transaction() {
        let root = temp_path("success");
        let pack = root.join("pack");
        let run = root.join("published-run");
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let output = repository
            .join("examples/proposal-flow-video/fixtures/normalize-opportunity-output.json");
        let source_audit =
            repository.join("examples/proposal-flow-video/fixtures/source-audit.json");
        let mut request = request_fixture(pack.to_str().unwrap(), output.to_str().unwrap());
        request.inputs.push(LocalArtifactInput {
            logical_name: "source-audit".into(),
            source_path: source_audit.display().to_string(),
            schema_id: "mdp.source-audit.v0".into(),
            media_type: "application/json".into(),
            provenance_refs: vec![],
        });

        let result = execute_run_inner(&request, &run, || Ok(())).unwrap();
        assert_eq!(result.terminal_state, TerminalState::Success);
        assert!(run.join("run-bundle.json").is_file());
        assert!(run.join("run-receipt.json").is_file());
        assert!(!run.join("private").exists());
        let verification = crate::commands::run_verification::verify_run_files(
            Some(&run.join("run-bundle.json")),
            &run.join("run-receipt.json"),
            Some(&run),
        )
        .unwrap();
        assert_eq!(verification["valid"], true);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gtm_run_qualifies_only_from_the_bound_decision_input_set() {
        let root = temp_path("gtm-success");
        let run = root.join("published-run");
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let pack = repository.join("examples/clay-audiences-self-serve-enterprise-expansion");
        let fixtures = pack.join("fixtures");
        let mut request = gtm_request_fixture(&pack, &fixtures);

        let result = execute_run_inner(&request, &run, || Ok(())).unwrap();
        assert_eq!(result.terminal_state, TerminalState::Success);
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(run.join("run-receipt.json")).unwrap()).unwrap();
        assert_eq!(
            receipt["decision"]["schema_id"],
            "mdp.gtm-qualification-decision.v1"
        );
        assert_eq!(receipt["compiled_context"].is_object(), true);
        assert_eq!(receipt["decision"]["decision"], "qualified");
        assert_eq!(
            receipt["decision"]["reason_codes"],
            serde_json::json!(["ready"])
        );
        assert!(receipt["assurance"].as_array().unwrap().iter().any(|item| {
            item["dimension"] == "declared-input-isolation" && item["state"] == "observed"
        }));
        assert!(receipt["assurance"].as_array().unwrap().iter().any(|item| {
            item["dimension"] == "declared-input-byte-binding" && item["state"] == "verified"
        }));
        assert!(run.join("artifacts/output.json").is_file());
        assert!(!run.join("private").exists());
        let verification = crate::commands::run_verification::verify_run_files(
            Some(&run.join("run-bundle.json")),
            &run.join("run-receipt.json"),
            Some(&run),
        )
        .unwrap();
        assert_eq!(verification["valid"], true);

        request.execution_id = "run-gtm-reuse".into();
        assert!(execute_run_inner(&request, &run, || Ok(())).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gtm_lineage_input_versions_follow_the_normalized_contract() {
        assert_eq!(
            gtm_lineage_schema_ids(false),
            (
                "mdp.source-attempt-request.v1",
                "mdp.collected-attempt-results.v1"
            )
        );
        assert_eq!(
            gtm_lineage_schema_ids(true),
            (
                "mdp.source-attempt-request.v2",
                "mdp.collected-attempt-results.v2"
            )
        );
    }

    #[test]
    fn gtm_signal_aware_disqualification_is_a_governed_outcome() {
        assert!(governed_normalization_outcome(
            true,
            true,
            Some("disqualified")
        ));
        assert!(!governed_normalization_outcome(
            true,
            false,
            Some("disqualified")
        ));
        assert!(!governed_normalization_outcome(
            false,
            true,
            Some("disqualified")
        ));
    }

    #[test]
    fn gtm_decision_mapping_covers_qualified_disqualified_and_insufficient_context() {
        let qualified = gtm_artifacts_for_fit_status("fit");
        assert_eq!(qualified.decision.decision, "qualified");
        assert_eq!(qualified.decision.reason_codes, vec!["ready"]);

        let disqualified = gtm_artifacts_for_fit_status("disqualified");
        assert_eq!(disqualified.decision.decision, "no-draft");
        assert_eq!(disqualified.decision.reason_codes, vec!["disqualified"]);

        let insufficient = gtm_artifacts_for_fit_status("insufficient-context");
        assert_eq!(insufficient.decision.decision, "no-draft");
        assert_eq!(
            insufficient.decision.reason_codes,
            vec!["insufficient-context"]
        );
    }

    #[test]
    fn gtm_missing_required_evidence_publishes_no_authority() {
        let root = temp_path("gtm-missing-evidence");
        let run = root.join("run");
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let pack = repository.join("examples/clay-audiences-self-serve-enterprise-expansion");
        let fixtures = pack.join("fixtures");
        let mut request = gtm_request_fixture(&pack, &fixtures);
        request
            .inputs
            .retain(|input| input.logical_name != "source-attempt-request");

        assert!(execute_run_inner(&request, &run, || Ok(())).is_err());
        assert!(!run.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gtm_contradictory_source_binding_commits_invalid_no_draft_without_decision() {
        let root = temp_path("gtm-invalid-binding");
        let run = root.join("run");
        fs::create_dir_all(&root).unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let pack = repository.join("examples/clay-audiences-self-serve-enterprise-expansion");
        let fixtures = pack.join("fixtures");
        let normalized_path = root.join("contradictory-normalized.json");
        let mut normalized: serde_json::Value = serde_json::from_slice(
            &fs::read(fixtures.join("normalized-response-ready.json")).unwrap(),
        )
        .unwrap();
        normalized["source_attempt_request_sha256"] = serde_json::Value::String("f".repeat(64));
        fs::write(
            &normalized_path,
            serde_json::to_vec_pretty(&normalized).unwrap(),
        )
        .unwrap();
        let mut request = gtm_request_fixture(&pack, &fixtures);
        request.inputs[0].source_path = normalized_path.display().to_string();

        let result = execute_run_inner(&request, &run, || Ok(())).unwrap();
        assert_eq!(result.terminal_state, TerminalState::NoDraftOutputInvalid);
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(run.join("run-receipt.json")).unwrap()).unwrap();
        assert!(receipt["decision"].is_null());
        assert!(receipt["output"].is_null());
        assert!(receipt["compiled_context"].is_null());
        assert!(receipt["validation"].is_object());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn source_mutation_forces_audit_incomplete_and_no_output() {
        let root = temp_path("mutation");
        let pack = root.join("pack");
        let runs = root.join("runs");
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        let output = root.join("invalid.json");
        fs::write(&output, "{}\n").unwrap();
        let request = request_fixture(pack.to_str().unwrap(), output.to_str().unwrap());
        let mutate = output.clone();

        let result = execute_run_inner(&request, &runs, || {
            fs::write(&mutate, "{\"changed\":true}\n")?;
            Ok(())
        })
        .unwrap();
        assert_eq!(result.terminal_state, TerminalState::NoDraftAuditIncomplete);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pack_mutation_forces_audit_incomplete_and_no_output() {
        let root = temp_path("pack-mutation");
        let pack = root.join("pack");
        let run = root.join("run");
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let output = repository
            .join("examples/proposal-flow-video/fixtures/normalize-opportunity-output.json");
        let source_audit =
            repository.join("examples/proposal-flow-video/fixtures/source-audit.json");
        let mut request = request_fixture(pack.to_str().unwrap(), output.to_str().unwrap());
        request.inputs.push(LocalArtifactInput {
            logical_name: "source-audit".into(),
            source_path: source_audit.display().to_string(),
            schema_id: "mdp.source-audit.v0".into(),
            media_type: "application/json".into(),
            provenance_refs: vec![],
        });
        let manifest = pack.join(".mdp/manifest.yaml");

        let result = execute_run_inner(&request, &run, || {
            let mut bytes = fs::read(&manifest)?;
            bytes.extend_from_slice(b"\n# mutated during run\n");
            fs::write(&manifest, bytes)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(result.terminal_state, TerminalState::NoDraftAuditIncomplete);
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(run.join("run-receipt.json")).unwrap()).unwrap();
        assert!(receipt["output"].is_null());
        assert!(!run.join("artifacts/output.json").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn declared_input_symlink_is_refused_without_committed_run() {
        use std::os::unix::fs::symlink;
        let root = temp_path("symlink");
        let pack = root.join("pack");
        let runs = root.join("runs");
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        let actual = root.join("actual.json");
        let linked = root.join("linked.json");
        fs::write(&actual, "{}\n").unwrap();
        symlink(&actual, &linked).unwrap();
        let request = request_fixture(pack.to_str().unwrap(), linked.to_str().unwrap());
        assert!(execute_run_inner(&request, &runs, || Ok(())).is_err());
        assert!(!runs.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn declared_input_hard_link_is_refused_without_committed_run() {
        let root = temp_path("hard-link");
        let pack = root.join("pack");
        let runs = root.join("runs");
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        let actual = root.join("actual.json");
        let linked = root.join("linked.json");
        fs::write(&actual, "{}\n").unwrap();
        fs::hard_link(&actual, &linked).unwrap();
        let request = request_fixture(pack.to_str().unwrap(), linked.to_str().unwrap());
        assert!(execute_run_inner(&request, &runs, || Ok(())).is_err());
        assert!(!runs.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn semantic_input_roles_require_exact_logical_names() {
        let root = temp_path("exact-role");
        let pack = root.join("pack");
        let run = root.join("run");
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let output = repository
            .join("examples/proposal-flow-video/fixtures/normalize-opportunity-output.json");
        let mut request = request_fixture(pack.to_str().unwrap(), output.to_str().unwrap());
        request.inputs[0].logical_name = "backup-prompt-output".into();

        assert!(execute_run_inner(&request, &run, || Ok(())).is_err());
        assert!(!run.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deadline_failure_removes_transaction_and_output_claim() {
        let root = temp_path("timeout-cleanup");
        let pack = root.join("pack");
        let run = root.join("run");
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        let output = root.join("invalid.json");
        fs::write(&output, "{}\n").unwrap();
        let mut request = request_fixture(pack.to_str().unwrap(), output.to_str().unwrap());
        request.execution_policy.timeout_ms = 1;

        assert!(
            execute_run_inner(&request, &run, || {
                std::thread::sleep(std::time::Duration::from_millis(5));
                Ok(())
            })
            .is_err()
        );
        assert!(!run.exists());
        let leftovers = fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".run."))
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "leftover transaction state: {leftovers:?}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recovery_claim_binds_exact_transaction_leaf_and_is_removed() {
        let root = temp_path("recovery-claim");
        let pack = root.join("pack");
        let run = root.join("run");
        let claim = root.join(".run.mdp-run.claim");
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        let output = root.join("invalid.json");
        fs::write(&output, "{}\n").unwrap();
        let request = request_fixture(pack.to_str().unwrap(), output.to_str().unwrap());

        let result = execute_run_inner(&request, &run, || {
            let bytes = fs::read(&claim)?;
            assert!(bytes.len() <= 512);
            assert!(bytes.ends_with(b"\n"));
            let value: serde_json::Value = serde_json::from_slice(&bytes)?;
            assert_eq!(value["contract"], "mdp.run-recovery-claim.v1");
            assert_eq!(value["execution_id"], "run-1");
            let transaction_leaf = value["transaction_leaf"].as_str().unwrap();
            assert!(transaction_leaf.starts_with(".run.tmp-"));
            let nonce = transaction_leaf.strip_prefix(".run.tmp-").unwrap();
            assert!(nonce.len() >= 16);
            assert!(
                nonce
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            );
            assert!(!transaction_leaf.contains(['/', '\\']));
            assert!(root.join(transaction_leaf).is_dir());
            assert_eq!(value.as_object().unwrap().len(), 3);
            Ok(())
        })
        .unwrap();

        assert_eq!(result.terminal_state, TerminalState::NoDraftOutputInvalid);
        assert!(!claim.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn declared_input_metadata_limit_is_checked_before_reading() {
        let root = temp_path("input-bound");
        let pack = root.join("pack");
        let run = root.join("run");
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        let output = root.join("oversized.json");
        let file = fs::File::create(&output).unwrap();
        file.set_len(2_000_000).unwrap();
        let request = request_fixture(pack.to_str().unwrap(), output.to_str().unwrap());

        assert!(execute_run_inner(&request, &run, || Ok(())).is_err());
        assert!(!run.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn top_level_mdp_symlink_is_refused() {
        use std::os::unix::fs::symlink;
        let root = temp_path("pack-root-link");
        let actual = root.join("actual");
        let linked = root.join("linked");
        let run = root.join("run");
        init_pack(&actual, "Proposal Run", "proposal", true, false).unwrap();
        fs::create_dir_all(&linked).unwrap();
        symlink(actual.join(".mdp"), linked.join(".mdp")).unwrap();
        let output = root.join("invalid.json");
        fs::write(&output, "{}\n").unwrap();
        let request = request_fixture(linked.to_str().unwrap(), output.to_str().unwrap());

        assert!(execute_run_inner(&request, &run, || Ok(())).is_err());
        assert!(!run.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn output_directory_must_resolve_outside_pack_before_any_write() {
        use std::os::unix::fs::symlink;

        let root = temp_path("output-containment");
        let pack = root.join("pack");
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        let output = root.join("invalid.json");
        fs::write(&output, "{}\n").unwrap();
        let output_paths = [
            pack.clone(),
            pack.join("nested").join("run"),
            pack.join("nested").join("..").join("canonical-run"),
        ];
        for (index, output_path) in output_paths.iter().enumerate() {
            let mut request = request_fixture(pack.to_str().unwrap(), output.to_str().unwrap());
            request.execution_id = format!("inside-pack-{index}");
            let error = execute_run_inner(&request, output_path, || Ok(())).unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<super::RunFailure>()
                    .expect("containment failure should be sanitized")
                    .code(),
                "output-directory-inside-pack",
                "output path {} (case {index})",
                output_path.display()
            );
        }

        let pack_alias = root.join("pack-alias");
        symlink(&pack, &pack_alias).unwrap();
        let mut request = request_fixture(pack.to_str().unwrap(), output.to_str().unwrap());
        request.execution_id = "inside-pack-symlink".into();
        let error =
            execute_run_inner(&request, &pack_alias.join("symlink-run"), || Ok(())).unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<super::RunFailure>()
                .expect("symlink containment failure should be sanitized")
                .code(),
            "output-directory-inside-pack"
        );

        let safe_output = root.join("pack-scratch").join("run");
        let mut request = request_fixture(pack.to_str().unwrap(), output.to_str().unwrap());
        request.execution_id = "outside-pack".into();
        let result = execute_run_inner(&request, &safe_output, || Ok(())).unwrap();
        assert_eq!(result.terminal_state, TerminalState::NoDraftOutputInvalid);
        assert!(safe_output.join("run-receipt.json").is_file());
        assert!(!pack.join("nested").exists());
        assert!(pack_alias.exists());
        let _ = fs::remove_dir_all(root);
    }

    fn request_fixture(pack: &str, output: &str) -> RunRequestV1 {
        RunRequestV1 {
            contract: "mdp.run-request.v1".into(),
            execution_id: "run-1".into(),
            created_at: "2026-08-03T00:00:00Z".into(),
            profile: "proposal".into(),
            operation: "validate-existing-output".into(),
            mode: RunMode::Deterministic,
            job_identity: None,
            pack_dir: pack.into(),
            pack_release_id: "proposal-release-1".into(),
            prompt: None,
            inputs: vec![LocalArtifactInput {
                logical_name: "prompt-output".into(),
                source_path: output.into(),
                schema_id: "mdp.prompt-output.v0".into(),
                media_type: "application/json".into(),
                provenance_refs: vec![],
            }],
            execution_policy: ExecutionPolicy {
                environment_allowlist: vec![],
                filesystem_mode: "private-staging".into(),
                tool_mode: "none".into(),
                network_mode: "none".into(),
                authorized_endpoints: vec![],
                max_input_bytes: 1_048_576,
                max_output_bytes: 1_048_576,
                timeout_ms: 30_000,
                retention_policy: "receipt-only".into(),
            },
            driver: None,
            model: None,
        }
    }

    fn gtm_artifacts_for_fit_status(status: &str) -> super::SuccessArtifacts {
        let request = request_fixture("unused", "unused");
        let bundle = RunBundleV1 {
            contract: "mdp.run-bundle.v1".into(),
            execution_id: request.execution_id.clone(),
            created_at: request.created_at.clone(),
            profile: "gtm".into(),
            operation: "qualify".into(),
            mode: RunMode::Deterministic,
            job_identity: None,
            pack: PackAuthority {
                release_id: "release-1".into(),
                pack_id: "pack-1".into(),
                version: super::MDP_RUNTIME_VERSION.into(),
                profile_id: "gtm".into(),
                portable_digest: "a".repeat(64),
                files: vec![],
            },
            prompt: None,
            inputs: vec![],
            execution_policy_sha256: "b".repeat(64),
            driver: None,
            model: None,
            model_facts: None,
        };
        gtm_success_artifacts(
            &request,
            &bundle,
            &"c".repeat(64),
            serde_json::json!({
                "status": status,
                "context": {},
                "matches": [],
                "disqualifiers": []
            }),
        )
        .unwrap()
    }

    fn gtm_request_fixture(pack: &Path, fixtures: &Path) -> RunRequestV1 {
        let input =
            |logical_name: &str, path: std::path::PathBuf, schema_id: &str| LocalArtifactInput {
                logical_name: logical_name.into(),
                source_path: path.display().to_string(),
                schema_id: schema_id.into(),
                media_type: "application/json".into(),
                provenance_refs: vec![],
            };
        RunRequestV1 {
            contract: "mdp.run-request.v1".into(),
            execution_id: "run-gtm-1".into(),
            created_at: "2026-08-03T00:00:00Z".into(),
            profile: "gtm".into(),
            operation: "qualify".into(),
            mode: RunMode::Deterministic,
            job_identity: None,
            pack_dir: pack.display().to_string(),
            pack_release_id: "clay-expansion-release-1".into(),
            prompt: None,
            inputs: vec![
                input(
                    "normalized-decision-input",
                    fixtures.join("normalized-response-ready.json"),
                    "mdp.normalized-decision-input.v2",
                ),
                input(
                    "source-binding",
                    fixtures.join("source-binding-clay-adapter.json"),
                    "mdp.source-binding.v2",
                ),
                input(
                    "source-attempt-request",
                    fixtures.join("source-attempt-request.json"),
                    "mdp.source-attempt-request.v2",
                ),
                input(
                    "collected-attempt-results",
                    fixtures.join("collected-attempt-results.json"),
                    "mdp.collected-attempt-results.v2",
                ),
                LocalArtifactInput {
                    logical_name: "bound-prompt".into(),
                    source_path: pack
                        .join(".mdp/prompts/normalize-prospect.yaml")
                        .display()
                        .to_string(),
                    schema_id: "mdp.prompt.v0".into(),
                    media_type: "application/yaml".into(),
                    provenance_refs: vec![],
                },
            ],
            execution_policy: ExecutionPolicy {
                environment_allowlist: vec![],
                filesystem_mode: "private-staging".into(),
                tool_mode: "none".into(),
                network_mode: "none".into(),
                authorized_endpoints: vec![],
                max_input_bytes: 131_072,
                max_output_bytes: 1_048_576,
                timeout_ms: 30_000,
                retention_policy: "receipt-only".into(),
            },
            driver: None,
            model: None,
        }
    }

    fn sample_driver_request() -> DriverRequestV2 {
        let authority = ArtifactAuthority {
            logical_name: "prompt/000-prompt".into(),
            schema_id: "mdp.prompt.v1".into(),
            media_type: "application/yaml".into(),
            byte_count: 6,
            sha256: crate::artifact_hash::sha256_hex(b"prompt"),
            provenance: EvidenceProvenance::MdpObserved,
            provenance_refs: vec![],
        };
        let schema = serde_json::json!({
            "type": "object",
            "properties": {"status": {"type": "string"}},
            "required": ["status"],
            "additionalProperties": false
        });
        let schema_sha = crate::artifact_hash::canonical_json_sha256(&schema).unwrap();
        DriverRequestV2 {
            contract: DRIVER_REQUEST_V2.into(),
            execution_id: "exec-1".into(),
            profile: "gtm".into(),
            operation: "model:outbound-copy-brief/generation".into(),
            job_identity: JobIdentity {
                job_id: "outbound-copy-brief".into(),
                idempotency_key: "idem-1".into(),
            },
            phase: "generation".into(),
            prompt_id: "generate-outbound-copy-v1".into(),
            prompt_version: "2".into(),
            prompt_canonical_sha256: "a".repeat(64),
            prompt: DriverArtifactV2 {
                authority: authority.clone(),
                content_utf8: "prompt".into(),
            },
            prompt_invocation: DriverArtifactV2 {
                authority: ArtifactAuthority {
                    logical_name: "private/prompt-invocation.json".into(),
                    schema_id: "mdp.prompt-invocation.v1".into(),
                    media_type: "application/json".into(),
                    byte_count: 3,
                    sha256: crate::artifact_hash::sha256_hex(b"{}\n"),
                    provenance: EvidenceProvenance::MdpObserved,
                    provenance_refs: vec![],
                },
                content_utf8: "{}\n".into(),
            },
            inputs: vec![],
            canonical_output_schema: schema.clone(),
            canonical_output_schema_sha256: schema_sha.clone(),
            provider_output_schema: schema,
            provider_output_schema_sha256: schema_sha,
            provider_policy: DriverProviderPolicyV2 {
                provider: "openai".into(),
                requested_model: "gpt-5-mini".into(),
                authorized_endpoint: super::OFFICIAL_OPENAI_RESPONSES_ENDPOINT.into(),
                timeout_ms: 30_000,
                max_output_bytes: 1_048_576,
            },
            execution_policy_sha256: "f".repeat(64),
            request_sha256: String::new(),
        }
    }

    fn generative_request_fixture(pack: &Path, raw: &Path) -> RunRequestV1 {
        let driver_sha =
            crate::artifact_hash::sha256_hex(super::BUNDLED_NATIVE_DRIVER_SOURCE.as_bytes());
        let mut request = RunRequestV1 {
            contract: "mdp.run-request.v1".into(),
            execution_id: "run-generative-1".into(),
            created_at: "2026-08-14T00:00:00Z".into(),
            profile: "gtm".into(),
            operation: "model:prospect-fit-or-brief/normalization".into(),
            mode: RunMode::Generative,
            job_identity: Some(JobIdentity {
                job_id: "prospect-fit-or-brief".into(),
                idempotency_key: "idem-generative-1".into(),
            }),
            pack_dir: pack.display().to_string(),
            pack_release_id: "basic-release-1".into(),
            prompt: Some(LocalArtifactInput {
                logical_name: "normalize-prospect-row".into(),
                source_path: pack
                    .join(".mdp/prompts/normalize-prospect.yaml")
                    .display()
                    .to_string(),
                schema_id: "mdp.prompt.v1".into(),
                media_type: "application/yaml".into(),
                provenance_refs: vec![],
            }),
            inputs: vec![LocalArtifactInput {
                logical_name: "raw_row".into(),
                source_path: raw.display().to_string(),
                schema_id: "mdp.input.raw-row.v1".into(),
                media_type: "application/json".into(),
                provenance_refs: vec![],
            }],
            execution_policy: ExecutionPolicy {
                environment_allowlist: vec!["OPENAI_API_KEY".into()],
                filesystem_mode: "private-staging".into(),
                tool_mode: "none".into(),
                network_mode: "authorized-endpoints-only".into(),
                authorized_endpoints: vec![super::OFFICIAL_OPENAI_RESPONSES_ENDPOINT.into()],
                max_input_bytes: 131_072,
                max_output_bytes: 1_048_576,
                timeout_ms: 30_000,
                retention_policy: "receipt-only".into(),
            },
            driver: Some(DriverIdentity {
                driver_id: "mdp-native-openai".into(),
                implementation: super::BUNDLED_NATIVE_DRIVER_ID.into(),
                version: super::MDP_RUNTIME_VERSION.into(),
                build_sha256: None,
                executable_sha256: Some(driver_sha),
                image_digest: None,
                configuration_sha256: "b".repeat(64),
                dependency_lock_sha256: Some("d".repeat(64)),
                identity_provenance: EvidenceProvenance::MdpObserved,
            }),
            model: Some(ModelIdentity {
                provider: "openai".into(),
                requested_model: "gpt-5-mini".into(),
                resolved_model: None,
                authorized_endpoint: super::OFFICIAL_OPENAI_RESPONSES_ENDPOINT.into(),
                parameters_sha256: "c".repeat(64),
                session_behavior: AssuranceEvidenceState::NotApplicable,
                cache_behavior: AssuranceEvidenceState::Unknown,
                storage_behavior: AssuranceEvidenceState::Declared,
            }),
        };
        refresh_test_native_declarations(&mut request);
        request
    }

    fn refresh_test_native_declarations(request: &mut RunRequestV1) {
        let root = temp_path("native-declarations");
        let staged_pack = root.join("pack");
        let staged_inputs_dir = root.join("inputs");
        let staged_prompt_dir = root.join("prompt");
        fs::create_dir_all(&staged_inputs_dir).unwrap();
        fs::create_dir_all(&staged_prompt_dir).unwrap();
        fs::create_dir_all(&staged_pack).unwrap();
        super::copy_pack(Path::new(&request.pack_dir), &staged_pack).unwrap();
        let staged = super::stage_inputs(request, &staged_inputs_dir).unwrap();
        let staged_prompt = super::stage_local_artifact(
            request.prompt.as_ref().unwrap(),
            &staged_prompt_dir,
            0,
            request.execution_policy.max_input_bytes,
            "prompt",
        )
        .unwrap();
        let manifest = crate::pack_io::read_manifest(&staged_pack).unwrap();
        let Ok(prepared) = super::prepare_native_request(
            request,
            &manifest,
            &staged_pack,
            &staged_prompt,
            &staged,
        ) else {
            let _ = fs::remove_dir_all(root);
            return;
        };
        let node = super::resolve_node_executable().unwrap();
        let node_sha = crate::artifact_hash::sha256_hex(
            &super::read_bounded(&node, 200 * 1024 * 1024, "node executable").unwrap(),
        );
        let driver = request.driver.as_mut().unwrap();
        driver.dependency_lock_sha256 = Some(node_sha.clone());
        let driver_projection = super::driver_configuration_projection(
            driver,
            driver.executable_sha256.clone().unwrap(),
            node_sha,
        );
        driver.configuration_sha256 = super::projection_hash(
            super::DRIVER_CONFIGURATION_PROJECTION_V1,
            &driver_projection,
        )
        .unwrap();
        let model = request.model.as_ref().unwrap();
        let model_facts = super::model_parameters_facts(
            model,
            &prepared,
            request.execution_policy.timeout_ms,
            request.execution_policy.max_output_bytes,
        );
        let model_projection: super::ModelParametersProjectionV1 = (&model_facts).into();
        request.model.as_mut().unwrap().parameters_sha256 =
            super::projection_hash(super::MODEL_PARAMETERS_PROJECTION_V1, &model_projection)
                .unwrap();
        super::bind_native_identity(request, &prepared).unwrap();
        let _ = fs::remove_dir_all(root);
    }

    fn temp_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("mdp-run-runtime-{label}-{}", nonce()))
    }

    fn nonce() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
