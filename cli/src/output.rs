use crate::cli::{Commands, HumanBriefFormat, SampleLeadsFormat, TraceFormat};
use crate::routing::route_budget_summary_projection;
use anyhow::Result;
use serde_json::{Value, json};

/// Stable error code for human-only presentation flags combined with `--json`.
pub(crate) const OUTPUT_MODE_CONFLICT_CODE: &str = "output_mode_conflict";

/// Resolves the effective JSON presentation from parsed CLI state.
///
/// The runtime gate and the capabilities metadata both consume one shared
/// table so they cannot drift. The matrix is intentionally exhaustive: every
/// public presentation selector/value combination must resolve to exactly one
/// of the outcomes below. Adding a new presentation flag is a typed change in
/// this module, not free-form metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PresentationOutcome {
    /// No presentation conflict; success envelope may proceed.
    Ok,
    /// `--json` combined with a human-only presentation flag.
    Conflict {
        selector: &'static str,
        value: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisplayKind {
    Help,
    Version,
}

impl DisplayKind {
    pub(crate) fn command(&self) -> &'static str {
        match self {
            Self::Help => "help",
            Self::Version => "version",
        }
    }
}

/// One accepted value for a presentation selector. The runtime gate, the
/// capabilities metadata, and the in-source contract tests all consume this
/// single entry, so the matrix and the runtime cannot drift.
#[derive(Debug, Clone, Copy)]
struct SelectorValue {
    /// CLI value as the user types it (e.g. `"mermaid"`, `"true"`).
    name: &'static str,
    /// True when the value can coexist with global `--json`.
    json_compatible: bool,
}

impl SelectorValue {
    const fn new(name: &'static str, json_compatible: bool) -> Self {
        Self {
            name,
            json_compatible,
        }
    }
}

/// One public presentation selector. The `matcher` resolves which value is
/// active for a parsed `Commands`; returning `None` means the selector does
/// not apply to that command.
#[derive(Debug, Clone, Copy)]
struct PresentationSelector {
    /// Fully qualified selector name used in the capabilities matrix
    /// (e.g. `"trace --format"`).
    name: &'static str,
    /// Short flag surfaced in the conflict envelope
    /// (e.g. `"--format"`).
    selector: &'static str,
    /// Every public value the selector accepts, in declared order.
    values: &'static [SelectorValue],
    /// Resolves which value (by `name`) is active for a parsed `Commands`,
    /// or `None` if this selector does not apply.
    matcher: fn(&Commands) -> Option<&'static str>,
}

const fn selector(
    name: &'static str,
    selector: &'static str,
    values: &'static [SelectorValue],
    matcher: fn(&Commands) -> Option<&'static str>,
) -> PresentationSelector {
    PresentationSelector {
        name,
        selector,
        values,
        matcher,
    }
}

fn match_trace(command: &Commands) -> Option<&'static str> {
    match command {
        Commands::Trace { format, .. } => {
            if *format == TraceFormat::Mermaid {
                Some("mermaid")
            } else {
                Some("json")
            }
        }
        _ => None,
    }
}

fn match_verify_output_readable(command: &Commands) -> Option<&'static str> {
    match command {
        Commands::VerifyOutput { readable, .. } => {
            if *readable {
                Some("true")
            } else {
                Some("false")
            }
        }
        _ => None,
    }
}

fn match_render_brief_format(command: &Commands) -> Option<&'static str> {
    match command {
        Commands::RenderBrief { format, .. } => {
            if *format == HumanBriefFormat::Markdown {
                Some("markdown")
            } else {
                Some("json")
            }
        }
        _ => None,
    }
}

fn match_sample_leads_format(command: &Commands) -> Option<&'static str> {
    match command {
        Commands::SampleLeads { format, .. } => {
            if *format == SampleLeadsFormat::Yaml {
                Some("yaml")
            } else {
                Some("json")
            }
        }
        _ => None,
    }
}

fn match_brief_readable(command: &Commands) -> Option<&'static str> {
    match command {
        Commands::Brief { readable, .. } => {
            if *readable {
                Some("true")
            } else {
                Some("false")
            }
        }
        _ => None,
    }
}

/// The single source of truth for the presentation contract. The runtime
/// gate, the capabilities metadata, and the contract tests all read from this
/// array, so adding a new public presentation flag is a typed change here.
static PRESENTATION_SELECTORS: &[PresentationSelector] = &[
    selector(
        "trace --format",
        "--format",
        &[
            SelectorValue::new("json", true),
            SelectorValue::new("mermaid", false),
        ],
        match_trace,
    ),
    selector(
        "verify-output --readable",
        "--readable",
        &[
            SelectorValue::new("false", true),
            SelectorValue::new("true", false),
        ],
        match_verify_output_readable,
    ),
    selector(
        "render-brief --format",
        "--format",
        &[
            SelectorValue::new("json", true),
            SelectorValue::new("markdown", false),
        ],
        match_render_brief_format,
    ),
    selector(
        "sample-leads --format",
        "--format",
        &[
            SelectorValue::new("json", true),
            SelectorValue::new("yaml", false),
        ],
        match_sample_leads_format,
    ),
    selector(
        "brief --readable",
        "--readable",
        &[
            SelectorValue::new("false", true),
            SelectorValue::new("true", false),
        ],
        match_brief_readable,
    ),
];

fn find_selector_value(
    entry: &PresentationSelector,
    value_name: &str,
) -> Option<&'static SelectorValue> {
    entry.values.iter().find(|value| value.name == value_name)
}

/// Resolves whether a parsed CLI invocation is compatible with JSON output.
pub(crate) fn resolve_presentation(command: &Commands) -> PresentationOutcome {
    for entry in PRESENTATION_SELECTORS {
        let Some(active_name) = (entry.matcher)(command) else {
            continue;
        };
        let Some(active) = find_selector_value(entry, active_name) else {
            // The matcher returned a value name that is not in the declared
            // `values` list. Treat it as a conflict so the gate never silently
            // allows a value the contract does not advertise.
            return PresentationOutcome::Conflict {
                selector: entry.selector,
                value: active_name,
            };
        };
        if !active.json_compatible {
            return PresentationOutcome::Conflict {
                selector: entry.selector,
                value: active.name,
            };
        }
    }
    PresentationOutcome::Ok
}

/// Public selectors covered by the presentation contract. Consumed by both
/// `resolve_presentation` and the capabilities metadata projection. The data
/// is derived from the shared `PRESENTATION_SELECTORS` table.
pub(crate) fn presentation_selectors() -> Vec<(&'static str, Vec<&'static str>)> {
    PRESENTATION_SELECTORS
        .iter()
        .map(|entry| {
            (
                entry.name,
                entry.values.iter().map(|value| value.name).collect(),
            )
        })
        .collect()
}

pub(crate) fn presentation_compatibility_matrix() -> Value {
    let total_rows: usize = PRESENTATION_SELECTORS
        .iter()
        .map(|entry| entry.values.len())
        .sum();
    let mut rows: Vec<Value> = Vec::with_capacity(total_rows);
    for entry in PRESENTATION_SELECTORS {
        for value in entry.values {
            rows.push(json!({
                "selector": entry.name,
                "value": value.name,
                "json_compatible": value.json_compatible,
                "conflict_code": if value.json_compatible { Value::Null } else { json!(OUTPUT_MODE_CONFLICT_CODE) },
            }));
        }
    }
    json!({
        "selectors": rows,
        "summary_compatible": true,
        "help_envelope": "ok_with_text",
        "version_envelope": "ok_with_text",
        "display_actions": ["help", "version"]
    })
}

