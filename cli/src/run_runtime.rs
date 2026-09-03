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
use crate::commands::v3_normalization::{
    V3SealInputs, reject_host_field_injection, seal_v3_envelope, v3_issue_diagnostic_detail,
    v3_schema_error_detail, v3_semantic_provider_schema, v3_static_diagnostic_detail,
    validate_v3_sealed_envelope, validate_v3_semantic_payload,
};
use crate::constants::{
    COLLECTED_ATTEMPT_RESULTS_CONTRACT_V2, GENERATED_PACK_DIRECTORIES,
    NORMALIZED_DECISION_INPUT_CONTRACT, NORMALIZED_DECISION_INPUT_CONTRACT_V2,
    NORMALIZED_DECISION_INPUT_CONTRACT_V3, REQUIREMENTS_MODEL_CONTEXT_CONTRACT_V1,
    ROUTED_CONTEXT_CONTRACT, SOURCE_ATTEMPT_REQUEST_CONTRACT_V2, SOURCE_BINDING_CONTRACT_V2,
    V3_OUTCOME_KIND,
};
use crate::model_steps::{CompiledModelStepV1, ModelStepPhase, resolve_selected_model_step};
use crate::models::ClassificationTaxonomy;
use crate::pack_io::{read_manifest, resolve_pack_path};
use crate::run_contracts::{
    ArtifactAuthority, AssuranceDimension, AssuranceEvidenceState, DEADLINE_OBSERVATION_V1,
    DRIVER_CONFIGURATION_PROJECTION_V1, DRIVER_REQUEST_V2, DRIVER_RESULT_V2, DeadlineObservationV1,
    DeadlineOutcome, DeadlinePhase, DecisionAuthority, DiagnosticDetailV1, DriverArtifactV2,
    DriverConfigurationProjectionV1, DriverOutputV2, DriverProviderObservationV2,
    DriverProviderPolicyV2, DriverRequestV2, DriverResultV2, EvidenceProvenance,
    IdentityObservationV1, MDP_RUNTIME_VERSION, MODEL_PARAMETERS_PROJECTION_V1,
    ModelParametersFactsV1, ModelParametersProjectionV1, OPENAI_PROVIDER_REQUEST_SCHEMA_ID,
    PROVIDER_REQUEST_NOT_OBSERVED_V1, PROVIDER_REQUEST_RELATION_V1, PackAuthority,
    ProviderRequestObservationV1, RUN_BUNDLE_V1, RUN_RECEIPT_V1, RUN_REQUEST_V1, RUNNER_AUDIT_V1,
    RunBundleV1, RunMode, RunReceiptV1, RunRequestV1, RunnerAuditV1, TerminalState,
};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
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
const REVIEW: &str = "review";
const GTM_PROFILE: &str = "gtm";
const QUALIFY: &str = "qualify";
const MAX_PACK_FILES: usize = 10_000;
const MAX_PACK_BYTES: u64 = 100 * 1024 * 1024;
const MAX_EXECUTION_ID_BYTES: usize = 128;
const MAX_OUTPUT_LEAF_BYTES: usize = 120;
// Exact compact-JSON ceiling for a v2 claim with the accepted 128-byte
// execution ID, 158-byte transaction leaf, maximum-width u32/u64 fields, and
// its terminating newline. The boundary test below locks this to the record.
const MAX_RECOVERY_CLAIM_BYTES: usize = 536;
const MIN_RECOVERY_AGE_SECONDS: u64 = 300;
const MAX_POLICY_INPUT_BYTES: u64 = 100 * 1024 * 1024;
// Native requests also contain the prompt envelope and projected provider
// schema. Keep the public generative input budget well below the driver's
// 2 MiB serialized-request ceiling so requests cannot pass preflight and then
// fail only after the immutable bundle has been published.
const MAX_NATIVE_DECLARED_INPUT_BYTES: u64 = 128 * 1024;
const MAX_NATIVE_SERIALIZED_REQUEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_POLICY_OUTPUT_BYTES: u64 = 1024 * 1024;
const MAX_POLICY_DIAGNOSTICS: usize = 4;
const MAX_POLICY_DIAGNOSTIC_BYTES: usize = 4096;
const MAX_DIAGNOSTIC_INPUT_BYTES: usize = 64;
const DRIVER_RESULT_ENVELOPE_BYTES: u64 = 64 * 1024;
const MAX_FINALIZATION_RESERVE_MS: u64 = 250;
pub(crate) const RECOMMENDED_TIMEOUT_MS: u64 = 60_000;
const MAX_TRANSPORT_TIMEOUT_MS: u64 = 300_000;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostic_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostic_phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostic_detail: Option<DiagnosticDetailV1>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RunFailureKind {
    Preflight,
    PolicyBlocked,
    RunnerFailed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunDiagnostic {
    pub(crate) stage: &'static str,
    pub(crate) gate: &'static str,
    pub(crate) code: &'static str,
    pub(crate) input: Option<Cow<'static, str>>,
    pub(crate) field: Option<&'static str>,
    pub(crate) expected: DiagnosticValue,
    pub(crate) observed: DiagnosticValue,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DiagnosticValue {
    pub(crate) kind: &'static str,
    pub(crate) value: DiagnosticScalar,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum DiagnosticScalar {
    Text(&'static str),
    Count(u64),
}

#[derive(Debug)]
pub(crate) struct RunFailure {
    kind: RunFailureKind,
    code: &'static str,
    diagnostics: Vec<RunDiagnostic>,
    deadline: Option<DeadlineObservationV1>,
    diagnostic_detail: Option<DiagnosticDetailV1>,
}

impl RunFailure {
    fn new(kind: RunFailureKind, code: &'static str) -> Self {
        let diagnostics = if matches!(kind, RunFailureKind::PolicyBlocked) {
            vec![fallback_policy_diagnostic(code)]
        } else {
            Vec::new()
        };
        Self {
            kind,
            code,
            diagnostics,
            deadline: None,
            diagnostic_detail: None,
        }
    }

    fn with_diagnostic(
        kind: RunFailureKind,
        code: &'static str,
        diagnostic: RunDiagnostic,
    ) -> Self {
        let mut failure = Self {
            kind,
            code,
            diagnostics: if matches!(kind, RunFailureKind::PolicyBlocked) {
                vec![diagnostic]
            } else {
                Vec::new()
            },
            deadline: None,
            diagnostic_detail: None,
        };
        failure.bound_diagnostics();
        failure
    }

    pub(crate) fn kind(&self) -> RunFailureKind {
        self.kind
    }

    pub(crate) fn code(&self) -> &'static str {
        self.code
    }

    pub(crate) fn diagnostics(&self) -> &[RunDiagnostic] {
        &self.diagnostics
    }

    pub(crate) fn deadline(&self) -> Option<&DeadlineObservationV1> {
        self.deadline.as_ref()
    }

    pub(crate) fn diagnostic_detail(&self) -> Option<&DiagnosticDetailV1> {
        self.diagnostic_detail.as_ref()
    }

    fn bound_diagnostics(&mut self) {
        self.diagnostics.truncate(MAX_POLICY_DIAGNOSTICS);
        while self.diagnostics.len() > 1
            && serde_json::to_vec(&self.diagnostics)
                .is_ok_and(|bytes| bytes.len() > MAX_POLICY_DIAGNOSTIC_BYTES)
        {
            self.diagnostics.pop();
        }
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

fn run_failure_with_diagnostic(
    kind: RunFailureKind,
    code: &'static str,
    diagnostic: RunDiagnostic,
) -> anyhow::Error {
    anyhow::Error::new(RunFailure::with_diagnostic(kind, code, diagnostic))
}

fn run_failure_with_diagnostic_detail(
    kind: RunFailureKind,
    code: &'static str,
    diagnostic_detail: DiagnosticDetailV1,
) -> anyhow::Error {
    let mut failure = RunFailure::new(kind, code);
    failure.diagnostic_detail = Some(diagnostic_detail);
    anyhow::Error::new(failure)
}

fn run_failure_with_deadline(
    kind: RunFailureKind,
    code: &'static str,
    deadline: DeadlineObservationV1,
) -> anyhow::Error {
    let mut failure = RunFailure::new(kind, code);
    failure.deadline = Some(deadline);
    anyhow::Error::new(failure)
}

fn diagnostic_value(kind: &'static str, value: &'static str) -> DiagnosticValue {
    DiagnosticValue {
        kind,
        value: DiagnosticScalar::Text(value),
    }
}

fn count_value(value: usize) -> DiagnosticValue {
    DiagnosticValue {
        kind: "count",
        value: DiagnosticScalar::Count(value as u64),
    }
}

fn policy_diagnostic(
    stage: &'static str,
    gate: &'static str,
    code: &'static str,
    input: Option<&'static str>,
    field: Option<&'static str>,
    expected: DiagnosticValue,
    observed: DiagnosticValue,
) -> RunDiagnostic {
    RunDiagnostic {
        stage,
        gate,
        code,
        input: input.map(Cow::Borrowed),
        field,
        expected,
        observed,
    }
}

fn fallback_policy_diagnostic(code: &str) -> RunDiagnostic {
    let (stage, gate, category, input, field, expected, observed) = match code {
        "draft-readiness-blocked" | "job-readiness-blocked" | "job-readiness-unavailable" => (
            "generative-preflight",
            "routed-context-readiness",
            "readiness-failure",
            None,
            None,
            diagnostic_value("readiness", "ready"),
            diagnostic_value("readiness", "blocked"),
        ),
        "required-model-input-missing" => (
            "generative-preflight",
            "declared-inputs",
            "missing-required-field",
            None,
            None,
            count_value(1),
            count_value(0),
        ),
        "undeclared-model-input" => (
            "generative-preflight",
            "declared-inputs",
            "disallowed-field",
            None,
            Some("/unknown-field"),
            diagnostic_value("field", "declared-input"),
            diagnostic_value("field", "unknown-field"),
        ),
        "routed-context-invalid" => (
            "generative-preflight",
            "routed-context-schema",
            "internal-contract-mismatch",
            Some("routed_context"),
            None,
            diagnostic_value("contract", ROUTED_CONTEXT_CONTRACT),
            diagnostic_value("contract", "unavailable"),
        ),
        "routed-context-stale-binding" => (
            "generative-preflight",
            "routed-context-readiness",
            "stale-binding",
            Some("routed_context"),
            None,
            diagnostic_value("binding", "matched"),
            diagnostic_value("binding", "mismatch"),
        ),
        "source-integrity-failed" => (
            "source-integrity",
            "declared-input-immutability",
            "stale-binding",
            None,
            None,
            diagnostic_value("binding", "unchanged"),
            diagnostic_value("binding", "changed"),
        ),
        _ => (
            "run-preflight",
            "policy",
            "internal-contract-mismatch",
            None,
            None,
            diagnostic_value("binding", "available"),
            diagnostic_value("binding", "unavailable"),
        ),
    };
    policy_diagnostic(stage, gate, category, input, field, expected, observed)
}

fn bounded_diagnostic_input(name: &str) -> String {
    let mut value = name
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.') {
                byte as char
            } else {
                '_'
            }
        })
        .take(MAX_DIAGNOSTIC_INPUT_BYTES)
        .collect::<String>();
    if value.is_empty() {
        value.push_str("unknown");
    }
    value
}

fn source_integrity_diagnostic(subject: &str) -> RunDiagnostic {
    let mut diagnostic = fallback_policy_diagnostic("source-integrity-failed");
    diagnostic.input = Some(Cow::Owned(bounded_diagnostic_input(subject)));
    diagnostic
}

fn source_integrity_input_diagnostic(input: &StagedInput) -> RunDiagnostic {
    source_integrity_diagnostic(&input.logical_name)
}

#[derive(Debug)]
struct RunDeadline {
    started_at: Instant,
    started_wall_ms: u64,
    runtime_configured_ms: u64,
    transport_configured_ms: Option<u64>,
    provider_configured_ms: u64,
    finalization_reserve_ms: u64,
    effective_limit_ms: u64,
    warnings: Vec<String>,
    phase: Cell<DeadlinePhase>,
    terminal: RefCell<Option<DeadlineObservationV1>>,
}

struct TransactionOutcome {
    bundle_sha256: String,
    receipt: RunReceiptV1,
    diagnostics: Vec<RunDiagnostic>,
    deadline: Option<DeadlineObservationV1>,
}

impl RunDeadline {
    fn new(timeout_ms: u64) -> Self {
        Self::try_new(timeout_ms, None).expect("validated execution timeout")
    }

    fn try_new(runtime_configured_ms: u64, transport_configured_ms: Option<u64>) -> Result<Self> {
        if runtime_configured_ms <= MAX_FINALIZATION_RESERVE_MS {
            return Err(run_failure(
                RunFailureKind::Preflight,
                "deadline-reserve-underflow",
            ));
        }
        if let Some(transport) = transport_configured_ms {
            if !(MAX_FINALIZATION_RESERVE_MS + 1..=MAX_TRANSPORT_TIMEOUT_MS).contains(&transport) {
                return Err(run_failure(
                    RunFailureKind::Preflight,
                    "transport-timeout-invalid",
                ));
            }
        }
        let transport_effective =
            transport_configured_ms.map(|value| value.saturating_sub(MAX_FINALIZATION_RESERVE_MS));
        let effective_limit_ms =
            runtime_configured_ms.min(transport_effective.unwrap_or(runtime_configured_ms));
        let mut warnings = Vec::new();
        if let Some(transport) = transport_configured_ms {
            if transport_effective.is_some_and(|value| value < runtime_configured_ms) {
                warnings.push("outer-timeout-truncates-runtime".into());
            } else if transport > runtime_configured_ms {
                warnings.push("outer-timeout-cannot-extend-inner".into());
            }
        }
        Ok(Self {
            started_at: Instant::now(),
            started_wall_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| run_failure(RunFailureKind::Preflight, "deadline-clock-invalid"))?
                .as_millis()
                .min(u128::from(u64::MAX)) as u64,
            runtime_configured_ms,
            transport_configured_ms,
            provider_configured_ms: RECOMMENDED_TIMEOUT_MS,
            finalization_reserve_ms: MAX_FINALIZATION_RESERVE_MS,
            effective_limit_ms,
            warnings,
            phase: Cell::new(DeadlinePhase::Preflight),
            terminal: RefCell::new(None),
        })
    }

    fn check_phase(&self, phase: DeadlinePhase) -> Result<()> {
        self.phase.set(phase);
        if self.started_at.elapsed() >= Duration::from_millis(self.effective_limit_ms) {
            self.record_terminal(self.observation(
                DeadlineOutcome::TimedOut,
                phase,
                TerminalState::NoDraftRunnerFailed,
            ));
            return Err(run_failure_with_deadline(
                RunFailureKind::RunnerFailed,
                "execution-timeout",
                self.terminal
                    .borrow()
                    .clone()
                    .expect("terminal deadline observation was recorded"),
            ));
        }
        Ok(())
    }

    fn expired(&self) -> bool {
        self.started_at.elapsed() >= Duration::from_millis(self.effective_limit_ms)
    }

    fn driver_timeout_ms(&self) -> Option<u64> {
        let elapsed = self.started_at.elapsed();
        let remaining = Duration::from_millis(self.effective_limit_ms).checked_sub(elapsed)?;
        let driver_budget =
            remaining.checked_sub(Duration::from_millis(MAX_FINALIZATION_RESERVE_MS))?;
        u64::try_from(driver_budget.as_millis())
            .ok()
            .filter(|value| *value > 0)
    }

    fn provider_deadline(&self) -> Instant {
        self.started_at
            + Duration::from_millis(
                self.effective_limit_ms
                    .saturating_sub(self.finalization_reserve_ms),
            )
    }

    fn provider_deadline_at_ms(&self) -> u64 {
        self.started_wall_ms.saturating_add(
            self.effective_limit_ms
                .saturating_sub(self.finalization_reserve_ms),
        )
    }

    fn record_terminal(&self, observation: DeadlineObservationV1) {
        let mut terminal = self.terminal.borrow_mut();
        if terminal.is_none() {
            *terminal = Some(observation);
        }
    }

    fn terminal_observation(&self) -> Option<DeadlineObservationV1> {
        self.terminal.borrow().clone()
    }

    fn record_cancelled(&self, phase: DeadlinePhase) {
        self.phase.set(phase);
        self.record_terminal(self.observation(
            DeadlineOutcome::Cancelled,
            phase,
            TerminalState::NoDraftRunnerFailed,
        ));
    }

    fn remaining_ms(&self) -> u64 {
        Duration::from_millis(self.effective_limit_ms)
            .checked_sub(self.started_at.elapsed())
            .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
            .unwrap_or(0)
    }

    fn mark_phase(&self, phase: DeadlinePhase) {
        self.phase.set(phase);
    }

    fn current_phase(&self) -> DeadlinePhase {
        self.phase.get()
    }

    fn observation(
        &self,
        outcome: DeadlineOutcome,
        phase: DeadlinePhase,
        terminal_state: TerminalState,
    ) -> DeadlineObservationV1 {
        DeadlineObservationV1 {
            contract: DEADLINE_OBSERVATION_V1.into(),
            outcome,
            phase,
            elapsed_ms: self
                .started_at
                .elapsed()
                .as_millis()
                .min(u128::from(self.effective_limit_ms)) as u64,
            configured_limit_ms: self.runtime_configured_ms,
            effective_limit_ms: self.effective_limit_ms,
            transport_configured_ms: self.transport_configured_ms,
            runtime_configured_ms: self.runtime_configured_ms,
            provider_configured_ms: self.provider_configured_ms,
            finalization_reserve_ms: self.finalization_reserve_ms,
            terminal_state,
            warnings: self.warnings.clone(),
        }
    }
}

struct TransactionGuard {
    transaction_dir: PathBuf,
    claim_path: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RunRecoveryClaim {
    contract: String,
    execution_id: String,
    transaction_leaf: String,
    created_unix_seconds: u64,
    owner_uid: u32,
    process_id: u32,
    transaction_dev: u64,
    transaction_ino: u64,
}

fn serialize_recovery_claim(claim: &RunRecoveryClaim) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(claim)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_RECOVERY_CLAIM_BYTES {
        return Err(run_failure(
            RunFailureKind::RunnerFailed,
            "output-claim-failed",
        ));
    }
    Ok(bytes)
}

impl Drop for TransactionGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.transaction_dir);
        let _ = fs::remove_file(&self.claim_path);
    }
}

#[derive(Clone)]
pub(crate) struct StagedInput {
    pub(crate) logical_name: String,
    pub(crate) authority: ArtifactAuthority,
    pub(crate) source_path: PathBuf,
    pub(crate) staged_path: PathBuf,
    pub(crate) initial_sha256: String,
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
    deadline_at_ms: u64,
    max_output_tokens: u64,
}

