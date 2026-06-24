use crate::constants::DEFAULT_DIR;
use crate::models::{CardKind, Manifest};
use crate::pack_io::read_card;
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

pub(crate) fn select_cards(
    manifest: &Manifest,
    persona: Option<&str>,
    job: Option<&str>,
) -> Vec<Value> {
    let persona_lower = persona.map(|p| p.to_lowercase());
    let job_lower = job.unwrap_or("").to_lowercase();
    let is_message_job = is_message_job(&job_lower);
    let mut selected = Vec::new();
    let mut candidates = Vec::new();

    for card in &manifest.cards {
        if matches!(card.kind, CardKind::Personas | CardKind::AvoidRules) {
            selected.push(json!({"id": card.id, "kind": card.kind, "path": format!("{DEFAULT_DIR}/{}", card.path), "reason": "base guardrail", "description": card.description}));
        }
    }

    for (index, card) in manifest.cards.iter().enumerate() {
        if matches!(card.kind, CardKind::Personas | CardKind::AvoidRules) {
            continue;
        }
        let persona_match = persona_lower
            .as_ref()
            .map(|p| {
                card.personas
                    .iter()
                    .any(|candidate| candidate.to_lowercase() == *p)
                    || card.description.to_lowercase().contains(p)
            })
            .unwrap_or(false);
        let job_match = !job_lower.is_empty()
            && (card.description.to_lowercase().contains(&job_lower)
                || card.tags.iter().any(|tag| {
                    job_lower.contains(&tag.to_lowercase())
                        || tag.to_lowercase().contains(&job_lower)
                }));
        if persona_match || job_match {
            let reason = match (persona_match, job_match) {
                (true, true) => "persona and job/tag match",
                (true, false) => "persona match",
                (false, true) => "job/tag match",
                (false, false) => "matched",
            };
            candidates.push((
                card_priority(&card.kind, is_message_job),
                index,
                json!({"id": card.id, "kind": card.kind, "path": format!("{DEFAULT_DIR}/{}", card.path), "reason": reason, "description": card.description}),
            ));
        }
    }

    candidates.sort_by_key(|(priority, index, _)| (*priority, *index));
    for (_, _, card) in candidates {
        if selected.len() >= manifest.policy.max_cards_per_route {
            break;
        }
        selected.push(card);
    }
    selected
}

fn is_message_job(job_lower: &str) -> bool {
    [
        "copy", "outbound", "linkedin", "email", "message", "brief", "cta", "ask", "reply",
    ]
    .iter()
    .any(|token| job_lower.contains(token))
}

fn card_priority(kind: &CardKind, is_message_job: bool) -> usize {
    if is_message_job {
        match kind {
            CardKind::Personas | CardKind::AvoidRules => 0,
            CardKind::FitRules => 5,
            CardKind::Positioning => 10,
            CardKind::Pains => 20,
            CardKind::Signals => 25,
            CardKind::Hooks => 30,
            CardKind::Claims => 35,
            CardKind::CopyPatterns => 40,
            CardKind::Ctas => 45,
            CardKind::ChannelPolicies => 50,
            CardKind::Objections => 60,
            CardKind::Motions => 70,
            CardKind::Gaps => 80,
        }
    } else {
        match kind {
            CardKind::Personas | CardKind::AvoidRules => 0,
            CardKind::FitRules => 5,
            CardKind::Positioning => 10,
            CardKind::Motions => 20,
            CardKind::Signals => 30,
            CardKind::Pains => 40,
            CardKind::Claims => 50,
            CardKind::ChannelPolicies => 60,
            CardKind::Objections => 70,
            CardKind::Hooks => 80,
            CardKind::CopyPatterns => 90,
            CardKind::Ctas => 100,
            CardKind::Gaps => 110,
        }
    }
}

