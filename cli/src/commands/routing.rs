use crate::pack_io::{read_card_by_id, read_manifest, read_prospect};
use crate::routing::{entry_route, select_cards};
use crate::utils::slugify;
use crate::utils::{prospect_haystack_with_persona, resolve_persona};
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

pub(crate) fn route(
    root: &Path,
    persona: &str,
    job: &str,
    include_entries: bool,
    include_eval_fixture: bool,
) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let selected = select_cards(&manifest, Some(persona), Some(job));
    let load_order: Vec<String> = selected
        .iter()
        .filter_map(|v| v["path"].as_str().map(str::to_string))
        .collect();
    let mut payload = json!({
        "persona": persona,
        "job": job,
        "route": selected,
        "decision_trace": [
            "manifest loaded",
            "persona matched against card metadata",
            "job keywords matched against card descriptions and tags",
            "base policy cards included for guardrails"
        ],
        "load_order": load_order
    });
    if include_entries || include_eval_fixture {
        let routed_entries = entry_route(root, &manifest, persona, job)?;
        if include_eval_fixture {
            payload["eval_fixture"] = eval_fixture(persona, job, &payload, &routed_entries);
        }
        if include_entries {
            payload["entry_route"] = json!(routed_entries);
        }
    }
    Ok(payload)
}

fn eval_fixture(persona: &str, job: &str, route_output: &Value, routed_entries: &Value) -> Value {
    let expected_titles: Vec<Value> = routed_entries["matches"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry["status"].as_str() == Some("required"))
        .filter_map(|entry| entry["title"].as_str().map(|title| json!(title)))
        .take(8)
        .collect();
    json!({
        "id": slugify(&format!("{persona}-{job}")),
        "command": "route",
        "persona": persona,
        "job": job,
        "expect_load_order_contains": route_output["load_order"],
        "expect_entry_titles_contains": expected_titles,
        "notes": [
            "Generated from current route output. Review before committing so evals encode desired behavior, not accidental noise.",
            "Add expect_load_order_excludes or expect_entry_titles_excludes for known wrong-route regressions."
        ]
    })
}

pub(crate) fn fit(root: &Path, prospect_path: &Path) -> Result<Value> {
    let prospect = read_prospect(prospect_path)?;
    fit_prospect(root, prospect)
}

pub(crate) fn fit_prospect(root: &Path, prospect: crate::models::Prospect) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let fit_card = read_card_by_id(root, "fit-rules")?;
    let mut matches = Vec::new();
    let mut disqualifiers = Vec::new();
    let persona_resolution = resolve_persona(&manifest, &prospect);
    let resolved_persona_for_fit = persona_resolution
        .fit_usable
        .then_some(persona_resolution.persona.as_str());
    let haystack = prospect_haystack_with_persona(&prospect, resolved_persona_for_fit);
    let context = fit_context(&prospect, &persona_resolution);

    for entry in &fit_card.entries {
        let entry_text = format!("{} {}", entry.title, entry.body).to_lowercase();
        let applies = entry.applies_to.iter().any(|candidate| {
            haystack.contains(&candidate.to_lowercase())
                || prospect
                    .segment
                    .as_ref()
                    .map(|s| s.eq_ignore_ascii_case(candidate))
                    .unwrap_or(false)
        });
        let keyword_match = entry_text
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|token| token.len() >= 5)
            .any(|token| haystack.contains(token));
        if entry.avoid.is_empty() && (applies || keyword_match) {
            matches.push(json!({"id": entry.id, "title": entry.title, "reason": if applies { "segment/persona match" } else { "keyword match" }}));
        }
        for avoid in &entry.avoid {
            if haystack.contains(&avoid.to_lowercase()) {
                disqualifiers
                    .push(json!({"entry_id": entry.id, "term": avoid, "title": entry.title}));
            }
        }
    }

    let status = if !disqualifiers.is_empty() {
        "disqualified"
    } else if !context["ready"].as_bool().unwrap_or(false) {
        "insufficient-context"
    } else if !matches.is_empty() {
        "fit"
    } else {
        "insufficient-context"
    };
    Ok(json!({
        "contract": "mdp.fit.v0",
        "prospect": prospect,
        "persona_resolution": persona_resolution,
        "status": status,
        "context": context,
        "matches": matches,
        "disqualifiers": disqualifiers,
        "decision": match status {
            "fit" => "Proceed to route/brief with stated assumptions.",
            "disqualified" => "Do not draft outbound copy unless the user overrides the disqualifier.",
            _ => "Ask for more context before drafting.",
        }
    }))
}