pub(crate) struct PreparedNativeRequest {
    pub(crate) step: CompiledModelStepV1,
    pub(crate) invocation_value: Value,
    pub(crate) invocation_bytes: Vec<u8>,
    pub(crate) invocation_sha256: String,
    pub(crate) visible_input: String,
    pub(crate) canonical_output_schema: Value,
    pub(crate) canonical_output_schema_sha256: String,
    pub(crate) provider_output_schema: Value,
    pub(crate) provider_output_schema_sha256: String,
    pub(crate) schema_name: String,
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
    deadline: &RunDeadline,
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
        deadline_at_ms: deadline.provider_deadline_at_ms(),
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

/// The single runtime identity material authority shared by preparation and
/// execution. Callers may derive declarations, but never substitute values.
fn runtime_identity_material() -> Result<(String, String)> {
    let source_sha256 = sha256_hex(BUNDLED_NATIVE_DRIVER_SOURCE.as_bytes());
    let node = resolve_node_executable()?;
    let node_sha256 = sha256_hex(&read_bounded(&node, 200 * 1024 * 1024, "node executable")?);
    Ok((source_sha256, node_sha256))
}

/// Hash the exact identity declaration carried by the closed request.  This
/// is deliberately separate from the projection hash: the declaration is
/// caller/request material, while the projection is the runtime observation.
pub(crate) fn canonical_driver_declaration_hash(
    driver: &crate::run_contracts::DriverIdentity,
) -> Result<String> {
    canonical_json_sha256_for_domain("mdp.driver-declaration.v1", &serde_json::to_value(driver)?)
}

pub(crate) fn canonical_model_declaration_hash(
    model: &crate::run_contracts::ModelIdentity,
) -> Result<String> {
    canonical_json_sha256_for_domain("mdp.model-declaration.v1", &serde_json::to_value(model)?)
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
    let (source_sha256, node_sha256) = runtime_identity_material()?;
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
        driver_declaration_sha256: canonical_driver_declaration_hash(declared_driver)?,
        driver_observed_sha256,
        driver_projection,
        driver_facts,
        model_declaration_sha256: canonical_model_declaration_hash(declared_model)?,
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

/// Prepare the exact native payload used by execution without creating a run
/// directory. This is intentionally crate-private: the compiler is the only
/// caller outside the execution transaction, and both paths share the same
/// model-step, routed-context, output-schema, and request-size gates.
pub(crate) fn compiler_prepare_native_request(
    request: &RunRequestV1,
    manifest: &crate::models::Manifest,
    pack_root: &Path,
    prompt_path: PathBuf,
    prompt_authority: ArtifactAuthority,
    inputs: Vec<(String, ArtifactAuthority, PathBuf)>,
) -> Result<PreparedNativeRequest> {
    let staged_prompt = StagedInput {
        logical_name: "prompt".into(),
        authority: prompt_authority,
        source_path: prompt_path.clone(),
        staged_path: prompt_path,
        initial_sha256: String::new(),
    };
    let staged_inputs = inputs
        .into_iter()
        .map(|(logical_name, authority, path)| StagedInput {
            logical_name,
            authority,
            source_path: path.clone(),
            staged_path: path,
            initial_sha256: String::new(),
        })
        .collect::<Vec<_>>();
    // Compilation only uses the prepared payload for bounded-size and
    // identity derivation checks; execution creates and owns the real
    // deadline that is passed to the native driver.
    let deadline = RunDeadline::try_new(request.execution_policy.timeout_ms, None)?;
    prepare_native_request(
        request,
        manifest,
        pack_root,
        &staged_prompt,
        &staged_inputs,
        &deadline,
    )
}

/// Derive runtime-observed driver/model identities for preparation. Unlike
/// `bind_native_identity`, this helper does not compare caller declarations;
/// it computes the declarations that the compiler will place into the closed
/// v1 request. Runtime still re-observes and compares them before execution.
pub(crate) fn compiler_observe_native_identity(
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
        .ok_or_else(|| anyhow!("driver identity is required"))?;
    let declared_model = request
        .model
        .as_ref()
        .ok_or_else(|| anyhow!("model identity is required"))?;
    let (source_sha256, node_sha256) = runtime_identity_material()?;
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
    let mut observed_driver = declared_driver.clone();
    observed_driver.executable_sha256 = Some(driver_projection.bundled_source_sha256.clone());
    observed_driver.dependency_lock_sha256 = Some(driver_projection.node_executable_sha256.clone());
    observed_driver.version = MDP_RUNTIME_VERSION.into();
    observed_driver.configuration_sha256 = driver_observed_sha256.clone();
    observed_driver.identity_provenance = EvidenceProvenance::MdpObserved;
    let mut observed_model = declared_model.clone();
    observed_model.parameters_sha256 = model_observed_sha256.clone();
    let identity_observations = IdentityObservationV1 {
        driver_declaration_sha256: canonical_driver_declaration_hash(&observed_driver)?,
        driver_observed_sha256: driver_observed_sha256.clone(),
        driver_projection,
        driver_facts,
        model_declaration_sha256: canonical_model_declaration_hash(&observed_model)?,
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
    provider_deadline: Instant,
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
        deadline_at_ms: request.provider_policy.deadline_at_ms,
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
    let (status, stdout_bytes) =
        supervise_child(&mut child, provider_deadline, driver_stdout_limit)?;
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
    deadline: Instant,
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
    execute_run_with_transport(request, output_root, None)
}

pub(crate) fn execute_run_with_transport(
    request: &RunRequestV1,
    output_root: &Path,
    transport_timeout_ms: Option<u64>,
) -> Result<RunExecution> {
    execute_run_inner_with_transport(request, output_root, || Ok(()), transport_timeout_ms)
}

/// Read-only, path-free deadline preflight. It intentionally does not inspect
/// the pack or declared inputs; normal run preflight remains authoritative for
/// those boundaries.
pub(crate) fn deadline_preflight(
    request: &RunRequestV1,
    transport_timeout_ms: Option<u64>,
) -> Result<Value> {
    validate_request(request)
        .map_err(|_| run_failure(RunFailureKind::Preflight, "request-policy-invalid"))?;
    let deadline = RunDeadline::try_new(request.execution_policy.timeout_ms, transport_timeout_ms)?;
    Ok(json!({
        "contract": "mdp.run-preflight.v1",
        "execution_id": request.execution_id,
        "mode": request.mode,
        "recommended_timeout_ms": RECOMMENDED_TIMEOUT_MS,
        "runtime_configured_ms": request.execution_policy.timeout_ms,
        "transport_configured_ms": transport_timeout_ms,
        "provider_configured_ms": RECOMMENDED_TIMEOUT_MS,
        "finalization_reserve_ms": MAX_FINALIZATION_RESERVE_MS,
        "effective_limit_ms": deadline.effective_limit_ms,
        "warnings": deadline.warnings,
        "staging": "not-started",
        "provider": "not-started"
    }))
}

fn execute_run_inner<F>(
    request: &RunRequestV1,
    output_root: &Path,
    before_post_check: F,
) -> Result<RunExecution>
where
    F: FnOnce() -> Result<()>,
{
    execute_run_inner_with_transport(request, output_root, before_post_check, None)
}

fn execute_run_inner_with_transport<F>(
    request: &RunRequestV1,
    output_root: &Path,
    before_post_check: F,
    transport_timeout_ms: Option<u64>,
) -> Result<RunExecution>
where
    F: FnOnce() -> Result<()>,
{
    execute_run_inner_with_driver_and_transport(
        request,
        output_root,
        before_post_check,
        invoke_native_driver,
        transport_timeout_ms,
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
    execute_run_inner_with_driver_and_transport(
        request,
        output_root,
        before_post_check,
        move |request, identity, _deadline| driver(request, identity),
        None,
    )
}

fn execute_run_inner_with_driver_and_transport<F, D>(
    request: &RunRequestV1,
    output_root: &Path,
    before_post_check: F,
    driver: D,
    transport_timeout_ms: Option<u64>,
) -> Result<RunExecution>
where
    F: FnOnce() -> Result<()>,
    D: FnOnce(
        &DriverRequestV2,
        &crate::run_contracts::DriverIdentity,
        Instant,
    ) -> Result<DriverResultV2>,
{
    validate_request(request)
        .map_err(|_| run_failure(RunFailureKind::Preflight, "request-policy-invalid"))?;
    let deadline = RunDeadline::try_new(request.execution_policy.timeout_ms, transport_timeout_ms)?;
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
    let mut claim_options = OpenOptions::new();
    claim_options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        claim_options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut claim = claim_options
        .open(&claim_path)
        .map_err(|_| run_failure(RunFailureKind::Preflight, "output-directory-claimed"))?;
    let transaction_guard = TransactionGuard {
        transaction_dir: transaction_dir.clone(),
        claim_path: claim_path.clone(),
    };
    fs::create_dir(&transaction_dir).with_context(|| {
        format!(
            "creating transaction directory {}",
            transaction_dir.display()
        )
    })?;
    set_private_directory(&transaction_dir)?;
    let transaction_metadata = fs::symlink_metadata(&transaction_dir)?;
    let (owner_uid, transaction_dev, transaction_ino) = recovery_identity(&transaction_metadata);
    let claim_value = RunRecoveryClaim {
        contract: "mdp.run-recovery-claim.v2".into(),
        execution_id: request.execution_id.clone(),
        transaction_leaf: transaction_leaf.clone(),
        created_unix_seconds: unix_seconds_now(),
        owner_uid,
        process_id: std::process::id(),
        transaction_dev,
        transaction_ino,
    };
    let claim_bytes = match serialize_recovery_claim(&claim_value) {
        Ok(bytes) => bytes,
        Err(error) => {
            drop(claim);
            return Err(error);
        }
    };
    if claim
        .write_all(&claim_bytes)
        .and_then(|_| claim.sync_all())
        .is_err()
    {
        drop(claim);
        return Err(run_failure(
            RunFailureKind::RunnerFailed,
            "output-claim-failed",
        ));
    }
    drop(claim);
    deadline.check_phase(DeadlinePhase::Staging)?;

    let TransactionOutcome {
        bundle_sha256,
        receipt,
        diagnostics,
        deadline: transaction_deadline,
    } = match execute_transaction(
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
        && let Err(error) = deadline.check_phase(DeadlinePhase::Finalization)
    {
        cleanup_failed_transaction(&transaction_dir)?;
        return Err(error);
    }
    if request.mode == RunMode::Generative
        && let Err(error) = deadline.check_phase(DeadlinePhase::Finalization)
    {
        let _ = cleanup_failed_transaction(&transaction_dir);
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
    if request.mode == RunMode::Generative
        && let Err(error) = deadline.check_phase(DeadlinePhase::Finalization)
    {
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

    let mut authority_block = json!({
        "contract": "mdp.canonical-authority-block.v1",
        "execution_id": request.execution_id,
        "terminal_state": receipt.terminal_state,
        "decision": receipt.decision,
        "assurance": receipt.assurance,
        "limitations": receipt.limitations,
        "bundle_sha256": bundle_sha256,
        "receipt_sha256": receipt.receipt_sha256,
        "deadline": transaction_deadline,
        "verification": {
            "bundle": output_root.join("run-bundle.json"),
            "receipt": output_root.join("run-receipt.json"),
            "artifact_root": output_root
        },
        "authority_notice": "Only this block and its hash-bound artifacts are authoritative; surrounding conversation commentary is outside the receipt."
    });
    // The bounded rejection reason travels inline so receipt-only consumers
    // can see the code and phase without opening runner-audit.json. Omit both
    // when no governed diagnostic was classified.
    if let Some(code) = receipt.diagnostic_code.as_deref() {
        authority_block["diagnostic_code"] = json!(code);
    }
    if let Some(phase) = receipt.diagnostic_phase.as_deref() {
        authority_block["diagnostic_phase"] = json!(phase);
    }
    if let Some(detail) = receipt.diagnostic_detail.as_ref() {
        authority_block["diagnostic_detail"] = serde_json::to_value(detail)?;
    }
    if !diagnostics.is_empty() {
        authority_block["diagnostics"] = serde_json::to_value(&diagnostics)?;
    }
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
        diagnostic_code: receipt.diagnostic_code.clone(),
        diagnostic_phase: receipt.diagnostic_phase.clone(),
        diagnostic_detail: receipt.diagnostic_detail.clone(),
    })
}

fn classify_execution_error(error: anyhow::Error) -> anyhow::Error {
    if error.downcast_ref::<RunFailure>().is_some() {
        error
    } else {
        run_failure(RunFailureKind::RunnerFailed, "run-execution-failed")
    }
}

/// Diagnose or explicitly remove one stale run transaction. Recovery is
/// intentionally scoped by the *final* output directory: only MDP's hidden
/// claim and the exact transaction inode bound by that claim can be removed.
/// The final output, pack, and customer-controlled workdirs are never removal
/// candidates.
#[cfg(unix)]
pub(crate) fn recover_run_output(output_root: &Path, apply: bool) -> Result<Value> {
    use std::os::unix::fs::MetadataExt;

    let parent = output_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let leaf = match output_root.file_name().and_then(|name| name.to_str()) {
        Some(leaf) if validate_output_leaf(leaf).is_ok() => leaf,
        _ => return Ok(recovery_refusal("recovery-output-name-invalid")),
    };
    let parent_metadata = match fs::symlink_metadata(parent) {
        Ok(metadata) if metadata.file_type().is_dir() => metadata,
        _ => return Ok(recovery_refusal("recovery-parent-unsafe")),
    };
    if parent_metadata.uid() != unsafe { libc::geteuid() }
        && parent_metadata.mode() & (libc::S_ISVTX as u32) == 0
    {
        return Ok(recovery_refusal("recovery-parent-owner-unsafe"));
    }
    match fs::symlink_metadata(output_root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Ok(recovery_refusal("recovery-destination-present")),
    }

    let claim_path = parent.join(format!(".{leaf}.mdp-run.claim"));
    let (claim, claim_metadata) = match read_recovery_claim(&claim_path) {
        Ok(value) => value,
        Err(code) => return Ok(recovery_refusal(code)),
    };
    let expected_prefix = format!(".{leaf}.tmp-");
    if claim.contract != "mdp.run-recovery-claim.v2"
        || claim.execution_id.is_empty()
        || claim.execution_id.len() > MAX_EXECUTION_ID_BYTES
        || !claim
            .execution_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || !claim.transaction_leaf.starts_with(&expected_prefix)
        || claim.transaction_leaf.len() != expected_prefix.len() + 32
        || !claim.transaction_leaf[expected_prefix.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || claim.transaction_leaf.contains(['/', '\\'])
    {
        return Ok(recovery_refusal("recovery-claim-metadata-invalid"));
    }
    let current_uid = unsafe { libc::geteuid() };
    if claim.owner_uid != current_uid
        || claim_metadata.uid() != current_uid
        || claim_metadata.mode() & 0o777 != 0o600
        || claim_metadata.nlink() != 1
    {
        return Ok(recovery_refusal("recovery-claim-authority-unsafe"));
    }
    let transaction_path = parent.join(&claim.transaction_leaf);
    let transaction_metadata = match fs::symlink_metadata(&transaction_path) {
        Ok(metadata) if metadata.file_type().is_dir() => metadata,
        _ => return Ok(recovery_refusal("recovery-transaction-type-unsafe")),
    };
    if transaction_metadata.uid() != current_uid
        || transaction_metadata.mode() & 0o777 != 0o700
        || transaction_metadata.dev() != claim.transaction_dev
        || transaction_metadata.ino() != claim.transaction_ino
    {
        return Ok(recovery_refusal("recovery-transaction-authority-unsafe"));
    }
    let now = unix_seconds_now();
    let logical_age = match now.checked_sub(claim.created_unix_seconds) {
        Some(age) => age,
        None => return Ok(recovery_refusal("recovery-claim-age-invalid")),
    };
    let claim_age = match metadata_age_seconds(&claim_metadata, now) {
        Some(age) => age,
        None => return Ok(recovery_refusal("recovery-claim-age-invalid")),
    };
    let transaction_age = match metadata_age_seconds(&transaction_metadata, now) {
        Some(age) => age,
        None => return Ok(recovery_refusal("recovery-transaction-age-invalid")),
    };
    if logical_age < MIN_RECOVERY_AGE_SECONDS
        || claim_age < MIN_RECOVERY_AGE_SECONDS
        || transaction_age < MIN_RECOVERY_AGE_SECONDS
    {
        return Ok(recovery_refusal("recovery-claim-recent"));
    }
    if process_is_live(claim.process_id) != Some(false) {
        return Ok(recovery_refusal("recovery-process-live-or-unknown"));
    }

    let would_remove = json!([
        {"kind": "transaction-directory", "path": transaction_path},
        {"kind": "claim-file", "path": claim_path}
    ]);
    if !apply {
        return Ok(json!({
            "contract": "mdp.run-recovery.v1",
            "valid": true,
            "status": "ready",
            "applied": false,
            "stale_after_seconds": MIN_RECOVERY_AGE_SECONDS,
            "claim_age_seconds": claim_age,
            "transaction_age_seconds": transaction_age,
            "process_state": "not-running",
            "would_remove": would_remove,
            "removed": [],
            "diagnostics": []
        }));
    }

    // Recheck the destination and both filesystem identities immediately
    // before deletion. remove_dir_all does not follow a symlink at its root;
    // an identity change is nevertheless treated as ambiguity and refused.
    if fs::symlink_metadata(output_root).is_ok() {
        return Ok(recovery_refusal("recovery-destination-present"));
    }
    let transaction_recheck = fs::symlink_metadata(&transaction_path)?;
    if !transaction_recheck.file_type().is_dir()
        || transaction_recheck.dev() != claim.transaction_dev
        || transaction_recheck.ino() != claim.transaction_ino
        || transaction_recheck.uid() != current_uid
        || transaction_recheck.mode() & 0o777 != 0o700
    {
        return Ok(recovery_refusal("recovery-transaction-changed"));
    }
    fs::remove_dir_all(&transaction_path)
        .map_err(|_| run_failure(RunFailureKind::RunnerFailed, "recovery-removal-failed"))?;
    let claim_recheck = fs::symlink_metadata(&claim_path)?;
    if !claim_recheck.file_type().is_file()
        || claim_recheck.dev() != claim_metadata.dev()
        || claim_recheck.ino() != claim_metadata.ino()
        || claim_recheck.uid() != current_uid
        || claim_recheck.mode() & 0o777 != 0o600
        || claim_recheck.nlink() != 1
    {
        return Ok(recovery_refusal("recovery-claim-changed"));
    }
    fs::remove_file(&claim_path)
        .map_err(|_| run_failure(RunFailureKind::RunnerFailed, "recovery-removal-failed"))?;
    Ok(json!({
        "contract": "mdp.run-recovery.v1",
        "valid": true,
        "status": "recovered",
        "applied": true,
        "stale_after_seconds": MIN_RECOVERY_AGE_SECONDS,
        "claim_age_seconds": claim_age,
        "transaction_age_seconds": transaction_age,
        "process_state": "not-running",
        "would_remove": would_remove,
        "removed": [
            {"kind": "transaction-directory", "path": transaction_path},
            {"kind": "claim-file", "path": claim_path}
        ],
        "diagnostics": []
    }))
}

#[cfg(not(unix))]
pub(crate) fn recover_run_output(_output_root: &Path, _apply: bool) -> Result<Value> {
    Ok(recovery_refusal("recovery-platform-unsupported"))
}

fn recovery_refusal(code: &'static str) -> Value {
    json!({
        "contract": "mdp.run-recovery.v1",
        "valid": false,
        "status": "refused",
        "applied": false,
        "would_remove": [],
        "removed": [],
        "diagnostics": [{"code": code}]
    })
}

#[cfg(unix)]
fn read_recovery_claim(
    path: &Path,
) -> std::result::Result<(RunRecoveryClaim, fs::Metadata), &'static str> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| "recovery-claim-unavailable")?;
    let metadata = file.metadata().map_err(|_| "recovery-claim-unavailable")?;
    if !metadata.file_type().is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_RECOVERY_CLAIM_BYTES as u64
        || metadata.nlink() != 1
    {
        return Err("recovery-claim-type-unsafe");
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(|_| "recovery-claim-unreadable")?;
    let claim = serde_json::from_slice(&bytes).map_err(|_| "recovery-claim-metadata-invalid")?;
    Ok((claim, metadata))
}

#[cfg(unix)]
fn metadata_age_seconds(metadata: &fs::Metadata, now: u64) -> Option<u64> {
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    now.checked_sub(modified)
}

#[cfg(unix)]
fn process_is_live(process_id: u32) -> Option<bool> {
    let process_id = i32::try_from(process_id).ok()?;
    let result = unsafe { libc::kill(process_id, 0) };
    if result == 0 {
        Some(true)
    } else {
        match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::ESRCH) => Some(false),
            Some(libc::EPERM) => Some(true),
            _ => None,
        }
    }
}

fn unix_seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(unix)]
fn recovery_identity(metadata: &fs::Metadata) -> (u32, u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (metadata.uid(), metadata.dev(), metadata.ino())
}

#[cfg(not(unix))]
fn recovery_identity(_metadata: &fs::Metadata) -> (u32, u64, u64) {
    (0, 0, 0)
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
) -> Result<TransactionOutcome>
where
    F: FnOnce() -> Result<()>,
    D: FnOnce(
        &DriverRequestV2,
        &crate::run_contracts::DriverIdentity,
        Instant,
    ) -> Result<DriverResultV2>,
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
    deadline.check_phase(DeadlinePhase::Staging)?;
    let source_snapshot = pack_content_snapshot(source_pack)?;
    validate_pack_snapshot_bounds(&source_snapshot)?;
    copy_pack(source_pack, &staged_pack)?;
    deadline.check_phase(DeadlinePhase::Staging)?;
    let staged_snapshot = pack_content_snapshot(&staged_pack)?;
    if source_snapshot != staged_snapshot {
        return Err(run_failure_with_diagnostic(
            RunFailureKind::PolicyBlocked,
            "source-integrity-failed",
            source_integrity_diagnostic("pack"),
        ));
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
    deadline.check_phase(DeadlinePhase::Staging)?;
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
        return Err(run_failure_with_diagnostic(
            RunFailureKind::PolicyBlocked,
            "source-integrity-failed",
            source_integrity_diagnostic("pack"),
        ));
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
                deadline,
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
    let mut diagnostic_phase = None;
    let mut diagnostic_detail = None;
    let (mut terminal_state, mut success_values) = if request.mode == RunMode::Generative {
        let prompt = staged_prompt.as_ref().ok_or_else(|| {
            run_failure(RunFailureKind::PolicyBlocked, "generative-prompt-missing")
        })?;
        let outcome = execute_generative_step_with_deadline(
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
        diagnostic_phase = outcome.diagnostic_phase.clone();
        diagnostic_detail = outcome.diagnostic_detail.clone();
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
        crate::decision_input::select_adapter(&manifest, &["opportunity"])
            .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "job-ingress-invalid"))?;
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
    } else if (request.profile == GTM_PROFILE && request.operation == QUALIFY)
        || (request.profile == PROPOSAL_PROFILE && request.operation == REVIEW)
    {
        let proposal_v3 = request.profile == PROPOSAL_PROFILE;
        crate::decision_input::select_adapter(
            &manifest,
            &[if proposal_v3 {
                "opportunity"
            } else {
                "prospect"
            }],
        )
        .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "job-ingress-invalid"))?;
        let normalized = required_input(&staged, "normalized-decision-input")?;
        if normalized.authority.media_type != "application/json"
            || !matches!(
                normalized.authority.schema_id.as_str(),
                NORMALIZED_DECISION_INPUT_CONTRACT
                    | NORMALIZED_DECISION_INPUT_CONTRACT_V2
                    | NORMALIZED_DECISION_INPUT_CONTRACT_V3
            )
        {
            return Err(anyhow!("declared input schema or media type mismatch"));
        }
        let is_v3 = normalized.authority.schema_id == NORMALIZED_DECISION_INPUT_CONTRACT_V3;
        if proposal_v3 && !is_v3 {
            return Err(run_failure(
                RunFailureKind::PolicyBlocked,
                "proposal-review-requires-v3",
            ));
        }
        let signal_aware = matches!(
            normalized.authority.schema_id.as_str(),
            NORMALIZED_DECISION_INPUT_CONTRACT_V2 | NORMALIZED_DECISION_INPUT_CONTRACT_V3
        );
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
        if is_v3 {
            validate_v3_sealed_envelope(&normalized_value).map_err(|_| {
                run_failure(RunFailureKind::PolicyBlocked, "v3-sealed-envelope-invalid")
            })?;
            if normalized_value["source_binding_sha256"]
                != source_binding.expect("v3 source binding").initial_sha256
                || normalized_value["source_attempt_request_sha256"]
                    != source_attempt.initial_sha256
                || normalized_value["collected_attempt_results_sha256"]
                    != attempt_results.initial_sha256
            {
                return Err(run_failure(
                    RunFailureKind::PolicyBlocked,
                    "v3-lineage-hash-mismatch",
                ));
            }
            let compiled = requirements(&staged_pack, normalized_job_id)?;
            if normalized_value["requirements_sha256"] != compiled["requirements_sha256"]
                || normalized_value["taxonomy_set_sha256"] != compiled["taxonomy_set_sha256"]
            {
                return Err(run_failure(
                    RunFailureKind::PolicyBlocked,
                    "v3-compiled-identity-mismatch",
                ));
            }
            let selected_taxonomies: Vec<ClassificationTaxonomy> = serde_json::from_value(
                compiled["classification_specification"]["taxonomies"].clone(),
            )
            .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "v3-taxonomy-set-invalid"))?;
            let compiled_attribute_ids = compiled["decision_input_contracts"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|contract| contract["attributes"].as_array().into_iter().flatten())
                .filter_map(|attribute| attribute["id"].as_str().map(str::to_owned))
                .collect::<Vec<_>>();
            let collected_value: Value =
                serde_json::from_slice(&fs::read(&attempt_results.staged_path)?)?;
            let observed_attempts = data_object(&collected_value)["attempt_results"]
                .as_array()
                .ok_or_else(|| {
                    run_failure(
                        RunFailureKind::PolicyBlocked,
                        "v3-collected-attempt-results-invalid",
                    )
                })?
                .iter()
                .filter(|attempt| attempt["status"] == "observed")
                .collect::<Vec<_>>();
            let semantic = json!({
                "classifications": normalized_value["classifications"].clone(),
                "gaps": normalized_value["gaps"].clone(),
                "rejected_claims": normalized_value["rejected_claims"].clone(),
            });
            validate_v3_classification_evidence(
                &semantic,
                &selected_taxonomies,
                &compiled_attribute_ids,
                &observed_attempts,
            )?;
            let envelope = normalized_value.as_object().ok_or_else(|| {
                run_failure(RunFailureKind::PolicyBlocked, "v3-sealed-envelope-invalid")
            })?;
            let decision_input =
                crate::decision_input::from_v3_normalized(envelope).map_err(|_| {
                    run_failure(RunFailureKind::PolicyBlocked, "v3-neutral-input-invalid")
                })?;
            let classifications_ready = normalized_value["classifications"]
                .as_object()
                .is_some_and(|values| {
                    !values.is_empty()
                        && values.values().all(|value| value["status"] == "classified")
                });
            if proposal_v3 {
                let (decision, reason_codes) =
                    deterministic_proposal_pursuit(&decision_input, classifications_ready);
                validation = Some(
                    json!({"contract": NORMALIZED_DECISION_INPUT_CONTRACT_V3, "valid": true, "ready": decision == "pursue"}),
                );
                (
                    TerminalState::Success,
                    Some(proposal_v3_success_artifacts(
                        request,
                        &bundle,
                        &bundle_sha256,
                        &normalized_value,
                        decision,
                        reason_codes,
                    )?),
                )
            } else if !classifications_ready {
                validation = Some(
                    json!({"contract": NORMALIZED_DECISION_INPUT_CONTRACT_V3, "valid": true, "ready": false}),
                );
                (TerminalState::NoDraftOutputInvalid, None)
            } else {
                validation = Some(
                    json!({"contract": NORMALIZED_DECISION_INPUT_CONTRACT_V3, "valid": true, "ready": true}),
                );
                let prospect = decision_input.to_gtm_prospect().map_err(|_| {
                    run_failure(RunFailureKind::PolicyBlocked, "v3-gtm-projection-invalid")
                })?;
                let fit_result = fit_prospect_with_governed_authority(
                    &staged_pack,
                    prospect,
                    normalized_job_id,
                    json!({
                        "normalized_output_sha256": normalized.initial_sha256,
                        "requirements_sha256": normalized_value["requirements_sha256"],
                        "taxonomy_set_sha256": normalized_value["taxonomy_set_sha256"],
                        "source_binding_sha256": source_binding.expect("v3 source binding").initial_sha256,
                        "source_attempt_request_sha256": source_attempt.initial_sha256,
                        "collected_attempt_results_sha256": attempt_results.initial_sha256
                    }),
                )?;
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
        }
    } else {
        (TerminalState::NoDraftPolicyBlocked, None)
    };
    if request.mode == RunMode::Deterministic {
        if deadline.check_phase(DeadlinePhase::Validation).is_err() {
            terminal_state = TerminalState::NoDraftRunnerFailed;
            success_values = None;
            validation = None;
        }
    } else if deadline.expired() {
        let phase = deadline.current_phase();
        deadline.record_terminal(deadline.observation(
            DeadlineOutcome::TimedOut,
            phase,
            TerminalState::NoDraftRunnerFailed,
        ));
        terminal_state = TerminalState::NoDraftRunnerFailed;
        success_values = None;
        validation = None;
        // The timeout replaces any model-classified rejection, so the
        // published reason must name the timeout phase instead of the stale
        // model code carried by the generative outcome.
        diagnostic_code = Some(format!("{}-timeout", deadline_phase_label(phase)));
        diagnostic_phase = Some(deadline_phase_label(phase).into());
        diagnostic_detail = None;
    }

    deadline.mark_phase(DeadlinePhase::Finalization);
    before_post_check()?;
    if request.mode == RunMode::Deterministic {
        if deadline.check_phase(DeadlinePhase::Finalization).is_err() {
            terminal_state = TerminalState::NoDraftRunnerFailed;
            success_values = None;
            validation = None;
        }
    } else if deadline.expired() {
        deadline.record_terminal(deadline.observation(
            DeadlineOutcome::TimedOut,
            DeadlinePhase::Finalization,
            TerminalState::NoDraftRunnerFailed,
        ));
        terminal_state = TerminalState::NoDraftRunnerFailed;
        success_values = None;
        validation = None;
        diagnostic_code = Some("finalization-timeout".into());
        diagnostic_phase = Some(deadline_phase_label(DeadlinePhase::Finalization).into());
        diagnostic_detail = None;
    }
    if !deadline.expired() {
        deadline.mark_phase(DeadlinePhase::Cleanup);
    }
    let staged_pack_after = pack_content_snapshot(&staged_pack)?;
    let source_pack_after = pack_content_snapshot(source_pack)?;
    let mut diagnostics = Vec::new();
    let staged_sources_unchanged = match check_sources_unchanged(&staged) {
        Ok(()) => true,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            false
        }
    };
    let prompt_unchanged =
        staged_prompt.as_ref().is_none_or(|prompt| {
            match check_sources_unchanged(std::slice::from_ref(prompt)) {
                Ok(()) => true,
                Err(diagnostic) => {
                    if diagnostics.is_empty() {
                        diagnostics.push(diagnostic);
                    }
                    false
                }
            }
        });
    let sources_unchanged = staged_sources_unchanged && prompt_unchanged;
    let pack_unchanged =
        staged_pack_after == staged_snapshot && source_pack_after == source_snapshot;
    if !pack_unchanged || !sources_unchanged {
        if diagnostics.is_empty() {
            diagnostics.push(source_integrity_diagnostic("pack"));
        }
        terminal_state = TerminalState::NoDraftAuditIncomplete;
        success_values = None;
        // The schema-valid source-integrity diagnostics in the authority
        // block carry the rejection reason. Publishing the prior
        // model-rejection code under an audit-incomplete terminal state
        // would mislabel the cause, so no scalar code or phase survives the
        // override.
        diagnostic_code = None;
        diagnostic_phase = None;
        diagnostic_detail = None;
    }

    if !deadline.expired() {
        deadline.mark_phase(DeadlinePhase::Validation);
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

    if request.mode == RunMode::Generative
        && terminal_state.is_success()
        && deadline.check_phase(DeadlinePhase::Finalization).is_err()
    {
        terminal_state = TerminalState::NoDraftRunnerFailed;
        success_values = None;
        validation = None;
    }
    let deadline_observation = deadline.terminal_observation();
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
        deadline: deadline_observation.clone(),
        diagnostic_code,
        diagnostic_phase,
        diagnostic_detail,
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
        deadline: deadline_observation.clone(),
        diagnostic_code: audit.diagnostic_code.clone(),
        diagnostic_phase: audit.diagnostic_phase.clone(),
        diagnostic_detail: audit.diagnostic_detail.clone(),
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
    Ok(TransactionOutcome {
        bundle_sha256,
        receipt,
        diagnostics,
        deadline: deadline_observation,
    })
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
    diagnostic_phase: Option<String>,
    diagnostic_detail: Option<DiagnosticDetailV1>,
    driver_request_sha256: String,
    driver_result_sha256: String,
}

