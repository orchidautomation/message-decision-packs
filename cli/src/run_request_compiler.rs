//! Offline compiler for the closed `mdp.run-request.v1` execution contract.
//!
//! This module owns derivation only. It never reads provider credentials,
//! starts a subprocess, creates a run directory, or performs network I/O.

use crate::artifact_hash::{
    canonical_json_bytes, canonical_json_sha256_for_domain, pack_content_snapshot, sha256_hex,
};
use crate::cli::SchemaTarget;
use crate::commands::schemas::schema;
use crate::constants::{
    DEFAULT_DIR, REQUIREMENTS_CONTRACT_V2, REQUIREMENTS_MODEL_CONTEXT_CONTRACT_V1,
    ROUTED_CONTEXT_CONTRACT,
};
use crate::model_steps::{
    CompiledModelStepV1, ModelStepPhase, resolve_model_steps, resolve_selected_model_step,
};
use crate::models::{Manifest, ProfileJob};
use crate::pack_io::read_manifest;
use crate::run_contracts::{
    ArtifactAuthority, DriverIdentity, EvidenceProvenance, ExecutionPolicy, JobIdentity,
    LocalArtifactInput, ModelIdentity, RUN_REQUEST_V1, RunMode, RunRequestV1,
};
use crate::run_runtime::{
    compiler_observe_native_identity, compiler_prepare_native_request, compiler_validate_request,
};
use crate::value_contracts::valid_date_time;
use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const RUN_REQUEST_COMPILE_V1: &str = "mdp.run-request-compile.v1";
const MAX_INPUT_BYTES: u64 = 128 * 1024;
const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;
const ENDPOINT: &str = "https://api.openai.com/v1/responses";

#[derive(Clone, Debug)]
pub(crate) struct PrepareRunOptions {
    pub(crate) dir: PathBuf,
    pub(crate) job: String,
    pub(crate) operation: Option<String>,
    pub(crate) inputs: Vec<String>,
    pub(crate) model: String,
    pub(crate) retention_policy: String,
    pub(crate) created_at: Option<String>,
    pub(crate) out: Option<PathBuf>,
    pub(crate) manifest_out: Option<PathBuf>,
    pub(crate) full: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompilerDiagnostic {
    pub(crate) code: String,
    pub(crate) contract: String,
    pub(crate) message: String,
    pub(crate) next_command: String,
}

#[derive(Debug)]
pub(crate) struct CompilerError(pub(crate) CompilerDiagnostic);

impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.message)
    }
}
impl std::error::Error for CompilerError {}

#[derive(Clone, Debug)]
pub(crate) struct CompiledRunRequest {
    pub(crate) request: RunRequestV1,
    pub(crate) request_bytes: Vec<u8>,
    pub(crate) manifest: Value,
    pub(crate) concise: Value,
    pub(crate) diagnostics: Vec<CompilerDiagnostic>,
}

impl CompiledRunRequest {
    pub(crate) fn output(&self, full: bool) -> Value {
        if full {
            let mut output = self.concise.clone();
            if let Some(object) = output.as_object_mut() {
                object.insert(
                    "request".into(),
                    serde_json::to_value(&self.request).unwrap_or(Value::Null),
                );
                object.insert("manifest".into(), self.manifest.clone());
                object.insert(
                    "diagnostics".into(),
                    serde_json::to_value(&self.diagnostics).unwrap_or(Value::Array(Vec::new())),
                );
            }
            output
        } else {
            self.concise.clone()
        }
    }
}

pub(crate) fn compile_native_run_request(
    options: &PrepareRunOptions,
) -> Result<CompiledRunRequest> {
    compile_native_run_request_inner(options).map_err(|error| {
        if error.downcast_ref::<CompilerError>().is_some() {
            error
        } else {
            diagnostic("prepare-run-failed", "mdp prepare-run --help")
        }
    })
}