/// One shared, exhaustive matrix used by the runtime gate and capabilities
/// metadata. The entries enumerate every (selector, value) pair that may appear
/// in a parsed `Cli` so the contract is auditable in one place.
pub(crate) fn presentation_contract() -> Value {
    json!({
        "selectors": presentation_selectors(),
        "summary": {
            "selector": "--summary",
            "json_compatible": true,
            "envelope": "summary"
        },
        "conflict": {
            "code": OUTPUT_MODE_CONFLICT_CODE,
            "exit_code": 1,
            "stdout": "single_json_envelope",
            "stderr": "empty"
        },
        "display": {
            "help": {
                "command": "help",
                "envelope": {"ok": true, "command": "help", "data": {"text": "..."}},
                "exit_code": 0,
                "stdout": "single_json_envelope",
                "stderr": "empty"
            },
            "version": {
                "command": "version",
                "envelope": {"ok": true, "command": "version", "data": {"text": "..."}},
                "exit_code": 0,
                "stdout": "single_json_envelope",
                "stderr": "empty"
            }
        },
        "stdout_invariant": {
            "mode": "json",
            "selector": "--json",
            "value_count": 1,
            "prelude_allowed": false,
            "trailing_text_allowed": false,
            "stderr_empty": true
        }
    })
}

pub(crate) fn print_output(
    json_mode: bool,
    summary_mode: bool,
    command: &str,
    data: Value,
) -> Result<()> {
    if summary_mode {
        let summary = summarize(command, &data);
        if command == "route-budget" {
            crate::commands::schemas::validate_route_budget_summary_output(&summary)?;
        }
        if json_mode {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &json!({"ok": true, "command": command, "summary": summary})
                )?
            );
        } else {
            print_summary(command, &summary)?;
        }
        return Ok(());
    }
    if json_mode {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"ok": true, "command": command, "data": data}))?
        );
    } else {
        print_human(command, &data)?;
    }
    Ok(())
}

