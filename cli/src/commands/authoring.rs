//! Failure-safe publication for multi-file pack authoring.
//!
//! A candidate is a complete pack tree outside the live pack. `preview` validates
//! that tree with the normal pack validator, seals the expected live and staged
//! file hashes into a bounded change set, and does not touch the live tree.
//! `apply` revalidates both sides, refuses drift, then publishes only `.mdp`
//! authority files through a rollback-protected transaction. Runtime output
//! directories (`.mdp/briefs` and `.mdp/traces`) are excluded by the shared
//! portable-pack snapshot contract and are never changed here.

use crate::artifact_hash::{canonical_json_bytes, pack_content_snapshot, sha256_hex};
use crate::commands::health::validate_pack;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CHANGE_SET_CONTRACT: &str = "mdp.pack-change-set.v1";
pub(crate) const PACK_AUTHORING_RESULT_V1: &str = "mdp.pack-authoring-result.v1";
const MAX_CHANGE_SET_BYTES: u64 = 2_097_152;
const MAX_MANAGED_FILES: usize = 2_048;
const MAX_MANAGED_FILE_BYTES: u64 = 8_388_608;
const MAX_MANAGED_TOTAL_BYTES: u64 = 67_108_864;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FileState {
    present: bool,
    sha256: Option<String>,
    bytes: Option<u64>,
}

