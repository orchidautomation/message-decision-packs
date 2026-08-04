use crate::artifact_hash::{canonical_json_sha256_for_domain, pack_content_snapshot, sha256_hex};
use crate::commands::prompt_output::validate_prompt_output_file_with_inputs;
use crate::commands::routing::fit;
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
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const PROPOSAL_PROFILE: &str = "proposal";
const VALIDATE_EXISTING_OUTPUT: &str = "validate-existing-output";
const GTM_PROFILE: &str = "gtm";
const QUALIFY: &str = "qualify";
const GENERATED_PACK_DIRECTORIES: &[&str] = &["briefs", "traces"];

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

#[derive(Clone)]
struct StagedInput {
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
    validate_request(request)?;
    let final_dir = output_root;
    if final_dir.exists() {
        return Err(anyhow!(
            "run output directory already exists: {}",
            final_dir.display()
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
    let transaction_dir = parent.join(format!(".{leaf}.tmp-{}", unique_suffix()));
    fs::create_dir(&transaction_dir).with_context(|| {
        format!(
            "creating transaction directory {}",
            transaction_dir.display()
        )
    })?;
    set_private_directory(&transaction_dir)?;

    let outcome = execute_transaction(request, &transaction_dir, before_post_check);
    if outcome.is_err() {
        let _ = fs::remove_dir_all(&transaction_dir);
    }
    let (bundle_sha256, receipt) = outcome?;
    fs::remove_dir_all(transaction_dir.join("private"))
        .context("removing private staged inputs before commit")?;
    fs::rename(&transaction_dir, &final_dir).with_context(|| {
        format!(
            "atomically committing run directory {}",
            final_dir.display()
        )
    })?;

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

fn execute_transaction<F>(
    request: &RunRequestV1,
    transaction_dir: &Path,
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
    let source_snapshot = pack_content_snapshot(source_pack)?;
    copy_pack(source_pack, &staged_pack)?;
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

    let staged = stage_inputs(request, &staged_inputs)?;
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
    let (mut terminal_state, mut success_values) = if request.profile != profile_id {
        (TerminalState::NoDraftPolicyBlocked, None)
    } else if request.profile == PROPOSAL_PROFILE && request.operation == VALIDATE_EXISTING_OUTPUT {
        let prompt_output = required_input(&staged, "prompt-output")?;
        let source_audit = optional_input(&staged, "source-audit");
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
        let source_attempt = required_input(&staged, "source-attempt-request")?;
        let attempt_results = required_input(&staged, "collected-attempt-results")?;
        let bound_prompt = required_input(&staged, "bound-prompt")?;
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
        let result = validate_prompt_output_file_with_inputs(
            &staged_pack,
            &normalized.staged_path,
            Some(&staged_bound_prompt),
            None,
            None,
            Some(&source_attempt.staged_path),
            Some(&attempt_results.staged_path),
        )?;
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
                let fit_result = fit(&staged_pack, &prospect_path)?;
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

    before_post_check()?;
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
            "disqualifiers": fit_result["disqualifiers"]
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
        .find(|input| input.logical_name.ends_with("prompt-output"))
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

fn validate_request(request: &RunRequestV1) -> Result<()> {
    if request.contract != RUN_REQUEST_V1 {
        return Err(anyhow!("unsupported run request contract"));
    }
    if request.execution_id.is_empty()
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
    let mut names = HashSet::new();
    for input in &request.inputs {
        validate_logical_name(&input.logical_name)?;
        if !names.insert(input.logical_name.as_str()) {
            return Err(anyhow!("duplicate declared input logical_name"));
        }
    }
    Ok(())
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
            let bytes = fs::read(source)?;
            total_bytes = total_bytes
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| anyhow!("declared input byte count overflow"))?;
            if total_bytes > request.execution_policy.max_input_bytes {
                return Err(anyhow!(
                    "declared inputs exceed execution policy byte limit"
                ));
            }
            let initial_sha256 = sha256_hex(&bytes);
            let staged_path = target.join(format!("{index:03}-{}", input.logical_name));
            write_bytes_create_new(&staged_path, &bytes)?;
            let staged_bytes = fs::read(&staged_path)?;
            if sha256_hex(&staged_bytes) != initial_sha256 {
                return Err(anyhow!("declared input changed while it was staged"));
            }
            Ok(StagedInput {
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
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || sha256_hex(&fs::read(&input.source_path)?) != input.initial_sha256
            || sha256_hex(&fs::read(&input.staged_path)?) != input.initial_sha256
        {
            return Err(anyhow!("declared input mutated during execution"));
        }
    }
    Ok(())
}

fn required_input<'a>(inputs: &'a [StagedInput], name: &str) -> Result<&'a StagedInput> {
    optional_input(inputs, name).ok_or_else(|| anyhow!("required declared input missing: {name}"))
}

fn optional_input<'a>(inputs: &'a [StagedInput], name: &str) -> Option<&'a StagedInput> {
    inputs.iter().find(|input| {
        input
            .authority
            .logical_name
            .rsplit('-')
            .next()
            .is_some_and(|suffix| suffix == name)
            || input.authority.logical_name.ends_with(&format!("-{name}"))
    })
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
            state: AssuranceEvidenceState::Enforced,
            provenance: EvidenceProvenance::MdpObserved,
            evidence_refs: vec![bundle_sha256.into()],
            limitations: vec![
                "OS-level access outside the private staging tree is not attested".into(),
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
    copy_pack_directory(&source, &target, true)
}

fn copy_pack_directory(source: &Path, target: &Path, pack_root: bool) -> Result<()> {
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
            copy_pack_directory(&entry.path(), &destination, false)?;
        } else if metadata.is_file() {
            reject_hard_link(&metadata, "pack staging")?;
            write_bytes_create_new(&destination, &fs::read(entry.path())?)?;
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
    use super::{execute_run_inner, validate_request};
    use crate::commands::init::init_pack;
    use crate::run_contracts::{
        ExecutionPolicy, LocalArtifactInput, RunMode, RunRequestV1, TerminalState,
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
