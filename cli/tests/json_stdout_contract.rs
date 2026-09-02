//! Process-level exhaustive JSON stdout contract test.
//!
//! Every invocation containing global `--json` must write exactly one parseable
//! JSON value to stdout, with empty stderr. The matrix below exercises every
//! public presentation selector/value combination plus a representative set of
//! success and failure paths.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn mdp_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mdp"))
}

#[derive(Debug, Clone, Copy)]
enum Case {
    Ok,
    Conflict,
}

fn run(args: &[&str], case: Case) -> (i32, String, String, Option<serde_json::Value>) {
    let output = Command::new(mdp_bin())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("mdp should run for the contract matrix");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit_code = output.status.code().unwrap_or(-1);

    let parsed = serde_json::from_str::<serde_json::Value>(stdout.trim());
    if matches!(case, Case::Ok) {
        assert!(
            parsed.is_ok(),
            "expected one JSON value on stdout for {args:?}, got: {stdout:?}"
        );
        assert!(
            stderr.is_empty(),
            "expected empty stderr for {args:?}, got: {stderr:?}"
        );
    } else {
        assert!(
            parsed.is_ok(),
            "expected one JSON value on stdout for conflict {args:?}, got: {stdout:?}"
        );
        assert!(
            stderr.is_empty(),
            "expected empty stderr for conflict {args:?}, got: {stderr:?}"
        );
        let value = parsed.as_ref().expect("conflict envelope parses");
        assert_eq!(value["ok"], serde_json::json!(false));
        assert_eq!(
            value["error"]["code"], "output_mode_conflict",
            "conflict {args:?} must use stable code"
        );
    }
    (exit_code, stdout, stderr, parsed.ok())
}

#[allow(dead_code)]
fn assert_ok(args: &[&str]) {
    let (code, _stdout, _stderr, value) = run(args, Case::Ok);
    assert_eq!(code, 0, "expected exit 0 for {args:?}");
    let value = value.expect("json parsed");
    assert_eq!(
        value["ok"],
        serde_json::json!(true),
        "ok envelope expected for {args:?}"
    );
}

fn assert_conflict(args: &[&str], expected_selector: &str, expected_value: &str) {
    let (code, _stdout, _stderr, value) = run(args, Case::Conflict);
    assert_eq!(code, 1, "expected exit 1 for {args:?}");
    let value = value.expect("json parsed");
    assert_eq!(value["ok"], serde_json::json!(false));
    assert_eq!(value["error"]["code"], "output_mode_conflict");
    let details = value["error"]["details"]
        .as_array()
        .expect("conflict details array");
    assert!(
        details
            .iter()
            .any(|d| d.as_str() == Some(expected_selector)),
        "details should include selector {expected_selector} for {args:?}, got {value}"
    );
    assert!(
        details.iter().any(|d| d.as_str() == Some(expected_value)),
        "details should include value {expected_value} for {args:?}, got {value}"
    );
}

fn write_fixture(root: &std::path::Path, name: &str, body: serde_json::Value) -> PathBuf {
    let path = root.join(name);
    std::fs::write(&path, serde_json::to_string_pretty(&body).unwrap())
        .expect("fixture should be written");
    path
}

fn copy_dir_all(source: &std::path::Path, destination: &std::path::Path) {
    std::fs::create_dir_all(destination).expect("fixture destination");
    for entry in std::fs::read_dir(source).expect("fixture source") {
        let entry = entry.expect("fixture entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().expect("fixture file type").is_dir() {
            copy_dir_all(&source_path, &destination_path);
        } else {
            std::fs::copy(&source_path, &destination_path).expect("copy fixture file");
        }
    }
}

fn copy_tree(source: &std::path::Path, target: &std::path::Path) {
    std::fs::create_dir_all(target).expect("target directory should be created");
    for entry in std::fs::read_dir(source).expect("source directory should be readable") {
        let entry = entry.expect("source entry should be readable");
        let destination = target.join(entry.file_name());
        if entry
            .file_type()
            .expect("entry type should be readable")
            .is_dir()
        {
            copy_tree(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), destination).expect("fixture file should be copied");
        }
    }
}

fn temporary_root(label: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should follow epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("mdp-json-contract-{label}-{suffix}"))
}

struct TemporaryFixture {
    root: PathBuf,
}

impl TemporaryFixture {
    fn new(label: &str) -> Self {
        Self {
            root: temporary_root(label),
        }
    }

    fn path(&self) -> &std::path::Path {
        &self.root
    }
}