impl FileState {
    fn absent() -> Self {
        Self {
            present: false,
            sha256: None,
            bytes: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PlannedFile {
    path: String,
    action: String,
    expected: FileState,
    candidate: FileState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ValidationDiagnostic {
    code: String,
    severity: String,
    path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ValidationSummary {
    valid: bool,
    error_count: u64,
    warning_count: u64,
    diagnostics: Vec<ValidationDiagnostic>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ChangeSetCore {
    contract: String,
    live_root_sha256: String,
    validation: ValidationSummary,
    files: Vec<PlannedFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ChangeSet {
    contract: String,
    live_root_sha256: String,
    validation: ValidationSummary,
    files: Vec<PlannedFile>,
    change_set_sha256: String,
}

impl ChangeSet {
    fn core(&self) -> ChangeSetCore {
        ChangeSetCore {
            contract: self.contract.clone(),
            live_root_sha256: self.live_root_sha256.clone(),
            validation: self.validation.clone(),
            files: self.files.clone(),
        }
    }

    fn verify(&self) -> Result<()> {
        if self.contract != CHANGE_SET_CONTRACT {
            return Err(anyhow!("unsupported pack change-set contract"));
        }
        validate_hash(&self.live_root_sha256, "live root binding")?;
        validate_hash(&self.change_set_sha256, "change-set digest")?;
        if self.files.len() > MAX_MANAGED_FILES {
            return Err(anyhow!("pack change set exceeds managed file limit"));
        }
        let mut paths = BTreeSet::new();
        for file in &self.files {
            validate_logical_path(&file.path)?;
            if !paths.insert(file.path.clone()) {
                return Err(anyhow!("pack change set contains duplicate paths"));
            }
            validate_state(&file.expected)?;
            validate_state(&file.candidate)?;
            if action_for(&file.expected, &file.candidate) != file.action {
                return Err(anyhow!("pack change set action does not match file states"));
            }
        }
        let expected = sha256_hex(&canonical_json_bytes(&serde_json::to_value(self.core())?)?);
        if expected != self.change_set_sha256 {
            return Err(anyhow!(
                "pack change set digest does not match its contents"
            ));
        }
        Ok(())
    }
}

struct AuthorLock {
    path: PathBuf,
}

impl Drop for AuthorLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

struct BoundaryFault {
    after: Option<usize>,
    count: usize,
}

impl BoundaryFault {
    fn from_env() -> Self {
        let after = if cfg!(debug_assertions) {
            std::env::var("MDP_TEST_AUTHOR_FAULT_AFTER")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        } else {
            None
        };
        Self { after, count: 0 }
    }

    fn crossed(&mut self) -> Result<()> {
        self.count += 1;
        if self.after == Some(self.count) {
            return Err(anyhow!("test fault after author publication boundary"));
        }
        Ok(())
    }
}

struct PublishedBackup {
    live: PathBuf,
    backup: PathBuf,
    relative: String,
}

struct PublishedInstall {
    live: PathBuf,
    relative: String,
    expected: FileState,
}

pub(crate) fn preview_pack_change_set(
    live_root: &Path,
    candidate_root: &Path,
    out: &Path,
) -> Result<Value> {
    let live_root = canonical_directory(live_root, "live pack")?;
    let candidate_root = canonical_directory(candidate_root, "candidate pack")?;
    ensure_disjoint(&live_root, &candidate_root)?;
    let output = new_output_path(out)?;
    if output.starts_with(&live_root) || output.starts_with(&candidate_root) {
        return Err(anyhow!(
            "author preview output must remain outside the live and candidate packs"
        ));
    }

    let validation = validation_summary(&candidate_root);
    if !validation.valid {
        return Ok(result_value(
            None,
            "refused",
            false,
            &[],
            &[],
            &[],
            &[],
            &[".mdp".to_string()],
            &[],
            &["candidate-validation-failed"],
            Some(validation),
        ));
    }

    let live = inventory(&live_root)?;
    let candidate = inventory(&candidate_root)?;
    let files = plan_files(&live, &candidate);
    let core = ChangeSetCore {
        contract: CHANGE_SET_CONTRACT.to_string(),
        live_root_sha256: root_binding(&live_root),
        validation,
        files,
    };
    let change_set_sha256 = sha256_hex(&canonical_json_bytes(&serde_json::to_value(&core)?)?);
    let change_set = ChangeSet {
        contract: core.contract,
        live_root_sha256: core.live_root_sha256,
        validation: core.validation,
        files: core.files,
        change_set_sha256,
    };
    write_new_json(&output, &change_set)?;
    Ok(result_from_change_set(
        &change_set,
        "previewed",
        true,
        &[],
        &[],
        &[],
        Some(output.display().to_string()),
    ))
}

pub(crate) fn apply_pack_change_set(
    live_root: &Path,
    candidate_root: &Path,
    change_set_path: &Path,
) -> Result<Value> {
    let live_root = canonical_directory(live_root, "live pack")?;
    let candidate_root = canonical_directory(candidate_root, "candidate pack")?;
    ensure_disjoint(&live_root, &candidate_root)?;
    let change_set = read_change_set(change_set_path)?;
    change_set.verify()?;

    if change_set.live_root_sha256 != root_binding(&live_root) {
        return Ok(result_from_change_set(
            &change_set,
            "refused",
            false,
            &[".mdp".to_string()],
            &[],
            &["live-root-mismatch"],
            None,
        ));
    }

    let validation = validation_summary(&candidate_root);
    if !validation.valid {
        return Ok(result_value(
            Some(&change_set),
            "refused",
            false,
            &paths_for(&change_set, "create"),
            &paths_for(&change_set, "change"),
            &paths_for(&change_set, "unchanged"),
            &paths_for(&change_set, "delete"),
            &[".mdp".to_string()],
            &[],
            &["candidate-validation-failed"],
            Some(validation),
        ));
    }

    let candidate = inventory(&candidate_root)?;
    let expected_candidate = candidate_inventory(&change_set);
    let candidate_conflicts = inventory_conflicts(&expected_candidate, &candidate);
    if !candidate_conflicts.is_empty() {
        return Ok(result_from_change_set(
            &change_set,
            "refused",
            false,
            &candidate_conflicts,
            &[],
            &["candidate-changed-after-preview"],
            None,
        ));
    }

    let _lock = match acquire_lock(&live_root)? {
        Some(lock) => lock,
        None => {
            return Ok(result_from_change_set(
                &change_set,
                "refused",
                false,
                &[".mdp".to_string()],
                &[],
                &["authoring-lock-active"],
                None,
            ));
        }
    };

    let live = inventory(&live_root)?;
    let expected_live = expected_inventory(&change_set);
    let live_conflicts = inventory_conflicts(&expected_live, &live);
    if !live_conflicts.is_empty() {
        return Ok(result_from_change_set(
            &change_set,
            "refused",
            false,
            &live_conflicts,
            &[],
            &["live-pack-changed-after-preview"],
            None,
        ));
    }

    match publish(&live_root, &candidate_root, &change_set, &candidate) {
        Ok(()) => Ok(result_from_change_set(
            &change_set,
            "applied",
            true,
            &[],
            &[],
            &[],
            None,
        )),
        Err(PublicationFailure::RolledBack { paths }) => Ok(result_from_change_set(
            &change_set,
            "rolled-back",
            false,
            &[],
            &paths,
            &["publication-failed-rolled-back"],
            None,
        )),
        Err(PublicationFailure::Indeterminate { message }) => Err(anyhow!(message)),
    }
}

enum PublicationFailure {
    RolledBack { paths: Vec<String> },
    Indeterminate { message: String },
}

fn publish(
    live_root: &Path,
    candidate_root: &Path,
    change_set: &ChangeSet,
    candidate_inventory: &BTreeMap<String, FileState>,
) -> std::result::Result<(), PublicationFailure> {
    let nonce = nonce();
    let parent = live_root.parent().unwrap_or(live_root);
    let staging_root = parent.join(format!(".mdp.author.staging.{nonce}"));
    let backup_root = parent.join(format!(".mdp.author.backup.{nonce}"));
    let mut backups = Vec::new();
    let mut installed = Vec::new();
    let mut created_directories = Vec::new();
    let mut touched = BTreeSet::new();
    let mut fault = BoundaryFault::from_env();

    let publication = (|| -> Result<()> {
        fs::create_dir(&staging_root)
            .with_context(|| format!("creating author staging root {}", staging_root.display()))?;
        fault.crossed()?;
        fs::create_dir(&backup_root)
            .with_context(|| format!("creating author backup root {}", backup_root.display()))?;
        fault.crossed()?;

        for file in &change_set.files {
            if !file.candidate.present || file.action == "unchanged" {
                continue;
            }
            let logical = logical_from_path(&file.path)?;
            let source = candidate_root.join(".mdp").join(&logical);
            let bytes = read_regular_file(&source)?;
            if sha256_hex(&bytes) != file.candidate.sha256.as_deref().unwrap_or_default() {
                return Err(anyhow!("candidate changed while being staged"));
            }
            let staged = staging_root.join(".mdp").join(&logical);
            if let Some(parent) = staged.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&staged, bytes)?;
            fault.crossed()?;
        }

        for file in &change_set.files {
            if !matches!(file.action.as_str(), "change" | "delete") {
                continue;
            }
            let logical = logical_from_path(&file.path)?;
            let live = live_root.join(".mdp").join(&logical);
            let backup = backup_root.join(".mdp").join(&logical);
            if let Some(parent) = backup.parent() {
                fs::create_dir_all(parent)?;
            }
            let state = state_for_path(&live)?;
            if state != file.expected {
                return Err(anyhow!("live pack changed during publication preflight"));
            }
            fs::rename(&live, &backup).with_context(|| format!("backing up {}", file.path))?;
            backups.push(PublishedBackup {
                live,
                backup,
                relative: file.path.clone(),
            });
            if state_for_path(&backups.last().expect("backup was recorded").backup)?
                != file.expected
            {
                return Err(anyhow!("live pack changed while being backed up"));
            }
            touched.insert(file.path.clone());
            fault.crossed()?;
        }

        for file in &change_set.files {
            if !matches!(file.action.as_str(), "create" | "change") {
                continue;
            }
            let logical = logical_from_path(&file.path)?;
            let target = live_root.join(".mdp").join(&logical);
            ensure_live_parent(
                live_root,
                target
                    .parent()
                    .ok_or_else(|| anyhow!("invalid author target"))?,
                &mut created_directories,
                &mut fault,
            )?;
            let staged = staging_root.join(".mdp").join(&logical);
            let mut source = File::open(&staged)?;
            let mut target_file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .with_context(|| format!("publishing {} without overwrite", file.path))?;
            installed.push(PublishedInstall {
                live: target.clone(),
                relative: file.path.clone(),
                expected: file.candidate.clone(),
            });
            std::io::copy(&mut source, &mut target_file)?;
            target_file.sync_all()?;
            touched.insert(file.path.clone());
            fault.crossed()?;
        }

        let candidate_after = inventory(candidate_root)?;
        if &candidate_after != candidate_inventory {
            return Err(anyhow!("candidate changed during publication"));
        }
        let published = inventory(live_root)?;
        if &published != candidate_inventory {
            return Err(anyhow!(
                "published pack does not match the staged candidate"
            ));
        }
        Ok(())
    })();

    if let Err(error) = publication {
        let mut rollback_error = None;
        for install in installed.iter().rev() {
            match state_for_path(&install.live) {
                Ok(state) if state == install.expected => {
                    if let Err(remove) = fs::remove_file(&install.live) {
                        rollback_error.get_or_insert_with(|| {
                            format!("rollback failed to remove {}: {remove}", install.relative)
                        });
                    }
                }
                Ok(state) if !state.present => {}
                Ok(_) => {
                    rollback_error.get_or_insert_with(|| {
                        format!(
                            "rollback preserved a concurrent edit at {}",
                            install.relative
                        )
                    });
                }
                Err(inspect) => {
                    rollback_error.get_or_insert_with(|| {
                        format!("rollback could not inspect {}: {inspect}", install.relative)
                    });
                }
            }
        }
        for backup in backups.iter().rev() {
            match fs::symlink_metadata(&backup.live) {
                Ok(_) => {
                    rollback_error.get_or_insert_with(|| {
                        format!("rollback refused an unexpected node at {}", backup.relative)
                    });
                    continue;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(inspect) => {
                    rollback_error.get_or_insert_with(|| {
                        format!(
                            "rollback could not inspect {} before restore: {inspect}",
                            backup.relative
                        )
                    });
                    continue;
                }
            }
            if let Err(restore) = fs::rename(&backup.backup, &backup.live) {
                rollback_error.get_or_insert_with(|| {
                    format!("rollback failed for {}: {}", backup.relative, restore)
                });
            }
        }
        remove_created_directories(&created_directories);
        let _ = fs::remove_dir_all(&staging_root);
        if let Some(rollback_error) = rollback_error {
            return Err(PublicationFailure::Indeterminate {
                message: format!(
                    "author publication indeterminate; recovery backup retained at {} ({rollback_error}; publication error: {error})",
                    backup_root.display()
                ),
            });
        }
        let _ = fs::remove_dir_all(&backup_root);
        return Err(PublicationFailure::RolledBack {
            paths: touched.into_iter().collect(),
        });
    }

    if let Err(error) = fs::remove_dir_all(&backup_root) {
        return Err(PublicationFailure::Indeterminate {
            message: format!(
                "author publication applied but recovery backup cleanup failed at {}: {error}",
                backup_root.display()
            ),
        });
    }
    if let Err(error) = fs::remove_dir_all(&staging_root) {
        return Err(PublicationFailure::Indeterminate {
            message: format!(
                "author publication applied but staging cleanup failed at {}: {error}",
                staging_root.display()
            ),
        });
    }
    Ok(())
}

fn inventory(root: &Path) -> Result<BTreeMap<String, FileState>> {
    let snapshot = pack_content_snapshot(root)?;
    if snapshot.files.len() > MAX_MANAGED_FILES {
        return Err(anyhow!("pack exceeds managed file limit"));
    }
    let mut total = 0u64;
    let mut result = BTreeMap::new();
    for record in snapshot.files {
        if record.byte_count > MAX_MANAGED_FILE_BYTES {
            return Err(anyhow!("pack file exceeds managed byte limit"));
        }
        total = total
            .checked_add(record.byte_count)
            .ok_or_else(|| anyhow!("pack byte count overflow"))?;
        if total > MAX_MANAGED_TOTAL_BYTES {
            return Err(anyhow!("pack exceeds managed total byte limit"));
        }
        result.insert(
            format!(".mdp/{}", record.logical_path),
            FileState {
                present: true,
                sha256: Some(record.sha256),
                bytes: Some(record.byte_count),
            },
        );
    }
    Ok(result)
}

fn plan_files(
    live: &BTreeMap<String, FileState>,
    candidate: &BTreeMap<String, FileState>,
) -> Vec<PlannedFile> {
    live.keys()
        .chain(candidate.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|path| {
            let expected = live.get(&path).cloned().unwrap_or_else(FileState::absent);
            let candidate = candidate
                .get(&path)
                .cloned()
                .unwrap_or_else(FileState::absent);
            PlannedFile {
                action: action_for(&expected, &candidate).to_string(),
                path,
                expected,
                candidate,
            }
        })
        .collect()
}

fn action_for(expected: &FileState, candidate: &FileState) -> &'static str {
    match (expected.present, candidate.present, expected == candidate) {
        (false, true, _) => "create",
        (true, false, _) => "delete",
        (true, true, true) => "unchanged",
        (true, true, false) => "change",
        (false, false, _) => "unchanged",
    }
}

fn expected_inventory(change_set: &ChangeSet) -> BTreeMap<String, FileState> {
    change_set
        .files
        .iter()
        .filter(|file| file.expected.present)
        .map(|file| (file.path.clone(), file.expected.clone()))
        .collect()
}

fn candidate_inventory(change_set: &ChangeSet) -> BTreeMap<String, FileState> {
    change_set
        .files
        .iter()
        .filter(|file| file.candidate.present)
        .map(|file| (file.path.clone(), file.candidate.clone()))
        .collect()
}

fn inventory_conflicts(
    expected: &BTreeMap<String, FileState>,
    actual: &BTreeMap<String, FileState>,
) -> Vec<String> {
    expected
        .keys()
        .chain(actual.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|path| expected.get(path) != actual.get(path))
        .collect()
}

fn validation_summary(root: &Path) -> ValidationSummary {
    match validate_pack(root) {
        Ok(value) => ValidationSummary {
            valid: value["valid"].as_bool().unwrap_or(false),
            error_count: value["error_count"].as_u64().unwrap_or(1),
            warning_count: value["warning_count"].as_u64().unwrap_or(0),
            diagnostics: value["issues"]
                .as_array()
                .into_iter()
                .flatten()
                .take(256)
                .map(|item| ValidationDiagnostic {
                    code: item["code"]
                        .as_str()
                        .unwrap_or("candidate-invalid")
                        .to_string(),
                    severity: item["severity"].as_str().unwrap_or("error").to_string(),
                    path: item["path"].as_str().unwrap_or(".mdp").to_string(),
                })
                .collect(),
        },
        Err(_) => ValidationSummary {
            valid: false,
            error_count: 1,
            warning_count: 0,
            diagnostics: vec![ValidationDiagnostic {
                code: "candidate-validation-failed".to_string(),
                severity: "error".to_string(),
                path: ".mdp".to_string(),
            }],
        },
    }
}

fn read_change_set(path: &Path) -> Result<ChangeSet> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("reading pack change set metadata {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(anyhow!(
            "pack change set must be a regular non-symlink file"
        ));
    }
    if metadata.len() > MAX_CHANGE_SET_BYTES {
        return Err(anyhow!("pack change set exceeds byte limit"));
    }
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes).context("parsing pack change set")
}

fn write_new_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("creating new pack change set {}", path.display()))?;
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("reading candidate metadata {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(anyhow!(
            "candidate authority must be a regular non-symlink file"
        ));
    }
    if metadata.len() > MAX_MANAGED_FILE_BYTES {
        return Err(anyhow!("candidate authority exceeds managed byte limit"));
    }
    let file = File::open(path)?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_MANAGED_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANAGED_FILE_BYTES {
        return Err(anyhow!("candidate authority exceeds managed byte limit"));
    }
    Ok(bytes)
}

