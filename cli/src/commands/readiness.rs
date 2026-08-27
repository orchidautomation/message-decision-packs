use crate::artifact_hash::canonical_json_sha256;
use crate::cli::SchemaTarget;
use crate::commands::health::{doctor, validate_pack};
use crate::commands::requirements::requirements;
use crate::commands::routing::route_budget_preflight_query_command;
use crate::commands::skills::skills;
use crate::constants::PROMPT_OUTPUT_VALIDATION_CONTRACT;
use crate::routing::RouteBudgetQuery;
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::Path;

pub(crate) const READINESS_CONTRACT: &str = "mdp.readiness.v1";
const MAX_INPUT_VALIDATION_BYTES: u64 = 1_048_576;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    True,
    False,
    Unknown,
    NotApplicable,
}

impl State {
    fn as_str(self) -> &'static str {
        match self {
            Self::True => "true",
            Self::False => "false",
            Self::Unknown => "unknown",
            Self::NotApplicable => "not-applicable",
        }
    }
}

struct Gate {
    id: &'static str,
    state: State,
    authority: &'static str,
    reason_code: &'static str,
    message: &'static str,
    next_action: &'static str,
}

impl Gate {
    fn value(&self) -> Value {
        json!({
            "state": self.state.as_str(),
            "authority": self.authority,
            "reason_code": self.reason_code
        })
    }

    fn list_value(&self) -> Value {
        json!({
            "id": self.id,
            "state": self.state.as_str(),
            "authority": self.authority,
            "reason_code": self.reason_code
        })
    }
}

