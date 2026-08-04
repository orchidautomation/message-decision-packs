use crate::constants::DEFAULT_DIR;
use anyhow::{Context, Result, anyhow};
use serde::de::{
    DeserializeOwned, DeserializeSeed, Error as DeError, MapAccess, SeqAccess, Visitor,
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const GENERATED_PACK_DIRECTORIES: &[&str] = &["briefs", "traces"];
const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthorityJsonLimits {
    pub(crate) max_bytes: usize,
    pub(crate) max_depth: usize,
    pub(crate) max_object_members: usize,
    pub(crate) max_array_length: usize,
    pub(crate) max_string_bytes: usize,
}

impl Default for AuthorityJsonLimits {
    fn default() -> Self {
        Self {
            max_bytes: 1_048_576,
            max_depth: 32,
            max_object_members: 1_024,
            max_array_length: 10_000,
            max_string_bytes: 262_144,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableFileRecord {
    pub(crate) logical_path: String,
    pub(crate) byte_count: u64,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortablePackSnapshot {
    pub(crate) contract: String,
    pub(crate) files: Vec<PortableFileRecord>,
    pub(crate) sha256: String,
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn canonical_json_sha256(value: &Value) -> Result<String> {
    let canonical = canonicalize_json(value);
    Ok(sha256_hex(&serde_json::to_vec(&canonical)?))
}

pub(crate) fn canonical_json_sha256_for_domain(domain: &str, value: &Value) -> Result<String> {
    if domain.is_empty() || !domain.is_ascii() {
        return Err(anyhow!(
            "canonical JSON hash domain must be non-empty ASCII"
        ));
    }
    validate_authority_numbers(value)?;
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical)?;
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn parse_authority_json<T: DeserializeOwned>(
    bytes: &[u8],
    limits: AuthorityJsonLimits,
) -> Result<T> {
    if bytes.len() > limits.max_bytes {
        return Err(anyhow!(
            "authority JSON exceeds {} byte limit",
            limits.max_bytes
        ));
    }
    if contains_negative_zero_token(bytes) {
        return Err(anyhow!("authority JSON does not allow negative zero"));
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = AuthoritySeed { limits, depth: 0 }
        .deserialize(&mut deserializer)
        .map_err(|error| anyhow!(error.to_string()))?;
    deserializer
        .end()
        .map_err(|error| anyhow!(error.to_string()))?;
    serde_json::from_value(value).map_err(Into::into)
}

pub(crate) fn pack_content_sha256(root: &Path) -> Result<String> {
    Ok(pack_content_snapshot(root)?.sha256)
}

pub(crate) fn pack_content_snapshot(root: &Path) -> Result<PortablePackSnapshot> {
    let pack_root = root.join(DEFAULT_DIR);
    let mut files = Vec::new();
    collect_regular_files(&pack_root, &pack_root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    let mut records = Vec::with_capacity(files.len());
    let mut case_folded = HashSet::new();
    for (relative, path) in files {
        if !relative.is_ascii() {
            return Err(anyhow!(
                "portable pack digest only allows ASCII logical paths: {relative}"
            ));
        }
        if !case_folded.insert(relative.to_ascii_lowercase()) {
            return Err(anyhow!(
                "portable pack digest rejects case-colliding logical path: {relative}"
            ));
        }
        let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let file_hash = sha256_hex(&bytes);
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(file_hash.as_bytes());
        hasher.update(b"\n");
        records.push(PortableFileRecord {
            logical_path: relative,
            byte_count: bytes.len() as u64,
            sha256: file_hash,
        });
    }
    Ok(PortablePackSnapshot {
        contract: "mdp.portable-pack-snapshot.v1".to_string(),
        files: records,
        sha256: format!("{:x}", hasher.finalize()),
    })
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
            if directory == pack_root
                && GENERATED_PACK_DIRECTORIES
                    .iter()
                    .any(|generated| entry.file_name() == *generated)
            {
                continue;
            }
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
            .map(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .map(str::to_string)
                    .ok_or_else(|| {
                        anyhow!("portable pack path is not valid UTF-8: {}", path.display())
                    })
            })
            .collect::<Result<Vec<_>>>()?
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

fn validate_authority_numbers(value: &Value) -> Result<()> {
    match value {
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                if value.unsigned_abs() > MAX_SAFE_JSON_INTEGER {
                    return Err(anyhow!("authority JSON integer is outside the safe range"));
                }
            } else if let Some(value) = number.as_u64() {
                if value > MAX_SAFE_JSON_INTEGER {
                    return Err(anyhow!("authority JSON integer is outside the safe range"));
                }
            } else {
                return Err(anyhow!(
                    "authority JSON does not allow floating-point numbers"
                ));
            }
        }
        Value::Array(items) => {
            for item in items {
                validate_authority_numbers(item)?;
            }
        }
        Value::Object(object) => {
            for item in object.values() {
                validate_authority_numbers(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct AuthoritySeed {
    limits: AuthorityJsonLimits,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for AuthoritySeed {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        if self.depth > self.limits.max_depth {
            return Err(D::Error::custom(
                "authority JSON exceeds nesting-depth limit",
            ));
        }
        deserializer.deserialize_any(AuthorityVisitor(self))
    }
}

struct AuthorityVisitor(AuthoritySeed);

impl<'de> Visitor<'de> for AuthorityVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded authority JSON")
    }

    fn visit_bool<E>(self, value: bool) -> std::result::Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E>
    where
        E: DeError,
    {
        if value.unsigned_abs() > MAX_SAFE_JSON_INTEGER {
            return Err(E::custom(
                "authority JSON integer is outside the safe range",
            ));
        }
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E>
    where
        E: DeError,
    {
        if value > MAX_SAFE_JSON_INTEGER {
            return Err(E::custom(
                "authority JSON integer is outside the safe range",
            ));
        }
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, _value: f64) -> std::result::Result<Self::Value, E>
    where
        E: DeError,
    {
        Err(E::custom(
            "authority JSON does not allow floating-point numbers",
        ))
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
    where
        E: DeError,
    {
        self.visit_string(value.to_string())
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E>
    where
        E: DeError,
    {
        if value.len() > self.0.limits.max_string_bytes {
            return Err(E::custom("authority JSON string exceeds byte limit"));
        }
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(AuthoritySeed {
            limits: self.0.limits,
            depth: self.0.depth + 1,
        })? {
            if values.len() >= self.0.limits.max_array_length {
                return Err(A::Error::custom(
                    "authority JSON array exceeds length limit",
                ));
            }
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut object: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if key.len() > self.0.limits.max_string_bytes {
                return Err(A::Error::custom("authority JSON key exceeds byte limit"));
            }
            if values.len() >= self.0.limits.max_object_members {
                return Err(A::Error::custom(
                    "authority JSON object exceeds member limit",
                ));
            }
            if values.contains_key(&key) {
                return Err(A::Error::custom(format!(
                    "authority JSON contains duplicate member: {key}"
                )));
            }
            let value = object.next_value_seed(AuthoritySeed {
                limits: self.0.limits,
                depth: self.0.depth + 1,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

fn contains_negative_zero_token(bytes: &[u8]) -> bool {
    let mut in_string = false;
    let mut escaped = false;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            index += 1;
            continue;
        }
        if byte == b'-' && bytes.get(index + 1) == Some(&b'0') {
            let prior_is_number = index > 0
                && matches!(
                    bytes[index - 1],
                    b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-'
                );
            let next = bytes.get(index + 2).copied();
            let next_ends_number = next.is_none()
                || matches!(
                    next,
                    Some(b' ' | b'\t' | b'\r' | b'\n' | b',' | b']' | b'}')
                );
            if !prior_is_number && next_ends_number {
                return true;
            }
        }
        index += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{
        AuthorityJsonLimits, canonical_json_sha256, canonical_json_sha256_for_domain,
        pack_content_sha256, pack_content_snapshot, parse_authority_json,
    };
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
    fn authority_hash_is_domain_separated_and_rejects_unsafe_numbers() {
        let value = json!({"count": 9_007_199_254_740_991_i64});
        let bundle = canonical_json_sha256_for_domain("mdp.run-bundle.v1", &value).unwrap();
        let receipt = canonical_json_sha256_for_domain("mdp.run-receipt.v1", &value).unwrap();
        assert_ne!(bundle, receipt);

        let unsafe_value = json!({"count": 9_007_199_254_740_992_u64});
        assert!(canonical_json_sha256_for_domain("mdp.run-bundle.v1", &unsafe_value).is_err());
        assert!(
            canonical_json_sha256_for_domain("mdp.run-bundle.v1", &json!({"ratio": 1.5})).is_err()
        );
    }

    #[test]
    fn bounded_authority_parser_rejects_duplicates_negative_zero_and_depth() {
        let limits = AuthorityJsonLimits {
            max_depth: 2,
            ..AuthorityJsonLimits::default()
        };
        assert!(parse_authority_json::<serde_json::Value>(br#"{"a":1,"a":2}"#, limits).is_err());
        assert!(parse_authority_json::<serde_json::Value>(br#"{"a":-0}"#, limits).is_err());
        assert!(
            parse_authority_json::<serde_json::Value>(br#"{"a":{"b":{"c":1}}}"#, limits).is_err()
        );
        assert!(parse_authority_json::<serde_json::Value>(br#"{"a":{"b":1}}"#, limits).is_ok());
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
        let snapshot = pack_content_snapshot(&first).unwrap();
        assert_eq!(snapshot.contract, "mdp.portable-pack-snapshot.v1");
        assert_eq!(snapshot.files.len(), 2);
        assert_eq!(snapshot.sha256, pack_content_sha256(&first).unwrap());
        assert_eq!(snapshot.files[0].logical_path, "cards/a.yaml");
        assert_eq!(snapshot.files[1].logical_path, "manifest.yaml");

        fs::write(second.join(".mdp/cards/a.yaml"), "id: changed\n").unwrap();
        assert_ne!(
            pack_content_sha256(&first).unwrap(),
            pack_content_sha256(&second).unwrap()
        );
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn pack_hash_excludes_generated_briefs_and_traces() {
        let root = std::env::temp_dir().join(format!("mdp-pack-generated-hash-{}", nonce()));
        fs::create_dir_all(root.join(".mdp/cards")).unwrap();
        fs::write(root.join(".mdp/manifest.yaml"), "format: mdp.v0\n").unwrap();
        fs::write(root.join(".mdp/cards/a.yaml"), "id: a\n").unwrap();
        let authored_hash = pack_content_sha256(&root).unwrap();

        fs::create_dir_all(root.join(".mdp/briefs")).unwrap();
        fs::create_dir_all(root.join(".mdp/traces")).unwrap();
        fs::write(
            root.join(".mdp/briefs/brief.json"),
            "{\"generated\":true}\n",
        )
        .unwrap();
        fs::write(root.join(".mdp/traces/run.json"), "{\"generated\":true}\n").unwrap();
        assert_eq!(pack_content_sha256(&root).unwrap(), authored_hash);

        fs::write(root.join(".mdp/cards/a.yaml"), "id: changed\n").unwrap();
        assert_ne!(pack_content_sha256(&root).unwrap(), authored_hash);
        let _ = fs::remove_dir_all(root);
    }

    fn nonce() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
