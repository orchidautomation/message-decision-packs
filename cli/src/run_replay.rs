//! Conformance-only local replay protection for clean-run receipts.
//!
//! The ledger serializes compare-and-consume operations with an exclusive
//! `create_new` lock and stores newline-delimited, hash-chained records. It is
//! intentionally a local reference implementation, not production replay
//! infrastructure.

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub(crate) const REPLAY_LEDGER_CONTRACT: &str = "mdp.local-replay-ledger.v1";
pub(crate) const LOCAL_LEDGER_DURABILITY_LIMITATION: &str = "The local reference ledger cannot detect filesystem rollback, snapshot restore, or cloning. Production replay protection requires host-owned durable, atomic storage with an independently enforced monotonic version.";

const RECORD_HASH_DOMAIN: &[u8] = b"mdp.local-replay-ledger-record.v1\0";
const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_LEDGER_BYTES: u64 = 16 * 1024 * 1024;
const MAX_IDENTITY_BYTES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReplayConsumeRequest {
    pub(crate) job_id: String,
    pub(crate) idempotency_key: String,
    pub(crate) receipt_sha256: String,
    /// For a first consume, this is the current ledger version. For an exact
    /// replay, this remains the version that preceded the original record.
    pub(crate) expected_prior_version: u64,
    pub(crate) permit_exact_replay: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub(crate) enum ReplayConsumeOutcome {
    AcceptedFirst { version: u64, record_sha256: String },
    PermittedExactReplay { version: u64, record_sha256: String },
    Duplicate { existing_version: u64 },
    CrossJob { existing_version: u64 },
    PriorVersionMismatch { expected: u64, actual: u64 },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReplayRecord {
    contract: String,
    version: u64,
    prior_record_sha256: String,
    job_id: String,
    idempotency_key: String,
    receipt_sha256: String,
    record_sha256: String,
}

#[derive(Serialize)]
struct RecordHashInput<'a> {
    contract: &'a str,
    version: u64,
    prior_record_sha256: &'a str,
    job_id: &'a str,
    idempotency_key: &'a str,
    receipt_sha256: &'a str,
}

/// Atomically compares a request with the local ledger and consumes it once.
///
/// A leftover lock, malformed final line, broken record hash, or broken chain
/// is treated as unsafe state and returns an error without consuming anything.
pub(crate) fn compare_and_consume(
    ledger_path: &Path,
    request: &ReplayConsumeRequest,
) -> Result<ReplayConsumeOutcome> {
    validate_request(request)?;
    ensure_safe_ledger_target(ledger_path)?;
    let _lock = LedgerLock::acquire(ledger_path)?;
    let records = read_verified_records(ledger_path)?;

    if let Some(existing) = records.iter().find(|record| {
        record.job_id == request.job_id
            || record.idempotency_key == request.idempotency_key
            || record.receipt_sha256 == request.receipt_sha256
    }) {
        let original_prior_version = existing.version - 1;
        if request.expected_prior_version != original_prior_version {
            return Ok(ReplayConsumeOutcome::PriorVersionMismatch {
                expected: request.expected_prior_version,
                actual: original_prior_version,
            });
        }
        if existing.job_id != request.job_id {
            return Ok(ReplayConsumeOutcome::CrossJob {
                existing_version: existing.version,
            });
        }
        if existing.receipt_sha256 != request.receipt_sha256 || !request.permit_exact_replay {
            return Ok(ReplayConsumeOutcome::Duplicate {
                existing_version: existing.version,
            });
        }
        return Ok(ReplayConsumeOutcome::PermittedExactReplay {
            version: existing.version,
            record_sha256: existing.record_sha256.clone(),
        });
    }

    let current_version = records.last().map_or(0, |record| record.version);
    if request.expected_prior_version != current_version {
        return Ok(ReplayConsumeOutcome::PriorVersionMismatch {
            expected: request.expected_prior_version,
            actual: current_version,
        });
    }

    let version = current_version
        .checked_add(1)
        .ok_or_else(|| anyhow!("local replay ledger version overflow"))?;
    let prior_record_sha256 = records.last().map_or_else(
        || ZERO_HASH.to_string(),
        |record| record.record_sha256.clone(),
    );
    let mut record = ReplayRecord {
        contract: REPLAY_LEDGER_CONTRACT.to_string(),
        version,
        prior_record_sha256,
        job_id: request.job_id.clone(),
        idempotency_key: request.idempotency_key.clone(),
        receipt_sha256: request.receipt_sha256.clone(),
        record_sha256: String::new(),
    };
    record.record_sha256 = calculate_record_hash(&record)?;
    append_record(ledger_path, &record)?;

    Ok(ReplayConsumeOutcome::AcceptedFirst {
        version,
        record_sha256: record.record_sha256,
    })
}

fn validate_request(request: &ReplayConsumeRequest) -> Result<()> {
    validate_identity("job_id", &request.job_id)?;
    validate_identity("idempotency_key", &request.idempotency_key)?;
    validate_hash("receipt_sha256", &request.receipt_sha256)
}

fn validate_identity(name: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_IDENTITY_BYTES
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        bail!("{name} must be non-empty printable ASCII no longer than {MAX_IDENTITY_BYTES} bytes");
    }
    Ok(())
}

