//! Failure-safe publication for multi-file pack authoring.
//!
//! A candidate is a complete pack tree outside the live pack. `preview` validates
//! that tree with the normal pack validator, seals the expected live and staged
//! file hashes into a bounded change set, and does not touch the live tree.
//! `apply` revalidates both sides, refuses drift, then publishes only `.mdp`
//! authority files through a rollback-protected transaction. Runtime output
//! directories (`.mdp/briefs` and `.mdp/traces`) are excluded by the shared
//! portable-pack snapshot contract and are never changed here.

use crate::artifact_hash::{canonical_json_bytes, sha256_hex};
use crate::commands::health::validate_pack;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::fd::{AsRawFd, FromRawFd};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

const CHANGE_SET_CONTRACT: &str = "mdp.pack-change-set.v1";
pub(crate) const PACK_AUTHORING_RESULT_V1: &str = "mdp.pack-authoring-result.v1";
const MAX_CHANGE_SET_BYTES: u64 = 2_097_152;
const MAX_MANAGED_FILES: usize = 2_048;
const MAX_MANAGED_FILE_BYTES: u64 = 8_388_608;
const MAX_MANAGED_TOTAL_BYTES: u64 = 67_108_864;

struct InventorySnapshot {
    states: BTreeMap<String, FileState>,
    bytes: BTreeMap<String, Vec<u8>>,
}

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
        if serde_json::to_vec(self)?.len() as u64 + 1 > MAX_CHANGE_SET_BYTES {
            return Err(anyhow!("pack change set exceeds byte limit"));
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
    #[allow(dead_code)]
    file: File,
}

