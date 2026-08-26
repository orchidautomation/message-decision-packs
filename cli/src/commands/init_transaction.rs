//! Transactional publication helpers for `mdp init`.
//!
//! `init` builds a generated-artifact inventory in a nonce-named staging
//! directory beneath the destination parent, validates the staged tree,
//! then publishes through one of two routes:
//!
//! 1. **Atomic directory rename** when the destination root is absent —
//!    the staged tree is renamed into place as a single filesystem
//!    operation. The destination must live on the same filesystem as
//!    the staging directory.
//! 2. **Rollback-protected merge** when the destination root already
//!    exists — eligible existing generated files are moved to a
//!    transaction-owned backup directory, staged files replace them,
//!    and any failure restores the prior generated tree in reverse
//!    order. This branch is not crash-atomic; failures are handled
//!    locally but a process crash is reported as indeterminate.

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A single generated file that initialization intends to publish.
#[derive(Debug, Clone)]
pub(crate) struct GeneratedArtifact {
    /// Path relative to the destination root, e.g. `.mdp/manifest.yaml`.
    pub relative: String,
    /// Rendered file bytes.
    pub bytes: Vec<u8>,
    /// Content kind label: `yaml-file`, `json-file`, or `markdown-file`.
    pub kind: &'static str,
    /// `true` when the artifact is part of the public generated tree and
    /// may be replaced or removed as part of a publication transaction.
    pub eligible_for_force: bool,
}

impl GeneratedArtifact {
    pub(crate) fn absolute(&self, root: &Path) -> PathBuf {
        root.join(&self.relative)
    }
}

/// Snapshot of an eligible generated file that existed at the destination
/// before the publication transaction began. The bytes are the original
/// destination content captured before staging replaced the file.
#[derive(Debug)]
struct ExistingBackup {
    /// The original destination path.
    absolute: PathBuf,
    /// The original bytes captured before staging replaced them.
    bytes: Vec<u8>,
    /// The relative path used to name the backup file in `backup_root`.
    relative: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PublicationMode {
    AtomicDirectoryRename,
    RollbackProtectedMerge,
}

impl PublicationMode {
    fn label(self) -> &'static str {
        match self {
            PublicationMode::AtomicDirectoryRename => "atomic-directory-rename",
            PublicationMode::RollbackProtectedMerge => "rollback-protected-merge",
        }
    }

    fn atomic(self) -> bool {
        matches!(self, PublicationMode::AtomicDirectoryRename)
    }
}

/// Outcome of a publication attempt returned to the caller.
pub(crate) struct PublicationOutcome {
    pub status: &'static str,
    pub mode: PublicationMode,
    pub staging_root: PathBuf,
    pub backup_root: PathBuf,
    /// `true` when the transaction completed without a handled failure.
    pub published: bool,
}

pub(crate) struct DryRunPlan {
    pub mode: PublicationMode,
    pub entries: Vec<DryRunEntry>,
}

pub(crate) struct DryRunEntry {
    pub path: String,
    pub kind: &'static str,
    pub relative: String,
    pub action: String,
    pub existed: bool,
    pub eligible: bool,
}

/// Render the artifacts into a fresh nonce-named staging directory
/// beneath `parent` and return the staged root path.
///
/// The parent must already exist; this function only creates the
/// nonce-named staging directory and the generated tree beneath it.
pub(crate) fn stage_artifacts(
    parent: &Path,
    artifacts: &[GeneratedArtifact],
    nonce: &str,
) -> Result<PathBuf> {
    let staging_root = parent.join(format!(".mdp.init.staging.{nonce}"));
    if staging_root.exists() {
        let _ = remove_quietly(&staging_root);
    }
    for artifact in artifacts {
        let target = staging_root.join(&artifact.relative);
        if let Some(parent_dir) = target.parent() {
            fs::create_dir_all(parent_dir)
                .with_context(|| format!("creating staging directory {}", parent_dir.display()))?;
        }
        fs::write(&target, &artifact.bytes)
            .with_context(|| format!("writing staged artifact {}", target.display()))?;
    }
    Ok(staging_root)
}

/// Inspect the destination root and report a per-artifact preflight
/// decision. The function does not mutate the destination. A symlink or
/// non-regular file at a generated path always reports a hard error
/// because it cannot be replaced safely; otherwise the entry records
/// whether the path would be created, overwritten, or blocked.
pub(crate) fn preflight(
    destination: &Path,
    artifacts: &[GeneratedArtifact],
    force: bool,
) -> Result<Vec<DryRunEntry>> {
    let mut entries = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        let absolute = artifact.absolute(destination);
        let entry = match fs::symlink_metadata(&absolute) {
            Ok(metadata) => {
                let file_type = metadata.file_type();
                if file_type.is_symlink() {
                    return Err(anyhow!(
                        "refusing to traverse symlink at {} during init preflight",
                        absolute.display()
                    ));
                }
                if !file_type.is_file() {
                    return Err(anyhow!(
                        "refusing to overwrite non-regular node at {} during init preflight",
                        absolute.display()
                    ));
                }
                if force && artifact.eligible_for_force {
                    DryRunEntry {
                        path: absolute.display().to_string(),
                        kind: artifact.kind,
                        relative: artifact.relative.clone(),
                        action: "overwrite".to_string(),
                        existed: true,
                        eligible: artifact.eligible_for_force,
                    }
                } else {
                    DryRunEntry {
                        path: absolute.display().to_string(),
                        kind: artifact.kind,
                        relative: artifact.relative.clone(),
                        action: "blocked".to_string(),
                        existed: true,
                        eligible: artifact.eligible_for_force,
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => DryRunEntry {
                path: absolute.display().to_string(),
                kind: artifact.kind,
                relative: artifact.relative.clone(),
                action: "create".to_string(),
                existed: false,
                eligible: artifact.eligible_for_force,
            },
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("preflight metadata for {}", absolute.display()));
            }
        };
        entries.push(entry);
    }
    Ok(entries)
}