/// Additive, read-only composer. Every state is a monotonic projection of an
/// existing command result; this module deliberately owns no validation or
/// drafting authority of its own.
pub(crate) fn readiness(
    root: &Path,
    requested_job: Option<&str>,
    input_validation_path: Option<&Path>,
) -> Value {
    let doctor_result = doctor(root);
    let installation = Gate {
        id: "installation_version",
        state: State::True,
        authority: "mdp.cli-runtime.v1",
        reason_code: "cli_running",
        message: "The local CLI is unavailable.",
        next_action: "Install or repair the local mdp CLI.",
    };

    let validation_result = validate_pack(root);
    let structurally_valid = match validation_result.as_ref() {
        Ok(value) if value["valid"] == true => Gate {
            id: "structurally_valid",
            state: State::True,
            authority: "mdp.validate.v0",
            reason_code: "pack_structurally_valid",
            message: "The pack is structurally invalid.",
            next_action: "Run `mdp validate --dir PACK_ROOT` and repair the first error.",
        },
        Ok(_) => Gate {
            id: "structurally_valid",
            state: State::False,
            authority: "mdp.validate.v0",
            reason_code: "pack_structurally_invalid",
            message: "The pack is not structurally valid.",
            next_action: "Run `mdp validate --dir PACK_ROOT` and repair the first error.",
        },
        Err(_) => Gate {
            id: "structurally_valid",
            state: State::False,
            authority: "mdp.validate.v0",
            reason_code: "pack_validation_unavailable",
            message: "The pack cannot be loaded for structural validation.",
            next_action: "Select a readable pack containing `.mdp/manifest.yaml`.",
        },
    };

    let skills_result = skills(Some(root), requested_job);
    let profile_activation = match skills_result["profile_activation"]["status"].as_str() {
        Some("ready") => gate_true("profile_activation", "mdp.skills.v1", "profile_active"),
        Some("blocked") | Some("unresolved") => Gate {
            id: "profile_activation",
            state: State::False,
            authority: "mdp.skills.v1",
            reason_code: "profile_activation_blocked",
            message: "The selected profile is not active for this pack and job.",
            next_action: "Resolve the profile activation diagnostic reported by `mdp skills`.",
        },
        _ if skills_result["profile"].is_null() => Gate {
            id: "profile_activation",
            state: State::NotApplicable,
            authority: "mdp.skills.v1",
            reason_code: "profile_not_declared",
            message: "No profile is declared.",
            next_action: "Select a profile-aware pack before choosing a job.",
        },
        _ => Gate {
            id: "profile_activation",
            state: State::Unknown,
            authority: "mdp.skills.v1",
            reason_code: "profile_activation_unknown",
            message: "Profile activation has not been established.",
            next_action: "Run `mdp skills --dir PACK_ROOT` and resolve its first diagnostic.",
        },
    };

    let requirements_result = requested_job.and_then(|job| requirements(root, job).ok());
    let job_ready = match (requested_job, requirements_result.as_ref()) {
        (None, _) => Gate {
            id: "job_ready",
            state: State::NotApplicable,
            authority: "mdp.requirements.v1",
            reason_code: "job_not_selected",
            message: "No job was selected.",
            next_action: "Choose an exact jobs[].id and rerun `mdp check --job JOB_ID`.",
        },
        (Some(_), Some(value))
            if value["status"] == "ready" && profile_activation.state != State::False =>
        {
            gate_true("job_ready", "mdp.requirements.v1", "job_ready")
        }
        (Some(_), Some(value)) if value["status"] == "unavailable" => Gate {
            id: "job_ready",
            state: State::Unknown,
            authority: "mdp.requirements.v1",
            reason_code: "job_readiness_unavailable",
            message: "The selected job has no compiled readiness result.",
            next_action: "Inspect `mdp requirements --dir PACK_ROOT --job JOB_ID`.",
        },
        (Some(_), Some(_)) => Gate {
            id: "job_ready",
            state: State::False,
            authority: "mdp.requirements.v1",
            reason_code: "job_readiness_blocked",
            message: "The selected job is not ready.",
            next_action: "Resolve the first diagnostic from `mdp requirements --dir PACK_ROOT --job JOB_ID`.",
        },
        (Some(_), None) => Gate {
            id: "job_ready",
            state: State::False,
            authority: "mdp.requirements.v1",
            reason_code: "job_not_found_or_unavailable",
            message: "The selected job cannot be compiled from this pack.",
            next_action: "Choose an exact jobs[].id reported by `mdp skills --dir PACK_ROOT`.",
        },
    };

    let route_budget_result = requested_job.and_then(|job| {
        route_budget_preflight_query_command(
            root,
            false,
            RouteBudgetQuery {
                job_id: Some(job.to_string()),
                persona: None,
            },
        )
        .ok()
    });
    let route_budget = budget_gate(requested_job, route_budget_result.as_ref());
    let input_validation_result = input_validation_path.and_then(read_input_validation);
    let input_ready = input_gate(
        requirements_result.as_ref(),
        requested_job,
        input_validation_path.is_some(),
        input_validation_result.as_ref(),
    );

    let safe_state = if requested_job.is_none() {
        State::NotApplicable
    } else {
        combine(&[
            structurally_valid.state,
            profile_activation.state,
            job_ready.state,
            route_budget.state,
            input_ready.state,
        ])
    };
    let safe = Gate {
        id: "safe_to_draft_or_act",
        state: safe_state,
        authority: "mdp.readiness.v1 projection",
        reason_code: match safe_state {
            State::True => "all_applicable_gates_ready",
            State::False => "one_or_more_gates_blocked",
            State::Unknown => "one_or_more_gates_unknown",
            State::NotApplicable => "job_not_selected",
        },
        message: "The pack is not yet safe to draft from or act on.",
        next_action: "Resolve the first blocker before drafting or acting.",
    };

    let gates = vec![
        installation,
        structurally_valid,
        profile_activation,
        job_ready,
        route_budget,
        input_ready,
        safe,
    ];
    let first = gates
        .iter()
        .take(6)
        .find(|gate| matches!(gate.state, State::False | State::Unknown));
    let next_action = first.map_or_else(
        || {
            if requested_job.is_none() {
                "Choose a job to assess draft/action safety."
            } else {
                "Proceed using the selected job's existing governed workflow."
            }
        },
        |gate| gate.next_action,
    );
    let status = if gates.iter().take(6).any(|gate| gate.state == State::False) {
        "blocked"
    } else if gates
        .iter()
        .take(6)
        .any(|gate| gate.state == State::Unknown)
    {
        "unknown"
    } else {
        "ready"
    };

    let diagnostics = gates
        .iter()
        .take(6)
        .filter(|gate| matches!(gate.state, State::False | State::Unknown))
        .map(|gate| json!({"code": gate.reason_code, "severity": if gate.state == State::False { "error" } else { "warning" }}))
        .collect::<Vec<_>>();
    let contributors = contributor_projection(
        &doctor_result,
        validation_result.as_ref().ok(),
        &skills_result,
        requirements_result.as_ref(),
        route_budget_result.as_ref(),
        input_validation_path.is_some(),
        input_validation_result.as_ref(),
    );
    json!({
        "contract": READINESS_CONTRACT,
        "status": status,
        "read_only": true,
        "offline": true,
        "runtime": {"tool": "mdp", "version": env!("CARGO_PKG_VERSION")},
        "selection": {"job_id": requested_job},
        "structurally_valid": gates[1].value(),
        "job_ready": gates[3].value(),
        "input_ready": gates[5].value(),
        "safe_to_draft_or_act": gates[6].value(),
        "gates": gates.iter().map(Gate::list_value).collect::<Vec<_>>(),
        "first_blocker": first.map(|gate| json!({
            "gate": gate.id,
            "reason_code": gate.reason_code,
            "message": gate.message
        })),
        "next_action": next_action,
        "contributors": contributors,
        "diagnostics": diagnostics
    })
}