fn compile_native_run_request_inner(options: &PrepareRunOptions) -> Result<CompiledRunRequest> {
    let root = fs::canonicalize(&options.dir)
        .map_err(|_| diagnostic("pack-invalid", "mdp validate --dir <pack>"))?;
    if options.job.trim().is_empty() || options.model.trim().is_empty() {
        return Err(diagnostic("invalid-options", "mdp prepare-run --help"));
    }
    if !matches!(
        options.retention_policy.as_str(),
        "receipt-only" | "customer-controlled-workdir"
    ) {
        return Err(diagnostic(
            "retention-policy-unsupported",
            "mdp prepare-run --help",
        ));
    }
    let created_at = options.created_at.clone().unwrap_or_else(now_utc);
    if !valid_date_time(&created_at) || !created_at.ends_with('Z') {
        return Err(diagnostic("created-at-invalid", "mdp prepare-run --help"));
    }
    let manifest = read_manifest(&root)
        .map_err(|_| diagnostic("pack-invalid", "mdp validate --dir <pack>"))?;
    let snapshot = pack_content_snapshot(&root)
        .map_err(|_| diagnostic("pack-invalid", "mdp validate --dir <pack>"))?;
    let profile = manifest
        .profile
        .as_ref()
        .map(|p| p.id.as_str())
        .unwrap_or("gtm")
        .to_string();
    let selected_job = manifest
        .jobs
        .iter()
        .find(|candidate| candidate.id == options.job)
        .ok_or_else(|| diagnostic("job-not-found", "mdp requirements --dir <pack> --job <job>"))?;
    let step = select_step(&root, &manifest, &options.job, options.operation.as_deref())?;
    let input_paths = parse_input_mappings(&options.inputs)?;
    let declarations = step
        .declared_inputs
        .iter()
        .filter(|input| !host_metadata(&input.name))
        .collect::<Vec<_>>();
    let declared_names = declarations
        .iter()
        .map(|input| input.name.as_str())
        .collect::<BTreeSet<_>>();
    for name in input_paths.keys() {
        if !declared_names.contains(name.as_str()) {
            return Err(diagnostic(
                "declared-input-extra",
                "mdp requirements --dir <pack> --job <job>",
            ));
        }
    }
    for input in &declarations {
        if input.required && !input_paths.contains_key(&input.name) {
            return Err(diagnostic(
                "declared-input-missing",
                "mdp prepare-run --input <name>=<path>",
            ));
        }
    }
    let policy = ExecutionPolicy {
        environment_allowlist: vec!["OPENAI_API_KEY".into()],
        filesystem_mode: "private-staging".into(),
        tool_mode: "none".into(),
        network_mode: "authorized-endpoints-only".into(),
        authorized_endpoints: vec![ENDPOINT.into()],
        max_input_bytes: MAX_INPUT_BYTES,
        max_output_bytes: 1024 * 1024,
        timeout_ms: 30_000,
        retention_policy: options.retention_policy.clone(),
    };
    let release_tuple = json!({"contract": RUN_REQUEST_V1, "pack_id": manifest.id, "version": manifest.version, "portable_digest": snapshot.sha256});
    let release_hash = canonical_json_sha256_for_domain("mdp.pack-release.v1", &release_tuple)?;
    let pack_release_id = format!("{}-{}", manifest.id, &release_hash[..16]);
    let prompt_path = root.join(DEFAULT_DIR).join(&step.prompt_path);
    let prompt_bytes = read_regular(&prompt_path, MAX_INPUT_BYTES, "prompt")?;
    let prompt_sha = sha256_hex(&prompt_bytes);
    let prompt_authority = ArtifactAuthority {
        logical_name: "prompt".into(),
        schema_id: "mdp.prompt.v1".into(),
        media_type: "text/yaml".into(),
        byte_count: prompt_bytes.len() as u64,
        sha256: prompt_sha.clone(),
        provenance: EvidenceProvenance::MdpObserved,
        provenance_refs: vec![step.prompt_sha256.clone()],
    };
    let mut authorities = BTreeMap::new();
    let mut input_values = Vec::new();
    let mut total_bytes = 0u64;
    for input in declarations {
        let Some(path) = input_paths.get(&input.name) else {
            continue;
        };
        let bytes = read_regular(path, MAX_INPUT_BYTES, "declared input")?;
        let stable_path = fs::canonicalize(path).map_err(|_| {
            diagnostic(
                "declared-input-unreadable",
                "mdp prepare-run --input <name>=<path>",
            )
        })?;
        total_bytes = total_bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| input_budget_diagnostic(u64::MAX, policy.max_input_bytes))?;
        if total_bytes > policy.max_input_bytes {
            return Err(input_budget_diagnostic(total_bytes, policy.max_input_bytes));
        }
        let schema = input_authority(input, &manifest, selected_job)?;
        let sha = sha256_hex(&bytes);
        let logical_name = canonical_lineage_name(&input.name)
            .unwrap_or(input.name.as_str())
            .to_string();
        if authorities.contains_key(&logical_name) {
            return Err(diagnostic(
                "declared-input-duplicate-alias",
                "mdp requirements --dir <pack> --job <job>",
            ));
        }
        let authority = ArtifactAuthority {
            logical_name: logical_name.clone(),
            schema_id: schema.0,
            media_type: schema.1,
            byte_count: bytes.len() as u64,
            sha256: sha.clone(),
            provenance: EvidenceProvenance::MdpObserved,
            provenance_refs: if schema.2.is_empty() {
                vec![format!("model-step:{}", step.step_id)]
            } else {
                schema.2
            },
        };
        authorities.insert(logical_name.clone(), (authority, stable_path));
        input_values.push(json!({"name": logical_name, "sha256": sha, "bytes": bytes.len()}));
    }
    let identity_tuple = json!({
        "contract": RUN_REQUEST_COMPILE_V1,
        "pack": snapshot.sha256,
        "profile": profile,
        "job": options.job,
        "operation": step.step_id,
        "model": options.model,
        "inputs": input_values,
        "policy": policy,
    });
    let idempotency_hash =
        canonical_json_sha256_for_domain("mdp.run-idempotency.v1", &identity_tuple)?;
    let execution_hash = canonical_json_sha256_for_domain("mdp.run-execution.v1", &identity_tuple)?;
    let idempotency_key = format!("mdp-{}", &idempotency_hash[..32]);
    let execution_id = format!("mdp-run-{}", &execution_hash[..32]);
    let temp_driver = DriverIdentity {
        driver_id: "mdp-native-openai".into(),
        implementation: "bundled:mdp-native-model-openai".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        build_sha256: None,
        executable_sha256: None,
        image_digest: None,
        configuration_sha256: String::new(),
        dependency_lock_sha256: None,
        identity_provenance: EvidenceProvenance::CompilerDerived,
    };
    let temp_model = ModelIdentity {
        provider: "openai".into(),
        requested_model: options.model.clone(),
        resolved_model: None,
        authorized_endpoint: ENDPOINT.into(),
        parameters_sha256: String::new(),
        session_behavior: crate::run_contracts::AssuranceEvidenceState::Declared,
        cache_behavior: crate::run_contracts::AssuranceEvidenceState::Declared,
        storage_behavior: crate::run_contracts::AssuranceEvidenceState::Declared,
    };
    let mut request = RunRequestV1 {
        contract: RUN_REQUEST_V1.into(),
        execution_id: execution_id.clone(),
        created_at: created_at.clone(),
        profile: profile.clone(),
        operation: step.step_id.clone(),
        mode: RunMode::Generative,
        job_identity: Some(JobIdentity {
            job_id: options.job.clone(),
            idempotency_key,
        }),
        pack_dir: root.to_string_lossy().into_owned(),
        pack_release_id,
        prompt: Some(LocalArtifactInput {
            logical_name: "prompt".into(),
            source_path: prompt_path.to_string_lossy().into_owned(),
            schema_id: "mdp.prompt.v1".into(),
            media_type: "text/yaml".into(),
            provenance_refs: vec![step.prompt_sha256.clone()],
        }),
        inputs: authorities
            .iter()
            .map(|(name, (authority, path))| LocalArtifactInput {
                logical_name: name.clone(),
                source_path: path.to_string_lossy().into_owned(),
                schema_id: authority.schema_id.clone(),
                media_type: authority.media_type.clone(),
                provenance_refs: authority.provenance_refs.clone(),
            })
            .collect(),
        execution_policy: policy,
        driver: Some(temp_driver),
        model: Some(temp_model),
    };
    let pseudo_inputs = authorities
        .iter()
        .map(|(name, (authority, path))| (name.clone(), authority.clone(), path.clone()))
        .collect();
    let prepared = compiler_prepare_native_request(
        &request,
        &manifest,
        &root,
        prompt_path.clone(),
        prompt_authority,
        pseudo_inputs,
    )
    .map_err(|_| {
        diagnostic(
            "preflight-gate-failed",
            "mdp requirements --dir <pack> --job <job>",
        )
    })?;
    // A normalization invocation has only upstream lineage: the normalized
    // envelope is the output of this step and therefore cannot be supplied to
    // prepare-run. Fit/brief/review invocations still require the complete
    // four-artifact chain when they consume a pre-normalized input.
    validate_governed_lineage(
        &root,
        &options.job,
        &prompt_path,
        &authorities,
        step.phase != ModelStepPhase::Normalization,
        step.phase == ModelStepPhase::Normalization
            && step.output_contract.output_kind.as_deref() == Some("decision-input-normalization"),
    )?;
    let (driver, model, observations, model_facts) =
        compiler_observe_native_identity(&request, &prepared).map_err(|_| {
            diagnostic(
                "identity-observation-unavailable",
                "resolve MDP-231 runtime identity contract",
            )
        })?;
    request.driver = Some(driver.clone());
    request.model = Some(model.clone());
    compiler_validate_request(&request)
        .map_err(|_| diagnostic("request-invalid", "mdp prepare-run --help"))?;
    let request_value = serde_json::to_value(&request)?;
    let request_bytes = canonical_json_bytes(&request_value)?;
    if request_bytes.len() > MAX_REQUEST_BYTES {
        return Err(diagnostic("request-too-large", "mdp prepare-run --help"));
    }
    let request_sha = sha256_hex(&request_bytes);
    let policy_sha = canonical_json_sha256_for_domain(
        "mdp.execution-policy.v1",
        &serde_json::to_value(&request.execution_policy)?,
    )?;
    let manifest_value = json!({
        "contract": RUN_REQUEST_COMPILE_V1,
        "request_contract": RUN_REQUEST_V1,
        "request_sha256": request_sha,
        "created_at": created_at,
        "pack": {"id": manifest.id, "version": manifest.version, "profile": profile, "portable_digest": snapshot.sha256, "files": snapshot.files},
        "job": options.job,
        "operation": step.step_id,
        "prompt": {"id": step.prompt_id, "version": step.prompt_version, "path": step.prompt_path, "sha256": step.prompt_sha256, "bytes": prompt_bytes.len()},
        "inputs": input_values,
        "execution_policy": request.execution_policy,
        "execution_policy_sha256": policy_sha,
        "driver": driver,
        "model": model,
        "identity_observations": observations,
        "model_facts": model_facts,
        "provider_authorization": "required-at-execution",
        "network": "offline-preparation",
        "assurance": ["derived", "observed", "anticipated"],
    });
    let next_command = format!(
        "mdp run --request {} --out-dir <run-dir>",
        options
            .out
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "<request.json>".to_string())
    );
    let concise = json!({"contract": RUN_REQUEST_COMPILE_V1, "status": "ready", "execution_id": execution_id, "job": options.job, "operation": step.step_id, "pack_sha256": snapshot.sha256, "prompt_sha256": step.prompt_sha256, "input_sha256s": input_values.iter().map(|v| json!({"name":v["name"],"sha256":v["sha256"]})).collect::<Vec<_>>(), "driver_configuration_sha256": driver.configuration_sha256, "model_parameters_sha256": model.parameters_sha256, "endpoint": ENDPOINT, "max_input_bytes": MAX_INPUT_BYTES, "max_output_bytes": request.execution_policy.max_output_bytes, "timeout_ms": request.execution_policy.timeout_ms, "data_boundary": "private-staging", "provider_authorization": "required-at-execution", "anticipated_assurance": ["derived", "observed", "anticipated"], "request_sha256": request_sha, "next_command": next_command});
    jsonschema::draft202012::validate(&schema(SchemaTarget::RunRequestCompileV1), &concise)
        .map_err(|_| diagnostic("compiled-output-invalid", "mdp prepare-run --help"))?;
    Ok(CompiledRunRequest {
        request,
        request_bytes,
        manifest: manifest_value,
        concise,
        diagnostics: Vec::new(),
    })
}

