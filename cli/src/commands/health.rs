use crate::constants::{DEFAULT_DIR, FORMAT_NAME, FORMAT_VERSION};
use crate::models::CardKind;
use crate::pack_io::{read_card, read_card_by_id, read_manifest};
use crate::routing::select_cards;
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) fn doctor(root: &Path) -> Value {
    let pack_dir = root.join(DEFAULT_DIR);
    let manifest_path = pack_dir.join("manifest.yaml");
    let mut issues = Vec::new();
    let mut checks = BTreeMap::new();
    checks.insert("auth_required", json!(false));
    checks.insert("offline_mode", json!(true));
    checks.insert("pack_dir_exists", json!(pack_dir.exists()));
    checks.insert("manifest_exists", json!(manifest_path.exists()));
    if !pack_dir.exists() {
        issues.push(format!("missing {}", pack_dir.display()));
    }
    if !manifest_path.exists() {
        issues.push(format!("missing {}", manifest_path.display()));
    }
    if manifest_path.exists() {
        match read_manifest(root) {
            Ok(manifest) => {
                checks.insert("format", json!(manifest.format));
                checks.insert("manifest_parseable", json!(true));
            }
            Err(err) => {
                checks.insert("manifest_parseable", json!(false));
                issues.push(err.to_string());
            }
        }
    }
    json!({
        "tool": "mdp",
        "format_name": FORMAT_NAME,
        "expected_format": FORMAT_VERSION,
        "valid": issues.is_empty(),
        "checks": checks,
        "issues": issues,
        "setup": if issues.is_empty() { Value::Null } else { json!("Run `mdp init --name <name>` from the repo or workspace root.") }
    })
}

pub(crate) fn validate_pack(root: &Path) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let mut issues = Vec::new();
    let mut card_ids = BTreeSet::new();
    let mut loaded_cards = Vec::new();
    if manifest.format != FORMAT_VERSION {
        issues.push(format!(
            "manifest format must be {FORMAT_VERSION}, found {}",
            manifest.format
        ));
    }
    if manifest.personas.is_empty() {
        issues.push("manifest personas must not be empty".to_string());
    }
    if manifest.cards.is_empty() {
        issues.push("manifest cards must not be empty".to_string());
    }
    if !manifest.policy.progressive_disclosure {
        issues.push("policy.progressive_disclosure should be true".to_string());
    }
    for card_ref in &manifest.cards {
        if !card_ids.insert(card_ref.id.clone()) {
            issues.push(format!("duplicate card id {}", card_ref.id));
        }
        let path = root.join(DEFAULT_DIR).join(&card_ref.path);
        match read_card(&path) {
            Ok(card) => {
                if card.id != card_ref.id {
                    issues.push(format!(
                        "{} id mismatch: manifest has {}, card has {}",
                        path.display(),
                        card_ref.id,
                        card.id
                    ));
                }
                if card.kind != card_ref.kind {
                    issues.push(format!("{} kind mismatch", path.display()));
                }
                if card.entries.is_empty() {
                    issues.push(format!("{} has no entries", path.display()));
                }
                loaded_cards.push(json!({"id": card.id, "kind": card_ref.kind, "path": path.display().to_string(), "entries": card.entries.len()}));
            }
            Err(err) => issues.push(err.to_string()),
        }
    }
    Ok(
        json!({"valid": issues.is_empty(), "manifest": root.join(DEFAULT_DIR).join("manifest.yaml").display().to_string(), "cards": loaded_cards, "issues": issues}),
    )
}

pub(crate) fn explain(root: &Path, persona: Option<&str>) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let selected = select_cards(&manifest, persona, None);
    Ok(json!({
        "format": manifest.format,
        "name": manifest.name,
        "principle": "Load the manifest first, then load only the card paths returned for the persona/job.",
        "persona": persona,
        "cards_to_consider": selected,
        "do_not": [
            "Do not ingest every card unless route says the task needs it.",
            "Do not treat the decision pack as a sequencer, CRM, enrichment tool, or execution agent.",
            "Do not invent missing GTM facts; surface gaps in the brief."
        ]
    }))
}

pub(crate) fn gaps(root: &Path) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let mut durable_gaps = Vec::new();
    let mut evidence_gaps = Vec::new();
    if let Ok(card) = read_card_by_id(root, "gaps") {
        for entry in card.entries {
            durable_gaps.push(json!({"id": entry.id, "title": entry.title, "body": entry.body, "applies_to": entry.applies_to}));
        }
    }
    for card_ref in &manifest.cards {
        let card = read_card(&root.join(DEFAULT_DIR).join(&card_ref.path))?;
        for entry in &card.entries {
            if entry.evidence.is_empty()
                && !matches!(
                    card.kind,
                    CardKind::AvoidRules | CardKind::Gaps | CardKind::Ctas
                )
            {
                evidence_gaps.push(json!({"card_id": card.id, "entry_id": entry.id, "title": entry.title, "reason": "missing evidence"}));
            }
        }
    }
    let durable_count = durable_gaps.len();
    let evidence_count = evidence_gaps.len();
    Ok(json!({
        "contract": "mdp.gaps.v0",
        "durable_gaps": durable_gaps,
        "evidence_gaps": evidence_gaps,
        "summary": {"durable": durable_count, "evidence": evidence_count}
    }))
}