/// Publish a validated staging tree into the destination with one of
/// the supported publication modes.
pub(crate) fn publish(
    destination: &Path,
    staging_root: &Path,
    artifacts: &[GeneratedArtifact],
    backup_root: &Path,
    force: bool,
) -> Result<PublicationOutcome> {
    let root_exists = destination.exists();
    let mode = if root_exists {
        PublicationMode::RollbackProtectedMerge
    } else {
        PublicationMode::AtomicDirectoryRename
    };

    if !root_exists {
        // Atomic directory rename: the destination must be on the same
        // filesystem as the staging directory. We assume the staging
        // directory is always created beneath the destination's parent,
        // so this is the case when the destination itself is absent.
        if let Some(parent) = destination.parent() {
            ensure_same_filesystem(parent, staging_root, destination)?;
        }
        fs::rename(staging_root, destination).with_context(|| {
            format!(
                "atomic rename of {} into {}",
                staging_root.display(),
                destination.display()
            )
        })?;
        let _ = remove_quietly(backup_root);
        return Ok(PublicationOutcome {
            status: "published",
            mode,
            staging_root: staging_root.to_path_buf(),
            backup_root: backup_root.to_path_buf(),
            published: true,
        });
    }

    // Rollback-protected merge.
    merge_into_existing_root(destination, staging_root, artifacts, backup_root, force)
}

fn merge_into_existing_root(
    destination: &Path,
    staging_root: &Path,
    artifacts: &[GeneratedArtifact],
    backup_root: &Path,
    force: bool,
) -> Result<PublicationOutcome> {
    let _ = remove_quietly(backup_root);
    fs::create_dir_all(backup_root)
        .with_context(|| format!("creating backup root {}", backup_root.display()))?;
    let mut backups: Vec<ExistingBackup> = Vec::new();
    let mut staged_replacements: Vec<PathBuf> = Vec::new();

    let merge = (|| -> Result<()> {
        for artifact in artifacts {
            let absolute = artifact.absolute(destination);
            if absolute.exists() {
                if !artifact.eligible_for_force || !force {
                    return Err(anyhow!(
                        "{} already exists; pass --force to overwrite",
                        absolute.display()
                    ));
                }
                let metadata = fs::symlink_metadata(&absolute)
                    .with_context(|| format!("recheck metadata for {}", absolute.display()))?;
                let file_type = metadata.file_type();
                if file_type.is_symlink() {
                    return Err(anyhow!(
                        "refusing to traverse symlink at {} during publication",
                        absolute.display()
                    ));
                }
                if !file_type.is_file() {
                    return Err(anyhow!(
                        "refusing to overwrite non-regular node at {} during publication",
                        absolute.display()
                    ));
                }
                let backup_name = backup_name_for(&artifact.relative);
                let backup_path = backup_root.join(&backup_name);
                let bytes = fs::read(&absolute).with_context(|| {
                    format!("reading existing file {} before backup", absolute.display())
                })?;
                fs::rename(&absolute, &backup_path).with_context(|| {
                    format!(
                        "moving existing {} into backup {}",
                        absolute.display(),
                        backup_path.display()
                    )
                })?;
                backups.push(ExistingBackup {
                    absolute: absolute.clone(),
                    bytes,
                    relative: artifact.relative.clone(),
                });
            }
        }
        // Replace from staging. Each staged file is moved into the
        // destination; partial replacement is rolled back on failure.
        for artifact in artifacts {
            let staged = staging_root.join(&artifact.relative);
            if !staged.exists() {
                return Err(anyhow!(
                    "staged artifact {} missing before publication",
                    staged.display()
                ));
            }
            let target = artifact.absolute(destination);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating destination parent {}", parent.display()))?;
            }
            fs::rename(&staged, &target).with_context(|| {
                format!(
                    "publishing staged {} to {}",
                    staged.display(),
                    target.display()
                )
            })?;
            staged_replacements.push(target);
        }
        Ok(())
    })();

    if let Err(error) = merge {
        // Roll back: remove any partial replacement, then restore
        // backup files into their original paths.
        for replacement in staged_replacements.iter().rev() {
            let _ = fs::remove_file(replacement);
        }
        for backup in backups.iter() {
            let backup_path = backup_root.join(backup_name_for(&backup.relative));
            if backup_path.exists() {
                if let Err(rename_error) = fs::rename(&backup_path, &backup.absolute) {
                    // If rename fails (e.g. cross-device), try to
                    // rewrite the original bytes so the destination
                    // path holds its prior content.
                    if let Err(write_error) = fs::write(&backup.absolute, &backup.bytes) {
                        return Err(error.context(anyhow!(
                            "rollback failed for {}: rename={} write={}",
                            backup.absolute.display(),
                            rename_error,
                            write_error
                        )));
                    }
                    let _ = fs::remove_file(&backup_path);
                }
            }
        }
        let _ = remove_quietly(backup_root);
        let _ = remove_quietly(staging_root);
        return Err(error);
    }

    // Publication succeeded; clean up backups and any staging residue.
    for backup in &backups {
        let backup_path = backup_root.join(backup_name_for(&backup.relative));
        let _ = fs::remove_file(&backup_path);
    }
    let _ = remove_quietly(backup_root);
    let _ = remove_quietly(staging_root);

    Ok(PublicationOutcome {
        status: "published",
        mode: PublicationMode::RollbackProtectedMerge,
        staging_root: staging_root.to_path_buf(),
        backup_root: backup_root.to_path_buf(),
        published: true,
    })
}

