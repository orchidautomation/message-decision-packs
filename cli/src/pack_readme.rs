use crate::models::{Card, CardKind, Manifest};
use serde_json::Value;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeSet;

pub(crate) const README_INVENTORY_CONTRACT: &str = "mdp.readme-inventory.v1";
pub(crate) const README_INVENTORY_BEGIN: &str = "<!-- mdp:readme-inventory v1 begin -->";
pub(crate) const README_INVENTORY_END: &str = "<!-- mdp:readme-inventory v1 end -->";

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
    line(
        &mut out,
        "The inventory block below is a machine-generated projection of loaded structured authority; refresh it with `mdp readme refresh` and never hand-maintain its counts.",
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

    section(&mut out, "Actors");
    for persona in &manifest.personas {
        bullet(&mut out, persona);
    }

    section(&mut out, "ICP and Fit Authority");
    let mut fit_rule_ids = BTreeSet::new();
    for card in cards.iter().filter(|card| card.kind == CardKind::FitRules) {
        if fit_rule_ids.insert(&card.id) {
            bullet(
                &mut out,
                &format!("`cards/{}.yaml`: {}", card.id, card.title),
            );
        }
    }
    if fit_rule_ids.is_empty() {
        bullet(
            &mut out,
            "No `fit-rules` card is declared; use the profile's structured decision criteria and explicit gaps.",
        );
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
        "Use detached prospect input only when the selected job has no direct or transitive Decision Input Contract; governed jobs require the exact normalized envelope and lineage artifacts.",
        "Treat raw prompt output as untrusted. Only a successful validation receipt bound to the exact pack, prompt, job when applicable, validator inputs, and output bytes may provide prompt-output decision-trace authority.",
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
    bullet(&mut out, "`mdp --json readme check --dir .`");
    bullet(&mut out, "`mdp --json readme refresh --dir .`");
    bullet(&mut out, "`mdp --json schema prompt-output-validation-v1`");

    section(&mut out, "Gaps");
    for card in cards.iter().filter(|card| card.kind == CardKind::Gaps) {
        for entry in &card.entries {
            bullet(&mut out, &format!("{}: {}", entry.title, entry.body));
        }
    }

    out.push('\n');
    out.push_str(&render_inventory_block(
        manifest,
        cards,
        source_ledger,
        prompt_ids,
    ));
    out
}

/// Render the deterministic, marker-delimited inventory block that is the only
/// machine-owned region of the README. Refresh replaces exactly this region and
/// preserves arbitrary human orientation outside it.
pub(crate) fn render_inventory_block(
    _manifest: &Manifest,
    cards: &[&Card],
    source_ledger: &Value,
    prompt_ids: &[String],
) -> String {
    let mut out = String::new();
    line(&mut out, README_INVENTORY_BEGIN);
    line(&mut out, "");
    line(&mut out, "## Inventory");
    line(&mut out, "");
    line(
        &mut out,
        "Machine-generated from loaded structured authority. Do not edit by hand; run `mdp readme refresh` to update. This block is a projection of the manifest, cards, sources, and prompts; it cannot satisfy a product-foundation facet, close a gap, or override structured authority.",
    );
    line(&mut out, "");

    let mut sorted_cards = cards.iter().collect::<Vec<_>>();
    sorted_cards.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| card_kind_token(&left.kind).cmp(&card_kind_token(&right.kind)))
    });
    let total_entries = sorted_cards
        .iter()
        .map(|card| card.entries.len())
        .sum::<usize>();
    let source_ids = source_ids(source_ledger);
    let mut sorted_prompts = prompt_ids.to_vec();
    sorted_prompts.sort();
    sorted_prompts.dedup();

    line(&mut out, &format!("- cards: {}", sorted_cards.len()));
    line(&mut out, &format!("- card entries: {total_entries}"));
    line(&mut out, &format!("- prompts: {}", sorted_prompts.len()));
    line(&mut out, &format!("- sources: {}", source_ids.len()));

    line(&mut out, "");
    line(&mut out, "### Card entries");
    line(&mut out, "");
    if sorted_cards.is_empty() {
        bullet(&mut out, "No cards declared.");
    }
    for card in &sorted_cards {
        bullet(
            &mut out,
            &format!(
                "`{}` ({}): {} entries",
                card.id,
                card_kind_token(&card.kind),
                card.entries.len()
            ),
        );
    }

    line(&mut out, "");
    line(&mut out, "### Sources");
    line(&mut out, "");
    if source_ids.is_empty() {
        bullet(&mut out, "No sources declared.");
    }
    for id in &source_ids {
        bullet(&mut out, &format!("`{id}`"));
    }

    line(&mut out, "");
    line(&mut out, "### Prompts");
    line(&mut out, "");
    if sorted_prompts.is_empty() {
        bullet(&mut out, "No prompts declared.");
    }
    for id in &sorted_prompts {
        bullet(&mut out, &format!("`{id}`"));
    }

    line(&mut out, README_INVENTORY_END);
    out
}