/// Bounded phase labels reuse the existing `DeadlinePhase` vocabulary; no new
/// phase names are invented for diagnostics.
fn deadline_phase_label(phase: DeadlinePhase) -> &'static str {
    match phase {
        DeadlinePhase::Preflight => "preflight",
        DeadlinePhase::Staging => "staging",
        DeadlinePhase::Driver => "driver",
        DeadlinePhase::Provider => "provider",
        DeadlinePhase::Validation => "validation",
        DeadlinePhase::Finalization => "finalization",
        DeadlinePhase::Cancellation => "cancellation",
        DeadlinePhase::Transport => "transport",
        DeadlinePhase::Cleanup => "cleanup",
    }
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
    execute_generative_step_with_deadline(
        request,
        staged_pack,
        staged_prompt,
        staged_inputs,
        private_dir,
        bundle,
        bundle_sha256,
        prepared,
        driver_identity,
        deadline,
        move |request, identity, _deadline| driver(request, identity),
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_generative_step_with_deadline<D>(
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
    D: FnOnce(
        &DriverRequestV2,
        &crate::run_contracts::DriverIdentity,
        Instant,
    ) -> Result<DriverResultV2>,
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
            deadline_at_ms: deadline.provider_deadline_at_ms(),
            max_output_bytes: request.execution_policy.max_output_bytes,
        },
        execution_policy_sha256: policy_hash,
        request_sha256: String::new(),
    };
    seal_driver_request(&mut driver_request)?;

    if driver_timeout_ms.is_none() {
        return failed_generative_outcome(driver_request, "driver_budget_exhausted", "driver");
    }
    deadline.mark_phase(DeadlinePhase::Driver);
    let result = match driver(
        &driver_request,
        driver_identity,
        deadline.provider_deadline(),
    ) {
        Ok(result) => result,
        Err(error) => {
            let code = error
                .downcast_ref::<RunFailure>()
                .map(RunFailure::code)
                .unwrap_or("driver_invocation_failed");
            if matches!(
                code,
                "cancelled" | "driver-cancelled" | "provider-cancelled"
            ) {
                deadline.record_cancelled(DeadlinePhase::Cancellation);
                return failed_generative_outcome(driver_request, "cancelled", "cancellation");
            }
            if matches!(
                code,
                "driver-timeout" | "provider-timeout" | "execution-timeout"
            ) {
                deadline.record_terminal(deadline.observation(
                    DeadlineOutcome::TimedOut,
                    DeadlinePhase::Provider,
                    TerminalState::NoDraftRunnerFailed,
                ));
                return failed_generative_outcome(
                    driver_request,
                    "driver-timeout",
                    deadline_phase_label(DeadlinePhase::Provider),
                );
            }
            return failed_generative_outcome(driver_request, code, "driver");
        }
    };
    if deadline.expired() {
        deadline.record_terminal(deadline.observation(
            DeadlineOutcome::TimedOut,
            DeadlinePhase::Provider,
            TerminalState::NoDraftRunnerFailed,
        ));
        return failed_generative_outcome(
            driver_request,
            "driver-timeout",
            deadline_phase_label(DeadlinePhase::Provider),
        );
    }
    deadline.mark_phase(DeadlinePhase::Validation);
    if validate_driver_result(&driver_request, &result).is_err() {
        return Ok(GenerativeOutcome {
            terminal_state: TerminalState::NoDraftRunnerFailed,
            success: None,
            validation: None,
            provider_request_body_sha256: None,
            provider_request_schema_id: None,
            provider_response_body_sha256: None,
            provider_observation: None,
            diagnostic_code: Some("driver-result-invalid".into()),
            diagnostic_phase: Some("driver".into()),
            diagnostic_detail: None,
            driver_request_sha256: driver_request.request_sha256,
            driver_result_sha256: result.result_sha256,
        });
    }
    if !result.terminal_state.is_success() {
        if result.terminal_state == TerminalState::NoDraftPolicyBlocked {
            // A driver policy refusal is still a CLI policy block. Route it
            // through the typed failure carrier so execute_transaction cleans
            // up the private transaction and the public CLI result remains a
            // receipt-free, diagnostic-bearing no-draft response.
            return Err(run_failure(
                RunFailureKind::PolicyBlocked,
                "driver-policy-blocked",
            ));
        }
        return Ok(GenerativeOutcome {
            terminal_state: result.terminal_state,
            success: None,
            validation: None,
            provider_request_body_sha256: result.provider_request_body_sha256,
            provider_request_schema_id: result.provider_request_schema_id,
            provider_response_body_sha256: result.provider_response_body_sha256,
            provider_observation: result.provider_observation,
            diagnostic_code: result.diagnostic_code.clone(),
            diagnostic_phase: Some("driver".into()),
            diagnostic_detail: None,
            driver_request_sha256: driver_request.request_sha256,
            driver_result_sha256: result.result_sha256,
        });
    }
    let output = result.output.as_ref().expect("validated success output");
    let output_path = private_dir.join("driver-output.json");
    let output_bytes = match (
        prepared.step.output_contract.host_envelope.as_ref(),
        prepared.step.output_contract.output_kind.as_deref(),
    ) {
        (Some(_), Some(crate::constants::OUTPUT_KIND_DECISION_INPUT_NORMALIZATION)) => {
            match host_wrap_v3_normalization_output(
                &prepared.step,
                staged_inputs,
                &prepared.invocation_value,
                &prepared.invocation_bytes,
                &output.content_utf8,
            ) {
                Ok(bytes) => bytes,
                Err(error) => {
                    let diagnostic_detail = error
                        .downcast_ref::<RunFailure>()
                        .and_then(RunFailure::diagnostic_detail)
                        .cloned();
                    return Ok(host_envelope_failure_outcome(
                        &driver_request,
                        result,
                        sanitized_host_envelope_diagnostic(&error),
                        diagnostic_detail,
                    ));
                }
            }
        }
        (Some(_), _) => {
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
                    let diagnostic_detail = error
                        .downcast_ref::<RunFailure>()
                        .and_then(RunFailure::diagnostic_detail)
                        .cloned();
                    return Ok(host_envelope_failure_outcome(
                        &driver_request,
                        result,
                        sanitized_host_envelope_diagnostic(&error),
                        diagnostic_detail,
                    ));
                }
            }
        }
        (None, _) => output.content_utf8.as_bytes().to_vec(),
    };
    write_bytes_create_new(&output_path, &output_bytes)?;
    let routed_context = optional_input(staged_inputs, "routed_context")
        .or_else(|| optional_input(staged_inputs, "routed-context"));
    // The validator binds normalization output to the prompt declared by the
    // compiled job. The private staged copy is the byte-frozen driver input,
    // but it intentionally lives at a different filesystem path. Validate
    // against the canonical prompt inside the staged pack after
    // `validate_selected_prompt` has proven both copies are byte-identical.
    let bound_prompt_path = canonical_selected_prompt_path(staged_pack, &prepared.step);
    let validation = validate_prompt_output_file_with_lineage_inputs(
        staged_pack,
        &output_path,
        Some(&bound_prompt_path),
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
    if deadline.expired() {
        deadline.record_terminal(deadline.observation(
            DeadlineOutcome::TimedOut,
            deadline.current_phase(),
            TerminalState::NoDraftRunnerFailed,
        ));
        return failed_generative_outcome(
            driver_request,
            "validation-timeout",
            deadline_phase_label(deadline.current_phase()),
        );
    }
    let valid = validation["valid"].as_bool() == Some(true);
    let validation_diagnostic =
        (!valid).then(|| sanitized_prompt_validation_diagnostic(&validation));
    let diagnostic_phase = validation_diagnostic
        .is_some()
        .then(|| "validation".to_string());
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
        diagnostic_code: validation_diagnostic,
        diagnostic_phase,
        diagnostic_detail: None,
        driver_request_sha256: driver_request.request_sha256,
        driver_result_sha256: result.result_sha256,
    })
}

