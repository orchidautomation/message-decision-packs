use crate::artifact_hash::sha256_hex;
use crate::commands::health::issue;
use crate::constants::DEFAULT_DIR;
use crate::models::{Card, Manifest};
use crate::pack_io::{read_card, read_manifest, resolve_pack_path};
use crate::pack_readme::{
    README_INVENTORY_CONTRACT, extract_inventory_block, extract_ownership_block,
    human_owned_readme, render_inventory_block, render_ownership_block, replace_readme_regions,
    validate_readme_regions,
};
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use serde_yaml::Value as YamlValue;
use std::fs;
use std::path::Path;

/// Compare the owned README inventory block against freshly loaded structured
/// authority. A legacy README without the owned marker is `unassessed` and never
/// blocks pack readiness; a stale owned block is `stale` and blocks.
pub(crate) fn check_readme(root: &Path) -> Result<Value> {
    let readme_path = root.join(DEFAULT_DIR).join("README.md");
    let existing = fs::read_to_string(&readme_path).unwrap_or_default();
    validate_readme_regions(&existing).map_err(|message| anyhow!(message))?;
    let (manifest, cards, source_ledger, prompt_ids) = load_readme_authority(root)?;
    let warnings = human_reference_warnings(&existing, &manifest, &source_ledger, &readme_path);
    let card_refs = cards.iter().collect::<Vec<_>>();
    let fresh_block = render_inventory_block(&manifest, &card_refs, &source_ledger, &prompt_ids);
    let fresh_ownership = render_ownership_block();
    let existing_ownership = extract_ownership_block(&existing);
    let existing_inventory = extract_inventory_block(&existing);
    let changed_generated_regions = changed_generated_regions(&existing, Some(&fresh_block));
    if existing_ownership.is_none() && existing_inventory.is_none() {
        return Ok(json!({
            "contract": README_INVENTORY_CONTRACT,
            "status": "unassessed",
            "valid": true,
            "drift": false,
            "has_owned_block": false,
            "has_ownership_region": false,
            "has_inventory_region": false,
            "path": readme_path.display().to_string(),
            "generated_regions": generated_region_report(&changed_generated_regions),
            "changed_generated_regions": changed_generated_regions,
            "untouched_human_regions": untouched_human_regions(),
            "semantic_prose_review": "not-performed",
            "warnings": warnings
        }));
    }

    if changed_generated_regions.is_empty() {
        Ok(json!({
            "contract": README_INVENTORY_CONTRACT,
            "status": "fresh",
            "valid": true,
            "drift": false,
            "has_owned_block": true,
            "has_ownership_region": true,
            "has_inventory_region": true,
            "path": readme_path.display().to_string(),
            "inventory_sha256": sha256_hex(fresh_block.as_bytes()),
            "generated_regions": generated_region_report(&changed_generated_regions),
            "changed_generated_regions": changed_generated_regions,
            "untouched_human_regions": untouched_human_regions(),
            "semantic_prose_review": "not-performed",
            "warnings": warnings
        }))
    } else {
        Ok(json!({
            "contract": README_INVENTORY_CONTRACT,
            "status": "stale",
            "valid": false,
            "drift": true,
            "has_owned_block": existing_inventory.is_some(),
            "has_ownership_region": existing_ownership.is_some(),
            "has_inventory_region": existing_inventory.is_some(),
            "path": readme_path.display().to_string(),
            "expected_sha256": sha256_hex(fresh_block.as_bytes()),
            "actual_sha256": existing_inventory.as_deref().map(|block| sha256_hex(block.as_bytes())),
            "generated_region_sha256": {
                "ownership": region_hash_evidence(&fresh_ownership, existing_ownership.as_deref()),
                "inventory": region_hash_evidence(&fresh_block, existing_inventory.as_deref())
            },
            "diagnostics": [stale_drift_issue(&readme_path, "error")],
            "generated_regions": generated_region_report(&changed_generated_regions),
            "changed_generated_regions": changed_generated_regions,
            "untouched_human_regions": untouched_human_regions(),
            "semantic_prose_review": "not-performed",
            "warnings": warnings
        }))
    }
}

