//! Offline compiler for the closed `mdp.run-request.v1` execution contract.
//!
//! This module owns derivation only. It never reads provider credentials,
//! starts a subprocess, creates a run directory, or performs network I/O.

use crate::artifact_hash::{
    canonical_json_bytes, canonical_json_sha256_for_domain, pack_content_snapshot, sha256_hex,
};
use crate::constants::{DEFAULT_DIR, ROUTED_CONTEXT_CONTRACT};
use crate::model_steps::{CompiledModelStepV1, resolve_model_steps, resolve_selected_model_step};
use crate::models::Manifest;
use crate::pack_io::read_manifest;
use crate::run_contracts::{
    ArtifactAuthority, DriverIdentity, EvidenceProvenance, ExecutionPolicy, JobIdentity,
    LocalArtifactInput, ModelIdentity, RUN_REQUEST_V1, RunMode, RunRequestV1,
};
use crate::run_runtime::{
    compiler_observe_native_identity, compiler_prepare_native_request, compiler_validate_request,
};
use crate::value_contracts::valid_date_time;
use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
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
    manifest
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
        total_bytes = total_bytes.checked_add(bytes.len() as u64).ok_or_else(|| {
            diagnostic("input-too-large", "mdp prepare-run --input <name>=<path>")
        })?;
        if total_bytes > policy.max_input_bytes {
            return Err(diagnostic(
                "input-too-large",
                "mdp prepare-run --input <name>=<path>",
            ));
        }
        let schema = schema_for_input(&input.name);
        let sha = sha256_hex(&bytes);
        let authority = ArtifactAuthority {
            logical_name: input.name.clone(),
            schema_id: schema.0.into(),
            media_type: schema.1.into(),
            byte_count: bytes.len() as u64,
            sha256: sha.clone(),
            provenance: EvidenceProvenance::MdpObserved,
            provenance_refs: vec![format!("model-step:{}", step.step_id)],
        };
        authorities.insert(input.name.clone(), (authority, stable_path));
        input_values.push(json!({"name": input.name, "sha256": sha, "bytes": bytes.len()}));
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
        executable_sha256: Some("0".repeat(64)),
        image_digest: None,
        configuration_sha256: "0".repeat(64),
        dependency_lock_sha256: Some("0".repeat(64)),
        identity_provenance: EvidenceProvenance::MdpObserved,
    };
    let temp_model = ModelIdentity {
        provider: "openai".into(),
        requested_model: options.model.clone(),
        resolved_model: None,
        authorized_endpoint: ENDPOINT.into(),
        parameters_sha256: "0".repeat(64),
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
    Ok(CompiledRunRequest {
        request,
        request_bytes,
        manifest: manifest_value,
        concise,
        diagnostics: Vec::new(),
    })
}

pub(crate) fn write_compiled_request(
    compiled: &CompiledRunRequest,
    options: &PrepareRunOptions,
) -> Result<()> {
    if let Some(path) = &options.out {
        atomic_write(path, &compiled.request_bytes)?;
    }
    if let Some(path) = &options.manifest_out {
        atomic_write(path, &canonical_json_bytes(&compiled.manifest)?)?;
    }
    Ok(())
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

fn read_regular(path: &Path, max: u64, label: &str) -> Result<Vec<u8>> {
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
    let bytes = fs::read(path).map_err(|_| anyhow!("{label} unreadable"))?;
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

fn schema_for_input(name: &str) -> (&'static str, &'static str) {
    match name {
        "routed_context" | "routed-context" => (ROUTED_CONTEXT_CONTRACT, "application/json"),
        "normalized_input" | "normalized-input" => {
            ("mdp.normalized-decision-input.v2", "application/json")
        }
        "source_binding" | "source-binding" => ("mdp.source-binding.v2", "application/json"),
        _ => ("mdp.declared-input.v1", "application/json"),
    }
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

fn diagnostic(code: &str, next: &str) -> anyhow::Error {
    anyhow!("{code}: preparation refused; next safe command: {next}")
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("output path has no parent"))?;
    fs::create_dir_all(parent)?;
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(tmp, path)?;
    Ok(())
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
    use super::{civil_from_days, parse_input_mappings};
    #[test]
    fn mappings_reject_duplicates() {
        assert!(parse_input_mappings(&["a=x".into(), "a=y".into()]).is_err());
    }
    #[test]
    fn civil_epoch_is_stable() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }
}