/// Run a dry-run analysis that returns a write plan without touching
/// the destination.
pub(crate) fn dry_run(
    destination: &Path,
    artifacts: &[GeneratedArtifact],
    force: bool,
) -> Result<DryRunPlan> {
    let entries = preflight(destination, artifacts, force)?;
    let mode = if destination.exists() {
        PublicationMode::RollbackProtectedMerge
    } else {
        PublicationMode::AtomicDirectoryRename
    };
    Ok(DryRunPlan { mode, entries })
}

/// Remove the transaction-owned staging, backup, and snapshot paths.
pub(crate) fn cleanup(paths: &[&Path]) {
    for path in paths {
        let _ = remove_quietly(path);
    }
}

/// Build the canonical staging and backup directory names for a given
/// nonce. Returned paths are siblings of `destination` so atomic rename
/// stays on the same filesystem.
pub(crate) fn publication_paths(destination: &Path, nonce: &str) -> (PathBuf, PathBuf) {
    let parent = destination.parent().unwrap_or(destination);
    let staging = parent.join(format!(".mdp.init.staging.{nonce}"));
    let backup = parent.join(format!(".mdp.init.backup.{nonce}"));
    (staging, backup)
}

pub(crate) fn dry_run_value(plan: &DryRunPlan) -> Value {
    let entries: Vec<Value> = plan
        .entries
        .iter()
        .map(|entry| {
            json!({
                "kind": entry.kind,
                "path": entry.path,
                "relative": entry.relative,
                "action": entry.action,
                "existed": entry.existed,
                "eligible": entry.eligible,
            })
        })
        .collect();
    json!({
        "mode": plan.mode.label(),
        "atomic": plan.mode.atomic(),
        "entries": entries,
    })
}

fn remove_quietly(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_dir() {
                fs::remove_dir_all(path)
                    .with_context(|| format!("removing directory {}", path.display()))?;
            } else {
                fs::remove_file(path)
                    .with_context(|| format!("removing file {}", path.display()))?;
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("inspecting {}", path.display())),
    }
}

fn ensure_same_filesystem(parent: &Path, staging: &Path, destination: &Path) -> Result<()> {
    let parent_dev = device_id(parent)?;
    let staging_dev = device_id(staging)?;
    let dest_parent = destination.parent().unwrap_or(destination);
    let dest_dev = device_id(dest_parent)?;
    if parent_dev != staging_dev || parent_dev != dest_dev {
        return Err(anyhow!(
            "atomic rename requires destination, staging, and parent to share a filesystem"
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn device_id(path: &Path) -> Result<u64> {
    use std::os::unix::fs::MetadataExt;
    let metadata =
        fs::metadata(path).with_context(|| format!("reading device id for {}", path.display()))?;
    Ok(metadata.dev())
}

#[cfg(not(unix))]
fn device_id(_path: &Path) -> Result<u64> {
    Ok(0)
}

fn backup_name_for(relative: &str) -> String {
    format!("existing.{}", relative.replace(['/', '\\'], "_"))
}

/// Generate a fresh nonce based on the system clock combined with an
/// atomic counter to keep successive calls distinct even within the
/// same nanosecond.
pub(crate) fn fresh_nonce() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}", nanos, counter)
}

/// Build the `publication` JSON envelope for an outcome.
pub(crate) fn publication_envelope(outcome: &PublicationOutcome) -> Value {
    json!({
        "status": outcome.status,
        "mode": outcome.mode.label(),
        "atomic": outcome.mode.atomic(),
        "staging_root": outcome.staging_root.display().to_string(),
        "backup_root": outcome.backup_root.display().to_string(),
    })
}

/// Build the `publication` JSON envelope for a dry-run plan.
pub(crate) fn dry_run_envelope(plan: &DryRunPlan) -> Value {
    json!({
        "status": "dry-run",
        "mode": plan.mode.label(),
        "atomic": plan.mode.atomic(),
    })
}
