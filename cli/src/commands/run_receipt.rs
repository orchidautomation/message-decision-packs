use crate::cli::{RunIsolation, RunReceiptWorkflow};
use crate::constants::{DEFAULT_DIR, PROMPT_OUTPUT_CONTRACT, SOURCE_AUDIT_CONTRACT};
use crate::pack_io::read_manifest;
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) struct RunReceiptOptions<'a> {
    pub(crate) root: &'a Path,
    pub(crate) workflow: RunReceiptWorkflow,
    pub(crate) isolation: RunIsolation,
    pub(crate) declared_inputs_only: bool,
    pub(crate) prompt_id: Option<&'a str>,
    pub(crate) prompt_output: Option<&'a Path>,
    pub(crate) validation: Option<&'a Path>,
    pub(crate) source_audit: Option<&'a Path>,
    pub(crate) artifacts: &'a [String],
}

pub(crate) fn run_receipt(options: RunReceiptOptions<'_>) -> Result<Value> {
    let mut issues = Vec::new();
    let mut blocked = false;
    let mut boundary_failure = false;
    let mut artifacts = Vec::new();

    let manifest_path = options.root.join(DEFAULT_DIR).join("manifest.yaml");
    let pack = match read_manifest(options.root) {
        Ok(manifest) => {
            let manifest_artifact = artifact_record(
                "pack-manifest",
                &manifest_path,
                true,
                &mut issues,
                &mut blocked,
            );
            artifacts.push(manifest_artifact);
            json!({
                "dir": options.root.display().to_string(),
                "manifest": manifest_path.display().to_string(),
                "id": manifest.id,
                "name": manifest.name,
                "version": manifest.version,
                "profile_id": manifest.profile.map(|profile| profile.id).unwrap_or_default()
            })
        }
        Err(err) => {
            blocked = true;
            issues.push(issue(
                "pack_manifest_unreadable",
                "error",
                manifest_path.display().to_string(),
                err.to_string(),
            ));
            json!({
                "dir": options.root.display().to_string(),
                "manifest": manifest_path.display().to_string()
            })
        }
    };

    let prompt_output_value = required_json_artifact(
        "prompt-output",
        options.prompt_output,
        &mut artifacts,
        &mut issues,
        &mut blocked,
    )?;
    if let Some(value) = prompt_output_value.as_ref() {
        validate_prompt_output_summary(value, options.prompt_id, &mut issues, &mut blocked);
    }

    let validation_value = required_json_artifact(
        "validation",
        options.validation,
        &mut artifacts,
        &mut issues,
        &mut blocked,
    )?;
    let validation_data = validation_value.as_ref().map(validation_payload);
    if let Some(value) = validation_data.as_ref() {
        validate_validation_summary(
            value,
            options.prompt_id,
            options.workflow.requires_source_audit(),
            options.source_audit.is_some(),
            &mut issues,
            &mut blocked,
        );
    }

    let source_audit_value = if options.workflow.requires_source_audit() {
        required_json_artifact(
            "source-audit",
            options.source_audit,
            &mut artifacts,
            &mut issues,
            &mut blocked,
        )?
    } else {
        optional_json_artifact(
            "source-audit",
            options.source_audit,
            &mut artifacts,
            &mut issues,
            &mut blocked,
        )?
    };
    if let Some(value) = source_audit_value.as_ref() {
        validate_source_audit_summary(value, &mut issues, &mut blocked);
    }

    for raw in options.artifacts {
        let (kind, path) = parse_extra_artifact(raw)?;
        artifacts.push(artifact_record(
            &kind,
            &path,
            false,
            &mut issues,
            &mut blocked,
        ));
    }

    if options.isolation != RunIsolation::Isolated {
        boundary_failure = true;
        issues.push(issue(
            "context_isolation_not_confirmed",
            "error",
            "boundary.isolation",
            match options.isolation {
                RunIsolation::Ambient => {
                    "audit-grade receipts require a fresh/stateless model call; ambient conversation context was used"
                }
                RunIsolation::Unknown => {
                    "audit-grade receipts require a fresh/stateless model call; model context isolation is unknown"
                }
                RunIsolation::Isolated => unreachable!(),
            },
        ));
    }
    if !options.declared_inputs_only {
        boundary_failure = true;
        issues.push(issue(
            "declared_inputs_only_not_confirmed",
            "error",
            "boundary.declared_inputs_only",
            "audit-grade receipts require the host runner to pass only prompt-declared inputs to the model call",
        ));
    }

    let decision = if blocked {
        "blocked"
    } else if boundary_failure {
        "advisory"
    } else {
        "audit-grade"
    };
    let valid = decision == "audit-grade";
    let error_count = issues
        .iter()
        .filter(|issue| issue["severity"].as_str() == Some("error"))
        .count();
    let warning_count = issues
        .iter()
        .filter(|issue| issue["severity"].as_str() == Some("warning"))
        .count();

    Ok(json!({
        "contract": "mdp.run-receipt.v0",
        "valid": valid,
        "decision": decision,
        "workflow": options.workflow.as_str(),
        "pack": pack,
        "boundary": {
            "isolation": options.isolation.as_str(),
            "conversation_context_used": options.isolation.conversation_context_used(),
            "declared_inputs_only": options.declared_inputs_only,
            "audit_grade_requires": {
                "fresh_stateless_model_call": true,
                "no_prior_conversation_context": true,
                "prompt_declared_inputs_only": true,
                "local_artifact_receipts": true
            }
        },
        "prompt": {
            "id": options.prompt_id,
            "prompt_output": options.prompt_output.map(|path| path.display().to_string()),
            "validation": options.validation.map(|path| path.display().to_string()),
            "source_audit": options.source_audit.map(|path| path.display().to_string()),
            "source_audit_required": options.workflow.requires_source_audit()
        },
        "guarantee_owners": {
            "host_runner": [
                "create a fresh/stateless model call for normalization",
                "pass only prompt-declared payload fields",
                "persist raw/local source, prompt-output, validation, and review artifacts in customer-controlled storage",
                "record artifact hashes in this receipt"
            ],
            "mdp_cli": [
                "hash local artifacts named in the receipt",
                "confirm prompt-output validation succeeded",
                "confirm proposal source-audit artifacts are present and used when required",
                "run deterministic pack validation, fit, route, claim, and proof-output checks"
            ],
            "not_guaranteed_by_cli": [
                "semantic truth of a model claim beyond supplied artifacts",
                "host model context isolation unless the runner reports it",
                "PDF/OCR extraction quality beyond the provided source-audit ledger"
            ]
        },
        "artifacts": artifacts,
        "issues": issues,
        "error_count": error_count,
        "warning_count": warning_count
    }))
}

