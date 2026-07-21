use crate::cli::{RunIsolation, RunReceiptWorkflow};
use crate::constants::{
    DEFAULT_DIR, PROMPT_OUTPUT_CONTRACT, RUNNER_AUDIT_CONTRACT, SOURCE_AUDIT_CONTRACT,
};
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
    pub(crate) runner_audit: Option<&'a Path>,
    pub(crate) require_runner_audit: bool,
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

    let runner_audit_value = if options.require_runner_audit {
        required_json_artifact(
            "runner-audit",
            options.runner_audit,
            &mut artifacts,
            &mut issues,
            &mut blocked,
        )?
    } else {
        optional_json_artifact(
            "runner-audit",
            options.runner_audit,
            &mut artifacts,
            &mut issues,
            &mut blocked,
        )?
    };
    let runner = validate_runner_audit_summary(
        runner_audit_value.as_ref(),
        options.runner_audit,
        options.require_runner_audit,
        options.declared_inputs_only,
        &mut issues,
        &mut blocked,
    );

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
        "runner": runner,
        "prompt": {
            "id": options.prompt_id,
            "prompt_output": options.prompt_output.map(|path| path.display().to_string()),
            "validation": options.validation.map(|path| path.display().to_string()),
            "source_audit": options.source_audit.map(|path| path.display().to_string()),
            "runner_audit": options.runner_audit.map(|path| path.display().to_string()),
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

fn validate_runner_audit_summary(
    value: Option<&Value>,
    runner_audit_path: Option<&Path>,
    require_runner_audit: bool,
    declared_inputs_only: bool,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
) -> Value {
    let Some(value) = value else {
        if !require_runner_audit {
            issues.push(issue(
                "runner_audit_not_supplied",
                "warning",
                "runner.runner_audit",
                "runner-audit was not supplied; receipt relies on host assertion flags instead of headless/native runner evidence",
            ));
        }
        return json!({
            "runner_audit": runner_audit_path.map(|path| path.display().to_string()),
            "runner_audit_required": require_runner_audit,
            "assurance": if require_runner_audit { "missing" } else { "asserted" },
            "summary": Value::Null
        });
    };

    let mut valid = true;
    validate_runner_required_string(
        value,
        "contract",
        RUNNER_AUDIT_CONTRACT,
        "runner_audit_contract_mismatch",
        "runner_audit.contract",
        issues,
        blocked,
        &mut valid,
    );

    let runner = value["runner"].as_str().unwrap_or_default();
    if !matches!(
        runner,
        "native-api"
            | "codex-exec"
            | "claude-print"
            | "cursor-print"
            | "opencode-run"
            | "custom-headless"
    ) {
        valid = false;
        *blocked = true;
        issues.push(issue(
            "runner_audit_runner_unsupported",
            "error",
            "runner_audit.runner",
            "runner-audit runner must be one of native-api, codex-exec, claude-print, cursor-print, opencode-run, or custom-headless",
        ));
    }

    require_bool(
        value,
        "isolated_invocation",
        true,
        "runner_audit_not_isolated",
        "runner_audit.isolated_invocation",
        "runner-audit must report an isolated invocation",
        issues,
        blocked,
        &mut valid,
    );
    require_bool(
        value,
        "conversation_resume",
        false,
        "runner_audit_resumed_conversation",
        "runner_audit.conversation_resume",
        "runner-audit must not resume or continue a prior conversation",
        issues,
        blocked,
        &mut valid,
    );
    require_bool(
        value,
        "declared_inputs_only",
        true,
        "runner_audit_declared_inputs_only_false",
        "runner_audit.declared_inputs_only",
        "runner-audit must report that only prompt-declared inputs crossed the model boundary",
        issues,
        blocked,
        &mut valid,
    );
    require_bool(
        value,
        "output_schema_used",
        true,
        "runner_audit_output_schema_missing",
        "runner_audit.output_schema_used",
        "runner-audit must report that a JSON output schema or equivalent structured-output contract was used",
        issues,
        blocked,
        &mut valid,
    );

    if !declared_inputs_only && value["declared_inputs_only"].as_bool() == Some(true) {
        valid = false;
        *blocked = true;
        issues.push(issue(
            "runner_audit_cli_boundary_mismatch",
            "error",
            "boundary.declared_inputs_only",
            "runner-audit reports declared-input-only, but the run receipt CLI flag did not confirm it",
        ));
    }

    if value["prior_messages_included"].as_bool() == Some(true) {
        valid = false;
        *blocked = true;
        issues.push(issue(
            "runner_audit_prior_messages_included",
            "error",
            "runner_audit.prior_messages_included",
            "runner-audit must not include prior conversation messages in the normalization request",
        ));
    }

    if let Some(count) = value["tool_invocations_observed"].as_u64() {
        if count != 0 {
            valid = false;
            *blocked = true;
            issues.push(issue(
                "runner_audit_tool_invocations_observed",
                "error",
                "runner_audit.tool_invocations_observed",
                "normalization runner audit-grade mode must observe zero tool invocations during the model run",
            ));
        }
    }

    match runner {
        "native-api" => validate_native_api_runner(value, issues, blocked, &mut valid),
        "codex-exec" => validate_codex_exec_runner(value, issues, blocked, &mut valid),
        "claude-print" => validate_claude_print_runner(value, issues, blocked, &mut valid),
        "cursor-print" => validate_cursor_print_runner(value, issues, blocked, &mut valid),
        "opencode-run" => validate_opencode_run_runner(value, issues, blocked, &mut valid),
        "custom-headless" => validate_custom_headless_runner(value, issues, blocked, &mut valid),
        _ => {}
    }

    let assurance = if !valid {
        "invalid"
    } else if runner == "native-api" {
        "stateless-api-verified"
    } else {
        "headless-verified"
    };

    json!({
        "runner_audit": runner_audit_path.map(|path| path.display().to_string()),
        "runner_audit_required": require_runner_audit,
        "assurance": assurance,
        "summary": {
            "contract": value["contract"].clone(),
            "runner": value["runner"].clone(),
            "model": value.get("model").cloned().unwrap_or(Value::Null),
            "isolated_invocation": value["isolated_invocation"].clone(),
            "conversation_resume": value["conversation_resume"].clone(),
            "declared_inputs_only": value["declared_inputs_only"].clone(),
            "output_schema_used": value["output_schema_used"].clone(),
            "tool_invocations_observed": value.get("tool_invocations_observed").cloned().unwrap_or(Value::Null)
        }
    })
}

fn validate_native_api_runner(
    value: &Value,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
    valid: &mut bool,
) {
    require_bool(
        value,
        "stateless_request",
        true,
        "runner_audit_native_api_not_stateless",
        "runner_audit.stateless_request",
        "native-api runner audits must report a stateless request",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "prior_messages_included",
        false,
        "runner_audit_native_api_prior_messages",
        "runner_audit.prior_messages_included",
        "native-api runner audits must report no prior messages in the request",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "tools_disabled",
        true,
        "runner_audit_native_api_tools_enabled",
        "runner_audit.tools_disabled",
        "native-api runner audits must report no tools made available to the model",
        issues,
        blocked,
        valid,
    );
}

fn validate_codex_exec_runner(
    value: &Value,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
    valid: &mut bool,
) {
    require_bool(
        value,
        "ephemeral",
        true,
        "runner_audit_codex_not_ephemeral",
        "runner_audit.ephemeral",
        "codex-exec runner audits must report --ephemeral",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "session_persistence",
        false,
        "runner_audit_codex_session_persistence",
        "runner_audit.session_persistence",
        "codex-exec runner audits must report no session persistence",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "sterile_workdir",
        true,
        "runner_audit_codex_workdir_not_sterile",
        "runner_audit.sterile_workdir",
        "codex-exec runner audits must run from a sterile workdir rather than the proposal repo or chat workspace",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "prompt_input_audited",
        true,
        "runner_audit_codex_prompt_input_not_audited",
        "runner_audit.prompt_input_audited",
        "codex-exec runner audits must inspect the model-visible prompt input list",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "config_discovery_disabled",
        true,
        "runner_audit_codex_config_discovery_enabled",
        "runner_audit.config_discovery_disabled",
        "codex-exec runner audits must disable user config discovery for normalization",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "instructions_discovery_disabled",
        true,
        "runner_audit_codex_instructions_discovery_enabled",
        "runner_audit.instructions_discovery_disabled",
        "codex-exec runner audits must prevent global/project AGENTS.md or equivalent instruction discovery from entering the normalization prompt",
        issues,
        blocked,
        valid,
    );
    validate_runner_required_string(
        value,
        "sandbox",
        "read-only",
        "runner_audit_codex_sandbox_not_read_only",
        "runner_audit.sandbox",
        issues,
        blocked,
        valid,
    );
}

fn validate_claude_print_runner(
    value: &Value,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
    valid: &mut bool,
) {
    require_bool(
        value,
        "bare",
        true,
        "runner_audit_claude_not_bare",
        "runner_audit.bare",
        "claude-print runner audits must report --bare",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "session_persistence",
        false,
        "runner_audit_claude_session_persistence",
        "runner_audit.session_persistence",
        "claude-print runner audits must report --no-session-persistence",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "tools_disabled",
        true,
        "runner_audit_claude_tools_enabled",
        "runner_audit.tools_disabled",
        "claude-print runner audits must report tools disabled for normalization",
        issues,
        blocked,
        valid,
    );
}

fn validate_cursor_print_runner(
    value: &Value,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
    valid: &mut bool,
) {
    require_bool(
        value,
        "session_persistence",
        false,
        "runner_audit_cursor_session_persistence",
        "runner_audit.session_persistence",
        "cursor-print runner audits must report no session persistence or resume",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "force_enabled",
        false,
        "runner_audit_cursor_force_enabled",
        "runner_audit.force_enabled",
        "cursor-print runner audits must not enable --force for normalization",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "sterile_workdir",
        true,
        "runner_audit_cursor_workdir_not_sterile",
        "runner_audit.sterile_workdir",
        "cursor-print runner audits must run from a sterile workdir rather than the proposal repo or chat workspace",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "prompt_input_audited",
        true,
        "runner_audit_cursor_prompt_input_not_audited",
        "runner_audit.prompt_input_audited",
        "cursor-print runner audits must inspect or otherwise constrain the model-visible prompt input",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "tools_disabled",
        true,
        "runner_audit_cursor_tools_enabled",
        "runner_audit.tools_disabled",
        "cursor-print exposes tools in headless mode, so the wrapper must disable or policy-deny tool use for normalization",
        issues,
        blocked,
        valid,
    );
}

fn validate_opencode_run_runner(
    value: &Value,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
    valid: &mut bool,
) {
    require_bool(
        value,
        "session_persistence",
        false,
        "runner_audit_opencode_session_persistence",
        "runner_audit.session_persistence",
        "opencode-run runner audits must report no session persistence or resume",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "pure",
        true,
        "runner_audit_opencode_not_pure",
        "runner_audit.pure",
        "opencode-run runner audits must report --pure so external plugins are disabled",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "default_plugins_disabled",
        true,
        "runner_audit_opencode_default_plugins_enabled",
        "runner_audit.default_plugins_disabled",
        "opencode-run runner audits must disable default plugins for normalization",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "claude_code_discovery_disabled",
        true,
        "runner_audit_opencode_claude_code_discovery_enabled",
        "runner_audit.claude_code_discovery_disabled",
        "opencode-run runner audits must disable Claude Code prompt/skill discovery for normalization",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "sterile_workdir",
        true,
        "runner_audit_opencode_workdir_not_sterile",
        "runner_audit.sterile_workdir",
        "opencode-run runner audits must run from a sterile workdir rather than the proposal repo or chat workspace",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "tools_disabled",
        true,
        "runner_audit_opencode_tools_enabled",
        "runner_audit.tools_disabled",
        "opencode-run runner audits must run with a no-tool/no-permission agent for normalization",
        issues,
        blocked,
        valid,
    );
}

fn validate_custom_headless_runner(
    value: &Value,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
    valid: &mut bool,
) {
    require_bool(
        value,
        "session_persistence",
        false,
        "runner_audit_custom_session_persistence",
        "runner_audit.session_persistence",
        "custom-headless runner audits must report no session persistence",
        issues,
        blocked,
        valid,
    );
    require_bool(
        value,
        "tools_disabled",
        true,
        "runner_audit_custom_tools_enabled",
        "runner_audit.tools_disabled",
        "custom-headless runner audits must report tools disabled for normalization",
        issues,
        blocked,
        valid,
    );
}

fn require_bool(
    value: &Value,
    field: &str,
    expected: bool,
    code: &'static str,
    path: &'static str,
    message: &'static str,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
    valid: &mut bool,
) {
    if value[field].as_bool() != Some(expected) {
        *valid = false;
        *blocked = true;
        issues.push(issue(code, "error", path, message));
    }
}

fn validate_runner_required_string(
    value: &Value,
    field: &str,
    expected: &str,
    code: &'static str,
    path: &'static str,
    issues: &mut Vec<Value>,
    blocked: &mut bool,
    valid: &mut bool,
) {
    if value[field].as_str() != Some(expected) {
        *valid = false;
        *blocked = true;
        issues.push(issue(
            code,
            "error",
            path,
            format!("runner-audit {field} must be {expected}"),
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
            runner_audit: None,
            require_runner_audit: false,
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
            runner_audit: None,
            require_runner_audit: false,
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
            runner_audit: None,
            require_runner_audit: false,
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
            runner_audit: None,
            require_runner_audit: false,
            artifacts: &[],
        })
        .expect("receipt should build");

        assert_eq!(receipt["valid"], true);
        assert_eq!(receipt["decision"], "audit-grade");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn required_claude_bare_runner_audit_is_headless_verified() {
        let root = test_pack("receipt-claude-runner-audit");
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
        let runner_audit = write_json(
            &root,
            "runner-audit.json",
            json!({
                "contract": "mdp.runner-audit.v0",
                "runner": "claude-print",
                "model": "claude-sonnet-5",
                "isolated_invocation": true,
                "conversation_resume": false,
                "declared_inputs_only": true,
                "output_schema_used": true,
                "bare": true,
                "session_persistence": false,
                "tools_disabled": true,
                "tool_invocations_observed": 0
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
            runner_audit: Some(&runner_audit),
            require_runner_audit: true,
            artifacts: &[],
        })
        .expect("receipt should build");

        assert_eq!(receipt["valid"], true);
        assert_eq!(receipt["decision"], "audit-grade");
        assert_eq!(receipt["runner"]["assurance"], "headless-verified");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_required_runner_audit_blocks_receipt() {
        let root = test_pack("receipt-missing-runner-audit");
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
            isolation: RunIsolation::Isolated,
            declared_inputs_only: true,
            prompt_id: Some("normalize-opportunity"),
            prompt_output: Some(&prompt_output),
            validation: Some(&validation),
            source_audit: Some(&source_audit),
            runner_audit: None,
            require_runner_audit: true,
            artifacts: &[],
        })
        .expect("receipt should build");

        assert_eq!(receipt["valid"], false);
        assert_eq!(receipt["decision"], "blocked");
        assert_eq!(receipt["runner"]["assurance"], "missing");
        assert!(
            receipt["issues"]
                .as_array()
                .expect("issues")
                .iter()
                .any(|issue| issue["code"] == "runner_audit_artifact_missing")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn codex_runner_audit_without_prompt_input_audit_blocks_receipt() {
        let root = test_pack("receipt-codex-runner-audit-invalid");
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
        let runner_audit = write_json(
            &root,
            "runner-audit.json",
            json!({
                "contract": "mdp.runner-audit.v0",
                "runner": "codex-exec",
                "isolated_invocation": true,
                "conversation_resume": false,
                "declared_inputs_only": true,
                "output_schema_used": true,
                "ephemeral": true,
                "session_persistence": false,
                "sterile_workdir": true,
                "prompt_input_audited": false,
                "config_discovery_disabled": true,
                "instructions_discovery_disabled": true,
                "sandbox": "read-only",
                "tool_invocations_observed": 0
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
            runner_audit: Some(&runner_audit),
            require_runner_audit: true,
            artifacts: &[],
        })
        .expect("receipt should build");

        assert_eq!(receipt["valid"], false);
        assert_eq!(receipt["decision"], "blocked");
        assert_eq!(receipt["runner"]["assurance"], "invalid");
        assert!(
            receipt["issues"]
                .as_array()
                .expect("issues")
                .iter()
                .any(|issue| issue["code"] == "runner_audit_codex_prompt_input_not_audited")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cursor_runner_audit_requires_no_force_and_disabled_tools() {
        let root = test_pack("receipt-cursor-runner-audit-invalid");
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
        let runner_audit = write_json(
            &root,
            "runner-audit.json",
            json!({
                "contract": "mdp.runner-audit.v0",
                "runner": "cursor-print",
                "isolated_invocation": true,
                "conversation_resume": false,
                "declared_inputs_only": true,
                "output_schema_used": true,
                "session_persistence": false,
                "force_enabled": true,
                "sterile_workdir": true,
                "prompt_input_audited": true,
                "tools_disabled": false,
                "tool_invocations_observed": 0
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
            runner_audit: Some(&runner_audit),
            require_runner_audit: true,
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
                .any(|issue| issue["code"] == "runner_audit_cursor_force_enabled")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn opencode_runner_audit_requires_pure_no_tool_mode() {
        let root = test_pack("receipt-opencode-runner-audit-invalid");
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
        let runner_audit = write_json(
            &root,
            "runner-audit.json",
            json!({
                "contract": "mdp.runner-audit.v0",
                "runner": "opencode-run",
                "isolated_invocation": true,
                "conversation_resume": false,
                "declared_inputs_only": true,
                "output_schema_used": true,
                "session_persistence": false,
                "pure": false,
                "default_plugins_disabled": true,
                "claude_code_discovery_disabled": true,
                "sterile_workdir": true,
                "tools_disabled": true,
                "tool_invocations_observed": 0
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
            runner_audit: Some(&runner_audit),
            require_runner_audit: true,
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
                .any(|issue| issue["code"] == "runner_audit_opencode_not_pure")
        );

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
