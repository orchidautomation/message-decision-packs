use crate::constants::DEFAULT_DIR;
use crate::pack_io::read_manifest;
use anyhow::{Context, Result};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub(crate) fn pack(root: &Path) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let mut packed_cards = Vec::new();
    for card_ref in &manifest.cards {
        let path = root.join(DEFAULT_DIR).join(&card_ref.path);
        let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let hash = Sha256::digest(&bytes);
        packed_cards.push(json!({"id": card_ref.id, "kind": card_ref.kind, "path": card_ref.path, "sha256": format!("{hash:x}"), "bytes": bytes.len()}));
    }
    Ok(json!({"format": "mdp.pack.v0", "manifest": manifest, "cards": packed_cards}))
}
