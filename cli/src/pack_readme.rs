use crate::models::{Card, CardKind, Manifest};
use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) fn render_pack_readme(
    manifest: &Manifest,
    cards: &[&Card],
    source_ledger: &Value,
    prompt_ids: &[String],
) -> String {
    let mut out = String::new();
    line(&mut out, &format!("# {}", manifest.name));
    line(&mut out, "");
    section(&mut out, "Authority");
    line(
        &mut out,
        "This README is orientation only. The manifest, referenced card entries, source ledger, contracts, and explicit gaps remain the machine authority. README prose cannot satisfy readiness or override structured authority.",
    );

    section(&mut out, "Thesis");
    line(
        &mut out,
        manifest
            .description
            .as_deref()
            .unwrap_or("A local Message Decision Pack."),
    );
    if manifest
        .profile
        .as_ref()
        .is_some_and(|profile| profile.id == "proposal")
    {
        line(
            &mut out,
            "This public sample is synthetic review support. It does not certify compliance, approve regulated-data handling, replace legal or procurement review, or authorize proposal submission.",
        );
    }

    section(&mut out, "Actors and ICP");
    for persona in &manifest.personas {
        bullet(&mut out, persona);
    }

    section(&mut out, "Supported Jobs");
    for job in &manifest.jobs {
        bullet(
            &mut out,
            &format!(
                "`{}`: {}",
                job.id,
                job.label.as_deref().unwrap_or("Canonical pack job")
            ),
        );
    }

    section(&mut out, "Decision Flow");
    for step in [
        "Select one exact canonical job.",
        "Inspect its resolved product foundation and diagnostics.",
        "Load only the referenced cards, entries, contracts, sources, and gaps.",
        "Stop on blocked authority; never fill a gap from this README.",
        "Apply the job output and review boundaries before using the result.",
    ] {
        bullet(&mut out, step);
    }

    section(&mut out, "Boundaries");
    let proposal = manifest
        .profile
        .as_ref()
        .is_some_and(|profile| profile.id == "proposal");
    let mut boundary_ids = BTreeSet::new();
    for card in cards.iter().filter(|card| {
        matches!(
            card.kind,
            CardKind::AvoidRules | CardKind::ChannelPolicies | CardKind::OutputRules
        ) || (proposal
            && matches!(
                card.id.as_str(),
                "proposal-boundaries" | "compliance-boundaries"
            ))
    }) {
        if boundary_ids.insert(&card.id) {
            bullet(
                &mut out,
                &format!("`cards/{}.yaml`: {}", card.id, card.title),
            );
        }
    }

    section(&mut out, "Sources");
    if let Some(sources) = source_ledger["sources"].as_array() {
        for source in sources {
            if let Some(id) = source["id"].as_str() {
                let locator = source["locator"].as_str().unwrap_or("locator not recorded");
                bullet(&mut out, &format!("`{id}`: {locator}"));
            }
        }
    }

    section(&mut out, "Prompts");
    for prompt_id in prompt_ids {
        bullet(&mut out, &format!("`{prompt_id}`"));
    }

    section(&mut out, "Commands");
    bullet(&mut out, "`mdp --json validate --dir .`");
    for job in &manifest.jobs {
        bullet(
            &mut out,
            &format!("`mdp --json skills --job {} --dir .`", job.id),
        );
        bullet(
            &mut out,
            &format!("`mdp --json requirements --job {} --dir .`", job.id),
        );
    }

    section(&mut out, "Gaps");
    for card in cards.iter().filter(|card| card.kind == CardKind::Gaps) {
        for entry in &card.entries {
            bullet(&mut out, &format!("{}: {}", entry.title, entry.body));
        }
    }
    out
}

fn section(out: &mut String, title: &str) {
    line(out, "");
    line(out, &format!("## {title}"));
    line(out, "");
}

fn bullet(out: &mut String, value: &str) {
    line(out, &format!("- {value}"));
}

fn line(out: &mut String, value: &str) {
    out.push_str(value);
    out.push('\n');
}
