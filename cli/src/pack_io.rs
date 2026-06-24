use crate::constants::DEFAULT_DIR;
use crate::models::{Card, Manifest, Prospect};
use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

pub(crate) fn read_manifest(root: &Path) -> Result<Manifest> {
    let path = root.join(DEFAULT_DIR).join("manifest.yaml");
    let raw = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

pub(crate) fn read_card(path: &Path) -> Result<Card> {
    let raw = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

pub(crate) fn resolve_pack_path(root: &Path, manifest_path: &str) -> Result<PathBuf> {
    let path = Path::new(manifest_path);
    if path.is_absolute() {
        return Err(anyhow!(
            "manifest card path must be relative under .mdp: {manifest_path}"
        ));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(anyhow!(
            "manifest card path must not contain traversal: {manifest_path}"
        ));
    }
    let pack_dir = root.join(DEFAULT_DIR);
    let resolved = pack_dir.join(path);
    if let (Ok(pack_root), Ok(real_path)) = (pack_dir.canonicalize(), resolved.canonicalize()) {
        if !real_path.starts_with(&pack_root) {
            return Err(anyhow!(
                "manifest card path resolves outside .mdp: {manifest_path}"
            ));
        }
    }
    Ok(resolved)
}

pub(crate) fn display_pack_path(manifest_path: &str) -> String {
    format!("{DEFAULT_DIR}/{manifest_path}")
}

pub(crate) fn read_card_by_id(root: &Path, id: &str) -> Result<Card> {
    let manifest = read_manifest(root)?;
    let card_ref = manifest
        .cards
        .iter()
        .find(|card_ref| card_ref.id == id)
        .ok_or_else(|| anyhow!("missing card id {id}"))?;
    read_card(&resolve_pack_path(root, &card_ref.path)?)
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
