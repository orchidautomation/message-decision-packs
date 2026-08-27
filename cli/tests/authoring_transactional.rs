//! Black-box proof for previewable, failure-safe multi-file pack authoring.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
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
    let mut command = Command::new(binary());
    command.arg("--json").args(args);
    if let Some(boundary) = fault_after {
        command.env("MDP_TEST_AUTHOR_FAULT_AFTER", boundary.to_string());
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
    run(
        &[
            "author",
            "preview",
            "--dir",
            live.to_str().unwrap(),
            "--candidate",
            candidate.to_str().unwrap(),
            "--out",
            plan.to_str().unwrap(),
        ],
        None,
    )
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
