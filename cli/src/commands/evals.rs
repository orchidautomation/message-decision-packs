use crate::commands::briefs::prospect_brief_from_value;
use crate::commands::health::issue;
use crate::commands::routing::{check_claims, fit_prospect, route};
use crate::constants::DEFAULT_DIR;
use crate::models::Prospect;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct EvalFixture {
    id: String,
    #[serde(default = "default_command")]
    command: String,
    persona: Option<String>,
    job: Option<String>,
    channel: Option<String>,
    prospect: Option<Value>,
    text: Option<String>,
    expect_load_order_contains: Option<Vec<String>>,
    expect_load_order_excludes: Option<Vec<String>>,
    expect_entry_titles_contains: Option<Vec<String>>,
    expect_entry_titles_excludes: Option<Vec<String>>,
    expect_status: Option<String>,
    expect_draft_status: Option<String>,
    expect_valid: Option<bool>,
}

pub(crate) fn eval_pack(root: &Path) -> Result<Value> {
    let eval_dir = root.join(DEFAULT_DIR).join("evals");
    if !eval_dir.exists() {
        return Ok(json!({
            "contract": "mdp.eval.v0",
            "valid": true,
            "fixtures": [],
            "issues": [],
            "note": "No .mdp/evals directory found. Add YAML fixtures to make route behavior testable."
        }));
    }
    let mut fixtures = Vec::new();
    let mut issues = Vec::new();
    for entry in
        fs::read_dir(&eval_dir).with_context(|| format!("reading {}", eval_dir.display()))?
    {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let fixture: EvalFixture = match serde_yaml::from_str(&raw) {
            Ok(fixture) => fixture,
            Err(err) => {
                issues.push(issue(
                    "eval_fixture_parse_failed",
                    "error",
                    path.display().to_string(),
                    err.to_string(),
                ));
                continue;
            }
        };
        match run_fixture(root, &path, &fixture) {
            Ok(result) => {
                issues.extend(result["issues"].as_array().cloned().unwrap_or_default());
                fixtures.push(result);
            }
            Err(err) => {
                issues.push(issue(
                    "eval_fixture_failed",
                    "error",
                    path.display().to_string(),
                    err.to_string(),
                ));
            }
        }
    }
    let fixture_count = fixtures.len();
    Ok(json!({
        "contract": "mdp.eval.v0",
        "valid": issues.is_empty(),
        "fixtures": fixtures,
        "issues": issues,
        "summary": {"fixture_count": fixture_count}
    }))
}

fn run_fixture(root: &Path, path: &Path, fixture: &EvalFixture) -> Result<Value> {
    let mut issues = validate_fixture(path, fixture);
    if !issues.is_empty() {
        return Ok(fixture_result(path, fixture, Value::Null, issues));
    }

    let output = match fixture.command.as_str() {
        "route" => route(
            root,
            fixture.persona.as_deref().expect("validated persona"),
            fixture.job.as_deref().expect("validated job"),
            true,
            false,
        )?,
        "fit" => {
            let prospect = parse_prospect(path, fixture)?;
            fit_prospect(root, prospect)?
        }
        "brief" => prospect_brief_from_value(
            root,
            parse_prospect(path, fixture)?,
            fixture.channel.as_deref().unwrap_or("linkedin"),
            fixture.job.as_deref(),
        )?,
        "check-claims" => check_claims(root, fixture.text.as_deref(), None)?,
        other => {
            issues.push(issue(
                "eval_fixture_unknown_command",
                "error",
                path.display().to_string(),
                format!("unknown eval command {other}"),
            ));
            Value::Null
        }
    };

    assert_expected(path, fixture, &output, &mut issues);
    Ok(fixture_result(path, fixture, output, issues))
}

fn validate_fixture(path: &Path, fixture: &EvalFixture) -> Vec<Value> {
    let mut issues = Vec::new();
    match fixture.command.as_str() {
        "route" => {
            require(path, fixture.persona.as_ref(), "persona", &mut issues);
            require(path, fixture.job.as_ref(), "job", &mut issues);
        }
        "fit" => {
            require(path, fixture.prospect.as_ref(), "prospect", &mut issues);
        }
        "brief" => {
            require(path, fixture.prospect.as_ref(), "prospect", &mut issues);
        }
        "check-claims" => {
            require(path, fixture.text.as_ref(), "text", &mut issues);
        }
        _ => {}
    }
    issues
}

fn require<T>(path: &Path, value: Option<&T>, field: &str, issues: &mut Vec<Value>) {
    if value.is_none() {
        issues.push(issue(
            "eval_fixture_missing_field",
            "error",
            format!("{}#/{field}", path.display()),
            format!("fixture must define {field}"),
        ));
    }
}

fn parse_prospect(path: &Path, fixture: &EvalFixture) -> Result<Prospect> {
    serde_json::from_value(
        fixture
            .prospect
            .clone()
            .with_context(|| format!("{} missing prospect", path.display()))?,
    )
    .with_context(|| format!("{} invalid prospect", path.display()))
}

