use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

pub(crate) const ACTIONABLE_DIAGNOSTIC_CONTRACT: &str = "mdp.actionable-diagnostic.v1";
pub(crate) const ACTIONABLE_DIAGNOSTICS_FIELD: &str = "actionable_diagnostics";
const MAX_DIAGNOSTICS: usize = 32;

const TARGET_COMMANDS: &[&str] = &[
    "init",
    "doctor",
    "validate",
    "skills",
    "requirements",
    "prepare-run",
    "run",
    "run-preflight",
    "verify-run",
    "check-claims",
    "route",
    "route-budget",
    "fit",
    "brief",
    "emit-brief",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Retryability {
    AfterUserAction,
    Transient,
    NotRetryable,
}

#[derive(Debug, Clone, Serialize)]
struct Prerequisite {
    kind: &'static str,
    summary: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct NextAction {
    kind: &'static str,
    summary: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct ActionableDiagnostic {
    contract: &'static str,
    phase: &'static str,
    code: String,
    retryability: Retryability,
    summary: String,
    prerequisites: Vec<Prerequisite>,
    next_action: NextAction,
}

/// Add the versioned diagnostic projection to public command results without
/// replacing their established low-level issue and authority fields.
pub(crate) fn enrich_result(command: &str, data: &mut Value) {
    if !TARGET_COMMANDS.contains(&command) || !data.is_object() {
        return;
    }
    let mut raw = Vec::new();
    collect_legacy_diagnostics(data, &mut raw, 0);
    if raw.is_empty() && result_is_blocked(data) {
        raw.push(fallback_code(command).to_string());
    }
    let diagnostics = project_codes(command, raw);
    if let Some(object) = data.as_object_mut() {
        object.insert(
            "diagnostic_contract".to_string(),
            json!(ACTIONABLE_DIAGNOSTIC_CONTRACT),
        );
        object.insert(
            ACTIONABLE_DIAGNOSTICS_FIELD.to_string(),
            serde_json::to_value(diagnostics).unwrap_or_else(|_| json!([])),
        );
    }
}

pub(crate) fn error_diagnostic(code: &str) -> Value {
    serde_json::to_value(project("command", code)).unwrap_or_else(|_| {
        json!({
            "contract": ACTIONABLE_DIAGNOSTIC_CONTRACT,
            "phase": "input",
            "code": "mdp_error",
            "retryability": "after-user-action",
            "summary": "The command could not complete.",
            "prerequisites": [],
            "next_action": {"kind": "manual", "summary": "Review the bounded error details and correct the command input."}
        })
    })
}

pub(crate) fn render_human(diagnostics: &Value) {
    render(diagnostics, false);
}

pub(crate) fn render_human_error(diagnostics: &Value) {
    render(diagnostics, true);
}

fn render(diagnostics: &Value, stderr: bool) {
    let Some(items) = diagnostics.as_array() else {
        return;
    };
    for item in items {
        let header = format!(
            "diagnostic [{} / {}]: {}",
            item["phase"].as_str().unwrap_or("input"),
            item["code"].as_str().unwrap_or("mdp_error"),
            item["summary"].as_str().unwrap_or("Action is required.")
        );
        if stderr {
            eprintln!("{header}");
        } else {
            println!("{header}");
        }
        if let Some(prerequisites) = item["prerequisites"].as_array() {
            for prerequisite in prerequisites {
                let line = format!(
                    "  prerequisite: {}",
                    prerequisite["summary"]
                        .as_str()
                        .unwrap_or("Operator review is required.")
                );
                if stderr {
                    eprintln!("{line}");
                } else {
                    println!("{line}");
                }
            }
        }
        let next = if let Some(command) = item["next_action"]["command"].as_str() {
            Some(command)
        } else if let Some(summary) = item["next_action"]["summary"].as_str() {
            Some(summary)
        } else {
            None
        };
        if let Some(next) = next {
            if stderr {
                eprintln!("  next: {next}");
            } else {
                println!("  next: {next}");
            }
        }
    }
}

pub(crate) fn contract_metadata() -> Value {
    json!({
        "id": ACTIONABLE_DIAGNOSTIC_CONTRACT,
        "compatibility": "additive-v1",
        "legacy_diagnostics_preserved": true,
        "required": ["contract", "phase", "code", "retryability", "summary", "prerequisites", "next_action"],
        "retryability": ["after-user-action", "transient", "not-retryable"],
        "exact_command_policy": "omit command unless the command is complete, bounded, and safe"
    })
}

fn project_codes(command: &str, codes: Vec<String>) -> Vec<ActionableDiagnostic> {
    let mut seen = BTreeSet::new();
    codes
        .into_iter()
        .map(|code| stable_code(&code))
        .filter(|code| seen.insert(code.clone()))
        .take(MAX_DIAGNOSTICS)
        .map(|code| project(command, &code))
        .collect()
}

fn project(command: &str, raw_code: &str) -> ActionableDiagnostic {
    let code = stable_code(raw_code);
    let class = diagnostic_class(&code);
    let (retryability, summary, prerequisites, next_action) = match class {
        DiagnosticClass::InvalidInput => (
            Retryability::AfterUserAction,
            "The command arguments are incomplete or unsupported.",
            vec![Prerequisite {
                kind: "operator-input",
                summary: "Provide only supported arguments and required values.",
            }],
            NextAction {
                kind: "command",
                summary: "Inspect the supported CLI surface.",
                command: Some("mdp --help"),
            },
        ),
        DiagnosticClass::Pack => (
            Retryability::AfterUserAction,
            "The selected MDP pack or one of its declared files is unavailable.",
            vec![Prerequisite {
                kind: "pack",
                summary: "Select a readable pack whose .mdp/manifest.yaml and declared files exist.",
            }],
            manual_action(
                "Select or repair the pack before retrying; no write command is inferred.",
            ),
        ),
        DiagnosticClass::MissingAuthority => (
            Retryability::AfterUserAction,
            "Required structured decision authority is missing or unassessed.",
            vec![Prerequisite {
                kind: "authority",
                summary: "Add or select the required structured authority; README prose cannot satisfy it.",
            }],
            manual_action("Resolve the named authority gap before retrying."),
        ),
        DiagnosticClass::Policy => (
            Retryability::NotRetryable,
            "The governing policy or authority refused this operation.",
            vec![Prerequisite {
                kind: "policy",
                summary: "A changed request or changed governing authority is required; an unchanged retry cannot override policy.",
            }],
            manual_action("Do not retry unchanged input; review the policy refusal."),
        ),
        DiagnosticClass::Transient => (
            Retryability::Transient,
            "Execution did not complete because a runtime dependency was unavailable or timed out.",
            vec![Prerequisite {
                kind: "runtime",
                summary: "Confirm the local runtime and declared files are available and unchanged.",
            }],
            manual_action("Retry only after the runtime prerequisite is restored."),
        ),
        DiagnosticClass::Other => (
            Retryability::AfterUserAction,
            "The command requires operator review before it can continue.",
            Vec::new(),
            manual_action(
                "Review the bounded low-level diagnostic; no repair command is inferred.",
            ),
        ),
    };
    ActionableDiagnostic {
        contract: ACTIONABLE_DIAGNOSTIC_CONTRACT,
        phase: phase(command, &code),
        code,
        retryability,
        summary: summary.to_string(),
        prerequisites,
        next_action,
    }
}

fn manual_action(summary: &'static str) -> NextAction {
    NextAction {
        kind: "manual",
        summary,
        command: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiagnosticClass {
    InvalidInput,
    Pack,
    MissingAuthority,
    Policy,
    Transient,
    Other,
}

fn diagnostic_class(code: &str) -> DiagnosticClass {
    if code == "invalid_argument" || code == "output_mode_conflict" {
        DiagnosticClass::InvalidInput
    } else if code.contains("pack")
        || code.contains("manifest")
        || code.contains("missing_card")
        || code == "write_conflict"
    {
        DiagnosticClass::Pack
    } else if code.contains("policy")
        || code.contains("unsupported_claim")
        || code.contains("disqualified")
        || code.contains("do_not_contact")
    {
        DiagnosticClass::Policy
    } else if code.contains("timeout")
        || code.contains("runner_failed")
        || code.contains("runner-failed")
        || code.contains("unavailable")
        || code.contains("transport")
    {
        DiagnosticClass::Transient
    } else if code.contains("missing")
        || code.contains("gap")
        || code.contains("unassessed")
        || code.contains("insufficient")
        || code.contains("not_ready")
        || code.contains("not-ready")
        || code.contains("blocked")
    {
        DiagnosticClass::MissingAuthority
    } else {
        DiagnosticClass::Other
    }
}

fn phase(command: &str, code: &str) -> &'static str {
    match diagnostic_class(code) {
        DiagnosticClass::InvalidInput => return "input",
        DiagnosticClass::Policy => return "policy",
        DiagnosticClass::Transient => return "execution",
        DiagnosticClass::Pack if command == "command" => return "setup",
        DiagnosticClass::MissingAuthority if command == "command" => return "readiness",
        _ => {}
    }
    match command {
        "init" | "doctor" => "setup",
        "validate" => "validation",
        "skills" | "requirements" => "readiness",
        "prepare-run" | "run" | "run-preflight" | "verify-run" => "execution",
        "check-claims" => "policy",
        "route" | "route-budget" | "fit" | "brief" | "emit-brief" => "routing",
        _ => "input",
    }
}

fn stable_code(raw: &str) -> String {
    let mut code = raw
        .chars()
        .take(128)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while code.contains("__") {
        code = code.replace("__", "_");
    }
    let code = code.trim_matches('_').to_string();
    if code == "cli-arguments-invalid" {
        // `prepare-run` preserves its older compiler-envelope code in the
        // low-level payload. The shared actionable projection must still use
        // the same stable input code as every other Clap failure so JSON and
        // human presentation cannot classify identical input differently.
        "invalid_argument".to_string()
    } else if code.is_empty() {
        "mdp_error".to_string()
    } else {
        code
    }
}

fn collect_legacy_diagnostics(value: &Value, codes: &mut Vec<String>, depth: usize) {
    if depth > 8 || codes.len() >= MAX_DIAGNOSTICS {
        return;
    }
    match value {
        Value::Object(object) => {
            collect_named_array(object, "issues", codes, depth);
            collect_named_array(object, "diagnostics", codes, depth);
            collect_named_array(object, "reason_codes", codes, depth);
            for (key, nested) in object {
                if !matches!(key.as_str(), "issues" | "diagnostics" | "reason_codes") {
                    collect_legacy_diagnostics(nested, codes, depth + 1);
                }
            }
        }
        Value::Array(items) => {
            for item in items.iter().take(MAX_DIAGNOSTICS) {
                collect_legacy_diagnostics(item, codes, depth + 1);
            }
        }
        _ => {}
    }
}

fn collect_named_array(
    object: &Map<String, Value>,
    key: &str,
    codes: &mut Vec<String>,
    depth: usize,
) {
    let Some(items) = object.get(key).and_then(Value::as_array) else {
        return;
    };
    for item in items.iter().take(MAX_DIAGNOSTICS) {
        if let Some(code) = item.as_str().or_else(|| item["code"].as_str()) {
            codes.push(code.to_string());
        } else {
            collect_legacy_diagnostics(item, codes, depth + 1);
        }
    }
}

fn result_is_blocked(data: &Value) -> bool {
    data["valid"].as_bool() == Some(false)
        || matches!(
            data["status"].as_str(),
            Some("blocked" | "unavailable" | "unresolved" | "needs-review")
        )
        || data["draft_status"].as_str() == Some("blocked")
        || data["terminal_state"]
            .as_str()
            .is_some_and(|state| state.starts_with("no-draft:"))
}

fn fallback_code(command: &str) -> &'static str {
    match command {
        "skills" | "requirements" => "readiness_blocked",
        "check-claims" => "claim_policy_blocked",
        "prepare-run" | "run" | "run-preflight" | "verify-run" => "execution_unavailable",
        "route" | "route-budget" | "fit" | "brief" | "emit-brief" => "governed_routing_blocked",
        _ => "command_blocked",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projections_are_bounded_closed_and_do_not_copy_private_prose() {
        let mut data = json!({
            "valid": false,
            "issues": [{
                "code": "manifest_parse_failed",
                "path": "/private/customer/acme/.mdp/manifest.yaml",
                "message": "secret customer prose at /private/customer/acme/source.txt"
            }]
        });
        enrich_result("validate", &mut data);
        let encoded = serde_json::to_string(&data["actionable_diagnostics"]).unwrap();
        assert!(!encoded.contains("private/customer"));
        assert!(!encoded.contains("secret customer"));
        assert_eq!(
            data["actionable_diagnostics"][0]["contract"],
            ACTIONABLE_DIAGNOSTIC_CONTRACT
        );
        assert_eq!(data["actionable_diagnostics"][0]["phase"], "validation");
        assert!(
            data["actionable_diagnostics"][0]["next_action"]
                .get("command")
                .is_none()
        );
    }

    #[test]
    fn policy_transient_authority_and_user_repair_are_distinct() {
        let cases = [
            ("invalid_argument", "after-user-action", "command"),
            ("missing_required_authority", "after-user-action", "manual"),
            ("run-policy-blocked", "not-retryable", "manual"),
            ("runner-timeout", "transient", "manual"),
        ];
        for (code, retryability, action_kind) in cases {
            let value = serde_json::to_value(project("run", code)).unwrap();
            assert_eq!(value["retryability"], retryability);
            assert_eq!(value["next_action"]["kind"], action_kind);
        }
    }

    #[test]
    fn compatibility_fixture_validates_against_the_v1_schema() {
        let schema: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/actionable-diagnostics/v1.schema.json"
        ))
        .unwrap();
        let fixture: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/actionable-diagnostics/valid-v1.json"
        ))
        .unwrap();
        jsonschema::draft202012::meta::validate(&schema).unwrap();
        jsonschema::draft202012::validate(&schema, &fixture).unwrap();

        let mut unsafe_command = fixture.clone();
        unsafe_command["next_action"]["command"] = json!("mdp run --request <unknown>");
        assert!(jsonschema::draft202012::validate(&schema, &unsafe_command).is_err());
    }
}