fn state_for_path(path: &Path) -> Result<FileState> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(anyhow!(
                    "live authority must remain a regular non-symlink file"
                ));
            }
            let bytes = fs::read(path)?;
            Ok(FileState {
                present: true,
                sha256: Some(sha256_hex(&bytes)),
                bytes: Some(bytes.len() as u64),
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(FileState::absent()),
        Err(error) => Err(error.into()),
    }
}

fn ensure_live_parent(
    live_root: &Path,
    parent: &Path,
    created: &mut Vec<PathBuf>,
    fault: &mut BoundaryFault,
) -> Result<()> {
    let mut missing = Vec::new();
    let mut current = parent;
    while !current.exists() {
        if !current.starts_with(live_root) {
            return Err(anyhow!("author target escaped the live pack"));
        }
        missing.push(current.to_path_buf());
        current = current
            .parent()
            .ok_or_else(|| anyhow!("author target has no existing ancestor"))?;
    }
    let metadata = fs::symlink_metadata(current)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(anyhow!("author target ancestor is not a safe directory"));
    }
    for directory in missing.into_iter().rev() {
        fs::create_dir(&directory)?;
        created.push(directory);
        fault.crossed()?;
    }
    Ok(())
}

fn remove_created_directories(created: &[PathBuf]) {
    for directory in created.iter().rev() {
        let _ = fs::remove_dir(directory);
    }
}

