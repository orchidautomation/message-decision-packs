use crate::models::Prospect;
use crate::pack_io::{read_manifest, read_prospect};
use crate::routing::select_cards;
use crate::utils::resolve_persona;
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
    let prospect = read_prospect(prospect_path)?;
    prospect_brief_from_value(root, prospect, channel, job)
}

pub(crate) fn prospect_brief_from_value(
    root: &Path,
    prospect: Prospect,
    channel: &str,
    job: Option<&str>,
) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let persona_resolution = resolve_persona(&manifest, &prospect);
    let fit_result = crate::commands::routing::fit_prospect(root, prospect.clone())?;
    let fit_status = fit_result["status"]
        .as_str()
        .unwrap_or("insufficient-context");
    let persona = persona_resolution.persona.clone();
    let prospect_source_kind = prospect
        .source_kind
        .clone()
        .unwrap_or_else(|| "prospect-json".to_string());
    let prospect_is_synthetic = prospect.synthetic;
    let job_text = job.unwrap_or("write outbound message");
    let route = select_cards(&manifest, Some(&persona), Some(job_text));
    let load_order: Vec<String> = route
        .iter()
        .filter_map(|v| v["path"].as_str().map(str::to_string))
        .collect();
    let draft_status = if fit_status == "fit" {
        "ready"
    } else {
        "no-draft"
    };
    Ok(json!({
        "contract": "mdp.message-brief.v0",
        "pack": {"id": manifest.id, "name": manifest.name, "version": manifest.version},
        "channel": channel,
        "prospect": prospect,
        "prospect_source": {
            "kind": prospect_source_kind,
            "synthetic": prospect_is_synthetic,
            "guidance": if prospect_is_synthetic { "Synthetic example fixture. Replace with a real or intentionally sanitized prospect row before production use." } else { "User-provided or sanitized prospect row." }
        },
        "persona": persona,
        "persona_resolution": persona_resolution,
        "fit": fit_result,
        "draft_status": draft_status,
        "draft_decision": if draft_status == "ready" { "Proceed with routed brief using stated assumptions." } else { "Do not draft outbound copy unless the user explicitly overrides this fit gate." },
        "job": job_text,
        "required_load_order": load_order,
        "route": route,
        "decision_trace": [
            {"step": "read_prospect", "reason": "use enriched Clay/Deepline-style row as task input"},
            {"step": "infer_or_use_persona", "reason": "map person title to pack persona"},
            {"step": "route_cards", "reason": "load only relevant message decision cards"},
            {"step": "generate_or_handoff", "reason": "use the brief as the agent/model input contract"}
        ],
        "agent_instruction": if draft_status == "ready" { "Read only required_load_order card files, combine them with prospect, then draft copy. Use the routed CTA policy when present. Do not invent claims outside the loaded cards." } else { "Stop before drafting. Surface the fit status and missing context/disqualifiers, then ask for explicit user override before creating outbound copy." }
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
    if brief["draft_status"] != "ready" {
        return Ok(json!({
            "contract": "mdp.copy-demo.v0",
            "channel": channel,
            "draft_status": "no-draft",
            "fit": brief["fit"].clone(),
            "decision": "Demo copy was not generated because the fit gate did not pass."
        }));
    }
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
    fn brief_marks_no_draft_when_fit_is_insufficient() {
        let root = temp_pack("brief-no-draft");
        let prospect_path = root.join("examples").join("thin.json");
        std::fs::write(
            &prospect_path,
            r#"{"name":"Taylor Lee","title":"GTM Engineering Lead","company":"ExampleCo"}"#,
        )
        .expect("prospect should be writable");

        let result =
            prospect_brief(&root, &prospect_path, "linkedin", None).expect("brief should succeed");

        assert_eq!(result["fit"]["status"], "insufficient-context");
        assert_eq!(result["draft_status"], "no-draft");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn brief_marks_starter_prospect_as_synthetic_example() {
        let root = temp_pack("brief-synthetic");
        let prospect_path = root.join("examples").join("clay-row.json");

        let result =
            prospect_brief(&root, &prospect_path, "linkedin", None).expect("brief should succeed");

        assert_eq!(result["prospect_source"]["kind"], "synthetic-example");
        assert_eq!(result["prospect_source"]["synthetic"], true);
        assert!(
            result["prospect_source"]["guidance"]
                .as_str()
                .expect("guidance should be a string")
                .contains("Synthetic example fixture")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn brief_routes_with_manifest_persona_mapping() {
        let root = temp_pack("brief-persona-mapping");
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

        let result =
            prospect_brief(&root, &prospect_path, "linkedin", None).expect("brief should succeed");

        assert_eq!(result["persona"], "PMM");
        assert_eq!(result["draft_status"], "ready");
        assert_eq!(
            result["persona_resolution"]["source"],
            "manifest.persona_mappings.title_keywords"
        );
        assert!(
            result["required_load_order"]
                .as_array()
                .expect("load order should be an array")
                .iter()
                .any(|path| path == ".mdp/cards/ctas.yaml")
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
