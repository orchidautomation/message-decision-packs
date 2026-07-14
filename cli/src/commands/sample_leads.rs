use crate::models::TargetIdentity;
use crate::pack_io::read_manifest;
use crate::routing::{entry_route, select_cards};
use crate::utils::{resolve_persona_label, routable_persona};
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::path::Path;

const FIXTURE_CONTRACT: &str = "mdp.sample-leads.v0";
const FIXTURE_SOURCE_KIND: &str = "synthetic-example";

pub(crate) fn sample_leads(
    root: &Path,
    persona: &str,
    job: &str,
    count: usize,
    seed: u64,
) -> Result<Value> {
    if !(2..=5).contains(&count) {
        return Err(anyhow!("--count must be between 2 and 5"));
    }

    let manifest = read_manifest(root)?;
    let persona_resolution = resolve_persona_label(&manifest, persona);
    let resolved_persona = routable_persona(persona, &persona_resolution);
    let route = select_cards(&manifest, Some(resolved_persona), Some(job));
    let load_order: Vec<Value> = route
        .iter()
        .filter_map(|card| card["path"].as_str().map(|path| json!(path)))
        .collect();
    let entries = entry_route(root, &manifest, resolved_persona, job)?;
    let matched_entry_titles: Vec<Value> = entries["matches"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["title"].as_str().map(|title| json!(title)))
        .take(6)
        .collect();
    let fixture_leads: Vec<Value> = (0..count)
        .map(|index| {
            fixture_lead(
                persona,
                resolved_persona,
                job,
                seed,
                index,
                &matched_entry_titles,
                manifest.target.as_ref(),
            )
        })
        .collect();
    let workflow = if manifest.target.is_some() {
        json!([
            "Generate 2 to 5 synthetic fixture leads.",
            "Save one fixture row to ignored scratch if a command requires a prospect file.",
            "Run the pack's fit gate and brief gate for each fixture.",
            "Draft only from safe_personalization while retaining every known gap.",
            "Check claims before treating any copy as ready.",
            "Never treat fixture leads as real prospects or evidence about the target."
        ])
    } else {
        json!([
            "Generate 2 to 5 fake fixture leads.",
            "Save one fixture row to ignored scratch if a command requires --prospect.",
            "Run mdp fit, then mdp brief --context for each fixture.",
            "Draft only against safe_personalization and known_gaps.",
            "Run mdp check-claims before treating copy as ready.",
            "Never treat fixture leads as real prospects."
        ])
    };

    Ok(json!({
        "contract": FIXTURE_CONTRACT,
        "pack": {"id": manifest.id, "name": manifest.name, "version": manifest.version},
        "inputs": {
            "persona": resolved_persona,
            "requested_persona": persona,
            "job": job,
            "count": count,
            "seed": seed,
            "deterministic": true
        },
        "persona_resolution": persona_resolution,
        "fixture_notice": {
            "source_kind": FIXTURE_SOURCE_KIND,
            "synthetic": true,
            "do_not_contact": true,
            "guidance": "Fake fixture prospects for local outbound-copy testing only. Do not enrich, research, upload, sequence, send to, or treat them as real people or accounts."
        },
        "route": {
            "load_order": load_order,
            "matched_entry_titles": matched_entry_titles,
            "gaps": entries["gaps"].clone()
        },
        "fixture_leads": fixture_leads,
        "workflow": workflow
    }))
}