fn acquire_lock(live_root: &Path) -> Result<Option<AuthorLock>> {
    let binding = root_binding(live_root);
    let parent = live_root.parent().unwrap_or(live_root);
    let path = parent.join(format!(".mdp.author.lock.{}", &binding[..16]));
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(b"mdp author transaction\n")?;
            file.sync_all()?;
            Ok(Some(AuthorLock { path }))
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
        Err(error) => {
            Err(error).with_context(|| format!("creating author lock {}", path.display()))
        }
    }
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path)
        .with_context(|| format!("resolving {label} directory {}", path.display()))?;
    let metadata = fs::symlink_metadata(&canonical)?;
    if !metadata.is_dir() {
        return Err(anyhow!("{label} must be a directory"));
    }
    Ok(canonical)
}

fn ensure_disjoint(live: &Path, candidate: &Path) -> Result<()> {
    if live == candidate || live.starts_with(candidate) || candidate.starts_with(live) {
        return Err(anyhow!(
            "candidate pack must be outside and disjoint from the live pack"
        ));
    }
    Ok(())
}

fn new_output_path(path: &Path) -> Result<PathBuf> {
    if path.file_name().is_none() {
        return Err(anyhow!("pack change-set output must name a file"));
    }
    if fs::symlink_metadata(path).is_ok() {
        return Err(anyhow!("pack change-set output must not already exist"));
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let parent = absolute
        .parent()
        .ok_or_else(|| anyhow!("pack change-set output has no parent"))?;
    let parent = fs::canonicalize(parent)
        .with_context(|| format!("resolving output parent {}", parent.display()))?;
    Ok(parent.join(
        absolute
            .file_name()
            .ok_or_else(|| anyhow!("pack change-set output must name a file"))?,
    ))
}

fn root_binding(root: &Path) -> String {
    sha256_hex(root.as_os_str().to_string_lossy().as_bytes())
}

fn logical_from_path(path: &str) -> Result<PathBuf> {
    validate_logical_path(path)?;
    Ok(PathBuf::from(path.trim_start_matches(".mdp/")))
}

fn validate_logical_path(path: &str) -> Result<()> {
    if !path.is_ascii() || !path.starts_with(".mdp/") || path.ends_with('/') {
        return Err(anyhow!("invalid pack change-set logical path"));
    }
    let relative = Path::new(path.trim_start_matches(".mdp/"));
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(anyhow!("unsafe pack change-set logical path"));
    }
    let first = relative
        .components()
        .next()
        .and_then(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            _ => None,
        });
    if matches!(first, Some("briefs" | "traces")) {
        return Err(anyhow!("runtime output paths are not author-managed"));
    }
    Ok(())
}

