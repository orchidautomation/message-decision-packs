//! Black-box proof for previewable, failure-safe multi-file pack authoring.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mdp"))
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos()
}

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("mdp-author-{label}-{}", nonce()))
}

fn template() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../plugin/assets/templates/basic")
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("target directory should be created");
    for entry in fs::read_dir(source).expect("source directory should be readable") {
        let entry = entry.expect("directory entry should be readable");
        let kind = entry.file_type().expect("entry type should be readable");
        let destination = target.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            assert!(
                kind.is_file(),
                "fixtures must contain only files and directories"
            );
            fs::copy(entry.path(), destination).expect("fixture file should copy");
        }
    }
}

fn run(args: &[&str], fault_after: Option<usize>) -> Output {
    run_env(args, fault_after, &[])
}

fn run_env(args: &[&str], fault_after: Option<usize>, environment: &[(&str, String)]) -> Output {
    let mut command = Command::new(binary());
    command.arg("--json").args(args);
    if let Some(boundary) = fault_after {
        command.env("MDP_TEST_AUTHOR_FAULT_AFTER", boundary.to_string());
    }
    for (name, value) in environment {
        command.env(name, value);
    }
    command.output().expect("mdp author command should run")
}

fn data(output: &Output) -> Value {
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "author output should be JSON ({error}); stderr={} stdout={}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout),
        )
    });
    envelope["data"].clone()
}

fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn collect(root: &Path, current: &Path, values: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(current).expect("snapshot directory should be readable") {
            let entry = entry.expect("snapshot entry should be readable");
            let kind = entry
                .file_type()
                .expect("snapshot entry type should be readable");
            if kind.is_dir() {
                collect(root, &entry.path(), values);
            } else if kind.is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .expect("snapshot path should be relative")
                    .to_string_lossy()
                    .replace('\\', "/");
                values.insert(
                    relative,
                    fs::read(entry.path()).expect("snapshot file readable"),
                );
            }
        }
    }
    let mut values = BTreeMap::new();
    collect(root, root, &mut values);
    values
}

fn fixture(label: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let root = temp_root(label);
    let live = root.join("live");
    let candidate = root.join("candidate");
    copy_tree(&template(), &live);
    copy_tree(&live, &candidate);
    fs::write(
        candidate.join(".mdp/README.md"),
        "# Candidate pack\n\nReviewed multi-file authoring fixture.\n",
    )
    .expect("candidate README should change");
    fs::create_dir_all(candidate.join(".mdp/authoring")).expect("candidate directory should exist");
    fs::write(
        candidate.join(".mdp/authoring/review-note.txt"),
        "reviewed\n",
    )
    .expect("candidate file should be created");
    fs::remove_file(candidate.join(".mdp/sources.yaml")).expect("candidate file should delete");
    let plan = root.join("change-set.json");
    (root, live, candidate, plan)
}

fn preview(live: &Path, candidate: &Path, plan: &Path) -> Output {
    run(&preview_args(live, candidate, plan), None)
}

fn preview_args<'a>(live: &'a Path, candidate: &'a Path, plan: &'a Path) -> [&'a str; 8] {
    [
        "author",
        "preview",
        "--dir",
        live.to_str().unwrap(),
        "--candidate",
        candidate.to_str().unwrap(),
        "--out",
        plan.to_str().unwrap(),
    ]
}

fn apply(live: &Path, candidate: &Path, plan: &Path, fault_after: Option<usize>) -> Output {
    run(
        &[
            "author",
            "apply",
            "--dir",
            live.to_str().unwrap(),
            "--candidate",
            candidate.to_str().unwrap(),
            "--change-set",
            plan.to_str().unwrap(),
        ],
        fault_after,
    )
}

fn apply_args<'a>(live: &'a Path, candidate: &'a Path, plan: &'a Path) -> [&'a str; 8] {
    [
        "author",
        "apply",
        "--dir",
        live.to_str().unwrap(),
        "--candidate",
        candidate.to_str().unwrap(),
        "--change-set",
        plan.to_str().unwrap(),
    ]
}