pub(crate) fn entry_route(
    root: &Path,
    manifest: &Manifest,
    persona: &str,
    job: &str,
) -> Result<Value> {
    let selected = select_cards(manifest, Some(persona), Some(job));
    let selected_ids: BTreeSet<String> = selected
        .iter()
        .filter_map(|value| value["id"].as_str().map(str::to_string))
        .collect();
    let persona_lower = persona.to_lowercase();
    let job_lower = job.to_lowercase();
    let job_tokens: Vec<String> = job_lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| token.len() >= 4)
        .map(str::to_string)
        .collect();
    let mut matches = Vec::new();
    let mut gaps = Vec::new();

    for card_ref in &manifest.cards {
        if !selected_ids.contains(&card_ref.id) {
            continue;
        }
        let card = read_card(&root.join(DEFAULT_DIR).join(&card_ref.path))?;
        let mut card_match_count = 0usize;
        for entry in &card.entries {
            let entry_text = format!(
                "{} {} {}",
                entry.title,
                entry.body,
                entry.applies_to.join(" ")
            )
            .to_lowercase();
            let applies = entry.applies_to.is_empty()
                || entry
                    .applies_to
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(persona));
            let job_match = card.tags.iter().any(|tag| {
                let tag_lower = tag.to_lowercase();
                job_lower.contains(&tag_lower) || tag_lower.contains(&job_lower)
            }) || job_tokens.iter().any(|token| entry_text.contains(token));
            let persona_match = entry_text.contains(&persona_lower);
            if applies || job_match || persona_match {
                card_match_count += 1;
                matches.push(json!({
                    "card_id": card.id,
                    "card_kind": card.kind,
                    "entry_id": entry.id,
                    "title": entry.title,
                    "status": if matches!(card.kind, CardKind::AvoidRules | CardKind::FitRules | CardKind::Claims | CardKind::Positioning | CardKind::ChannelPolicies) { "required" } else { "supporting" },
                    "reason": if applies { "persona applies" } else if job_match { "job/tag match" } else { "persona text match" },
                    "evidence_count": entry.evidence.len(),
                    "avoid_count": entry.avoid.len()
                }));
            }
        }
        if card_match_count == 0 {
            gaps.push(json!({
                "card_id": card.id,
                "path": format!("{DEFAULT_DIR}/{}", card_ref.path),
                "reason": "card routed, but no entry matched persona/job cleanly"
            }));
        }
    }

    Ok(json!({
        "contract": "mdp.entry-route.v0",
        "persona": persona,
        "job": job,
        "matches": matches,
        "gaps": gaps,
        "policy": "Load matched entries first. Load the full card only when an entry is ambiguous, missing, or a guardrail card needs complete review."
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CardRef, Policy, Provenance};

    fn manifest(max_cards_per_route: usize) -> Manifest {
        Manifest {
            format: "mdp.v0".to_string(),
            id: "test".to_string(),
            name: "Test".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            personas: vec!["PMM".to_string()],
            target_personas: vec![],
            operator_roles: vec![],
            supported_channels: vec![],
            cards: vec![
                CardRef {
                    id: "personas".to_string(),
                    path: "cards/personas.yaml".to_string(),
                    kind: CardKind::Personas,
                    description: "Personas".to_string(),
                    personas: vec!["PMM".to_string()],
                    tags: vec!["persona".to_string()],
                },
                CardRef {
                    id: "avoid-rules".to_string(),
                    path: "cards/avoid-rules.yaml".to_string(),
                    kind: CardKind::AvoidRules,
                    description: "Avoid".to_string(),
                    personas: vec!["PMM".to_string()],
                    tags: vec!["avoid".to_string()],
                },
                CardRef {
                    id: "ctas".to_string(),
                    path: "cards/ctas.yaml".to_string(),
                    kind: CardKind::Ctas,
                    description: "CTA policy".to_string(),
                    personas: vec!["PMM".to_string()],
                    tags: vec!["cta".to_string()],
                },
                CardRef {
                    id: "motions".to_string(),
                    path: "cards/motions.yaml".to_string(),
                    kind: CardKind::Motions,
                    description: "Motions".to_string(),
                    personas: vec!["PMM".to_string()],
                    tags: vec!["motion".to_string()],
                },
            ],
            policy: Policy {
                progressive_disclosure: true,
                load_manifest_first: true,
                max_cards_per_route,
                json_contract: "mdp.cli.v0".to_string(),
                no_auth_required: true,
            },
            provenance: Provenance {
                owner: "local".to_string(),
                created_by: "test".to_string(),
                notes: vec![],
            },
        }
    }

    #[test]
    fn select_cards_keeps_base_guardrails_and_message_priority() {
        let selected = select_cards(&manifest(4), Some("PMM"), Some("linkedin outbound copy"));
        let ids: Vec<&str> = selected
            .iter()
            .filter_map(|card| card["id"].as_str())
            .collect();
        assert_eq!(ids, vec!["personas", "avoid-rules", "ctas", "motions"]);
    }

    #[test]
    fn select_cards_respects_route_card_limit_after_base_cards() {
        let selected = select_cards(&manifest(2), Some("PMM"), Some("linkedin outbound copy"));
        let ids: Vec<&str> = selected
            .iter()
            .filter_map(|card| card["id"].as_str())
            .collect();
        assert_eq!(ids, vec!["personas", "avoid-rules"]);
    }
}