fn region_hash_evidence(expected: &str, actual: Option<&str>) -> Value {
    json!({
        "expected": sha256_hex(expected.as_bytes()),
        "actual": actual.map(|region| sha256_hex(region.as_bytes()))
    })
}

/// Regenerate only the owned README inventory block, preserving arbitrary human
/// orientation prose outside the marker-delimited region. When the README has no
/// owned block yet, the block is appended as an explicit legacy migration.
pub(crate) fn refresh_readme(root: &Path, out: Option<&Path>, dry_run: bool) -> Result<Value> {
    let (manifest, cards, source_ledger, prompt_ids) = load_readme_authority(root)?;
    let card_refs = cards.iter().collect::<Vec<_>>();
    let fresh_block = render_inventory_block(&manifest, &card_refs, &source_ledger, &prompt_ids);
    let readme_path = root.join(DEFAULT_DIR).join("README.md");
    let existing = fs::read_to_string(&readme_path).unwrap_or_default();
    let updated =
        replace_readme_regions(&existing, &fresh_block).map_err(|message| anyhow!(message))?;
    let had_owned_block = extract_inventory_block(&existing).is_some();
    let changed_generated_regions = changed_generated_regions(&existing, Some(&fresh_block));
    let warnings = human_reference_warnings(&existing, &manifest, &source_ledger, &readme_path);
    let bytes = updated.len() as u64;

    let (status, target_path) = if dry_run {
        (
            "dry-run",
            out.map(|p| p.to_path_buf())
                .unwrap_or_else(|| readme_path.clone()),
        )
    } else {
        let write_target = out.unwrap_or(&readme_path);
        if let Some(parent) = write_target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::write(write_target, &updated)
            .with_context(|| format!("writing {}", write_target.display()))?;
        ("saved", write_target.to_path_buf())
    };

    let mut data = json!({
        "contract": README_INVENTORY_CONTRACT,
        "status": status,
        "path": readme_path.display().to_string(),
        "write_path": target_path.display().to_string(),
        "had_owned_block": had_owned_block,
        "inventory_sha256": sha256_hex(fresh_block.as_bytes()),
        "bytes": bytes,
        "block": fresh_block,
        "generated_regions": generated_region_report(&changed_generated_regions),
        "changed_generated_regions": changed_generated_regions,
        "untouched_human_regions": untouched_human_regions(),
        "semantic_prose_review": "not-performed",
        "warnings": warnings
    });
    if dry_run {
        if let Some(object) = data.as_object_mut() {
            object.insert("dry_run".to_string(), json!(true));
        }
    }
    Ok(data)
}

/// Validate-pack integration hook. Malformed generated-region markers are a
/// stable blocking validation error. Inventory drift remains warning-first;
/// missing README, legacy prose without markers, unrelated authority-read
/// failures, and a fresh block add no README-owned blocker.
pub(crate) fn readme_validation_issues(root: &Path) -> Vec<Value> {
    let readme_path = root.join(DEFAULT_DIR).join("README.md");
    if let Ok(existing) = fs::read_to_string(&readme_path)
        && let Err(message) = validate_readme_regions(&existing)
    {
        return vec![issue(
            "readme_marker_layout_invalid",
            "error",
            readme_path.display().to_string(),
            message,
        )];
    }
    let Some(result) = check_readme(root).ok() else {
        return vec![];
    };
    let mut issues = result["warnings"].as_array().cloned().unwrap_or_default();
    if result["status"] == "stale" {
        // Validate integration surfaces drift as a warning so legacy and
        // card-mutating flows remain compatible; strict validation promotes
        // it to a blocker. The standalone `readme check` command keeps its own
        // error-level diagnostic.
        issues.push(stale_drift_issue(
            &std::path::PathBuf::from(result["path"].as_str().unwrap_or(".mdp/README.md")),
            "warning",
        ));
    }
    issues
}

fn changed_generated_regions(existing: &str, fresh_inventory: Option<&str>) -> Vec<&'static str> {
    let mut changed = Vec::new();
    let fresh_ownership = render_ownership_block();
    if extract_ownership_block(existing).as_deref() != Some(fresh_ownership.as_str()) {
        changed.push("ownership");
    }
    if fresh_inventory
        .is_some_and(|fresh| extract_inventory_block(existing).as_deref() != Some(fresh))
    {
        changed.push("inventory");
    }
    changed
}

