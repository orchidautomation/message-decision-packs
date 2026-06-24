use crate::constants::DEFAULT_DIR;
use crate::models::{Card, Manifest, Prospect};
use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::Path;

pub(crate) fn read_manifest(root: &Path) -> Result<Manifest> {
    let path = root.join(DEFAULT_DIR).join("manifest.yaml");
    let raw = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

pub(crate) fn read_card(path: &Path) -> Result<Card> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

pub(crate) fn read_card_by_id(root: &Path, id: &str) -> Result<Card> {
    let manifest = read_manifest(root)?;
    let card_ref = manifest
        .cards
        .iter()
        .find(|card_ref| card_ref.id == id)
        .ok_or_else(|| anyhow!("missing card id {id}"))?;
    read_card(&root.join(DEFAULT_DIR).join(&card_ref.path))
}

pub(crate) fn read_prospect(path: &Path) -> Result<Prospect> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

pub(crate) fn write_yaml<T: Serialize>(path: &Path, value: &T, force: bool) -> Result<()> {
    if path.exists() && !force {
        return Err(anyhow!(
            "{} already exists; pass --force to overwrite",
            path.display()
        ));
    }
    let raw = serde_yaml::to_string(value)?;
    fs::write(path, raw).with_context(|| format!("writing {}", path.display()))
}

pub(crate) fn write_json_file(path: &Path, value: &Value) -> Result<()> {
    let mut file =
        fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    file.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}
