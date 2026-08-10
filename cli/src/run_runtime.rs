use crate::artifact_hash::{canonical_json_sha256_for_domain, pack_content_snapshot, sha256_hex};
use crate::commands::prompt_output::{
    validate_prompt_output_file_with_inputs, validate_prompt_output_file_with_lineage_inputs,
};
use crate::commands::routing::{fit, fit_normalized};
use crate::pack_io::{read_manifest, resolve_pack_path};
use crate::run_contracts::{
    ArtifactAuthority, AssuranceDimension, AssuranceEvidenceState, DecisionAuthority,
    EvidenceProvenance, PackAuthority, RUN_BUNDLE_V1, RUN_RECEIPT_V1, RUN_REQUEST_V1,
    RUNNER_AUDIT_V1, RunBundleV1, RunMode, RunReceiptV1, RunRequestV1, RunnerAuditV1,
    TerminalState,
};
use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PROPOSAL_PROFILE: &str = "proposal";
const VALIDATE_EXISTING_OUTPUT: &str = "validate-existing-output";
const GTM_PROFILE: &str = "gtm";
const QUALIFY: &str = "qualify";
const GENERATED_PACK_DIRECTORIES: &[&str] = &["briefs", "traces"];
const MAX_PACK_FILES: usize = 10_000;
const MAX_PACK_BYTES: u64 = 100 * 1024 * 1024;
const MAX_EXECUTION_ID_BYTES: usize = 128;
const MAX_OUTPUT_LEAF_BYTES: usize = 120;
const MAX_RECOVERY_CLAIM_BYTES: usize = 512;

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunExecution {
    pub(crate) contract: String,
    pub(crate) valid: bool,
    pub(crate) execution_id: String,
    pub(crate) terminal_state: TerminalState,
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
    validate_request(request)
        .map_err(|_| run_failure(RunFailureKind::Preflight, "request-policy-invalid"))?;
    let deadline = RunDeadline::new(request.execution_policy.timeout_ms);
    let final_dir = output_root;
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

    let (bundle_sha256, receipt) =
        match execute_transaction(request, &transaction_dir, &deadline, before_post_check) {
            Ok(outcome) => outcome,
            Err(error) => {
                cleanup_failed_transaction(&transaction_dir)?;
                return Err(classify_execution_error(error));
            }
        };
    if let Err(error) = deadline.check() {
        cleanup_failed_transaction(&transaction_dir)?;
        return Err(error);
    }
    fs::remove_dir_all(transaction_dir.join("private"))
        .map_err(|_| run_failure(RunFailureKind::RunnerFailed, "private-cleanup-failed"))?;
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
    Ok(RunExecution {
        contract: "mdp.run-execution.v1".into(),
        valid: receipt.terminal_state.is_success(),
        execution_id: request.execution_id.clone(),
        terminal_state: receipt.terminal_state,
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

fn execute_transaction<F>(
    request: &RunRequestV1,
    transaction_dir: &Path,
    deadline: &RunDeadline,
    before_post_check: F,
) -> Result<(String, RunReceiptV1)>
where
    F: FnOnce() -> Result<()>,
{
    let private_dir = transaction_dir.join("private");
    let staged_pack = private_dir.join("pack");
    let staged_inputs = private_dir.join("inputs");
    let artifacts_dir = transaction_dir.join("artifacts");
    for directory in [&private_dir, &staged_pack, &staged_inputs, &artifacts_dir] {
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
    verify_sources_unchanged(&staged)?;
    if pack_content_snapshot(source_pack)? != source_snapshot {
        return Err(anyhow!("pack changed while declared inputs were staged"));
    }

    let policy_hash = canonical_json_sha256_for_domain(
        "mdp.execution-policy.v1",
        &serde_json::to_value(&request.execution_policy)?,
    )?;
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
            pack_id: manifest.id,
            version: manifest.version,
            profile_id: profile_id.to_string(),
            portable_digest: staged_snapshot.sha256.clone(),
            files: staged_snapshot.files.clone(),
        },
        prompt: None,
        inputs: staged.iter().map(|input| input.authority.clone()).collect(),
        execution_policy_sha256: policy_hash,
        driver: None,
        model: None,
    };
    let bundle_value = serde_json::to_value(&bundle)?;
    let bundle_sha256 = canonical_json_sha256_for_domain(RUN_BUNDLE_V1, &bundle_value)?;
    write_json_create_new(&transaction_dir.join("run-bundle.json"), &bundle)?;

    let mut validation = None;
    let (mut terminal_state, mut success_values) = if request.profile == PROPOSAL_PROFILE
        && request.operation == VALIDATE_EXISTING_OUTPUT
    {
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
                "mdp.normalized-decision-input.v1" | "mdp.normalized-decision-input.v2"
            )
        {
            return Err(anyhow!("declared input schema or media type mismatch"));
        }
        let signal_aware = normalized.authority.schema_id == "mdp.normalized-decision-input.v2";
        let source_attempt = required_typed_input(
            &staged,
            "source-attempt-request",
            "mdp.source-attempt-request.v1",
            "application/json",
        )?;
        let attempt_results = required_typed_input(
            &staged,
            "collected-attempt-results",
            "mdp.collected-attempt-results.v1",
            "application/json",
        )?;
        let bound_prompt =
            required_typed_input(&staged, "bound-prompt", "mdp.prompt.v0", "application/yaml")?;
        let normalized_value: Value = serde_json::from_slice(&fs::read(&normalized.staged_path)?)?;
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
                "mdp.source-binding.v2",
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
            )?
        };
        let ready = result["valid"].as_bool() == Some(true)
            && normalized_value["outcome"].as_str() == Some("ready");
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
                        request
                            .job_identity
                            .as_ref()
                            .map(|identity| identity.job_id.as_str()),
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
    deadline.check()?;

    before_post_check()?;
    deadline.check()?;
    let staged_pack_after = pack_content_snapshot(&staged_pack)?;
    let source_pack_after = pack_content_snapshot(source_pack)?;
    let sources_unchanged = verify_sources_unchanged(&staged).is_ok();
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
        provider_request_body_sha256: None,
        provider_request_schema_id: None,
        terminal_state,
        assurance: assurance.clone(),
        limitations: vec![
            "local deterministic validation does not attest to authoring-context provenance".into(),
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
    if request.mode != RunMode::Deterministic {
        return Err(anyhow!(
            "the local kernel currently supports deterministic runs only"
        ));
    }
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
    if request.execution_policy.max_input_bytes == 0
        || request.execution_policy.max_output_bytes == 0
        || request.execution_policy.timeout_ms == 0
    {
        return Err(anyhow!("execution policy limits must be positive"));
    }
    if !matches!(
        request.execution_policy.retention_policy.as_str(),
        "receipt-only" | "customer-controlled-workdir"
    ) {
        return Err(anyhow!("unsupported deterministic retention policy"));
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
            let source = Path::new(&input.source_path);
            let metadata = fs::symlink_metadata(source)
                .with_context(|| format!("reading declared input {}", source.display()))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(anyhow!("declared inputs must be regular non-symlink files"));
            }
            reject_hard_link(&metadata, "declared inputs")?;
            total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or_else(|| anyhow!("declared input byte count overflow"))?;
            if total_bytes > request.execution_policy.max_input_bytes {
                return Err(anyhow!(
                    "declared inputs exceed execution policy byte limit"
                ));
            }
            let remaining = request
                .execution_policy
                .max_input_bytes
                .checked_sub(total_bytes - metadata.len())
                .ok_or_else(|| anyhow!("declared input byte count overflow"))?;
            let bytes = read_bounded(source, remaining, "declared input")?;
            if bytes.len() as u64 != metadata.len() {
                return Err(anyhow!("declared input changed while it was staged"));
            }
            let initial_sha256 = sha256_hex(&bytes);
            let staged_path = target.join(format!("{index:03}-{}", input.logical_name));
            write_bytes_create_new(&staged_path, &bytes)?;
            let staged_bytes = fs::read(&staged_path)?;
            if sha256_hex(&staged_bytes) != initial_sha256 {
                return Err(anyhow!("declared input changed while it was staged"));
            }
            Ok(StagedInput {
                logical_name: input.logical_name.clone(),
                authority: ArtifactAuthority {
                    logical_name: format!("declared/{index:03}-{}", input.logical_name),
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
        })
        .collect()
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
            state: AssuranceEvidenceState::NotApplicable,
            provenance: EvidenceProvenance::MdpObserved,
            evidence_refs: vec![],
            limitations: vec!["this operation performs no model inference".into()],
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
    use super::{execute_run_inner, gtm_success_artifacts, validate_request};
    use crate::commands::init::init_pack;
    use crate::run_contracts::{
        ExecutionPolicy, LocalArtifactInput, PackAuthority, RunBundleV1, RunMode, RunRequestV1,
        TerminalState,
    };
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rejects_ambient_authority_for_deterministic_run() {
        let mut request = request_fixture("not-used", "not-used");
        request.execution_policy.network_mode = "allow".into();
        assert!(validate_request(&request).is_err());
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
        assert_eq!(receipt["decision"]["decision"], "no-draft");
        assert_eq!(
            receipt["decision"]["reason_codes"],
            serde_json::json!(["insufficient-context"])
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
                version: "1".into(),
                profile_id: "gtm".into(),
                portable_digest: "a".repeat(64),
                files: vec![],
            },
            prompt: None,
            inputs: vec![],
            execution_policy_sha256: "b".repeat(64),
            driver: None,
            model: None,
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
                    "mdp.normalized-decision-input.v1",
                ),
                input(
                    "source-attempt-request",
                    fixtures.join("source-attempt-request.json"),
                    "mdp.source-attempt-request.v1",
                ),
                input(
                    "collected-attempt-results",
                    fixtures.join("collected-attempt-results.json"),
                    "mdp.collected-attempt-results.v1",
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
                max_input_bytes: 2_097_152,
                max_output_bytes: 1_048_576,
                timeout_ms: 30_000,
                retention_policy: "receipt-only".into(),
            },
            driver: None,
            model: None,
        }
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