/// Extract the owned inventory block, including its begin and end markers and
/// the single line terminator after the end marker. Returns `None` when the
/// README is legacy orientation-only prose without the owned marker, leaving it
/// unassessed and unable to affect pack readiness. The captured string equals
/// the freshly rendered block byte-for-byte for unchanged authority.
pub(crate) fn extract_inventory_block(readme: &str) -> Option<String> {
    let begin = readme.find(README_INVENTORY_BEGIN)?;
    let end = block_end_offset(readme, begin)?;
    Some(readme[begin..end].to_string())
}

/// Replace the owned inventory block in `readme` with `fresh_block`. When no
/// owned block is present, append the block at the end as an explicit migration
/// from legacy orientation-only prose.
pub(crate) fn replace_inventory_block(readme: &str, fresh_block: &str) -> String {
    match extract_inventory_block_offsets(readme) {
        Some((begin, end)) => {
            let mut out = String::with_capacity(readme.len() + fresh_block.len());
            out.push_str(&readme[..begin]);
            out.push_str(fresh_block);
            out.push_str(&readme[end..]);
            out
        }
        None => {
            let mut out = readme.trim_end_matches('\n').to_string();
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(fresh_block);
            out
        }
    }
}

fn extract_inventory_block_offsets(readme: &str) -> Option<(usize, usize)> {
    let begin = readme.find(README_INVENTORY_BEGIN)?;
    let end = block_end_offset(readme, begin)?;
    Some((begin, end))
}

/// Return the offset just past the end of the owned block beginning at `begin`,
/// consuming the single line terminator after the end marker so the captured
/// string equals the freshly rendered block byte-for-byte.
fn block_end_offset(readme: &str, begin: usize) -> Option<usize> {
    let end_marker_start = begin + readme[begin..].find(README_INVENTORY_END)?;
    let after_marker = end_marker_start + README_INVENTORY_END.len();
    let bytes = readme.as_bytes();
    if after_marker < bytes.len() && bytes[after_marker] == b'\n' {
        Some(after_marker + 1)
    } else {
        Some(after_marker)
    }
}

fn source_ids(source_ledger: &Value) -> Vec<String> {
    let yaml_value: YamlValue = serde_yaml::to_value(source_ledger).unwrap_or(YamlValue::Null);
    yaml_value["sources"]
        .as_sequence()
        .into_iter()
        .flatten()
        .filter_map(|source| source["id"].as_str().map(str::to_string))
        .collect()
}