fn gate_true(id: &'static str, authority: &'static str, reason: &'static str) -> Gate {
    Gate {
        id,
        state: State::True,
        authority,
        reason_code: reason,
        message: "This gate is ready.",
        next_action: "Continue.",
    }
}

fn budget_gate(job: Option<&str>, value: Option<&Value>) -> Gate {
    if job.is_none() {
        return Gate {
            id: "route_budget",
            state: State::NotApplicable,
            authority: "mdp.route-budget.v0",
            reason_code: "job_not_selected",
            message: "No job was selected for route-budget assessment.",
            next_action: "Choose a job before assessing its route budget.",
        };
    }
    let Some(value) = value else {
        return Gate {
            id: "route_budget",
            state: State::Unknown,
            authority: "mdp.route-budget.v0",
            reason_code: "route_budget_unavailable",
            message: "Route-budget readiness could not be projected.",
            next_action: "Run `mdp route-budget --dir PACK_ROOT --job JOB_ID`.",
        };
    };
    let statuses = value["routes"].as_array().cloned().unwrap_or_default();
    if statuses.iter().any(|route| route["status"] == "blocked") || value["valid"] == false {
        Gate {
            id: "route_budget",
            state: State::False,
            authority: "mdp.route-budget.v0",
            reason_code: "route_budget_blocked",
            message: "At least one selected route exceeds or loses required context authority.",
            next_action: "Narrow applicability or increase the declared budget, then rerun route-budget.",
        }
    } else if statuses.is_empty() {
        Gate {
            id: "route_budget",
            state: State::NotApplicable,
            authority: "mdp.route-budget.v0",
            reason_code: "route_budget_not_applicable",
            message: "The selected job has no route-budget projection.",
            next_action: "Continue with the job's other gates.",
        }
    } else if statuses.iter().any(|route| route["status"] == "unassessed") {
        Gate {
            id: "route_budget",
            state: State::Unknown,
            authority: "mdp.route-budget.v0",
            reason_code: "route_budget_unassessed",
            message: "The selected job's context budget is unassessed.",
            next_action: "Declare a context_budget for the job and rerun route-budget.",
        }
    } else {
        gate_true("route_budget", "mdp.route-budget.v0", "route_budget_ready")
    }
}

fn input_gate(
    requirements: Option<&Value>,
    requested_job: Option<&str>,
    input_supplied: bool,
    validation: Option<&Value>,
) -> Gate {
    let governed = requirements.is_some_and(|value| value["available"] == true);
    if !governed {
        return Gate {
            id: "input_ready",
            state: State::NotApplicable,
            authority: "mdp.requirements.v1",
            reason_code: "governed_input_not_required",
            message: "This job does not expose a governed input contract.",
            next_action: "Continue with the job's other gates.",
        };
    }
    if !input_supplied {
        return Gate {
            id: "input_ready",
            state: State::Unknown,
            authority: "mdp.requirements.v1",
            reason_code: "governed_input_not_assessed",
            message: "The selected job requires governed input, but no validation result was supplied.",
            next_action: "Validate the normalized input, then rerun `mdp check --input-validation RESULT.json`.",
        };
    }
    match validation {
        Some(value)
            if validation_result_is_bound(value, requirements, requested_job)
                && value["valid"] == true
                && value["authority"]["validation_state"] == "valid"
                && value["authority"]["decision_state"] == "available" =>
        {
            gate_true(
                "input_ready",
                PROMPT_OUTPUT_VALIDATION_CONTRACT,
                "governed_input_valid",
            )
        }
        Some(value) if validation_result_is_bound(value, requirements, requested_job) => Gate {
            id: "input_ready",
            state: State::False,
            authority: PROMPT_OUTPUT_VALIDATION_CONTRACT,
            reason_code: "governed_input_invalid",
            message: "The supplied governed-input validation result is invalid.",
            next_action: "Resolve the validation result's first diagnostic and validate again.",
        },
        _ => Gate {
            id: "input_ready",
            state: State::False,
            authority: PROMPT_OUTPUT_VALIDATION_CONTRACT,
            reason_code: "input_validation_unrecognized",
            message: "The supplied file is not a bounded prompt-output validation result.",
            next_action: "Supply JSON emitted by `mdp --json validate-prompt-output`.",
        },
    }
}

