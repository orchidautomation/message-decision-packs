use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_mdp")
}

fn run(args: &[&str]) -> Output {
    Command::new(bin())
        .current_dir(repo_root())
        .args(args)
        .output()
        .expect("mdp should run")
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn json(args: &[&str]) -> (Output, Value) {
    let output = run(args);
    let value = serde_json::from_slice(&output.stdout).expect("one JSON envelope");
    (output, value)
}

fn temp_root(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("mdp-202-{label}-{nonce}"))
}

#[test]
fn bare_human_and_json_are_first_contact_paths() {
    let human = run(&[]);
    assert!(human.status.success());
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.contains("Author") && text.contains("Use"));
    assert!(text.contains("mdp status --dir PACK_ROOT"));
    assert!(human.stderr.is_empty());

    let (json_output, value) = json(&["--json"]);
    assert!(json_output.status.success());
    assert!(json_output.stderr.is_empty());
    assert_eq!(value["command"], "status");
    assert_eq!(value["data"]["contract"], "mdp.status.v1");
    assert_eq!(value["data"]["mode"], "local-offline");
    assert_eq!(value["data"]["auth_required"], false);
}

#[test]
fn status_is_observational_for_valid_missing_and_malformed_packs() {
    let valid = run(&["--json", "status", "--dir", "plugin/assets/templates/basic"]);
    assert!(valid.status.success());
    assert!(valid.stderr.is_empty());
    let valid_value: Value = serde_json::from_slice(&valid.stdout).unwrap();
    assert_eq!(valid_value["data"]["health"]["state"], "ready");

    let missing_root = temp_root("missing");
    let (missing, missing_value) =
        json(&["--json", "status", "--dir", missing_root.to_str().unwrap()]);
    assert!(missing.status.success());
    assert!(missing.stderr.is_empty());
    assert_eq!(missing_value["data"]["health"]["state"], "needs-input");
    assert_eq!(missing_value["data"]["manifest_observed"], false);
    assert!(!missing_root.exists());

    let malformed_root = temp_root("malformed");
    fs::create_dir_all(malformed_root.join(".mdp")).unwrap();
    fs::write(
        malformed_root.join(".mdp/manifest.yaml"),
        "format: [not valid",
    )
    .unwrap();
    let (malformed, malformed_value) = json(&[
        "--json",
        "status",
        "--dir",
        malformed_root.to_str().unwrap(),
    ]);
    assert!(malformed.status.success());
    assert!(malformed.stderr.is_empty());
    assert_eq!(malformed_value["data"]["health"]["state"], "invalid");
    assert_eq!(malformed_value["data"]["manifest_observed"], true);
    assert_eq!(
        fs::read_to_string(malformed_root.join(".mdp/manifest.yaml")).unwrap(),
        "format: [not valid"
    );
    fs::remove_dir_all(malformed_root).unwrap();
}

#[test]
fn summaries_are_concise_and_json_omits_null_objects_recursively() {
    let human = run(&[
        "--summary",
        "validate",
        "--dir",
        "plugin/assets/templates/basic",
    ]);
    assert!(human.status.success());
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.lines().next().unwrap().starts_with("validate:"));
    assert!(!text.contains("\"issues\": ["));

    let (output, value) = json(&[
        "--json",
        "--summary",
        "validate",
        "--dir",
        "plugin/assets/templates/basic",
    ]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(value["summary"].get("strict").is_none());
    assert_eq!(value["summary"]["valid"], true);
    assert!(value["summary"]["error_count"].is_number());
    assert!(value["summary"]["issue_count"].is_number());
}

#[test]
fn representative_human_output_is_not_raw_pretty_json() {
    let output = run(&["skills", "--dir", "plugin/assets/templates/basic"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("skills:"));
    assert!(!text.starts_with('{'));
}

#[test]
fn help_and_capabilities_expose_grouping_status_and_canonical_options() {
    let help = run(&["--help"]);
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("Start") && text.contains("Inspect") && text.contains("Decide"));
    assert!(text.contains("Produce/Verify") && text.contains("Advanced"));

    let check_help = run(&["check", "--help"]);
    let check_text = String::from_utf8(check_help.stdout).unwrap();
    assert!(check_text.contains("Exact canonical jobs[].id"));
    assert!(check_text.contains("PACK_ROOT"));
    let requirements_help = run(&["requirements", "--help"]);
    let requirements_text = String::from_utf8(requirements_help.stdout).unwrap();
    assert!(requirements_text.contains("Exact canonical jobs[].id"));

    let (output, value) = json(&["--json", "capabilities"]);
    assert!(output.status.success());
    let status = value["data"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "status")
        .unwrap();
    assert_eq!(status["output_contract"], "mdp.status.v1");
    assert_eq!(status["side_effects"], "read-only-observational");
}

#[allow(dead_code)]
fn _path_display(path: &Path) -> String {
    path.display().to_string()
}
