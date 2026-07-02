use crate::commands::briefs::prospect_brief_from_value;
use crate::commands::health::{KNOWN_PRIMITIVES, KNOWN_PROFILE_EVAL_CATEGORIES, gaps, issue};
use crate::commands::prompt_output::validate_prompt_output_value;
use crate::commands::routing::{check_claims, fit_prospect, route};
use crate::constants::DEFAULT_DIR;
use crate::models::Prospect;
use crate::pack_io::read_manifest;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct EvalFixture {
    id: String,
    #[serde(default = "default_command")]
    command: String,
    profile_eval: Option<EvalProfileMetadata>,
    persona: Option<String>,
    job: Option<String>,
    channel: Option<String>,
    prospect: Option<Value>,
    prompt: Option<std::path::PathBuf>,
    prompt_id: Option<String>,
    prompt_output: Option<Value>,
    text: Option<String>,
    subject: Option<String>,
    expect_load_order_contains: Option<Vec<String>>,
    expect_load_order_excludes: Option<Vec<String>>,
    expect_entry_titles_contains: Option<Vec<String>>,
    expect_entry_titles_excludes: Option<Vec<String>>,
    expect_status: Option<String>,
    expect_draft_status: Option<String>,
    expect_valid: Option<bool>,
    expect_normalization_ready: Option<bool>,
    expect_issue_codes_contains: Option<Vec<String>>,
    expect_gap_titles_contains: Option<Vec<String>>,
    expect_guardrail_terms_contains: Option<Vec<String>>,
    expect_unsupported_claims_contains: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct EvalProfileMetadata {
    category: String,
    #[serde(default)]
    primitives: Vec<String>,
    #[serde(default)]
    jobs: Vec<String>,
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
    let profile_job_ids = match read_manifest(root) {
        Ok(manifest) => Some(
            manifest
                .jobs
                .into_iter()
                .map(|job| job.id)
                .collect::<BTreeSet<_>>(),
        ),
        Err(err) => {
            issues.push(issue(
                "eval_manifest_read_failed",
                "error",
                root.join(DEFAULT_DIR)
                    .join("manifest.yaml")
                    .display()
                    .to_string(),
                err.to_string(),
            ));
            None
        }
    };
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
        match run_fixture(root, &path, &fixture, profile_job_ids.as_ref()) {
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

fn run_fixture(
    root: &Path,
    path: &Path,
    fixture: &EvalFixture,
    profile_job_ids: Option<&BTreeSet<String>>,
) -> Result<Value> {
    let mut issues = validate_fixture(path, fixture, profile_job_ids);
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
        "gaps" => gaps(root)?,
        "validate-prompt-output" => validate_prompt_output_fixture(root, path, fixture)?,
        "check-claims" => check_claims(
            root,
            fixture.text.as_deref(),
            None,
            fixture.subject.as_deref(),
            fixture.persona.as_deref(),
            fixture.job.as_deref(),
        )?,
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

fn validate_fixture(
    path: &Path,
    fixture: &EvalFixture,
    profile_job_ids: Option<&BTreeSet<String>>,
) -> Vec<Value> {
    let mut issues = Vec::new();
    if let Some(profile_eval) = &fixture.profile_eval {
        if profile_eval.category.trim().is_empty() {
            issues.push(issue(
                "eval_profile_category_empty",
                "error",
                format!("{}#/profile_eval/category", path.display()),
                "profile_eval.category must not be empty when profile eval metadata is present",
            ));
        } else if !KNOWN_PROFILE_EVAL_CATEGORIES.contains(&profile_eval.category.as_str()) {
            issues.push(issue(
                "eval_profile_category_unknown",
                "error",
                format!("{}#/profile_eval/category", path.display()),
                format!(
                    "unknown profile eval category {}; expected one of {}",
                    profile_eval.category,
                    KNOWN_PROFILE_EVAL_CATEGORIES.join(", ")
                ),
            ));
        }
        validate_profile_eval_primitives(path, &profile_eval.primitives, &mut issues);
        if let Some(profile_job_ids) = profile_job_ids {
            validate_profile_eval_jobs(path, &profile_eval.jobs, profile_job_ids, &mut issues);
        }
    }
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
        "gaps" => {}
        "validate-prompt-output" => {
            require(
                path,
                fixture.prompt_output.as_ref(),
                "prompt_output",
                &mut issues,
            );
            if fixture.prompt.is_some() && fixture.prompt_id.is_some() {
                issues.push(issue(
                    "eval_fixture_prompt_reference_conflict",
                    "error",
                    path.display().to_string(),
                    "validate-prompt-output fixtures must define only one of prompt or prompt_id",
                ));
            }
            if fixture.prompt.is_none() && fixture.prompt_id.is_none() {
                issues.push(issue(
                    "eval_fixture_missing_field",
                    "error",
                    format!("{}#/prompt_id", path.display()),
                    "fixture must define prompt_id or prompt",
                ));
            }
        }
        "check-claims" => {
            require(path, fixture.text.as_ref(), "text", &mut issues);
        }
        _ => {}
    }
    issues
}

fn validate_profile_eval_primitives(path: &Path, values: &[String], issues: &mut Vec<Value>) {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if value.trim().is_empty() {
            issues.push(issue(
                "eval_profile_primitive_unknown_empty",
                "error",
                format!("{}#/profile_eval/primitives/{index}", path.display()),
                "profile_eval.primitives entries must not be empty",
            ));
        } else if !KNOWN_PRIMITIVES.contains(&value.as_str()) {
            issues.push(issue(
                "eval_profile_primitive_unknown",
                "error",
                format!("{}#/profile_eval/primitives/{index}", path.display()),
                format!(
                    "profile eval fixture references unknown primitive {value}; expected one of {}",
                    KNOWN_PRIMITIVES.join(", ")
                ),
            ));
        } else if !seen.insert(value) {
            issues.push(issue(
                "eval_profile_primitive_unknown_duplicate",
                "warning",
                format!("{}#/profile_eval/primitives/{index}", path.display()),
                format!("duplicate profile eval primitive {value}"),
            ));
        }
    }
}

fn validate_profile_eval_jobs(
    path: &Path,
    values: &[String],
    profile_job_ids: &BTreeSet<String>,
    issues: &mut Vec<Value>,
) {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if value.trim().is_empty() {
            issues.push(issue(
                "eval_profile_job_missing_empty",
                "error",
                format!("{}#/profile_eval/jobs/{index}", path.display()),
                "profile_eval.jobs entries must not be empty",
            ));
        } else if !profile_job_ids.contains(value) {
            issues.push(issue(
                "eval_profile_job_missing",
                "error",
                format!("{}#/profile_eval/jobs/{index}", path.display()),
                format!("profile eval fixture references missing profile job {value}"),
            ));
        } else if !seen.insert(value) {
            issues.push(issue(
                "eval_profile_job_missing_duplicate",
                "warning",
                format!("{}#/profile_eval/jobs/{index}", path.display()),
                format!("duplicate profile eval job {value}"),
            ));
        }
    }
}

fn validate_prompt_output_fixture(
    root: &Path,
    path: &Path,
    fixture: &EvalFixture,
) -> Result<Value> {
    let prompt_output = fixture
        .prompt_output
        .as_ref()
        .context("validated prompt_output")?;
    let mut result = validate_prompt_output_value(
        root,
        prompt_output,
        &format!("{}#/prompt_output", path.display()),
        fixture.prompt.as_deref(),
        fixture.prompt_id.as_deref(),
    );

    if let Ok(value) = result.as_mut() {
        if let Some(object) = value.as_object_mut() {
            if let Some(ready) = prompt_output
                .pointer("/normalization_trace/fit_readiness/ready_for_mdp_fit")
                .and_then(Value::as_bool)
            {
                object.insert("normalization_ready".to_string(), json!(ready));
            }
        }
    }

    result
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
    if let Some(expected_ready) = fixture.expect_normalization_ready {
        if output["normalization_ready"].as_bool() != Some(expected_ready) {
            issues.push(issue(
                "eval_normalization_ready_mismatch",
                "error",
                path.display().to_string(),
                format!(
                    "expected normalization_ready {expected_ready}, got {}",
                    output["normalization_ready"]
                ),
            ));
        }
    }
    if let Some(expected_codes) = &fixture.expect_issue_codes_contains {
        let actual_codes = output["issues"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|issue| issue["code"].as_str().map(str::to_string))
            .collect::<Vec<_>>();
        for expected_code in expected_codes {
            if !actual_codes.iter().any(|code| code == expected_code) {
                issues.push(issue(
                    "eval_expected_issue_code_missing",
                    "error",
                    path.display().to_string(),
                    format!("missing expected prompt-output issue code {expected_code}"),
                ));
            }
        }
    }
    if let Some(expected_titles) = &fixture.expect_gap_titles_contains {
        let titles = gap_titles(output);
        for expected_title in expected_titles {
            if !titles.iter().any(|title| title == expected_title) {
                issues.push(issue(
                    "eval_expected_gap_missing",
                    "error",
                    path.display().to_string(),
                    format!("missing expected gap title {expected_title}"),
                ));
            }
        }
    }
    if let Some(expected_terms) = &fixture.expect_guardrail_terms_contains {
        let terms = guardrail_terms(output);
        for expected_term in expected_terms {
            if !terms.iter().any(|term| term == expected_term) {
                issues.push(issue(
                    "eval_expected_guardrail_term_missing",
                    "error",
                    path.display().to_string(),
                    format!("missing expected guardrail term {expected_term}"),
                ));
            }
        }
    }
    if let Some(expected_claims) = &fixture.expect_unsupported_claims_contains {
        let claims = unsupported_claim_labels(output);
        for expected_claim in expected_claims {
            if !claims.iter().any(|claim| claim == expected_claim) {
                issues.push(issue(
                    "eval_expected_unsupported_claim_missing",
                    "error",
                    path.display().to_string(),
                    format!("missing expected unsupported claim {expected_claim}"),
                ));
            }
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

fn gap_titles(output: &Value) -> Vec<String> {
    output["durable_gaps"]
        .as_array()
        .into_iter()
        .chain(output["evidence_gaps"].as_array())
        .flatten()
        .filter_map(|value| value["title"].as_str().map(str::to_string))
        .collect()
}

fn guardrail_terms(output: &Value) -> Vec<String> {
    output["guardrail_hits"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value["term"].as_str().map(str::to_string))
        .collect()
}

fn unsupported_claim_labels(output: &Value) -> Vec<String> {
    output["unsupported_claims"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .flat_map(|value| {
            [
                value["category"].as_str().map(str::to_string),
                value["trigger"].as_str().map(str::to_string),
            ]
        })
        .flatten()
        .collect()
}

fn fixture_result(path: &Path, fixture: &EvalFixture, output: Value, issues: Vec<Value>) -> Value {
    let valid = issues.is_empty();
    json!({
        "path": path.display().to_string(),
        "id": fixture.id,
        "command": fixture.command,
        "profile_eval": &fixture.profile_eval,
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

    #[test]
    fn eval_rejects_bad_profile_metadata_refs() {
        let root = temp_pack("eval-profile-metadata");
        let fixture_path = root.join(".mdp").join("evals").join("bad-profile.yaml");
        std::fs::write(
            &fixture_path,
            r#"id: bad-profile
persona: PMM
job: linkedin outbound copy
profile_eval:
  category: prompt-output-validation
  primitives:
    - account-context
  jobs:
    - missing-profile-job
expect_load_order_contains:
  - .mdp/cards/personas.yaml
"#,
        )
        .expect("fixture should be writable");

        let result = eval_pack(&root).expect("eval should succeed");
        let codes: Vec<&str> = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter_map(|issue| issue["code"].as_str())
            .collect();

        assert_eq!(result["valid"], false);
        assert!(codes.contains(&"eval_profile_primitive_unknown"));
        assert!(codes.contains(&"eval_profile_job_missing"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn eval_supports_inline_prompt_output_validation() {
        let root = temp_pack("eval-prompt-output");
        let fixture_path = root.join(".mdp").join("evals").join("prompt-output.yaml");
        std::fs::write(
            &fixture_path,
            r#"id: prompt-output
command: validate-prompt-output
prompt_id: normalize-prospect-row
expect_valid: false
profile_eval:
  category: prompt-output-validation
  primitives:
    - actors
    - source-signals
    - gaps
  jobs:
    - prospect-fit-or-brief
prompt_output:
  contract: mdp.prompt-output.v0
  prompt_id: normalize-prospect-row
  source_summary:
    account_name: ExampleCo
    company_domain: example.com
    company_name: ExampleCo
    confidence: medium
    inputs_used:
      - company_domain
    person_name: N/A
    person_title: N/A
  normalized_prospect:
    company: ExampleCo
    company_domain: example.com
    name: Alex Rivera
    title: Revenue Operations Lead
  normalization_trace:
    fit_readiness:
      ready_for_mdp_fit: true
    missing_required: []
    persona: {}
    preserved_raw_fields:
      - company_domain
  card_patches: []
  gaps: []
  rejected_claims: []
"#,
        )
        .expect("fixture should be writable");

        let result = eval_pack(&root).expect("eval should succeed");
        let fixture = result["fixtures"]
            .as_array()
            .expect("fixtures array")
            .iter()
            .find(|fixture| fixture["id"] == "prompt-output")
            .expect("prompt-output fixture should be present");

        assert_eq!(result["valid"], true);
        assert_eq!(fixture["valid"], true);
        assert_eq!(fixture["output"]["valid"], false);
        assert!(
            fixture["output"]["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "prompt_output_fake_person")
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