fn failed_generative_outcome(
    driver_request: DriverRequestV2,
    diagnostic_code: &str,
    diagnostic_phase: &'static str,
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
        diagnostic_phase: Some(diagnostic_phase.into()),
        diagnostic_detail: None,
        driver_request_sha256: driver_request.request_sha256,
        driver_result_sha256: failed_result.result_sha256,
    })
}

fn host_envelope_failure_outcome(
    driver_request: &DriverRequestV2,
    result: DriverResultV2,
    diagnostic_code: &'static str,
    diagnostic_detail: Option<DiagnosticDetailV1>,
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
        diagnostic_phase: Some("validation".into()),
        diagnostic_detail,
        driver_request_sha256: driver_request.request_sha256.clone(),
        driver_result_sha256: result.result_sha256,
    }
}

fn sanitized_host_envelope_diagnostic(error: &anyhow::Error) -> &'static str {
    let code = error
        .downcast_ref::<RunFailure>()
        .map(RunFailure::code)
        .unwrap_or("host-envelope-failed");
    // v3 normalization failures are already bounded, static policy codes.
    // Preserve them so receipt-only runs remain diagnosable without retaining
    // provider output, evidence prose, or schema-validation error strings.
    if code.starts_with("v3-") {
        return code;
    }
    match code {
        "host-envelope-metadata-missing"
        | "host-envelope-metadata-invalid"
        | "normalization-host-envelope-metadata-missing"
        | "normalization-host-envelope-metadata-invalid"
        | "semantic-output-malformed"
        | "semantic-output-not-object"
        | "host-owned-field-injection"
        | "semantic-output-invalid"
        | "host-context-source-missing"
        | "semantic-output-missing" => code,
        _ => "host-envelope-failed",
    }
}

fn sanitized_prompt_validation_diagnostic(validation: &Value) -> String {
    let code = validation["issues"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|issue| issue["code"].as_str())
        .find(|code| {
            code.len() <= 96
                && code.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
                })
                && [
                    "decision_input_",
                    "v3_",
                    "prompt_output_",
                    "source_",
                    "collected_",
                ]
                .iter()
                .any(|prefix| code.starts_with(prefix))
        });
    code.map(|code| code.replace('_', "-"))
        .unwrap_or_else(|| "prompt-output-validation-failed".into())
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
    let canonical_path = canonical_selected_prompt_path(staged_pack, step);
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

fn canonical_selected_prompt_path(staged_pack: &Path, step: &CompiledModelStepV1) -> PathBuf {
    staged_pack.join(".mdp").join(&step.prompt_path)
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
        NORMALIZED_DECISION_INPUT_CONTRACT
            | NORMALIZED_DECISION_INPUT_CONTRACT_V2
            | NORMALIZED_DECISION_INPUT_CONTRACT_V3
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

fn routed_context_field_pointer(name: &str) -> &'static str {
    match name {
        "contract" => "/contract",
        "job" => "/job",
        "persona" => "/persona",
        "scope" => "/scope",
        "product_foundation" => "/product_foundation",
        "product_foundation_load_order" => "/product_foundation_load_order",
        "entries" => "/entries",
        "gaps" => "/gaps",
        "policy" => "/policy",
        _ => "/unknown-field",
    }
}

fn safe_contract_value(value: Option<&Value>) -> &'static str {
    match value.and_then(Value::as_str) {
        Some(ROUTED_CONTEXT_CONTRACT) => ROUTED_CONTEXT_CONTRACT,
        Some("mdp.routed-context.v0") => "mdp.routed-context.v0",
        Some(_) => "other",
        None => "missing",
    }
}

fn routed_context_shape_diagnostic(value: &Value) -> Option<RunDiagnostic> {
    let object = value.as_object()?;
    let expected_fields = [
        "contract",
        "job",
        "persona",
        "scope",
        "product_foundation",
        "product_foundation_load_order",
        "entries",
        "gaps",
        "policy",
    ];
    if let Some(field) = object
        .keys()
        .find(|field| !expected_fields.contains(&field.as_str()))
    {
        return Some(policy_diagnostic(
            "generative-preflight",
            "routed-context-schema",
            "disallowed-field",
            Some("routed_context"),
            Some(routed_context_field_pointer(field)),
            diagnostic_value("field", "declared"),
            diagnostic_value("field", "unknown-field"),
        ));
    }
    if object.get("contract") != Some(&json!(ROUTED_CONTEXT_CONTRACT)) {
        return Some(policy_diagnostic(
            "generative-preflight",
            "routed-context-schema",
            "wrong-contract",
            Some("routed_context"),
            Some("/contract"),
            diagnostic_value("contract", ROUTED_CONTEXT_CONTRACT),
            diagnostic_value("contract", safe_contract_value(object.get("contract"))),
        ));
    }
    for field in [
        "job",
        "persona",
        "scope",
        "product_foundation",
        "product_foundation_load_order",
        "entries",
        "gaps",
        "policy",
    ] {
        if !object.contains_key(field) {
            return Some(policy_diagnostic(
                "generative-preflight",
                "routed-context-schema",
                "missing-required-field",
                Some("routed_context"),
                Some(routed_context_field_pointer(field)),
                diagnostic_value("field", "present"),
                diagnostic_value("field", "missing"),
            ));
        }
    }
    None
}

fn routed_context_validation_diagnostic(
    kind: crate::routing::RoutedContextValidationKind,
) -> RunDiagnostic {
    use crate::routing::RoutedContextValidationKind;
    match kind {
        RoutedContextValidationKind::Contract => policy_diagnostic(
            "generative-preflight",
            "routed-context-schema",
            "wrong-contract",
            Some("routed_context"),
            Some("/contract"),
            diagnostic_value("contract", ROUTED_CONTEXT_CONTRACT),
            diagnostic_value("contract", "other"),
        ),
        RoutedContextValidationKind::Job => policy_diagnostic(
            "generative-preflight",
            "routed-context-readiness",
            "stale-binding",
            Some("routed_context"),
            Some("/job"),
            diagnostic_value("binding", "matched"),
            diagnostic_value("binding", "mismatch"),
        ),
        RoutedContextValidationKind::Scope => policy_diagnostic(
            "generative-preflight",
            "routed-context-readiness",
            "stale-binding",
            Some("routed_context"),
            Some("/scope"),
            diagnostic_value("binding", "matched"),
            diagnostic_value("binding", "mismatch"),
        ),
        RoutedContextValidationKind::Canonical => policy_diagnostic(
            "generative-preflight",
            "routed-context-readiness",
            "stale-binding",
            Some("routed_context"),
            None,
            diagnostic_value("binding", "canonical"),
            diagnostic_value("binding", "changed"),
        ),
        RoutedContextValidationKind::ReadinessBlocked => policy_diagnostic(
            "generative-preflight",
            "routed-context-readiness",
            "readiness-failure",
            Some("routed_context"),
            None,
            diagnostic_value("readiness", "ready"),
            diagnostic_value("readiness", "blocked"),
        ),
        RoutedContextValidationKind::Schema | RoutedContextValidationKind::NotCompiled => {
            policy_diagnostic(
                "generative-preflight",
                "routed-context-schema",
                "internal-contract-mismatch",
                Some("routed_context"),
                None,
                diagnostic_value("contract", ROUTED_CONTEXT_CONTRACT),
                diagnostic_value("contract", "unavailable"),
            )
        }
    }
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
        if input.authority.schema_id != ROUTED_CONTEXT_CONTRACT
            || input.authority.media_type != "application/json"
        {
            return Err(run_failure_with_diagnostic(
                RunFailureKind::PolicyBlocked,
                "routed-context-invalid",
                policy_diagnostic(
                    "generative-preflight",
                    "routed-context-schema",
                    "wrong-contract",
                    Some("routed_context"),
                    Some("/contract"),
                    diagnostic_value("contract", ROUTED_CONTEXT_CONTRACT),
                    diagnostic_value("contract", "declared-input-mismatch"),
                ),
            ));
        }
        let bytes = fs::read(&input.staged_path)
            .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "routed-context-invalid"))?;
        let value = serde_json::from_slice::<Value>(&bytes).map_err(|_| {
            run_failure_with_diagnostic(
                RunFailureKind::PolicyBlocked,
                "routed-context-invalid",
                policy_diagnostic(
                    "generative-preflight",
                    "routed-context-schema",
                    "malformed-json",
                    Some("routed_context"),
                    None,
                    diagnostic_value("json-type", "object"),
                    diagnostic_value("json-type", "malformed"),
                ),
            )
        })?;
        if let Some(diagnostic) = routed_context_shape_diagnostic(&value) {
            return Err(run_failure_with_diagnostic(
                RunFailureKind::PolicyBlocked,
                "routed-context-invalid",
                diagnostic,
            ));
        }
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
            run_failure_with_diagnostic(
                RunFailureKind::PolicyBlocked,
                code,
                routed_context_validation_diagnostic(error.kind()),
            )
        })?;
        if validation.sha256 != input.authority.sha256 {
            return Err(run_failure_with_diagnostic(
                RunFailureKind::PolicyBlocked,
                "routed-context-invalid",
                policy_diagnostic(
                    "generative-preflight",
                    "routed-context-readiness",
                    "stale-binding",
                    Some("routed_context"),
                    None,
                    diagnostic_value("binding", "matched"),
                    diagnostic_value("binding", "changed"),
                ),
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
    if let Some(input) = staged.iter().find(|input| {
        is_host_invocation_metadata(&input.logical_name)
            || !declared.contains(input.logical_name.as_str())
    }) {
        return Err(run_failure_with_diagnostic(
            RunFailureKind::PolicyBlocked,
            "undeclared-model-input",
            policy_diagnostic(
                "generative-preflight",
                "declared-inputs",
                "disallowed-field",
                safe_logical_input_name(&input.logical_name),
                Some("/unknown-field"),
                diagnostic_value("field", "declared"),
                diagnostic_value("field", "unknown-field"),
            ),
        ));
    }
    if let Some(input) = step.declared_inputs.iter().find(|input| {
        input.required
            && !is_host_invocation_metadata(&input.name)
            && !staged.iter().any(|item| item.logical_name == input.name)
    }) {
        return Err(run_failure_with_diagnostic(
            RunFailureKind::PolicyBlocked,
            "required-model-input-missing",
            policy_diagnostic(
                "generative-preflight",
                "declared-inputs",
                "missing-required-field",
                safe_logical_input_name(&input.name),
                None,
                diagnostic_value("binding", "declared"),
                diagnostic_value("binding", "missing"),
            ),
        ));
    }
    Ok(())
}

fn safe_logical_input_name(name: &str) -> Option<&'static str> {
    match name {
        "routed_context" | "routed-context" => Some("routed_context"),
        "prompt" => Some("prompt"),
        "prompt_receipt" | "prompt-receipt" => Some("prompt_receipt"),
        "invocation_receipt_sha256" | "invocation-receipt-sha256" => {
            Some("invocation_receipt_sha256")
        }
        _ => None,
    }
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

fn json_schema_type_for_value(value: &Value) -> Option<&'static str> {
    match value {
        Value::Null => Some("null"),
        Value::Bool(_) => Some("boolean"),
        Value::String(_) => Some("string"),
        Value::Array(_) => Some("array"),
        Value::Object(_) => Some("object"),
        Value::Number(number) => {
            if number
                .as_f64()
                .is_some_and(|value| value.is_finite() && value.fract() == 0.0)
            {
                Some("integer")
            } else {
                Some("number")
            }
        }
    }
}

fn infer_enum_type(value: &Value) -> Result<&'static str> {
    let values = value
        .as_array()
        .ok_or_else(|| anyhow!("schema enum must be an array"))?;
    if values.is_empty() {
        return Err(anyhow!("schema enum must be non-empty"));
    }
    let types = values
        .iter()
        .map(json_schema_type_for_value)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| anyhow!("enum schema values must have provider-compatible types"))?;
    if types.iter().all(|schema_type| *schema_type == types[0]) {
        return Ok(types[0]);
    }
    if values.iter().all(Value::is_number) {
        return Ok("number");
    }
    Err(anyhow!(
        "enum schema values must share a provider-compatible type"
    ))
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
                contract.output_kind.as_deref(),
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

/// Seal a v3 semantic provider payload with the host-owned envelope. The
/// model returns only the three semantic fields (classifications, gaps,
/// rejected_claims). The host enforces:
///   - rejection of host-field injection on the provider payload,
///   - semantic-schema validation against `v3_semantic_provider_schema`,
///   - canonical sealed-envelope schema validation before the deterministic
///     evaluator reads the result,
///   - disjoint authority: every envelope field is set by the host and never
///     accepted from the model.
///
/// The returned bytes are the exact sealed envelope used by deterministic
/// evaluation. Failures use stable policy-blocked diagnostics so the CLI can
/// surface them as actionable.
fn staged_json_value(
    staged_inputs: &[StagedInput],
    names: &[&str],
    missing: &'static str,
) -> Result<Value> {
    let input = staged_inputs
        .iter()
        .find(|input| names.contains(&input.logical_name.as_str()))
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, missing))?;
    serde_json::from_slice(&fs::read(&input.staged_path)?)
        .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, missing))
}

fn data_object(value: &Value) -> &Value {
    value
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(value)
}