fn fit_context(
    prospect: &crate::models::Prospect,
    persona_resolution: &crate::utils::PersonaResolution,
) -> Value {
    let has_trigger = prospect
        .trigger
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_persona = persona_resolution.fit_usable;
    let has_segment = prospect
        .segment
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_background = prospect
        .background
        .as_ref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_signal = !prospect.signals.is_empty();
    let has_source = prospect.signals.iter().any(|signal| {
        signal
            .source
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty())
    });
    let mut missing = Vec::new();
    if !has_trigger {
        missing.push("trigger");
    }
    if !has_persona {
        missing.push("persona");
    }
    if !has_segment {
        missing.push("segment");
    }
    if !has_signal {
        missing.push("signals");
    }
    if !has_source {
        missing.push("source");
    }
    json!({
        "ready": has_trigger && has_persona && has_segment && has_signal && has_source,
        "has_background": has_background,
        "missing": missing
    })
}

pub(crate) fn check_claims(root: &Path, text: Option<&str>, file: Option<&Path>) -> Result<Value> {
    let raw = match (text, file) {
        (Some(value), None) => value.to_string(),
        (None, Some(path)) => {
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?
        }
        (Some(_), Some(_)) => return Err(anyhow!("pass either --text or --file, not both")),
        (None, None) => return Err(anyhow!("pass --text or --file")),
    };
    let lower = raw.to_lowercase();
    let claims_card = read_card_by_id(root, "claims")?;
    let avoid_card = read_card_by_id(root, "avoid-rules")?;
    let approved_claim_context = claims_card
        .entries
        .iter()
        .map(|entry| {
            format!(
                "{} {} {}",
                entry.title,
                entry.body,
                entry.evidence.join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let mut matched_claims = Vec::new();
    let mut claim_gaps = Vec::new();
    let mut guardrail_hits = Vec::new();
    let unsupported_claims = unsupported_claims(&lower, &approved_claim_context);

    for entry in &claims_card.entries {
        let title = entry.title.to_lowercase();
        let title_match = title.len() > 4 && lower.contains(&title);
        let evidence_missing = entry.evidence.is_empty();
        if title_match {
            matched_claims.push(json!({"id": entry.id, "title": entry.title, "evidence": entry.evidence, "evidence_missing": evidence_missing}));
            if evidence_missing {
                claim_gaps.push(json!({"id": entry.id, "title": entry.title, "reason": "matched claim has no evidence"}));
            }
        }
    }
    for entry in &avoid_card.entries {
        for term in &entry.avoid {
            if lower.contains(&term.to_lowercase()) {
                guardrail_hits
                    .push(json!({"entry_id": entry.id, "term": term, "title": entry.title}));
            }
        }
    }
    let valid = guardrail_hits.is_empty() && claim_gaps.is_empty() && unsupported_claims.is_empty();
    Ok(json!({
        "contract": "mdp.claim-check.v0",
        "valid": valid,
        "matched_claims": matched_claims,
        "claim_gaps": claim_gaps,
        "guardrail_hits": guardrail_hits,
        "unsupported_claims": unsupported_claims,
        "decision": if valid { "claim-safe" } else { "needs-revision" }
    }))
}

fn unsupported_claims(text: &str, approved_context: &str) -> Vec<Value> {
    let mut hits = Vec::new();
    let mut push_hit = |category: &str, trigger: &str, reason: &str| {
        if !approved_context.contains(trigger) {
            hits.push(json!({
                "category": category,
                "trigger": trigger,
                "reason": reason
            }));
        }
    };

    let has_number = text.chars().any(|c| c.is_ascii_digit());
    if (text.contains('%') || text.contains(" percent") || has_number)
        && contains_any(
            text,
            &[
                "reply rate",
                "reply rates",
                "meetings",
                "pipeline",
                "revenue",
                "conversion",
                "roi",
            ],
        )
    {
        push_hit(
            "quantified-outcome",
            "quantified outcome",
            "Quantified performance or ROI claims require explicit approved evidence.",
        );
    }
    if contains_any(text, &["guarantee", "guarantees", "guaranteed"])
        && contains_any(text, &["meeting", "meetings", "reply", "replies"])
    {
        push_hit(
            "quantified-outcome",
            "guarantee",
            "Guaranteed meetings, replies, or outcomes are unsupported unless explicitly approved.",
        );
    }
    if contains_any(
        text,
        &[
            "integrates with",
            "integration with",
            "connects to",
            "syncs with",
        ],
    ) && contains_any(text, &["salesforce", "hubspot", "outreach", "salesloft"])
    {
        push_hit(
            "integration",
            "integration",
            "Integration claims require an approved product capability claim.",
        );
    }
    if contains_any(
        text,
        &[
            "soc 2",
            "soc2",
            "hipaa",
            "gdpr",
            "compliant",
            "compliance",
            "secure",
            "security certified",
        ],
    ) {
        push_hit(
            "compliance-security",
            "compliance/security",
            "Compliance and security claims require explicit approved evidence.",
        );
    }
    if contains_any(
        text,
        &[
            "trusted by",
            "used by",
            "customers include",
            "customer like",
            "customers like",
        ],
    ) {
        push_hit(
            "customer-name",
            "customer proof",
            "Customer-name and social-proof claims require explicit approved source context.",
        );
    }
    if contains_any(
        text,
        &[
            "updates crm",
            "update crm",
            "writes to crm",
            "send emails",
            "sends emails",
            "send linkedin",
            "sends linkedin",
            "auto-send",
            "autosend",
            "sequence prospects",
            "sequencer",
        ],
    ) {
        push_hit(
            "execution-crm-sending",
            "execution",
            "MDP stops at pack, route, and brief; execution claims require a separate exact-action tool.",
        );
    }

    hits
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_pack(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-{name}-{nonce}"));
        init_pack(&root, "Example Message Pack", "gtm", true)
            .expect("starter pack should initialize");
        root
    }

    #[test]
    fn route_preserves_skill_load_order_contract() {
        let root = temp_pack("route-contract");

        let result = route(
            &root,
            "GTM Engineering",
            "linkedin outbound copy",
            false,
            false,
        )
        .expect("route should succeed");
        let load_order: Vec<&str> = result["load_order"]
            .as_array()
            .expect("load_order should be an array")
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("load_order entries should be strings")
            })
            .collect();

        assert_eq!(
            &load_order[..2],
            &[".mdp/cards/personas.yaml", ".mdp/cards/avoid-rules.yaml"]
        );
        assert!(load_order.contains(&".mdp/cards/ctas.yaml"));
        assert!(load_order.len() <= 12);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn linkedin_entry_route_excludes_email_and_call_prep_entries() {
        let root = temp_pack("linkedin-entry-route");

        let result = route(&root, "PMM", "linkedin outbound copy", true, false)
            .expect("route should succeed");
        let titles: Vec<&str> = result["entry_route"]["matches"]
            .as_array()
            .expect("entry matches array")
            .iter()
            .filter_map(|entry| entry["title"].as_str())
            .collect();

        assert!(titles.contains(&"LinkedIn opener"));
        assert!(!titles.contains(&"Email follow-up"));
        assert!(!titles.contains(&"Call prep"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unknown_task_does_not_match_ask_substring() {
        let root = temp_pack("route-token-boundary");

        let result =
            route(&root, "Unknown", "task hygiene", false, false).expect("route should succeed");
        let ids: Vec<&str> = result["route"]
            .as_array()
            .expect("route array")
            .iter()
            .filter_map(|entry| entry["id"].as_str())
            .collect();

        assert_eq!(ids, vec!["personas", "avoid-rules"]);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_gate_allows_generated_prospect() {
        let root = temp_pack("fit-contract");
        let prospect_path = root.join("examples").join("clay-row.json");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["contract"], "mdp.fit.v0");
        assert_eq!(result["status"], "fit");
        assert!(result["matches"].as_array().expect("matches array").len() > 0);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_gate_uses_manifest_persona_mapping_for_titles() {
        let root = temp_pack("fit-persona-mapping");
        let prospect_path = root.join("examples").join("demand-gen.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents",
  "signals": [{"id": "agent-gtm-workflow", "title": "Building multi-agent GTM workflow", "source": "example row"}]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "fit");
        assert_eq!(result["persona_resolution"]["persona"], "PMM");
        assert_eq!(
            result["persona_resolution"]["source"],
            "manifest.persona_mappings.title_keywords"
        );
        assert_eq!(result["context"]["missing"].as_array().unwrap().len(), 0);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_gate_still_requires_persona_when_no_mapping_matches() {
        let root = temp_pack("fit-no-persona-mapping");
        let prospect_path = root.join("examples").join("chief-of-staff.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Chief of Staff",
  "company": "ExampleCo",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents",
  "signals": [{"id": "agent-gtm-workflow", "title": "Building multi-agent GTM workflow", "source": "example row"}]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "insufficient-context");
        assert_eq!(result["persona_resolution"]["source"], "fallback");
        assert!(
            result["context"]["missing"]
                .as_array()
                .expect("missing context array")
                .iter()
                .any(|item| item == "persona")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn route_can_emit_eval_fixture_scaffold_from_actual_matches() {
        let root = temp_pack("route-eval-fixture");

        let result = route(&root, "PMM", "linkedin outbound copy", false, true)
            .expect("route should succeed");

        assert_eq!(result["eval_fixture"]["command"], "route");
        assert_eq!(result["eval_fixture"]["persona"], "PMM");
        assert_eq!(result["eval_fixture"]["job"], "linkedin outbound copy");
        assert!(
            result["eval_fixture"]["expect_load_order_contains"]
                .as_array()
                .expect("expected load order should be an array")
                .iter()
                .any(|path| path == ".mdp/cards/ctas.yaml")
        );
        assert!(
            result["eval_fixture"]["notes"][0]
                .as_str()
                .expect("fixture note should be a string")
                .contains("Review before committing")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_gate_rejects_thin_gtm_title_without_context() {
        let root = temp_pack("fit-thin");
        let prospect_path = root.join("examples").join("thin.json");
        std::fs::write(
            &prospect_path,
            r#"{"name":"Taylor Lee","title":"GTM Engineering Lead","company":"ExampleCo"}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "insufficient-context");
        assert!(
            result["context"]["missing"]
                .as_array()
                .expect("missing context array")
                .iter()
                .any(|item| item == "trigger")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_check_flags_execution_category_drift() {
        let root = temp_pack("claim-contract");

        let result = check_claims(
            &root,
            Some("This turns your messaging pack into an AI SDR."),
            None,
        )
        .expect("claim check should succeed");

        assert_eq!(result["contract"], "mdp.claim-check.v0");
        assert_eq!(result["valid"], false);
        assert!(
            result["guardrail_hits"]
                .as_array()
                .expect("guardrail_hits array")
                .iter()
                .any(|hit| hit["term"] == "AI SDR")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_check_flags_obvious_unsupported_claims() {
        let root = temp_pack("claim-unsupported");

        for text in [
            "MDP guarantees meetings with enterprise buyers.",
            "MDP improves reply rates by 30%.",
            "MDP integrates with Salesforce and HubSpot.",
            "MDP updates CRM fields after every send.",
            "MDP is SOC 2 compliant and trusted by Acme.",
        ] {
            let result = check_claims(&root, Some(text), None).expect("claim check should succeed");
            assert_eq!(result["valid"], false, "text should fail: {text}");
            assert!(
                result["unsupported_claims"]
                    .as_array()
                    .expect("unsupported_claims array")
                    .len()
                    > 0,
                "text should produce unsupported claim: {text}"
            );
        }

        let _ = std::fs::remove_dir_all(root);
    }
}
