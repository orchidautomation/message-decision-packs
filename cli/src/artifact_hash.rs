use crate::constants::DEFAULT_DIR;
use anyhow::{Context, Result, anyhow};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn canonical_json_sha256(value: &Value) -> Result<String> {
    let canonical = canonicalize_json(value);
    Ok(sha256_hex(&serde_json::to_vec(&canonical)?))
}

pub(crate) fn pack_content_sha256(root: &Path) -> Result<String> {
    let pack_root = root.join(DEFAULT_DIR);
    let mut files = Vec::new();
    collect_regular_files(&pack_root, &pack_root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    for (relative, path) in files {
        let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(sha256_hex(&bytes).as_bytes());
        hasher.update(b"\n");
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_regular_files(
    pack_root: &Path,
    directory: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<()> {
    let mut entries = fs::read_dir(directory)
        .with_context(|| format!("reading pack directory {}", directory.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("reading metadata for {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(anyhow!(
                "portable pack digest does not allow symlinks: {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_regular_files(pack_root, &path, files)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(anyhow!(
                "portable pack digest only allows regular files: {}",
                path.display()
            ));
        }
        let relative = path
            .strip_prefix(pack_root)
            .expect("collected pack file should remain under pack root")
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        files.push((relative, path));
    }
    Ok(())
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let canonical = keys
                .into_iter()
                .map(|key| (key.clone(), canonicalize_json(&object[key])))
                .collect::<Map<_, _>>();
            Value::Object(canonical)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json).collect()),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{canonical_json_sha256, pack_content_sha256};
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn canonical_json_hash_ignores_object_key_order() {
        let first = json!({"b": 2, "a": {"d": 4, "c": 3}});
        let second = json!({"a": {"c": 3, "d": 4}, "b": 2});
        assert_eq!(
            canonical_json_sha256(&first).unwrap(),
            canonical_json_sha256(&second).unwrap()
        );
    }

    #[test]
    fn pack_hash_is_path_independent_and_changes_with_bytes() {
        let base = std::env::temp_dir().join(format!("mdp-pack-hash-{}", nonce()));
        let first = base.join("first");
        let second = base.join("second");
        for root in [&first, &second] {
            fs::create_dir_all(root.join(".mdp/cards")).unwrap();
            fs::write(root.join(".mdp/manifest.yaml"), "format: mdp.v0\n").unwrap();
            fs::write(root.join(".mdp/cards/a.yaml"), "id: a\n").unwrap();
        }
        assert_eq!(
            pack_content_sha256(&first).unwrap(),
            pack_content_sha256(&second).unwrap()
        );

        fs::write(second.join(".mdp/cards/a.yaml"), "id: changed\n").unwrap();
        assert_ne!(
            pack_content_sha256(&first).unwrap(),
            pack_content_sha256(&second).unwrap()
        );
        let _ = fs::remove_dir_all(base);
    }

    fn nonce() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