impl Drop for TemporaryFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn capabilities_envelope_is_one_parseable_json_value() {
    let (code, stdout, stderr, value) = run(&["--json", "capabilities"], Case::Ok);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let value = value.expect("parseable");
    assert_eq!(value["ok"], serde_json::json!(true));
    assert_eq!(value["command"], "capabilities");
    assert_eq!(value["data"]["contract"], "mdp.capabilities.v1");
    assert_eq!(value["data"]["cli"]["contract"], "mdp.cli-graph.v1");
    let skills = value["data"]["cli"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|command| command["path"] == serde_json::json!(["skills"]))
        .expect("skills command projection");
    let job = skills["arguments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|argument| argument["canonical"] == "--job")
        .expect("skills --job projection");
    assert_eq!(job["requires_when_present"], serde_json::json!(["--dir"]));
    assert!(
        value["data"]["presentation_contract"].is_object(),
        "capabilities should expose the presentation contract"
    );
    let doctor = value["data"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|command| command["name"] == "doctor")
        .expect("doctor command projection");
    assert_eq!(doctor["output_contract"], "mdp.doctor.v1");
    assert!(stdout.contains("presentation_contract"));
}

#[test]
fn all_unassessed_route_budget_summary_emits_schema_valid_explicit_null_headroom() {
    let root = std::env::temp_dir().join(format!(
        "mdp-route-budget-unassessed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../plugin/assets/templates/basic");
    copy_dir_all(&source, &root);

    let manifest_path = root.join(".mdp/manifest.yaml");
    let mut manifest: serde_yaml::Value = serde_yaml::from_str(
        &std::fs::read_to_string(&manifest_path).expect("read fixture manifest"),
    )
    .expect("parse fixture manifest");
    for job in manifest["jobs"].as_sequence_mut().expect("manifest jobs") {
        job.as_mapping_mut()
            .expect("job mapping")
            .remove(serde_yaml::Value::String("context_budget".to_string()));
    }
    std::fs::write(
        &manifest_path,
        serde_yaml::to_string(&manifest).expect("serialize unassessed manifest"),
    )
    .expect("write unassessed manifest");

    let root_arg = root.display().to_string();
    let (code, _, stderr, envelope) = run(
        &[
            "--json",
            "--summary",
            "route-budget",
            "--dir",
            root_arg.as_str(),
        ],
        Case::Ok,
    );
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let envelope = envelope.expect("route-budget summary envelope");
    let summary = &envelope["summary"];
    assert_eq!(summary["route_status_counts"]["unassessed"], 9);
    assert_eq!(summary["unassessed_generation_count"], 2);
    assert!(summary.get("tightest_headroom").is_some());
    assert!(summary["tightest_headroom"].is_null());
    assert!(summary["query"].get("job_id").is_none());
    assert!(summary["query"].get("persona").is_none());

    let (_, _, _, schema_envelope) =
        run(&["--json", "schema", "route-budget-summary-v1"], Case::Ok);
    let schema_envelope = schema_envelope.expect("route-budget summary schema envelope");
    jsonschema::draft202012::validate(&schema_envelope["data"], summary)
        .expect("real unassessed summary should satisfy the advertised schema");

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn doctor_exit_envelope_and_human_output_match_pack_validity() {
    let missing = std::env::temp_dir().join(format!(
        "mdp-doctor-contract-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let missing_arg = missing.display().to_string();
    let (code, _, stderr, value) = run(
        &["--json", "doctor", "--dir", missing_arg.as_str()],
        Case::Ok,
    );
    assert_eq!(code, 1);
    assert!(stderr.is_empty());
    let value = value.expect("doctor JSON envelope");
    assert_eq!(value["ok"], false);
    assert_eq!(value["data"]["status"], "pack-missing");
    assert!(value["data"]["error_count"].is_u64());
    assert!(value["data"]["warning_count"].is_u64());

    let (summary_code, _, _, summary) = run(
        &[
            "--json",
            "--summary",
            "doctor",
            "--dir",
            missing_arg.as_str(),
        ],
        Case::Ok,
    );
    assert_eq!(summary_code, 1);
    let summary = summary.expect("doctor summary envelope");
    assert_eq!(summary["ok"], false);
    assert!(summary["summary"]["error_count"].is_u64());
    assert!(summary["summary"]["warning_count"].is_u64());

    let output = Command::new(mdp_bin())
        .args(["doctor", "--dir", missing_arg.as_str()])
        .output()
        .expect("human doctor invocation");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("installation: ready"), "{stdout}");
    assert!(stdout.contains("pack validity: invalid"), "{stdout}");
    assert!(
        stdout.contains("profile activation: not-assessed"),
        "{stdout}"
    );
    assert!(stdout.contains("job readiness: not-assessed"), "{stdout}");

    let ready = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../plugin/assets/templates/basic")
        .display()
        .to_string();
    let (code, _, _, value) = run(&["--json", "doctor", "--dir", &ready], Case::Ok);
    assert_eq!(code, 0);
    let value = value.expect("ready doctor JSON envelope");
    assert_eq!(value["ok"], true);
    assert_eq!(value["data"]["status"], "ready");
    assert_eq!(value["data"]["job_readiness"]["state"], "not-assessed");
}

#[test]
fn actionable_diagnostic_is_shared_by_json_and_human_errors() {
    let args = ["init", "--bogus"];
    let (json_code, _, json_stderr, value) = run(&["--json", "init", "--bogus"], Case::Ok);
    assert_eq!(json_code, 2);
    assert!(json_stderr.is_empty());
    let value = value.expect("JSON error envelope");
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "invalid_argument");
    assert_eq!(value["diagnostic_contract"], "mdp.actionable-diagnostic.v1");
    let diagnostic = &value["actionable_diagnostics"][0];
    assert_eq!(diagnostic["phase"], "input");
    assert_eq!(diagnostic["code"], "invalid_argument");
    assert_eq!(diagnostic["retryability"], "after-user-action");
    assert_eq!(diagnostic["next_action"]["command"], "mdp --help");

    let output = Command::new(mdp_bin())
        .args(args)
        .output()
        .expect("human error invocation");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected = format!(
        "diagnostic [{} / {}]: {}",
        diagnostic["phase"].as_str().unwrap(),
        diagnostic["code"].as_str().unwrap(),
        diagnostic["summary"].as_str().unwrap()
    );
    assert!(stderr.contains(&expected), "human stderr: {stderr}");
    assert!(
        stderr.contains("next: mdp --help"),
        "human stderr: {stderr}"
    );
}

#[test]
fn prepare_run_argument_errors_share_the_actionable_code_across_presentations() {
    let (json_code, _, json_stderr, value) = run(
        &["--json", "prepare-run", "--definitely-unsupported"],
        Case::Ok,
    );
    assert_eq!(json_code, 2);
    assert!(json_stderr.is_empty());
    let value = value.expect("JSON prepare-run error envelope");
    assert_eq!(
        value["actionable_diagnostics"][0]["code"],
        "invalid_argument"
    );
    assert_eq!(value["diagnostic_contract"], "mdp.actionable-diagnostic.v1");
    assert!(value["data"].get("diagnostic_contract").is_none());
    assert!(value["data"].get("actionable_diagnostics").is_none());

    let (_, _, _, schema) = run(&["--json", "schema", "run-request-compile-v1"], Case::Ok);
    let schema = schema.expect("compile schema envelope");
    jsonschema::draft202012::validate(&schema["data"], &value["data"])
        .expect("argument-error data must satisfy the closed compile schema");

    let output = Command::new(mdp_bin())
        .args(["prepare-run", "--definitely-unsupported"])
        .output()
        .expect("human prepare-run error invocation");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("diagnostic [input / invalid_argument]"),
        "human stderr: {stderr}"
    );
}

#[test]
fn successful_target_command_advertises_empty_versioned_diagnostics() {
    let root = std::env::temp_dir().join(format!(
        "mdp-actionable-diagnostics-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("fixture root");
    let root_arg = root.display().to_string();
    let (code, _, stderr, value) = run(&["--json", "init", "--dir", root_arg.as_str()], Case::Ok);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let value = value.expect("init envelope");
    assert_eq!(value["diagnostic_contract"], "mdp.actionable-diagnostic.v1");
    assert_eq!(value["actionable_diagnostics"], serde_json::json!([]));
    assert!(
        value["data"].get("diagnostic_contract").is_none(),
        "transport diagnostics must not mutate the init domain contract"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn strict_validate_keeps_readme_mechanical_warning_advisory_in_json_and_exit() {
    let root = std::env::temp_dir().join(format!(
        "mdp-strict-readme-advisory-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create fixture root");
    let init = Command::new(mdp_bin())
        .args(["--json", "init", "--dir"])
        .arg(&root)
        .output()
        .expect("initialize fixture pack");
    assert!(init.status.success(), "init failed: {:?}", init.stdout);

    let readme_path = root.join(".mdp/README.md");
    let readme = std::fs::read_to_string(&readme_path).expect("read starter README");
    let edited = readme.replacen(
        "## Sources\n\n",
        "## Sources\n\n- `missing-advisory`: synthetic fixture\n",
        1,
    );
    assert_ne!(
        edited, readme,
        "fixture should edit the exact Sources section"
    );
    std::fs::write(&readme_path, edited).expect("write advisory reference");

    let output = Command::new(mdp_bin())
        .args(["--json", "validate", "--strict", "--dir"])
        .arg(&root)
        .output()
        .expect("strict validate fixture pack");
    assert_eq!(output.status.code(), Some(0), "stdout: {:?}", output.stdout);
    assert!(output.stderr.is_empty());
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("one JSON validation envelope");
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["valid"], true);
    assert_eq!(envelope["data"]["strict"]["warning_count"], 0);
    assert!(envelope["data"]["issues"].as_array().is_some_and(|issues| {
        issues.iter().any(|issue| {
            issue["code"] == "readme_human_source_reference_missing"
                && issue["authority"] == "non-authoritative-mechanical-warning"
        })
    }));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn validate_and_strict_fail_closed_on_malformed_readme_markers() {
    let root = std::env::temp_dir().join(format!(
        "mdp-validate-readme-markers-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create fixture root");
    let init = Command::new(mdp_bin())
        .args(["--json", "init", "--dir"])
        .arg(&root)
        .output()
        .expect("initialize fixture pack");
    assert!(init.status.success(), "init failed: {:?}", init.stdout);

    let readme_path = root.join(".mdp/README.md");
    let mut readme = std::fs::read_to_string(&readme_path).expect("read starter README");
    readme.push_str("<!-- mdp:readme-ownership v1 begin -->");
    std::fs::write(&readme_path, readme).expect("write malformed marker layout");

    for strict in [false, true] {
        let mut command = Command::new(mdp_bin());
        command.args(["--json", "validate"]);
        if strict {
            command.arg("--strict");
        }
        let output = command
            .arg("--dir")
            .arg(&root)
            .output()
            .expect("validate fixture pack");
        assert_eq!(output.status.code(), Some(1), "stdout: {:?}", output.stdout);
        assert!(output.stderr.is_empty());
        let envelope: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("one JSON validation envelope");
        assert_eq!(envelope["ok"], true);
        assert_eq!(envelope["data"]["valid"], false);
        assert!(envelope["data"]["issues"].as_array().is_some_and(|issues| {
            issues.iter().any(|issue| {
                issue["code"] == "readme_marker_layout_invalid" && issue["severity"] == "error"
            })
        }));
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ownership_legend_drift_blocks_check_and_strict_but_remains_warning_first() {
    let root = std::env::temp_dir().join(format!(
        "mdp-readme-ownership-drift-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create fixture root");
    let init = Command::new(mdp_bin())
        .args(["--json", "init", "--dir"])
        .arg(&root)
        .output()
        .expect("initialize fixture pack");
    assert!(init.status.success(), "init failed: {:?}", init.stdout);

    let readme_path = root.join(".mdp/README.md");
    let readme = std::fs::read_to_string(&readme_path).expect("read starter README");
    let edited = readme.replacen(
        "Machine-owned: this ownership legend",
        "Machine-owned: edited ownership legend",
        1,
    );
    assert_ne!(edited, readme, "fixture must edit the owned legend");
    std::fs::write(&readme_path, edited).expect("write edited legend");

    let check = Command::new(mdp_bin())
        .args(["--json", "readme", "check", "--dir"])
        .arg(&root)
        .output()
        .expect("readme check");
    assert_eq!(check.status.code(), Some(1), "stdout: {:?}", check.stdout);
    assert!(check.stderr.is_empty());
    let check_envelope: serde_json::Value =
        serde_json::from_slice(&check.stdout).expect("readme check JSON");
    assert_eq!(check_envelope["data"]["status"], "stale");
    assert_eq!(
        check_envelope["data"]["changed_generated_regions"],
        serde_json::json!(["ownership"])
    );
    assert_eq!(
        check_envelope["data"]["diagnostics"][0]["code"],
        "readme_inventory_drift"
    );

    for (strict, expected_exit, expected_valid) in [(false, 0, true), (true, 1, false)] {
        let mut command = Command::new(mdp_bin());
        command.args(["--json", "validate"]);
        if strict {
            command.arg("--strict");
        }
        let output = command
            .arg("--dir")
            .arg(&root)
            .output()
            .expect("validate fixture pack");
        assert_eq!(output.status.code(), Some(expected_exit));
        assert!(output.stderr.is_empty());
        let envelope: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("validate JSON");
        assert_eq!(envelope["data"]["valid"], expected_valid);
        assert!(envelope["data"]["issues"].as_array().is_some_and(|issues| {
            issues.iter().any(|issue| {
                issue["code"] == "readme_inventory_drift" && issue["severity"] == "warning"
            })
        }));
        if strict {
            assert_eq!(envelope["data"]["strict"]["warning_count"], 1);
            assert_eq!(
                envelope["data"]["strict_warnings"][0]["code"],
                "readme_inventory_drift"
            );
        }
    }

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn partial_readme_regions_are_stale_and_strict_blocked_in_both_directions() {
    let cases = [
        (
            "ownership",
            "<!-- mdp:readme-inventory v1 begin -->",
            "<!-- mdp:readme-inventory v1 end -->",
            "inventory",
        ),
        (
            "inventory",
            "<!-- mdp:readme-ownership v1 begin -->",
            "<!-- mdp:readme-ownership v1 end -->",
            "ownership",
        ),
    ];
    for (kept, remove_begin, remove_end, missing) in cases {
        let root = std::env::temp_dir().join(format!(
            "mdp-readme-partial-{kept}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create fixture root");
        let init = Command::new(mdp_bin())
            .args(["--json", "init", "--dir"])
            .arg(&root)
            .output()
            .expect("initialize fixture pack");
        assert!(init.status.success(), "init failed: {:?}", init.stdout);

        let readme_path = root.join(".mdp/README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("read starter README");
        let begin = readme.find(remove_begin).expect("begin marker");
        let end_marker =
            readme[begin..].find(remove_end).expect("end marker") + begin + remove_end.len();
        let end = if readme.as_bytes().get(end_marker) == Some(&b'\n') {
            end_marker + 1
        } else {
            end_marker
        };
        let partial = format!("{}{}", &readme[..begin], &readme[end..]);
        std::fs::write(&readme_path, partial).expect("write partial README");

        let check = Command::new(mdp_bin())
            .args(["--json", "readme", "check", "--dir"])
            .arg(&root)
            .output()
            .expect("readme check");
        assert_eq!(check.status.code(), Some(1), "kept {kept}");
        assert!(check.stderr.is_empty());
        let check_envelope: serde_json::Value =
            serde_json::from_slice(&check.stdout).expect("readme check JSON");
        assert_eq!(check_envelope["data"]["status"], "stale");
        assert_eq!(check_envelope["data"]["valid"], false);
        assert_eq!(
            check_envelope["data"]["generated_region_sha256"][missing]["actual"],
            serde_json::Value::Null
        );
        assert!(check_envelope["data"]["generated_region_sha256"][kept]["actual"].is_string());

        let strict = Command::new(mdp_bin())
            .args(["--json", "validate", "--strict", "--dir"])
            .arg(&root)
            .output()
            .expect("strict validate");
        assert_eq!(strict.status.code(), Some(1), "kept {kept}");
        assert!(strict.stderr.is_empty());
        let strict_envelope: serde_json::Value =
            serde_json::from_slice(&strict.stdout).expect("strict validate JSON");
        assert_eq!(strict_envelope["data"]["valid"], false);
        assert!(
            strict_envelope["data"]["strict_warnings"]
                .as_array()
                .is_some_and(|warnings| warnings
                    .iter()
                    .any(|warning| warning["code"] == "readme_inventory_drift"))
        );
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn help_and_version_wrap_as_one_parseable_json_value() {
    let (help_code, _, help_stderr, help_value) = run(&["--json", "--help"], Case::Ok);
    assert_eq!(help_code, 0);
    assert!(help_stderr.is_empty());
    let help = help_value.expect("help parses");
    assert_eq!(help["ok"], serde_json::json!(true));
    assert_eq!(help["command"], "help");
    assert!(help["data"]["text"].is_string());

    let (version_code, _, version_stderr, version_value) = run(&["--json", "--version"], Case::Ok);
    assert_eq!(version_code, 0);
    assert!(version_stderr.is_empty());
    let version = version_value.expect("version parses");
    assert_eq!(version["ok"], serde_json::json!(true));
    assert_eq!(version["command"], "version");
    assert!(version["data"]["text"].is_string());
}

#[test]
fn help_after_subcommand_wraps_as_one_parseable_json_value() {
    let (code, _, stderr, value) = run(&["--json", "trace", "--help"], Case::Ok);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let value = value.expect("trace help parses");
    assert_eq!(value["ok"], serde_json::json!(true));
    assert_eq!(value["command"], "help");
    // The help text must be scoped to the requested subcommand, not the root.
    let text = value["data"]["text"]
        .as_str()
        .expect("help text is a string");
    assert!(
        text.contains("mdp trace [OPTIONS]"),
        "subcommand help should describe the trace subcommand, got: {text}"
    );
    assert!(
        !text.contains("Usage: mdp [OPTIONS] <COMMAND>"),
        "subcommand help should not be the root help, got: {text}"
    );
}

#[test]
fn decision_card_global_json_wins_and_markdown_conflicts_explicitly() {
    let root = std::env::temp_dir().join(format!(
        "mdp-decision-card-json-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let fixture = write_fixture(
        &root,
        "fit.json",
        serde_json::json!({
            "contract": "mdp.fit.v0",
            "status": "fit",
            "context": {"missing_requirements": [], "invalid_requirements": []},
            "matches": [{"id": "rule-one"}],
            "disqualifiers": []
        }),
    );
    let fixture = fixture.to_string_lossy().into_owned();
    let (code, _, stderr, value) = run(&["--json", "decision-card", "--file", &fixture], Case::Ok);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let value = value.unwrap();
    assert_eq!(value["command"], "decision-card");
    assert_eq!(value["data"]["contract"], "mdp.decision-card.v1");

    let (code, _, stderr, value) = run(
        &["decision-card", "--file", &fixture, "--format", "json"],
        Case::Ok,
    );
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert_eq!(value.unwrap()["data"]["contract"], "mdp.decision-card.v1");

    let human_summary = Command::new(mdp_bin())
        .args(["--summary", "decision-card", "--file", &fixture])
        .output()
        .expect("human summary should run");
    assert!(human_summary.status.success());
    let human_stdout = String::from_utf8_lossy(&human_summary.stdout);
    assert!(human_stdout.contains("decision-card: available"));
    assert!(serde_json::from_str::<serde_json::Value>(&human_stdout).is_err());

    let (code, _, _, value) = run(
        &[
            "--summary",
            "decision-card",
            "--file",
            &fixture,
            "--format",
            "json",
        ],
        Case::Ok,
    );
    assert_eq!(code, 0);
    assert!(value.unwrap().get("summary").is_some());

    std::fs::write(root.join("bad-conformance.json"), b"{not-json").unwrap();
    let root_arg = root.to_string_lossy().into_owned();
    let (code, _, stderr, value) = run(
        &[
            "--json",
            "decision-card",
            "--file",
            "bad-conformance.json",
            "--artifact-root",
            &root_arg,
        ],
        Case::Ok,
    );
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let value = value.unwrap();
    assert_eq!(value["data"]["status"], "unavailable");
    assert_eq!(value["data"]["decision"]["action_gate"], "unavailable");
    assert!(!value.to_string().contains("bad-conformance"));

    assert_conflict(
        &[
            "--json",
            "decision-card",
            "--file",
            &fixture,
            "--format",
            "markdown",
        ],
        "--format",
        "markdown",
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn help_for_nested_subcommand_wraps_as_one_parseable_json_value() {
    // Deep subcommand help (e.g. `conformance compile --help`) must also be
    // scoped to the requested subcommand.
    let (code, _, stderr, value) = run(&["--json", "conformance", "compile", "--help"], Case::Ok);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let value = value.expect("conformance compile help parses");
    assert_eq!(value["ok"], serde_json::json!(true));
    assert_eq!(value["command"], "help");
    let text = value["data"]["text"]
        .as_str()
        .expect("help text is a string");
    assert!(
        text.contains("mdp conformance compile [OPTIONS]"),
        "subcommand help should describe conformance compile, got: {text}"
    );
}

#[test]
fn conflict_matrix_emits_stable_envelope_with_empty_stderr() {
    let root = std::env::temp_dir().join(format!(
        "mdp-json-stdout-contract-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&root).expect("temp root");
    // Use arbitrary absolute paths for selectors that require --file or
    // --dir. The gate runs before any command work, so the file contents
    // and pack state are not consulted.
    let draft = write_fixture(
        &root,
        "proof-output-draft.json",
        serde_json::json!({
            "contract": "mdp.proof-output-draft.v0",
            "pack_id": "fixture-pack",
            "release_id": "fixture-release",
            "segments": []
        }),
    );
    let fit = write_fixture(&root, "fit.json", serde_json::json!({}));
    let prospect = write_fixture(
        &root,
        "prospect.json",
        serde_json::json!({
            "contract": "mdp.prospect.v0",
            "name": "Example Person",
            "company": "Example Co"
        }),
    );
    let rendered = write_fixture(&root, "render.json", serde_json::json!({}));
    let draft_str = draft.display().to_string();
    let fit_str = fit.display().to_string();
    let prospect_str = prospect.display().to_string();
    let rendered_str = rendered.display().to_string();
    let root_str = root.display().to_string();

    // (selector, expected_value) pairs the matrix covers.
    let cases: &[(&[&str], &str, &str)] = &[
        (
            &[
                "--json", "trace", "--file", "fit.json", "--format", "mermaid",
            ],
            "--format",
            "mermaid",
        ),
        (
            &[
                "--json",
                "verify-output",
                "--dir",
                ".",
                "--file",
                "x.json",
                "--readable",
            ],
            "--readable",
            "true",
        ),
        (
            &[
                "--json",
                "render-brief",
                "--file",
                "x.json",
                "--template",
                "default",
                "--format",
                "markdown",
            ],
            "--format",
            "markdown",
        ),
        (
            &[
                "--json",
                "sample-leads",
                "--dir",
                ".",
                "--persona",
                "PMM",
                "--format",
                "yaml",
            ],
            "--format",
            "yaml",
        ),
        (
            &[
                "--json",
                "brief",
                "--dir",
                ".",
                "--prospect",
                "x.json",
                "--readable",
            ],
            "--readable",
            "true",
        ),
    ];

    for (raw_args, expected_selector, expected_value) in cases {
        let mut absolute_args: Vec<String> =
            raw_args.iter().map(|value| (*value).to_string()).collect();
        // Reshape "." / "x.json" / "fit.json" to absolute paths.
        let mut index = 0;
        while index < absolute_args.len() {
            if absolute_args[index] == "." {
                absolute_args[index] = root_str.clone();
            } else if absolute_args[index] == "x.json" {
                absolute_args[index] =
                    if raw_args.contains(&"--readable") && raw_args.contains(&"verify-output") {
                        draft_str.clone()
                    } else if raw_args.contains(&"render-brief") {
                        rendered_str.clone()
                    } else {
                        prospect_str.clone()
                    };
            } else if absolute_args[index] == "fit.json" {
                absolute_args[index] = fit_str.clone();
            }
            index += 1;
        }
        let arg_refs: Vec<&str> = absolute_args.iter().map(String::as_str).collect();
        assert_conflict(&arg_refs, expected_selector, expected_value);
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn json_compatible_selectors_pass_the_gate() {
    let cases: &[&[&str]] = &[
        &["--json", "capabilities"],
        &["--json", "--summary", "capabilities"],
        // trace --format=json is JSON compatible (the runtime still allows
        // summary envelopes for routes).
        &["--json", "trace", "--file", "x.json", "--format", "json"],
        // render-brief --format=json is JSON compatible.
        &[
            "--json",
            "render-brief",
            "--file",
            "x.json",
            "--template",
            "default",
            "--format",
            "json",
        ],
        // sample-leads --format=json is JSON compatible.
        &[
            "--json",
            "sample-leads",
            "--dir",
            ".",
            "--persona",
            "PMM",
            "--format",
            "json",
        ],
    ];
    for args in cases {
        let (code, _stdout, _stderr, value) = run(args, Case::Ok);
        let parsed = value.expect("ok json parses");
        assert_ne!(
            parsed["error"]["code"], "output_mode_conflict",
            "gate must not block JSON-compatible selectors: {args:?}"
        );
        let _ = code;
    }
}

#[test]
fn invalid_argument_in_json_mode_writes_one_json_envelope() {
    let (code, _stdout, _stderr, value) = run(&["--json", "trace", "--bogus"], Case::Ok);
    let parsed = value.expect("clap error parses as JSON");
    assert_eq!(parsed["ok"], serde_json::json!(false));
    assert_ne!(parsed["error"]["code"], "output_mode_conflict");
    let _ = code;
}

#[test]
fn unknown_subcommand_in_json_mode_writes_one_json_envelope() {
    let (code, _stdout, _stderr, value) = run(&["--json", "definitely-not-a-command"], Case::Ok);
    let parsed = value.expect("unknown subcommand parses as JSON");
    assert_eq!(parsed["ok"], serde_json::json!(false));
    assert_ne!(parsed["error"]["code"], "output_mode_conflict");
    let _ = code;
}

#[test]
fn prepare_run_blocked_envelope_preserves_single_json_value() {
    let (code, stdout, _stderr, value) = run(
        &[
            "--json",
            "prepare-run",
            "--job",
            "missing-job",
            "--model",
            "test-model",
        ],
        Case::Ok,
    );
    let parsed = value.expect("prepare-run blocked parses as JSON");
    assert_eq!(parsed["ok"], serde_json::json!(false));
    assert_eq!(parsed["command"], "prepare-run");
    assert_eq!(parsed["data"]["contract"], "mdp.run-request-compile.v1");
    assert_eq!(parsed["data"]["status"], "blocked");
    let blocked_code = parsed["data"]["diagnostics"][0]["code"]
        .as_str()
        .expect("bounded blocked diagnostic code");
    assert_eq!(
        parsed["diagnostic_contract"],
        "mdp.actionable-diagnostic.v1"
    );
    assert_eq!(parsed["actionable_diagnostics"][0]["code"], blocked_code);
    assert!(parsed["data"].get("diagnostic_contract").is_none());
    assert!(parsed["data"].get("actionable_diagnostics").is_none());

    let (_, _, _, schema) = run(&["--json", "schema", "run-request-compile-v1"], Case::Ok);
    let schema = schema.expect("compile schema envelope");
    jsonschema::draft202012::validate(&schema["data"], &parsed["data"])
        .expect("blocked data must satisfy the closed compile schema");
    assert!(!stdout.contains("error:"));
    let _ = code;
}

#[test]
fn check_envelope_classifies_missing_readiness_authority_as_stable() {
    let fixture = TemporaryFixture::new("readiness-authority");
    let root = fixture.path();
    let template =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../plugin/assets/templates/basic");
    copy_tree(&template, &root);

    let manifest_path = root.join(".mdp/manifest.yaml");
    let manifest = std::fs::read_to_string(&manifest_path).unwrap();
    let unbound = manifest.replacen(
        "  decision_input_contracts:\n  - gtm.prospect-context\n",
        "",
        1,
    );
    assert_ne!(
        manifest, unbound,
        "fixture must remove the decision authority"
    );
    std::fs::write(&manifest_path, unbound).unwrap();

    let root_arg = root.to_string_lossy().into_owned();
    let (_code, _stdout, _stderr, value) = run(
        &[
            "--json",
            "check",
            "--dir",
            &root_arg,
            "--job",
            "prospect-fit-or-brief",
        ],
        Case::Ok,
    );
    let envelope = value.expect("blocked check should produce one JSON envelope");
    let diagnostic = envelope["actionable_diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["code"] == "job_readiness_unavailable")
        .expect("job readiness authority diagnostic");
    assert_eq!(diagnostic["phase"], "validation");
    assert_eq!(diagnostic["retryability"], "after-user-action");
    assert_eq!(diagnostic["prerequisites"][0]["kind"], "authority");
    assert_eq!(diagnostic["next_action"]["kind"], "manual");
    assert!(diagnostic["next_action"].get("command").is_none());
    assert!(envelope["data"].get("actionable_diagnostics").is_none());

    let (_, _, _, schema) = run(&["--json", "schema", "readiness-v1"], Case::Ok);
    let schema = schema.expect("readiness schema envelope");
    jsonschema::draft202012::validate(&schema["data"], &envelope["data"])
        .expect("diagnostic metadata must not change the closed readiness payload");
}

#[test]
fn every_json_invocation_has_exactly_one_stdout_value_no_prelude() {
    let cases: &[&[&str]] = &[
        &["--json", "capabilities"],
        &["--json", "--help"],
        &["--json", "--version"],
        &["--json", "trace", "--help"],
    ];
    for args in cases {
        let output = Command::new(mdp_bin())
            .args(*args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("mdp should run");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(
            serde_json::from_str::<serde_json::Value>(stdout.trim()).is_ok(),
            "stdout must be exactly one JSON value for {args:?}: {stdout:?}"
        );
    }
}

#[test]
fn json_mode_writes_nothing_to_stderr() {
    let cases: &[&[&str]] = &[
        &["--json", "capabilities"],
        &["--json", "--help"],
        &["--json", "--version"],
        &[
            "--json", "trace", "--file", "fit.json", "--format", "mermaid",
        ],
        &["--json", "trace", "--help"],
    ];
    for args in cases {
        let output = Command::new(mdp_bin())
            .args(*args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("mdp should run");
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(
            stderr.is_empty(),
            "stderr must be empty for {args:?}: {stderr:?}"
        );
    }
}

#[test]
fn temporal_health_json_stdout_contract() {
    const PACK: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plugin/assets/templates/basic"
    );
    let args = [
        "--json",
        "temporal-health",
        "--dir",
        PACK,
        "--as-of",
        "2026-09-02T00:00:00Z",
    ];
    let output = Command::new(mdp_bin())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("mdp should run");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["data"]["contract"], "mdp.temporal-health.v1");
    assert_eq!(value["data"]["evaluation"]["as_of"], "2026-09-02T00:00:00Z");

    let summary = Command::new(mdp_bin())
        .args([
            "--json",
            "temporal-health",
            "--summary",
            "--dir",
            PACK,
            "--as-of",
            "2026-09-02T00:00:00Z",
        ])
        .output()
        .unwrap();
    assert!(summary.status.success() && summary.stderr.is_empty());
    let summary_value: serde_json::Value = serde_json::from_slice(&summary.stdout).unwrap();
    assert_eq!(
        summary_value["summary"]["contract"],
        "mdp.temporal-health.v1"
    );

    let invalid = Command::new(mdp_bin())
        .args([
            "--json",
            "temporal-health",
            "--dir",
            PACK,
            "--as-of",
            "invalid",
        ])
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(invalid.stderr.is_empty());
    let invalid_value: serde_json::Value = serde_json::from_slice(&invalid.stdout).unwrap();
    assert_eq!(invalid_value["ok"], false);
    assert_eq!(
        invalid_value["error"]["message"],
        "--as-of must be strict UTC timestamp"
    );
}

#[test]
fn mutating_upgrade_json_rejection_is_one_stable_envelope() {
    let output = Command::new(mdp_bin())
        .args(["--json", "upgrade", "-y"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("mdp should reject JSON upgrade execution");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "upgrade_json_execution_unsupported");
}