fn fixture_lead(
    requested_persona: &str,
    persona: &str,
    job: &str,
    seed: u64,
    index: usize,
    matched_entry_titles: &[Value],
    target: Option<&TargetIdentity>,
) -> Value {
    let offset = seed as usize + index;
    let name = pick(
        &[
            "Avery Fixture",
            "Jordan Example",
            "Riley Testrow",
            "Morgan Sample",
            "Casey Placeholder",
        ],
        offset,
    );
    let company = pick(
        &[
            "Northstar Example Labs",
            "FixtureWorks GTM",
            "Acme Test Systems",
            "Placeholder Revenue Co",
            "SampleStack Labs",
        ],
        offset,
    );
    let focus = target.map_or_else(
        || {
            pick(
                &[
                    "standardizing agent-generated outbound context",
                    "testing a new message QA workflow before campaign launch",
                    "reducing claim drift across GTM agent handoffs",
                    "turning supplied source rows into safer message briefs",
                    "evaluating first-touch email constraints across draft variants",
                ],
                offset,
            )
            .to_string()
        },
        |target| {
            format!(
                "testing evidence-gated messaging decisions for {} with synthetic inputs",
                target.name
            )
        },
    );
    let source_signal = target.map_or_else(
        || {
            pick(
                &[
                    "synthetic fixture row: no external source",
                    "fake QA signal: generated only for MDP evals",
                    "local test fixture: intentionally not research-backed",
                    "synthetic row: no LinkedIn, CRM, Clay, or web lookup",
                    "fixture-only context: created by mdp sample-leads",
                ],
                offset,
            )
            .to_string()
        },
        |_| "synthetic fixture row: no external source or real account research".to_string(),
    );
    let title = title_for_persona(persona, offset);
    let matched_titles: Vec<String> = matched_entry_titles
        .iter()
        .filter_map(|title| title.as_str().map(str::to_string))
        .take(3)
        .collect();
    let observed_context = format!(
        "{company} is a fictional account used to test {job}. The only observed context is that the fake {persona} buyer is {focus}."
    );
    let fit_reason = target.map_or_else(
        || {
            format!(
                "Synthetic fit because the fixture explicitly names {persona}, agent-assisted GTM context, and a source-labeled trigger for local copy testing."
            )
        },
        |target| {
            format!(
                "Synthetic workflow context only; this fixture does not establish real-account fit or product evidence for {}.",
                target.name
            )
        },
    );
    let known_gaps = vec![
        "No real person, company, profile, buying intent, or account research exists.".to_string(),
        "No customer proof, integration claim, performance metric, or urgency is supported."
            .to_string(),
        "Personalization must stay framed as a test hypothesis, not a researched observation."
            .to_string(),
    ];
    let mut safe_personalization = vec![
        format!("Use only the fixture premise: {focus}."),
        format!("Say this is a hypothesis for a {persona} owner, not a confirmed priority."),
        "Avoid implying the sender researched the person or company.".to_string(),
    ];
    if let Some(target) = target {
        safe_personalization.push(format!(
            "Do not infer any capability, outcome, customer proof, or fit claim for {} from this fixture.",
            target.name
        ));
    }
    if !matched_titles.is_empty() {
        safe_personalization.push(format!(
            "Route matched entries available for testing: {}.",
            matched_titles.join(", ")
        ));
    }

    json!({
        "id": format!("fixture-lead-{}", index + 1),
        "name": name,
        "title": title,
        "company": company,
        "source_kind": FIXTURE_SOURCE_KIND,
        "synthetic": true,
        "do_not_contact": true,
        "persona": persona,
        "requested_persona": requested_persona,
        "segment": target.map_or("agent-assisted GTM", |_| "target-segment"),
        "trigger": focus,
        "background": observed_context,
        "source_signal": source_signal,
        "observed_context": observed_context,
        "fit_reason": fit_reason,
        "known_gaps": known_gaps,
        "safe_personalization": safe_personalization,
        "signals": [
            {
                "id": target.map_or("synthetic-mdp-example", |_| "synthetic-target-example"),
                "title": focus,
                "source": source_signal,
                "confidence": "fixture-only",
                "freshness": "generated",
                "state_as": "Synthetic fixture only; not evidence about a real prospect."
            }
        ],
        "testing_workflow": [
            "Run route for the same persona/job.",
            "Run fit and brief before drafting.",
            "Draft from safe_personalization while naming known_gaps as assumptions.",
            "Run check-claims on the draft."
        ]
    })
}

fn pick<'a>(values: &'a [&str], offset: usize) -> &'a str {
    values[offset % values.len()]
}