fn validate_governed_lineage(
    root: &Path,
    job: &str,
    prompt: &Path,
    authorities: &BTreeMap<String, (ArtifactAuthority, PathBuf)>,
    require_normalized_output: bool,
    require_model_context: bool,
) -> Result<()> {
    let aliases: [(&str, &[&str]); 4] = [
        ("source-binding", &["source-binding", "source_binding"]),
        (
            "source-attempt-request",
            &["source-attempt-request", "source_attempt_request"],
        ),
        (
            "collected-attempt-results",
            &["collected-attempt-results", "collected_attempt_results"],
        ),
        (
            "normalized-decision-input",
            &[
                "normalized-decision-input",
                "normalized_decision_input",
                "normalized-input",
                "normalized_input",
            ],
        ),
    ];
    let mut resolved = BTreeMap::new();
    for (canonical, names) in aliases {
        let matches = authorities
            .iter()
            .filter(|(name, _)| names.iter().any(|alias| name == alias))
            .collect::<Vec<_>>();
        if matches.len() > 1 {
            return Err(diagnostic(
                "governed-lineage-duplicate-alias",
                "mdp requirements --dir <pack> --job <job>",
            ));
        }
        if let Some((_, (_, path))) = matches.into_iter().next() {
            let bytes = read_regular(path, MAX_INPUT_BYTES, "governed lineage artifact")?;
            let sha = sha256_hex(&bytes);
            let value = serde_json::from_slice(&bytes).map_err(|_| {
                diagnostic(
                    "governed-lineage-invalid",
                    "mdp requirements --dir <pack> --job <job>",
                )
            })?;
            resolved.insert(canonical, (value, path.to_string_lossy().into_owned(), sha));
        }
    }
    let read_named =
        |name: &str| -> Option<(Value, String, String)> { resolved.get(name).cloned() };
    let requirements_context = authorities
        .iter()
        .filter(|(name, _)| {
            matches!(
                name.as_str(),
                "decision-input-requirements" | "decision_input_requirements"
            )
        })
        .map(|(_, (_, path))| {
            let bytes = read_regular(path, MAX_INPUT_BYTES, "requirements model context")?;
            let sha = sha256_hex(&bytes);
            let value = serde_json::from_slice(&bytes).map_err(|_| {
                diagnostic(
                    "governed-lineage-invalid",
                    "mdp requirements --dir <pack> --job <job>",
                )
            })?;
            Ok::<_, anyhow::Error>((value, path.to_string_lossy().into_owned(), sha))
        })
        .collect::<Result<Vec<_>>>()?;
    if requirements_context.len() > 1 {
        return Err(diagnostic(
            "governed-lineage-duplicate-alias",
            "mdp requirements --dir <pack> --job <job>",
        ));
    }
    if require_model_context && requirements_context.is_empty() {
        return Err(diagnostic(
            "governed-lineage-incomplete",
            "mdp requirements --dir <pack> --job <job>",
        ));
    }
    if let Some((context, context_path, _)) = requirements_context.into_iter().next() {
        let context_schema = schema(SchemaTarget::RequirementsModelContextV1);
        if jsonschema::draft202012::validate(&context_schema, &context).is_err() {
            return Err(diagnostic(
                "governed-lineage-invalid",
                "mdp requirements --dir <pack> --job <job>",
            ));
        }
        let expected = crate::commands::requirements::requirements_model_context(root, job)
            .map_err(|_| {
                diagnostic(
                    "governed-lineage-invalid",
                    "mdp requirements --dir <pack> --job <job>",
                )
            })?;
        let context_bytes = canonical_json_bytes(&context).map_err(|_| {
            diagnostic(
                "governed-lineage-invalid",
                "mdp requirements --dir <pack> --job <job>",
            )
        })?;
        let expected_bytes = canonical_json_bytes(&expected).map_err(|_| {
            diagnostic(
                "governed-lineage-invalid",
                "mdp requirements --dir <pack> --job <job>",
            )
        })?;
        if context_bytes != expected_bytes
            || context["contract"] != REQUIREMENTS_MODEL_CONTEXT_CONTRACT_V1
            || context["source_contract"] != REQUIREMENTS_CONTRACT_V2
        {
            return Err(diagnostic(
                "governed-lineage-invalid",
                "mdp requirements --dir <pack> --job <job>",
            ));
        }
        let _ = context_path;
    }
    let binding = read_named("source-binding");
    let request = read_named("source-attempt-request");
    let results = read_named("collected-attempt-results");
    let normalized = read_named("normalized-decision-input");
    let present = [&binding, &request, &results, &normalized]
        .into_iter()
        .filter(|item| item.is_some())
        .count();
    if present == 0 {
        return Ok(());
    }
    let Some((binding_value, binding_path, binding_sha)) = binding else {
        return Err(diagnostic(
            "governed-lineage-incomplete",
            "mdp requirements --dir <pack> --job <job>",
        ));
    };
    if request.is_none() || results.is_none() || (require_normalized_output && normalized.is_none())
    {
        return Err(diagnostic(
            "governed-lineage-incomplete",
            "mdp requirements --dir <pack> --job <job>",
        ));
    }
    // Establish ownership before any normalized payload is interpreted.  In
    // particular, payload aliases must never be used to guess a profile.
    if normalized.is_some() {
        let manifest = crate::pack_io::read_manifest(root).map_err(|_| {
            diagnostic(
                "governed-lineage-invalid",
                "mdp requirements --dir <pack> --job <job>",
            )
        })?;
        let job_definition = manifest
            .jobs
            .iter()
            .find(|candidate| candidate.id == job)
            .ok_or_else(|| {
                diagnostic(
                    "governed-lineage-invalid",
                    "mdp requirements --dir <pack> --job <job>",
                )
            })?;
        crate::decision_input::select_adapter_for_job(&manifest, job_definition).map_err(|_| {
            diagnostic(
                "governed-lineage-invalid",
                "mdp requirements --dir <pack> --job <job>",
            )
        })?;
    }
    let compiled = crate::commands::requirements::requirements(root, job).map_err(|_| {
        diagnostic(
            "governed-lineage-invalid",
            "mdp requirements --dir <pack> --job <job>",
        )
    })?;
    let result = crate::commands::source_binding::validate_source_binding_v2(
        &compiled,
        &binding_value,
        &binding_path,
    )
    .map_err(|_| {
        diagnostic(
            "governed-lineage-invalid",
            "mdp requirements --dir <pack> --job <job>",
        )
    })?;
    if result["valid"] != true {
        return Err(diagnostic(
            "governed-lineage-invalid",
            "mdp requirements --dir <pack> --job <job>",
        ));
    }
    for (artifact, schema_key) in [
        (request.as_ref().unwrap(), "source_attempt_request_schema"),
        (
            results.as_ref().unwrap(),
            "collected_attempt_results_schema",
        ),
    ] {
        if jsonschema::draft202012::validate(&compiled[schema_key], &artifact.0).is_err() {
            return Err(diagnostic(
                "governed-lineage-invalid",
                "mdp requirements --dir <pack> --job <job>",
            ));
        }
        if artifact.0["source_binding_sha256"].as_str() != Some(binding_sha.as_str()) {
            return Err(diagnostic(
                "governed-lineage-invalid",
                "mdp requirements --dir <pack> --job <job>",
            ));
        }
    }
    if let Some((normalized_value, normalized_path, _)) = normalized {
        let request_ref = request
            .as_ref()
            .map(|(value, path, sha)| (value, path.as_str(), sha.as_str()));
        let results_ref = results
            .as_ref()
            .map(|(value, path, sha)| (value, path.as_str(), sha.as_str()));
        let validation =
            crate::commands::requirements::validate_normalized_decision_input_with_projection(
                root,
                &normalized_value,
                &normalized_path,
                prompt,
                Some((&binding_value, &binding_path, &binding_sha)),
                request_ref,
                results_ref,
                None,
            )
            .map_err(|_| {
                diagnostic(
                    "governed-lineage-invalid",
                    "mdp requirements --dir <pack> --job <job>",
                )
            })?;
        if !validation.issues.is_empty() {
            return Err(diagnostic(
                "governed-lineage-invalid",
                "mdp requirements --dir <pack> --job <job>",
            ));
        }
    }
    Ok(())
}

