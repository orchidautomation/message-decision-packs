use crate::constants::DEFAULT_DIR;
use crate::pack_io::{display_pack_path, read_manifest, read_prompt, resolve_pack_path};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub(crate) fn pack(root: &Path) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let mut packed_cards = Vec::new();
    for card_ref in &manifest.cards {
        let path = resolve_pack_path(root, &card_ref.path)?;
        let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let hash = Sha256::digest(&bytes);
        packed_cards.push(json!({"id": card_ref.id, "kind": card_ref.kind, "path": display_pack_path(&card_ref.path), "sha256": format!("{hash:x}"), "bytes": bytes.len()}));
    }
    let mut packed_prompts = Vec::new();
    let prompts_dir = root.join(DEFAULT_DIR).join("prompts");
    if prompts_dir.exists() {
        let mut prompt_paths = fs::read_dir(&prompts_dir)?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.is_file())
            .filter(|path| {
                matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("yaml" | "yml")
                )
            })
            .collect::<Vec<_>>();
        prompt_paths.sort();
        for path in prompt_paths {
            let prompt = read_prompt(&path)?;
            let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            let hash = Sha256::digest(&bytes);
            let display_path = format!(
                "{DEFAULT_DIR}/prompts/{}",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("<invalid>")
            );
            packed_prompts.push(json!({"id": prompt.id, "target_card_kinds": prompt.target_card_kinds, "path": display_path, "sha256": format!("{hash:x}"), "bytes": bytes.len()}));
        }
    }
    Ok(
        json!({"format": "mdp.pack.v0", "manifest": manifest, "cards": packed_cards, "prompts": packed_prompts}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn pack_includes_prompt_hashes_when_present() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-pack-prompts-{nonce}"));
        init_pack(&root, "Pack Prompts", "gtm", true).expect("pack should initialize");

        let result = pack(&root).expect("pack should compile");

        assert_eq!(
            result["prompts"].as_array().expect("prompts array").len(),
            8
        );
        assert_eq!(
            result["prompts"][0]["path"],
            ".mdp/prompts/avoid-rules.yaml"
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