fn read_input_validation(path: &Path) -> Option<Value> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_INPUT_VALIDATION_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(MAX_INPUT_VALIDATION_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_INPUT_VALIDATION_BYTES {
        return None;
    }
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    let mut result = if value["data"].is_object() {
        value["data"].clone()
    } else {
        value
    };
    if let Some(object) = result.as_object_mut() {
        object.remove("diagnostic_contract");
        object.remove("actionable_diagnostics");
    }
    Some(result)
}

fn validation_result_is_bound(
    value: &Value,
    requirements: Option<&Value>,
    requested_job: Option<&str>,
) -> bool {
    if jsonschema::draft202012::validate(
        &crate::commands::schemas::schema(SchemaTarget::PromptOutputValidationV1),
        value,
    )
    .is_err()
        || value["authority"]["job_id"].as_str() != requested_job
        || requirements
            .is_none_or(|result| value["authority"]["pack"]["sha256"] != result["pack"]["sha256"])
    {
        return false;
    }
    let mut authority = value["authority"].clone();
    let claimed = authority["binding_sha256"].as_str().map(str::to_string);
    authority
        .as_object_mut()
        .expect("validated authority is an object")
        .remove("binding_sha256");
    claimed
        .is_some_and(|claimed| canonical_json_sha256(&authority).ok().as_deref() == Some(&claimed))
}

fn combine(states: &[State]) -> State {
    if states.contains(&State::False) {
        State::False
    } else if states.contains(&State::Unknown) {
        State::Unknown
    } else {
        State::True
    }
}