pub(crate) fn write_compiled_request(
    compiled: &CompiledRunRequest,
    options: &PrepareRunOptions,
) -> Result<()> {
    write_compiled_request_inner(compiled, options).map_err(|error| {
        if error.downcast_ref::<CompilerError>().is_some() {
            error
        } else {
            diagnostic("output-transaction-failed", "mdp prepare-run --help")
        }
    })
}

fn write_compiled_request_inner(
    compiled: &CompiledRunRequest,
    options: &PrepareRunOptions,
) -> Result<()> {
    let manifest_bytes = canonical_json_bytes(&compiled.manifest)?;
    let targets: Vec<(&Path, &[u8])> = [
        options
            .out
            .as_deref()
            .map(|p| (p, compiled.request_bytes.as_slice())),
        options
            .manifest_out
            .as_deref()
            .map(|p| (p, manifest_bytes.as_slice())),
    ]
    .into_iter()
    .flatten()
    .collect();
    if targets.len() == 2 && output_alias(targets[0].0, targets[1].0) {
        return Err(diagnostic(
            "output-path-collision",
            "mdp prepare-run --help",
        ));
    }
    let mut transaction = OutputTransactionGuard::default();
    let mut staged = Vec::new();
    for (index, (path, bytes)) in targets.iter().enumerate() {
        if fs::symlink_metadata(path).is_ok() {
            return Err(diagnostic("output-path-exists", "mdp prepare-run --help"));
        }
        let parent = path
            .parent()
            .ok_or_else(|| diagnostic("output-path-invalid", "mdp prepare-run --help"))?;
        ensure_parent_dirs(parent, &mut transaction)?;
        let tmp = path.with_extension(format!("{}.{}.tmp", std::process::id(), index));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|_| diagnostic("output-path-unwritable", "mdp prepare-run --help"))?;
        let staged_identity = match file_identity(&tmp) {
            Ok(identity) => identity,
            Err(_) => {
                let _ = fs::remove_file(&tmp);
                return Err(diagnostic(
                    "output-path-unwritable",
                    "mdp prepare-run --help",
                ));
            }
        };
        transaction.staged.push(OwnedEntry {
            path: tmp.clone(),
            identity: staged_identity,
        });
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| diagnostic("output-path-unwritable", "mdp prepare-run --help"))?;
        staged.push((tmp, (*path).to_path_buf()));
    }
    for (tmp, path) in &staged {
        let identity = file_identity(tmp)
            .map_err(|_| diagnostic("output-transaction-failed", "mdp prepare-run --help"))?;
        if fs::hard_link(tmp, path).is_err() {
            return Err(diagnostic(
                "output-transaction-failed",
                "mdp prepare-run --help",
            ));
        }
        // Register immediately after the no-replace install succeeds.  If
        // removing the staged inode fails, Drop still removes this owned
        // destination and every remaining staged file.
        transaction.installed.push(OwnedEntry {
            path: path.clone(),
            identity,
        });
        fs::remove_file(tmp)
            .map_err(|_| diagnostic("output-transaction-failed", "mdp prepare-run --help"))?;
    }
    transaction.committed = true;
    Ok(())
}

