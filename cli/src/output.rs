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
            "root": data["root"],
            "pack_dir": data["pack_dir"],
            "manifest": data["manifest"],
            "source_ledger": data["source_ledger"],
            "example_prospect": data["example_prospect"],
            "example_prospect_kind": data["example_prospect_kind"],
            "next_commands": data["next_commands"]
        }),
        "doctor" | "validate" => json!({
            "valid": data["valid"],
            "issue_count": array_len(&data["issues"]),
            "issues": data["issues"]
        }),
        "route" => json!({
            "persona": data["persona"],
            "job": data["job"],
            "card_count": array_len(&data["load_order"]),
            "load_order": data["load_order"],
            "entry_match_count": array_len(&data["entry_route"]["matches"]),
            "entry_gap_count": array_len(&data["entry_route"]["gaps"]),
            "eval_fixture": data["eval_fixture"]
        }),
        "fit" => json!({
            "status": data["status"],
            "decision": data["decision"],
            "match_count": array_len(&data["matches"]),
            "disqualifier_count": array_len(&data["disqualifiers"]),
            "missing_context": data["context"]["missing"]
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
            "artifact": data["artifact"]
        }),
        "emit-brief" => json!({
            "contract": data["contract"],
            "persona": data["inputs"]["persona"],
            "job": data["inputs"]["job"],
            "required_card_count": array_len(&data["required_load_order"]),
            "required_load_order": data["required_load_order"],
            "artifact": data["artifact"]
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
            "artifact": data["artifact"]
        }),
        "check-claims" => json!({
            "valid": data["valid"],
            "decision": data["decision"],
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
    let payload = json!({"ok": false, "error": {"code": "mdp_error", "message": err.to_string(), "details": []}});
    if json_mode {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        eprintln!("error: {}", err);
    }
    Ok(())
}

fn print_human(command: &str, data: &Value) -> Result<()> {
    match command {
        "init" => {
            println!(
                "Created MDP package at {}",
                data["pack_dir"].as_str().unwrap_or("")
            );
            println!(
                "Next: mdp validate --dir {}",
                data["root"].as_str().unwrap_or(".")
            );
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
        _ => println!("{}", serde_json::to_string_pretty(data)?),
    }
    Ok(())
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
}