fn card_kind_token(kind: &CardKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{kind:?}"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CardKind, Entry, Manifest};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn manifest_with(name: &str) -> Manifest {
        Manifest {
            format: "mdp.v0".into(),
            id: "pack".into(),
            name: name.into(),
            version: "0.1.0".into(),
            description: Some("A pack".into()),
            target: None,
            profile: None,
            personas: vec!["PMM".into()],
            target_personas: vec![],
            operator_roles: vec![],
            supported_channels: vec![],
            persona_mappings: vec![],
            lead_input_requirements: Default::default(),
            qualification_gates: None,
            required_primitives: vec![],
            primitive_map: BTreeMap::new(),
            decision_input_contracts: vec![],
            input_contracts: vec![],
            jobs: vec![],
            profile_eval: Default::default(),
            cards: vec![],
            policy: crate::models::Policy {
                progressive_disclosure: true,
                load_manifest_first: true,
                max_cards_per_route: 4,
                json_contract: "mdp.json.v0".into(),
                no_auth_required: true,
            },
            provenance: crate::models::Provenance {
                owner: "test".into(),
                created_by: "test".into(),
                notes: vec![],
            },
        }
    }

    fn card(id: &str, kind: CardKind, entries: usize) -> Card {
        Card {
            id: id.into(),
            kind,
            title: format!("{id} title"),
            description: "desc".into(),
            personas: vec![],
            tags: vec![],
            entries: (0..entries)
                .map(|index| Entry {
                    id: format!("{id}-{index}"),
                    title: format!("entry {index}"),
                    body: "body".into(),
                    applies_to: vec![],
                    scope: BTreeMap::new(),
                    evidence: vec![],
                    avoid: vec![],
                    exact_paragraphs: None,
                    constraints: Default::default(),
                    metadata: BTreeMap::new(),
                })
                .collect(),
        }
    }

    fn source_ledger(ids: &[&str]) -> Value {
        json!({
            "sources": ids.iter().map(|id| json!({"id": id, "locator": "synthetic"})).collect::<Vec<_>>()
        })
    }

    #[test]
    fn inventory_block_is_deterministic_and_byte_stable_for_unchanged_authority() {
        let manifest = manifest_with("Deterministic Pack");
        let cards = vec![
            card("hooks", CardKind::Hooks, 10),
            card("pains", CardKind::Pains, 6),
            card("positioning", CardKind::Positioning, 3),
        ];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let ledger = source_ledger(&["target-identity", "public-web"]);
        let prompts = vec![
            "generate-outbound-copy".to_string(),
            "review-outbound-copy".to_string(),
        ];

        let first = render_inventory_block(&manifest, &card_refs, &ledger, &prompts);
        let second = render_inventory_block(&manifest, &card_refs, &ledger, &prompts);
        assert_eq!(first, second, "byte-stable for unchanged authority");
        assert!(first.contains(README_INVENTORY_BEGIN));
        assert!(first.contains(README_INVENTORY_END));
        assert!(first.contains("- cards: 3"));
        assert!(first.contains("- card entries: 19"));
        assert!(first.contains("- prompts: 2"));
        assert!(first.contains("- sources: 2"));
        assert!(first.contains("`pains` (pains): 6 entries"));
        assert!(first.contains("`hooks` (hooks): 10 entries"));
        // cards are sorted by id, so pains precedes positioning precedes hooks? No:
        // sorted ids: hooks, pains, positioning. Confirm ordering is stable.
        let pains_pos = first.find("`pains`").unwrap();
        let hooks_pos = first.find("`hooks`").unwrap();
        assert!(
            hooks_pos < pains_pos,
            "cards sorted by id: hooks before pains"
        );
    }

    #[test]
    fn inventory_block_order_is_independent_of_input_card_order() {
        let manifest = manifest_with("Order Independent");
        let cards_a = vec![
            card("zeta", CardKind::Pains, 1),
            card("alpha", CardKind::Hooks, 2),
        ];
        let cards_b = vec![
            card("alpha", CardKind::Hooks, 2),
            card("zeta", CardKind::Pains, 1),
        ];
        let ledger = source_ledger(&[]);
        let prompts: Vec<String> = vec![];
        let refs_a = cards_a.iter().collect::<Vec<_>>();
        let refs_b = cards_b.iter().collect::<Vec<_>>();
        assert_eq!(
            render_inventory_block(&manifest, &refs_a, &ledger, &prompts),
            render_inventory_block(&manifest, &refs_b, &ledger, &prompts),
        );
    }

    #[test]
    fn render_pack_readme_emits_owned_inventory_block() {
        let manifest = manifest_with("Full Readme");
        let cards = vec![
            card("pains", CardKind::Pains, 6),
            card("hooks", CardKind::Hooks, 10),
        ];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let ledger = source_ledger(&["target-identity"]);
        let prompts = vec!["generate-outbound-copy".to_string()];
        let readme = render_pack_readme(&manifest, &card_refs, &ledger, &prompts);
        assert!(readme.contains("## Authority"));
        assert!(readme.contains(README_INVENTORY_BEGIN));
        assert!(readme.contains(README_INVENTORY_END));
        let extracted = extract_inventory_block(&readme).expect("block should extract");
        assert!(extracted.contains("`pains` (pains): 6 entries"));
        assert!(extracted.contains("`hooks` (hooks): 10 entries"));
    }

    #[test]
    fn extract_returns_none_for_legacy_readme_without_marker() {
        let legacy = "# Human Pack\n\nOrientation prose only.\n";
        assert!(extract_inventory_block(legacy).is_none());
    }

    #[test]
    fn replace_preserves_human_prose_outside_owned_block() {
        let mut human = String::from("# Human Pack\n\n");
        human.push_str("Free-form orientation the author wrote by hand.\n\n");
        human.push_str("<!-- mdp:readme-inventory v1 begin -->\n## Inventory\n\n- stale: yes\n\n<!-- mdp:readme-inventory v1 end -->\n");
        human.push_str("\nMore human notes after the block.\n");

        let manifest = manifest_with("Replace Pack");
        let cards = vec![card("pains", CardKind::Pains, 6)];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let fresh = render_inventory_block(&manifest, &card_refs, &source_ledger(&[]), &[]);
        let updated = replace_inventory_block(&human, &fresh);

        assert!(updated.contains("Free-form orientation the author wrote by hand."));
        assert!(updated.contains("More human notes after the block."));
        assert!(!updated.contains("stale: yes"));
        assert!(updated.contains("`pains` (pains): 6 entries"));
        // The owned block appears exactly once after replacement.
        assert_eq!(
            updated.matches(README_INVENTORY_BEGIN).count(),
            1,
            "exactly one owned block"
        );
    }

    #[test]
    fn replace_appends_block_when_absent_to_migrate_legacy_readme() {
        let human = "# Legacy Pack\n\nOrientation only.\n";
        let manifest = manifest_with("Migrate Pack");
        let cards = vec![card("pains", CardKind::Pains, 6)];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let fresh = render_inventory_block(&manifest, &card_refs, &source_ledger(&[]), &[]);
        let updated = replace_inventory_block(&human, &fresh);
        assert!(updated.starts_with("# Legacy Pack"));
        assert!(updated.contains(README_INVENTORY_BEGIN));
        assert_eq!(updated.matches(README_INVENTORY_BEGIN).count(), 1);
    }

    #[test]
    fn refreshing_twice_is_byte_stable_when_authority_unchanged() {
        let manifest = manifest_with("Stable Refresh");
        let cards = vec![
            card("pains", CardKind::Pains, 6),
            card("hooks", CardKind::Hooks, 10),
        ];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let ledger = source_ledger(&["target-identity"]);
        let prompts = vec!["generate-outbound-copy".to_string()];
        let fresh = render_inventory_block(&manifest, &card_refs, &ledger, &prompts);
        let mut readme = "# Stable Pack\n\nOrientation.\n".to_string();
        readme = replace_inventory_block(&readme, &fresh);
        let once = readme.clone();
        readme = replace_inventory_block(&readme, &fresh);
        assert_eq!(once, readme, "second refresh is byte-stable");
    }

    #[test]
    fn inventory_drift_regression_fixture_catches_mismatched_counts() {
        // Reproduces the audited mismatch class: a README claims pains 14 and
        // hooks 21, while the loaded structured authority reports 6 and 10. The
        // fresh block must disagree and the stale authored block must fail to
        // equal the projection.
        let manifest = manifest_with("Mismatched Pack");
        let cards = vec![
            card("pains", CardKind::Pains, 6),
            card("hooks", CardKind::Hooks, 10),
        ];
        let card_refs = cards.iter().collect::<Vec<_>>();
        let fresh = render_inventory_block(&manifest, &card_refs, &source_ledger(&[]), &[]);
        let mut stale = String::new();
        line(&mut stale, README_INVENTORY_BEGIN);
        line(&mut stale, "");
        line(&mut stale, "## Inventory");
        line(&mut stale, "");
        bullet(&mut stale, "- pains: 14 entries");
        bullet(&mut stale, "- hooks: 21 entries");
        line(&mut stale, "");
        line(&mut stale, README_INVENTORY_END);
        assert_ne!(
            stale, fresh,
            "stale authored counts must not match the projection"
        );
        assert!(fresh.contains("`pains` (pains): 6 entries"));
        assert!(fresh.contains("`hooks` (hooks): 10 entries"));
    }
}