fn validate_hash(name: &str, value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("{name} must be a lowercase 64-character SHA-256 hex digest");
    }
    Ok(())
}

fn ensure_safe_ledger_target(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_metadata = fs::metadata(parent)
        .with_context(|| format!("reading replay ledger parent {}", parent.display()))?;
    if !parent_metadata.is_dir() {
        bail!(
            "replay ledger parent is not a directory: {}",
            parent.display()
        );
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("replay ledger must not be a symlink: {}", path.display())
        }
        Ok(metadata) if !metadata.is_file() => {
            bail!("replay ledger is not a regular file: {}", path.display())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("reading replay ledger metadata {}", path.display()))
        }
    }
}

fn read_verified_records(path: &Path) -> Result<Vec<ReplayRecord>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("opening replay ledger {}", path.display()));
        }
    };
    let metadata = file
        .metadata()
        .with_context(|| format!("reading replay ledger metadata {}", path.display()))?;
    if metadata.len() > MAX_LEDGER_BYTES {
        bail!(
            "replay ledger exceeds the {} byte conformance limit",
            MAX_LEDGER_BYTES
        );
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut bytes)
        .with_context(|| format!("reading replay ledger {}", path.display()))?;
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        bail!("replay ledger has an interrupted or unterminated append");
    }
    if bytes.is_empty() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    let mut expected_prior_hash = ZERO_HASH.to_string();
    for (index, line) in bytes[..bytes.len() - 1]
        .split(|byte| *byte == b'\n')
        .enumerate()
    {
        if line.is_empty() {
            bail!(
                "replay ledger contains an empty record at line {}",
                index + 1
            );
        }
        let record: ReplayRecord = serde_json::from_slice(line)
            .with_context(|| format!("parsing replay ledger line {}", index + 1))?;
        let expected_version = (index as u64)
            .checked_add(1)
            .ok_or_else(|| anyhow!("replay ledger version overflow"))?;
        if record.contract != REPLAY_LEDGER_CONTRACT {
            bail!("replay ledger contract mismatch at line {}", index + 1);
        }
        if record.version != expected_version {
            bail!("replay ledger version mismatch at line {}", index + 1);
        }
        if record.prior_record_sha256 != expected_prior_hash {
            bail!("replay ledger hash chain mismatch at line {}", index + 1);
        }
        validate_identity("record.job_id", &record.job_id)?;
        validate_identity("record.idempotency_key", &record.idempotency_key)?;
        validate_hash("record.receipt_sha256", &record.receipt_sha256)?;
        validate_hash("record.record_sha256", &record.record_sha256)?;
        let calculated = calculate_record_hash(&record)?;
        if record.record_sha256 != calculated {
            bail!("replay ledger record hash mismatch at line {}", index + 1);
        }
        expected_prior_hash = record.record_sha256.clone();
        records.push(record);
    }
    Ok(records)
}

fn calculate_record_hash(record: &ReplayRecord) -> Result<String> {
    let hash_input = RecordHashInput {
        contract: &record.contract,
        version: record.version,
        prior_record_sha256: &record.prior_record_sha256,
        job_id: &record.job_id,
        idempotency_key: &record.idempotency_key,
        receipt_sha256: &record.receipt_sha256,
    };
    let serialized = serde_json::to_vec(&hash_input)?;
    let mut hasher = Sha256::new();
    hasher.update(RECORD_HASH_DOMAIN);
    hasher.update(serialized);
    Ok(format!("{:x}", hasher.finalize()))
}

fn append_record(path: &Path, record: &ReplayRecord) -> Result<()> {
    let mut bytes = serde_json::to_vec(record)?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("opening replay ledger for append {}", path.display()))?;
    file.write_all(&bytes)
        .with_context(|| format!("appending replay ledger {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("syncing replay ledger {}", path.display()))
}

struct LedgerLock {
    path: PathBuf,
    file: Option<File>,
}

impl LedgerLock {
    fn acquire(ledger_path: &Path) -> Result<Self> {
        let path = lock_path(ledger_path);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                anyhow!(
                    "refusing replay consume because lock {} exists or cannot be created: {error}",
                    path.display()
                )
            })?;
        writeln!(file, "pid={}", std::process::id())
            .with_context(|| format!("writing replay ledger lock {}", path.display()))?;
        file.sync_all()
            .with_context(|| format!("syncing replay ledger lock {}", path.display()))?;
        Ok(Self {
            path,
            file: Some(file),
        })
    }
}

impl Drop for LedgerLock {
    fn drop(&mut self) {
        self.file.take();
        let _ = fs::remove_file(&self.path);
    }
}

