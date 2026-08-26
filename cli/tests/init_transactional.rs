//! Black-box coverage for the transactional `mdp init` publication path.
//!
//! These tests drive the compiled `mdp` binary with a fresh
//! `CARGO_BIN_EXE_mdp` per case and assert on the rendered JSON output
//! and the destination filesystem. The CLI must either publish a
//! complete validated starter tree or leave the destination unchanged
//! after any handled failure.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
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
    std::env::temp_dir().join(format!("mdp-init-tx-{label}-{}", nonce()))
}

fn run_init(dir: &Path, template: &str, force: bool, extra: &[&str]) -> (bool, String, String) {
    let mut command = Command::new(binary());
    command.arg("--json");
    command.arg("init");
    command.arg("--template").arg(template);
    command.arg("--dir").arg(dir);
    if force {
        command.arg("--force");
    }
    for flag in extra {
        command.arg(flag);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = command.output().expect("init should run");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Extract the inner `data` value from the CLI's top-level JSON
/// envelope. The CLI wraps every command response in
/// `{"command": ..., "data": ..., "ok": ...}`.
fn data_envelope(stdout: &str) -> Value {
    let envelope: Value = serde_json::from_str(stdout).expect("init output should be JSON");
    envelope
        .get("data")
        .cloned()
        .expect("init envelope should include data")
}

fn assert_published_tree(root: &Path) {
    for relative in [
        ".mdp/manifest.yaml",
        ".mdp/sources.yaml",
        ".mdp/README.md",
        "examples/decision-input-scenarios.json",
    ] {
        assert!(
            root.join(relative).exists(),
            "{} should exist after publication",
            relative
        );
    }
    let example_prospect = if root.join("examples").join("clay-row.json").exists() {
        "examples/clay-row.json"
    } else {
        "examples/prospect-row.json"
    };
    assert!(
        root.join(example_prospect).exists(),
        "example prospect should exist"
    );
}

fn assert_publication_paths_clean(publication: &Value) {
    for key in ["staging_root", "backup_root"] {
        if let Some(value) = publication.get(key).and_then(Value::as_str) {
            let path = Path::new(value);
            assert!(
                !path.exists(),
                "publication path {key}={value} should be removed after success"
            );
        }
    }
}

#[test]
fn absent_root_publishes_atomically_and_reports_published() {
    let root = temp_root("absent");
    let (ok, stdout, stderr) = run_init(&root, "gtm", false, &[]);
    assert!(ok, "stderr: {stderr}");
    let payload = data_envelope(&stdout);
    let publication = payload
        .get("publication")
        .expect("publication envelope should be present");
    assert_eq!(publication["status"], "published");
    assert_eq!(publication["mode"], "atomic-directory-rename");
    assert_eq!(publication["atomic"], true);
    assert_published_tree(&root);
    assert_publication_paths_clean(&publication);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn existing_root_publishes_through_rollback_protected_merge() {
    let root = temp_root("existing");
    let (ok, _stdout, stderr) = run_init(&root, "gtm", false, &[]);
    assert!(ok, "first init should succeed: {stderr}");
    // Capture pre-publication bytes for a generated file we will be
    // replacing through the merge path.
    let readme_path = root.join(".mdp/README.md");
    let original = fs::read_to_string(&readme_path).expect("readme should be readable");

    // Replace the existing pack with custom human-authored content
    // for the manifest to make sure rollback-protected merge can
    // replace generated files while preserving the tree structure.
    let manifest_path = root.join(".mdp/manifest.yaml");
    fs::write(&manifest_path, "# Human authored manifest\n").expect("manifest writable");
    fs::write(&readme_path, "# Human authored README\n").expect("readme writable");

    let (ok, stdout, stderr) = run_init(&root, "gtm", true, &[]);
    assert!(ok, "force re-init should succeed: {stderr}");
    let payload = data_envelope(&stdout);
    let publication = payload
        .get("publication")
        .expect("publication envelope should be present");
    assert_eq!(publication["status"], "published");
    assert_eq!(publication["mode"], "rollback-protected-merge");
    assert_eq!(publication["atomic"], false);
    assert_published_tree(&root);
    assert_publication_paths_clean(&publication);
    let _ = fs::remove_dir_all(&root);
    let _ = original;
}

#[test]
fn late_example_collision_preserves_destination_byte_for_byte() {
    let root = temp_root("late-collision");
    fs::create_dir_all(&root).expect("root should be creatable");
    let examples = root.join("examples");
    fs::create_dir_all(&examples).expect("examples dir should be creatable");
    let prospect = examples.join("clay-row.json");
    let sentinel = "{\"sentinel\":\"preserved\"}\n";
    fs::write(&prospect, sentinel).expect("prospect sentinel should be writable");
    // Touch a few non-generated files to make sure they are not modified.
    let unrelated = root.join("README.md");
    let unrelated_text = "# unrelated user file\n";
    fs::write(&unrelated, unrelated_text).expect("unrelated file should be writable");

    let (ok, stdout, _stderr) = run_init(&root, "gtm", false, &[]);
    assert!(!ok, "init must fail when a generated example collides");
    assert!(
        stdout.contains("clay-row.json") || stdout.contains("not published"),
        "stdout should mention the late collision or the not-published state, got: {stdout}"
    );
    assert_eq!(
        fs::read_to_string(&prospect).expect("sentinel should be preserved"),
        sentinel
    );
    assert_eq!(
        fs::read_to_string(&unrelated).expect("unrelated file should be preserved"),
        unrelated_text
    );
    assert!(
        !root.join(".mdp/manifest.yaml").exists(),
        "manifest should not be written"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn force_collision_preserves_destination_byte_for_byte_without_force() {
    // The early README collision (and the late example collision) must
    // both leave the destination unchanged when the caller did not pass
    // --force.
    let root = temp_root("early-collision");
    fs::create_dir_all(root.join(".mdp")).expect("pack dir should be creatable");
    let readme = root.join(".mdp/README.md");
    let sentinel = "# Human authored README\n";
    fs::write(&readme, sentinel).expect("readme should be writable");

    let (ok, stdout, _stderr) = run_init(&root, "gtm", false, &[]);
    assert!(!ok, "init must fail on README collision");
    assert!(
        stdout.contains("README.md") || stdout.contains("not published"),
        "stdout should reference the README or the not-published state, got: {stdout}"
    );
    assert_eq!(
        fs::read_to_string(&readme).expect("readme should be preserved"),
        sentinel
    );
    assert!(!root.join(".mdp/manifest.yaml").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn dry_run_does_not_touch_destination_or_create_staging() {
    let root = temp_root("dry-run");
    let (ok, stdout, stderr) = run_init(&root, "gtm", false, &["--dry-run"]);
    assert!(ok, "dry run should succeed: {stderr}");
    let payload = data_envelope(&stdout);
    assert_eq!(payload["dry_run"], true);
    let publication = payload
        .get("publication")
        .expect("publication envelope should be present");
    assert_eq!(publication["status"], "dry-run");
    assert_eq!(publication["mode"], "atomic-directory-rename");
    assert_eq!(publication["atomic"], true);
    assert!(!root.exists(), "dry run must not create the destination");
    assert_publication_paths_clean(&publication);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn dry_run_reports_blocked_action_for_existing_example() {
    let root = temp_root("dry-run-blocked");
    let examples = root.join("examples");
    fs::create_dir_all(&examples).expect("examples dir should be creatable");
    fs::write(examples.join("clay-row.json"), "{}").expect("sentinel should be writable");
    let (ok, stdout, stderr) = run_init(&root, "gtm", false, &["--dry-run"]);
    assert!(ok, "dry run should still report a plan: {stderr}");
    let payload = data_envelope(&stdout);
    let write_plan = payload["write_plan"]
        .as_array()
        .expect("write_plan should be an array");
    let blocked = write_plan
        .iter()
        .find(|entry| entry["path"] == examples.join("clay-row.json").display().to_string())
        .expect("clay-row plan entry should be present");
    assert_eq!(blocked["action"], "blocked");
    assert_eq!(blocked["would_write"], false);
    assert!(!root.join(".mdp/manifest.yaml").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn proposal_init_publishes_atomic_directory_rename() {
    let root = temp_root("proposal-absent");
    let (ok, stdout, stderr) = run_init(&root, "proposal", false, &[]);
    assert!(ok, "proposal init should succeed: {stderr}");
    let payload = data_envelope(&stdout);
    let publication = payload
        .get("publication")
        .expect("publication envelope should be present");
    assert_eq!(publication["status"], "published");
    assert_eq!(publication["mode"], "atomic-directory-rename");
    assert_eq!(publication["atomic"], true);
    assert!(root.join(".mdp/manifest.yaml").exists());
    assert!(root.join(".mdp/briefs").is_dir());
    assert!(root.join(".mdp/README.md").exists());
    assert!(
        root.join("examples/proof-output/compliance-row.json")
            .exists()
    );
    assert_publication_paths_clean(&publication);
    let _ = fs::remove_dir_all(&root);
}