fn contributor_projection(
    doctor: &Value,
    validation: Option<&Value>,
    skills: &Value,
    requirements: Option<&Value>,
    budget: Option<&Value>,
    input_supplied: bool,
    input_validation: Option<&Value>,
) -> Vec<Value> {
    let mut values = vec![
        json!({"contract": "mdp.cli-runtime.v1", "observed": {"tool": "mdp", "version": env!("CARGO_PKG_VERSION")}}),
        json!({"contract": "mdp.doctor.v0", "observed": {"valid": doctor["valid"]}}),
        json!({"contract": "mdp.validate.v0", "observed": {"valid": validation.map(|v| v["valid"].clone()).unwrap_or(Value::Null)}}),
        json!({"contract": "mdp.skills.v1", "observed": {"status": skills["status"], "profile_activation_status": skills["profile_activation"]["status"]}}),
    ];
    if let Some(value) = requirements {
        values.push(json!({"contract": value["contract"], "observed": {"status": value["status"], "valid": value["valid"], "available": value["available"], "draft_allowed": value["draft_allowed"]}}));
    }
    if let Some(value) = budget {
        values.push(json!({"contract": "mdp.route-budget.v0", "observed": {"valid": value["valid"], "matched_route_count": value["query"]["matched_route_count"], "overflow_count": value["overflow_count"], "unassessed_generation_count": value["unassessed_generation_count"]}}));
    }
    if input_supplied {
        values.push(match input_validation {
            Some(value) => json!({"contract": PROMPT_OUTPUT_VALIDATION_CONTRACT, "observed": {
                "supplied": true,
                "valid": value["valid"],
                "job_id": value["authority"]["job_id"],
                "decision_state": value["authority"]["decision_state"],
                "binding_sha256": value["authority"]["binding_sha256"]
            }}),
            None => json!({"contract": PROMPT_OUTPUT_VALIDATION_CONTRACT, "observed": {"supplied": true, "recognized": false}}),
        });
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::SchemaTarget;
    use crate::commands::schemas::schema;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn template_root() -> &'static Path {
        Path::new("../plugin/assets/templates/basic")
    }

    fn temporary_root(label: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("mdp-readiness-{label}-{suffix}"))
    }

    #[test]
    fn fixture_matrix_preserves_tristate_and_first_blocker_order() {
        let generic = readiness(template_root(), None, None);
        assert_eq!(generic["status"], "ready");
        assert_eq!(generic["job_ready"]["state"], "not-applicable");
        assert_eq!(generic["input_ready"]["state"], "not-applicable");

        let missing_input = readiness(template_root(), Some("outbound-copy-brief"), None);
        assert_eq!(missing_input["status"], "unknown");
        assert_eq!(missing_input["input_ready"]["state"], "unknown");
        assert_eq!(missing_input["first_blocker"]["gate"], "input_ready");

        let unknown_job = readiness(template_root(), Some("not-a-job"), None);
        assert_eq!(unknown_job["status"], "blocked");
        assert_eq!(unknown_job["first_blocker"]["gate"], "job_ready");

        let absent = temporary_root("absent");
        let invalid = readiness(&absent, Some("outbound-copy-brief"), None);
        assert_eq!(invalid["first_blocker"]["gate"], "structurally_valid");

        jsonschema::draft202012::validate(&schema(SchemaTarget::ReadinessV1), &missing_input)
            .expect("readiness fixture should match its schema");
    }

    #[test]
    fn valid_input_validation_makes_fully_ready_fixture_ready() {
        let root = temporary_root("input");
        fs::create_dir_all(&root).unwrap();
        let file = root.join("validation.json");
        let requirements = requirements(template_root(), "outbound-copy-brief").unwrap();
        let mut authority = json!({
            "pack": {"id": "basic-mdp-template", "version": "0.1.0", "sha256": requirements["pack"]["sha256"]},
            "prompt": {"id": "generate-outbound-copy-v1", "version": "v1", "sha256": "a".repeat(64)},
            "job_id": "outbound-copy-brief",
            "input_artifacts": [],
            "prompt_output_sha256": "b".repeat(64),
            "validation_state": "valid",
            "decision_state": "available"
        });
        authority["binding_sha256"] = json!(canonical_json_sha256(&authority).unwrap());
        let result = json!({
            "contract": PROMPT_OUTPUT_VALIDATION_CONTRACT,
            "valid": true,
            "file": "redacted",
            "prompt": {"id": "generate-outbound-copy-v1", "output_kind": "governed-artifact", "target_card_kinds": [], "declared_inputs": [], "pack_dir": "redacted"},
            "artifacts": {"prompt_output": {"path": "redacted", "sha256": "b".repeat(64)}},
            "issues": [],
            "authority": authority
        });
        fs::write(&file, serde_json::to_vec(&result).unwrap()).unwrap();

        let result = readiness(template_root(), Some("outbound-copy-brief"), Some(&file));
        assert_eq!(result["status"], "ready", "{result}");
        assert_eq!(result["safe_to_draft_or_act"]["state"], "true");
        assert!(
            result
                .to_string()
                .find(file.to_string_lossy().as_ref())
                .is_none()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unbound_or_partial_validation_result_never_grants_input_readiness() {
        let root = temporary_root("partial-input");
        fs::create_dir_all(&root).unwrap();
        let file = root.join("validation.json");
        fs::write(
            &file,
            serde_json::to_vec(&json!({
                "contract": PROMPT_OUTPUT_VALIDATION_CONTRACT,
                "valid": true
            }))
            .unwrap(),
        )
        .unwrap();
        let result = readiness(template_root(), Some("outbound-copy-brief"), Some(&file));
        assert_eq!(result["status"], "blocked");
        assert_eq!(result["input_ready"]["state"], "false");
        assert_eq!(
            result["first_blocker"]["reason_code"],
            "input_validation_unrecognized"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn budget_block_is_projected_without_reinterpreting_authority() {
        let root = temporary_root("budget");
        crate::commands::init::init_pack(&root, "Example Message Pack", "gtm", true, false)
            .unwrap();
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = fs::read_to_string(&manifest_path).unwrap();
        fs::write(
            &manifest_path,
            raw.replacen("max_entries: 52", "max_entries: 1", 1),
        )
        .unwrap();
        let result = readiness(&root, Some("outbound-copy-brief"), None);
        assert_eq!(result["status"], "blocked", "{result}");
        assert_eq!(result["first_blocker"]["gate"], "route_budget");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn target_aware_profile_block_precedes_downstream_gates() {
        let root = temporary_root("profile-block");
        crate::commands::init::init_pack(&root, "Example Message Pack", "gtm", true, false)
            .unwrap();
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = fs::read_to_string(&manifest_path).unwrap();
        fs::write(
            &manifest_path,
            raw.replace(
                "activation:\n    status: ready",
                "activation:\n    status: blocked",
            ),
        )
        .unwrap();
        let result = readiness(&root, Some("outbound-copy-brief"), None);
        assert_eq!(result["status"], "blocked", "{result}");
        assert_eq!(result["first_blocker"]["gate"], "profile_activation");
        fs::remove_dir_all(root).unwrap();
    }
}