fn assert_expected(path: &Path, fixture: &EvalFixture, output: &Value, issues: &mut Vec<Value>) {
    if let Some(expected) = &fixture.expect_load_order_contains {
        let actual = output["load_order"]
            .as_array()
            .or_else(|| output["required_load_order"].as_array())
            .cloned()
            .unwrap_or_default();
        for expected_path in expected {
            if !actual.iter().any(|value| value == expected_path) {
                issues.push(issue(
                    "eval_expected_load_order_missing",
                    "error",
                    path.display().to_string(),
                    format!("missing expected load_order path {expected_path}"),
                ));
            }
        }
    }
    if let Some(excluded) = &fixture.expect_load_order_excludes {
        let actual = output["load_order"]
            .as_array()
            .or_else(|| output["required_load_order"].as_array())
            .cloned()
            .unwrap_or_default();
        for excluded_path in excluded {
            if actual.iter().any(|value| value == excluded_path) {
                issues.push(issue(
                    "eval_excluded_load_order_present",
                    "error",
                    path.display().to_string(),
                    format!("load_order included excluded path {excluded_path}"),
                ));
            }
        }
    }
    if let Some(expected_titles) = &fixture.expect_entry_titles_contains {
        let titles = entry_titles(output);
        for expected_title in expected_titles {
            if !titles.iter().any(|title| title == expected_title) {
                issues.push(issue(
                    "eval_expected_entry_missing",
                    "error",
                    path.display().to_string(),
                    format!("missing expected entry title {expected_title}"),
                ));
            }
        }
    }
    if let Some(excluded_titles) = &fixture.expect_entry_titles_excludes {
        let titles = entry_titles(output);
        for excluded_title in excluded_titles {
            if titles.iter().any(|title| title == excluded_title) {
                issues.push(issue(
                    "eval_excluded_entry_present",
                    "error",
                    path.display().to_string(),
                    format!("entry route included excluded title {excluded_title}"),
                ));
            }
        }
    }
    if let Some(expected_status) = &fixture.expect_status {
        if output["status"] != expected_status.as_str() {
            issues.push(issue(
                "eval_status_mismatch",
                "error",
                path.display().to_string(),
                format!(
                    "expected status {expected_status}, got {}",
                    output["status"]
                ),
            ));
        }
    }
    if let Some(expected_draft_status) = &fixture.expect_draft_status {
        if output["draft_status"] != expected_draft_status.as_str() {
            issues.push(issue(
                "eval_draft_status_mismatch",
                "error",
                path.display().to_string(),
                format!(
                    "expected draft_status {expected_draft_status}, got {}",
                    output["draft_status"]
                ),
            ));
        }
    }
    if let Some(expected_valid) = fixture.expect_valid {
        if output["valid"].as_bool() != Some(expected_valid) {
            issues.push(issue(
                "eval_valid_mismatch",
                "error",
                path.display().to_string(),
                format!("expected valid {expected_valid}, got {}", output["valid"]),
            ));
        }
    }
}

fn entry_titles(output: &Value) -> Vec<String> {
    output["entry_route"]["matches"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value["title"].as_str().map(str::to_string))
        .collect()
}

fn fixture_result(path: &Path, fixture: &EvalFixture, output: Value, issues: Vec<Value>) -> Value {
    let valid = issues.is_empty();
    json!({
        "path": path.display().to_string(),
        "id": fixture.id,
        "command": fixture.command,
        "valid": valid,
        "issues": issues,
        "output": output
    })
}

fn default_command() -> String {
    "route".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_pack(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-{name}-{nonce}"));
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        root
    }

    #[test]
    fn eval_requires_explicit_route_fields() {
        let root = temp_pack("eval-required");
        let fixture_path = root.join(".mdp").join("evals").join("missing-field.yaml");
        std::fs::write(
            &fixture_path,
            r#"id: missing-card
command: route
job: linkedin outbound copy
expect_load_order_contains:
  - .mdp/cards/personas.yaml
"#,
        )
        .expect("fixture should be writable");

        let result = eval_pack(&root).expect("eval should succeed");

        assert_eq!(result["valid"], false);
        assert_eq!(result["issues"][0]["code"], "eval_fixture_missing_field");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn eval_reports_missing_expected_route_paths() {
        let root = temp_pack("eval-contract");
        let fixture_path = root.join(".mdp").join("evals").join("missing-card.yaml");
        std::fs::write(
            &fixture_path,
            r#"id: missing-card
persona: PMM
job: linkedin outbound copy
expect_load_order_contains:
  - .mdp/cards/not-real.yaml
"#,
        )
        .expect("fixture should be writable");

        let result = eval_pack(&root).expect("eval should succeed");

        assert_eq!(result["contract"], "mdp.eval.v0");
        assert_eq!(result["valid"], false);
        assert_eq!(
            result["issues"][0]["code"],
            "eval_expected_load_order_missing"
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