fn title_for_persona(persona: &str, offset: usize) -> String {
    const GTM_TITLES: &[&str] = &[
        "GTM Engineering Lead",
        "Revenue Operations Manager",
        "GTM Systems Director",
    ];
    const PMM_TITLES: &[&str] = &[
        "Product Marketing Lead",
        "Director of Demand Generation",
        "Growth Marketing Manager",
    ];
    const PM_TITLES: &[&str] = &["Product Manager", "Head of Product", "Product Lead"];
    const DEFAULT_TITLES: &[&str] = &["GTM Owner", "Messaging Program Lead", "Operations Lead"];

    let lower = persona.to_lowercase();
    let titles = if lower.contains("gtm") || lower.contains("revops") {
        GTM_TITLES
    } else if lower.contains("pmm")
        || lower.contains("product marketing")
        || lower.contains("demand")
        || lower.contains("growth")
    {
        PMM_TITLES
    } else if lower.contains("pm") || lower.contains("product") {
        PM_TITLES
    } else {
        DEFAULT_TITLES
    };
    titles[offset % titles.len()].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::{TargetInitOptions, init_pack, init_pack_targeted};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_pack(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-sample-leads-{name}-{nonce}"));
        init_pack(&root, "Sample Lead Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        root
    }

    #[test]
    fn sample_leads_generates_bounded_fake_rows() {
        let root = temp_pack("bounded");

        let result = sample_leads(
            &root,
            "GTM Engineering",
            "initial email outbound copy testing",
            3,
            0,
        )
        .expect("sample leads should generate");
        let rows = result["fixture_leads"]
            .as_array()
            .expect("fixture_leads should be an array");

        assert_eq!(result["contract"], FIXTURE_CONTRACT);
        assert_eq!(rows.len(), 3);
        assert_eq!(result["fixture_notice"]["source_kind"], "synthetic-example");
        assert_eq!(rows[0]["source_kind"], "synthetic-example");
        assert_eq!(rows[0]["synthetic"], true);
        assert_eq!(rows[0]["do_not_contact"], true);
        assert!(rows[0]["safe_personalization"].is_array());
        assert!(rows[0]["known_gaps"].is_array());
        assert!(result["route"]["load_order"].is_array());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sample_leads_resolves_persona_aliases_before_routing() {
        let root = temp_pack("persona-alias");

        let result = sample_leads(
            &root,
            "Growth Engineer",
            "agent brief for enriched row",
            2,
            0,
        )
        .expect("sample leads should generate");

        assert_eq!(result["inputs"]["requested_persona"], "Growth Engineer");
        assert_eq!(result["inputs"]["persona"], "GTM Engineering");
        assert_eq!(result["persona_resolution"]["resolved"], true);
        assert_eq!(result["fixture_leads"][0]["persona"], "GTM Engineering");
        assert_eq!(
            result["fixture_leads"][0]["requested_persona"],
            "Growth Engineer"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sample_leads_rejects_counts_outside_fixture_bounds() {
        let root = temp_pack("count");

        let err = sample_leads(&root, "PMM", "linkedin outbound copy", 6, 0)
            .expect_err("count above 5 should fail");

        assert!(err.to_string().contains("between 2 and 5"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sample_leads_seed_changes_deterministic_variants() {
        let root = temp_pack("seed");

        let first = sample_leads(&root, "PMM", "linkedin outbound copy", 2, 0)
            .expect("first seed should generate");
        let second = sample_leads(&root, "PMM", "linkedin outbound copy", 2, 1)
            .expect("second seed should generate");

        assert_ne!(
            first["fixture_leads"][0]["name"],
            second["fixture_leads"][0]["name"]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sample_leads_uses_resolved_target_without_generic_positioning_residue() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-sample-targeted-{nonce}"));
        init_pack_targeted(
            &root,
            "Company B Messaging",
            "gtm",
            &TargetInitOptions {
                custom_name: true,
                name: Some("Company B"),
                excluded_terms: &["Company A".to_string()],
                ..TargetInitOptions::default()
            },
            true,
            false,
        )
        .expect("targeted pack should initialize");

        let result = sample_leads(&root, "Operator", "outbound copy test", 2, 0)
            .expect("target-aware sample leads should generate");
        let first = &result["fixture_leads"][0];
        assert_eq!(first["segment"], "target-segment");
        assert!(first["trigger"].as_str().unwrap().contains("Company B"));
        assert!(
            first["fit_reason"]
                .as_str()
                .unwrap()
                .contains("does not establish real-account fit")
        );
        assert!(!first["source_signal"].as_str().unwrap().contains("MDP"));
        assert!(
            result["workflow"]
                .as_array()
                .unwrap()
                .iter()
                .all(|step| !step.as_str().unwrap().contains("mdp "))
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