struct BoundaryFault {
    after: Option<usize>,
    crash_after: Option<usize>,
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
        let crash_after = if cfg!(debug_assertions) {
            std::env::var("MDP_TEST_AUTHOR_CRASH_AFTER")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| *value > 0)
        } else {
            None
        };
        Self {
            after,
            crash_after,
            count: 0,
        }
    }

    fn crossed(&mut self) -> Result<()> {
        self.count += 1;
        if self.crash_after == Some(self.count) {
            std::process::abort();
        }
        if self.after == Some(self.count) {
            return Err(anyhow!("test fault after author publication boundary"));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TransactionState {
    contract: String,
    phase: u8,
    live_root_sha256: String,
    live_root_dev: u64,
    live_root_ino: u64,
    parent_dev: u64,
    parent_ino: u64,
    staging_name: String,
    backup_name: String,
    created_directories: Vec<String>,
    backups: Vec<PublishedBackupState>,
    installs: Vec<PublishedInstallState>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PublishedBackupState {
    path: String,
    dev: u64,
    ino: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PublishedInstallState {
    path: String,
    dev: u64,
    ino: u64,
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

    let captured_candidate = capture_inventory(&candidate_root)?;
    let validation = validation_summary_for_snapshot(&captured_candidate)?;
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
    let files = plan_files(&live, &captured_candidate.states);
    if files.len() > MAX_MANAGED_FILES {
        return Err(anyhow!("pack change set exceeds managed file limit"));
    }
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
    change_set.verify()?;
    let serialized = serde_json::to_vec_pretty(&change_set)?;
    if serialized.len() as u64 + 1 > MAX_CHANGE_SET_BYTES {
        return Err(anyhow!("pack change set exceeds byte limit"));
    }
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
    let initial_live_identity = file_identity(&open_verified_directory(&live_root)?)?;
    let initial_parent_identity = file_identity(&open_verified_directory(
        live_root.parent().unwrap_or(&live_root),
    )?)?;
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
    let recovered = recover_pending_transaction(&live_root)?;

    let captured_candidate = capture_inventory(&candidate_root)?;
    let validation = validation_summary_for_snapshot(&captured_candidate)?;
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

    let expected_candidate = candidate_inventory(&change_set);
    let candidate_conflicts = inventory_conflicts(&expected_candidate, &captured_candidate.states);
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

    if recovered == RecoveryOutcome::Committed && inventory(&live_root)? == expected_candidate {
        return Ok(result_from_change_set(
            &change_set,
            "applied",
            true,
            &[],
            &[],
            &["interrupted-commit-recovered"],
            None,
        ));
    }

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

    match publish(
        &live_root,
        &change_set,
        &captured_candidate,
        initial_live_identity,
        initial_parent_identity,
    ) {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecoveryOutcome {
    None,
    RolledBack,
    Committed,
}

fn publish(
    live_root: &Path,
    change_set: &ChangeSet,
    candidate: &InventorySnapshot,
    expected_live_identity: (u64, u64),
    expected_parent_identity: (u64, u64),
) -> std::result::Result<(), PublicationFailure> {
    let nonce = nonce();
    let staging_name = format!(".mdp.author.staging.{nonce}");
    let backup_name = format!(".mdp.author.backup.{nonce}");
    let mut touched = BTreeSet::new();
    let mut fault = BoundaryFault::from_env();
    let publication = (|| -> Result<()> {
        let parent = open_verified_directory(live_root.parent().unwrap_or(live_root))?;
        if file_identity(&parent)? != expected_parent_identity {
            return Err(anyhow!(
                "live pack parent identity changed before publication"
            ));
        }
        create_child_directory(&parent, &staging_name)?;
        fault.crossed()?;
        create_child_directory(&parent, &backup_name)?;
        fault.crossed()?;
        let staging = open_child_directory(&parent, &staging_name)?;
        let backup = open_child_directory(&parent, &backup_name)?;
        let root = open_verified_directory(live_root)?;
        if file_identity(&root)? != expected_live_identity {
            return Err(anyhow!("live pack identity changed before publication"));
        }
        let live = open_child_directory(&root, ".mdp")?;
        let mut backup_states = Vec::new();
        let mut install_states = Vec::new();
        let mut created_directories = BTreeSet::new();

        for file in &change_set.files {
            if !file.candidate.present || file.action == "unchanged" {
                continue;
            }
            let bytes = candidate
                .bytes
                .get(&file.path)
                .ok_or_else(|| anyhow!("candidate snapshot omitted managed authority"))?;
            let identity = write_new_logical(&staging, &file.path, bytes)?;
            install_states.push(PublishedInstallState {
                path: file.path.clone(),
                dev: identity.0,
                ino: identity.1,
            });
            created_directories.extend(missing_logical_parents(&live, &file.path)?);
            fault.crossed()?;
        }
        for file in &change_set.files {
            if !matches!(file.action.as_str(), "change" | "delete") {
                continue;
            }
            let (state, identity) = state_and_identity(&live, &file.path)?;
            if state != file.expected {
                return Err(anyhow!("live pack changed during publication preflight"));
            }
            let (dev, ino) =
                identity.ok_or_else(|| anyhow!("expected live authority is absent"))?;
            backup_states.push(PublishedBackupState {
                path: file.path.clone(),
                dev,
                ino,
            });
        }
        let mut transaction = TransactionState {
            contract: "mdp.pack-authoring-transaction.v1".to_string(),
            phase: 0,
            live_root_sha256: root_binding(live_root),
            live_root_dev: file_identity(&root)?.0,
            live_root_ino: file_identity(&root)?.1,
            parent_dev: file_identity(&parent)?.0,
            parent_ino: file_identity(&parent)?.1,
            staging_name: staging_name.clone(),
            backup_name: backup_name.clone(),
            created_directories: created_directories.into_iter().collect(),
            backups: backup_states,
            installs: install_states,
        };
        write_transaction_state(&parent, live_root, &transaction)?;
        test_pause("MDP_TEST_AUTHOR_PUBLICATION_MARKER")?;
        fault.crossed()?;
        for moved in &transaction.backups {
            rename_logical_no_replace(&live, &backup, &moved.path, true)?;
            touched.insert(moved.path.clone());
            let planned = change_set
                .files
                .iter()
                .find(|file| file.path == moved.path)
                .ok_or_else(|| anyhow!("backup authority is absent from the sealed plan"))?;
            let (backed_up, identity) = state_and_identity(&backup, &moved.path)?;
            if identity != Some((moved.dev, moved.ino)) {
                return Err(anyhow!(
                    "live authority identity changed while being backed up"
                ));
            }
            if backed_up != planned.expected {
                return Err(anyhow!(
                    "live authority content changed while being backed up"
                ));
            }
            fault.crossed()?;
        }
        for installed in &transaction.installs {
            rename_logical_no_replace(&staging, &live, &installed.path, true)?;
            if logical_identity(&live, &installed.path)? != Some((installed.dev, installed.ino)) {
                return Err(anyhow!("published authority identity mismatch"));
            }
            touched.insert(installed.path.clone());
            fault.crossed()?;
        }
        let published = capture_directory_snapshot(&live)?;
        if published.states != candidate.states {
            return Err(anyhow!(
                "published pack does not match the staged candidate"
            ));
        }
        mark_transaction_committed(&parent, live_root, &transaction)?;
        transaction.phase = 1;
        fault.crossed()?;
        cleanup_transaction(&parent, live_root, &transaction, false)?;
        Ok(())
    })();
    if let Err(error) = publication {
        if let Err(pause) = test_pause("MDP_TEST_AUTHOR_ROLLBACK_MARKER") {
            return Err(PublicationFailure::Indeterminate {
                message: format!("author rollback test handshake failed: {pause}"),
            });
        }
        return match recover_or_cleanup_transaction(live_root, &staging_name, &backup_name) {
            Ok(RecoveryOutcome::Committed) => Ok(()),
            Ok(_) => Err(PublicationFailure::RolledBack {
                paths: touched.into_iter().collect(),
            }),
            Err(rollback) => Err(PublicationFailure::Indeterminate {
                message: format!(
                    "author publication indeterminate; durable recovery state retained ({rollback}; publication error: {error})"
                ),
            }),
        };
    }
    Ok(())
}

#[cfg(unix)]
fn component_name(value: &str) -> Result<CString> {
    if value.is_empty() || value == "." || value == ".." || value.contains(['/', '\\']) {
        return Err(anyhow!("unsafe authoring path component"));
    }
    CString::new(value).map_err(|_| anyhow!("authoring path contains NUL"))
}

#[cfg(unix)]
fn file_identity(file: &File) -> Result<(u64, u64)> {
    let metadata = file.metadata()?;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(unix)]
fn open_verified_directory(path: &Path) -> Result<File> {
    let before = fs::symlink_metadata(path)?;
    if before.file_type().is_symlink() || !before.is_dir() {
        return Err(anyhow!("authoring directory must be a real directory"));
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let opened = file_identity(&file)?;
    let after = fs::symlink_metadata(path)?;
    if opened != (before.dev(), before.ino()) || opened != (after.dev(), after.ino()) {
        return Err(anyhow!(
            "authoring directory identity changed while opening"
        ));
    }
    Ok(file)
}

#[cfg(unix)]
fn create_child_directory(parent: &File, name: &str) -> Result<()> {
    let name = component_name(name)?;
    if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
        return Err(std::io::Error::last_os_error()).context("creating author workspace");
    }
    parent.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn open_child_directory(parent: &File, name: &str) -> Result<File> {
    let name = component_name(name)?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error()).context("opening author workspace component");
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(unix)]
fn open_optional_child_directory(parent: &File, name: &str) -> Result<Option<File>> {
    match open_child_directory(parent, name) {
        Ok(directory) => Ok(Some(directory)),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "linux")]
fn rename_no_replace_between(
    from_dir: &File,
    from: &CString,
    to_dir: &File,
    to: &CString,
) -> Result<()> {
    let status = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            from_dir.as_raw_fd(),
            from.as_ptr(),
            to_dir.as_raw_fd(),
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if status != 0 {
        return Err(std::io::Error::last_os_error())
            .context("moving author authority without replacement");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn rename_no_replace_between(
    from_dir: &File,
    from: &CString,
    to_dir: &File,
    to: &CString,
) -> Result<()> {
    const RENAME_EXCL: u32 = 0x00000004;
    let status = unsafe {
        libc::renameatx_np(
            from_dir.as_raw_fd(),
            from.as_ptr(),
            to_dir.as_raw_fd(),
            to.as_ptr(),
            RENAME_EXCL,
        )
    };
    if status != 0 {
        return Err(std::io::Error::last_os_error())
            .context("moving author authority without replacement");
    }
    Ok(())
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn rename_no_replace_between(_: &File, _: &CString, _: &File, _: &CString) -> Result<()> {
    Err(anyhow!(
        "transactional pack authoring is unsupported on this platform"
    ))
}

#[cfg(unix)]
fn logical_components(path: &str) -> Result<Vec<String>> {
    Ok(logical_from_path(path)?
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect())
}

#[cfg(unix)]
fn open_logical_parent(root: &File, path: &str, create: bool) -> Result<(File, CString)> {
    let components = logical_components(path)?;
    let (leaf, parents) = components
        .split_last()
        .ok_or_else(|| anyhow!("authoring path must name a file"))?;
    let mut directory = root.try_clone()?;
    for component in parents {
        let name = component_name(component)?;
        let mut fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0
            && create
            && std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound
        {
            if unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(error).context("creating author authority parent");
                }
            }
            directory.sync_all()?;
            fd = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
        }
        if fd < 0 {
            return Err(std::io::Error::last_os_error()).context("opening author authority parent");
        }
        directory = unsafe { File::from_raw_fd(fd) };
    }
    Ok((directory, component_name(leaf)?))
}

#[cfg(unix)]
fn missing_logical_parents(root: &File, path: &str) -> Result<Vec<String>> {
    let components = logical_components(path)?;
    let (_, parents) = components
        .split_last()
        .ok_or_else(|| anyhow!("authoring path must name a file"))?;
    let mut directory = root.try_clone()?;
    let mut logical = Vec::new();
    let mut missing = Vec::new();
    let mut absent = false;
    for component in parents {
        logical.push(component.clone());
        if absent {
            missing.push(format!(".mdp/{}", logical.join("/")));
            continue;
        }
        let name = component_name(component)?;
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd >= 0 {
            directory = unsafe { File::from_raw_fd(fd) };
            continue;
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(error).context("inspecting author authority parent");
        }
        absent = true;
        missing.push(format!(".mdp/{}", logical.join("/")));
    }
    Ok(missing)
}

#[cfg(unix)]
fn remove_recorded_directories(root: &File, directories: &[String]) -> Result<()> {
    let mut directories = directories.to_vec();
    directories.sort_by_key(|path| std::cmp::Reverse(path.matches('/').count()));
    for path in directories {
        let (parent, leaf) = match open_logical_parent(root, &path, false) {
            Ok(value) => value,
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        };
        if unsafe { libc::unlinkat(parent.as_raw_fd(), leaf.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(error).context("removing rollback-created author directory");
            }
        } else {
            parent.sync_all()?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn named_identity(parent: &File, leaf: &CString) -> Result<Option<(u64, u64, u32)>> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            leaf.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(error).context("inspecting author authority leaf");
    }
    let stat = unsafe { stat.assume_init() };
    Ok(Some((
        stat.st_dev as u64,
        stat.st_ino as u64,
        stat.st_mode as u32,
    )))
}

#[cfg(unix)]
fn logical_identity(root: &File, path: &str) -> Result<Option<(u64, u64)>> {
    let (parent, leaf) = match open_logical_parent(root, path, false) {
        Ok(value) => value,
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    match named_identity(&parent, &leaf)? {
        None => Ok(None),
        Some((dev, ino, mode)) if mode & (libc::S_IFMT as u32) == libc::S_IFREG as u32 => {
            Ok(Some((dev, ino)))
        }
        Some(_) => Err(anyhow!("author authority leaf is not a regular file")),
    }
}

#[cfg(unix)]
fn state_and_identity(root: &File, path: &str) -> Result<(FileState, Option<(u64, u64)>)> {
    let (parent, leaf) = open_logical_parent(root, path, false)?;
    let Some((dev, ino, mode)) = named_identity(&parent, &leaf)? else {
        return Ok((FileState::absent(), None));
    };
    if mode & (libc::S_IFMT as u32) != libc::S_IFREG as u32 {
        return Err(anyhow!("author authority leaf is not a regular file"));
    }
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            leaf.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error()).context("opening author authority leaf");
    }
    let file = unsafe { File::from_raw_fd(fd) };
    if file_identity(&file)? != (dev, ino) {
        return Err(anyhow!("author authority identity changed while reading"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_MANAGED_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MANAGED_FILE_BYTES {
        return Err(anyhow!("author authority exceeds managed byte limit"));
    }
    Ok((
        FileState {
            present: true,
            sha256: Some(sha256_hex(&bytes)),
            bytes: Some(bytes.len() as u64),
        },
        Some((dev, ino)),
    ))
}

#[cfg(unix)]
fn write_new_logical(root: &File, path: &str, bytes: &[u8]) -> Result<(u64, u64)> {
    let (parent, leaf) = open_logical_parent(root, path, true)?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            leaf.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error()).context("staging author authority");
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(bytes)?;
    file.sync_all()?;
    parent.sync_all()?;
    file_identity(&file)
}

#[cfg(unix)]
fn rename_logical_no_replace(from: &File, to: &File, path: &str, create_to: bool) -> Result<()> {
    let (from_parent, from_leaf) = open_logical_parent(from, path, false)?;
    let (to_parent, to_leaf) = open_logical_parent(to, path, create_to)?;
    rename_no_replace_between(&from_parent, &from_leaf, &to_parent, &to_leaf)?;
    from_parent.sync_all()?;
    to_parent.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn capture_directory_snapshot(directory: &File) -> Result<InventorySnapshot> {
    let mut snapshot = InventorySnapshot {
        states: BTreeMap::new(),
        bytes: BTreeMap::new(),
    };
    capture_directory(directory, Path::new(""), true, &mut snapshot)?;
    Ok(snapshot)
}

#[cfg(unix)]
fn capture_workspace_snapshot(directory: &File) -> Result<InventorySnapshot> {
    let mut snapshot = InventorySnapshot {
        states: BTreeMap::new(),
        bytes: BTreeMap::new(),
    };
    capture_directory(directory, Path::new(""), false, &mut snapshot)?;
    Ok(snapshot)
}

#[cfg(unix)]
fn state_leaf(live_root: &Path) -> String {
    format!(".mdp.author.state.{}", &root_binding(live_root)[..16])
}

#[cfg(unix)]
fn pending_state_leaf(live_root: &Path) -> String {
    format!(
        ".mdp.author.state-pending.{}",
        &root_binding(live_root)[..16]
    )
}

#[cfg(unix)]
fn write_transaction_state(
    parent: &File,
    live_root: &Path,
    state: &TransactionState,
) -> Result<()> {
    let mut bytes = serde_json::to_vec(state)?;
    bytes.push(b'\n');
    if bytes.len() as u64 > MAX_CHANGE_SET_BYTES {
        return Err(anyhow!("author transaction state exceeds byte limit"));
    }
    let leaf = component_name(&state_leaf(live_root))?;
    let pending_name = pending_state_leaf(live_root);
    let pending = component_name(&pending_name)?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            pending.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error())
            .context("creating pending durable author transaction state");
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(&bytes)?;
    file.sync_all()?;
    parent.sync_all()?;
    if cfg!(debug_assertions)
        && std::env::var_os("MDP_TEST_AUTHOR_CRASH_BEFORE_STATE_PUBLISH").is_some()
    {
        std::process::abort();
    }
    if let Err(error) = rename_no_replace_between(parent, &pending, parent, &leaf) {
        let _ = unsafe { libc::unlinkat(parent.as_raw_fd(), pending.as_ptr(), 0) };
        return Err(error).context("publishing durable author transaction state");
    }
    parent.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn read_transaction_state(
    parent: &File,
    live_root: &Path,
) -> Result<Option<(TransactionState, (u64, u64))>> {
    read_transaction_state_named(parent, &state_leaf(live_root))
}

#[cfg(unix)]
fn read_transaction_state_named(
    parent: &File,
    name: &str,
) -> Result<Option<(TransactionState, (u64, u64))>> {
    let leaf = component_name(name)?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            leaf.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(error).context("opening durable author transaction state");
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let identity = file_identity(&file)?;
    let mut bytes = Vec::new();
    file.take(MAX_CHANGE_SET_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CHANGE_SET_BYTES {
        return Err(anyhow!("author transaction state exceeds byte limit"));
    }
    let state: TransactionState = serde_json::from_slice(&bytes)?;
    Ok(Some((state, identity)))
}

#[cfg(unix)]
fn mark_transaction_committed(
    parent: &File,
    live_root: &Path,
    state: &TransactionState,
) -> Result<()> {
    if state.phase != 0 {
        return Err(anyhow!("author transaction is not in publishing phase"));
    }
    let leaf = component_name(&state_leaf(live_root))?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            leaf.as_ptr(),
            libc::O_RDWR | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error())
            .context("opening author transaction state for commit");
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    let expected_identity = read_transaction_state(parent, live_root)?
        .ok_or_else(|| anyhow!("author transaction state disappeared before commit"))?
        .1;
    if file_identity(&file)? != expected_identity {
        return Err(anyhow!(
            "author transaction state identity changed before commit"
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let marker = b"\"phase\":0";
    let offset = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .ok_or_else(|| anyhow!("author transaction phase marker is missing"))?
        + marker.len()
        - 1;
    let written = unsafe {
        libc::pwrite(
            file.as_raw_fd(),
            b"1".as_ptr().cast(),
            1,
            offset as libc::off_t,
        )
    };
    if written != 1 {
        return Err(std::io::Error::last_os_error()).context("committing author transaction state");
    }
    file.sync_all()?;
    parent.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn validate_transaction_state(
    state: &TransactionState,
    live_root: &Path,
    parent: &File,
    root: &File,
) -> Result<()> {
    let staging_suffix = state.staging_name.strip_prefix(".mdp.author.staging.");
    let backup_suffix = state.backup_name.strip_prefix(".mdp.author.backup.");
    if state.contract != "mdp.pack-authoring-transaction.v1"
        || state.phase > 1
        || state.live_root_sha256 != root_binding(live_root)
        || (state.live_root_dev, state.live_root_ino) != file_identity(root)?
        || (state.parent_dev, state.parent_ino) != file_identity(parent)?
        || !staging_suffix.is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
        })
        || !backup_suffix.is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
        })
    {
        return Err(anyhow!("invalid durable author transaction state"));
    }
    for item in &state.backups {
        validate_logical_path(&item.path)?;
    }
    for item in &state.installs {
        validate_logical_path(&item.path)?;
    }
    for directory in &state.created_directories {
        validate_logical_path(&format!("{directory}/placeholder"))?;
    }
    Ok(())
}

#[cfg(unix)]
fn remove_tree_contents(directory: &File) -> Result<()> {
    let entries = fs::read_dir(descriptor_path(directory))?.collect::<std::io::Result<Vec<_>>>()?;
    for entry in entries {
        let name = entry.file_name();
        let leaf =
            CString::new(name.as_bytes()).map_err(|_| anyhow!("workspace path contains NUL"))?;
        let Some((_, _, mode)) = named_identity(directory, &leaf)? else {
            continue;
        };
        if mode & (libc::S_IFMT as u32) == libc::S_IFDIR as u32 {
            let child = open_child_directory(
                directory,
                name.to_str()
                    .ok_or_else(|| anyhow!("workspace path is not UTF-8"))?,
            )?;
            remove_tree_contents(&child)?;
            if unsafe { libc::unlinkat(directory.as_raw_fd(), leaf.as_ptr(), libc::AT_REMOVEDIR) }
                != 0
            {
                return Err(std::io::Error::last_os_error())
                    .context("removing author workspace directory");
            }
        } else if unsafe { libc::unlinkat(directory.as_raw_fd(), leaf.as_ptr(), 0) } != 0 {
            return Err(std::io::Error::last_os_error()).context("removing author workspace file");
        }
    }
    directory.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn remove_workspace(parent: &File, name: &str, directory: &File) -> Result<()> {
    if cfg!(debug_assertions)
        && std::env::var("MDP_TEST_AUTHOR_CLEANUP_FAIL").is_ok_and(|kind| {
            (kind == "staging" && name.starts_with(".mdp.author.staging."))
                || (kind == "backup" && name.starts_with(".mdp.author.backup."))
        })
    {
        return Err(anyhow!("test cleanup refusal for {name}"));
    }
    remove_tree_contents(directory)?;
    let leaf = component_name(name)?;
    let expected = file_identity(directory)?;
    let named = named_identity(parent, &leaf)?.map(|(dev, ino, _)| (dev, ino));
    if named != Some(expected) {
        return Err(anyhow!("author workspace identity changed before cleanup"));
    }
    if unsafe { libc::unlinkat(parent.as_raw_fd(), leaf.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
        return Err(std::io::Error::last_os_error()).context("removing author workspace root");
    }
    parent.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn remove_named_identity(parent: &File, name: &str, expected: (u64, u64)) -> Result<()> {
    let leaf = component_name(name)?;
    let quarantine = component_name(&format!(".mdp.author.state-cleanup.{}", nonce()))?;
    if named_identity(parent, &leaf)?.map(|(dev, ino, _)| (dev, ino)) != Some(expected) {
        return Err(anyhow!(
            "author transaction state identity changed before cleanup"
        ));
    }
    rename_no_replace_between(parent, &leaf, parent, &quarantine)?;
    if named_identity(parent, &quarantine)?.map(|(dev, ino, _)| (dev, ino)) != Some(expected) {
        let _ = rename_no_replace_between(parent, &quarantine, parent, &leaf);
        return Err(anyhow!(
            "author transaction state identity changed during cleanup"
        ));
    }
    if unsafe { libc::unlinkat(parent.as_raw_fd(), quarantine.as_ptr(), 0) } != 0 {
        return Err(std::io::Error::last_os_error())
            .context("removing durable author transaction state");
    }
    parent.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn cleanup_transaction(
    parent: &File,
    live_root: &Path,
    state: &TransactionState,
    rollback: bool,
) -> Result<Vec<String>> {
    let root = open_verified_directory(live_root)?;
    validate_transaction_state(state, live_root, parent, &root)?;
    let staging = open_optional_child_directory(parent, &state.staging_name)?;
    let backup = open_optional_child_directory(parent, &state.backup_name)?;
    let live = open_child_directory(&root, ".mdp")?;
    let mut rolled_back = Vec::new();
    if rollback {
        for installed in state.installs.iter().rev() {
            match logical_identity(&live, &installed.path)? {
                None => {
                    if let Some(identity) = staging
                        .as_ref()
                        .map(|staging| logical_identity(staging, &installed.path))
                        .transpose()?
                        .flatten()
                        && identity != (installed.dev, installed.ino)
                    {
                        return Err(anyhow!(
                            "rollback retained an unrecognized quarantined authority at {}",
                            installed.path
                        ));
                    }
                }
                Some(identity) if identity == (installed.dev, installed.ino) => {
                    let staging = staging.as_ref().ok_or_else(|| {
                        anyhow!("rollback staging workspace is absent while live authority remains")
                    })?;
                    rename_logical_no_replace(&live, staging, &installed.path, true)?;
                    if logical_identity(staging, &installed.path)?
                        != Some((installed.dev, installed.ino))
                    {
                        let _ = rename_logical_no_replace(staging, &live, &installed.path, true);
                        return Err(anyhow!(
                            "rollback quarantined a concurrent replacement at {}",
                            installed.path
                        ));
                    }
                    rolled_back.push(installed.path.clone());
                }
                Some(identity)
                    if state.backups.iter().any(|backup| {
                        backup.path == installed.path && identity == (backup.dev, backup.ino)
                    }) => {}
                Some(_) => {
                    return Err(anyhow!(
                        "rollback preserved a concurrent edit at {}",
                        installed.path
                    ));
                }
            }
        }
        for moved in state.backups.iter().rev() {
            let backup_identity = backup
                .as_ref()
                .map(|backup| logical_identity(backup, &moved.path))
                .transpose()?
                .flatten();
            match backup_identity {
                None => {
                    if logical_identity(&live, &moved.path)? != Some((moved.dev, moved.ino)) {
                        return Err(anyhow!(
                            "rollback cannot locate original authority {}",
                            moved.path
                        ));
                    }
                }
                Some(identity) if identity == (moved.dev, moved.ino) => {
                    let backup = backup.as_ref().ok_or_else(|| {
                        anyhow!("rollback backup workspace disappeared during recovery")
                    })?;
                    if logical_identity(&live, &moved.path)?.is_some() {
                        return Err(anyhow!(
                            "rollback refused to overwrite concurrent authority {}",
                            moved.path
                        ));
                    }
                    rename_logical_no_replace(backup, &live, &moved.path, true)?;
                    if logical_identity(&live, &moved.path)? != Some((moved.dev, moved.ino)) {
                        let _ = rename_logical_no_replace(&live, backup, &moved.path, true);
                        return Err(anyhow!(
                            "rollback moved an unexpected backup authority at {}",
                            moved.path
                        ));
                    }
                    rolled_back.push(moved.path.clone());
                }
                Some(_) => {
                    return Err(anyhow!(
                        "recovery backup identity mismatch at {}",
                        moved.path
                    ));
                }
            }
        }
        remove_recorded_directories(&live, &state.created_directories)?;
    }
    if let Some(staging) = staging.as_ref() {
        validate_workspace_contents(staging, &state.installs)?;
        remove_workspace(parent, &state.staging_name, staging)?;
    }
    if cfg!(debug_assertions)
        && std::env::var_os("MDP_TEST_AUTHOR_CRASH_AFTER_STAGING_CLEANUP").is_some()
    {
        std::process::abort();
    }
    if let Some(backup) = backup.as_ref() {
        validate_backup_contents(backup, &state.backups)?;
        remove_workspace(parent, &state.backup_name, backup)?;
    }
    let (_, state_identity) = read_transaction_state(parent, live_root)?
        .ok_or_else(|| anyhow!("durable author transaction state disappeared during cleanup"))?;
    remove_named_identity(parent, &state_leaf(live_root), state_identity)?;
    Ok(rolled_back)
}

#[cfg(unix)]
fn validate_workspace_contents(root: &File, expected: &[PublishedInstallState]) -> Result<()> {
    let snapshot = capture_workspace_snapshot(root)?;
    if snapshot.states.len() > expected.len() {
        return Err(anyhow!(
            "author staging workspace contains unrecognized files"
        ));
    }
    for path in snapshot.states.keys() {
        let item = expected
            .iter()
            .find(|item| &item.path == path)
            .ok_or_else(|| anyhow!("author staging workspace contains an unrecognized file"))?;
        if logical_identity(root, path)? != Some((item.dev, item.ino)) {
            return Err(anyhow!("author staging workspace identity mismatch"));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn validate_backup_contents(root: &File, expected: &[PublishedBackupState]) -> Result<()> {
    let snapshot = capture_workspace_snapshot(root)?;
    if snapshot.states.len() > expected.len() {
        return Err(anyhow!(
            "author backup workspace contains unrecognized files"
        ));
    }
    for path in snapshot.states.keys() {
        let item = expected
            .iter()
            .find(|item| &item.path == path)
            .ok_or_else(|| anyhow!("author backup workspace contains an unrecognized file"))?;
        if logical_identity(root, path)? != Some((item.dev, item.ino)) {
            return Err(anyhow!("author backup workspace identity mismatch"));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn recover_pending_transaction(live_root: &Path) -> Result<RecoveryOutcome> {
    let parent = open_verified_directory(live_root.parent().unwrap_or(live_root))?;
    if read_transaction_state(&parent, live_root)?.is_none() {
        let pending_name = pending_state_leaf(live_root);
        let pending_state = match read_transaction_state_named(&parent, &pending_name) {
            Ok(value) => value,
            Err(error) => {
                remove_incomplete_pending_state(&parent, &pending_name).with_context(|| {
                    format!("discarding incomplete pending transaction state after {error}")
                })?;
                None
            }
        };
        if let Some((pending_state, pending_identity)) = pending_state {
            let root = open_verified_directory(live_root)?;
            validate_transaction_state(&pending_state, live_root, &parent, &root)?;
            let pending = component_name(&pending_name)?;
            let final_leaf = component_name(&state_leaf(live_root))?;
            rename_no_replace_between(&parent, &pending, &parent, &final_leaf)
                .context("recovering atomically staged author transaction state")?;
            if named_identity(&parent, &final_leaf)?.map(|(dev, ino, _)| (dev, ino))
                != Some(pending_identity)
            {
                return Err(anyhow!(
                    "pending author transaction identity changed during recovery"
                ));
            }
            parent.sync_all()?;
        }
    }
    let Some((state, _)) = read_transaction_state(&parent, live_root)? else {
        return Ok(RecoveryOutcome::None);
    };
    let root = open_verified_directory(live_root)?;
    validate_transaction_state(&state, live_root, &parent, &root)?;
    let committed = state.phase == 1;
    cleanup_transaction(&parent, live_root, &state, !committed)?;
    Ok(if committed {
        RecoveryOutcome::Committed
    } else {
        RecoveryOutcome::RolledBack
    })
}

#[cfg(unix)]
fn recover_or_cleanup_transaction(
    live_root: &Path,
    staging_name: &str,
    backup_name: &str,
) -> Result<RecoveryOutcome> {
    let parent = open_verified_directory(live_root.parent().unwrap_or(live_root))?;
    if read_transaction_state(&parent, live_root)?.is_some() {
        return recover_pending_transaction(live_root);
    }
    for name in [staging_name, backup_name] {
        match open_child_directory(&parent, name) {
            Ok(directory) => remove_workspace(&parent, name, &directory)?,
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) => {}
            Err(error) => return Err(error),
        }
    }
    remove_incomplete_pending_state(&parent, &pending_state_leaf(live_root))?;
    Ok(RecoveryOutcome::None)
}

#[cfg(unix)]
fn remove_incomplete_pending_state(parent: &File, name: &str) -> Result<()> {
    let leaf = component_name(name)?;
    let Some((dev, ino, mode)) = named_identity(parent, &leaf)? else {
        return Ok(());
    };
    if mode & (libc::S_IFMT as u32) != libc::S_IFREG as u32 {
        return Err(anyhow!(
            "pending author transaction state is not a regular file"
        ));
    }
    remove_named_identity(parent, name, (dev, ino))
}

#[cfg(not(unix))]
fn recover_pending_transaction(_: &Path) -> Result<RecoveryOutcome> {
    Err(anyhow!(
        "identity-bound pack authoring is unsupported on this platform"
    ))
}

#[cfg(not(unix))]
fn recover_or_cleanup_transaction(_: &Path, _: &str, _: &str) -> Result<RecoveryOutcome> {
    Err(anyhow!(
        "identity-bound pack authoring is unsupported on this platform"
    ))
}

fn inventory(root: &Path) -> Result<BTreeMap<String, FileState>> {
    Ok(capture_inventory(root)?.states)
}

#[cfg(unix)]
fn capture_inventory(root: &Path) -> Result<InventorySnapshot> {
    let requested = root.to_path_buf();
    let root = open_verified_directory(root)?;
    if std::env::var_os("MDP_TEST_AUTHOR_PAUSE_ROOT")
        .is_some_and(|value| Path::new(&value) == requested)
    {
        test_pause("MDP_TEST_AUTHOR_ROOT_MARKER")?;
    }
    let pack = open_child_directory(&root, ".mdp")?;
    let mut snapshot = InventorySnapshot {
        states: BTreeMap::new(),
        bytes: BTreeMap::new(),
    };
    capture_directory(&pack, Path::new(""), true, &mut snapshot)?;
    Ok(snapshot)
}

#[cfg(unix)]
fn descriptor_path(file: &File) -> PathBuf {
    #[cfg(target_os = "linux")]
    let base = "/proc/self/fd";
    #[cfg(not(target_os = "linux"))]
    let base = "/dev/fd";
    Path::new(base).join(file.as_raw_fd().to_string())
}

#[cfg(unix)]
fn capture_directory(
    directory: &File,
    relative: &Path,
    top_level: bool,
    snapshot: &mut InventorySnapshot,
) -> Result<()> {
    let mut entries =
        fs::read_dir(descriptor_path(directory))?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        if top_level && matches!(name.to_str(), Some("briefs" | "traces")) {
            continue;
        }
        let name_c =
            CString::new(name.as_bytes()).map_err(|_| anyhow!("pack path contains NUL"))?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe {
            libc::fstatat(
                directory.as_raw_fd(),
                name_c.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error()).context("inspecting pack component");
        }
        let stat = unsafe { stat.assume_init() };
        if stat.st_mode & libc::S_IFMT == libc::S_IFLNK {
            return Err(anyhow!("author-managed pack does not allow symlinks"));
        }
        let child_relative = relative.join(&name);
        if stat.st_mode & libc::S_IFMT == libc::S_IFDIR {
            let fd = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name_c.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(std::io::Error::last_os_error())
                    .context("opening pack directory component");
            }
            let child = unsafe { File::from_raw_fd(fd) };
            capture_directory(&child, &child_relative, false, snapshot)?;
            continue;
        }
        if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
            return Err(anyhow!("author-managed pack only allows regular files"));
        }
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name_c.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error())
                .context("opening pack authority component");
        }
        let file = unsafe { File::from_raw_fd(fd) };
        let opened = file.metadata()?;
        if opened.dev() != stat.st_dev as u64 || opened.ino() != stat.st_ino as u64 {
            return Err(anyhow!("pack authority identity changed during capture"));
        }
        let mut bytes = Vec::new();
        file.take(MAX_MANAGED_FILE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_MANAGED_FILE_BYTES {
            return Err(anyhow!("pack file exceeds managed byte limit"));
        }
        let logical = child_relative
            .to_str()
            .ok_or_else(|| anyhow!("portable pack paths must be UTF-8"))?
            .replace('\\', "/");
        if !logical.is_ascii() {
            return Err(anyhow!("portable pack paths must be ASCII"));
        }
        let path = format!(".mdp/{logical}");
        if snapshot
            .states
            .keys()
            .any(|existing| existing.eq_ignore_ascii_case(&path))
        {
            return Err(anyhow!(
                "portable pack paths must not collide by ASCII case"
            ));
        }
        if snapshot.states.len() >= MAX_MANAGED_FILES {
            return Err(anyhow!("pack exceeds managed file limit"));
        }
        let total = snapshot
            .bytes
            .values()
            .try_fold(bytes.len() as u64, |sum, item| {
                sum.checked_add(item.len() as u64)
            })
            .ok_or_else(|| anyhow!("pack byte count overflow"))?;
        if total > MAX_MANAGED_TOTAL_BYTES {
            return Err(anyhow!("pack exceeds managed total byte limit"));
        }
        snapshot.states.insert(
            path.clone(),
            FileState {
                present: true,
                sha256: Some(sha256_hex(&bytes)),
                bytes: Some(bytes.len() as u64),
            },
        );
        snapshot.bytes.insert(path, bytes);
    }
    Ok(())
}

#[cfg(not(unix))]
fn capture_inventory(_root: &Path) -> Result<InventorySnapshot> {
    Err(anyhow!(
        "identity-bound pack authoring is unsupported on this platform"
    ))
}

fn validation_summary_for_snapshot(snapshot: &InventorySnapshot) -> Result<ValidationSummary> {
    let root = std::env::temp_dir().join(format!("mdp-author-validate-{}", secure_nonce()?));
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(&root)?;
    }
    #[cfg(not(unix))]
    fs::create_dir(&root)?;
    let materialized = (|| -> Result<ValidationSummary> {
        for (path, bytes) in &snapshot.bytes {
            let logical = logical_from_path(path)?;
            let target = root.join(".mdp").join(logical);
            let parent = target
                .parent()
                .ok_or_else(|| anyhow!("invalid snapshot path"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(parent)?;
            }
            #[cfg(not(unix))]
            fs::create_dir_all(parent)?;
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            options.mode(0o600);
            let mut file = options.open(target)?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        #[cfg(unix)]
        {
            if cfg!(debug_assertions)
                && let Some(marker) = std::env::var_os("MDP_TEST_AUTHOR_VALIDATION_MARKER")
            {
                let marker = PathBuf::from(marker);
                fs::write(&marker, root.as_os_str().as_bytes())?;
                let release = marker.with_extension("go");
                for _ in 0..500 {
                    if release.exists() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                if !release.exists() {
                    return Err(anyhow!(
                        "timed out waiting for validation snapshot test handshake"
                    ));
                }
            }
        }
        Ok(validation_summary(&root))
    })();
    let cleanup = fs::remove_dir_all(&root);
    match (materialized, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(anyhow!("validation snapshot cleanup failed: {error}")),
    }
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

#[cfg(unix)]
fn acquire_lock(live_root: &Path) -> Result<Option<AuthorLock>> {
    let binding = root_binding(live_root);
    let parent = open_verified_directory(live_root.parent().unwrap_or(live_root))?;
    let leaf = component_name(&format!(".mdp.author.lock.{}", &binding[..16]))?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            leaf.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error()).context("opening author transaction lock");
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let error = std::io::Error::last_os_error();
        if error
            .raw_os_error()
            .is_some_and(|code| code == libc::EWOULDBLOCK || code == libc::EAGAIN)
        {
            return Ok(None);
        }
        return Err(error).context("locking author transaction");
    }
    file.set_len(0)?;
    file.write_all(b"mdp author transaction lock\n")?;
    file.sync_all()?;
    Ok(Some(AuthorLock { file }))
}

#[cfg(not(unix))]
fn acquire_lock(_: &Path) -> Result<Option<AuthorLock>> {
    Err(anyhow!(
        "identity-bound pack authoring is unsupported on this platform"
    ))
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

#[cfg(unix)]
fn secure_nonce() -> Result<String> {
    let mut bytes = [0_u8; 16];
    File::open("/dev/urandom")
        .context("opening operating-system random source")?
        .read_exact(&mut bytes)
        .context("reading operating-system random source")?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(not(unix))]
fn secure_nonce() -> Result<String> {
    Ok(format!("{:032x}", nonce()))
}

fn test_pause(variable: &str) -> Result<()> {
    if !cfg!(debug_assertions) {
        return Ok(());
    }
    let Some(marker) = std::env::var_os(variable) else {
        return Ok(());
    };
    let marker = PathBuf::from(marker);
    fs::write(&marker, b"ready\n")?;
    let release = marker.with_extension("go");
    for _ in 0..500 {
        if release.exists() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Err(anyhow!("timed out waiting for authoring test handshake"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_set_verification_rejects_oversized_serialized_plans() {
        let present = FileState {
            present: true,
            sha256: Some("a".repeat(64)),
            bytes: Some(1),
        };
        let files = (0..700)
            .map(|index| PlannedFile {
                path: format!(".mdp/cards/{index:04}-{}.yaml", "x".repeat(3_000)),
                action: "create".to_string(),
                expected: FileState::absent(),
                candidate: present.clone(),
            })
            .collect::<Vec<_>>();
        let core = ChangeSetCore {
            contract: CHANGE_SET_CONTRACT.to_string(),
            live_root_sha256: "b".repeat(64),
            validation: ValidationSummary {
                valid: true,
                error_count: 0,
                warning_count: 0,
                diagnostics: vec![],
            },
            files,
        };
        let digest =
            sha256_hex(&canonical_json_bytes(&serde_json::to_value(&core).unwrap()).unwrap());
        let change_set = ChangeSet {
            contract: core.contract,
            live_root_sha256: core.live_root_sha256,
            validation: core.validation,
            files: core.files,
            change_set_sha256: digest,
        };
        assert!(
            change_set
                .verify()
                .unwrap_err()
                .to_string()
                .contains("exceeds byte limit")
        );
    }
}
