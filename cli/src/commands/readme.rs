use crate::artifact_hash::sha256_hex;
use crate::commands::health::issue;
use crate::constants::DEFAULT_DIR;
use crate::models::{Card, Manifest};
use crate::pack_io::{read_card, read_manifest, resolve_pack_path};
use crate::pack_readme::{
    README_INVENTORY_CONTRACT, extract_inventory_block, render_inventory_block,
    replace_inventory_block,
};
use anyhow::{Context, Result};
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
    let Some(existing_block) = extract_inventory_block(&existing) else {
        return Ok(json!({
            "contract": README_INVENTORY_CONTRACT,
            "status": "unassessed",
            "valid": true,
            "drift": false,
            "has_owned_block": false,
            "path": readme_path.display().to_string()
        }));
    };

    let (manifest, cards, source_ledger, prompt_ids) = load_readme_authority(root)?;
    let card_refs = cards.iter().collect::<Vec<_>>();
    let fresh_block = render_inventory_block(&manifest, &card_refs, &source_ledger, &prompt_ids);
    if existing_block == fresh_block {
        Ok(json!({
            "contract": README_INVENTORY_CONTRACT,
            "status": "fresh",
            "valid": true,
            "drift": false,
            "has_owned_block": true,
            "path": readme_path.display().to_string(),
            "inventory_sha256": sha256_hex(fresh_block.as_bytes())
        }))
    } else {
        Ok(json!({
            "contract": README_INVENTORY_CONTRACT,
            "status": "stale",
            "valid": false,
            "drift": true,
            "has_owned_block": true,
            "path": readme_path.display().to_string(),
            "expected_sha256": sha256_hex(fresh_block.as_bytes()),
            "actual_sha256": sha256_hex(existing_block.as_bytes()),
            "diagnostics": [stale_drift_issue(&readme_path, "error")]
        }))
    }
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
    let updated = replace_inventory_block(&existing, &fresh_block);
    let had_owned_block = extract_inventory_block(&existing).is_some();
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
        "block": fresh_block
    });
    if dry_run {
        if let Some(object) = data.as_object_mut() {
            object.insert("dry_run".to_string(), json!(true));
        }
    }
    Ok(data)
}

/// Validate-pack integration hook. Returns a blocking issue only when the owned
/// block exists and is stale. Missing README, legacy prose without the marker,
/// unreadable manifest/cards, or a fresh block all return `None`; the ordinary
/// validate paths already surface manifest/card/read failures.
pub(crate) fn readme_drift_issue(root: &Path) -> Option<Value> {
    let result = check_readme(root).ok()?;
    if result["status"] == "stale" {
        // Validate integration surfaces drift as a warning so legacy and
        // card-mutating flows remain compatible; strict validation promotes
        // it to a blocker. The standalone `readme check` command keeps its own
        // error-level diagnostic.
        Some(stale_drift_issue(
            &std::path::PathBuf::from(result["path"].as_str().unwrap_or(".mdp/README.md")),
            "warning",
        ))
    } else {
        None
    }
}

fn stale_drift_issue(readme_path: &Path, severity: &str) -> Value {
    issue(
        "readme_inventory_drift",
        severity,
        readme_path.display().to_string(),
        "README inventory block does not match loaded structured authority; run `mdp readme refresh --dir .` to regenerate the owned block, or remove unverifiable manual counts.",
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
        assert!(readme_drift_issue(&root).is_none());
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
        let after = std::fs::read_to_string(&readme_path).expect("readme after");
        assert!(after.starts_with("# Legacy Hand Authored"));
        assert!(after.contains("Custom orientation a human wrote."));
        assert!(after.contains(crate::pack_readme::README_INVENTORY_BEGIN));
        // After migration, check reports fresh.
        let check = check_readme(&root).expect("check");
        assert_eq!(check["status"], "fresh");
        let _ = std::fs::remove_dir_all(root);
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