fn summarize(command: &str, data: &Value) -> Value {
    match command {
        "author-preview" | "author-apply" => json!({
            "contract": data["contract"],
            "status": data["status"],
            "valid": data["valid"],
            "change_set_sha256": data["change_set_sha256"],
            "change_set_path": data["change_set_path"],
            "created_count": array_len(&data["created"]),
            "changed_count": array_len(&data["changed"]),
            "unchanged_count": array_len(&data["unchanged"]),
            "deleted_count": array_len(&data["deleted"]),
            "refused_count": array_len(&data["refused"]),
            "refused_sample": data["refused"].as_array().map(|items| items.iter().take(8).cloned().collect::<Vec<_>>()).unwrap_or_default(),
            "rolled_back_count": array_len(&data["rolled_back"]),
            "rolled_back_sample": data["rolled_back"].as_array().map(|items| items.iter().take(8).cloned().collect::<Vec<_>>()).unwrap_or_default(),
            "reason_codes": data["reason_codes"],
            "private_content_included": false
        }),
        "init" => json!({
            "dry_run": data["dry_run"],
            "root": data["root"],
            "pack_dir": data["pack_dir"],
            "manifest": data["manifest"],
            "source_ledger": data["source_ledger"],
            "example_prospect": data["example_prospect"],
            "example_prospect_kind": data["example_prospect_kind"],
            "write_count": array_len(&data["write_plan"]),
            "next_commands": data["next_commands"]
        }),
        "capabilities" => json!({
            "contract": data["contract"],
            "command_count": array_len(&data["commands"]),
            "stable_error_code_count": array_len(&data["stable_error_codes"]),
            "offline_by_default": data["defaults"]["offline_by_default"]
        }),
        "conformance-compile" => json!({
            "contract": data["contract"],
            "valid": data["valid"],
            "candidate_id": data["candidate_id"],
            "job_id": data["job_id"],
            "fixture_id": data["fixture_id"],
            "challenge_id": data["challenge_id"],
            "pack_id": data["pack_release"]["pack_id"],
            "release_id": data["pack_release"]["release_id"],
            "status": data["status"],
            "behavioral_qualification_allowed": data["behavioral_qualification_allowed"],
            "evaluator_inventory_sha256": data["evaluator"]["inventory_sha256"],
            "passed_assertion_count": data["summary"]["passed"],
            "failed_assertion_count": data["summary"]["failed"],
            "unassessed_assertion_count": data["summary"]["unassessed"]
        }),
        "conformance-validate" => json!({
            "contract": data["contract"],
            "valid": data["valid"],
            "job_id": data["job_id"],
            "deterministic_status": data["deterministic_status"],
            "job_sufficiency": data["job_sufficiency"],
            "behavioral_qualification": data["behavioral_qualification"],
            "overall_result": data["overall_result"],
            "drafting_authority_granted": data["drafting_authority_granted"],
            "trial_count": array_len(&data["trials"]),
            "reason_codes": data["reason_codes"]
        }),
        "conformance-assemble" => json!({
            "contract": data["contract"],
            "candidate_id": data["candidate_id"],
            "job_id": data["job_id"],
            "fixture_id": data["fixture_id"],
            "pack_id": data["pack_release"]["pack_id"],
            "release_id": data["pack_release"]["release_id"],
            "deterministic_status": data["deterministic_status"],
            "behavioral_status": data["behavioral_status"],
            "verdict": data["verdict"],
            "candidate_sha256": data["candidate_sha256"],
            "deterministic_evaluation_sha256": data["deterministic_evaluation_sha256"],
            "behavioral_evaluation_sha256": data["behavioral_evaluation_sha256"],
            "trial_count": array_len(&data["trial_sha256s"]),
            "journey_artifact_count": array_len(&data["journey"]["artifacts"]),
            "journey_link_count": array_len(&data["journey"]["links"]),
            "limitation_count": array_len(&data["limitations"])
        }),
        "conformance-report" if data["contract"] == "mdp.public-conformance-report.v1" => json!({
            "contract": data["contract"],
            "report_id": data["report_id"],
            "pack_id": data["pack_id"],
            "release_id": data["release_id"],
            "evaluator_id": data["evaluator_id"],
            "evaluator_version": data["evaluator_version"],
            "generated_at": data["generated_at"],
            "job_count": array_len(&data["jobs"]),
            "jobs": data["jobs"].as_array().map(|jobs| jobs.iter().map(|job| json!({
                "job_id": job["job_id"],
                "deterministic_status": job["deterministic_status"],
                "behavioral_status": job["behavioral_status"],
                "verdict": job["verdict"],
                "evidence_count": array_len(&job["evidence"]),
                "public_digest_count": job["evidence"].as_array().map_or(0, |evidence| evidence.iter().filter(|item| !item["artifact_sha256"].is_null()).count()),
                "limitation_count": array_len(&job["limitations"])
            })).collect::<Vec<_>>()).unwrap_or_default()
        }),
        "conformance-report" => json!({
            "contract": data["contract"],
            "report_id": data["report_id"],
            "pack_id": data["pack_release"]["pack_id"],
            "release_id": data["pack_release"]["release_id"],
            "generated_at": data["generated_at"],
            "evaluator_inventory_sha256": data["evaluator_inventory_sha256"],
            "lifecycle_policy_sha256": data["lifecycle_policy_sha256"],
            "job_conformance_count": array_len(&data["job_conformance_sha256s"]),
            "job_conformance_sha256s": data["job_conformance_sha256s"]
        }),
        "skills" => json!({
            "contract": data["contract"],
            "status": data["status"],
            "valid": data["valid"],
            "profile_id": data["profile"]["id"],
            "profile_activation": data["profile_activation"],
            "packaged_skill_ids": data["packaged_skill_ids"],
            "eligible_skill_ids": data["eligibility"]["eligible_skill_ids"],
            "requested_job": data["requested_job"],
            "recommendation": data["recommendation"],
            "route_count": array_len(&data["job_routes"]),
            "diagnostics": data["diagnostics"]
        }),
        "requirements" => json!({
            "contract": data["contract"],
            "status": data["status"],
            "valid": data["valid"],
            "available": data["available"],
            "model_task_available": data.get("model_task").is_some_and(|value| !value.is_null()),
            "pack_id": data["pack"]["id"],
            "job_id": data["job"]["id"],
            "profile_activation": data["profile_activation"],
            "model_task_status": data["model_task"]["status"],
            "model_task_kind": data["model_task"]["kind"],
            "prompt_id": data["model_task"]["prompt_id"],
            "prompt_version": data["model_task"]["prompt_version"],
            "prompt_sha256": data["model_task"]["prompt_sha256"],
            "decision_input_contract_count": array_len(&data["decision_input_contracts"]),
            "diagnostics": data["diagnostics"]
        }),
        "validate-source-binding" => json!({
            "contract": data["contract"],
            "status": data["status"],
            "valid": data["valid"],
            "available": data["available"],
            "pack_id": data["pack"]["id"],
            "job_id": data["job"]["id"],
            "coverage": data["coverage"],
            "integration_releases": data["integration_releases"],
            "diagnostics": data["diagnostics"]
        }),
        "doctor" | "validate" | "validate-prompt-output" => json!({
            "valid": data["valid"],
            "strict": data["strict"],
            "error_count": data["error_count"],
            "warning_count": data["warning_count"],
            "issue_count": array_len(&data["issues"]),
            "issues": data["issues"]
        }),
        "run-receipt" => json!({
            "valid": data["valid"],
            "decision": data["decision"],
            "workflow": data["workflow"],
            "isolation": data["boundary"]["isolation"],
            "conversation_context_used": data["boundary"]["conversation_context_used"],
            "declared_inputs_only": data["boundary"]["declared_inputs_only"],
            "source_audit_required": data["prompt"]["source_audit_required"],
            "artifact_count": array_len(&data["artifacts"]),
            "error_count": data["error_count"],
            "warning_count": data["warning_count"],
            "issue_count": array_len(&data["issues"]),
            "issues": data["issues"],
            "artifact": data["artifact"],
            "dry_run": data["dry_run"],
            "write_plan": data["write_plan"]
        }),
        "verify-run" => json!({
            "contract": data["contract"],
            "valid": data["valid"],
            "integrity_only": data["integrity_only"],
            "execution_id": data["execution_id"],
            "terminal_state": data["terminal_state"],
            "assurance": data["recomputed_assurance"],
            "issues": data["issues"]
        }),
        "run" => json!({
            "valid": data["valid"],
            "execution_id": data["execution_id"],
            "terminal_state": data["terminal_state"],
            "authority": data["authority"],
            "run_dir": data["run_dir"],
            "bundle_sha256": data["bundle_sha256"],
            "receipt_sha256": data["receipt_sha256"],
            "authority_block": data["authority_block"]
        }),
        "verify-output" => json!({
            "valid": data["valid"],
            "decision": data["decision"],
            "error_count": data["error_count"],
            "warning_count": data["warning_count"],
            "checked": data["checked"],
            "issue_count": array_len(&data["issues"]),
            "issues": data["issues"]
        }),
        "author-proof-output" => json!({
            "valid": data["valid"],
            "verification_decision": data["verification"]["decision"],
            "verification_valid": data["checked"]["verification_valid"],
            "error_count": data["error_count"],
            "warning_count": data["warning_count"],
            "author_error_count": data["author_error_count"],
            "author_warning_count": data["author_warning_count"],
            "verification_error_count": data["verification"]["error_count"],
            "verification_warning_count": data["verification"]["warning_count"],
            "checked": data["checked"],
            "issue_count": array_len(&data["issues"]),
            "verification_issue_count": array_len(&data["verification"]["issues"]),
            "issues": data["issues"],
            "verification_issues": data["verification"]["issues"],
            "input_artifact": data["input_artifact"],
            "artifact": data["artifact"],
            "dry_run": data["dry_run"],
            "write_plan": data["write_plan"]
        }),
        "render-brief" => json!({
            "artifact_type": data["artifact_type"],
            "template_id": data["template_id"],
            "decision": data["decision"],
            "authority": data["authority"],
            "pack_id": data["pack_id"],
            "source_artifact_type": data["source_artifact_type"],
            "section_count": array_len(&data["sections"]),
            "warning_count": array_len(&data["audit"]["warnings"]),
            "artifact": data["artifact"]
        }),
        "route" => json!({
            "persona": data["persona"],
            "requested_persona": data["requested_persona"],
            "persona_resolution": data["persona_resolution"],
            "job": data["job"],
            "scope": data["scope"],
            "portfolio_sensitive": data["portfolio_sensitive"],
            "route_card_cap": data["route_card_cap"],
            "draft_status": data["draft_status"],
            "profile_activation": data["profile_activation"],
            "card_count": array_len(&data["load_order"]),
            "load_order": data["load_order"],
            "entry_match_count": array_len(&data["entry_route"]["matches"]),
            "entry_gap_count": array_len(&data["entry_route"]["gaps"]),
        "entry_gaps": data["entry_route"]["gaps"],
            "minimality": data["entry_route"]["minimality"],
            "eval_fixture": data["eval_fixture"]
        }),
        "route-budget" => route_budget_summary_projection(data),
        "sample-leads" => json!({
            "contract": data["contract"],
            "persona": data["inputs"]["persona"],
            "requested_persona": data["inputs"]["requested_persona"],
            "persona_resolution": data["persona_resolution"],
            "job": data["inputs"]["job"],
            "count": array_len(&data["fixture_leads"]),
            "seed": data["inputs"]["seed"],
            "source_kind": data["fixture_notice"]["source_kind"],
            "synthetic": data["fixture_notice"]["synthetic"],
            "do_not_contact": data["fixture_notice"]["do_not_contact"],
            "route_card_count": array_len(&data["route"]["load_order"]),
            "lead_ids": data["fixture_leads"].as_array().map(|rows| {
                rows.iter()
                    .filter_map(|row| row["id"].as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            }).unwrap_or_default()
        }),
        "rebind-synthetic-chain" => json!({
            "contract": data["contract"],
            "valid": data["valid"],
            "status": data["status"],
            "mode": data["mode"],
            "job_id": data["job_id"],
            "pack_sha256": data["pack"]["sha256"],
            "file_count": array_len(&data["files"]),
            "files": data["files"].as_array().map(|files| files.iter().map(|file| json!({
                "name": file["name"], "action": file["action"], "bytes": file["bytes"], "sha256": file["sha256"], "backup": file["backup"]
            })).collect::<Vec<_>>()).unwrap_or_default(),
            "source_binding_valid": data["validation"]["source_binding"]["valid"],
            "prompt_output_valid": data["validation"]["prompt_output"]["valid"]
        }),
        "fit" => json!({
            "valid": data["valid"],
            "job_id": data["job_id"],
            "ingress": data["ingress"],
            "status": data["status"],
            "decision": data["decision"],
            "scope": data["scope"],
            "portfolio_sensitive": data["portfolio_sensitive"],
            "match_count": array_len(&data["matches"]),
            "disqualifier_count": array_len(&data["disqualifiers"]),
            "company_domain": data["prospect"]["company_domain"],
            "missing_context": data["context"]["missing"],
            "missing_requirements": data["context"]["missing_requirements"],
            "invalid_requirements": data["context"]["invalid_requirements"],
            "signal_authority_class": data["signal_authority"]["authority_class"],
            "accepted_signal_ids": data["signal_authority"]["accepted"].as_array().map(|items| items.iter().filter_map(|item| item["signal_id"].as_str()).collect::<Vec<_>>()).unwrap_or_default(),
            "rejected_signal_ids": data["signal_authority"]["rejected"].as_array().map(|items| items.iter().filter_map(|item| item["signal_id"].as_str()).collect::<Vec<_>>()).unwrap_or_default()
        }),
        "brief" => json!({
            "contract": data["contract"],
            "valid": data["valid"],
            "channel": data["channel"],
            "persona": data["persona"],
            "job": data["job"],
            "draft_status": data["draft_status"],
            "route_card_cap": data["route_card_cap"],
            "scope": data["scope"],
            "portfolio_sensitive": data["portfolio_sensitive"],
            "fit_status": data["fit"]["status"],
            "ingress": data["fit"]["ingress"],
            "signal_authority": signal_authority_summary(&data["fit"]["signal_authority"]),
            "required_card_count": array_len(&data["required_load_order"]),
            "required_load_order": data["required_load_order"],
            "product_foundation": product_foundation_summary(&data["product_foundation"]),
            "product_foundation_load_order": data["product_foundation_load_order"],
            "profile_activation": data["profile_activation"],
            "context": context_summary(&data["context"]),
            "prospect_source": data["prospect_source"],
            "input_artifact": data["input_artifact"],
            "artifact": data["artifact"],
            "dry_run": data["dry_run"],
            "write_plan": data["write_plan"]
        }),
        "emit-brief" => json!({
            "contract": data["contract"],
            "persona": data["inputs"]["persona"],
            "requested_persona": data["inputs"]["requested_persona"],
            "persona_resolution": data["persona_resolution"],
            "job": data["inputs"]["job"],
            "scope": data["scope"],
            "portfolio_sensitive": data["portfolio_sensitive"],
            "draft_status": data["draft_status"],
            "route_card_cap": data["route_card_cap"],
            "required_card_count": array_len(&data["required_load_order"]),
            "required_load_order": data["required_load_order"],
            "product_foundation": product_foundation_summary(&data["product_foundation"]),
            "product_foundation_load_order": data["product_foundation_load_order"],
            "profile_activation": data["profile_activation"],
            "context": context_summary(&data["context"]),
            "artifact": data["artifact"],
            "dry_run": data["dry_run"],
            "write_plan": data["write_plan"]
        }),
        "copy" => json!({
            "contract": data["contract"],
            "channel": data["channel"],
            "persona": data["persona"],
            "draft_status": data["draft_status"],
            "cards_used_count": array_len(&data["cards_used"]),
            "cards_used": data["cards_used"],
            "input_artifact": data["input_artifact"],
            "artifact": data["artifact"]
        }),
        "pack" => json!({
            "contract": data["contract"],
            "pack": data["pack"],
            "card_count": array_len(&data["cards"]),
            "artifact": data["artifact"],
            "dry_run": data["dry_run"],
            "write_plan": data["write_plan"]
        }),
        "check-claims" => json!({
            "valid": data["valid"],
            "decision": data["decision"],
            "strict": data["strict"],
            "scope": data["scope"],
            "portfolio_sensitive": data["portfolio_sensitive"],
            "scope_blocked": data["scope_blocked"],
            "matched_claim_count": array_len(&data["matched_claims"]),
            "claim_gap_count": array_len(&data["claim_gaps"]),
            "guardrail_hit_count": array_len(&data["guardrail_hits"]),
            "unsupported_claim_count": array_len(&data["unsupported_claims"])
        }),
        "gaps" => json!({
            "durable_gap_count": data["summary"]["durable"],
            "evidence_gap_count": data["summary"]["evidence"]
        }),
        "eval" => json!({
            "valid": data["valid"],
            "strict": data["strict"],
            "fixture_count": data["summary"]["fixture_count"],
            "issue_count": array_len(&data["issues"]),
            "failing_fixtures": failing_fixtures(data)
        }),
        _ => data.clone(),
    }
}

fn print_summary(command: &str, summary: &Value) -> Result<()> {
    println!("{command}: summary");
    println!("{}", serde_json::to_string_pretty(summary)?);
    Ok(())
}

fn signal_authority_summary(authority: &Value) -> Value {
    let mut summary = json!({
        "authority_class": authority["authority_class"],
        "aggregate_authority": authority["aggregate_authority"],
        "projection_status": authority["projection_status"],
        "roles": authority["roles"],
        "accepted": authority["accepted"],
        "rejected": authority["rejected"],
        "trust_boundary": authority["trust_boundary"]
    });
    for field in [
        "source_binding_sha256",
        "source_attempt_request_sha256",
        "collected_attempt_results_sha256",
        "normalized_output_sha256",
    ] {
        if !authority[field].is_null() {
            summary[field] = authority[field].clone();
        }
    }
    summary
}

fn array_len(value: &Value) -> usize {
    value.as_array().map(Vec::len).unwrap_or(0)
}

fn context_summary(context: &Value) -> Value {
    if !context.is_object() {
        return Value::Null;
    }
    let mut summary = json!({
        "contract": context["contract"],
        "status": context["status"],
        "reason": context["reason"],
        "scope": context["scope"],
        "portfolio_sensitive": context["portfolio_sensitive"],
        "route_card_cap": context["route_card_cap"],
        "profile_activation": context["profile_activation"],
        "entry_count": context["summary"]["entry_count"],
        "required_entry_count": context["summary"]["required_entry_count"],
        "supporting_entry_count": context["summary"]["supporting_entry_count"],
        "guardrail_entry_count": context["summary"]["guardrail_entry_count"],
        "gap_count": array_len(&context["gaps"]),
        "gaps": context["gaps"],
        "full_card_required": context["full_card_required"],
        "minimality": {
            "status": context["minimality"]["status"],
            "context_sha256": context["minimality"]["context_sha256"],
            "budget": context["minimality"]["budget"],
            "selected_count": context["minimality"]["selected_count"],
            "excluded_count": context["minimality"]["excluded_count"],
            "excluded": context["minimality"]["excluded"],
            "largest_contributing_cards": context["minimality"]["largest_contributing_cards"],
            "diagnostics": context["minimality"]["diagnostics"]
        }
    });
    if !context["minimality"]["allocation"].is_null() {
        summary["minimality"]["allocation"] = context["minimality"]["allocation"].clone();
    }
    summary
}

fn product_foundation_summary(foundation: &Value) -> Value {
    if !foundation.is_object() {
        return Value::Null;
    }
    json!({
        "status": foundation["status"],
        "diagnostics": foundation["diagnostics"]
    })
}

fn failing_fixtures(data: &Value) -> Vec<String> {
    data["fixtures"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|fixture| fixture["valid"].as_bool() == Some(false))
        .filter_map(|fixture| fixture["id"].as_str().map(str::to_string))
        .collect()
}

/// Emits the canonical `output_mode_conflict` JSON envelope to stdout and
/// leaves stderr empty. The conflict must be raised before any command work
/// or any stdout write, so this helper performs the only stdout emission for
/// the conflict path.
pub(crate) fn print_output_mode_conflict(
    json_mode: bool,
    selector: &str,
    value: &str,
) -> Result<()> {
    let envelope = json!({
        "ok": false,
        "error": {
            "code": OUTPUT_MODE_CONFLICT_CODE,
            "message": format!(
                "--json cannot be combined with human-only presentation {selector}={value}"
            ),
            "details": [selector, value]
        }
    });
    if json_mode {
        println!("{}", serde_json::to_string_pretty(&envelope)?);
    }
    Ok(())
}

/// Wraps a Clap help or version display string in the canonical JSON
/// success envelope. The `ok` field is `true` so the result is parseable as
/// a single JSON value and matches the existing success contract.
pub(crate) fn print_display_envelope(json_mode: bool, kind: DisplayKind, text: &str) -> Result<()> {
    if !json_mode {
        print!("{text}");
        return Ok(());
    }
    let envelope = json!({
        "ok": true,
        "command": kind.command(),
        "data": {"text": text.trim_end()}
    });
    println!("{}", serde_json::to_string_pretty(&envelope)?);
    Ok(())
}

pub(crate) fn print_error(json_mode: bool, err: anyhow::Error) -> Result<()> {
    let message = err.to_string();
    let details = err
        .chain()
        .skip(1)
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>();
    let code = classify_error(&message, &details);
    let payload =
        json!({"ok": false, "error": {"code": code, "message": message, "details": details}});
    if json_mode {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        eprintln!("error: {}", err);
    }
    Ok(())
}

fn classify_error(message: &str, details: &[String]) -> &'static str {
    let lower = format!("{} {}", message, details.join(" ")).to_lowercase();
    if lower.contains("output_mode_conflict") {
        OUTPUT_MODE_CONFLICT_CODE
    } else if lower.contains("route_budget_filter_not_found") {
        "route_budget_filter_not_found"
    } else if lower.contains("unrecognized subcommand")
        || lower.contains("unexpected argument")
        || lower.contains("required arguments")
        || lower.contains("pass either --text or --file")
        || lower.contains("pass --text or --file")
        || lower.contains("pass at most one of --prompt and --prompt-id")
        || lower.contains("unsupported template")
        || lower.contains("--count must")
        || lower.contains("--artifact must use")
        || lower.contains("invalid --scope")
    {
        "invalid_argument"
    } else if lower.contains("already exists; pass --force") {
        "write_conflict"
    } else if lower.contains(".mdp/manifest.yaml") && lower.contains("parsing") {
        "invalid_manifest"
    } else if lower.contains(".mdp/manifest.yaml") && lower.contains("reading") {
        "pack_not_found"
    } else if lower.contains("prospect_unknown_field")
        || lower.contains("prospect_signal_unknown_field")
        || lower.contains("invalid prospect input")
    {
        "invalid_prospect"
    } else if lower.contains("missing card id")
        || (lower.contains(".mdp/cards/") && lower.contains("reading"))
    {
        "missing_card"
    } else if lower.contains("unsupported claim") {
        "unsupported_claim"
    } else if lower.contains("insufficient-context") || lower.contains("insufficient context") {
        "insufficient_context"
    } else {
        "mdp_error"
    }
}