#[derive(Default)]
struct OutputTransactionGuard {
    staged: Vec<OwnedEntry>,
    installed: Vec<OwnedEntry>,
    created_dirs: Vec<OwnedDirectory>,
    committed: bool,
}

impl Drop for OutputTransactionGuard {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for entry in self.staged.iter().rev() {
            remove_owned_file(entry);
        }
        for entry in self.installed.iter().rev() {
            remove_owned_file(entry);
        }
        for directory in self.created_dirs.iter().rev() {
            remove_owned_directory(directory);
        }
    }
}

#[derive(Clone)]
struct OwnedEntry {
    path: PathBuf,
    identity: FileIdentity,
}

#[derive(Clone)]
struct OwnedDirectory {
    path: PathBuf,
    identity: FileIdentity,
}

#[cfg(unix)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(not(unix))]
#[derive(Clone, PartialEq, Eq)]
struct FileIdentity {
    length: u64,
    modified: Option<SystemTime>,
}

fn file_identity(path: &Path) -> std::io::Result<FileIdentity> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(unix)]
    {
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
    {
        Ok(FileIdentity {
            length: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
}

fn identity_matches(path: &Path, expected: &FileIdentity) -> bool {
    file_identity(path)
        .map(|actual| actual == *expected)
        .unwrap_or(false)
}

fn remove_owned_file(entry: &OwnedEntry) {
    if identity_matches(&entry.path, &entry.identity) {
        let _ = fs::remove_file(&entry.path);
    }
}

fn remove_owned_directory(directory: &OwnedDirectory) {
    if identity_matches(&directory.path, &directory.identity)
        && fs::read_dir(&directory.path)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false)
    {
        let _ = fs::remove_dir(&directory.path);
    }
}

fn ensure_parent_dirs(parent: &Path, transaction: &mut OutputTransactionGuard) -> Result<()> {
    let mut missing = Vec::new();
    let mut cursor = parent.to_path_buf();
    while fs::symlink_metadata(&cursor).is_err() {
        missing.push(cursor.clone());
        let Some(next) = cursor.parent() else {
            return Err(diagnostic("output-path-invalid", "mdp prepare-run --help"));
        };
        cursor = next.to_path_buf();
    }
    if !fs::symlink_metadata(&cursor)
        .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(diagnostic(
            "output-path-unwritable",
            "mdp prepare-run --help",
        ));
    }
    for directory in missing.into_iter().rev() {
        fs::create_dir(&directory)
            .map_err(|_| diagnostic("output-path-unwritable", "mdp prepare-run --help"))?;
        let identity = match file_identity(&directory) {
            Ok(identity) => identity,
            Err(_) => {
                let _ = fs::remove_dir(&directory);
                return Err(diagnostic(
                    "output-path-unwritable",
                    "mdp prepare-run --help",
                ));
            }
        };
        transaction.created_dirs.push(OwnedDirectory {
            path: directory,
            identity,
        });
    }
    Ok(())
}

fn output_alias(a: &Path, b: &Path) -> bool {
    let canonical_target = |path: &Path| {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::canonicalize(parent)
            .unwrap_or_else(|_| parent.to_path_buf())
            .join(path.file_name().unwrap_or_default())
    };
    let aa = canonical_target(a);
    let bb = canonical_target(b);
    aa == bb
}

fn select_step(
    root: &Path,
    manifest: &Manifest,
    job: &str,
    operation: Option<&str>,
) -> Result<CompiledModelStepV1> {
    if let Some(operation) = operation {
        return resolve_selected_model_step(root, manifest, job, operation).map_err(|_| {
            diagnostic(
                "model-step-not-found",
                "mdp requirements --dir <pack> --job <job>",
            )
        });
    }
    let job_value = manifest
        .jobs
        .iter()
        .find(|candidate| candidate.id == job)
        .ok_or_else(|| diagnostic("job-not-found", "mdp requirements --dir <pack> --job <job>"))?;
    let resolution = resolve_model_steps(root, manifest, job_value).map_err(|_| {
        diagnostic(
            "model-step-invalid",
            "mdp requirements --dir <pack> --job <job>",
        )
    })?;
    match resolution.steps.as_slice() {
        [step] => Ok(step.clone()),
        [] => Err(diagnostic(
            "model-step-missing",
            "mdp requirements --dir <pack> --job <job>",
        )),
        _ => Err(diagnostic(
            "model-step-ambiguous",
            "mdp requirements --dir <pack> --job <job>",
        )),
    }
}

fn parse_input_mappings(values: &[String]) -> Result<BTreeMap<String, PathBuf>> {
    let mut result = BTreeMap::new();
    for value in values {
        let (name, path) = value.split_once('=').ok_or_else(|| {
            diagnostic(
                "input-mapping-invalid",
                "mdp prepare-run --input <name>=<path>",
            )
        })?;
        if name.is_empty()
            || path.is_empty()
            || result
                .insert(name.to_string(), PathBuf::from(path))
                .is_some()
        {
            return Err(diagnostic(
                "input-mapping-duplicate",
                "mdp prepare-run --input <name>=<path>",
            ));
        }
    }
    Ok(result)
}

fn read_regular(path: &Path, max: u64, _label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        diagnostic(
            "declared-input-unreadable",
            "mdp prepare-run --input <name>=<path>",
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(diagnostic(
            "declared-input-unsafe",
            "mdp prepare-run --input <name>=<path>",
        ));
    }
    #[cfg(unix)]
    if metadata.nlink() > 1 {
        return Err(diagnostic(
            "declared-input-unsafe",
            "mdp prepare-run --input <name>=<path>",
        ));
    }
    let bytes = fs::read(path).map_err(|_| {
        diagnostic(
            "declared-input-unreadable",
            "mdp prepare-run --input <name>=<path>",
        )
    })?;
    if bytes.len() as u64 > max {
        return Err(diagnostic(
            "declared-input-too-large",
            "mdp prepare-run --input <name>=<path>",
        ));
    }
    let after = fs::symlink_metadata(path).map_err(|_| {
        diagnostic(
            "declared-input-changed",
            "mdp prepare-run --input <name>=<path>",
        )
    })?;
    #[cfg(unix)]
    let identity_changed = after.dev() != metadata.dev()
        || after.ino() != metadata.ino()
        || after.nlink() != metadata.nlink();
    #[cfg(not(unix))]
    let identity_changed = false;
    if identity_changed || after.len() != bytes.len() as u64 || after.file_type().is_symlink() {
        return Err(diagnostic(
            "declared-input-changed",
            "mdp prepare-run --input <name>=<path>",
        ));
    }
    Ok(bytes)
}

fn input_authority(
    input: &crate::models::PromptInput,
    manifest: &Manifest,
    job: &ProfileJob,
) -> Result<(String, String, Vec<String>)> {
    if let Some(schema) = &input.schema_ref {
        return Ok((
            schema.clone(),
            input
                .media_type
                .clone()
                .unwrap_or_else(|| "application/json".into()),
            input.provenance_refs.clone(),
        ));
    }
    if input.name == "routed_context" {
        return Ok((
            ROUTED_CONTEXT_CONTRACT.into(),
            "application/json".into(),
            vec![format!("job:{}", job.id)],
        ));
    }
    let selected_input_ids = job.input_contracts.iter().cloned().collect::<BTreeSet<_>>();
    if let Some(contract) = manifest.input_contracts.iter().find(|c| {
        selected_input_ids.contains(&c.id)
            && (c.id == input.name || c.prompt.as_deref() == Some(&input.name))
    }) {
        let schema_ref = contract.schema_ref.as_ref().ok_or_else(|| {
            diagnostic(
                "declared-input-authority-missing",
                "mdp requirements --dir <pack> --job <job>",
            )
        })?;
        return Ok((
            schema_ref.clone(),
            input
                .media_type
                .clone()
                .unwrap_or_else(|| "application/json".into()),
            vec![format!("input-contract:{}", contract.id)],
        ));
    }
    let mut selected_decision_ids = job
        .decision_input_contracts
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for input_id in &selected_input_ids {
        if let Some(contract) = manifest.input_contracts.iter().find(|c| c.id == *input_id) {
            selected_decision_ids.extend(contract.decision_input_contracts.iter().cloned());
        }
    }
    if let Some(contract) = manifest
        .decision_input_contracts
        .iter()
        .find(|c| selected_decision_ids.contains(&c.id) && c.id == input.name)
    {
        return Ok((
            contract.normalization.normalized_schema_ref.clone(),
            input
                .media_type
                .clone()
                .unwrap_or_else(|| "application/json".into()),
            vec![format!(
                "decision-input-contract:{}:{}",
                contract.id, contract.version
            )],
        ));
    }
    Err(diagnostic(
        "declared-input-authority-missing",
        "mdp requirements --dir <pack> --job <job>",
    ))
}

fn host_metadata(name: &str) -> bool {
    matches!(
        name,
        "prompt_receipt"
            | "prompt-receipt"
            | "invocation_receipt_sha256"
            | "invocation-receipt-sha256"
    )
}

fn canonical_lineage_name(name: &str) -> Option<&'static str> {
    match name {
        "source-binding" | "source_binding" => Some("source-binding"),
        "source-attempt-request" | "source_attempt_request" => Some("source-attempt-request"),
        "collected-attempt-results" | "collected_attempt_results" => {
            Some("collected-attempt-results")
        }
        "normalized-decision-input"
        | "normalized_decision_input"
        | "normalized-input"
        | "normalized_input" => Some("normalized-decision-input"),
        _ => None,
    }
}

