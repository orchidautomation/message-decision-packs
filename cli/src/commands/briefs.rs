use crate::models::Prospect;
use crate::pack_io::{read_manifest, read_prospect};
use crate::routing::select_cards;
use crate::utils::infer_persona;
use anyhow::Result;
use serde_json::{Value, json};
use std::path::Path;

pub(crate) fn emit_brief(
    root: &Path,
    persona: &str,
    motion: Option<&str>,
    job: Option<&str>,
) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let job_text = job.unwrap_or("unspecified GTM decision task");
    let selected = select_cards(&manifest, Some(persona), Some(job_text));
    let load_order: Vec<String> = selected
        .iter()
        .filter_map(|v| v["path"].as_str().map(str::to_string))
        .collect();
    Ok(json!({
        "contract": "mdp.brief.v0",
        "pack": {"id": manifest.id, "name": manifest.name, "version": manifest.version},
        "inputs": {"persona": persona, "motion": motion, "job": job_text},
        "required_load_order": load_order,
        "decision_trace": [
            {"step": "load_manifest", "reason": "discover pack metadata and card index"},
            {"step": "route_cards", "reason": "preserve progressive disclosure"},
            {"step": "apply_avoid_rules", "reason": "prevent category drift and unsupported claims"},
            {"step": "draft_or_decide", "reason": "use only loaded card evidence and cite gaps"}
        ],
        "output_requirements": {"state_assumptions": true, "cite_loaded_cards": true, "surface_gaps": true, "avoid_execution_claims": true, "use_loaded_cta_policy": true}
    }))
}

pub(crate) fn prospect_brief(
    root: &Path,
    prospect_path: &Path,
    channel: &str,
    job: Option<&str>,
) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let prospect = read_prospect(prospect_path)?;
    let persona = prospect
        .persona
        .as_deref()
        .unwrap_or_else(|| infer_persona(&prospect.title));
    let job_text = job.unwrap_or("write outbound message");
    let route = select_cards(&manifest, Some(persona), Some(job_text));
    let load_order: Vec<String> = route
        .iter()
        .filter_map(|v| v["path"].as_str().map(str::to_string))
        .collect();
    Ok(json!({
        "contract": "mdp.message-brief.v0",
        "pack": {"id": manifest.id, "name": manifest.name, "version": manifest.version},
        "channel": channel,
        "prospect": prospect,
        "persona": persona,
        "job": job_text,
        "required_load_order": load_order,
        "route": route,
        "decision_trace": [
            {"step": "read_prospect", "reason": "use enriched Clay/Deepline-style row as task input"},
            {"step": "infer_or_use_persona", "reason": "map person title to pack persona"},
            {"step": "route_cards", "reason": "load only relevant message decision cards"},
            {"step": "generate_or_handoff", "reason": "use the brief as the agent/model input contract"}
        ],
        "agent_instruction": "Read only required_load_order card files, combine them with prospect, then draft copy. Use the routed CTA policy when present. Do not invent claims outside the loaded cards."
    }))
}

pub(crate) fn demo_copy(root: &Path, prospect_path: &Path, channel: &str) -> Result<Value> {
    let brief = prospect_brief(
        root,
        prospect_path,
        channel,
        Some("write linkedin outbound copy"),
    )?;
    let prospect: Prospect = serde_json::from_value(brief["prospect"].clone())?;
    let persona = brief["persona"].as_str().unwrap_or("finance leader");
    let trigger = prospect
        .trigger
        .as_deref()
        .unwrap_or("scaling finance operations");
    let background = prospect
        .background
        .as_deref()
        .unwrap_or("working on finance systems");
    let first_name = prospect
        .name
        .split_whitespace()
        .next()
        .unwrap_or(&prospect.name);

    let (recommended, shorter, proof_led) = (
        format!(
            "Hey {first_name} - saw you're {background}. If {company} is {trigger}, a Message Decision Pack can keep persona, pain, hooks, CTA rules, and avoid-rules consistent across agents. Worth comparing notes?",
            company = prospect.company
        ),
        format!(
            "Hey {first_name} - noticed {company} is {trigger}. MDP helps teams version their GTM message context so agents draft from the same approved source. Open to a quick compare?",
            company = prospect.company
        ),
        format!(
            "Hey {first_name} - given your {title} role, thought a lightweight message decision layer could be relevant for keeping agent-generated GTM copy consistent.",
            title = prospect.title
        ),
    );

    Ok(json!({
        "contract": "mdp.copy-demo.v0",
        "channel": channel,
        "persona": persona,
        "prospect": {
            "name": prospect.name,
            "title": prospect.title,
            "company": prospect.company,
            "linkedin_url": prospect.linkedin_url
        },
        "recommended": recommended,
        "alternates": [shorter, proof_led],
        "decision_trace": brief["decision_trace"].clone(),
        "cards_used": brief["required_load_order"].clone(),
        "notes": [
            "Deterministic demo copy only; production should pass the brief to a model.",
            "No LinkedIn, Clay, CRM, or sequencer write was performed."
        ]
    }))
}