fn print_human(command: &str, data: &Value) -> Result<()> {
    match command {
        "init" => {
            if data["dry_run"].as_bool() == Some(true) {
                println!(
                    "init: dry run for {}",
                    data["pack_dir"].as_str().unwrap_or("")
                );
                print_write_plan(data);
            } else {
                println!(
                    "Created MDP package at {}",
                    data["pack_dir"].as_str().unwrap_or("")
                );
            }
            println!(
                "Next: mdp validate --dir {}",
                data["root"].as_str().unwrap_or(".")
            );
        }
        "capabilities" => {
            println!("mdp capabilities:");
            if let Some(commands) = data["commands"].as_array() {
                for command in commands {
                    println!(
                        "- {}: {}",
                        command["name"].as_str().unwrap_or("command"),
                        command["side_effects"].as_str().unwrap_or("unknown")
                    );
                }
            }
        }
        "doctor" | "validate" => {
            println!(
                "{}: {}",
                command,
                if data["valid"].as_bool().unwrap_or(false) {
                    "ok"
                } else {
                    "needs attention"
                }
            );
            if let Some(items) = data["issues"].as_array() {
                for item in items {
                    println!("- {}", issue_message(item));
                }
            }
        }
        "brief" | "emit-brief" | "pack" | "author-proof-output"
            if data["dry_run"].as_bool() == Some(true) =>
        {
            println!("{command}: dry run");
            print_write_plan(data);
        }
        "fit" => {
            println!("fit: {}", data["status"].as_str().unwrap_or("unknown"));
            println!("{}", data["decision"].as_str().unwrap_or(""));
            println!(
                "signal authority: {}",
                data["signal_authority"]["authority_class"]
                    .as_str()
                    .unwrap_or("unassessed")
            );
            print_requirement_list("missing", &data["context"]["missing_requirements"]);
            print_requirement_list("invalid", &data["context"]["invalid_requirements"]);
        }
        _ => println!("{}", serde_json::to_string_pretty(data)?),
    }
    Ok(())
}