fn required_json_artifact(
    kind: &str,
    path: Option<&Path>,
    artifacts: &mut Vec<Value>,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
) -> Result<Option<Value>> {
    let Some(path) = path else {
        *blocked = true;
        let code_kind = issue_code_kind(kind);
        issues.push(issue(
            format!("{code_kind}_artifact_missing"),
            "error",
            format!("prompt.{kind}"),
            format!("{kind} artifact is required for an audit-grade run receipt"),
        ));
        return Ok(None);
    };
    read_json_artifact(kind, path, true, artifacts, issues, blocked)
}

fn optional_json_artifact(
    kind: &str,
    path: Option<&Path>,
    artifacts: &mut Vec<Value>,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
) -> Result<Option<Value>> {
    let Some(path) = path else {
        return Ok(None);
    };
    read_json_artifact(kind, path, false, artifacts, issues, blocked)
}

fn read_json_artifact(
    kind: &str,
    path: &Path,
    required: bool,
    artifacts: &mut Vec<Value>,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
) -> Result<Option<Value>> {
    artifacts.push(artifact_record(kind, path, required, issues, blocked));
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    match serde_json::from_str::<Value>(&raw) {
        Ok(value) => Ok(Some(value)),
        Err(err) => {
            *blocked = true;
            let code_kind = issue_code_kind(kind);
            issues.push(issue(
                format!("{code_kind}_artifact_parse_failed"),
                "error",
                path.display().to_string(),
                format!("{kind} artifact must contain valid JSON: {err}"),
            ));
            Ok(None)
        }
    }
}

fn artifact_record(
    kind: &str,
    path: &Path,
    required: bool,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
) -> Value {
    match fs::read(path) {
        Ok(bytes) => {
            let hash = Sha256::digest(&bytes);
            json!({
                "kind": kind,
                "path": path.display().to_string(),
                "required": required,
                "exists": true,
                "bytes": bytes.len(),
                "sha256": format!("{hash:x}")
            })
        }
        Err(err) => {
            if required {
                *blocked = true;
                let code_kind = issue_code_kind(kind);
                issues.push(issue(
                    format!("{code_kind}_artifact_unreadable"),
                    "error",
                    path.display().to_string(),
                    format!("required {kind} artifact is unreadable: {err}"),
                ));
            }
            json!({
                "kind": kind,
                "path": path.display().to_string(),
                "required": required,
                "exists": false,
                "bytes": Value::Null,
                "sha256": Value::Null
            })
        }
    }
}

fn issue_code_kind(kind: &str) -> String {
    kind.replace('-', "_")
}