fn validate_state(state: &FileState) -> Result<()> {
    match (state.present, state.sha256.as_deref(), state.bytes) {
        (false, None, None) => Ok(()),
        (true, Some(hash), Some(bytes)) if bytes <= MAX_MANAGED_FILE_BYTES => {
            validate_hash(hash, "file digest")
        }
        _ => Err(anyhow!("invalid file state in pack change set")),
    }
}

fn validate_hash(value: &str, label: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(anyhow!("{label} must be a lowercase SHA-256 digest"));
    }
    if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(anyhow!("{label} must be lowercase"));
    }
    Ok(())
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn paths_for(change_set: &ChangeSet, action: &str) -> Vec<String> {
    change_set
        .files
        .iter()
        .filter(|file| file.action == action)
        .map(|file| file.path.clone())
        .collect()
}

fn result_from_change_set(
    change_set: &ChangeSet,
    status: &str,
    valid: bool,
    refused: &[String],
    rolled_back: &[String],
    reason_codes: &[&str],
    change_set_path: Option<String>,
) -> Value {
    let mut result = result_value(
        Some(change_set),
        status,
        valid,
        &paths_for(change_set, "create"),
        &paths_for(change_set, "change"),
        &paths_for(change_set, "unchanged"),
        &paths_for(change_set, "delete"),
        refused,
        rolled_back,
        reason_codes,
        Some(change_set.validation.clone()),
    );
    if let Some(path) = change_set_path {
        result["change_set_path"] = json!(path);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn result_value(
    change_set: Option<&ChangeSet>,
    status: &str,
    valid: bool,
    created: &[String],
    changed: &[String],
    unchanged: &[String],
    deleted: &[String],
    refused: &[String],
    rolled_back: &[String],
    reason_codes: &[&str],
    validation: Option<ValidationSummary>,
) -> Value {
    json!({
        "contract": PACK_AUTHORING_RESULT_V1,
        "status": status,
        "valid": valid,
        "change_set_sha256": change_set.map(|value| value.change_set_sha256.clone()),
        "validation": validation,
        "created": created,
        "changed": changed,
        "unchanged": unchanged,
        "deleted": deleted,
        "refused": refused,
        "rolled_back": rolled_back,
        "reason_codes": reason_codes,
        "private_content_included": false,
    })
}
