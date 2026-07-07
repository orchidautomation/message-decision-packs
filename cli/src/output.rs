use anyhow::Result;
use serde_json::{Value, json};

pub(crate) fn print_output(
    json_mode: bool,
    summary_mode: bool,
    command: &str,
    data: Value,
) -> Result<()> {
    if summary_mode {
        let summary = summarize(command, &data);
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
        "doctor" | "validate" | "validate-prompt-output" => json!({
            "valid": data["valid"],
            "strict": data["strict"],
            "error_count": data["error_count"],
            "warning_count": data["warning_count"],
            "issue_count": array_len(&data["issues"]),
            "issues": data["issues"]
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
        "route" => json!({
            "persona": data["persona"],
            "requested_persona": data["requested_persona"],
            "persona_resolution": data["persona_resolution"],
            "job": data["job"],
            "card_count": array_len(&data["load_order"]),
            "load_order": data["load_order"],
            "entry_match_count": array_len(&data["entry_route"]["matches"]),
            "entry_gap_count": array_len(&data["entry_route"]["gaps"]),
            "eval_fixture": data["eval_fixture"]
        }),
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
        "fit" => json!({
            "status": data["status"],
            "decision": data["decision"],
            "match_count": array_len(&data["matches"]),
            "disqualifier_count": array_len(&data["disqualifiers"]),
            "company_domain": data["prospect"]["company_domain"],
            "missing_context": data["context"]["missing"],
            "missing_requirements": data["context"]["missing_requirements"],
            "invalid_requirements": data["context"]["invalid_requirements"]
        }),
        "brief" => json!({
            "contract": data["contract"],
            "channel": data["channel"],
            "persona": data["persona"],
            "job": data["job"],
            "draft_status": data["draft_status"],
            "fit_status": data["fit"]["status"],
            "required_card_count": array_len(&data["required_load_order"]),
            "required_load_order": data["required_load_order"],
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
            "required_card_count": array_len(&data["required_load_order"]),
            "required_load_order": data["required_load_order"],
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

fn array_len(value: &Value) -> usize {
    value.as_array().map(Vec::len).unwrap_or(0)
}

fn context_summary(context: &Value) -> Value {
    if !context.is_object() {
        return Value::Null;
    }
    json!({
        "contract": context["contract"],
        "status": context["status"],
        "entry_count": context["summary"]["entry_count"],
        "required_entry_count": context["summary"]["required_entry_count"],
        "supporting_entry_count": context["summary"]["supporting_entry_count"],
        "guardrail_entry_count": context["summary"]["guardrail_entry_count"],
        "full_card_required": context["full_card_required"]
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
    if lower.contains("unrecognized subcommand")
        || lower.contains("unexpected argument")
        || lower.contains("required arguments")
        || lower.contains("pass either --text or --file")
        || lower.contains("pass --text or --file")
        || lower.contains("pass at most one of --prompt and --prompt-id")
        || lower.contains("unsupported template")
        || lower.contains("--count must")
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
        "brief" | "emit-brief" | "pack" if data["dry_run"].as_bool() == Some(true) => {
            println!("{command}: dry run");
            print_write_plan(data);
        }
        "fit" => {
            println!("fit: {}", data["status"].as_str().unwrap_or("unknown"));
            println!("{}", data["decision"].as_str().unwrap_or(""));
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
    fn brief_summary_exposes_status_artifacts_and_provenance() {
        let summary = summarize(
            "brief",
            &json!({
                "contract": "mdp.message-brief.v0",
                "channel": "email",
                "persona": "PMM",
                "job": "write outbound message",
                "draft_status": "ready",
                "fit": {"status": "fit"},
                "required_load_order": [".mdp/cards/personas.yaml", ".mdp/cards/claims.yaml"],
                "prospect_source": {"kind": "synthetic-example", "synthetic": true},
                "input_artifact": {"kind": "prospect", "path": "examples/clay-row.json"},
                "artifact": {"status": "stdout-only", "kind": "stdout", "path": null}
            }),
        );

        assert_eq!(summary["draft_status"], "ready");
        assert_eq!(summary["required_card_count"], 2);
        assert_eq!(summary["prospect_source"]["kind"], "synthetic-example");
        assert_eq!(summary["artifact"]["status"], "stdout-only");
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
                "fit": {"status": "fit"},
                "required_load_order": [".mdp/cards/personas.yaml"],
                "context": {
                    "contract": "mdp.context.v0",
                    "status": "ready",
                    "entries": [{"body": "should not appear in summary"}],
                    "full_card_required": [],
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
        assert!(summary["context"].get("entries").is_none());
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
    }
}