#[test]
fn preview_is_read_only_bounded_and_applies_outside_git() {
    let (root, live, candidate, plan) = fixture("success");
    let unrelated = live.join("operator-notes.txt");
    fs::write(&unrelated, "preserve me\n").expect("unrelated file should exist");
    let before = snapshot(&live);

    let output = preview(&live, &candidate, &plan);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let previewed = data(&output);
    assert_eq!(previewed["contract"], "mdp.pack-authoring-result.v1");
    assert_eq!(previewed["status"], "previewed");
    assert_eq!(previewed["valid"], true);
    assert!(
        previewed["created"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == ".mdp/authoring/review-note.txt")
    );
    assert!(
        previewed["changed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == ".mdp/README.md")
    );
    assert!(
        previewed["deleted"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == ".mdp/sources.yaml")
    );
    assert!(!previewed["unchanged"].as_array().unwrap().is_empty());
    assert_eq!(
        snapshot(&live),
        before,
        "preview must not write the live pack"
    );
    assert!(
        !root.join(".git").exists(),
        "the authoring workflow must not require Git"
    );
    let plan_text = fs::read_to_string(&plan).expect("change set should exist");
    assert!(!plan_text.contains("Reviewed multi-file authoring fixture"));
    assert!(!plan_text.contains("preserve me"));

    let output = apply(&live, &candidate, &plan, None);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let applied = data(&output);
    assert_eq!(applied["status"], "applied");
    assert_eq!(applied["valid"], true);
    assert_eq!(fs::read_to_string(&unrelated).unwrap(), "preserve me\n");
    let mut expected = snapshot(&candidate);
    expected.insert("operator-notes.txt".to_string(), b"preserve me\n".to_vec());
    assert_eq!(snapshot(&live), expected);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn apply_refuses_live_and_candidate_drift_with_exact_paths() {
    for drift in ["live", "candidate"] {
        let (root, live, candidate, plan) = fixture(drift);
        let output = preview(&live, &candidate, &plan);
        assert!(output.status.success());
        let path = if drift == "live" {
            live.join(".mdp/README.md")
        } else {
            candidate.join(".mdp/README.md")
        };
        fs::write(&path, format!("{drift} concurrent edit\n")).expect("drift write should succeed");
        let output = apply(&live, &candidate, &plan, None);
        assert!(!output.status.success());
        let refused = data(&output);
        assert_eq!(refused["status"], "refused");
        assert!(
            refused["refused"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item == ".mdp/README.md")
        );
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            format!("{drift} concurrent edit\n")
        );
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn invalid_candidate_is_refused_without_live_or_plan_writes() {
    let (root, live, candidate, plan) = fixture("invalid");
    fs::write(candidate.join(".mdp/manifest.yaml"), "not: [valid\n")
        .expect("candidate manifest should become invalid");
    let before = snapshot(&live);
    let output = preview(&live, &candidate, &plan);
    assert!(!output.status.success());
    let refused = data(&output);
    assert_eq!(refused["status"], "refused");
    assert!(
        refused["reason_codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "candidate-validation-failed")
    );
    assert!(!plan.exists());
    assert_eq!(snapshot(&live), before);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn tampered_change_set_and_in_pack_plan_paths_are_rejected_without_writes() {
    let (root, live, candidate, plan) = fixture("sealed");
    let before = snapshot(&live);
    let nested_plan = live.join("change-set.json");
    let output = preview(&live, &candidate, &nested_plan);
    assert!(!output.status.success());
    assert!(!nested_plan.exists());
    let nested_candidate_plan = candidate.join("change-set.json");
    let output = preview(&live, &candidate, &nested_candidate_plan);
    assert!(!output.status.success());
    assert!(!nested_candidate_plan.exists());

    let output = preview(&live, &candidate, &plan);
    assert!(output.status.success());
    let mut sealed: Value = serde_json::from_slice(&fs::read(&plan).unwrap()).unwrap();
    sealed["files"][0]["action"] = Value::String("create".to_string());
    fs::write(&plan, serde_json::to_vec_pretty(&sealed).unwrap()).unwrap();
    let output = apply(&live, &candidate, &plan, None);
    assert!(!output.status.success());
    assert_eq!(snapshot(&live), before);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn every_injected_publication_boundary_rolls_back_all_mdp_owned_writes() {
    let (root, live, candidate, plan) = fixture("faults");
    fs::write(live.join("operator-notes.txt"), "unrelated\n").expect("unrelated file should exist");
    let output = preview(&live, &candidate, &plan);
    assert!(output.status.success());
    let before = snapshot(&live);
    let mut exercised = 0usize;
    for boundary in 1..=16 {
        let output = apply(&live, &candidate, &plan, Some(boundary));
        let result = data(&output);
        if output.status.success() {
            assert_eq!(result["status"], "applied");
            break;
        }
        exercised += 1;
        assert_eq!(
            result["status"], "rolled-back",
            "boundary {boundary}: {result}"
        );
        assert!(
            result["rolled_back"].is_array(),
            "boundary {boundary} must report its rolled-back path list"
        );
        assert_eq!(
            snapshot(&live),
            before,
            "boundary {boundary} left a partial publication"
        );
        assert!(
            !live.join(".mdp/authoring").exists(),
            "boundary {boundary} left a created directory"
        );
        assert_eq!(
            fs::read_to_string(live.join("operator-notes.txt")).unwrap(),
            "unrelated\n"
        );
    }
    assert!(
        exercised >= 4,
        "test should cross every backup/create/install boundary"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn crash_after_live_backup_is_recovered_before_the_next_apply() {
    let (root, live, candidate, plan) = fixture("crash-recovery");
    assert!(preview(&live, &candidate, &plan).status.success());
    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args(apply_args(&live, &candidate, &plan))
        .env("MDP_TEST_AUTHOR_CRASH_AFTER", "7");
    let crashed = command.output().expect("crashing author child should run");
    assert!(!crashed.status.success());

    let recovered = apply(&live, &candidate, &plan, None);
    assert!(
        recovered.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    assert_eq!(data(&recovered)["status"], "applied");
    assert_eq!(snapshot(&live), snapshot(&candidate));
    let residues = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| {
            name.starts_with(".mdp.author.staging.")
                || name.starts_with(".mdp.author.backup.")
                || name.starts_with(".mdp.author.state.")
        })
        .collect::<Vec<_>>();
    assert!(residues.is_empty(), "recovery residue: {residues:?}");
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn crash_after_parent_publication_recovers_from_durable_directory_identity() {
    let (root, live, candidate, plan) = fixture("parent-publication-crash-recovery");
    assert!(preview(&live, &candidate, &plan).status.success());
    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args(apply_args(&live, &candidate, &plan))
        .env("MDP_TEST_AUTHOR_CRASH_AFTER_PARENT_PUBLISH", "1");
    let crashed = command.output().expect("crashing author child should run");
    assert!(!crashed.status.success());
    assert!(live.join(".mdp/authoring").is_dir());

    let recovered = apply(&live, &candidate, &plan, None);
    assert!(
        recovered.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    assert_eq!(data(&recovered)["status"], "applied");
    assert_eq!(snapshot(&live), snapshot(&candidate));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn crash_after_commit_marker_finishes_cleanup_without_rolling_back() {
    let (root, live, candidate, plan) = fixture("committed-crash-recovery");
    assert!(preview(&live, &candidate, &plan).status.success());
    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args(apply_args(&live, &candidate, &plan))
        .env("MDP_TEST_AUTHOR_CRASH_AFTER", "11");
    let crashed = command.output().expect("crashing author child should run");
    assert!(!crashed.status.success());
    assert_eq!(snapshot(&live), snapshot(&candidate));

    let recovered = apply(&live, &candidate, &plan, None);
    assert!(recovered.status.success());
    let result = data(&recovered);
    assert_eq!(result["status"], "applied");
    assert!(
        result["reason_codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "interrupted-commit-recovered")
    );
    assert!(
        result["reason_codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "recovery-evidence-retained"),
        "interrupted commit recovery must report retained evidence: {result}"
    );
    assert_eq!(snapshot(&live), snapshot(&candidate));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn crash_before_state_publication_recovers_the_complete_pending_state() {
    let (root, live, candidate, plan) = fixture("pending-state-recovery");
    assert!(preview(&live, &candidate, &plan).status.success());
    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args(apply_args(&live, &candidate, &plan))
        .env("MDP_TEST_AUTHOR_CRASH_BEFORE_STATE_PUBLISH", "1");
    let crashed = command.output().expect("crashing author child should run");
    assert!(!crashed.status.success());

    let recovered = apply(&live, &candidate, &plan, None);
    assert!(
        recovered.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    assert_eq!(data(&recovered)["status"], "applied");
    assert_eq!(snapshot(&live), snapshot(&candidate));
    let residues = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| {
            name.starts_with(".mdp.author.staging.")
                || name.starts_with(".mdp.author.backup.")
                || name.starts_with(".mdp.author.state.")
                || name.starts_with(".mdp.author.state-pending.")
        })
        .collect::<Vec<_>>();
    assert!(residues.is_empty(), "recovery residue: {residues:?}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn committed_cleanup_restarts_after_staging_workspace_was_removed() {
    let (root, live, candidate, plan) = fixture("partial-cleanup-recovery");
    assert!(preview(&live, &candidate, &plan).status.success());
    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args(apply_args(&live, &candidate, &plan))
        .env("MDP_TEST_AUTHOR_CRASH_AFTER_STAGING_CLEANUP", "1");
    let crashed = command.output().expect("crashing author child should run");
    assert!(!crashed.status.success());
    assert_eq!(snapshot(&live), snapshot(&candidate));

    let recovered = apply(&live, &candidate, &plan, None);
    assert!(recovered.status.success());
    assert_eq!(data(&recovered)["status"], "applied");
    let residues = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| {
            name.starts_with(".mdp.author.staging.")
                || name.starts_with(".mdp.author.backup.")
                || name.starts_with(".mdp.author.state.")
                || name.starts_with(".mdp.author.state-pending.")
        })
        .collect::<Vec<_>>();
    assert!(residues.is_empty(), "recovery residue: {residues:?}");
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn validation_snapshot_is_private_while_validation_runs() {
    use std::os::unix::fs::PermissionsExt;

    let (root, live, candidate, plan) = fixture("private-validation-snapshot");
    let marker = root.join("validation-ready");
    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args(preview_args(&live, &candidate, &plan))
        .env("MDP_TEST_AUTHOR_VALIDATION_MARKER", &marker)
        .stdout(Stdio::null());
    let mut child = command.spawn().expect("preview child should spawn");
    for _ in 0..500 {
        if marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(marker.exists(), "validation handshake should become ready");
    let snapshot_root = PathBuf::from(fs::read_to_string(&marker).unwrap());
    assert_eq!(
        fs::metadata(&snapshot_root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(snapshot_root.join(".mdp/README.md"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    fs::write(marker.with_extension("go"), "go\n").unwrap();
    assert!(child.wait().unwrap().success());
    assert!(!snapshot_root.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rollback_preserves_an_atomic_concurrent_replacement() {
    let (root, live, candidate, plan) = fixture("concurrent-rollback");
    assert!(preview(&live, &candidate, &plan).status.success());
    let marker = root.join("rollback-ready");
    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args(apply_args(&live, &candidate, &plan))
        .env("MDP_TEST_AUTHOR_FAULT_AFTER", "8")
        .env("MDP_TEST_AUTHOR_ROLLBACK_MARKER", &marker)
        .stdout(Stdio::null());
    let mut child = command.spawn().expect("author child should spawn");
    for _ in 0..500 {
        if marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(marker.exists(), "rollback handshake should become ready");
    let replacement = live.join(".mdp/concurrent-replacement");
    fs::write(&replacement, "operator concurrent replacement\n").unwrap();
    fs::rename(&replacement, live.join(".mdp/README.md")).unwrap();
    fs::write(marker.with_extension("go"), "go\n").unwrap();
    let status = child.wait().expect("author child should exit");
    assert!(!status.success());
    assert_eq!(
        fs::read_to_string(live.join(".mdp/README.md")).unwrap(),
        "operator concurrent replacement\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn publication_refuses_same_inode_rewrite_and_preserves_concurrent_bytes() {
    use std::os::unix::fs::MetadataExt;

    let (root, live, candidate, plan) = fixture("concurrent-in-place-rewrite");
    assert!(preview(&live, &candidate, &plan).status.success());
    let marker = root.join("publication-ready");
    let readme = live.join(".mdp/README.md");
    let inode_before = fs::metadata(&readme).unwrap().ino();
    let concurrent = b"operator concurrent in-place rewrite\n";
    let mut expected = snapshot(&live);
    expected.insert(".mdp/README.md".to_string(), concurrent.to_vec());

    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args(apply_args(&live, &candidate, &plan))
        .env("MDP_TEST_AUTHOR_PUBLICATION_MARKER", &marker)
        .stdout(Stdio::piped());
    let child = command.spawn().expect("author child should spawn");
    for _ in 0..500 {
        if marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(marker.exists(), "publication handshake should become ready");
    fs::write(&readme, concurrent).unwrap();
    assert_eq!(fs::metadata(&readme).unwrap().ino(), inode_before);
    fs::write(marker.with_extension("go"), "go\n").unwrap();

    let output = child.wait_with_output().expect("author child should exit");
    assert!(!output.status.success());
    let result = data(&output);
    assert_eq!(result["status"], "rolled-back");
    assert!(
        result["rolled_back"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == ".mdp/README.md")
    );
    assert_eq!(snapshot(&live), expected);
    assert!(!live.join(".mdp/authoring").exists());
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn final_backup_revalidation_retains_late_open_fd_rewrite() {
    let (root, live, candidate, plan) = fixture("late-backup-open-fd-rewrite");
    assert!(preview(&live, &candidate, &plan).status.success());
    let publication_marker = root.join("publication-ready");
    let cleanup_marker = root.join("backup-cleanup-validated");
    let readme = live.join(".mdp/README.md");
    let mut original = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&readme)
        .unwrap();
    let concurrent = b"operator late backup rewrite through open fd\n";

    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args(apply_args(&live, &candidate, &plan))
        .env("MDP_TEST_AUTHOR_PUBLICATION_MARKER", &publication_marker)
        .env(
            "MDP_TEST_AUTHOR_AFTER_BACKUP_VALIDATION_MARKER",
            &cleanup_marker,
        )
        .stdout(Stdio::piped());
    let child = command.spawn().expect("author child should spawn");
    for _ in 0..500 {
        if publication_marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(publication_marker.exists());
    fs::write(publication_marker.with_extension("go"), "go\n").unwrap();
    for _ in 0..500 {
        if cleanup_marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(cleanup_marker.exists());
    original.set_len(0).unwrap();
    original.seek(SeekFrom::Start(0)).unwrap();
    original.write_all(concurrent).unwrap();
    original.sync_all().unwrap();
    fs::write(cleanup_marker.with_extension("go"), "go\n").unwrap();

    let output = child.wait_with_output().expect("author child should exit");
    assert!(output.status.success());
    assert!(
        data(&output)["reason_codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "recovery-evidence-retained"),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(snapshot(&live), snapshot(&candidate));
    let backup = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".mdp.author.evidence.commit.")
        })
        .expect("drifted backup recovery evidence should remain");
    assert_eq!(fs::read(backup.join("README.md")).unwrap(), concurrent);
    assert!(fs::read_dir(&root).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".mdp.author.evidence-state.")
    }));
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn rollback_preserves_in_place_edit_of_newly_installed_inode() {
    let (root, live, candidate, plan) = fixture("installed-open-fd-rewrite");
    assert!(preview(&live, &candidate, &plan).status.success());
    let publication_marker = root.join("publication-ready");
    let validation_marker = root.join("published-validation-ready");
    let staging_marker = root.join("staging-validation-ready");
    let concurrent = b"operator edited newly installed authority\n";

    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args(apply_args(&live, &candidate, &plan))
        .env("MDP_TEST_AUTHOR_PUBLICATION_MARKER", &publication_marker)
        .env(
            "MDP_TEST_AUTHOR_BEFORE_PUBLISHED_VALIDATION_MARKER",
            &validation_marker,
        )
        .env(
            "MDP_TEST_AUTHOR_AFTER_STAGING_VALIDATION_MARKER",
            &staging_marker,
        )
        .env("MDP_TEST_AUTHOR_FAIL_BEFORE_COMMIT", "1")
        .stdout(Stdio::piped());
    let child = command.spawn().expect("author child should spawn");
    for _ in 0..500 {
        if publication_marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(publication_marker.exists());
    fs::write(publication_marker.with_extension("go"), "go\n").unwrap();
    for _ in 0..500 {
        if validation_marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(validation_marker.exists());
    let readme = live.join(".mdp/README.md");
    let mut installed = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&readme)
        .unwrap();
    fs::write(validation_marker.with_extension("go"), "go\n").unwrap();
    for _ in 0..500 {
        if staging_marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(staging_marker.exists());
    installed.set_len(0).unwrap();
    installed.seek(SeekFrom::Start(0)).unwrap();
    installed.write_all(concurrent).unwrap();
    installed.sync_all().unwrap();
    fs::write(staging_marker.with_extension("go"), "go\n").unwrap();

    let output = child.wait_with_output().expect("author child should exit");
    assert!(!output.status.success());
    assert_eq!(data(&output)["status"], "rolled-back");
    assert_ne!(fs::read(&readme).unwrap(), concurrent);
    let evidence = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".mdp.author.evidence.rollback.")
        })
        .expect("rollback evidence should remain");
    assert_eq!(fs::read(evidence.join("README.md")).unwrap(), concurrent);
    assert!(fs::read_dir(&root).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".mdp.author.evidence-state.")
    }));
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn rollback_does_not_remove_concurrently_created_parent_directory() {
    let (root, live, candidate, plan) = fixture("concurrent-parent-create");
    assert!(preview(&live, &candidate, &plan).status.success());
    let marker = root.join("parent-mkdir-ready");
    let parent = live.join(".mdp/authoring");
    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args(apply_args(&live, &candidate, &plan))
        .env("MDP_TEST_AUTHOR_BEFORE_PARENT_PUBLISH_MARKER", &marker)
        .env("MDP_TEST_AUTHOR_FAIL_BEFORE_COMMIT", "1")
        .stdout(Stdio::piped());
    let child = command.spawn().expect("author child should spawn");
    for _ in 0..500 {
        if marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(marker.exists());
    fs::create_dir(&parent).unwrap();
    fs::write(marker.with_extension("go"), "go\n").unwrap();

    let output = child.wait_with_output().expect("author child should exit");
    assert!(!output.status.success());
    assert_eq!(data(&output)["status"], "rolled-back");
    assert!(parent.is_dir());
    assert!(fs::read_dir(&parent).unwrap().next().is_none());
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn pre_state_failure_never_creates_live_parents_or_claims_evidence() {
    let (root, live, candidate, plan) = fixture("pre-state-parent-cleanup");
    assert!(preview(&live, &candidate, &plan).status.success());

    let output = run_env(
        &apply_args(&live, &candidate, &plan),
        None,
        &[(
            "MDP_TEST_AUTHOR_FAIL_AFTER_PARENT_RECORDING",
            "1".to_string(),
        )],
    );

    assert!(!output.status.success());
    let result = data(&output);
    assert_eq!(result["status"], "rolled-back");
    assert!(!live.join(".mdp/authoring").exists());
    assert!(
        !result["reason_codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "recovery-evidence-retained"),
        "delete-only recovery must not claim retained evidence: {result}"
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn pre_state_workspace_cleanup_reports_no_retained_evidence() {
    let (root, live, candidate, plan) = fixture("pre-state-workspace-cleanup");
    assert!(preview(&live, &candidate, &plan).status.success());

    let output = apply(&live, &candidate, &plan, Some(1));

    assert!(!output.status.success());
    let result = data(&output);
    assert_eq!(result["status"], "rolled-back");
    assert!(
        !result["reason_codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "recovery-evidence-retained"),
        "removed staging workspace is not retained evidence: {result}"
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn delete_only_rollback_reports_no_retained_evidence() {
    let root = temp_root("delete-only-no-evidence");
    let live = root.join("live");
    let candidate = root.join("candidate");
    copy_tree(&template(), &live);
    copy_tree(&live, &candidate);
    fs::remove_file(candidate.join(".mdp/sources.yaml")).unwrap();
    let plan = root.join("change-set.json");
    assert!(preview(&live, &candidate, &plan).status.success());

    let output = apply(&live, &candidate, &plan, Some(4));

    assert!(!output.status.success());
    let result = data(&output);
    assert_eq!(result["status"], "rolled-back");
    assert!(
        !result["reason_codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "recovery-evidence-retained"),
        "delete-only rollback retained no evidence: {result}"
    );
    assert!(live.join(".mdp/sources.yaml").is_file());
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn concurrent_parent_at_publish_is_not_recorded_or_removed() {
    let (root, live, candidate, plan) = fixture("concurrent-parent-publish");
    assert!(preview(&live, &candidate, &plan).status.success());
    let marker = root.join("parent-publish-ready");
    let parent = live.join(".mdp/authoring");
    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args(apply_args(&live, &candidate, &plan))
        .env("MDP_TEST_AUTHOR_BEFORE_PARENT_PUBLISH_MARKER", &marker)
        .stdout(Stdio::piped());
    let child = command.spawn().expect("author child should spawn");
    for _ in 0..500 {
        if marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(marker.exists());
    fs::create_dir(&parent).unwrap();
    fs::write(parent.join("keep"), b"concurrent replacement\n").unwrap();
    fs::write(marker.with_extension("go"), "go\n").unwrap();

    let output = child.wait_with_output().expect("author child should exit");
    assert!(!output.status.success());
    assert_eq!(data(&output)["status"], "rolled-back");
    assert_eq!(
        fs::read(parent.join("keep")).unwrap(),
        b"concurrent replacement\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cleanup_failure_is_indeterminate_and_recoverable() {
    let (root, live, candidate, plan) = fixture("cleanup-failure");
    assert!(preview(&live, &candidate, &plan).status.success());
    let failed = run_env(
        &apply_args(&live, &candidate, &plan),
        Some(6),
        &[("MDP_TEST_AUTHOR_CLEANUP_FAIL", "staging".to_string())],
    );
    assert!(!failed.status.success());
    assert_eq!(data(&failed)["status"], "rolled-back");
    assert!(
        data(&failed)["reason_codes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|code| code == "recovery-evidence-retained")
    );
    let recovered = apply(&live, &candidate, &plan, None);
    assert!(recovered.status.success());
    assert_eq!(data(&recovered)["status"], "applied");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn preview_rejects_a_union_larger_than_the_applicable_plan_limit() {
    let (root, live, candidate, plan) = fixture("union-limit");
    let live_extra = live.join(".mdp/live-only");
    let candidate_extra = candidate.join(".mdp/candidate-only");
    fs::create_dir_all(&live_extra).unwrap();
    fs::create_dir_all(&candidate_extra).unwrap();
    for index in 0..1_050 {
        fs::write(live_extra.join(format!("{index:04}.txt")), b"live\n").unwrap();
        fs::write(
            candidate_extra.join(format!("{index:04}.txt")),
            b"candidate\n",
        )
        .unwrap();
    }
    let output = preview(&live, &candidate, &plan);
    assert!(!output.status.success());
    assert!(!plan.exists());
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("change set exceeds managed file limit")
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn preview_stays_bound_to_opened_root_during_ancestor_swap() {
    use std::os::unix::fs::symlink;

    let (root, live, candidate, plan) = fixture("ancestor-swap");
    let marker = root.join("root-opened");
    let escape = root.join("escape");
    copy_tree(&template(), &escape);
    let escape_before = snapshot(&escape);
    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args([
            "author",
            "preview",
            "--dir",
            live.to_str().unwrap(),
            "--candidate",
            candidate.to_str().unwrap(),
            "--out",
            plan.to_str().unwrap(),
        ])
        .env("MDP_TEST_AUTHOR_PAUSE_ROOT", &live)
        .env("MDP_TEST_AUTHOR_ROOT_MARKER", &marker)
        .stdout(Stdio::null());
    let mut child = command.spawn().unwrap();
    for _ in 0..500 {
        if marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(marker.exists());
    let original = root.join("live-opened-original");
    fs::rename(&live, &original).unwrap();
    symlink(&escape, &live).unwrap();
    fs::write(marker.with_extension("go"), "go\n").unwrap();
    let status = child.wait().unwrap();
    assert!(status.success());
    assert_eq!(snapshot(&escape), escape_before);
    fs::remove_file(&live).unwrap();
    fs::rename(&original, &live).unwrap();
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn apply_never_follows_a_swapped_live_root_and_recovers_when_restored() {
    use std::os::unix::fs::symlink;

    let (root, live, candidate, plan) = fixture("apply-ancestor-swap");
    assert!(preview(&live, &candidate, &plan).status.success());
    let marker = root.join("publication-ready");
    let escape = root.join("escape-apply");
    copy_tree(&template(), &escape);
    let escape_before = snapshot(&escape);
    let mut command = Command::new(binary());
    command
        .arg("--json")
        .args(apply_args(&live, &candidate, &plan))
        .env("MDP_TEST_AUTHOR_PUBLICATION_MARKER", &marker)
        .stdout(Stdio::null());
    let mut child = command.spawn().unwrap();
    for _ in 0..500 {
        if marker.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(marker.exists());
    let original = root.join("live-opened-apply");
    fs::rename(&live, &original).unwrap();
    symlink(&escape, &live).unwrap();
    fs::write(marker.with_extension("go"), "go\n").unwrap();
    assert!(!child.wait().unwrap().success());
    assert_eq!(snapshot(&escape), escape_before);

    fs::remove_file(&live).unwrap();
    fs::rename(&original, &live).unwrap();
    let recovered = apply(&live, &candidate, &plan, None);
    assert!(recovered.status.success());
    assert_eq!(snapshot(&live), snapshot(&candidate));
    assert_eq!(snapshot(&escape), escape_before);
    let _ = fs::remove_dir_all(root);
}