fn validate_prompt_output_summary(
    value: &Value,
    prompt_id: Option<&str>,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
) {
    if value["contract"].as_str() != Some(PROMPT_OUTPUT_CONTRACT) {
        *blocked = true;
        issues.push(issue(
            "prompt_output_contract_mismatch",
            "error",
            "prompt_output.contract",
            format!("prompt-output contract must be {PROMPT_OUTPUT_CONTRACT}"),
        ));
    }
    if let Some(prompt_id) = prompt_id {
        let actual = value["prompt_id"].as_str().unwrap_or_default();
        if actual != prompt_id {
            *blocked = true;
            issues.push(issue(
                "prompt_output_prompt_id_mismatch",
                "error",
                "prompt_output.prompt_id",
                format!("prompt-output prompt_id must be {prompt_id}; got {actual}"),
            ));
        }
    }
}

fn validate_validation_summary(
    value: &Value,
    prompt_id: Option<&str>,
    source_audit_required: bool,
    source_audit_provided: bool,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
) {
    if value["valid"].as_bool() != Some(true) {
        *blocked = true;
        issues.push(issue(
            "prompt_output_validation_failed",
            "error",
            "validation.valid",
            "validate-prompt-output result must have valid: true",
        ));
    }
    if let Some(prompt_id) = prompt_id {
        let actual = value["prompt"]["id"].as_str().unwrap_or_default();
        if actual != prompt_id {
            *blocked = true;
            issues.push(issue(
                "validation_prompt_id_mismatch",
                "error",
                "validation.prompt.id",
                format!("validation prompt id must be {prompt_id}; got {actual}"),
            ));
        }
    }
    if source_audit_required {
        if !source_audit_provided {
            *blocked = true;
            issues.push(issue(
                "source_audit_artifact_missing",
                "error",
                "prompt.source_audit",
                "proposal-review receipts require the source-audit artifact used during normalization validation",
            ));
        }
        if value["source_audit"]["contract"].as_str() != Some(SOURCE_AUDIT_CONTRACT) {
            *blocked = true;
            issues.push(issue(
                "source_audit_not_validated",
                "error",
                "validation.source_audit.contract",
                "validation result must include a source_audit summary, proving validate-prompt-output ran with --source-audit",
            ));
        }
    }
}

fn validate_source_audit_summary(value: &Value, issues: &mut Vec<Value>, blocked: &mut bool) {
    if value["contract"].as_str() != Some(SOURCE_AUDIT_CONTRACT) {
        *blocked = true;
        issues.push(issue(
            "source_audit_contract_mismatch",
            "error",
            "source_audit.contract",
            format!("source-audit contract must be {SOURCE_AUDIT_CONTRACT}"),
        ));
    }
}

fn validation_payload(value: &Value) -> Value {
    if value["ok"].as_bool() == Some(true)
        && value["command"].as_str() == Some("validate-prompt-output")
    {
        value["data"].clone()
    } else {
        value.clone()
    }
}

fn parse_extra_artifact(raw: &str) -> Result<(String, PathBuf)> {
    let Some((kind, path)) = raw.split_once('=') else {
        return Err(anyhow!("--artifact must use KIND=PATH"));
    };
    let kind = kind.trim();
    let path = path.trim();
    if kind.is_empty() || path.is_empty() {
        return Err(anyhow!("--artifact must use non-empty KIND=PATH"));
    }
    Ok((kind.to_string(), PathBuf::from(path)))
}

