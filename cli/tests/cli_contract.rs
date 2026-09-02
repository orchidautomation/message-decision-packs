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

#[test]
fn temporal_health_command_accepts_explicit_as_of() {
    let help = run(&["temporal-health", "--help"]);
    assert!(help.status.success());
    let help_text = String::from_utf8(help.stdout).unwrap();
    assert!(help_text.contains("--as-of") && help_text.contains("strict UTC"));

    let output = run(&[
        "temporal-health",
        "--dir",
        "plugin/assets/templates/basic",
        "--as-of",
        "2026-09-02T00:00:00Z",
    ]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("temporal health evaluated at 2026-09-02T00:00:00Z"));

    let invalid = run(&[
        "temporal-health",
        "--dir",
        "plugin/assets/templates/basic",
        "--as-of",
        "not-a-timestamp",
    ]);
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(
        String::from_utf8(invalid.stderr)
            .unwrap()
            .contains("strict UTC timestamp")
    );

    let schema = run(&["--json", "schema", "temporal-health-v1"]);
    assert!(schema.status.success());
    assert!(schema.stderr.is_empty());
    let schema_value: Value = serde_json::from_slice(&schema.stdout).unwrap();
    assert_eq!(
        schema_value["data"]["properties"]["contract"]["const"],
        "mdp.temporal-health.v1"
    );
}

#[test]
fn temporal_health_human_output_lists_diagnostics_before_next() {
    let root = temp_root("temporal-diagnostic-human");
    let init = run(&[
        "init",
        "--name",
        "Temporal diagnostic fixture",
        "--template",
        "gtm",
        "--target-name",
        "Example Company",
        "--dir",
        root.to_str().unwrap(),
    ]);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    fs::write(
        root.join(".mdp/sources.yaml"),
        "format: mdp.sources.v0\nsources:\n- id: source\n  temporal:\n    observed_at: not-a-timestamp\n",
    )
    .unwrap();
    let output = run(&[
        "temporal-health",
        "--dir",
        root.to_str().unwrap(),
        "--as-of",
        "2026-09-02T00:00:00Z",
    ]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    let diagnostic = "temporal_timestamp_invalid_or_future at .mdp/sources.yaml#/sources/0/temporal/observed_at:";
    assert!(text.contains(diagnostic));
    assert!(text.find(diagnostic).unwrap() < text.find("Next:").unwrap());
    fs::remove_dir_all(root).unwrap();
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

    let doctor = run(&[
        "--summary",
        "doctor",
        "--dir",
        "plugin/assets/templates/basic",
    ]);
    assert!(doctor.status.success());
    let doctor_text = String::from_utf8(doctor.stdout).unwrap();
    assert!(doctor_text.starts_with("doctor: ready"));
    assert!(doctor_text.contains("error count: 0"));
    assert!(doctor_text.contains("Next:"));
    assert!(!doctor_text.contains("unknown"));
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
    assert!(!text.contains("__secure-install"));
    let sections = [
        ("Start:", "init"),
        ("Inspect:", "status"),
        ("Decide:", "check"),
        ("Produce/Verify:", "brief"),
        ("Advanced:", "conformance"),
    ];
    for (index, (heading, command)) in sections.iter().enumerate() {
        let section_start = text.find(heading).unwrap();
        let section_end = sections
            .get(index + 1)
            .and_then(|(next_heading, _)| text.find(next_heading))
            .unwrap_or(text.len());
        let section = &text[section_start..section_end];
        assert!(
            section.contains(&format!("  {command}")),
            "{command} should be listed under {heading}, not merely elsewhere in help"
        );
    }
    assert!(text.contains("Quickstart: mdp init --dir PACK_ROOT --name NAME"));
    assert!(text.contains("mdp check --dir PACK_ROOT --job JOB_ID"));

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

#[test]
fn capabilities_report_reverse_declared_trace_conflicts() {
    let (output, value) = json(&["--json", "capabilities"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let trace = value["data"]["cli"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|command| command["path"] == serde_json::json!(["trace"]))
        .unwrap();
    let conflicts = |canonical: &str| {
        trace["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .find(|argument| argument["canonical"] == canonical)
            .unwrap()["conflicts_with"]
            .clone()
    };

    assert_eq!(conflicts("--bundle"), serde_json::json!(["--file"]));
    assert_eq!(
        conflicts("--artifact-root"),
        serde_json::json!(["--dir", "--prompt-output", "--validation-input"])
    );

    let rejected_source_pair = run(&[
        "trace",
        "--file",
        "result.json",
        "--bundle",
        "bundle.json",
        "--receipt",
        "receipt.json",
    ]);
    assert_eq!(rejected_source_pair.status.code(), Some(2));

    let rejected_authority_pair = run(&[
        "trace",
        "--file",
        "result.json",
        "--dir",
        ".",
        "--prompt-output",
        "prompt-output.json",
        "--artifact-root",
        ".",
    ]);
    assert_eq!(rejected_authority_pair.status.code(), Some(2));
}