fn print_requirement_list(label: &str, value: &Value) {
    let Some(items) = value.as_array() else {
        return;
    };
    if items.is_empty() {
        return;
    }
    println!("{label} requirements:");
    for item in items {
        println!(
            "- {}: {}",
            item["path"].as_str().unwrap_or("unknown"),
            item["reason"].as_str().unwrap_or("required")
        );
    }
}

fn print_write_plan(data: &Value) {
    if let Some(items) = data["write_plan"].as_array() {
        for item in items {
            println!(
                "- {} {} ({})",
                item["action"].as_str().unwrap_or("write"),
                item["path"].as_str().unwrap_or(""),
                item["kind"].as_str().unwrap_or("file")
            );
        }
    }
}

fn issue_message(item: &Value) -> String {
    if let Some(message) = item.as_str() {
        return message.to_string();
    }
    let code = item["code"].as_str().unwrap_or("issue");
    let path = item["path"].as_str().unwrap_or("");
    let message = item["message"].as_str().unwrap_or("");
    if path.is_empty() {
        format!("{code}: {message}")
    } else {
        format!("{code} at {path}: {message}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_summary_is_counted_and_samples_only_bounded_conflicts() {
        let paths = (0..100)
            .map(|index| format!(".mdp/cards/{index}.yaml"))
            .collect::<Vec<_>>();
        let summary = summarize(
            "author-apply",
            &json!({
                "contract": "mdp.pack-authoring-result.v1",
                "status": "refused",
                "valid": false,
                "created": paths.clone(),
                "changed": [],
                "unchanged": [],
                "deleted": [],
                "refused": paths.clone(),
                "rolled_back": paths,
                "reason_codes": ["live-pack-changed-after-preview"],
                "private_content_included": false
            }),
        );
        assert_eq!(summary["created_count"], 100);
        assert_eq!(summary["refused_count"], 100);
        assert_eq!(summary["refused_sample"].as_array().unwrap().len(), 8);
        assert_eq!(summary["rolled_back_sample"].as_array().unwrap().len(), 8);
        assert!(summary.get("created").is_none());
    }

    #[test]
    fn brief_summary_exposes_status_artifacts_and_provenance() {
        let summary = summarize(
            "brief",
            &json!({
                "contract": "mdp.message-brief.v0",
                "channel": "email",
                "persona": "PMM",
                "job": "write outbound message",
                "draft_status": "ready",
                "scope": {"requested": {"product": ["local-cli"]}, "selected": {"product": ["local-cli"]}, "issues": []},
                "portfolio_sensitive": true,
                "fit": {"status": "fit"},
                "required_load_order": [".mdp/cards/personas.yaml", ".mdp/cards/claims.yaml"],
                "product_foundation": {
                    "status": "blocked",
                    "diagnostics": [{"code": "product_foundation_selected_facet_has_gaps"}],
                    "selected_facets": [{"entries": [{"body": "must not leak"}]}]
                },
                "product_foundation_load_order": [
                    {"facet_id": "identity", "card_id": "claims", "entry_id": "identity"}
                ],
                "prospect_source": {"kind": "synthetic-example", "synthetic": true},
                "input_artifact": {"kind": "prospect", "path": "examples/clay-row.json"},
                "artifact": {"status": "stdout-only", "kind": "stdout", "path": null}
            }),
        );

        assert_eq!(summary["draft_status"], "ready");
        assert_eq!(summary["required_card_count"], 2);
        assert_eq!(summary["prospect_source"]["kind"], "synthetic-example");
        assert_eq!(summary["artifact"]["status"], "stdout-only");
        assert_eq!(summary["product_foundation"]["status"], "blocked");
        assert_eq!(
            summary["product_foundation"]["diagnostics"][0]["code"],
            "product_foundation_selected_facet_has_gaps"
        );
        assert_eq!(
            summary["product_foundation_load_order"][0]["entry_id"],
            "identity"
        );
        assert!(
            summary["product_foundation"]
                .get("selected_facets")
                .is_none()
        );
    }

    #[test]
    fn emit_brief_summary_exposes_compact_foundation_and_exact_load_order() {
        let summary = summarize(
            "emit-brief",
            &json!({
                "contract": "mdp.brief.v0",
                "inputs": {"persona": "PMM", "requested_persona": "PMM", "job": "outbound-copy-brief"},
                "persona_resolution": {"source": "exact"},
                "scope": {},
                "portfolio_sensitive": false,
                "draft_status": "blocked",
                "required_load_order": [],
                "product_foundation": {
                    "status": "blocked",
                    "diagnostics": [{"code": "product_foundation_selected_facet_has_gaps"}],
                    "selected_facets": [{"entries": [{"body": "must not leak"}]}]
                },
                "product_foundation_load_order": [
                    {"facet_id": "identity", "reference_kind": "gap", "card_id": "gaps", "entry_id": "missing-proof"}
                ],
                "context": {"contract": "mdp.context.v0", "status": "blocked", "summary": {}, "gaps": [], "full_card_required": []},
                "artifact": {"status": "stdout-only"}
            }),
        );

        assert_eq!(summary["product_foundation"]["status"], "blocked");
        assert_eq!(
            summary["product_foundation"]["diagnostics"][0]["code"],
            "product_foundation_selected_facet_has_gaps"
        );
        assert_eq!(
            summary["product_foundation_load_order"][0]["entry_id"],
            "missing-proof"
        );
        assert!(
            summary["product_foundation"]
                .get("selected_facets")
                .is_none()
        );
    }

    #[test]
    fn sample_leads_summary_exposes_fixture_safety() {
        let summary = summarize(
            "sample-leads",
            &json!({
                "contract": "mdp.sample-leads.v0",
                "inputs": {"persona": "PMM", "job": "initial email outbound copy", "seed": 7},
                "persona_resolution": {"persona": "PMM", "source": "input.persona"},
                "fixture_notice": {"source_kind": "synthetic-example", "synthetic": true, "do_not_contact": true},
                "route": {"load_order": [".mdp/cards/personas.yaml"]},
                "fixture_leads": [
                    {"id": "fixture-lead-1"},
                    {"id": "fixture-lead-2"}
                ]
            }),
        );

        assert_eq!(summary["contract"], "mdp.sample-leads.v0");
        assert_eq!(summary["count"], 2);
        assert_eq!(summary["source_kind"], "synthetic-example");
        assert_eq!(summary["do_not_contact"], true);
        assert_eq!(summary["route_card_count"], 1);
    }

    #[test]
    fn brief_summary_exposes_context_counts_without_entry_bodies() {
        let summary = summarize(
            "brief",
            &json!({
                "contract": "mdp.message-brief.v0",
                "channel": "linkedin",
                "persona": "PMM",
                "job": "write outbound message",
                "draft_status": "ready",
                "scope": {"requested": {"product": ["local-cli"]}, "selected": {"product": ["local-cli"]}, "issues": []},
                "portfolio_sensitive": true,
                "fit": {"status": "fit"},
                "required_load_order": [".mdp/cards/personas.yaml"],
                "context": {
                    "contract": "mdp.context.v0",
                    "status": "ready",
                    "scope": {"requested": {"product": ["local-cli"]}, "selected": {"product": ["local-cli"]}, "issues": []},
                    "portfolio_sensitive": true,
                    "gaps": [],
                    "entries": [{"body": "should not appear in summary"}],
                    "full_card_required": [],
                    "minimality": {
                        "status": "ready",
                        "context_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "budget": {"max_entries": 8, "max_bytes": 4096, "actual_entries": 4, "actual_bytes": 1024},
                        "selected_count": 4,
                        "excluded_count": 1,
                        "allocation": {"strategy": "required-first", "required_count": 2, "optional_selected_count": 2, "optional_excluded_count": 1, "required_by_kind": {}, "quotas": {}},
                        "excluded": [{"card_id": "claims", "card_kind": "claims", "entry_id": "unselected", "reason_code": "not_applicable"}],
                        "diagnostics": []
                    },
                    "summary": {
                        "entry_count": 4,
                        "required_entry_count": 2,
                        "supporting_entry_count": 2,
                        "guardrail_entry_count": 1
                    }
                },
                "prospect_source": {"kind": "synthetic-example", "synthetic": true},
                "input_artifact": {"kind": "prospect", "path": "examples/clay-row.json"},
                "artifact": {"status": "stdout-only", "kind": "stdout", "path": null}
            }),
        );

        assert_eq!(summary["context"]["contract"], "mdp.context.v0");
        assert_eq!(summary["context"]["entry_count"], 4);
        assert_eq!(summary["context"]["minimality"]["status"], "ready");
        assert_eq!(summary["context"]["minimality"]["excluded_count"], 1);
        assert_eq!(
            summary["context"]["minimality"]["allocation"]["strategy"],
            "required-first"
        );
        assert!(summary.to_string().contains("unselected"));
        assert!(!summary.to_string().contains("should not appear in summary"));
        assert_eq!(summary["portfolio_sensitive"], true);
        assert_eq!(summary["scope"]["selected"]["product"][0], "local-cli");
        assert!(summary["context"].get("entries").is_none());
    }

    #[test]
    fn route_budget_summary_preserves_allocation_without_entry_bodies() {
        let allocation = json!({
            "strategy": "required-first",
            "required_count": 2,
            "optional_selected_count": 1,
            "optional_excluded_count": 1,
            "required_by_kind": {"channel-policies": 1, "gaps": 1},
            "quotas": {"hooks": {
                "max_optional_entries": 1,
                "reserved_count": 0,
                "optional_selected_count": 1,
                "optional_excluded_count": 1
            }}
        });
        let raw = json!({
            "contract": "mdp.route-budget.v0",
            "valid": true,
            "strict": {"enabled": false, "warnings_fail": true, "warning_count": 0},
            "pack_id": "synthetic-pack",
            "route_count": 1,
            "overflow_count": 0,
            "route_card_cap_exclusion_count": 0,
            "near_budget_count": 0,
            "unassessed_generation_count": 0,
            "routes": [{
                "persona": "PMM",
                "job": "outbound-copy-brief",
                "status": "ready",
                "budget": {"max_entries": 8, "max_bytes": 4096, "actual_entries": 3, "actual_bytes": 1024},
                "selected_count": 3,
                "excluded_count": 1,
                "allocation": allocation,
                "diagnostics": [],
                "reason_distribution": {},
                "excluded_reason_distribution": {"optional_kind_quota_exceeded": 1},
                "largest_contributing_cards": [],
                "route_card_cap": {"status": "ready"}
            }]
        });
        let summary = summarize("route-budget", &raw);

        assert_eq!(summary["contract"], "mdp.route-budget-summary.v1");
        assert_eq!(summary["route_status_counts"]["ready"], 1);
        assert!(summary.get("routes").is_none());
        assert!(!summary.to_string().contains("entry body"));
    }

    #[test]
    fn eval_summary_lists_failing_fixtures() {
        let summary = summarize(
            "eval",
            &json!({
                "valid": false,
                "summary": {"fixture_count": 2},
                "issues": [{"code": "eval_expected_entry_missing"}],
                "fixtures": [
                    {"id": "ok", "valid": true},
                    {"id": "bad", "valid": false}
                ]
            }),
        );

        assert_eq!(summary["valid"], false);
        assert_eq!(summary["fixture_count"], 2);
        assert_eq!(summary["issue_count"], 1);
        assert_eq!(summary["failing_fixtures"][0], "bad");
    }

    #[test]
    fn skills_summary_preserves_route_and_diagnostic_state() {
        let summary = summarize(
            "skills",
            &json!({
                "contract": "mdp.skills.v1",
                "status": "ready",
                "valid": true,
                "profile": {"id": "gtm"},
                "packaged_skill_ids": ["mdp", "mdp-gtm-brief"],
                "eligibility": {"eligible_skill_ids": ["mdp", "mdp-gtm-brief"]},
                "requested_job": "prospect-fit-or-brief",
                "recommendation": {"job_id": "prospect-fit-or-brief", "skill_id": "mdp-gtm-brief"},
                "job_routes": [{"job_id": "prospect-fit-or-brief"}],
                "diagnostics": []
            }),
        );

        assert_eq!(summary["contract"], "mdp.skills.v1");
        assert_eq!(summary["profile_id"], "gtm");
        assert_eq!(summary["route_count"], 1);
        assert_eq!(summary["recommendation"]["skill_id"], "mdp-gtm-brief");
    }

    #[test]
    fn requirements_summary_derives_model_task_availability_from_compiled_task() {
        let summary = summarize(
            "requirements",
            &json!({
                "contract": "mdp.requirements.v0",
                "status": "ready",
                "valid": true,
                "available": false,
                "pack": {"id": "example"},
                "job": {"id": "outbound-copy-brief"},
                "model_task": {
                    "status": "ready",
                    "kind": "generation",
                    "prompt_id": "generate-outbound-copy",
                    "prompt_version": "1",
                    "prompt_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                },
                "decision_input_contracts": [],
                "diagnostics": []
            }),
        );

        assert_eq!(summary["model_task_available"], true);
    }

    #[test]
    fn behavioral_conformance_summary_is_compact_and_never_grants_drafting() {
        let summary = summarize(
            "conformance-validate",
            &json!({
                "contract":"mdp.behavioral-evaluation.v1","valid":true,
                "job_id":"outbound-copy-brief","deterministic_status":"passed",
                "job_sufficiency":"sufficient-for-job",
                "behavioral_qualification":"qualified-for-job-under-envelope",
                "overall_result":"qualified-for-job-under-envelope",
                "drafting_authority_granted":false,
                "trials":[{"trial_id":"private-trial","raw_output":"must not escape"}],
                "reason_codes":[]
            }),
        );
        assert_eq!(summary["trial_count"], 1);
        assert_eq!(summary["drafting_authority_granted"], false);
        assert!(!summary.to_string().contains("private-trial"));
        assert!(!summary.to_string().contains("must not escape"));
    }

    #[test]
    fn conformance_summaries_keep_only_ids_status_counts_and_digests() {
        let compile = summarize(
            "conformance-compile",
            &json!({
                "contract":"mdp.deterministic-conformance.v1", "valid":true,
                "candidate_id":"candidate-1", "job_id":"job-1", "fixture_id":"fixture-1",
                "challenge_id":"challenge-1", "pack_release":{"pack_id":"pack-1","release_id":"release-1","private":"must-not-escape"},
                "status":"sufficient-for-job", "behavioral_qualification_allowed":true,
                "evaluator":{"inventory_sha256":"a".repeat(64),"private":"must-not-escape"},
                "assertions":[{"evidence_refs":["private/path"]}],
                "summary":{"passed":12,"failed":0,"unassessed":0}
            }),
        );
        assert_eq!(compile["passed_assertion_count"], 12);
        assert!(!compile.to_string().contains("must-not-escape"));
        assert!(!compile.to_string().contains("private/path"));

        let assembled = summarize(
            "conformance-assemble",
            &json!({
                "contract":"mdp.job-conformance.v1", "candidate_id":"candidate-1", "job_id":"job-1", "fixture_id":"fixture-1",
                "pack_release":{"pack_id":"pack-1","release_id":"release-1"},
                "deterministic_status":"passed", "behavioral_status":"passed", "verdict":"qualified-for-job-under-envelope",
                "candidate_sha256":"a".repeat(64), "deterministic_evaluation_sha256":"b".repeat(64), "behavioral_evaluation_sha256":"c".repeat(64),
                "trial_sha256s":["d".repeat(64)],
                "journey":{"artifacts":[{"relative_path":"private/path"}],"links":[{"from_artifact_id":"private-id"}]},
                "limitations":["private limitation"]
            }),
        );
        assert_eq!(assembled["journey_artifact_count"], 1);
        assert!(!assembled.to_string().contains("private/path"));
        assert!(!assembled.to_string().contains("private limitation"));
    }

    #[test]
    fn public_and_private_report_summaries_do_not_clone_evidence() {
        let public = summarize(
            "conformance-report",
            &json!({
                "contract":"mdp.public-conformance-report.v1", "report_id":"report-1", "pack_id":"pack-1", "release_id":"release-1",
                "evaluator_id":"evaluator-1", "evaluator_version":"1", "generated_at":"2026-08-13T14:00:00Z",
                "jobs":[{"job_id":"job-1","deterministic_status":"passed","behavioral_status":"passed","verdict":"qualified-for-job-under-envelope",
                    "evidence":[{"artifact_sha256":null,"opaque_artifact_id":"private-ref"},{"artifact_sha256":"a".repeat(64)}],"limitations":["private limitation"]}]
            }),
        );
        assert_eq!(public["jobs"][0]["evidence_count"], 2);
        assert_eq!(public["jobs"][0]["public_digest_count"], 1);
        assert!(!public.to_string().contains("private-ref"));
        assert!(!public.to_string().contains("private limitation"));

        let private = summarize(
            "conformance-report",
            &json!({
                "contract":"mdp.conformance-report.v1", "report_id":"private-report-1",
                "pack_release":{"pack_id":"pack-1","release_id":"release-1","private":"must-not-escape"},
                "evaluator_inventory_sha256":"a".repeat(64), "job_conformance_sha256s":["b".repeat(64)],
                "generated_at":"2026-08-13T14:00:00Z", "lifecycle_policy_sha256":"c".repeat(64)
            }),
        );
        assert_eq!(private["job_conformance_count"], 1);
        assert!(!private.to_string().contains("must-not-escape"));
    }

    #[test]
    fn json_errors_are_classified_for_agents() {
        assert_eq!(
            classify_error("unsupported template 'x'; available: gtm", &[]),
            "invalid_argument"
        );
        assert_eq!(
            classify_error(
                "/tmp/.mdp/manifest.yaml already exists; pass --force to overwrite",
                &[]
            ),
            "write_conflict"
        );
        assert_eq!(
            classify_error("reading .mdp/manifest.yaml", &[]),
            "pack_not_found"
        );
        assert_eq!(
            classify_error("invalid --scope \"product\"; expected dimension=value", &[]),
            "invalid_argument"
        );
        assert_eq!(
            classify_error("--artifact must use KIND=PATH", &[]),
            "invalid_argument"
        );
    }

    #[test]
    fn classify_error_recognizes_output_mode_conflict() {
        // The runtime gate emits the conflict envelope directly, but
        // `classify_error` must still recognise the stable code so legacy
        // error paths and operator diagnostics stay aligned with the
        // capability contract.
        assert_eq!(
            classify_error("output_mode_conflict: --readable=true", &[]),
            OUTPUT_MODE_CONFLICT_CODE
        );
    }

    #[test]
    fn presentation_compatibility_matrix_matches_shared_selectors() {
        let matrix = presentation_compatibility_matrix();
        let rows = matrix["selectors"].as_array().expect("rows");
        let mut json_compatible = 0;
        let mut human_only = 0;
        for row in rows {
            let selector = row["selector"].as_str().expect("selector");
            let value = row["value"].as_str().expect("value");
            if row["json_compatible"].as_bool().unwrap_or(false) {
                json_compatible += 1;
                assert!(row["conflict_code"].is_null());
            } else {
                human_only += 1;
                assert_eq!(row["conflict_code"], OUTPUT_MODE_CONFLICT_CODE);
            }
            let _ = (selector, value);
        }
        // Every selector exposes at least one human-only value so the contract
        // is meaningful, and at least one JSON-compatible value so summaries
        // and JSON envelopes can share the same selector.
        assert!(json_compatible > 0);
        assert!(human_only > 0);
        // The matrix is exhaustive: every (selector, value) pair the runtime
        // gate evaluates must appear here.
        let runtime_pairs: Vec<(String, String)> = rows
            .iter()
            .map(|row| {
                (
                    row["selector"].as_str().unwrap().to_string(),
                    row["value"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        let selectors = presentation_selectors();
        for (selector, values) in &selectors {
            for value in values {
                assert!(
                    runtime_pairs.contains(&((*selector).to_string(), (*value).to_string())),
                    "matrix missing ({selector}, {value})"
                );
            }
        }
    }

    #[test]
    fn presentation_contract_describes_stable_invariant() {
        let contract = presentation_contract();
        assert_eq!(contract["conflict"]["code"], OUTPUT_MODE_CONFLICT_CODE);
        assert_eq!(contract["conflict"]["exit_code"], 1);
        assert_eq!(contract["conflict"]["stdout"], "single_json_envelope");
        assert_eq!(contract["conflict"]["stderr"], "empty");
        assert_eq!(contract["display"]["help"]["command"], "help");
        assert_eq!(contract["display"]["help"]["exit_code"], 0);
        assert_eq!(contract["display"]["version"]["command"], "version");
        assert_eq!(contract["display"]["version"]["exit_code"], 0);
        assert_eq!(contract["stdout_invariant"]["mode"], "json");
        assert_eq!(contract["stdout_invariant"]["value_count"], 1);
        assert_eq!(contract["stdout_invariant"]["prelude_allowed"], false);
        assert_eq!(contract["stdout_invariant"]["trailing_text_allowed"], false);
        assert_eq!(contract["stdout_invariant"]["stderr_empty"], true);
    }

    #[test]
    fn presentation_selectors_runtime_and_metadata_share_one_typed_table() {
        // The runtime gate, the matrix projection, and the contract metadata
        // all read from the single `PRESENTATION_SELECTORS` table. If this
        // test compiles and passes, the three surfaces are guaranteed to use
        // the same entries. Any drift is a programming error in the table.
        let selectors = presentation_selectors();
        let matrix = presentation_compatibility_matrix();
        let contract = presentation_contract();
        let matrix_rows = matrix["selectors"].as_array().expect("matrix rows");

        // Every declared selector appears in `presentation_selectors()`,
        // every (selector, value) appears in the matrix, and the contract
        // exposes the same selector names.
        for entry in PRESENTATION_SELECTORS {
            let selector_match = selectors
                .iter()
                .find(|(name, _)| *name == entry.name)
                .unwrap_or_else(|| {
                    panic!(
                        "selector {} missing from presentation_selectors",
                        entry.name
                    )
                });
            let declared_values: Vec<&'static str> = entry.values.iter().map(|v| v.name).collect();
            assert_eq!(selector_match.1, declared_values);

            for value in entry.values {
                let row = matrix_rows
                    .iter()
                    .find(|row| row["selector"] == entry.name && row["value"] == value.name)
                    .unwrap_or_else(|| panic!("matrix missing ({}={})", entry.name, value.name));
                assert_eq!(
                    row["json_compatible"],
                    serde_json::json!(value.json_compatible)
                );
                if value.json_compatible {
                    assert!(row["conflict_code"].is_null());
                } else {
                    assert_eq!(row["conflict_code"], OUTPUT_MODE_CONFLICT_CODE);
                }
            }
        }

        // The matrix and `presentation_selectors()` agree on value count and
        // selector count exactly.
        let matrix_pair_count: usize = matrix_rows.len();
        let declared_pair_count: usize = PRESENTATION_SELECTORS
            .iter()
            .map(|entry| entry.values.len())
            .sum();
        assert_eq!(matrix_pair_count, declared_pair_count);
        assert_eq!(selectors.len(), PRESENTATION_SELECTORS.len());

        // The contract metadata surfaces the same selector list.
        let contract_selectors = contract["selectors"]
            .as_array()
            .expect("contract selectors array");
        let contract_names: Vec<String> = contract_selectors
            .iter()
            .map(|row| row[0].as_str().unwrap().to_string())
            .collect();
        let table_names: Vec<String> = PRESENTATION_SELECTORS
            .iter()
            .map(|entry| entry.name.to_string())
            .collect();
        assert_eq!(contract_names, table_names);
    }

    #[test]
    fn resolve_presentation_flags_human_only_combinations() {
        use crate::cli::{Cli, HumanBriefFormat, SampleLeadsFormat, TraceFormat};
        use clap::Parser;

        let cli = Cli::try_parse_from([
            "mdp", "--json", "trace", "--format", "mermaid", "--file", "x.json",
        ])
        .expect("trace should parse");
        assert_eq!(
            resolve_presentation(&cli.command),
            PresentationOutcome::Conflict {
                selector: "--format",
                value: "mermaid"
            }
        );

        let cli = Cli::try_parse_from([
            "mdp",
            "--json",
            "verify-output",
            "--readable",
            "--file",
            "x.json",
        ])
        .expect("verify-output should parse");
        assert_eq!(
            resolve_presentation(&cli.command),
            PresentationOutcome::Conflict {
                selector: "--readable",
                value: "true"
            }
        );

        let cli = Cli::try_parse_from(["mdp", "--json", "verify-output", "--file", "x.json"])
            .expect("verify-output without --readable should parse");
        assert_eq!(resolve_presentation(&cli.command), PresentationOutcome::Ok);

        let cli = Cli::try_parse_from([
            "mdp",
            "--json",
            "render-brief",
            "--file",
            "x.json",
            "--template",
            "default",
            "--format",
            "markdown",
        ])
        .expect("render-brief markdown should parse");
        assert_eq!(
            resolve_presentation(&cli.command),
            PresentationOutcome::Conflict {
                selector: "--format",
                value: "markdown"
            }
        );

        let cli = Cli::try_parse_from([
            "mdp",
            "--json",
            "sample-leads",
            "--dir",
            ".",
            "--persona",
            "PMM",
            "--format",
            "yaml",
        ])
        .expect("sample-leads yaml should parse");
        assert_eq!(
            resolve_presentation(&cli.command),
            PresentationOutcome::Conflict {
                selector: "--format",
                value: "yaml"
            }
        );

        let cli = Cli::try_parse_from([
            "mdp",
            "--json",
            "brief",
            "--dir",
            ".",
            "--prospect",
            "p.json",
            "--readable",
        ])
        .expect("brief --readable should parse");
        assert_eq!(
            resolve_presentation(&cli.command),
            PresentationOutcome::Conflict {
                selector: "--readable",
                value: "true"
            }
        );

        // The selectors cover every conflict surface; sampling JSON-compatible
        // equivalents confirms the gate does not over-fire.
        let cli = Cli::try_parse_from([
            "mdp", "--json", "trace", "--file", "x.json", "--format", "json",
        ])
        .expect("trace json should parse");
        assert_eq!(resolve_presentation(&cli.command), PresentationOutcome::Ok);

        let cli = Cli::try_parse_from([
            "mdp",
            "--json",
            "render-brief",
            "--file",
            "x.json",
            "--template",
            "default",
            "--format",
            "json",
        ])
        .expect("render-brief json should parse");
        assert_eq!(resolve_presentation(&cli.command), PresentationOutcome::Ok);

        let cli = Cli::try_parse_from([
            "mdp",
            "--json",
            "sample-leads",
            "--dir",
            ".",
            "--persona",
            "PMM",
            "--format",
            "json",
        ])
        .expect("sample-leads json should parse");
        assert_eq!(resolve_presentation(&cli.command), PresentationOutcome::Ok);

        let cli = Cli::try_parse_from([
            "mdp",
            "--json",
            "brief",
            "--dir",
            ".",
            "--prospect",
            "p.json",
        ])
        .expect("brief without --readable should parse");
        assert_eq!(resolve_presentation(&cli.command), PresentationOutcome::Ok);

        let cli = Cli::try_parse_from(["mdp", "--json", "capabilities"])
            .expect("capabilities should parse");
        assert_eq!(resolve_presentation(&cli.command), PresentationOutcome::Ok);

        // Human-only path is left untouched by the gate; the runtime never
        // calls the gate when `--json` is absent.
        let cli = Cli::try_parse_from(["mdp", "trace", "--file", "x.json", "--format", "mermaid"])
            .expect("human-only trace should parse");
        let _ = TraceFormat::Mermaid;
        let _ = HumanBriefFormat::Markdown;
        let _ = SampleLeadsFormat::Yaml;
        assert_eq!(
            resolve_presentation(&cli.command),
            PresentationOutcome::Conflict {
                selector: "--format",
                value: "mermaid"
            }
        );
        let _ = cli;
    }
}