fn validate_v3_classification_evidence(
    semantic: &Value,
    selected_taxonomies: &[ClassificationTaxonomy],
    compiled_attribute_ids: &[String],
    observed_attempts: &[&Value],
) -> Result<()> {
    let known_attempt_ids = observed_attempts
        .iter()
        .filter_map(|attempt| attempt["attempt_id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    validate_v3_semantic_payload(
        semantic,
        selected_taxonomies,
        compiled_attribute_ids,
        &known_attempt_ids,
    )
    .map_err(|issues| {
        let issue = issues.first();
        let code = issue
            .map(|issue| v3_issue_to_failure_code(issue.code))
            .unwrap_or("v3-semantic-output-invalid");
        let detail = issue
            .map(|issue| v3_issue_diagnostic_detail(issue, code))
            .unwrap_or_else(|| {
                v3_static_diagnostic_detail(code, "$", "semantic-object", "invalid")
            });
        run_failure_with_diagnostic_detail(RunFailureKind::PolicyBlocked, code, detail)
    })?;

    let classifications = semantic["classifications"].as_object().ok_or_else(|| {
        run_failure_with_diagnostic_detail(
            RunFailureKind::PolicyBlocked,
            "v3-semantic-output-invalid",
            v3_static_diagnostic_detail(
                "v3-semantic-output-invalid",
                "$",
                "semantic-object",
                "missing",
            ),
        )
    })?;
    let expected_outputs = selected_taxonomies
        .iter()
        .map(|taxonomy| taxonomy.output_attribute.as_str())
        .collect::<HashSet<_>>();
    let actual_outputs = classifications
        .keys()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    if actual_outputs != expected_outputs {
        return Err(run_failure_with_diagnostic_detail(
            RunFailureKind::PolicyBlocked,
            "v3-classification-coverage-mismatch",
            v3_static_diagnostic_detail(
                "v3-classification-coverage-mismatch",
                "$.classifications",
                "compiled-classification-keys",
                "coverage-mismatch",
            ),
        ));
    }

    for taxonomy in selected_taxonomies {
        let classification = classifications
            .get(&taxonomy.output_attribute)
            .ok_or_else(|| {
                run_failure_with_diagnostic_detail(
                    RunFailureKind::PolicyBlocked,
                    "v3-classification-missing",
                    v3_static_diagnostic_detail(
                        "v3-classification-missing",
                        "$.classifications/*",
                        "classification-entry",
                        "missing",
                    ),
                )
            })?;
        if classification["taxonomy_id"] != taxonomy.id
            || classification["taxonomy_version"] != taxonomy.version
        {
            return Err(run_failure_with_diagnostic_detail(
                RunFailureKind::PolicyBlocked,
                "v3-classification-taxonomy-mismatch",
                v3_static_diagnostic_detail(
                    "v3-classification-taxonomy-mismatch",
                    "$.classifications/*",
                    "selected-taxonomy",
                    "mismatch",
                ),
            ));
        }
        let mut contributor_ids = HashSet::new();
        for attempt_id in classification["derived_from"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let attempt = observed_attempts
                .iter()
                .find(|attempt| attempt["attempt_id"] == attempt_id)
                .ok_or_else(|| {
                    run_failure_with_diagnostic_detail(
                        RunFailureKind::PolicyBlocked,
                        "v3-classification-evidence-invalid",
                        v3_static_diagnostic_detail(
                            "v3-classification-evidence-invalid",
                            "$.classifications/*/derived_from",
                            "collected-attempt-id",
                            "unresolved",
                        ),
                    )
                })?;
            let attribute_id = attempt["attribute_id"].as_str().unwrap_or_default();
            let source_class = attempt["source_class"].as_str().unwrap_or_default();
            let source_class_allowed = taxonomy.source_classes.iter().any(|allowed| {
                serde_json::to_value(allowed)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .as_deref()
                    == Some(source_class)
            });
            if !taxonomy
                .contributor_attribute_ids
                .iter()
                .any(|id| id == attribute_id)
                || !source_class_allowed
            {
                return Err(run_failure_with_diagnostic_detail(
                    RunFailureKind::PolicyBlocked,
                    "v3-classification-evidence-ineligible",
                    v3_static_diagnostic_detail(
                        "v3-classification-evidence-ineligible",
                        "$.classifications/*/derived_from",
                        "eligible-contributor-attempt",
                        "ineligible",
                    ),
                ));
            }
            contributor_ids.insert(attribute_id);
        }
        if contributor_ids.len() < taxonomy.minimum_evidence.observed_contributors as usize {
            return Err(run_failure_with_diagnostic_detail(
                RunFailureKind::PolicyBlocked,
                "v3-classification-minimum-evidence",
                v3_static_diagnostic_detail(
                    "v3-classification-minimum-evidence",
                    "$.classifications/*/derived_from",
                    "minimum-evidence",
                    "insufficient",
                ),
            ));
        }
    }
    Ok(())
}

fn host_wrap_v3_normalization_output(
    step: &CompiledModelStepV1,
    staged_inputs: &[StagedInput],
    invocation_value: &Value,
    invocation_bytes: &[u8],
    model_output: &str,
) -> Result<Vec<u8>> {
    let envelope = step.output_contract.host_envelope.as_ref().ok_or_else(|| {
        run_failure(
            RunFailureKind::PolicyBlocked,
            "normalization-host-envelope-metadata-missing",
        )
    })?;
    let requirements_input = staged_inputs
        .iter()
        .find(|input| {
            matches!(
                input.logical_name.as_str(),
                "decision-input-requirements" | "decision_input_requirements"
            )
        })
        .ok_or_else(|| {
            run_failure(
                RunFailureKind::PolicyBlocked,
                "v3-requirements-source-missing",
            )
        })?;
    validate_input_type(
        requirements_input,
        REQUIREMENTS_MODEL_CONTEXT_CONTRACT_V1,
        "application/json",
    )
    .map_err(|_| {
        run_failure(
            RunFailureKind::PolicyBlocked,
            "v3-requirements-context-schema-mismatch",
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
                "normalization-host-envelope-metadata-invalid",
            )
        })?;

    let semantic = serde_json::from_str::<Value>(model_output).map_err(|_| {
        run_failure_with_diagnostic_detail(
            RunFailureKind::PolicyBlocked,
            "v3-semantic-output-malformed",
            v3_static_diagnostic_detail(
                "v3-semantic-output-malformed",
                "$",
                "semantic-object",
                "malformed",
            ),
        )
    })?;
    reject_host_field_injection(&semantic).map_err(|issue| {
        let code = v3_issue_to_failure_code(issue.code);
        run_failure_with_diagnostic_detail(
            RunFailureKind::PolicyBlocked,
            code,
            v3_issue_diagnostic_detail(&issue, code),
        )
    })?;
    let semantic_schema = v3_semantic_provider_schema();
    if jsonschema::draft202012::validate(&semantic_schema, &semantic).is_err() {
        let code = "v3-semantic-output-invalid";
        return Err(run_failure_with_diagnostic_detail(
            RunFailureKind::PolicyBlocked,
            code,
            v3_schema_error_detail(&semantic_schema, &semantic, code),
        ));
    }
    // Parse the exact compiled artifact instead of treating its file hash as
    // the semantic identities it contains. This binds the sealed envelope to
    // the compiler-issued requirements and taxonomy-set hashes.
    let requirements_value = staged_json_value(
        staged_inputs,
        &["decision-input-requirements", "decision_input_requirements"],
        "v3-requirements-source-missing",
    )?;
    let requirements_data = data_object(&requirements_value);
    if requirements_data["contract"] != REQUIREMENTS_MODEL_CONTEXT_CONTRACT_V1
        || requirements_data["source_contract"] != "mdp.requirements.v2"
        || requirements_data["runtime_contract_version"] != "v3"
    {
        return Err(run_failure(
            RunFailureKind::PolicyBlocked,
            "v3-requirements-context-invalid",
        ));
    }
    let requirements_sha256 = requirements_data["requirements_sha256"]
        .as_str()
        .filter(|value| is_canonical_sha256(value))
        .ok_or_else(|| {
            run_failure(
                RunFailureKind::PolicyBlocked,
                "v3-requirements-hash-missing",
            )
        })?
        .to_owned();
    let taxonomy_set_sha256 = requirements_data["taxonomy_set_sha256"]
        .as_str()
        .filter(|value| is_canonical_sha256(value))
        .ok_or_else(|| {
            run_failure(
                RunFailureKind::PolicyBlocked,
                "v3-taxonomy-set-hash-missing",
            )
        })?
        .to_owned();
    let selected_taxonomies: Vec<ClassificationTaxonomy> = serde_json::from_value(
        requirements_data["classification_specification"]["taxonomies"].clone(),
    )
    .map_err(|_| run_failure(RunFailureKind::PolicyBlocked, "v3-taxonomy-set-invalid"))?;
    let compiled_attributes = requirements_data["decision_input_contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|contract| contract["attributes"].as_array().into_iter().flatten())
        .collect::<Vec<_>>();
    let compiled_attribute_ids = compiled_attributes
        .iter()
        .filter_map(|attribute| attribute["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();

    let collected_value = staged_json_value(
        staged_inputs,
        &["collected-attempt-results", "collected_attempt_results"],
        "v3-collected-attempt-results-missing",
    )?;
    let collected_data = data_object(&collected_value);
    let collected_attributes = collected_data["attributes"].as_object().ok_or_else(|| {
        run_failure(
            RunFailureKind::PolicyBlocked,
            "v3-collected-attempt-results-invalid",
        )
    })?;
    let attempt_results = collected_data["attempt_results"]
        .as_array()
        .ok_or_else(|| {
            run_failure(
                RunFailureKind::PolicyBlocked,
                "v3-collected-attempt-results-invalid",
            )
        })?;
    let observed_attempts = attempt_results
        .iter()
        .filter(|attempt| attempt["status"] == "observed")
        .collect::<Vec<_>>();
    validate_v3_classification_evidence(
        &semantic,
        &selected_taxonomies,
        &compiled_attribute_ids,
        &observed_attempts,
    )?;
    let source_binding_sha256 = staged_inputs
        .iter()
        .find(|input| {
            matches!(
                input.logical_name.as_str(),
                "source-binding" | "source_binding"
            )
        })
        .map(|input| input.authority.sha256.clone())
        .ok_or_else(|| run_failure(RunFailureKind::PolicyBlocked, "v3-source-binding-missing"))?;
    let source_attempt_request_sha256 = staged_inputs
        .iter()
        .find(|input| {
            matches!(
                input.logical_name.as_str(),
                "source-attempt-request" | "source_attempt_request"
            )
        })
        .map(|input| input.authority.sha256.clone())
        .ok_or_else(|| {
            run_failure(
                RunFailureKind::PolicyBlocked,
                "v3-source-attempt-request-missing",
            )
        })?;
    let collected_attempt_results_sha256 = staged_inputs
        .iter()
        .find(|input| {
            matches!(
                input.logical_name.as_str(),
                "collected-attempt-results" | "collected_attempt_results"
            )
        })
        .map(|input| input.authority.sha256.clone())
        .ok_or_else(|| {
            run_failure(
                RunFailureKind::PolicyBlocked,
                "v3-collected-attempt-results-missing",
            )
        })?;

    let mut decision_input_contract_ids: Vec<String> = invocation_value["inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|input| input["decision_input_contract_ids"].as_array())
        .flatten()
        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
        .collect();
    if decision_input_contract_ids.is_empty() {
        // Fall back to the bound DIC ids from the resolved step authority so
        // the sealed envelope proves identity even when the invocation does
        // not surface them.
        decision_input_contract_ids.extend(step.authority.ids.iter().cloned());
    }
    let mut normalization_entries: Vec<Value> = invocation_value["inputs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|input| input["normalization"].as_array())
        .flatten()
        .cloned()
        .collect();
    if normalization_entries.is_empty() {
        normalization_entries.push(json!({
            "contract_id": step.authority.ids.first().cloned().unwrap_or_default(),
            "prompt": step.prompt_path.clone(),
            "prompt_version": step.prompt_version.clone(),
            "prompt_sha256": step.prompt_sha256.clone(),
        }));
    }
    // The v3 semantic payload owns only classifications, gaps, and
    // rejected_claims. The host builds the validated `attributes` map from
    // the staged collected-attempt-results and source binding, never from
    // the model output.
    let classifications = semantic["classifications"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let mut attributes = Map::new();
    let mut fields = Map::new();
    let mut projected_attributes = Map::new();
    for attribute in &compiled_attributes {
        let Some(attribute_id) = attribute["id"].as_str() else {
            continue;
        };
        let output_path = attribute["output_path"].as_str().unwrap_or_default();
        let processing = attribute["processing"].as_str().unwrap_or("observed");
        let projected_value = if processing == "model-classified" {
            classifications
                .get(attribute_id)
                .filter(|classification| classification["status"] == "classified")
                .and_then(|classification| classification.get("value"))
                .cloned()
        } else {
            if let Some(collected_attribute) = collected_attributes.get(attribute_id) {
                attributes.insert(attribute_id.to_owned(), collected_attribute.clone());
            }
            let attempt = observed_attempts
                .iter()
                .find(|attempt| attempt["attribute_id"] == attribute_id);
            attempt.and_then(|attempt| attempt.get("value")).cloned()
        };
        let Some(value) = projected_value else {
            continue;
        };
        if let Some(name) = output_path.strip_prefix("attributes.") {
            projected_attributes.insert(name.to_owned(), value);
        } else if !output_path.is_empty() && !output_path.contains('.') {
            fields.insert(output_path.to_owned(), value);
        }
    }

    let mut signal_observations = Vec::new();
    for projection in requirements_data["decision_input_contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|contract| {
            contract["signal_projections"]
                .as_array()
                .into_iter()
                .flatten()
        })
    {
        let contributors = projection["contributor_attribute_ids"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<HashSet<_>>();
        for attempt in observed_attempts.iter().filter(|attempt| {
            attempt["attribute_id"]
                .as_str()
                .is_some_and(|id| contributors.contains(id))
        }) {
            signal_observations.push(json!({
                "id": attempt["attempt_id"],
                "title": attempt["value"],
                "value": attempt["value"],
                "source": attempt["source_locator"],
                "source_class": attempt["source_class"],
                "source_locator": attempt["source_locator"],
                "observed_at": attempt["observed_at"],
                "confidence": attempt["confidence"],
                "attempt_ids": [attempt["attempt_id"].clone()],
                "contributor_attribute_ids": [attempt["attribute_id"].clone()],
                "projection_id": projection["id"],
                "roles": projection["roles"],
                "kind": projection["kind"]
            }));
        }
    }
    let gaps: Vec<Value> = semantic["gaps"].as_array().cloned().unwrap_or_default();
    let rejected_claims: Vec<Value> = semantic["rejected_claims"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    // Deterministic projection of the private neutral `normalized_input`
    // surface. The host builds the neutral shape from validated semantic
    // output and host-staged identity hashes, without copying raw evidence
    // prose into the compact envelope.
    let mut normalized_input = Map::new();
    normalized_input.insert("fields".into(), Value::Object(fields));
    normalized_input.insert("signals".into(), Value::Array(signal_observations.clone()));
    normalized_input.insert("attributes".into(), Value::Object(projected_attributes));

    let invocation_receipt_sha256 = sha256_hex(invocation_bytes);

    let sealed = seal_v3_envelope(V3SealInputs {
        job_id: &step.job_id,
        decision_input_contract_ids: &decision_input_contract_ids,
        normalization_entries: &normalization_entries,
        requirements_sha256: &requirements_sha256,
        taxonomy_set_sha256: &taxonomy_set_sha256,
        source_binding_sha256: &source_binding_sha256,
        source_attempt_request_sha256: &source_attempt_request_sha256,
        collected_attempt_results_sha256: &collected_attempt_results_sha256,
        invocation_receipt_sha256: &invocation_receipt_sha256,
        attributes: &attributes,
        classifications: &classifications,
        signal_observations: &signal_observations,
        normalized_input: &normalized_input,
        gaps: &gaps,
        rejected_claims: &rejected_claims,
        outcome: V3_OUTCOME_KIND,
    });

    validate_v3_sealed_envelope(&sealed).map_err(|issues| {
        let issue = issues.first();
        let code = issue
            .map(|issue| v3_issue_to_failure_code(issue.code))
            .unwrap_or("v3-sealed-envelope-invalid");
        let detail = issue
            .map(|issue| v3_issue_diagnostic_detail(issue, code))
            .unwrap_or_else(|| {
                v3_static_diagnostic_detail(code, "$", "sealed-envelope", "invalid")
            });
        run_failure_with_diagnostic_detail(RunFailureKind::PolicyBlocked, code, detail)
    })?;

    let mut bytes = serde_json::to_vec_pretty(&sealed)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn v3_issue_to_failure_code(code: &str) -> &'static str {
    match code {
        "v3_host_owned_field_injection" => "v3-host-owned-field-injection",
        "v3_legacy_alias_paired_with_v3" => "v3-legacy-alias-paired-with-v3",
        "v3_provider_authority_field" => "v3-provider-authority-field",
        "v3_output_not_object" => "v3-output-not-object",
        "v3_envelope_contract_mismatch" => "v3-envelope-contract-mismatch",
        "v3_envelope_missing_neutral_input" => "v3-envelope-missing-neutral-input",
        "v3_envelope_schema_mismatch" => "v3-sealed-envelope-schema-mismatch",
        "v3_semantic_payload_malformed" => "v3-semantic-output-malformed",
        "v3_classification_invalid_status" => "v3-classification-invalid-status",
        "v3_classification_missing_value" => "v3-classification-missing-value",
        "v3_classification_forbidden_value" => "v3-classification-forbidden-value",
        "v3_classification_unknown_taxonomy" => "v3-classification-unknown-taxonomy",
        "v3_classification_unknown_value" => "v3-classification-unknown-value",
        "v3_classification_unknown_evidence_ref" => "v3-classification-unknown-evidence-ref",
        "v3_classification_missing_derived_from" => "v3-classification-missing-evidence",
        "v3_classification_derived_from_overflow" => "v3-classification-evidence-overflow",
        "v3_classification_basis_empty" => "v3-classification-basis-empty",
        "v3_classification_basis_too_long" => "v3-classification-basis-too-long",
        "v3_classification_unknown_attribute" => "v3-classification-unknown-attribute",
        "v3_classification_envelope_overflow" => "v3-classification-envelope-overflow",
        "v3_classification_duplicate_attribute" => "v3-classification-duplicate-attribute",
        "v3_gap_unknown_attribute" => "v3-gap-unknown-attribute",
        "v3_gap_unknown_evidence_ref" => "v3-gap-unknown-evidence-ref",
        "v3_rejected_claim_empty" => "v3-rejected-claim-empty",
        _ => "v3-sealed-envelope-invalid",
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
    if !projected.contains_key("type") {
        let inferred_type = if let Some(constant) = projected.get("const") {
            json_schema_type_for_value(constant)
                .ok_or_else(|| anyhow!("const schema value must have a provider-compatible type"))?
        } else if let Some(enum_value) = projected.get("enum") {
            infer_enum_type(enum_value)?
        } else {
            ""
        };
        if !inferred_type.is_empty() {
            projected.insert("type".into(), Value::String(inferred_type.into()));
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

fn deterministic_proposal_pursuit(
    input: &crate::decision_input::DecisionInput,
    classifications_ready: bool,
) -> (&'static str, Vec<String>) {
    let attribute = |name: &str| {
        input
            .attributes()
            .get(name)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
    };
    if attribute("policy_conflict_status") == Some("present") {
        return ("decline", vec!["policy-conflict".into()]);
    }
    let required_fields_ready = ["company", "background", "trigger"].iter().all(|name| {
        input
            .field(name)
            .and_then(Value::as_str)
            .is_some_and(|v| !v.trim().is_empty())
    });
    let required_attributes_ready = [
        "opportunity_stage",
        "opportunity_category",
        "source_safety",
        "proof_status",
        "policy_conflict_status",
    ]
    .iter()
    .all(|name| attribute(name).is_some());
    if !classifications_ready || !required_fields_ready || !required_attributes_ready {
        return ("review", vec!["insufficient-context".into()]);
    }
    match attribute("proof_status") {
        Some("approved" | "not-required") => ("pursue", vec!["evidence-ready".into()]),
        Some("gap") => ("review", vec!["proof-gap".into()]),
        Some("unsupported") => ("review", vec!["unsupported-proof".into()]),
        _ => ("review", vec!["proof-status-missing".into()]),
    }
}

fn proposal_v3_success_artifacts(
    request: &RunRequestV1,
    bundle: &RunBundleV1,
    bundle_sha256: &str,
    normalized: &Value,
    pursuit_decision: &str,
    reason_codes: Vec<String>,
) -> Result<SuccessArtifacts> {
    let generation_allowed = pursuit_decision == "pursue";
    let output = json!({
        "contract": "mdp.proposal-pursuit.v1",
        "execution_id": request.execution_id,
        "decision": pursuit_decision,
        "reason_codes": reason_codes,
        "generation_allowed": generation_allowed,
        "normalization": {
            "contract": NORMALIZED_DECISION_INPUT_CONTRACT_V3,
            "requirements_sha256": normalized["requirements_sha256"],
            "taxonomy_set_sha256": normalized["taxonomy_set_sha256"],
            "classifications": normalized["classifications"]
        }
    });
    let compiled_context = json!({
        "contract": "mdp.compiled-run-context.v1",
        "execution_id": request.execution_id,
        "profile": request.profile,
        "operation": request.operation,
        "bundle_sha256": bundle_sha256,
        "pack_portable_digest": bundle.pack.portable_digest,
        "requirements_sha256": normalized["requirements_sha256"],
        "taxonomy_set_sha256": normalized["taxonomy_set_sha256"],
        "pursuit": {
            "decision": pursuit_decision,
            "generation_allowed": generation_allowed,
            "reason_codes": output["reason_codes"]
        }
    });
    let mut output_bytes = serde_json::to_vec_pretty(&output)?;
    output_bytes.push(b'\n');
    let mut decision = DecisionAuthority {
        schema_id: "mdp.proposal-pursuit-decision.v1".into(),
        decision: pursuit_decision.into(),
        reason_codes,
        sha256: String::new(),
    };
    decision.sha256 =
        canonical_json_sha256_for_domain(&decision.schema_id, &serde_json::to_value(&decision)?)?;
    Ok(SuccessArtifacts {
        output_bytes,
        output_schema_id: "mdp.proposal-pursuit.v1".into(),
        compiled_context,
        decision,
    })
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

pub(crate) fn compiler_validate_request(request: &RunRequestV1) -> Result<()> {
    validate_request(request)
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
        || request.execution_policy.timeout_ms <= MAX_FINALIZATION_RESERVE_MS
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

fn check_sources_unchanged(inputs: &[StagedInput]) -> std::result::Result<(), RunDiagnostic> {
    for input in inputs {
        let metadata = fs::symlink_metadata(&input.source_path)
            .map_err(|_| source_integrity_input_diagnostic(input))?;
        let source_bytes = read_bounded(
            &input.source_path,
            input.authority.byte_count,
            "declared input",
        )
        .map_err(|_| source_integrity_input_diagnostic(input))?;
        let staged_bytes = read_bounded(
            &input.staged_path,
            input.authority.byte_count,
            "staged input",
        )
        .map_err(|_| source_integrity_input_diagnostic(input))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() != input.authority.byte_count
            || sha256_hex(&source_bytes) != input.initial_sha256
            || sha256_hex(&staged_bytes) != input.initial_sha256
        {
            return Err(source_integrity_input_diagnostic(input));
        }
    }
    Ok(())
}

fn verify_sources_unchanged(inputs: &[StagedInput]) -> Result<()> {
    check_sources_unchanged(inputs).map_err(|diagnostic| {
        run_failure_with_diagnostic(
            RunFailureKind::PolicyBlocked,
            "source-integrity-failed",
            diagnostic,
        )
    })
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
    #[cfg(unix)]
    use super::read_recovery_claim;
    use super::{
        MAX_EXECUTION_ID_BYTES, MAX_OUTPUT_LEAF_BYTES, MAX_RECOVERY_CLAIM_BYTES, RunDeadline,
        RunFailure, RunFailureKind, RunRecoveryClaim, deterministic_proposal_pursuit,
        execute_generative_step, execute_run_inner, execute_run_inner_with_driver,
        governed_normalization_outcome, gtm_lineage_schema_ids, gtm_success_artifacts,
        host_wrap_governed_output, host_wrap_v3_normalization_output,
        project_output_schema_for_openai, provider_max_output_tokens, provider_schema_source,
        provider_schema_source_for_contract, routed_context_shape_diagnostic,
        routed_context_validation_diagnostic, sanitized_host_envelope_diagnostic,
        sanitized_prompt_validation_diagnostic, seal_driver_request, seal_driver_result,
        serialize_recovery_claim, validate_driver_result, validate_request,
    };
    use crate::commands::init::init_pack;
    use crate::models::{PromptEntryDefaults, PromptHostEnvelope, PromptOutputContract};
    use crate::run_contracts::{
        ArtifactAuthority, AssuranceEvidenceState, DRIVER_REQUEST_V2, DRIVER_RESULT_V2,
        DeadlinePhase, DriverArtifactV2, DriverIdentity, DriverOutputV2,
        DriverProviderObservationV2, DriverProviderPolicyV2, DriverRequestV2, DriverResultV2,
        EvidenceProvenance, ExecutionPolicy, JobIdentity, LocalArtifactInput, ModelIdentity,
        PackAuthority, RunBundleV1, RunMode, RunRequestV1, TerminalState,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn proposal_input(
        proof_status: &str,
        policy_conflict_status: &str,
    ) -> crate::decision_input::DecisionInput {
        crate::decision_input::DecisionInput::new(
            BTreeMap::from([
                ("company".into(), serde_json::json!("Synthetic Agency")),
                (
                    "background".into(),
                    serde_json::json!("Bounded opportunity"),
                ),
                ("trigger".into(), serde_json::json!("2030-01-01")),
            ]),
            vec![],
            BTreeMap::from([
                ("opportunity_stage".into(), serde_json::json!("bid-no-bid")),
                (
                    "opportunity_category".into(),
                    serde_json::json!("public-services-review"),
                ),
                ("source_safety".into(), serde_json::json!("synthetic")),
                ("proof_status".into(), serde_json::json!(proof_status)),
                (
                    "policy_conflict_status".into(),
                    serde_json::json!(policy_conflict_status),
                ),
            ]),
        )
        .unwrap()
    }

    #[test]
    fn proposal_pursuit_is_deterministic_and_model_cannot_select_it() {
        assert_eq!(
            deterministic_proposal_pursuit(&proposal_input("approved", "none"), true).0,
            "pursue"
        );
        assert_eq!(
            deterministic_proposal_pursuit(&proposal_input("gap", "none"), true).0,
            "review"
        );
        assert_eq!(
            deterministic_proposal_pursuit(&proposal_input("approved", "present"), true).0,
            "decline"
        );
        assert_eq!(
            deterministic_proposal_pursuit(&proposal_input("approved", "none"), false).0,
            "review"
        );
    }

    #[test]
    fn deadline_plan_exposes_one_effective_bound_and_outer_warning() {
        let plan = RunDeadline::try_new(60_000, Some(120_000)).unwrap();
        assert_eq!(plan.effective_limit_ms, 60_000);
        assert!(plan.driver_timeout_ms().unwrap() <= 59_750);
        assert!(plan.driver_timeout_ms().unwrap() >= 59_700);
        assert_eq!(plan.warnings, vec!["outer-timeout-cannot-extend-inner"]);
        let shorter = RunDeadline::try_new(60_000, Some(30_000)).unwrap();
        assert_eq!(shorter.effective_limit_ms, 29_750);
        assert_eq!(shorter.warnings, vec!["outer-timeout-truncates-runtime"]);
    }

    #[test]
    fn deadline_plan_rejects_reserve_underflow() {
        for timeout in [250, 249] {
            let error = RunDeadline::try_new(timeout, None).unwrap_err();
            assert_eq!(
                error.downcast_ref::<RunFailure>().unwrap().code(),
                "deadline-reserve-underflow"
            );
        }
        assert!(RunDeadline::try_new(251, None).is_ok());
    }

    #[test]
    fn deadline_plan_rejects_limits_that_cannot_preserve_fixed_reserve() {
        for transport in [Some(250), Some(0)] {
            let error = RunDeadline::try_new(60_000, transport).unwrap_err();
            assert_eq!(
                error.downcast_ref::<RunFailure>().unwrap().code(),
                "transport-timeout-invalid"
            );
        }
        let plan = RunDeadline::try_new(1_000, None).unwrap();
        assert!(plan.driver_timeout_ms().unwrap() <= 750);
        assert!(plan.driver_timeout_ms().unwrap() >= 700);
    }

    #[test]
    fn policy_diagnostics_use_bounded_allowlisted_values() {
        for code in [
            "malformed-json",
            "wrong-contract",
            "missing-required-field",
            "disallowed-field",
            "readiness-failure",
            "stale-binding",
            "internal-contract-mismatch",
        ] {
            let failure = RunFailure::new(
                super::RunFailureKind::PolicyBlocked,
                "routed-context-invalid",
            );
            let diagnostic = if code == "wrong-contract" {
                super::policy_diagnostic(
                    "generative-preflight",
                    "routed-context-schema",
                    code,
                    Some("routed_context"),
                    Some("/contract"),
                    super::diagnostic_value("contract", super::ROUTED_CONTEXT_CONTRACT),
                    super::diagnostic_value("contract", "missing"),
                )
            } else {
                failure.diagnostics()[0].clone()
            };
            let encoded = serde_json::to_value(diagnostic).expect("diagnostic should serialize");
            assert!(encoded["stage"].is_string());
            assert!(encoded["gate"].is_string());
            assert!(encoded["input"].is_null() || encoded["input"] == "routed_context");
            assert!(
                !serde_json::to_string(&encoded)
                    .unwrap()
                    .contains("/private/customer")
            );
        }
        let fallback = RunFailure::new(
            super::RunFailureKind::PolicyBlocked,
            "required-model-input-missing",
        );
        assert_eq!(fallback.diagnostics()[0].expected.kind, "count");
        assert!(
            serde_json::to_vec(fallback.diagnostics()).unwrap().len()
                <= super::MAX_POLICY_DIAGNOSTIC_BYTES
        );
    }

    #[test]
    fn routed_context_diagnostics_redact_unknown_keys_and_classify_bindings() {
        let mut wrong =
            serde_json::json!({"contract": "mdp.routed-context.v1", "job": "synthetic-job"});
        wrong["attacker_secret"] = serde_json::json!("PRIVATE-SOURCE-BODY");
        let diagnostic = routed_context_shape_diagnostic(&wrong).expect("unknown field diagnostic");
        assert_eq!(diagnostic.code, "disallowed-field");
        assert_eq!(diagnostic.field, Some("/unknown-field"));
        assert!(
            !serde_json::to_string(&diagnostic)
                .unwrap()
                .contains("attacker_secret")
        );
        let stale = routed_context_validation_diagnostic(
            crate::routing::RoutedContextValidationKind::Canonical,
        );
        assert_eq!(stale.code, "stale-binding");
        assert_eq!(stale.gate, "routed-context-readiness");
    }

    #[test]
    fn driver_policy_block_uses_receipt_free_sanitized_failure() {
        let failure = RunFailure::new(RunFailureKind::PolicyBlocked, "driver-policy-blocked");
        assert_eq!(failure.code(), "driver-policy-blocked");
        assert_eq!(failure.diagnostics().len(), 1);
        let diagnostic = &failure.diagnostics()[0];
        assert_eq!(diagnostic.stage, "run-preflight");
        assert_eq!(diagnostic.gate, "policy");
        assert_eq!(diagnostic.code, "internal-contract-mismatch");
        assert!(diagnostic.input.is_none());
        let encoded = serde_json::to_string(&failure.diagnostics()).unwrap();
        assert!(encoded.len() <= super::MAX_POLICY_DIAGNOSTIC_BYTES);
        assert!(!encoded.contains("OPENAI_API_KEY"));
        assert!(!encoded.contains("native_model_calls_not_allowed"));
    }

    #[test]
    fn rejects_ambient_authority_for_deterministic_run() {
        let mut request = request_fixture("not-used", "not-used");
        request.execution_policy.network_mode = "allow".into();
        assert!(validate_request(&request).is_err());
    }

    #[test]
    fn run_request_timeout_reserves_the_fixed_finalization_window() {
        let mut request = request_fixture("not-used", "not-used");
        request.execution_policy.timeout_ms = 250;
        assert!(validate_request(&request).is_err());
        request.execution_policy.timeout_ms = 251;
        assert!(validate_request(&request).is_ok());
    }

    #[test]
    fn delayed_output_validation_preserves_the_validation_phase() {
        let deadline = RunDeadline::try_new(251, None).unwrap();
        deadline.mark_phase(DeadlinePhase::Validation);
        std::thread::sleep(std::time::Duration::from_millis(260));
        assert!(deadline.check_phase(DeadlinePhase::Validation).is_err());
        assert_eq!(
            deadline.terminal_observation().unwrap().phase,
            DeadlinePhase::Validation
        );
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
    fn provider_schema_projection_infers_primitive_const_and_enum_types() {
        let projected = project_output_schema_for_openai(&serde_json::json!({
            "type": "object",
            "properties": {
                "contract": {"const": "mdp.prompt-output.v0"},
                "state": {"enum": ["ready", "gap"]},
                "count": {"enum": [1, 2]},
                "ratio": {"enum": [0.5, 1.5]},
                "enabled": {"const": true}
            }
        }))
        .unwrap();
        assert_eq!(projected["properties"]["contract"]["type"], "string");
        assert_eq!(projected["properties"]["state"]["type"], "string");
        assert_eq!(projected["properties"]["count"]["type"], "integer");
        assert_eq!(projected["properties"]["ratio"]["type"], "number");
        assert_eq!(projected["properties"]["enabled"]["type"], "boolean");
    }

    #[test]
    fn provider_schema_projection_rejects_mixed_enum_types() {
        let error = project_output_schema_for_openai(&serde_json::json!({
            "type": "object",
            "properties": {"mixed": {"enum": ["ready", 1]}}
        }))
        .unwrap_err();
        assert!(error.to_string().contains("enum schema values must share"));
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
            skill_id: "mdp-pack-apply".into(),
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
        let outcome = super::host_envelope_failure_outcome(
            &request,
            result,
            "host-owned-field-injection",
            None,
        );
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

    #[test]
    fn v3_host_wrapper_diagnostics_preserve_only_static_reason_codes() {
        for code in [
            "v3-semantic-output-invalid",
            "v3-classification-coverage-mismatch",
            "v3-classification-taxonomy-mismatch",
            "v3-classification-evidence-invalid",
            "v3-classification-evidence-ineligible",
            "v3-classification-minimum-evidence",
            "v3-sealed-envelope-schema-mismatch",
        ] {
            let failure = super::run_failure(RunFailureKind::PolicyBlocked, code);
            assert_eq!(sanitized_host_envelope_diagnostic(&failure), code);
        }

        let internal = super::run_failure(
            RunFailureKind::PolicyBlocked,
            "private-provider-output-detail",
        );
        assert_eq!(
            sanitized_host_envelope_diagnostic(&internal),
            "host-envelope-failed"
        );
        assert_eq!(
            sanitized_host_envelope_diagnostic(&anyhow::anyhow!("raw model output")),
            "host-envelope-failed"
        );
    }

    #[test]
    fn normalization_host_envelope_codes_are_preserved_not_collapsed() {
        // The v3 normalization host envelope emits its own static, host-owned
        // metadata codes. They are bounded policy codes, so they must survive
        // the sanitizer instead of collapsing to host-envelope-failed.
        for code in [
            "normalization-host-envelope-metadata-missing",
            "normalization-host-envelope-metadata-invalid",
        ] {
            let failure = super::run_failure(RunFailureKind::PolicyBlocked, code);
            assert_eq!(sanitized_host_envelope_diagnostic(&failure), code);
        }
    }

    #[test]
    fn prompt_validation_diagnostics_preserve_only_safe_local_issue_codes() {
        let validation = serde_json::json!({
            "valid": false,
            "issues": [{"code": "decision_input_schema_mismatch", "message": "private detail"}]
        });
        assert_eq!(
            sanitized_prompt_validation_diagnostic(&validation),
            "decision-input-schema-mismatch"
        );

        for unsafe_code in [
            "raw provider output",
            "decision_input_bad/path",
            "ATTACK",
            "decision_input_................................................................................",
        ] {
            let validation = serde_json::json!({
                "valid": false,
                "issues": [{"code": unsafe_code}]
            });
            assert_eq!(
                sanitized_prompt_validation_diagnostic(&validation),
                "prompt-output-validation-failed"
            );
        }
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
        let normalized_input = root.join("normalized-input.json");
        let supplied_material = root.join("supplied-material.json");
        fs::create_dir_all(&root).unwrap();
        init_pack(&pack, "Host Envelope Pack", "proposal", true, false).unwrap();

        let brief =
            crate::commands::briefs::emit_brief(&pack, "Proposal Lead", None, Some("proof-review"))
                .unwrap();
        let routed_context_bytes =
            crate::artifact_hash::canonical_json_bytes(&brief["context"]["model_context"]).unwrap();
        fs::write(&routed_context, &routed_context_bytes).unwrap();
        fs::write(&normalized_input, b"{}\n").unwrap();
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
                    logical_name: "normalized-decision-input".into(),
                    source_path: normalized_input.display().to_string(),
                    schema_id: "mdp.synthetic-normalized-input.v1".into(),
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
            (
                "{\"raw_private_model_sentinel\":",
                "semantic-output-malformed",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let run = root.join(format!("published-run-{index}"));
            let model_output = model_output.to_string();
            let carries_raw_sentinel = model_output.contains("raw_private_model_sentinel");
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
            .unwrap_or_else(|error| panic!("run should reach wrapper rejection: {error:?}"));

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
            assert_eq!(receipt["terminal_state"], "no-draft:output-invalid");
            assert_eq!(receipt["diagnostic_code"].as_str(), Some(diagnostic_code));
            assert_eq!(receipt["diagnostic_phase"].as_str(), Some("validation"));
            let audit: crate::run_contracts::RunnerAuditV1 =
                serde_json::from_slice(&fs::read(run.join("runner-audit.json")).unwrap())
                    .unwrap();
            assert_eq!(audit.diagnostic_code.as_deref(), Some(diagnostic_code));
            assert_eq!(audit.diagnostic_phase.as_deref(), Some("validation"));
            assert_eq!(audit.provider_response_body_sha256, Some("4".repeat(64)));
            let observation = audit
                .provider_observation
                .expect("successful provider observation must survive host rejection");
            assert_eq!(observation.provider, "openai");
            assert_eq!(observation.response_id.as_deref(), Some("resp_host_transaction"));
            assert_eq!(observation.resolved_model.as_deref(), Some("gpt-5-mini"));
            assert_eq!(
                crate::commands::run_verification::verify_run_files(
                    Some(&run.join("run-bundle.json")),
                    &run.join("run-receipt.json"),
                    Some(&run),
                )
                .unwrap()["valid"],
                true
            );
            if carries_raw_sentinel {
                assert_published_tree_excludes(&run, "raw_private_model_sentinel");
            }
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn driver_result_rejection_codes_pass_through_with_driver_phase() {
        // Canary case 2: a transport-side runner failure and a
        // received-but-invalid output must stay distinguishable through their
        // stable bounded codes, both classified in the driver phase.
        for (index, (terminal_state, diagnostic_code, expected_receipt_state)) in [
            (
                TerminalState::NoDraftRunnerFailed,
                "provider-http-error",
                "no-draft:runner-failed",
            ),
            (
                TerminalState::NoDraftOutputInvalid,
                "model-output-invalid-json",
                "no-draft:output-invalid",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let root = temp_path(&format!("driver-rejection-{index}"));
            let pack = root.join("pack");
            let raw = root.join("raw-row.json");
            fs::create_dir_all(&root).unwrap();
            crate::commands::init::init_pack(&pack, "Driver Rejection Pack", "gtm", true, false)
                .unwrap();
            fs::write(&raw, "{\"company\":\"Synthetic Co\"}\n").unwrap();
            let request = generative_request_fixture(&pack, &raw);
            let run = root.join("published-run");
            let result = execute_run_inner_with_driver(
                &request,
                &run,
                || Ok(()),
                move |driver_request, _| {
                    let mut result = DriverResultV2 {
                        contract: DRIVER_RESULT_V2.into(),
                        execution_id: driver_request.execution_id.clone(),
                        operation: driver_request.operation.clone(),
                        terminal_state,
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
                        diagnostic_code: Some(diagnostic_code.into()),
                        result_sha256: String::new(),
                    };
                    seal_driver_result(&mut result)?;
                    Ok(result)
                },
            )
            .unwrap();
            assert_eq!(result.terminal_state, terminal_state);
            assert_eq!(result.diagnostic_code.as_deref(), Some(diagnostic_code));
            assert_eq!(result.diagnostic_phase.as_deref(), Some("driver"));

            let receipt: serde_json::Value =
                serde_json::from_slice(&fs::read(run.join("run-receipt.json")).unwrap()).unwrap();
            assert_eq!(receipt["terminal_state"], expected_receipt_state);
            assert_eq!(receipt["diagnostic_code"].as_str(), Some(diagnostic_code));
            assert_eq!(receipt["diagnostic_phase"].as_str(), Some("driver"));
            let audit: crate::run_contracts::RunnerAuditV1 =
                serde_json::from_slice(&fs::read(run.join("runner-audit.json")).unwrap()).unwrap();
            assert_eq!(audit.diagnostic_code.as_deref(), Some(diagnostic_code));
            assert_eq!(audit.diagnostic_phase.as_deref(), Some("driver"));
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
    fn post_bundle_driver_policy_block_returns_sanitized_failure_without_receipt() {
        let root = temp_path("generative-driver-policy-block");
        let pack = root.join("pack");
        let raw = root.join("raw-row.json");
        fs::create_dir_all(&root).unwrap();
        crate::commands::init::init_pack(&pack, "Driver Policy Pack", "gtm", true, false).unwrap();
        fs::write(&raw, "{\"company\":\"Synthetic Co\"}\n").unwrap();
        let request = generative_request_fixture(&pack, &raw);
        let run = root.join("published-run");
        let error = execute_run_inner_with_driver(
            &request,
            &run,
            || Ok(()),
            |driver_request, _| {
                let mut result = DriverResultV2 {
                    contract: DRIVER_RESULT_V2.into(),
                    execution_id: driver_request.execution_id.clone(),
                    operation: driver_request.operation.clone(),
                    terminal_state: TerminalState::NoDraftPolicyBlocked,
                    output: None,
                    provider_request_body_sha256: None,
                    provider_request_schema_id: None,
                    provider_response_body_sha256: None,
                    provider_output_schema_sha256: Some(
                        driver_request.provider_output_schema_sha256.clone(),
                    ),
                    provider_observation: None,
                    diagnostic_code: Some("native_model_calls_not_allowed".into()),
                    result_sha256: String::new(),
                };
                seal_driver_result(&mut result)?;
                Ok(result)
            },
        )
        .unwrap_err();
        let failure = error.downcast_ref::<RunFailure>().unwrap();
        assert!(matches!(failure.kind(), RunFailureKind::PolicyBlocked));
        assert_eq!(failure.code(), "driver-policy-blocked");
        assert_eq!(failure.diagnostics()[0].stage, "run-preflight");
        assert_eq!(failure.diagnostics()[0].gate, "policy");
        assert_eq!(failure.diagnostics()[0].code, "internal-contract-mismatch");
        assert!(!run.exists());
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
    fn late_post_bundle_deadline_discards_transaction_without_success_publication() {
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
        );
        assert!(result.is_err());
        assert!(!run.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn post_step_deadline_override_replaces_stale_model_diagnostic() {
        let root = temp_path("generative-timeout-diagnostic");
        let pack = root.join("pack");
        let raw = root.join("raw-row.json");
        fs::create_dir_all(&root).unwrap();
        init_pack(&pack, "Deadline Pack", "gtm", true, false).unwrap();
        fs::write(&raw, "{\"company\":\"Synthetic Co\"}\n").unwrap();
        let mut request = generative_request_fixture(&pack, &raw);
        request.execution_policy.timeout_ms = 5_000;
        refresh_test_native_declarations(&mut request);
        let transaction = root.join("tx");
        let deadline = RunDeadline::new(5_000);
        // The driver classifies the run as received-but-invalid output, then
        // the forced post-step deadline expiry replaces that outcome. The
        // published diagnostic must name the timeout phase, not the stale
        // model rejection code.
        let outcome = super::execute_transaction(
            &request,
            &transaction,
            &deadline,
            || {
                std::thread::sleep(std::time::Duration::from_millis(5_600));
                Ok(())
            },
            |driver_request, _, _| {
                let mut result = DriverResultV2 {
                    contract: DRIVER_RESULT_V2.into(),
                    execution_id: driver_request.execution_id.clone(),
                    operation: driver_request.operation.clone(),
                    terminal_state: TerminalState::NoDraftOutputInvalid,
                    output: None,
                    provider_request_body_sha256: None,
                    provider_request_schema_id: None,
                    provider_response_body_sha256: None,
                    provider_output_schema_sha256: Some(
                        driver_request.provider_output_schema_sha256.clone(),
                    ),
                    provider_observation: None,
                    diagnostic_code: Some("model-output-invalid-json".into()),
                    result_sha256: String::new(),
                };
                seal_driver_result(&mut result)?;
                Ok(result)
            },
        )
        .unwrap();
        assert_eq!(
            outcome.receipt.terminal_state,
            TerminalState::NoDraftRunnerFailed
        );
        assert_eq!(
            outcome.receipt.diagnostic_code.as_deref(),
            Some("finalization-timeout")
        );
        assert_eq!(
            outcome.receipt.diagnostic_phase.as_deref(),
            Some("finalization")
        );
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(transaction.join("run-receipt.json")).unwrap())
                .unwrap();
        assert_eq!(receipt["terminal_state"], "no-draft:runner-failed");
        assert_eq!(receipt["diagnostic_code"], "finalization-timeout");
        assert_eq!(receipt["diagnostic_phase"], "finalization");
        assert_eq!(receipt["deadline"]["outcome"], "timed-out");
        assert_eq!(receipt["deadline"]["phase"], "finalization");
        let audit: serde_json::Value =
            serde_json::from_slice(&fs::read(transaction.join("runner-audit.json")).unwrap())
                .unwrap();
        assert_eq!(audit["diagnostic_code"], "finalization-timeout");
        assert_eq!(audit["diagnostic_phase"], "finalization");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn source_mutation_override_clears_stale_model_diagnostic() {
        let root = temp_path("generative-mutation-diagnostic");
        let pack = root.join("pack");
        let raw = root.join("raw-row.json");
        fs::create_dir_all(&root).unwrap();
        init_pack(&pack, "Mutation Pack", "gtm", true, false).unwrap();
        fs::write(&raw, "{\"company\":\"Synthetic Co\"}\n").unwrap();
        let request = generative_request_fixture(&pack, &raw);
        let transaction = root.join("tx");
        let deadline = RunDeadline::new(30_000);
        // The driver classifies the run as received-but-invalid output, then a
        // source mutation during the post-check window replaces the outcome
        // with audit-incomplete. No model-rejection code may survive that
        // override; the schema-valid source-integrity diagnostics carry the
        // reason instead.
        let outcome = super::execute_transaction(
            &request,
            &transaction,
            &deadline,
            || {
                fs::write(&raw, "{\"company\":\"Mutated Co\"}\n")?;
                Ok(())
            },
            |driver_request, _, _| {
                let mut result = DriverResultV2 {
                    contract: DRIVER_RESULT_V2.into(),
                    execution_id: driver_request.execution_id.clone(),
                    operation: driver_request.operation.clone(),
                    terminal_state: TerminalState::NoDraftOutputInvalid,
                    output: None,
                    provider_request_body_sha256: None,
                    provider_request_schema_id: None,
                    provider_response_body_sha256: None,
                    provider_output_schema_sha256: Some(
                        driver_request.provider_output_schema_sha256.clone(),
                    ),
                    provider_observation: None,
                    diagnostic_code: Some("model-output-invalid-json".into()),
                    result_sha256: String::new(),
                };
                seal_driver_result(&mut result)?;
                Ok(result)
            },
        )
        .unwrap();
        assert_eq!(
            outcome.receipt.terminal_state,
            TerminalState::NoDraftAuditIncomplete
        );
        assert_eq!(outcome.receipt.diagnostic_code, None);
        assert_eq!(outcome.receipt.diagnostic_phase, None);
        assert_eq!(outcome.diagnostics[0].code, "stale-binding");
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(transaction.join("run-receipt.json")).unwrap())
                .unwrap();
        assert_eq!(receipt["terminal_state"], "no-draft:audit-incomplete");
        assert!(receipt.get("diagnostic_code").is_none());
        assert!(receipt.get("diagnostic_phase").is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn post_bundle_cancellation_publishes_sanitized_no_draft_receipt() {
        let root = temp_path("generative-cancellation-receipt");
        let pack = root.join("pack");
        let raw = root.join("raw-row.json");
        fs::create_dir_all(&root).unwrap();
        crate::commands::init::init_pack(&pack, "Cancellation Pack", "gtm", true, false).unwrap();
        fs::write(&raw, "{\"company\":\"Synthetic Co\"}\n").unwrap();
        let request = generative_request_fixture(&pack, &raw);
        let run = root.join("published-run");
        let result = execute_run_inner_with_driver(
            &request,
            &run,
            || Ok(()),
            |_, _| {
                Err(super::run_failure(
                    RunFailureKind::RunnerFailed,
                    "cancelled",
                ))
            },
        )
        .unwrap();
        assert_eq!(result.terminal_state, TerminalState::NoDraftRunnerFailed);
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(run.join("run-receipt.json")).unwrap()).unwrap();
        assert_eq!(receipt["terminal_state"], "no-draft:runner-failed");
        assert_eq!(receipt["deadline"]["outcome"], "cancelled");
        assert_eq!(receipt["deadline"]["phase"], "cancellation");
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
        let error = super::invoke_native_driver(
            &request,
            &identity,
            std::time::Instant::now() + std::time::Duration::from_millis(30_000),
        )
        .unwrap_err();
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
        let error = super::supervise_child(
            &mut child,
            std::time::Instant::now() + std::time::Duration::from_millis(25),
            1024,
        )
        .unwrap_err();
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
        let error = super::supervise_child(
            &mut child,
            std::time::Instant::now() + std::time::Duration::from_secs(10),
            128,
        )
        .unwrap_err();
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
        fs::write(
            pack.join(".mdp/prompts/normalize-opportunity.yaml"),
            include_str!("../tests/fixtures/legacy-proposal/normalize-opportunity.yaml"),
        )
        .unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let legacy_fixture =
            repository.join("scripts/fixtures/proposal-runner/normalize-opportunity-output.json");
        let output = root.join("legacy-normalize-opportunity-output.json");
        let mut legacy_output: serde_json::Value =
            serde_json::from_slice(&fs::read(&legacy_fixture).unwrap()).unwrap();
        for normalized_key in ["normalized_prospect", "normalized_opportunity"] {
            let attributes = legacy_output[normalized_key]["attributes"]
                .as_object_mut()
                .unwrap();
            for (name, value) in [
                ("review_mode_observation", serde_json::json!("bid/no-bid")),
                (
                    "buyer_context_observation",
                    serde_json::json!("Example Public Services Agency"),
                ),
                (
                    "requirement_observation",
                    serde_json::json!(
                        "Service request intake, status notifications, reporting, and training"
                    ),
                ),
                (
                    "opportunity_category",
                    serde_json::json!("public-services-review"),
                ),
                ("proof_status", serde_json::json!("not-required")),
                ("policy_conflict_status", serde_json::json!("none")),
            ] {
                attributes.insert(name.into(), value);
            }
        }
        fs::write(&output, serde_json::to_vec_pretty(&legacy_output).unwrap()).unwrap();
        let source_audit = repository.join("scripts/fixtures/proposal-runner/source-audit.json");
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
        assert_eq!(
            result.authority_block["diagnostics"][0]["code"],
            "stale-binding"
        );
        assert_eq!(
            result.authority_block["diagnostics"][0]["stage"],
            "source-integrity"
        );
        assert_eq!(
            result.authority_block["diagnostics"][0]["input"],
            "prompt-output"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pack_mutation_forces_audit_incomplete_and_no_output() {
        let root = temp_path("pack-mutation");
        let pack = root.join("pack");
        let run = root.join("run");
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let output =
            repository.join("scripts/fixtures/proposal-runner/normalize-opportunity-output.json");
        let source_audit = repository.join("scripts/fixtures/proposal-runner/source-audit.json");
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
        assert_eq!(
            result.authority_block["diagnostics"][0]["code"],
            "stale-binding"
        );
        assert_eq!(result.authority_block["diagnostics"][0]["input"], "pack");
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
        let output =
            repository.join("scripts/fixtures/proposal-runner/normalize-opportunity-output.json");
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
            assert_eq!(value["contract"], "mdp.run-recovery-claim.v2");
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
            assert_eq!(value["owner_uid"], unsafe { libc::geteuid() });
            assert_eq!(value["process_id"], std::process::id());
            assert!(value["created_unix_seconds"].as_u64().unwrap() > 0);
            assert!(value["transaction_dev"].as_u64().unwrap() > 0);
            assert!(value["transaction_ino"].as_u64().unwrap() > 0);
            assert_eq!(value.as_object().unwrap().len(), 8);
            Ok(())
        })
        .unwrap();

        assert_eq!(result.terminal_state, TerminalState::NoDraftOutputInvalid);
        assert!(!claim.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn recovery_claim_serialization_matches_exact_boundary() {
        let root = temp_path("recovery-claim-boundary");
        fs::create_dir_all(&root).unwrap();
        let claim_path = root.join("claim");
        let mut claim = RunRecoveryClaim {
            contract: "mdp.run-recovery-claim.v2".into(),
            execution_id: "e".repeat(MAX_EXECUTION_ID_BYTES),
            transaction_leaf: format!(
                ".{}.tmp-{}",
                "o".repeat(MAX_OUTPUT_LEAF_BYTES),
                "f".repeat(32)
            ),
            created_unix_seconds: u64::MAX,
            owner_uid: u32::MAX,
            process_id: u32::MAX,
            transaction_dev: u64::MAX,
            transaction_ino: u64::MAX,
        };

        let bytes = serialize_recovery_claim(&claim).unwrap();
        assert_eq!(bytes.len(), MAX_RECOVERY_CLAIM_BYTES);
        fs::write(&claim_path, &bytes).unwrap();
        let (persisted, metadata) = read_recovery_claim(&claim_path).unwrap();
        assert_eq!(metadata.len(), MAX_RECOVERY_CLAIM_BYTES as u64);
        assert_eq!(persisted.execution_id, claim.execution_id);
        assert_eq!(persisted.transaction_leaf, claim.transaction_leaf);
        assert_eq!(persisted.created_unix_seconds, u64::MAX);
        assert_eq!(persisted.owner_uid, u32::MAX);
        assert_eq!(persisted.process_id, u32::MAX);
        assert_eq!(persisted.transaction_dev, u64::MAX);
        assert_eq!(persisted.transaction_ino, u64::MAX);

        claim.execution_id.push('e');
        assert_eq!(
            serialize_recovery_claim(&claim).unwrap_err().to_string(),
            "output-claim-failed"
        );

        let mut oversized = bytes;
        oversized.push(b' ');
        fs::write(&claim_path, oversized).unwrap();
        assert_eq!(
            read_recovery_claim(&claim_path).unwrap_err(),
            "recovery-claim-type-unsafe"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn maximum_accepted_identifiers_publish_recovery_claim() {
        let root = temp_path("maximum-recovery-identifiers");
        let pack = root.join("pack");
        let output_leaf = "o".repeat(MAX_OUTPUT_LEAF_BYTES);
        let run = root.join(&output_leaf);
        let claim_path = root.join(format!(".{output_leaf}.mdp-run.claim"));
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        let output = root.join("invalid.json");
        fs::write(&output, "{}\n").unwrap();
        let mut request = request_fixture(pack.to_str().unwrap(), output.to_str().unwrap());
        request.execution_id = "e".repeat(MAX_EXECUTION_ID_BYTES);

        let result = execute_run_inner(&request, &run, || {
            let bytes = fs::read(&claim_path)?;
            assert!(bytes.len() <= MAX_RECOVERY_CLAIM_BYTES);
            let claim: RunRecoveryClaim = serde_json::from_slice(&bytes)?;
            assert_eq!(claim.execution_id, request.execution_id);
            assert_eq!(claim.transaction_leaf.len(), 1 + output_leaf.len() + 5 + 32);
            Ok(())
        })
        .unwrap();

        assert_eq!(result.terminal_state, TerminalState::NoDraftOutputInvalid);
        assert!(!claim_path.exists());
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

    #[test]
    fn proposal_v3_run_normalizes_then_applies_deterministic_pursuit_policy() {
        let root = temp_path("proposal-v3-run");
        let inputs = root.join("inputs");
        let run = root.join("run");
        fs::create_dir_all(&inputs).unwrap();
        let pack =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../plugin/assets/templates/proposal");
        let compiled = crate::commands::requirements::requirements(&pack, "bid-no-bid-review")
            .expect("proposal v3 requirements compile");
        let source_binding = inputs.join("source-binding.json");
        let source_request = inputs.join("source-request.json");
        let collected = inputs.join("collected.json");
        fs::write(&source_binding, b"{}\n").unwrap();
        fs::write(&source_request, b"{}\n").unwrap();
        let attempts = serde_json::json!({"attempt_results": [
            {"attempt_id":"buyer","attribute_id":"buyer_context_observation","status":"observed","value":"Public-services agency","source_class":"synthetic_fixture","source_locator":"fixture:buyer"},
            {"attempt_id":"requirement","attribute_id":"requirement_observation","status":"observed","value":"Governed service intake","source_class":"synthetic_fixture","source_locator":"fixture:requirement"},
            {"attempt_id":"stage","attribute_id":"review_mode_observation","status":"observed","value":"Formal bid/no-bid gate","source_class":"synthetic_fixture","source_locator":"fixture:stage"}
        ]});
        fs::write(&collected, serde_json::to_vec_pretty(&attempts).unwrap()).unwrap();
        let file_hash = |path: &Path| crate::artifact_hash::sha256_hex(&fs::read(path).unwrap());
        let normalized = inputs.join("normalized.json");
        let envelope = serde_json::json!({
            "contract": "mdp.normalized-decision-input.v3",
            "job_id": "bid-no-bid-review",
            "decision_input_contracts": ["proposal.opportunity-context"],
            "normalization": [{"contract_id":"proposal.opportunity-context","prompt":"prompts/normalize-opportunity.yaml","prompt_version":"proposal-opportunity-context.v3"}],
            "requirements_sha256": compiled["requirements_sha256"],
            "taxonomy_set_sha256": compiled["taxonomy_set_sha256"],
            "source_binding_sha256": file_hash(&source_binding),
            "source_attempt_request_sha256": file_hash(&source_request),
            "collected_attempt_results_sha256": file_hash(&collected),
            "invocation_receipt_sha256": "f".repeat(64),
            "attributes": {},
            "signal_observations": [],
            "normalized_input": {
                "fields": {"company":"Synthetic Agency","background":"Bounded opportunity","trigger":"2030-01-01"},
                "signals": [],
                "attributes": {
                    "opportunity_stage":"bid-no-bid",
                    "opportunity_category":"public-services-review",
                    "source_safety":"synthetic",
                    "proof_status":"approved",
                    "policy_conflict_status":"none"
                }
            },
            "outcome": "decision-input-normalization",
            "classifications": {
                "opportunity_stage": {"status":"classified","value":"bid-no-bid","taxonomy_id":"proposal-stage","taxonomy_version":"1.0.0","derived_from":["stage"],"basis":"The observed review mode explicitly identifies the formal bid/no-bid gate."},
                "opportunity_category": {"status":"classified","value":"public-services-review","taxonomy_id":"proposal-category","taxonomy_version":"1.0.0","derived_from":["buyer","requirement"],"basis":"Separate buyer and requirement observations establish a public-services review."}
            },
            "gaps": [],
            "rejected_claims": []
        });
        if let Err(issues) =
            crate::commands::v3_normalization::validate_v3_sealed_envelope(&envelope)
        {
            panic!("proposal v3 fixture envelope invalid: {issues:?}");
        }
        fs::write(&normalized, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let input = |logical_name: &str, path: &Path, schema_id: &str, media_type: &str| {
            LocalArtifactInput {
                logical_name: logical_name.into(),
                source_path: path.display().to_string(),
                schema_id: schema_id.into(),
                media_type: media_type.into(),
                provenance_refs: vec![],
            }
        };
        let mut request = RunRequestV1 {
            contract: "mdp.run-request.v1".into(),
            execution_id: "proposal-v3-review".into(),
            created_at: "2026-08-31T00:00:00Z".into(),
            profile: "proposal".into(),
            operation: "review".into(),
            mode: RunMode::Deterministic,
            job_identity: None,
            pack_dir: pack.display().to_string(),
            pack_release_id: "proposal-v3-test".into(),
            prompt: None,
            inputs: vec![
                input(
                    "normalized-decision-input",
                    &normalized,
                    "mdp.normalized-decision-input.v3",
                    "application/json",
                ),
                input(
                    "source-binding",
                    &source_binding,
                    "mdp.source-binding.v2",
                    "application/json",
                ),
                input(
                    "source-attempt-request",
                    &source_request,
                    "mdp.source-attempt-request.v2",
                    "application/json",
                ),
                input(
                    "collected-attempt-results",
                    &collected,
                    "mdp.collected-attempt-results.v2",
                    "application/json",
                ),
                input(
                    "bound-prompt",
                    &pack.join(".mdp/prompts/normalize-opportunity.yaml"),
                    "mdp.prompt.v0",
                    "application/yaml",
                ),
            ],
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
        };
        let result = execute_run_inner(&request, &run, || Ok(())).expect("proposal v3 run");
        assert_eq!(result.terminal_state, TerminalState::Success);
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(run.join("run-receipt.json")).unwrap()).unwrap();
        assert_eq!(receipt["decision"]["decision"], "pursue");
        assert!(run.join("artifacts/output.json").is_file());

        for (field, value) in [
            ("taxonomy_set_sha256", serde_json::json!("a".repeat(64))),
            ("requirements_sha256", serde_json::json!("b".repeat(64))),
        ] {
            let mut tampered = envelope.clone();
            tampered[field] = value;
            fs::write(&normalized, serde_json::to_vec_pretty(&tampered).unwrap()).unwrap();
            request.execution_id = format!("proposal-v3-tampered-{field}");
            let tampered_run = root.join(format!("tampered-{field}"));
            let error = execute_run_inner(&request, &tampered_run, || Ok(()))
                .expect_err("compiled v3 identity tampering must fail closed");
            assert_eq!(
                error.downcast_ref::<RunFailure>().map(RunFailure::code),
                Some("v3-compiled-identity-mismatch")
            );
            assert!(!tampered_run.exists());
        }
        let _ = fs::remove_dir_all(root);
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
                deadline_at_ms: 59_750,
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
            &super::RunDeadline::new(30_000),
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

    fn assert_published_tree_excludes(run: &std::path::Path, needle: &str) {
        fn walk(dir: &std::path::Path, needle: &str) {
            for entry in fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    walk(&entry.path(), needle);
                } else {
                    let bytes = fs::read(entry.path()).unwrap();
                    assert!(
                        !bytes
                            .windows(needle.len())
                            .any(|window| window == needle.as_bytes()),
                        "raw model output bytes leaked into {}",
                        entry.path().display()
                    );
                }
            }
        }
        walk(run, needle);
    }

    fn nonce() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }

    fn v3_envelope_test_step() -> crate::model_steps::CompiledModelStepV1 {
        let owned: Vec<String> = crate::models::NORMALIZATION_HOST_ENVELOPE_OWNED_FIELDS
            .iter()
            .map(|field| (*field).to_string())
            .collect();
        let semantic: Vec<String> = crate::models::NORMALIZATION_HOST_ENVELOPE_SEMANTIC_FIELDS
            .iter()
            .map(|field| (*field).to_string())
            .collect();
        let mut required_top_level = owned.clone();
        required_top_level.extend(semantic.iter().cloned());
        crate::model_steps::CompiledModelStepV1 {
            contract: crate::model_steps::COMPILED_MODEL_STEP_V1.into(),
            step_id: "model:prospect-fit-or-brief/normalization".into(),
            job_id: "prospect-fit-or-brief".into(),
            skill_id: "mdp-pack-apply".into(),
            phase: crate::model_steps::ModelStepPhase::Normalization,
            authority: crate::model_steps::ModelStepAuthorityV1 {
                kind: "decision_input_contract".into(),
                ids: vec!["gtm.prospect-context".into()],
            },
            prompt_id: "normalize-prospect-row".into(),
            prompt_version: "gtm-prospect-context.v3".into(),
            prompt_path: "prompts/normalize-prospect.yaml".into(),
            prompt_sha256: "a".repeat(64),
            declared_inputs: vec![],
            routed_context_required: false,
            output_contract: PromptOutputContract {
                contract: crate::constants::PROMPT_OUTPUT_CONTRACT.into(),
                output_kind: Some(
                    crate::constants::OUTPUT_KIND_DECISION_INPUT_NORMALIZATION.into(),
                ),
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
                schema: Some(serde_json::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["classifications"],
                    "properties": {
                        "classifications": {"type": "object"},
                        "gaps": {"type": "array"},
                        "rejected_claims": {"type": "array"}
                    }
                })),
                host_envelope: Some(PromptHostEnvelope {
                    contract: crate::models::NORMALIZATION_HOST_ENVELOPE_CONTRACT.into(),
                    owned_top_level: owned,
                    semantic_required_top_level: semantic,
                }),
                example: serde_json::json!({}),
            },
            output_contract_sha256: "b".repeat(64),
        }
    }

    fn v3_staged_inputs(requirements_sha: &str) -> Vec<super::StagedInput> {
        vec![
            super::StagedInput {
                logical_name: "decision-input-requirements".into(),
                authority: super::ArtifactAuthority {
                    logical_name: "decision-input-requirements".into(),
                    schema_id: "mdp.requirements-model-context.v1".into(),
                    media_type: "application/json".into(),
                    byte_count: 0,
                    sha256: requirements_sha.into(),
                    provenance: EvidenceProvenance::MdpObserved,
                    provenance_refs: vec![],
                },
                source_path: PathBuf::new(),
                staged_path: PathBuf::new(),
                initial_sha256: requirements_sha.into(),
            },
            super::StagedInput {
                logical_name: "source-binding".into(),
                authority: super::ArtifactAuthority {
                    logical_name: "source-binding".into(),
                    schema_id: "mdp.source-binding.v2".into(),
                    media_type: "application/json".into(),
                    byte_count: 0,
                    sha256: "c".repeat(64),
                    provenance: EvidenceProvenance::MdpObserved,
                    provenance_refs: vec![],
                },
                source_path: PathBuf::new(),
                staged_path: PathBuf::new(),
                initial_sha256: "c".repeat(64),
            },
            super::StagedInput {
                logical_name: "source-attempt-request".into(),
                authority: super::ArtifactAuthority {
                    logical_name: "source-attempt-request".into(),
                    schema_id: "mdp.source-attempt-request.v2".into(),
                    media_type: "application/json".into(),
                    byte_count: 0,
                    sha256: "d".repeat(64),
                    provenance: EvidenceProvenance::MdpObserved,
                    provenance_refs: vec![],
                },
                source_path: PathBuf::new(),
                staged_path: PathBuf::new(),
                initial_sha256: "d".repeat(64),
            },
            super::StagedInput {
                logical_name: "collected-attempt-results".into(),
                authority: super::ArtifactAuthority {
                    logical_name: "collected-attempt-results".into(),
                    schema_id: "mdp.collected-attempt-results.v2".into(),
                    media_type: "application/json".into(),
                    byte_count: 0,
                    sha256: "e".repeat(64),
                    provenance: EvidenceProvenance::MdpObserved,
                    provenance_refs: vec![],
                },
                source_path: PathBuf::new(),
                staged_path: PathBuf::new(),
                initial_sha256: "e".repeat(64),
            },
        ]
    }

    fn v3_invocation(
        step: &crate::model_steps::CompiledModelStepV1,
    ) -> (serde_json::Value, Vec<u8>) {
        let invocation = serde_json::json!({
            "inputs": [{
                "name": "decision-input-requirements",
                "decision_input_contract_ids": ["gtm.prospect-context"],
                "normalization": [{
                    "contract_id": "gtm.prospect-context",
                    "prompt": "prompts/normalize-prospect.yaml",
                    "prompt_version": "gtm-prospect-context.v3",
                    "prompt_sha256": step.prompt_sha256.clone()
                }]
            }]
        });
        let bytes = serde_json::to_vec(&invocation).unwrap();
        (invocation, bytes)
    }

    fn materialize_v3_staged_inputs(staged: &mut [super::StagedInput]) -> PathBuf {
        let temp = temp_path("v3-staged");
        std::fs::create_dir_all(&temp).unwrap();
        for input in staged {
            let path = temp.join(format!("{}.json", input.logical_name));
            let value = match input.logical_name.as_str() {
                "decision-input-requirements" => serde_json::json!({
                    "contract": "mdp.requirements-model-context.v1",
                    "source_contract": "mdp.requirements.v2",
                    "runtime_contract_version": "v3",
                    "requirements_sha256": "a".repeat(64),
                    "taxonomy_set_sha256": "b".repeat(64),
                    "pack": {"id": "runtime-test-pack", "version": "0.1.0", "sha256": "c".repeat(64)},
                    "job": {"id": "prospect-fit-or-brief", "skill_id": "mdp-pack-apply"},
                    "collection_specification": {"contract": "mdp.source-attempt-request.v2", "attributes": []},
                    "classification_specification": {"taxonomies": [{
                        "id": "buyer-persona", "version": "1", "output_attribute": "persona",
                        "contributor_attribute_ids": ["person_title"],
                        "source_classes": ["synthetic_fixture"],
                        "minimum_evidence": {"observed_contributors": 1},
                        "basis_max_chars": 500,
                        "ambiguity_policy": "human-review", "no_match_policy": "gap", "conflict_policy": "human-review",
                        "values": [{"value": "GTM Engineering", "definition": "Owns GTM systems."}]
                    }]},
                    "decision_input_contracts": [{"id": "gtm.prospect-context", "attributes": [
                        {"id": "person_title", "processing": "observed", "output_path": "title"},
                        {"id": "person_location", "processing": "observed", "output_path": "attributes.location"},
                        {"id": "persona", "processing": "model-classified", "output_path": "persona"}
                    ], "signal_projections": []}],
                    "normalized_output_schema": {"type": "object"},
                    "semantic_validation": {"contract": "mdp.normalization-semantic-provider.v3"},
                    "no_draft_policy": {"draft_allowed": false},
                    "boundaries": {"model_owned": ["classifications", "gaps", "rejected_claims"]}
                }),
                "collected-attempt-results" => serde_json::json!({
                    "attributes": {
                        "person_title": {
                            "status": "observed",
                            "value": "Founding GTM Engineer"
                        },
                        "person_location": {
                            "status": "not_found"
                        }
                    },
                    "attempt_results": [
                        {
                            "attempt_id": "synthetic-attempt-001", "attribute_id": "person_title",
                            "status": "observed", "value": "Founding GTM Engineer",
                            "source_class": "synthetic_fixture", "source_locator": "fixture:title"
                        },
                        {
                            "attempt_id": "synthetic-attempt-002", "attribute_id": "person_location",
                            "status": "not_found", "source_class": "synthetic_fixture",
                            "source_locator": "fixture:location"
                        }
                    ]
                }),
                _ => serde_json::json!({}),
            };
            std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            input.staged_path = path.clone();
            input.source_path = path;
        }
        temp
    }

    #[test]
    fn native_validation_binds_to_canonical_prompt_after_private_staging() {
        let root = temp_path("canonical-prompt-binding");
        let staged_pack = root.join("pack");
        let canonical_prompt = staged_pack.join(".mdp/prompts/normalize-prospect.yaml");
        let private_prompt = root.join("private/prompt.yaml");
        fs::create_dir_all(canonical_prompt.parent().unwrap()).unwrap();
        fs::create_dir_all(private_prompt.parent().unwrap()).unwrap();
        fs::write(&canonical_prompt, "synthetic governed prompt\n").unwrap();
        fs::write(&private_prompt, "synthetic governed prompt\n").unwrap();

        let step = v3_envelope_test_step();
        let staged_prompt = super::StagedInput {
            logical_name: step.prompt_id.clone(),
            authority: super::ArtifactAuthority {
                logical_name: step.prompt_id.clone(),
                schema_id: "mdp.prompt.v1".into(),
                media_type: "application/yaml".into(),
                byte_count: 26,
                sha256: crate::artifact_hash::sha256_hex(b"synthetic governed prompt\n"),
                provenance: EvidenceProvenance::MdpObserved,
                provenance_refs: vec![],
            },
            source_path: canonical_prompt.clone(),
            staged_path: private_prompt.clone(),
            initial_sha256: crate::artifact_hash::sha256_hex(b"synthetic governed prompt\n"),
        };

        super::validate_selected_prompt(&staged_pack, &staged_prompt, &step)
            .expect("an identical private prompt copy must pass byte binding");
        assert_eq!(
            super::canonical_selected_prompt_path(&staged_pack, &step),
            canonical_prompt
        );
        assert_ne!(
            super::canonical_selected_prompt_path(&staged_pack, &step),
            private_prompt,
            "semantic validation must bind to the compiled pack path, not the private copy path"
        );

        fs::write(&private_prompt, "different prompt\n").unwrap();
        let error = super::validate_selected_prompt(&staged_pack, &staged_prompt, &step)
            .expect_err("a different private prompt copy must fail closed");
        assert_eq!(
            error.downcast_ref::<RunFailure>().unwrap().code(),
            "selected-prompt-mismatch"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn v3_provider_preflight_projects_only_semantic_fields() {
        let step = v3_envelope_test_step();
        let canonical = crate::commands::v3_normalization::v3_sealed_envelope_schema();

        let source = provider_schema_source_for_contract(&canonical, &step.output_contract)
            .expect("normalization host envelope should pass provider preflight");

        assert_eq!(
            source["required"],
            serde_json::json!(["classifications", "gaps", "rejected_claims"])
        );
        assert!(source["properties"].get("classifications").is_some());
        assert!(source["properties"].get("contract").is_none());
        assert!(source["properties"].get("normalized_input").is_none());
    }

    #[test]
    fn v3_wrap_seals_a_semantic_payload_with_host_owned_fields() {
        let step = v3_envelope_test_step();
        let mut staged = v3_staged_inputs(&"a".repeat(64));
        let temp = materialize_v3_staged_inputs(&mut staged);
        let (invocation, invocation_bytes) = v3_invocation(&step);
        let semantic = serde_json::json!({
            "classifications": {
                "persona": {
                    "status": "classified",
                    "value": "GTM Engineering",
                    "taxonomy_id": "buyer-persona",
                    "taxonomy_version": "1",
                    "derived_from": ["synthetic-attempt-001"],
                    "basis": "title says it"
                }
            },
            "gaps": [{
                "attribute": "person_location",
                "reason": "not observed",
                "derived_from": ["synthetic-attempt-001"],
                "taxonomy_id": "buyer-persona"
            }],
            "rejected_claims": [{
                "claim": "unsupported claim",
                "reason": "not evidenced"
            }]
        });
        let result = host_wrap_v3_normalization_output(
            &step,
            &staged,
            &invocation,
            &invocation_bytes,
            &serde_json::to_string(&semantic).unwrap(),
        );
        let bytes = result.expect("v3 seal should succeed");
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            parsed["invocation_receipt_sha256"],
            crate::artifact_hash::sha256_hex(&invocation_bytes)
        );
        assert_eq!(
            parsed["attributes"]["person_title"],
            serde_json::json!({
                "status": "observed",
                "value": "Founding GTM Engineer"
            })
        );
        assert_eq!(
            parsed["attributes"]["person_location"],
            serde_json::json!({"status": "not_found"})
        );
        assert!(
            parsed["normalized_input"]["attributes"]
                .get("location")
                .is_none()
        );
        assert_eq!(
            parsed["gaps"][0],
            serde_json::json!({
                "attribute": "person_location",
                "reason": "not observed",
                "derived_from": ["synthetic-attempt-001"],
                "taxonomy_id": "buyer-persona"
            })
        );
        assert_eq!(
            parsed["rejected_claims"][0],
            serde_json::json!({
                "claim": "unsupported claim",
                "reason": "not evidenced"
            })
        );
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn v3_wrap_rejects_classification_from_non_contributor_evidence() {
        let step = v3_envelope_test_step();
        let mut staged = v3_staged_inputs(&"a".repeat(64));
        let temp = materialize_v3_staged_inputs(&mut staged);
        let requirements = staged
            .iter()
            .find(|input| input.logical_name == "decision-input-requirements")
            .unwrap();
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&requirements.staged_path).unwrap()).unwrap();
        value["classification_specification"]["taxonomies"][0]["contributor_attribute_ids"] =
            serde_json::json!(["person_responsibilities"]);
        std::fs::write(
            &requirements.staged_path,
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        let (invocation, invocation_bytes) = v3_invocation(&step);
        let semantic = serde_json::json!({
            "classifications": {"persona": {
                "status": "classified", "value": "GTM Engineering",
                "taxonomy_id": "buyer-persona", "taxonomy_version": "1",
                "derived_from": ["synthetic-attempt-001"], "basis": "title only"
            }}, "gaps": [], "rejected_claims": []
        });
        let error = host_wrap_v3_normalization_output(
            &step,
            &staged,
            &invocation,
            &invocation_bytes,
            &semantic.to_string(),
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "v3-classification-evidence-ineligible");
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn v3_wrap_rejects_host_field_injection_in_provider_payload() {
        let step = v3_envelope_test_step();
        let staged = v3_staged_inputs(&"a".repeat(64));
        let (invocation, invocation_bytes) = v3_invocation(&step);
        let bad = serde_json::json!({
            "contract": "mdp.normalized-decision-input.v3",
            "classifications": {}
        });
        let error = host_wrap_v3_normalization_output(
            &step,
            &staged,
            &invocation,
            &invocation_bytes,
            &serde_json::to_string(&bad).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<RunFailure>().unwrap().code(),
            "v3-host-owned-field-injection"
        );
    }

    #[test]
    fn v3_wrap_rejects_mixed_v3_legacy_aliases_in_provider_payload() {
        let step = v3_envelope_test_step();
        let staged = v3_staged_inputs(&"a".repeat(64));
        let (invocation, invocation_bytes) = v3_invocation(&step);
        let bad = serde_json::json!({
            "classifications": {},
            "normalized_prospect": {"legacy": true}
        });
        let error = host_wrap_v3_normalization_output(
            &step,
            &staged,
            &invocation,
            &invocation_bytes,
            &serde_json::to_string(&bad).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<RunFailure>().unwrap().code(),
            "v3-legacy-alias-paired-with-v3"
        );
    }

    #[test]
    fn v3_wrap_rejects_malformed_provider_payload() {
        let step = v3_envelope_test_step();
        let staged = v3_staged_inputs(&"a".repeat(64));
        let (invocation, invocation_bytes) = v3_invocation(&step);
        let error =
            host_wrap_v3_normalization_output(&step, &staged, &invocation, &invocation_bytes, "{")
                .unwrap_err();
        assert_eq!(
            error.downcast_ref::<RunFailure>().unwrap().code(),
            "v3-semantic-output-malformed"
        );
    }

    #[test]
    fn v3_wrap_rejects_schema_invalid_payload_with_bounded_detail() {
        let step = v3_envelope_test_step();
        let staged = v3_staged_inputs(&"a".repeat(64));
        let (invocation, invocation_bytes) = v3_invocation(&step);
        let semantic = serde_json::json!({
            "classifications": {},
            "gaps": [{"attribute": 7, "reason": "raw-schema-secret-sentinel"}],
            "rejected_claims": []
        });
        let error = host_wrap_v3_normalization_output(
            &step,
            &staged,
            &invocation,
            &invocation_bytes,
            &semantic.to_string(),
        )
        .unwrap_err();
        let failure = error.downcast_ref::<RunFailure>().unwrap();
        assert_eq!(failure.code(), "v3-semantic-output-invalid");
        let detail = failure
            .diagnostic_detail()
            .expect("schema rejection should carry bounded detail");
        assert_eq!(detail.code, "v3-semantic-output-invalid");
        assert!(detail.path.starts_with("$/"));
        assert!(detail.path.chars().count() <= 256);
        assert_eq!(detail.expected, "json-type");
        assert_eq!(detail.observed, "number");
        assert!(
            !serde_json::to_string(detail)
                .unwrap()
                .contains("raw-schema-secret-sentinel")
        );
    }

    #[test]
    fn v3_wrap_rejects_when_requirements_source_missing() {
        let step = v3_envelope_test_step();
        let mut staged = v3_staged_inputs(&"a".repeat(64));
        staged.retain(|i| i.logical_name != "decision-input-requirements");
        let (invocation, invocation_bytes) = v3_invocation(&step);
        let semantic = serde_json::json!({"classifications": {}});
        let error = host_wrap_v3_normalization_output(
            &step,
            &staged,
            &invocation,
            &invocation_bytes,
            &serde_json::to_string(&semantic).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<RunFailure>().unwrap().code(),
            "v3-requirements-source-missing"
        );
    }

    #[test]
    fn v3_wrap_rejects_invalid_envelope_contract_for_decision_input_normalization() {
        let mut step = v3_envelope_test_step();
        // Force a wrong envelope contract; the metadata validator must
        // refuse before any provider call.
        step.output_contract.host_envelope = Some(PromptHostEnvelope {
            contract: crate::constants::GOVERNED_HOST_ENVELOPE_CONTRACT.into(),
            owned_top_level: crate::models::NORMALIZATION_HOST_ENVELOPE_OWNED_FIELDS
                .iter()
                .map(|f| (*f).to_string())
                .collect(),
            semantic_required_top_level: crate::models::NORMALIZATION_HOST_ENVELOPE_SEMANTIC_FIELDS
                .iter()
                .map(|f| (*f).to_string())
                .collect(),
        });
        let staged = v3_staged_inputs(&"a".repeat(64));
        let (invocation, invocation_bytes) = v3_invocation(&step);
        let semantic = serde_json::json!({"classifications": {}});
        let error = host_wrap_v3_normalization_output(
            &step,
            &staged,
            &invocation,
            &invocation_bytes,
            &serde_json::to_string(&semantic).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<RunFailure>().unwrap().code(),
            "normalization-host-envelope-metadata-invalid"
        );
    }
}