fn generated_region_report(changed: &[&str]) -> Value {
    json!([
        {"id": "ownership", "ownership": "machine", "changed": changed.contains(&"ownership")},
        {"id": "inventory", "ownership": "machine", "changed": changed.contains(&"inventory")}
    ])
}

fn untouched_human_regions() -> Value {
    json!([{
        "id": "readme-prose",
        "ownership": "human",
        "changed": false,
        "reviewed": false,
        "includes": ["thesis", "source interpretation", "gaps", "other prose"]
    }])
}

fn human_reference_warnings(
    readme: &str,
    manifest: &Manifest,
    source_ledger: &Value,
    readme_path: &Path,
) -> Vec<Value> {
    let human = human_owned_readme(readme);
    let card_paths = manifest
        .cards
        .iter()
        .map(|card| card.path.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let source_ids = source_ledger["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|source| source["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut warnings = Vec::new();
    let mut seen_cards = std::collections::BTreeSet::new();
    for token in inline_code_tokens(&human) {
        if token.starts_with("cards/")
            && matches!(token.rsplit_once('.'), Some((_, "yaml" | "yml")))
            && !card_paths.contains(token.as_str())
            && seen_cards.insert(token.clone())
        {
            warnings.push(reference_warning(
                "readme_human_card_reference_missing",
                "card",
                &token,
                readme_path,
            ));
        }
    }
    let mut seen_sources = std::collections::BTreeSet::new();
    for token in source_reference_ids(&human) {
        if !source_ids.contains(token.as_str()) && seen_sources.insert(token.clone()) {
            warnings.push(reference_warning(
                "readme_human_source_reference_missing",
                "source",
                &token,
                readme_path,
            ));
        }
    }
    warnings
}

fn reference_warning(code: &str, kind: &str, reference: &str, path: &Path) -> Value {
    let mut warning = issue(
        code,
        "warning",
        path.display().to_string(),
        format!(
            "Human-owned README prose references removed {kind} `{reference}`; refresh did not rewrite or semantically reconcile that prose."
        ),
    );
    warning["authority"] = json!("non-authoritative-mechanical-warning");
    warning["reference_kind"] = json!(kind);
    warning["reference"] = json!(reference);
    warning
}

fn inline_code_tokens(markdown: &str) -> Vec<String> {
    markdown
        .split('`')
        .enumerate()
        .filter_map(|(index, token)| (index % 2 == 1).then(|| token.trim().to_string()))
        .filter(|token| !token.is_empty())
        .collect()
}

fn source_reference_ids(markdown: &str) -> Vec<String> {
    let mut in_sources = false;
    let mut fence: Option<&str> = None;
    let mut ids = Vec::new();
    for raw_line in markdown.lines() {
        let line = raw_line.trim_end_matches('\r');
        let trimmed = line.trim_start();
        if let Some(active) = fence {
            if trimmed.starts_with(active) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some("```");
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some("~~~");
            continue;
        }
        if line.starts_with("## ") {
            in_sources = line == "## Sources";
            continue;
        }
        if !in_sources {
            continue;
        }
        let Some(rest) = line.strip_prefix("- `") else {
            continue;
        };
        let Some((id, _)) = rest.split_once("`:") else {
            continue;
        };
        if !id.is_empty() {
            ids.push(id.to_string());
        }
    }
    ids
}

fn stale_drift_issue(readme_path: &Path, severity: &str) -> Value {
    issue(
        "readme_inventory_drift",
        severity,
        readme_path.display().to_string(),
        "A machine-owned README region does not match the canonical ownership legend or loaded structured authority; run `mdp readme refresh --dir .` to regenerate both owned regions, or remove unverifiable manual counts.",
    )
}

fn load_readme_authority(root: &Path) -> Result<(Manifest, Vec<Card>, Value, Vec<String>)> {
    let manifest = read_manifest(root)?;
    let mut cards = Vec::new();
    for card_ref in &manifest.cards {
        let path = resolve_pack_path(root, &card_ref.path)
            .with_context(|| format!("resolving card {}", card_ref.id))?;
        let card = read_card(&path)
            .with_context(|| format!("reading card {} for README inventory", card_ref.id))?;
        cards.push(card);
    }
    let source_ledger = load_source_ledger(root)?;
    let prompt_ids = load_prompt_ids(root)?;
    Ok((manifest, cards, source_ledger, prompt_ids))
}

fn load_source_ledger(root: &Path) -> Result<Value> {
    let path = root.join(DEFAULT_DIR).join("sources.yaml");
    if !path.exists() {
        return Ok(Value::Null);
    }
    let raw = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let yaml: YamlValue =
        serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    Ok(serde_json::to_value(&yaml).unwrap_or(Value::Null))
}

fn load_prompt_ids(root: &Path) -> Result<Vec<String>> {
    let prompts_dir = root.join(DEFAULT_DIR).join("prompts");
    if !prompts_dir.exists() {
        return Ok(vec![]);
    }
    let mut paths = fs::read_dir(&prompts_dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    paths.sort();
    let mut ids = Vec::new();
    for path in paths {
        let extension = path.extension().and_then(|extension| extension.to_str());
        if !matches!(extension, Some("yaml" | "yml")) {
            continue;
        }
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(_) => continue,
        };
        let Ok(yaml) = serde_yaml::from_str::<YamlValue>(&raw) else {
            continue;
        };
        if let Some(id) = yaml["id"].as_str() {
            ids.push(id.to_string());
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use crate::models::{CardKind, Entry};
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn nonce() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    }

    #[test]
    fn fresh_generated_pack_has_fresh_inventory_block() {
        let root = std::env::temp_dir().join(format!("mdp-readme-fresh-{}", nonce()));
        init_pack(&root, "Fresh Readme Pack", "gtm", true, false).expect("pack should initialize");
        let result = check_readme(&root).expect("check should run");
        assert_eq!(result["status"], "fresh");
        assert_eq!(result["valid"], true);
        assert_eq!(result["drift"], false);
        assert_eq!(result["has_owned_block"], true);
        assert_eq!(result["changed_generated_regions"], json!([]));
        assert_eq!(result["semantic_prose_review"], "not-performed");
        assert!(
            std::fs::read_to_string(root.join(".mdp/README.md"))
                .unwrap()
                .contains(crate::pack_readme::README_OWNERSHIP_BEGIN)
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn modifying_a_card_after_rendering_makes_check_stale_with_drift_diagnostic() {
        let root = std::env::temp_dir().join(format!("mdp-readme-drift-{}", nonce()));
        init_pack(&root, "Drift Readme Pack", "gtm", true, false).expect("pack should initialize");
        // Mutate a card by appending an entry so the README inventory block is
        // now stale relative to loaded structured authority.
        let manifest = read_manifest(&root).expect("manifest");
        let pains_ref = manifest
            .cards
            .iter()
            .find(|card_ref| card_ref.kind == CardKind::Pains)
            .expect("pains card ref");
        let pains_path = resolve_pack_path(&root, &pains_ref.path).expect("path");
        let mut card = read_card(&pains_path).expect("card");
        card.entries.push(Entry {
            id: "extra-pain".into(),
            title: "Extra pain".into(),
            body: "added after rendering".into(),
            applies_to: vec![],
            scope: BTreeMap::new(),
            evidence: vec![],
            avoid: vec![],
            exact_paragraphs: None,
            constraints: Default::default(),
            metadata: BTreeMap::new(),
        });
        let serialized = serde_yaml::to_string(&card).expect("serialize");
        std::fs::write(&pains_path, serialized).expect("write card");

        let result = check_readme(&root).expect("check should run");
        assert_eq!(result["status"], "stale", "{result}");
        assert_eq!(result["valid"], false);
        assert_eq!(result["drift"], true);
        let diag = &result["diagnostics"][0];
        assert_eq!(diag["code"], "readme_inventory_drift");
        assert_eq!(diag["severity"], "error");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn editing_the_ownership_legend_makes_check_and_validate_stale() {
        let root = std::env::temp_dir().join(format!("mdp-readme-ownership-drift-{}", nonce()));
        init_pack(&root, "Ownership Drift Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("readme");
        let edited = readme.replacen(
            "Machine-owned: this ownership legend",
            "Machine-owned: edited ownership legend",
            1,
        );
        assert_ne!(edited, readme, "fixture must edit the owned legend");
        std::fs::write(&readme_path, edited).expect("write edited legend");

        let check = check_readme(&root).expect("check");
        assert_eq!(check["status"], "stale");
        assert_eq!(check["valid"], false);
        assert_eq!(check["changed_generated_regions"], json!(["ownership"]));
        assert_eq!(check["diagnostics"][0]["code"], "readme_inventory_drift");
        assert_ne!(
            check["generated_region_sha256"]["ownership"]["expected"],
            check["generated_region_sha256"]["ownership"]["actual"]
        );
        assert_eq!(
            check["generated_region_sha256"]["inventory"]["expected"],
            check["generated_region_sha256"]["inventory"]["actual"]
        );

        let validation = crate::commands::health::validate_pack(&root).expect("validate");
        assert_eq!(validation["valid"], true, "drift stays warning-first");
        let drift = validation["issues"]
            .as_array()
            .expect("issues")
            .iter()
            .find(|issue| issue["code"] == "readme_inventory_drift")
            .expect("ownership drift warning");
        assert_eq!(drift["severity"], "warning");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn partial_generated_region_presence_is_stale_in_both_directions() {
        let cases = [
            ("ownership", "inventory", json!(["inventory"])),
            ("inventory", "ownership", json!(["ownership"])),
        ];
        for (kept, removed, expected_changed) in cases {
            let root = std::env::temp_dir().join(format!("mdp-readme-partial-{kept}-{}", nonce()));
            init_pack(&root, "Partial Region Pack", "gtm", true, false)
                .expect("pack should initialize");
            let readme_path = root.join(".mdp/README.md");
            let readme = std::fs::read_to_string(&readme_path).expect("readme");
            let removed_block = if removed == "ownership" {
                extract_ownership_block(&readme).expect("ownership block")
            } else {
                extract_inventory_block(&readme).expect("inventory block")
            };
            let partial = readme.replacen(&removed_block, "", 1);
            std::fs::write(&readme_path, partial).expect("write partial readme");

            let check = check_readme(&root).expect("check");
            assert_eq!(check["status"], "stale", "kept {kept}");
            assert_eq!(check["valid"], false, "kept {kept}");
            assert_eq!(check["changed_generated_regions"], expected_changed);
            assert_eq!(check["has_ownership_region"], kept == "ownership");
            assert_eq!(check["has_inventory_region"], kept == "inventory");
            assert!(
                check["generated_region_sha256"][removed]["actual"].is_null(),
                "missing {removed} region must have null actual hash"
            );
            assert!(
                check["generated_region_sha256"][kept]["actual"].is_string(),
                "kept {kept} region must retain checksum evidence"
            );

            let validation = crate::commands::health::validate_pack(&root).expect("validate");
            assert_eq!(validation["valid"], true, "drift stays warning-first");
            assert!(validation["issues"].as_array().is_some_and(|issues| {
                issues.iter().any(|issue| {
                    issue["code"] == "readme_inventory_drift" && issue["severity"] == "warning"
                })
            }));
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn refresh_regenerates_only_owned_block_and_is_byte_stable() {
        let root = std::env::temp_dir().join(format!("mdp-readme-refresh-{}", nonce()));
        init_pack(&root, "Refresh Readme Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let before = std::fs::read_to_string(&readme_path).expect("readme");
        // Add human prose after the owned block and corrupt the block's counts.
        let corrupted = before.replace("- card entries:", "- card entries (hand-edited):");
        std::fs::write(&readme_path, &corrupted).expect("write corrupted readme");
        assert!(check_readme(&root).expect("check")["status"] == "stale");

        let refreshed = refresh_readme(&root, None, false).expect("refresh");
        assert_eq!(refreshed["status"], "saved");
        assert_eq!(refreshed["changed_generated_regions"], json!(["inventory"]));
        assert_eq!(refreshed["untouched_human_regions"][0]["reviewed"], false);
        let after = std::fs::read_to_string(&readme_path).expect("readme after");
        assert!(after.contains("- card entries:"));
        assert!(!after.contains("hand-edited"));

        // Second refresh with unchanged authority is byte-stable.
        let again = refresh_readme(&root, None, false).expect("refresh again");
        let after_again = std::fs::read_to_string(&readme_path).expect("readme again");
        assert_eq!(
            after, after_again,
            "second refresh is byte-stable for unchanged authority"
        );
        assert_eq!(again["inventory_sha256"], refreshed["inventory_sha256"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_dry_run_does_not_write_and_reports_plan() {
        let root = std::env::temp_dir().join(format!("mdp-readme-dry-{}", nonce()));
        init_pack(&root, "Dry Run Readme Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let before = std::fs::read_to_string(&readme_path).expect("readme");
        // Corrupt to confirm dry-run does not repair.
        let corrupted = before.replace("- cards:", "- packs:");
        std::fs::write(&readme_path, &corrupted).expect("write corrupted readme");

        let out = root.join(".mdp").join("README.dry.md");
        let result = refresh_readme(&root, Some(&out), true).expect("dry run");
        assert_eq!(result["status"], "dry-run");
        assert_eq!(result["dry_run"], true);
        // The in-place README is unchanged by dry-run.
        assert_eq!(
            std::fs::read_to_string(&readme_path).expect("readme unchanged"),
            corrupted
        );
        // The requested out path was not written by dry-run.
        assert!(!out.exists(), "dry-run must not write the out path");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_changes_portable_hash_and_is_stable_for_unchanged_authority() {
        use crate::artifact_hash::pack_content_sha256;
        let root = std::env::temp_dir().join(format!("mdp-readme-hash-{}", nonce()));
        init_pack(&root, "Hash Readme Pack", "gtm", true, false).expect("pack should initialize");
        let hash_fresh = pack_content_sha256(&root).expect("hash");
        // Corrupt the owned block; the portable hash changes because README is
        // inside .mdp, even though structured authority is unchanged.
        let readme_path = root.join(".mdp/README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("readme");
        let corrupted = readme.replace("- cards:", "- card count:");
        std::fs::write(&readme_path, &corrupted).expect("write corrupted");
        let hash_corrupted = pack_content_sha256(&root).expect("hash");
        assert_ne!(hash_fresh, hash_corrupted);

        refresh_readme(&root, None, false).expect("refresh");
        let hash_repaired = pack_content_sha256(&root).expect("hash");
        // Repairing the owned block restores the byte-stable portable hash for
        // unchanged structured authority.
        assert_eq!(hash_fresh, hash_repaired);
        // A second refresh with unchanged authority keeps the hash stable.
        refresh_readme(&root, None, false).expect("refresh again");
        assert_eq!(hash_fresh, pack_content_sha256(&root).expect("hash"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_readme_without_marker_is_unassessed_and_does_not_block() {
        let root = std::env::temp_dir().join(format!("mdp-readme-legacy-{}", nonce()));
        init_pack(&root, "Legacy Readme Pack", "gtm", true, false).expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        std::fs::write(&readme_path, "# Legacy Pack\n\nOrientation prose only.\n")
            .expect("write legacy readme");

        let result = check_readme(&root).expect("check should run");
        assert_eq!(result["status"], "unassessed");
        assert_eq!(result["valid"], true);
        assert_eq!(result["drift"], false);
        assert_eq!(result["has_owned_block"], false);
        assert_eq!(
            result["changed_generated_regions"],
            json!(["ownership", "inventory"])
        );
        assert_eq!(result["generated_regions"][0]["changed"], true);
        assert_eq!(result["generated_regions"][1]["changed"], true);
        assert!(readme_validation_issues(&root).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_migrates_legacy_readme_by_appending_owned_block() {
        let root = std::env::temp_dir().join(format!("mdp-readme-migrate-{}", nonce()));
        init_pack(&root, "Migrate Readme Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        std::fs::write(
            &readme_path,
            "# Legacy Hand Authored\n\nCustom orientation a human wrote.\n",
        )
        .expect("write legacy readme");

        let result = refresh_readme(&root, None, false).expect("refresh");
        assert_eq!(result["status"], "saved");
        assert_eq!(result["had_owned_block"], false);
        assert_eq!(
            result["changed_generated_regions"],
            json!(["ownership", "inventory"])
        );
        let after = std::fs::read_to_string(&readme_path).expect("readme after");
        assert!(after.starts_with("# Legacy Hand Authored"));
        assert!(after.contains("Custom orientation a human wrote."));
        assert!(after.contains(crate::pack_readme::README_OWNERSHIP_BEGIN));
        assert!(after.contains(crate::pack_readme::README_INVENTORY_BEGIN));
        // After migration, check reports fresh.
        let check = check_readme(&root).expect("check");
        assert_eq!(check["status"], "fresh");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_markers_fail_repeated_refresh_without_changing_human_bytes() {
        let root = std::env::temp_dir().join(format!("mdp-readme-malformed-{}", nonce()));
        init_pack(&root, "Malformed Readme Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let ownership_begin = crate::pack_readme::README_OWNERSHIP_BEGIN;
        let ownership_end = crate::pack_readme::README_OWNERSHIP_END;
        let inventory_begin = crate::pack_readme::README_INVENTORY_BEGIN;
        let inventory_end = crate::pack_readme::README_INVENTORY_END;
        let cases = [
            format!("# Human\n\nkeep unmatched bytes\n{ownership_begin}\n"),
            format!(
                "# Human\n\n{ownership_begin}\nlegend\n{ownership_end}\nkeep duplicate bytes\n{ownership_begin}\nlegend two\n{ownership_end}\n"
            ),
            format!(
                "# Human\n\n{ownership_begin}\nkeep nested bytes\n{inventory_begin}\ninventory\n{ownership_end}\n{inventory_end}\n"
            ),
        ];
        for malformed in cases {
            std::fs::write(&readme_path, &malformed).expect("write malformed readme");
            for _ in 0..2 {
                let error = refresh_readme(&root, None, false)
                    .expect_err("malformed markers must fail closed");
                assert_eq!(
                    error.to_string(),
                    crate::pack_readme::README_MARKER_DIAGNOSTIC
                );
                assert_eq!(
                    std::fs::read_to_string(&readme_path).expect("readme after refusal"),
                    malformed
                );
            }
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_markers_are_a_stable_validate_blocker() {
        let root = std::env::temp_dir().join(format!("mdp-readme-validate-markers-{}", nonce()));
        init_pack(&root, "Marker Validation Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let mut readme = std::fs::read_to_string(&readme_path).expect("readme");
        readme.push_str(crate::pack_readme::README_OWNERSHIP_BEGIN);
        std::fs::write(&readme_path, readme).expect("write malformed readme");

        let validation = crate::commands::health::validate_pack(&root).expect("validate");
        assert_eq!(validation["valid"], false);
        assert_eq!(validation["error_count"], 1);
        let marker = validation["issues"]
            .as_array()
            .expect("issues")
            .iter()
            .find(|issue| issue["code"] == "readme_marker_layout_invalid")
            .expect("marker layout diagnostic");
        assert_eq!(marker["severity"], "error");
        assert_eq!(marker["path"], readme_path.display().to_string());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_preserves_human_only_edits_and_reports_no_generated_change() {
        let root = std::env::temp_dir().join(format!("mdp-readme-human-only-{}", nonce()));
        init_pack(&root, "Human Only Readme Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let mut edited = std::fs::read_to_string(&readme_path).expect("readme");
        edited.push_str("\nHuman-owned conference note; preserve these exact bytes.\n");
        std::fs::write(&readme_path, &edited).expect("write human edit");

        let result = refresh_readme(&root, None, false).expect("refresh");
        assert_eq!(result["changed_generated_regions"], json!([]));
        assert_eq!(
            std::fs::read_to_string(&readme_path).expect("refreshed readme"),
            edited
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn removed_card_and_source_references_warn_without_rewriting_human_prose() {
        let root = std::env::temp_dir().join(format!("mdp-readme-broken-refs-{}", nonce()));
        init_pack(&root, "Broken Reference Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let before = std::fs::read_to_string(&readme_path).expect("readme");

        let mut manifest = read_manifest(&root).expect("manifest");
        let removed_index = manifest
            .cards
            .iter()
            .position(|card| before.contains(&format!("`{}`", card.path)))
            .expect("generated README should reference at least one manifest card");
        let removed_card = manifest.cards.remove(removed_index);
        std::fs::write(
            root.join(".mdp/manifest.yaml"),
            serde_yaml::to_string(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");

        let sources_path = root.join(".mdp/sources.yaml");
        let mut sources: YamlValue =
            serde_yaml::from_str(&std::fs::read_to_string(&sources_path).expect("sources"))
                .expect("parse sources");
        let source_rows = sources["sources"].as_sequence_mut().expect("source rows");
        let removed_source = source_rows[0]["id"].as_str().unwrap().to_string();
        assert!(before.contains(&format!("`{removed_source}`")));
        source_rows.remove(0);
        std::fs::write(
            &sources_path,
            serde_yaml::to_string(&sources).expect("serialize sources"),
        )
        .expect("write sources");

        let result = check_readme(&root).expect("check");
        let warnings = result["warnings"].as_array().expect("warnings");
        assert!(warnings.iter().any(|warning| {
            warning["code"] == "readme_human_card_reference_missing"
                && warning["reference"] == removed_card.path
                && warning["authority"] == "non-authoritative-mechanical-warning"
        }));
        assert!(warnings.iter().any(|warning| {
            warning["code"] == "readme_human_source_reference_missing"
                && warning["reference"] == removed_source
        }));
        assert_eq!(
            std::fs::read_to_string(&readme_path).expect("unchanged readme"),
            before,
            "checking references must not rewrite human prose"
        );
        let validation_issues = readme_validation_issues(&root);
        assert!(
            validation_issues
                .iter()
                .any(|warning| { warning["code"] == "readme_human_card_reference_missing" })
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn source_reference_parser_ignores_near_headings_inline_code_and_fences() {
        let markdown = r#"# Human README

## Sources appendix
- `near-heading`: must be ignored

```markdown
## Sources
- `fenced-heading`: must be ignored
```

## Sources
Inline `inline-code` must be ignored.

```text
- `fenced-code`: must be ignored
```

- not-backticked: ignored
- `missing-colon` ignored
- `declared-source`: accepted list shape

## Next
- `outside-section`: ignored
"#;
        assert_eq!(source_reference_ids(markdown), vec!["declared-source"]);
    }

    #[test]
    fn validate_pack_reports_drift_as_warning_that_strict_promotes_to_blocker() {
        let root = std::env::temp_dir().join(format!("mdp-readme-validate-{}", nonce()));
        init_pack(&root, "Validate Drift Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("readme");
        let corrupted = readme.replace("- cards:", "- card count:");
        std::fs::write(&readme_path, &corrupted).expect("write corrupted readme");

        let validation = crate::commands::health::validate_pack(&root).expect("validate");
        // Non-strict validate treats drift as a warning so legacy and
        // card-mutating flows remain compatible; strict validation promotes it
        // to a blocker via apply_strict.
        assert_eq!(validation["valid"], true);
        let issues = validation["issues"].as_array().expect("issues array");
        let drift = issues
            .iter()
            .find(|issue| issue["code"] == "readme_inventory_drift")
            .expect("validate should report readme drift");
        assert_eq!(drift["severity"], "warning");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_pack_leaves_legacy_readme_unassessed_without_blocking() {
        let root = std::env::temp_dir().join(format!("mdp-readme-validate-legacy-{}", nonce()));
        init_pack(&root, "Validate Legacy Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        std::fs::write(&readme_path, "# Legacy\n\nNo marker here.\n").expect("write legacy");
        let validation = crate::commands::health::validate_pack(&root).expect("validate");
        let issues = validation["issues"].as_array().expect("issues");
        assert!(
            !issues
                .iter()
                .any(|issue| issue["code"] == "readme_inventory_drift"),
            "legacy README must not produce a drift issue"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