fn issue(
    code: impl Into<String>,
    severity: &'static str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> Value {
    json!({
        "code": code.into(),
        "severity": severity,
        "path": path.into(),
        "message": message.into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn isolated_proposal_run_with_source_audit_is_audit_grade() {
        let root = test_pack("receipt-audit-grade");
        let prompt_output = write_json(
            &root,
            "prompt-output.json",
            json!({
                "contract": "mdp.prompt-output.v0",
                "prompt_id": "normalize-opportunity"
            }),
        );
        let source_audit = write_json(
            &root,
            "source-audit.json",
            json!({
                "contract": "mdp.source-audit.v0",
                "refs": []
            }),
        );
        let validation = write_json(
            &root,
            "validation.json",
            json!({
                "valid": true,
                "prompt": {"id": "normalize-opportunity"},
                "source_audit": {"contract": "mdp.source-audit.v0"}
            }),
        );

        let receipt = run_receipt(RunReceiptOptions {
            root: &root,
            workflow: RunReceiptWorkflow::ProposalReview,
            isolation: RunIsolation::Isolated,
            declared_inputs_only: true,
            prompt_id: Some("normalize-opportunity"),
            prompt_output: Some(&prompt_output),
            validation: Some(&validation),
            source_audit: Some(&source_audit),
            artifacts: &[],
        })
        .expect("receipt should build");

        assert_eq!(receipt["valid"], true);
        assert_eq!(receipt["decision"], "audit-grade");
        assert_eq!(receipt["boundary"]["conversation_context_used"], false);
        assert!(
            receipt["artifacts"]
                .as_array()
                .expect("artifacts")
                .iter()
                .any(|artifact| artifact["kind"] == "source-audit"
                    && artifact["sha256"].as_str().unwrap_or_default().len() == 64)
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ambient_context_marks_receipt_advisory() {
        let root = test_pack("receipt-advisory");
        let prompt_output = write_json(
            &root,
            "prompt-output.json",
            json!({"contract": "mdp.prompt-output.v0", "prompt_id": "normalize-opportunity"}),
        );
        let source_audit = write_json(
            &root,
            "source-audit.json",
            json!({"contract": "mdp.source-audit.v0", "refs": []}),
        );
        let validation = write_json(
            &root,
            "validation.json",
            json!({"valid": true, "prompt": {"id": "normalize-opportunity"}, "source_audit": {"contract": "mdp.source-audit.v0"}}),
        );

        let receipt = run_receipt(RunReceiptOptions {
            root: &root,
            workflow: RunReceiptWorkflow::ProposalReview,
            isolation: RunIsolation::Ambient,
            declared_inputs_only: true,
            prompt_id: Some("normalize-opportunity"),
            prompt_output: Some(&prompt_output),
            validation: Some(&validation),
            source_audit: Some(&source_audit),
            artifacts: &[],
        })
        .expect("receipt should build");

        assert_eq!(receipt["valid"], false);
        assert_eq!(receipt["decision"], "advisory");
        assert_eq!(receipt["boundary"]["conversation_context_used"], true);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_proposal_source_audit_blocks_receipt() {
        let root = test_pack("receipt-missing-source-audit");
        let prompt_output = write_json(
            &root,
            "prompt-output.json",
            json!({"contract": "mdp.prompt-output.v0", "prompt_id": "normalize-opportunity"}),
        );
        let validation = write_json(
            &root,
            "validation.json",
            json!({"valid": true, "prompt": {"id": "normalize-opportunity"}}),
        );

        let receipt = run_receipt(RunReceiptOptions {
            root: &root,
            workflow: RunReceiptWorkflow::ProposalReview,
            isolation: RunIsolation::Isolated,
            declared_inputs_only: true,
            prompt_id: Some("normalize-opportunity"),
            prompt_output: Some(&prompt_output),
            validation: Some(&validation),
            source_audit: None,
            artifacts: &[],
        })
        .expect("receipt should build");

        assert_eq!(receipt["valid"], false);
        assert_eq!(receipt["decision"], "blocked");
        assert!(
            receipt["issues"]
                .as_array()
                .expect("issues")
                .iter()
                .any(|issue| issue["code"] == "source_audit_artifact_missing")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validation_cli_wrapper_is_accepted() {
        let root = test_pack("receipt-validation-wrapper");
        let prompt_output = write_json(
            &root,
            "prompt-output.json",
            json!({"contract": "mdp.prompt-output.v0", "prompt_id": "normalize-opportunity"}),
        );
        let source_audit = write_json(
            &root,
            "source-audit.json",
            json!({"contract": "mdp.source-audit.v0", "refs": []}),
        );
        let validation = write_json(
            &root,
            "validation.json",
            json!({
                "ok": true,
                "command": "validate-prompt-output",
                "data": {
                    "valid": true,
                    "prompt": {"id": "normalize-opportunity"},
                    "source_audit": {"contract": "mdp.source-audit.v0"}
                }
            }),
        );

        let receipt = run_receipt(RunReceiptOptions {
            root: &root,
            workflow: RunReceiptWorkflow::ProposalReview,
            isolation: RunIsolation::Isolated,
            declared_inputs_only: true,
            prompt_id: Some("normalize-opportunity"),
            prompt_output: Some(&prompt_output),
            validation: Some(&validation),
            source_audit: Some(&source_audit),
            artifacts: &[],
        })
        .expect("receipt should build");

        assert_eq!(receipt["valid"], true);
        assert_eq!(receipt["decision"], "audit-grade");

        let _ = fs::remove_dir_all(root);
    }

    fn test_pack(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("{label}-{}", nonce()));
        init_pack(&root, "Receipt Pack", "proposal", true, false).expect("pack should init");
        root
    }

    fn write_json(root: &Path, name: &str, value: Value) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, serde_json::to_string_pretty(&value).expect("json")).expect("write json");
        path
    }

    fn nonce() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    }
}
