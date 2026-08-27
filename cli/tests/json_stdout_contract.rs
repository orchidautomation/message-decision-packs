//! Process-level exhaustive JSON stdout contract test.
//!
//! Every invocation containing global `--json` must write exactly one parseable
//! JSON value to stdout, with empty stderr. The matrix below exercises every
//! public presentation selector/value combination plus a representative set of
//! success and failure paths.

use std::path::PathBuf;
use std::process::{Command, Stdio};

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
    assert!(stdout.contains("presentation_contract"));
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
    let (code, stdout, _stderr, value) = run(&["--json", "prepare-run"], Case::Ok);
    let parsed = value.expect("prepare-run blocked parses as JSON");
    assert_eq!(parsed["ok"], serde_json::json!(false));
    assert_eq!(parsed["command"], "prepare-run");
    assert_eq!(parsed["data"]["contract"], "mdp.run-request-compile.v1");
    assert_eq!(parsed["data"]["status"], "blocked");
    assert_eq!(
        parsed["data"]["diagnostics"][0]["code"],
        "cli-arguments-invalid"
    );
    assert!(!stdout.contains("error:"));
    let _ = code;
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
