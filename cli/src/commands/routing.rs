use crate::pack_io::{read_card_by_id, read_manifest, read_prospect};
use crate::routing::{entry_route, select_cards};
use crate::utils::prospect_haystack;
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

pub(crate) fn route(root: &Path, persona: &str, job: &str, include_entries: bool) -> Result<Value> {
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
    if include_entries {
        payload["entry_route"] = json!(entry_route(root, &manifest, persona, job)?);
    }
    Ok(payload)
}

pub(crate) fn fit(root: &Path, prospect_path: &Path) -> Result<Value> {
    let prospect = read_prospect(prospect_path)?;
    let fit_card = read_card_by_id(root, "fit-rules")?;
    let mut matches = Vec::new();
    let mut disqualifiers = Vec::new();
    let haystack = prospect_haystack(&prospect);

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
    } else if !matches.is_empty() {
        "fit"
    } else {
        "insufficient-context"
    };
    Ok(json!({
        "contract": "mdp.fit.v0",
        "prospect": prospect,
        "status": status,
        "matches": matches,
        "disqualifiers": disqualifiers,
        "decision": match status {
            "fit" => "Proceed to route/brief with stated assumptions.",
            "disqualified" => "Do not draft outbound copy unless the user overrides the disqualifier.",
            _ => "Ask for more context before drafting.",
        }
    }))
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
    let mut matched_claims = Vec::new();
    let mut claim_gaps = Vec::new();
    let mut guardrail_hits = Vec::new();

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
    Ok(json!({
        "contract": "mdp.claim-check.v0",
        "valid": guardrail_hits.is_empty() && claim_gaps.is_empty(),
        "matched_claims": matched_claims,
        "claim_gaps": claim_gaps,
        "guardrail_hits": guardrail_hits,
        "decision": if guardrail_hits.is_empty() && claim_gaps.is_empty() { "claim-safe" } else { "needs-revision" }
    }))
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

        let result = route(&root, "GTM Engineering", "linkedin outbound copy", false)
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
}