fn diagnostic(code: &str, next: &str) -> anyhow::Error {
    anyhow::Error::new(CompilerError(CompilerDiagnostic {
        code: code.into(),
        contract: RUN_REQUEST_COMPILE_V1.into(),
        message: format!("{code}: preparation refused"),
        next_command: next.into(),
    }))
}

fn input_budget_diagnostic(total: u64, max: u64) -> anyhow::Error {
    anyhow::Error::new(CompilerError(CompilerDiagnostic {
        code: "input-too-large".into(),
        contract: RUN_REQUEST_COMPILE_V1.into(),
        message: format!(
            "input-too-large: aggregate declared input bytes {total} exceed budget {max}"
        ),
        next_command: "mdp prepare-run --input <name>=<path>".into(),
    }))
}

fn now_utc() -> String {
    // A valid, second-precision UTC timestamp without introducing a time crate.
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = seconds / 86_400;
    let rem = seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}

#[cfg(test)]
mod tests {
    use super::{
        civil_from_days, diagnostic, input_authority, output_alias, parse_input_mappings,
        validate_governed_lineage,
    };
    use crate::models::{Manifest, ProfileJob, PromptInput};
    use crate::run_contracts::{ArtifactAuthority, EvidenceProvenance};
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    #[test]
    fn mappings_reject_duplicates() {
        assert!(parse_input_mappings(&["a=x".into(), "a=y".into()]).is_err());
    }
    #[test]
    fn civil_epoch_is_stable() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }
    #[test]
    fn compiler_diagnostic_is_structured_and_bounded() {
        let error = diagnostic("lineage-stale", "mdp requirements --dir <pack> --job <job>");
        let failure = error.downcast_ref::<super::CompilerError>().unwrap();
        assert_eq!(failure.0.contract, super::RUN_REQUEST_COMPILE_V1);
        assert!(!failure.0.next_command.is_empty());
    }
    #[test]
    fn emitted_concise_shape_is_accepted_by_closed_schema() {
        let value = json!({
            "contract": super::RUN_REQUEST_COMPILE_V1, "status":"ready", "execution_id":"mdp-run-test", "job":"job", "operation":"step",
            "pack_sha256":"a".repeat(64), "prompt_sha256":"b".repeat(64), "input_sha256s":[], "driver_configuration_sha256":"c".repeat(64), "model_parameters_sha256":"d".repeat(64),
            "endpoint":"https://api.openai.com/v1/responses", "max_input_bytes":131072, "max_output_bytes":1048576, "timeout_ms":30000,
            "data_boundary":"private-staging", "provider_authorization":"required-at-execution", "anticipated_assurance":["derived","observed","anticipated"], "request_sha256":"e".repeat(64), "next_command":"mdp run --request request.json"
        });
        assert!(
            jsonschema::draft202012::validate(
                &crate::commands::schemas::schema(crate::cli::SchemaTarget::RunRequestCompileV1),
                &value
            )
            .is_ok()
        );
    }
    #[test]
    fn blocked_compile_shape_is_accepted_by_closed_schema() {
        let value = json!({
            "contract": super::RUN_REQUEST_COMPILE_V1,
            "status": "blocked",
            "diagnostics": [{"code":"governed-lineage-incomplete","contract":super::RUN_REQUEST_COMPILE_V1,"message":"blocked","next_command":"mdp requirements --dir <pack> --job <job>"}],
            "next_command": "mdp requirements --dir <pack> --job <job>"
        });
        assert!(
            jsonschema::draft202012::validate(
                &crate::commands::schemas::schema(crate::cli::SchemaTarget::RunRequestCompileV1),
                &value
            )
            .is_ok()
        );
    }
    #[test]
    fn custom_declared_input_authority_wins_over_name_heuristics() {
        let input = PromptInput {
            name: "raw_row".into(),
            description: String::new(),
            required: true,
            default: String::new(),
            missing_behavior: String::new(),
            producer: None,
            schema_ref: Some("custom.schema.v9".into()),
            media_type: Some("application/x-custom".into()),
            provenance_refs: vec!["contract:custom".into()],
        };
        let manifest = serde_json::from_value::<Manifest>(json!({"format":"mdp.pack.v1","id":"x","name":"x","version":"1","description":null,"personas":[],"jobs":[],"cards":[],"policy":{"progressive_disclosure":false,"load_manifest_first":true,"max_cards_per_route":1,"json_contract":"x","no_auth_required":true},"provenance":{"owner":"x","created_by":"x","notes":[]}})).unwrap();
        let authority = input_authority(
            &input,
            &manifest,
            &ProfileJob {
                id: "job".into(),
                ..ProfileJob::default()
            },
        )
        .unwrap();
        assert_eq!(
            authority,
            (
                "custom.schema.v9".into(),
                "application/x-custom".into(),
                vec!["contract:custom".into()]
            )
        );
    }
    #[test]
    fn output_alias_rejects_same_path_before_write() {
        assert!(output_alias(
            std::path::Path::new("/tmp/request.json"),
            std::path::Path::new("/tmp/./request.json")
        ));
    }

    #[test]
    fn output_transaction_guard_removes_only_owned_staged_and_installed_entries() {
        let root = std::env::temp_dir().join(format!(
            "mdp-prepare-transaction-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let owned_dir = root.join("new");
        let sibling = root.join("sibling.txt");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&sibling, b"keep").unwrap();
        std::fs::create_dir(&owned_dir).unwrap();
        let staged = owned_dir.join("request.tmp");
        let installed = owned_dir.join("request.json");
        std::fs::write(&staged, b"request").unwrap();
        std::fs::write(&installed, b"request").unwrap();
        {
            let mut guard = super::OutputTransactionGuard::default();
            guard.staged.push(super::OwnedEntry {
                path: staged.clone(),
                identity: super::file_identity(&staged).unwrap(),
            });
            guard.installed.push(super::OwnedEntry {
                path: installed.clone(),
                identity: super::file_identity(&installed).unwrap(),
            });
            guard.created_dirs.push(super::OwnedDirectory {
                path: owned_dir.clone(),
                identity: super::file_identity(&owned_dir).unwrap(),
            });
        }
        assert!(!staged.exists());
        assert!(!installed.exists());
        assert!(sibling.exists());
        assert!(!owned_dir.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn output_transaction_guard_preserves_replacements_after_rollback() {
        let root = std::env::temp_dir().join(format!(
            "mdp-prepare-transaction-race-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let owned_dir = root.join("new");
        let installed = owned_dir.join("request.json");
        std::fs::create_dir_all(&owned_dir).unwrap();
        std::fs::write(&installed, b"transaction-owned").unwrap();
        let mut guard = super::OutputTransactionGuard::default();
        guard.installed.push(super::OwnedEntry {
            path: installed.clone(),
            identity: super::file_identity(&installed).unwrap(),
        });
        guard.created_dirs.push(super::OwnedDirectory {
            path: owned_dir.clone(),
            identity: super::file_identity(&owned_dir).unwrap(),
        });

        let retained_file = root.join("retained-request.json");
        std::fs::rename(&installed, &retained_file).unwrap();
        std::fs::write(&installed, b"replacement").unwrap();
        let retained_dir = root.join("retained-directory");
        std::fs::rename(&owned_dir, &retained_dir).unwrap();
        std::fs::create_dir(&owned_dir).unwrap();
        std::fs::write(&installed, b"replacement").unwrap();
        std::fs::write(owned_dir.join("operator-owned.txt"), b"keep").unwrap();
        drop(guard);

        assert_eq!(std::fs::read(&installed).unwrap(), b"replacement");
        assert!(owned_dir.join("operator-owned.txt").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn undeclared_input_cannot_use_unrelated_manifest_contract() {
        let input = PromptInput {
            name: "unrelated".into(),
            description: String::new(),
            required: true,
            default: String::new(),
            missing_behavior: String::new(),
            producer: None,
            schema_ref: None,
            media_type: None,
            provenance_refs: Vec::new(),
        };
        let manifest = serde_json::from_value::<Manifest>(json!({
            "format":"mdp.pack.v1","id":"x","name":"x","version":"1",
            "description":null,"personas":[],"jobs":[],"cards":[],
            "input_contracts":[{"id":"other","schema_ref":"other.schema.v1"}],
            "policy":{"progressive_disclosure":false,"load_manifest_first":true,"max_cards_per_route":1,"json_contract":"x","no_auth_required":true},
            "provenance":{"owner":"x","created_by":"x","notes":[]}
        })).unwrap();
        let job = ProfileJob {
            id: "job".into(),
            ..ProfileJob::default()
        };
        assert!(input_authority(&input, &manifest, &job).is_err());
    }

    #[test]
    fn lineage_rejects_duplicate_explicit_aliases() {
        let root = std::env::temp_dir().join(format!(
            "mdp-lineage-alias-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let left = root.join("left.json");
        let right = root.join("right.json");
        std::fs::write(&left, b"{}\n").unwrap();
        std::fs::write(&right, b"{}\n").unwrap();
        let authority = |name: &str| {
            (
                name.to_string(),
                (
                    ArtifactAuthority {
                        logical_name: name.to_string(),
                        schema_id: "mdp.source-binding.v2".into(),
                        media_type: "application/json".into(),
                        byte_count: 3,
                        sha256: "a".repeat(64),
                        provenance: EvidenceProvenance::MdpObserved,
                        provenance_refs: Vec::new(),
                    },
                    if name == "source-binding" {
                        left.clone()
                    } else {
                        right.clone()
                    },
                ),
            )
        };
        let authorities: BTreeMap<String, (ArtifactAuthority, PathBuf)> =
            [authority("source-binding"), authority("source_binding")]
                .into_iter()
                .collect();
        assert!(validate_governed_lineage(&root, "job", &root, &authorities, true, false).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn normalization_lineage_allows_upstream_chain_without_output() {
        let root = std::env::temp_dir().join(format!(
            "mdp-lineage-normalization-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let binding = root.join("binding.json");
        let request = root.join("request.json");
        let results = root.join("results.json");
        for path in [&binding, &request, &results] {
            std::fs::write(path, b"{}\n").unwrap();
        }
        let authority = |name: &str, path: &PathBuf| {
            (
                name.to_string(),
                (
                    ArtifactAuthority {
                        logical_name: name.to_string(),
                        schema_id: match name {
                            "source-binding" => "mdp.source-binding.v2",
                            "source-attempt-request" => "mdp.source-attempt-request.v2",
                            _ => "mdp.collected-attempt-results.v2",
                        }
                        .into(),
                        media_type: "application/json".into(),
                        byte_count: 3,
                        sha256: "a".repeat(64),
                        provenance: EvidenceProvenance::MdpObserved,
                        provenance_refs: Vec::new(),
                    },
                    path.clone(),
                ),
            )
        };
        let authorities: BTreeMap<String, (ArtifactAuthority, PathBuf)> = [
            authority("source-binding", &binding),
            authority("source-attempt-request", &request),
            authority("collected-attempt-results", &results),
        ]
        .into_iter()
        .collect();
        // The fixture bytes are intentionally not a complete source binding;
        // this test isolates the compiler's phase rule. A pre-normalization
        // compile must not report the missing downstream output artifact;
        // a consumer of a normalized input still must.
        let normalization =
            validate_governed_lineage(&root, "job", &root, &authorities, false, false).unwrap_err();
        assert_eq!(
            normalization
                .downcast_ref::<super::CompilerError>()
                .map(|error| error.0.code.as_str()),
            Some("governed-lineage-invalid")
        );
        let downstream =
            validate_governed_lineage(&root, "job", &root, &authorities, true, false).unwrap_err();
        assert_eq!(
            downstream
                .downcast_ref::<super::CompilerError>()
                .map(|error| error.0.code.as_str()),
            Some("governed-lineage-incomplete")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