fn lock_path(ledger_path: &Path) -> PathBuf {
    let mut value = ledger_path.as_os_str().to_os_string();
    value.push(".lock");
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::{
        LOCAL_LEDGER_DURABILITY_LIMITATION, ReplayConsumeOutcome, ReplayConsumeRequest,
        compare_and_consume, lock_path,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mdp-run-replay-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn request(job: &str, key: &str, receipt: char, prior: u64) -> ReplayConsumeRequest {
        ReplayConsumeRequest {
            job_id: job.to_string(),
            idempotency_key: key.to_string(),
            receipt_sha256: receipt.to_string().repeat(64),
            expected_prior_version: prior,
            permit_exact_replay: false,
        }
    }

    #[test]
    fn accepts_first_and_permits_only_explicit_exact_replay() {
        let root = test_dir("exact");
        let ledger = root.join("ledger.jsonl");
        let first = request("job-1", "key-1", 'a', 0);
        let accepted = compare_and_consume(&ledger, &first).unwrap();
        let (version, hash) = match accepted {
            ReplayConsumeOutcome::AcceptedFirst {
                version,
                record_sha256,
            } => (version, record_sha256),
            other => panic!("unexpected outcome: {other:?}"),
        };
        assert_eq!(version, 1);

        assert_eq!(
            compare_and_consume(&ledger, &first).unwrap(),
            ReplayConsumeOutcome::Duplicate {
                existing_version: 1
            }
        );
        let mut replay = first;
        replay.permit_exact_replay = true;
        assert_eq!(
            compare_and_consume(&ledger, &replay).unwrap(),
            ReplayConsumeOutcome::PermittedExactReplay {
                version: 1,
                record_sha256: hash
            }
        );
        assert_eq!(fs::read_to_string(&ledger).unwrap().lines().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn distinguishes_duplicate_cross_job_and_prior_version_mismatch() {
        let root = test_dir("outcomes");
        let ledger = root.join("ledger.jsonl");
        compare_and_consume(&ledger, &request("job-1", "shared", 'a', 0)).unwrap();

        assert_eq!(
            compare_and_consume(&ledger, &request("job-1", "shared", 'b', 0)).unwrap(),
            ReplayConsumeOutcome::Duplicate {
                existing_version: 1
            }
        );
        assert_eq!(
            compare_and_consume(&ledger, &request("job-2", "shared", 'a', 0)).unwrap(),
            ReplayConsumeOutcome::CrossJob {
                existing_version: 1
            }
        );
        assert_eq!(
            compare_and_consume(&ledger, &request("job-1", "new-key", 'c', 0)).unwrap(),
            ReplayConsumeOutcome::Duplicate {
                existing_version: 1
            }
        );
        assert_eq!(
            compare_and_consume(&ledger, &request("job-2", "new-key", 'c', 0)).unwrap(),
            ReplayConsumeOutcome::PriorVersionMismatch {
                expected: 0,
                actual: 1
            }
        );
        assert!(matches!(
            compare_and_consume(&ledger, &request("job-2", "new-key", 'c', 1)).unwrap(),
            ReplayConsumeOutcome::AcceptedFirst { version: 2, .. }
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fails_closed_on_corruption_interrupted_append_and_stale_lock() {
        let root = test_dir("fail-closed");
        let ledger = root.join("ledger.jsonl");
        compare_and_consume(&ledger, &request("job-1", "key-1", 'a', 0)).unwrap();
        let original = fs::read(&ledger).unwrap();

        let mut corrupt = original.clone();
        let position = corrupt.iter().position(|byte| *byte == b'a').unwrap();
        corrupt[position] = b'b';
        fs::write(&ledger, &corrupt).unwrap();
        assert!(compare_and_consume(&ledger, &request("job-2", "key-2", 'b', 1)).is_err());

        let mut interrupted = original.clone();
        interrupted.pop();
        fs::write(&ledger, interrupted).unwrap();
        assert!(compare_and_consume(&ledger, &request("job-2", "key-2", 'b', 1)).is_err());

        fs::write(&ledger, original).unwrap();
        fs::write(lock_path(&ledger), b"stale\n").unwrap();
        assert!(compare_and_consume(&ledger, &request("job-2", "key-2", 'b', 1)).is_err());
        fs::remove_file(lock_path(&ledger)).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unsafe_inputs_without_creating_a_ledger() {
        let root = test_dir("inputs");
        let ledger = root.join("ledger.jsonl");
        let mut invalid = request("job-1", "key-1", 'A', 0);
        invalid.receipt_sha256 = "A".repeat(64);
        assert!(compare_and_consume(&ledger, &invalid).is_err());
        assert!(!ledger.exists());
        assert!(LOCAL_LEDGER_DURABILITY_LIMITATION.contains("rollback"));
        assert!(LOCAL_LEDGER_DURABILITY_LIMITATION.contains("cloning"));
        fs::remove_dir_all(root).unwrap();
    }
}
