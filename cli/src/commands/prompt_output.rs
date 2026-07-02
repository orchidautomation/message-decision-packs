use crate::constants::{DEFAULT_DIR, PROMPT_OUTPUT_CONTRACT};
use crate::models::{CardKind, Manifest, PromptFile};
use crate::pack_io::{read_card, read_manifest, read_prompt, resolve_pack_path};
use crate::runtime_context::validate_runtime_context;
use crate::utils::normalize_supplied_company_domain;
use crate::value_contracts::normalized_prospect_contract_violations;
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn validate_prompt_output_file(
    root: &Path,
    file: &Path,
    prompt_path: Option<&Path>,
    prompt_id: Option<&str>,
) -> Result<Value> {
    if prompt_path.is_some() && prompt_id.is_some() {
        return Err(anyhow!("pass at most one of --prompt and --prompt-id"));
    }

    let prompt = resolve_prompt(root, prompt_path, prompt_id)?;
    let artifact_path = display_path(file);
    let raw = fs::read_to_string(file)?;
    let mut issues = Vec::new();
    let (output, markdown_wrapped) = match parse_prompt_output(&raw) {
        Ok(parsed) => parsed,
        Err(err) => {
            issues.push(issue(
                "prompt_output_parse_failed",
                "error",
                &artifact_path,
                err.to_string(),
            ));
            return Ok(json!({
                "valid": false,
                "file": artifact_path,
                "prompt": prompt_summary(&prompt, root),
                "issues": issues
            }));
        }
    };

    if markdown_wrapped {
        issues.push(issue(
            "prompt_output_markdown_wrapped",
            "error",
            &artifact_path,
            "prompt output must be raw JSON, not markdown-wrapped JSON",
        ));
    }

    validate_prompt_output_parsed(root, &prompt, &output, &artifact_path, issues)
}

pub(crate) fn validate_prompt_output_value(
    root: &Path,
    output: &Value,
    artifact_path: &str,
    prompt_path: Option<&Path>,
    prompt_id: Option<&str>,
) -> Result<Value> {
    if prompt_path.is_some() && prompt_id.is_some() {
        return Err(anyhow!("pass at most one of prompt or prompt_id"));
    }

    let prompt = resolve_prompt(root, prompt_path, prompt_id)?;
    validate_prompt_output_parsed(root, &prompt, output, artifact_path, Vec::new())
}

fn validate_prompt_output_parsed(
    root: &Path,
    prompt: &PromptFile,
    output: &Value,
    artifact_path: &str,
    mut issues: Vec<Value>,
) -> Result<Value> {
    let manifest = read_manifest(root)?;
    validate_output_against_prompt(&manifest, prompt, output, artifact_path, &mut issues);
    validate_card_collisions(root, prompt, output, artifact_path, &mut issues)?;

    Ok(json!({
        "valid": issues.is_empty(),
        "file": artifact_path,
        "prompt": prompt_summary(prompt, root),
        "issues": issues
    }))
}

fn resolve_prompt(
    root: &Path,
    prompt_path: Option<&Path>,
    prompt_id: Option<&str>,
) -> Result<PromptFile> {
    if let Some(path) = prompt_path {
        return read_prompt(&resolve_prompt_path(root, path));
    }

    let resolved_id = prompt_id
        .map(str::to_string)
        .ok_or_else(|| anyhow!("--prompt-id is required when --prompt is not provided"))?;

    let prompts_dir = root.join(DEFAULT_DIR).join("prompts");
    let mut prompt_paths = fs::read_dir(&prompts_dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    prompt_paths.sort();

    for path in prompt_paths {
        let prompt = read_prompt(&path)?;
        if prompt.id == resolved_id {
            return Ok(prompt);
        }
    }

    Err(anyhow!(
        "prompt id {resolved_id} was not found under {DEFAULT_DIR}/prompts"
    ))
}

fn resolve_prompt_path(root: &Path, prompt_path: &Path) -> PathBuf {
    if prompt_path.is_absolute() {
        return prompt_path.to_path_buf();
    }
    let direct = root.join(prompt_path);
    if direct.exists() {
        return direct;
    }
    root.join(DEFAULT_DIR).join("prompts").join(prompt_path)
}

fn prompt_summary(prompt: &PromptFile, root: &Path) -> Value {
    json!({
        "id": prompt.id,
        "output_kind": prompt.output_contract.output_kind.as_deref().unwrap_or("card-patches"),
        "target_card_kinds": prompt.target_card_kinds.iter().map(card_kind_name).collect::<Vec<_>>(),
        "declared_inputs": prompt.inputs.iter().map(|input| input.name.clone()).collect::<Vec<_>>(),
        "pack_dir": root.join(DEFAULT_DIR).display().to_string()
    })
}

fn parse_prompt_output(raw: &str) -> Result<(Value, bool)> {
    if let Ok(value) = serde_json::from_str::<Value>(raw) {
        return Ok((value, false));
    }

    let trimmed = raw.trim();
    if !(trimmed.starts_with("```") && trimmed.ends_with("```")) {
        return Err(anyhow!("prompt output file must contain valid JSON"));
    }

    let mut lines = trimmed.lines();
    let Some(first_line) = lines.next() else {
        return Err(anyhow!("prompt output file must contain valid JSON"));
    };
    if !first_line.starts_with("```") {
        return Err(anyhow!("prompt output file must contain valid JSON"));
    }

    let mut body = lines.collect::<Vec<_>>();
    if body.last().is_some_and(|line| line.trim() == "```") {
        body.pop();
    }
    let inner = body.join("\n");
    let value = serde_json::from_str::<Value>(&inner)
        .map_err(|_| anyhow!("prompt output file must contain valid JSON"))?;
    Ok((value, true))
}

fn validate_output_against_prompt(
    manifest: &Manifest,
    prompt: &PromptFile,
    output: &Value,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let Some(root) = output.as_object() else {
        issues.push(issue(
            "prompt_output_root_type",
            "error",
            path,
            "prompt output root must be a JSON object",
        ));
        return;
    };

    let output_kind = prompt
        .output_contract
        .output_kind
        .as_deref()
        .unwrap_or("card-patches");
    let declared_inputs = prompt
        .inputs
        .iter()
        .map(|input| input.name.clone())
        .collect::<BTreeSet<_>>();
    let allowed_top_level = allowed_top_level_fields(output_kind);
    validate_json_object_keys(
        root,
        &allowed_top_level,
        path,
        "prompt_output_unknown_field",
        issues,
    );

    for field in &prompt.output_contract.required_top_level {
        if output.get(field).is_none() {
            issues.push(issue(
                "prompt_output_required_field_missing",
                "error",
                path,
                format!("prompt output is missing required field {field}"),
            ));
        }
    }

    if output["contract"].as_str() != Some(PROMPT_OUTPUT_CONTRACT) {
        issues.push(issue(
            "prompt_output_contract_mismatch",
            "error",
            format!("{path}#/contract"),
            format!("prompt output contract must be {PROMPT_OUTPUT_CONTRACT}"),
        ));
    }
    if output["prompt_id"].as_str() != Some(prompt.id.as_str()) {
        issues.push(issue(
            "prompt_output_prompt_id_mismatch",
            "error",
            format!("{path}#/prompt_id"),
            format!("prompt output prompt_id must be {}", prompt.id),
        ));
    }
    if let Some(runtime_context) = output.get("runtime_context") {
        if !declared_inputs.contains("runtime_context") {
            issues.push(issue(
                "prompt_output_runtime_context_undeclared",
                "error",
                format!("{path}#/runtime_context"),
                "prompt output may include runtime_context only when the prompt declares runtime_context as an input",
            ));
        }
        for violation in
            validate_runtime_context(runtime_context, &format!("{path}#/runtime_context"))
        {
            issues.push(issue(
                violation.code,
                "error",
                violation.path,
                violation.reason,
            ));
        }
    }

    let inputs_used = validate_source_summary(
        output.get("source_summary"),
        &declared_inputs,
        &format!("{path}#/source_summary"),
        issues,
    );

    let mut saw_supporting_reference = false;
    validate_card_patches(
        prompt,
        output.get("card_patches"),
        &declared_inputs,
        &format!("{path}#/card_patches"),
        &mut saw_supporting_reference,
        issues,
    );
    validate_rejected_claims(
        output.get("rejected_claims"),
        &format!("{path}#/rejected_claims"),
        issues,
    );
    validate_plain_string_array(
        output.get("gaps"),
        &format!("{path}#/gaps"),
        "prompt_output_gaps_type",
        "gaps must be an array of strings",
        issues,
    );

    if output_kind == "prospect-normalization" {
        validate_normalized_prospect(
            manifest,
            output.get("normalized_prospect"),
            &format!("{path}#/normalized_prospect"),
            issues,
        );
        validate_normalization_trace(
            output.get("normalization_trace"),
            &format!("{path}#/normalization_trace"),
            issues,
        );
        validate_normalization_invariants(
            output,
            &inputs_used,
            &format!("{path}#/normalized_prospect"),
            issues,
        );
    }

    if saw_supporting_reference && inputs_used.is_empty() {
        issues.push(issue(
            "prompt_output_inputs_used_empty",
            "error",
            format!("{path}#/source_summary/inputs_used"),
            "source_summary.inputs_used must name declared inputs when evidence or provenance is present",
        ));
    }
}

fn validate_source_summary(
    value: Option<&Value>,
    declared_inputs: &BTreeSet<String>,
    path: &str,
    issues: &mut Vec<Value>,
) -> BTreeSet<String> {
    let Some(summary) = value.and_then(Value::as_object) else {
        issues.push(issue(
            "prompt_output_source_summary_type",
            "error",
            path,
            "source_summary must be an object",
        ));
        return BTreeSet::new();
    };

    validate_json_object_keys(
        summary,
        &[
            "company_domain",
            "company_name",
            "person_name",
            "person_title",
            "account_name",
            "inputs_used",
            "confidence",
        ],
        path,
        "prompt_output_source_summary_unknown_field",
        issues,
    );

    for field in [
        "company_domain",
        "company_name",
        "person_name",
        "person_title",
        "account_name",
        "confidence",
    ] {
        if summary.get(field).and_then(Value::as_str).is_none() {
            issues.push(issue(
                "prompt_output_source_summary_field_type",
                "error",
                format!("{path}/{field}"),
                format!("source_summary.{field} must be a string"),
            ));
        }
    }

    let confidence = summary
        .get("confidence")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(confidence, "high" | "medium" | "low" | "unknown") {
        issues.push(issue(
            "prompt_output_confidence_invalid",
            "error",
            format!("{path}/confidence"),
            "source_summary.confidence must be high, medium, low, or unknown",
        ));
    }

    if let Some(domain) = summary.get("company_domain").and_then(Value::as_str) {
        validate_optional_domain(domain, &format!("{path}/company_domain"), issues);
    }

    let Some(inputs) = summary.get("inputs_used").and_then(Value::as_array) else {
        issues.push(issue(
            "prompt_output_inputs_used_type",
            "error",
            format!("{path}/inputs_used"),
            "source_summary.inputs_used must be an array",
        ));
        return BTreeSet::new();
    };

    let mut inputs_used = BTreeSet::new();
    for (index, input) in inputs.iter().enumerate() {
        let Some(input_name) = input.as_str() else {
            issues.push(issue(
                "prompt_output_inputs_used_type",
                "error",
                format!("{path}/inputs_used/{index}"),
                "source_summary.inputs_used values must be strings",
            ));
            continue;
        };
        if !declared_inputs.contains(input_name) {
            issues.push(issue(
                "prompt_output_inputs_used_undeclared",
                "error",
                format!("{path}/inputs_used/{index}"),
                format!("source_summary.inputs_used references undeclared input {input_name}"),
            ));
            continue;
        }
        inputs_used.insert(input_name.to_string());
    }

    inputs_used
}

fn validate_card_patches(
    prompt: &PromptFile,
    value: Option<&Value>,
    declared_inputs: &BTreeSet<String>,
    path: &str,
    saw_supporting_reference: &mut bool,
    issues: &mut Vec<Value>,
) {
    let Some(card_patches) = value.and_then(Value::as_array) else {
        issues.push(issue(
            "prompt_output_card_patches_type",
            "error",
            path,
            "card_patches must be an array",
        ));
        return;
    };

    if prompt.output_contract.output_kind.as_deref() == Some("prospect-normalization")
        && !card_patches.is_empty()
    {
        issues.push(issue(
            "prompt_output_normalization_card_patches",
            "error",
            path,
            "prospect-normalization outputs must keep card_patches empty",
        ));
    }

    let target_kinds = prompt
        .target_card_kinds
        .iter()
        .map(card_kind_name)
        .collect::<BTreeSet<_>>();
    let mut seen_entry_ids = BTreeSet::new();

    for (patch_index, patch) in card_patches.iter().enumerate() {
        let patch_path = format!("{path}/{patch_index}");
        let Some(patch) = patch.as_object() else {
            issues.push(issue(
                "prompt_output_patch_type",
                "error",
                &patch_path,
                "each card_patches item must be an object",
            ));
            continue;
        };
        validate_json_object_keys(
            patch,
            &["card_id", "kind", "entries"],
            &patch_path,
            "prompt_output_patch_unknown_field",
            issues,
        );

        let kind = patch
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !target_kinds.contains(kind) {
            issues.push(issue(
                "prompt_output_patch_kind_invalid",
                "error",
                format!("{patch_path}/kind"),
                format!(
                    "card_patches.kind must be one of the prompt target_card_kinds, found {kind}"
                ),
            ));
        }

        let Some(entries) = patch.get("entries").and_then(Value::as_array) else {
            issues.push(issue(
                "prompt_output_entries_type",
                "error",
                format!("{patch_path}/entries"),
                "each card patch must include an entries array",
            ));
            continue;
        };

        for (entry_index, entry) in entries.iter().enumerate() {
            let entry_path = format!("{patch_path}/entries/{entry_index}");
            let Some(entry) = entry.as_object() else {
                issues.push(issue(
                    "prompt_output_entry_type",
                    "error",
                    &entry_path,
                    "each candidate entry must be an object",
                ));
                continue;
            };
            validate_json_object_keys(
                entry,
                &[
                    "id",
                    "title",
                    "body",
                    "applies_to",
                    "evidence",
                    "avoid",
                    "exact_paragraphs",
                    "constraints",
                    "metadata",
                    "confidence",
                    "provenance",
                    "status",
                    "notes",
                ],
                &entry_path,
                "prompt_output_entry_unknown_field",
                issues,
            );

            for field in [
                "id",
                "title",
                "body",
                "applies_to",
                "evidence",
                "avoid",
                "confidence",
                "provenance",
                "status",
                "notes",
            ] {
                if entry.get(field).is_none() {
                    issues.push(issue(
                        "prompt_output_entry_field_missing",
                        "error",
                        &entry_path,
                        format!("candidate entries must include {field}"),
                    ));
                }
            }

            let entry_id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
            if !entry_id.is_empty() && !seen_entry_ids.insert(entry_id.to_string()) {
                issues.push(issue(
                    "prompt_output_duplicate_candidate_id",
                    "error",
                    format!("{entry_path}/id"),
                    format!("duplicate candidate entry id {entry_id}"),
                ));
            }

            validate_string_array(
                entry.get("evidence"),
                declared_inputs,
                &format!("{entry_path}/evidence"),
                "prompt_output_evidence_reference_undeclared",
                saw_supporting_reference,
                issues,
            );
            validate_string_array(
                entry.get("provenance"),
                declared_inputs,
                &format!("{entry_path}/provenance"),
                "prompt_output_provenance_reference_undeclared",
                saw_supporting_reference,
                issues,
            );
            validate_plain_string_array(
                entry.get("applies_to"),
                &format!("{entry_path}/applies_to"),
                "prompt_output_applies_to_type",
                "candidate entry applies_to must be an array of strings",
                issues,
            );
            validate_plain_string_array(
                entry.get("avoid"),
                &format!("{entry_path}/avoid"),
                "prompt_output_avoid_type",
                "candidate entry avoid must be an array of strings",
                issues,
            );
            validate_plain_string_array(
                entry.get("notes"),
                &format!("{entry_path}/notes"),
                "prompt_output_notes_type",
                "candidate entry notes must be an array of strings",
                issues,
            );

            let body = entry
                .get("body")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let status = entry
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let evidence_count = entry
                .get("evidence")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let provenance_count = entry
                .get("provenance")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            if body != "N/A" && status != "gap" && evidence_count == 0 && provenance_count == 0 {
                issues.push(issue(
                    "prompt_output_entry_unproven",
                    "error",
                    &entry_path,
                    "non-gap candidate entries with a real body need evidence or provenance",
                ));
            }
        }
    }
}

fn validate_rejected_claims(value: Option<&Value>, path: &str, issues: &mut Vec<Value>) {
    let Some(rejected_claims) = value.and_then(Value::as_array) else {
        issues.push(issue(
            "prompt_output_rejected_claims_type",
            "error",
            path,
            "rejected_claims must be an array",
        ));
        return;
    };

    for (index, claim) in rejected_claims.iter().enumerate() {
        let claim_path = format!("{path}/{index}");
        let Some(claim) = claim.as_object() else {
            issues.push(issue(
                "prompt_output_rejected_claim_type",
                "error",
                &claim_path,
                "rejected_claims entries must be objects",
            ));
            continue;
        };
        validate_json_object_keys(
            claim,
            &["claim", "reason", "source"],
            &claim_path,
            "prompt_output_rejected_claim_unknown_field",
            issues,
        );
        for field in ["claim", "reason", "source"] {
            if claim.get(field).and_then(Value::as_str).is_none() {
                issues.push(issue(
                    "prompt_output_rejected_claim_field_missing",
                    "error",
                    &claim_path,
                    format!("rejected claims must include string field {field}"),
                ));
            }
        }
    }
}

fn validate_normalized_prospect(
    manifest: &Manifest,
    value: Option<&Value>,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let Some(prospect) = value.and_then(Value::as_object) else {
        issues.push(issue(
            "prompt_output_normalized_prospect_type",
            "error",
            path,
            "normalized_prospect must be an object",
        ));
        return;
    };

    validate_json_object_keys(
        prospect,
        &[
            "name",
            "title",
            "company",
            "company_domain",
            "source_kind",
            "synthetic",
            "linkedin_url",
            "company_url",
            "background",
            "trigger",
            "persona",
            "segment",
            "signals",
            "attributes",
        ],
        path,
        "prompt_output_normalized_prospect_unknown_field",
        issues,
    );

    for field in ["name", "title", "company"] {
        if prospect
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            issues.push(issue(
                "prompt_output_normalized_prospect_field_missing",
                "error",
                format!("{path}/{field}"),
                format!("normalized_prospect must include non-empty {field}"),
            ));
        }
    }

    if let Some(signals) = prospect.get("signals") {
        let Some(signals) = signals.as_array() else {
            issues.push(issue(
                "prompt_output_normalized_prospect_signals_type",
                "error",
                format!("{path}/signals"),
                "normalized_prospect.signals must be an array",
            ));
            return;
        };
        for (index, signal) in signals.iter().enumerate() {
            let signal_path = format!("{path}/signals/{index}");
            let Some(signal) = signal.as_object() else {
                issues.push(issue(
                    "prompt_output_normalized_prospect_signal_type",
                    "error",
                    &signal_path,
                    "normalized prospect signals must be objects",
                ));
                continue;
            };
            validate_json_object_keys(
                signal,
                &[
                    "id",
                    "title",
                    "source",
                    "confidence",
                    "freshness",
                    "state_as",
                ],
                &signal_path,
                "prompt_output_normalized_prospect_signal_unknown_field",
                issues,
            );
            for field in ["id", "title"] {
                if signal
                    .get(field)
                    .and_then(Value::as_str)
                    .is_none_or(|value| value.trim().is_empty())
                {
                    issues.push(issue(
                        "prompt_output_normalized_prospect_signal_field_missing",
                        "error",
                        &signal_path,
                        format!("normalized prospect signals must include non-empty {field}"),
                    ));
                }
            }
        }
    }

    if let Some(domain) = prospect.get("company_domain").and_then(Value::as_str) {
        validate_optional_domain(domain, &format!("{path}/company_domain"), issues);
    }
    if let Some(attributes) = prospect.get("attributes") {
        validate_attributes(attributes, &format!("{path}/attributes"), issues);
    }

    for violation in normalized_prospect_contract_violations(manifest, prospect, path) {
        issues.push(issue(
            violation.code,
            "error",
            violation.path,
            violation.reason,
        ));
    }
}

fn validate_optional_domain(value: &str, path: &str, issues: &mut Vec<Value>) {
    if value.trim().is_empty() || value.trim().eq_ignore_ascii_case("n/a") {
        return;
    }
    if let Err(err) = normalize_supplied_company_domain(value) {
        issues.push(issue(
            "prompt_output_company_domain_invalid",
            "error",
            path,
            err.to_string(),
        ));
    }
}

fn validate_attributes(value: &Value, path: &str, issues: &mut Vec<Value>) {
    let Some(attributes) = value.as_object() else {
        issues.push(issue(
            "prompt_output_attributes_type",
            "error",
            path,
            "attributes must be an object of bounded reviewed metadata",
        ));
        return;
    };
    if attributes.len() > 25 {
        issues.push(issue(
            "prompt_output_attributes_too_many",
            "error",
            path,
            "attributes must contain at most 25 reviewed metadata keys",
        ));
    }
    for (key, value) in attributes {
        let attribute_path = format!("{path}/{key}");
        if !valid_attribute_key(key) {
            issues.push(issue(
                "prompt_output_attribute_key_invalid",
                "error",
                &attribute_path,
                "attribute keys must start with a letter and contain only letters, numbers, underscores, or hyphens",
            ));
        }
        if !(value.is_string() || value.is_number() || value.is_boolean()) {
            issues.push(issue(
                "prompt_output_attribute_value_invalid",
                "error",
                &attribute_path,
                "attribute values must be strings, numbers, or booleans; use signals with sources for evidence",
            ));
        }
    }
}

fn valid_attribute_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && key.len() <= 64
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn validate_normalization_trace(value: Option<&Value>, path: &str, issues: &mut Vec<Value>) {
    let Some(trace) = value.and_then(Value::as_object) else {
        issues.push(issue(
            "prompt_output_normalization_trace_type",
            "error",
            path,
            "normalization_trace must be an object",
        ));
        return;
    };

    validate_json_object_keys(
        trace,
        &[
            "persona",
            "fit_readiness",
            "preserved_raw_fields",
            "missing_required",
        ],
        path,
        "prompt_output_normalization_trace_unknown_field",
        issues,
    );

    for field in ["persona", "fit_readiness"] {
        if !trace.get(field).is_some_and(Value::is_object) {
            issues.push(issue(
                "prompt_output_normalization_trace_field_type",
                "error",
                format!("{path}/{field}"),
                format!("normalization_trace.{field} must be an object"),
            ));
        }
    }
    for field in ["preserved_raw_fields", "missing_required"] {
        if trace.get(field).and_then(Value::as_array).is_none() {
            issues.push(issue(
                "prompt_output_normalization_trace_field_type",
                "error",
                format!("{path}/{field}"),
                format!("normalization_trace.{field} must be an array"),
            ));
        }
    }
}

fn validate_normalization_invariants(
    output: &Value,
    inputs_used: &BTreeSet<String>,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let Some(prospect) = output.get("normalized_prospect").and_then(Value::as_object) else {
        return;
    };

    let has_person_data = inputs_used.contains("person_data") || inputs_used.contains("raw_row");
    let name = prospect
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let title = prospect
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !has_person_data && (name != "N/A" || title != "N/A") {
        issues.push(issue(
            "prompt_output_fake_person",
            "error",
            path,
            "normalized_prospect must not invent a person when inputs_used does not include person-level input",
        ));
    }
}

fn validate_card_collisions(
    root: &Path,
    prompt: &PromptFile,
    output: &Value,
    path: &str,
    issues: &mut Vec<Value>,
) -> Result<()> {
    let manifest = read_manifest(root)?;
    let mut cards_by_id = BTreeMap::new();
    let mut existing_ids_by_card = BTreeMap::new();
    for card_ref in &manifest.cards {
        let card = read_card(&resolve_pack_path(root, &card_ref.path)?)?;
        cards_by_id.insert(card.id.clone(), card.kind.clone());
        existing_ids_by_card.insert(
            card.id.clone(),
            card.entries
                .into_iter()
                .map(|entry| entry.id)
                .collect::<BTreeSet<_>>(),
        );
    }

    let Some(card_patches) = output.get("card_patches").and_then(Value::as_array) else {
        return Ok(());
    };
    for (patch_index, patch) in card_patches.iter().enumerate() {
        let Some(patch) = patch.as_object() else {
            continue;
        };
        let patch_path = format!("{path}#/card_patches/{patch_index}");
        let card_id = patch
            .get("card_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let kind = patch
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if let Some(existing_kind) = cards_by_id.get(card_id) {
            if card_kind_name(existing_kind) != kind {
                issues.push(issue(
                    "prompt_output_card_kind_mismatch",
                    "error",
                    format!("{patch_path}/kind"),
                    format!("card patch kind {kind} does not match existing card {card_id}"),
                ));
            }
        } else {
            issues.push(issue(
                "prompt_output_card_id_unknown",
                "error",
                format!("{patch_path}/card_id"),
                format!("card patch references unknown card id {card_id}"),
            ));
        }

        if !prompt
            .target_card_kinds
            .iter()
            .map(card_kind_name)
            .any(|target| target == kind)
        {
            continue;
        }

        let Some(existing_ids) = existing_ids_by_card.get(card_id) else {
            continue;
        };
        let Some(entries) = patch.get("entries").and_then(Value::as_array) else {
            continue;
        };
        for (entry_index, entry) in entries.iter().enumerate() {
            let entry_id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
            if !entry_id.is_empty() && existing_ids.contains(entry_id) {
                issues.push(issue(
                    "prompt_output_candidate_id_collision",
                    "error",
                    format!("{patch_path}/entries/{entry_index}/id"),
                    format!("candidate entry id {entry_id} collides with existing card entry id in {card_id}"),
                ));
            }
        }
    }

    Ok(())
}

fn validate_string_array(
    value: Option<&Value>,
    declared_inputs: &BTreeSet<String>,
    path: &str,
    code: &str,
    saw_supporting_reference: &mut bool,
    issues: &mut Vec<Value>,
) {
    let Some(items) = value.and_then(Value::as_array) else {
        issues.push(issue(
            "prompt_output_array_type",
            "error",
            path,
            "expected an array of strings",
        ));
        return;
    };

    for (index, item) in items.iter().enumerate() {
        let Some(reference) = item.as_str() else {
            issues.push(issue(
                "prompt_output_array_type",
                "error",
                format!("{path}/{index}"),
                "expected an array of strings",
            ));
            continue;
        };
        *saw_supporting_reference = true;
        if !references_declared_input(reference, declared_inputs) {
            issues.push(issue(
                code,
                "error",
                format!("{path}/{index}"),
                format!("reference {reference} does not match a declared prompt input"),
            ));
        }
    }
}

fn validate_plain_string_array(
    value: Option<&Value>,
    path: &str,
    code: &str,
    message: &str,
    issues: &mut Vec<Value>,
) {
    let Some(items) = value.and_then(Value::as_array) else {
        issues.push(issue(code, "error", path, message));
        return;
    };

    for (index, item) in items.iter().enumerate() {
        if item.as_str().is_none() {
            issues.push(issue(code, "error", format!("{path}/{index}"), message));
        }
    }
}

fn validate_json_object_keys(
    value: &serde_json::Map<String, Value>,
    allowed: &[&str],
    path: &str,
    code: &str,
    issues: &mut Vec<Value>,
) {
    let allowed = allowed.iter().copied().collect::<BTreeSet<_>>();
    for key in value.keys() {
        if !allowed.contains(key.as_str()) {
            issues.push(issue(
                code,
                "error",
                format!("{path}/{key}"),
                format!("prompt output contains unsupported field {key}"),
            ));
        }
    }
}

fn allowed_top_level_fields(output_kind: &str) -> Vec<&'static str> {
    let mut fields = vec![
        "contract",
        "prompt_id",
        "source_summary",
        "card_patches",
        "gaps",
        "rejected_claims",
        "runtime_context",
    ];
    if output_kind == "prospect-normalization" {
        fields.push("normalized_prospect");
        fields.push("normalization_trace");
    }
    fields
}

fn references_declared_input(reference: &str, declared_inputs: &BTreeSet<String>) -> bool {
    declared_inputs.iter().any(|input| {
        reference == input
            || reference.starts_with(&format!("{input}:"))
            || reference.starts_with(&format!("{input}."))
            || reference.starts_with(&format!("{input}["))
    })
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn issue(code: &str, severity: &str, path: impl Into<String>, message: impl Into<String>) -> Value {
    json!({
        "code": code,
        "severity": severity,
        "path": path.into(),
        "message": message.into()
    })
}

fn card_kind_name(kind: &CardKind) -> &'static str {
    match kind {
        CardKind::Personas => "personas",
        CardKind::Pains => "pains",
        CardKind::Motions => "motions",
        CardKind::Hooks => "hooks",
        CardKind::AvoidRules => "avoid-rules",
        CardKind::OutputRules => "output-rules",
        CardKind::CopyPatterns => "copy-patterns",
        CardKind::Ctas => "ctas",
        CardKind::FitRules => "fit-rules",
        CardKind::Claims => "claims",
        CardKind::Signals => "signals",
        CardKind::Positioning => "positioning",
        CardKind::ChannelPolicies => "channel-policies",
        CardKind::Objections => "objections",
        CardKind::Gaps => "gaps",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_pack(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-prompt-output-{name}-{nonce}"));
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        root
    }

    fn write_output(root: &Path, name: &str, body: &str) -> PathBuf {
        let path = root.join(name);
        std::fs::write(&path, body).expect("output fixture should be writable");
        path
    }

    #[test]
    fn validate_accepts_valid_prompt_output() {
        let root = temp_pack("valid");
        let path = write_output(
            &root,
            "claims-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "extract-claims-proof",
  "source_summary": {
    "company_domain": "N/A",
    "company_name": "N/A",
    "person_name": "N/A",
    "person_title": "N/A",
    "account_name": "N/A",
    "inputs_used": ["source_notes"],
    "confidence": "medium"
  },
  "card_patches": [
    {
      "card_id": "claims",
      "kind": "claims",
      "entries": [
        {
          "id": "claim-reviewed-local-context",
          "title": "Local decision context",
          "body": "Supplied source material describes the product as local decision context for GTM messaging.",
          "applies_to": ["PMM"],
          "evidence": ["source_notes"],
          "avoid": [],
          "confidence": "medium",
          "provenance": ["source_notes: supplied source notes"],
          "status": "needs-review",
          "notes": []
        }
      ]
    }
  ],
  "gaps": [],
  "rejected_claims": []
}"#,
        );

        let result = validate_prompt_output_file(&root, &path, None, Some("extract-claims-proof"))
            .expect("validation should return diagnostics");

        assert_eq!(result["valid"], true);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_invalid_runtime_context() {
        let root = temp_pack("runtime-context");
        let path = write_output(
            &root,
            "claims-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "extract-claims-proof",
  "source_summary": {
    "company_domain": "N/A",
    "company_name": "N/A",
    "person_name": "N/A",
    "person_title": "N/A",
    "account_name": "N/A",
    "inputs_used": ["source_notes", "runtime_context"],
    "confidence": "medium"
  },
  "runtime_context": {
    "contract": "mdp.runtime-context.v0",
    "now_utc": "2026-13-02 03:45:00",
    "date_utc": "2026-02-30",
    "timezone": "America/New_York",
    "local_time_policy": ""
  },
  "card_patches": [],
  "gaps": [],
  "rejected_claims": []
}"#,
        );

        let result = validate_prompt_output_file(&root, &path, None, Some("extract-claims-proof"))
            .expect("validation should return diagnostics");
        let codes: Vec<&str> = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter_map(|issue| issue["code"].as_str())
            .collect();

        assert_eq!(result["valid"], false);
        assert!(codes.contains(&"runtime_context_now_utc_format"));
        assert!(codes.contains(&"runtime_context_date_utc_format"));
        assert!(codes.contains(&"runtime_context_timezone"));
        assert!(codes.contains(&"runtime_context_local_time_policy"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_runtime_context_date_mismatch() {
        let root = temp_pack("runtime-context-mismatch");
        let path = write_output(
            &root,
            "claims-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "extract-claims-proof",
  "source_summary": {
    "company_domain": "N/A",
    "company_name": "N/A",
    "person_name": "N/A",
    "person_title": "N/A",
    "account_name": "N/A",
    "inputs_used": ["source_notes", "runtime_context"],
    "confidence": "medium"
  },
  "runtime_context": {
    "contract": "mdp.runtime-context.v0",
    "now_utc": "2026-07-02T00:15:00Z",
    "date_utc": "2026-07-03",
    "timezone": "UTC",
    "local_time_policy": "Use supplied source fields for fiscal/calendar logic."
  },
  "card_patches": [],
  "gaps": [],
  "rejected_claims": []
}"#,
        );

        let result = validate_prompt_output_file(&root, &path, None, Some("extract-claims-proof"))
            .expect("validation should return diagnostics");
        let codes: Vec<&str> = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter_map(|issue| issue["code"].as_str())
            .collect();

        assert_eq!(result["valid"], false);
        assert!(codes.contains(&"runtime_context_date_utc_mismatch"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_markdown_wrapped_json() {
        let root = temp_pack("markdown");
        let path = write_output(
            &root,
            "claims-output.md",
            "```json\n{\"contract\":\"mdp.prompt-output.v0\",\"prompt_id\":\"extract-claims-proof\",\"source_summary\":{\"company_domain\":\"N/A\",\"company_name\":\"N/A\",\"inputs_used\":[\"source_notes\"],\"confidence\":\"medium\"},\"card_patches\":[],\"gaps\":[],\"rejected_claims\":[]}\n```",
        );

        let result = validate_prompt_output_file(&root, &path, None, Some("extract-claims-proof"))
            .expect("validation should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "prompt_output_markdown_wrapped")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_wrong_prompt_id() {
        let root = temp_pack("wrong-id");
        let path = write_output(
            &root,
            "claims-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "extract-hooks",
  "source_summary": {
    "company_domain": "N/A",
    "company_name": "N/A",
    "inputs_used": ["source_notes"],
    "confidence": "medium"
  },
  "card_patches": [],
  "gaps": [],
  "rejected_claims": []
}"#,
        );

        let result = validate_prompt_output_file(&root, &path, None, Some("extract-claims-proof"))
            .expect("validation should return diagnostics");

        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "prompt_output_prompt_id_mismatch")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_undeclared_references_and_empty_inputs_used() {
        let root = temp_pack("undeclared");
        let path = write_output(
            &root,
            "output-rules.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "extract-output-rules",
  "source_summary": {
    "company_domain": "N/A",
    "company_name": "N/A",
    "inputs_used": [],
    "confidence": "medium"
  },
  "card_patches": [
    {
      "card_id": "output-rules",
      "kind": "output-rules",
      "entries": [
        {
          "id": "avoid-em-dashes-review",
          "title": "Avoid em dashes",
          "body": "Do not use em dashes in generated copy.",
          "applies_to": ["PMM"],
          "evidence": ["style_guidance"],
          "avoid": ["—"],
          "confidence": "medium",
          "provenance": ["style_guidance: supplied style preference"],
          "status": "needs-review",
          "notes": []
        }
      ]
    }
  ],
  "gaps": [],
  "rejected_claims": []
}"#,
        );

        let result = validate_prompt_output_file(&root, &path, None, Some("extract-output-rules"))
            .expect("validation should return diagnostics");

        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "prompt_output_evidence_reference_undeclared")
        );
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "prompt_output_inputs_used_empty")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_wrong_card_kind_and_existing_id_collision() {
        let root = temp_pack("card-kind");
        let path = write_output(
            &root,
            "claims-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "extract-claims-proof",
  "source_summary": {
    "company_domain": "N/A",
    "company_name": "N/A",
    "inputs_used": ["source_notes"],
    "confidence": "medium"
  },
  "card_patches": [
    {
      "card_id": "claims",
      "kind": "hooks",
      "entries": [
        {
          "id": "wrong-kind-entry",
          "title": "Local decision context",
          "body": "Supported claim",
          "applies_to": ["PMM"],
          "evidence": ["source_notes"],
          "avoid": [],
          "confidence": "medium",
          "provenance": ["source_notes: supplied source notes"],
          "status": "needs-review",
          "notes": []
        }
      ]
    },
    {
      "card_id": "claims",
      "kind": "claims",
      "entries": [
        {
          "id": "local-offline",
          "title": "Local offline CLI",
          "body": "Supported claim",
          "applies_to": ["PMM"],
          "evidence": ["source_notes"],
          "avoid": [],
          "confidence": "medium",
          "provenance": ["source_notes: supplied source notes"],
          "status": "needs-review",
          "notes": []
        }
      ]
    }
  ],
  "gaps": [],
  "rejected_claims": []
}"#,
        );

        let result = validate_prompt_output_file(&root, &path, None, Some("extract-claims-proof"))
            .expect("validation should return diagnostics");

        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "prompt_output_patch_kind_invalid")
        );
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "prompt_output_candidate_id_collision")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_fake_person_normalization_output() {
        let root = temp_pack("fake-person");
        let path = write_output(
            &root,
            "normalize-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "normalize-prospect-row",
  "source_summary": {
    "company_domain": "example.com",
    "company_name": "ExampleCo",
    "person_name": "N/A",
    "person_title": "N/A",
    "account_name": "ExampleCo",
    "inputs_used": ["existing_pack_context"],
    "confidence": "low"
  },
  "normalized_prospect": {
    "name": "Alex Rivera",
    "title": "Revenue Operations Lead",
    "company": "ExampleCo"
  },
  "normalization_trace": {
    "persona": {},
    "fit_readiness": {},
    "preserved_raw_fields": [],
    "missing_required": []
  },
  "card_patches": [],
  "gaps": [],
  "rejected_claims": []
}"#,
        );

        let result =
            validate_prompt_output_file(&root, &path, None, Some("normalize-prospect-row"))
                .expect("validation should return diagnostics");

        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "prompt_output_fake_person")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_non_pack_persona_and_segment_values() {
        let root = temp_pack("value-contracts");
        let path = write_output(
            &root,
            "normalize-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "normalize-prospect-row",
  "source_summary": {
    "company_domain": "example.com",
    "company_name": "ExampleCo",
    "person_name": "Alex Rivera",
    "person_title": "Sales Development Lead",
    "account_name": "ExampleCo",
    "inputs_used": ["raw_row"],
    "confidence": "medium"
  },
  "normalized_prospect": {
    "name": "Alex Rivera",
    "title": "Sales Development Lead",
    "company": "ExampleCo",
    "company_domain": "example.com",
    "source_kind": "user-provided-row",
    "persona": "Sales Development",
    "segment": "enterprise SaaS",
    "trigger": "testing a value contract",
    "signals": [
      {
        "id": "contract-test",
        "title": "Contract test",
        "source": "raw_row.note"
      }
    ],
    "attributes": {
      "fiscal_year": "FY2027"
    }
  },
  "normalization_trace": {
    "persona": {},
    "fit_readiness": {},
    "preserved_raw_fields": ["raw_row"],
    "missing_required": []
  },
  "card_patches": [],
  "gaps": [],
  "rejected_claims": []
}"#,
        );

        let result =
            validate_prompt_output_file(&root, &path, None, Some("normalize-prospect-row"))
                .expect("validation should return diagnostics");
        let codes: Vec<&str> = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter_map(|issue| issue["code"].as_str())
            .collect();

        assert_eq!(result["valid"], false);
        assert!(codes.contains(&"value_contract_persona_unrecognized"));
        assert!(codes.contains(&"value_contract_enum_mismatch"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_accepts_pack_persona_alias_for_persona_contract() {
        let root = temp_pack("persona-alias-contract");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace(
                "  value_contracts:\n    segment:",
                "  value_contracts:\n    persona:\n      type: string\n      enum:\n      - GTM Engineering\n      description: Pack-owned persona labels accepted from normalization prompts.\n    segment:",
            ),
        )
        .expect("manifest should be writable");
        let path = write_output(
            &root,
            "normalize-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "normalize-prospect-row",
  "source_summary": {
    "company_domain": "example.com",
    "company_name": "ExampleCo",
    "person_name": "Taylor Lee",
    "person_title": "Revenue Operations Lead",
    "account_name": "ExampleCo",
    "inputs_used": ["raw_row"],
    "confidence": "medium"
  },
  "normalized_prospect": {
    "name": "Taylor Lee",
    "title": "Revenue Operations Lead",
    "company": "ExampleCo",
    "company_domain": "example.com",
    "source_kind": "user-provided-row",
    "persona": "revops",
    "segment": "agent-assisted GTM",
    "trigger": "testing a persona alias value contract",
    "signals": [
      {
        "id": "contract-test",
        "title": "Contract test",
        "source": "raw_row.note"
      }
    ]
  },
  "normalization_trace": {
    "persona": {},
    "fit_readiness": {},
    "preserved_raw_fields": ["raw_row"],
    "missing_required": []
  },
  "card_patches": [],
  "gaps": [],
  "rejected_claims": []
}"#,
        );

        let result =
            validate_prompt_output_file(&root, &path, None, Some("normalize-prospect-row"))
                .expect("validation should return diagnostics");

        assert_eq!(result["valid"], true);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_accepts_title_mapped_persona_contract_when_persona_omitted() {
        let root = temp_pack("title-mapped-persona-contract");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace(
                "  value_contracts:\n    segment:",
                "  value_contracts:\n    persona:\n      type: string\n      enum:\n      - PMM\n      required: true\n      description: Pack-owned persona labels accepted from normalization prompts.\n    segment:",
            ),
        )
        .expect("manifest should be writable");
        let path = write_output(
            &root,
            "normalize-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "normalize-prospect-row",
  "source_summary": {
    "company_domain": "example.com",
    "company_name": "ExampleCo",
    "person_name": "Taylor Lee",
    "person_title": "Demand Generation Manager",
    "account_name": "ExampleCo",
    "inputs_used": ["raw_row"],
    "confidence": "medium"
  },
  "normalized_prospect": {
    "name": "Taylor Lee",
    "title": "Demand Generation Manager",
    "company": "ExampleCo",
    "company_domain": "example.com",
    "source_kind": "user-provided-row",
    "segment": "agent-assisted GTM",
    "trigger": "testing a title-mapped persona value contract",
    "signals": [
      {
        "id": "contract-test",
        "title": "Contract test",
        "source": "raw_row.note"
      }
    ]
  },
  "normalization_trace": {
    "persona": {},
    "fit_readiness": {},
    "preserved_raw_fields": ["raw_row"],
    "missing_required": []
  },
  "card_patches": [],
  "gaps": [],
  "rejected_claims": []
}"#,
        );

        let result =
            validate_prompt_output_file(&root, &path, None, Some("normalize-prospect-row"))
                .expect("validation should return diagnostics");

        assert_eq!(result["valid"], true);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_invalid_declared_attribute_date() {
        let root = temp_pack("attribute-date-contract");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace(
                "    fiscal_year:\n      type: string\n      description: Optional reviewed account metadata. Keep proof in signals, not attributes.",
                "    fiscal_year:\n      type: string\n      description: Optional reviewed account metadata. Keep proof in signals, not attributes.\n    next_review_date:\n      type: string\n      format: date\n      required: true",
            ),
        )
        .expect("manifest should be writable");
        let path = write_output(
            &root,
            "normalize-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "normalize-prospect-row",
  "source_summary": {
    "company_domain": "example.com",
    "company_name": "ExampleCo",
    "person_name": "Alex Rivera",
    "person_title": "GTM Engineering Lead",
    "account_name": "ExampleCo",
    "inputs_used": ["raw_row"],
    "confidence": "medium"
  },
  "normalized_prospect": {
    "name": "Alex Rivera",
    "title": "GTM Engineering Lead",
    "company": "ExampleCo",
    "company_domain": "example.com",
    "source_kind": "user-provided-row",
    "persona": "GTM Engineering",
    "segment": "agent-assisted GTM",
    "trigger": "testing a value contract",
    "signals": [
      {
        "id": "contract-test",
        "title": "Contract test",
        "source": "raw_row.note"
      }
    ],
    "attributes": {
      "fiscal_year": "FY2027",
      "next_review_date": "next Friday"
    }
  },
  "normalization_trace": {
    "persona": {},
    "fit_readiness": {},
    "preserved_raw_fields": ["raw_row"],
    "missing_required": []
  },
  "card_patches": [],
  "gaps": [],
  "rejected_claims": []
}"#,
        );

        let result =
            validate_prompt_output_file(&root, &path, None, Some("normalize-prospect-row"))
                .expect("validation should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "value_contract_format_mismatch")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_missing_required_attribute_object() {
        let root = temp_pack("missing-required-attribute-object");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace(
                "    fiscal_year:\n      type: string\n      description: Optional reviewed account metadata. Keep proof in signals, not attributes.",
                "    fiscal_year:\n      type: string\n      description: Optional reviewed account metadata. Keep proof in signals, not attributes.\n      required: true",
            ),
        )
        .expect("manifest should be writable");
        let path = write_output(
            &root,
            "normalize-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "normalize-prospect-row",
  "source_summary": {
    "company_domain": "example.com",
    "company_name": "ExampleCo",
    "person_name": "Alex Rivera",
    "person_title": "GTM Engineering Lead",
    "account_name": "ExampleCo",
    "inputs_used": ["raw_row"],
    "confidence": "medium"
  },
  "normalized_prospect": {
    "name": "Alex Rivera",
    "title": "GTM Engineering Lead",
    "company": "ExampleCo",
    "company_domain": "example.com",
    "source_kind": "user-provided-row",
    "persona": "GTM Engineering",
    "segment": "agent-assisted GTM",
    "trigger": "testing a value contract",
    "signals": [
      {
        "id": "contract-test",
        "title": "Contract test",
        "source": "raw_row.note"
      }
    ]
  },
  "normalization_trace": {
    "persona": {},
    "fit_readiness": {},
    "preserved_raw_fields": ["raw_row"],
    "missing_required": []
  },
  "card_patches": [],
  "gaps": [],
  "rejected_claims": []
}"#,
        );

        let result =
            validate_prompt_output_file(&root, &path, None, Some("normalize-prospect-row"))
                .expect("validation should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "value_contract_required_missing"
                    && issue["path"].as_str().is_some_and(
                        |path| path.ends_with("#/normalized_prospect/attributes/fiscal_year")
                    ))
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_non_string_candidate_entry_arrays() {
        let root = temp_pack("non-string-arrays");
        let path = write_output(
            &root,
            "claims-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "extract-claims-proof",
  "source_summary": {
    "company_domain": "N/A",
    "company_name": "N/A",
    "inputs_used": ["source_notes"],
    "confidence": "medium"
  },
  "card_patches": [
    {
      "card_id": "claims",
      "kind": "claims",
      "entries": [
        {
          "id": "claim-reviewed-local-context",
          "title": "Local decision context",
          "body": "Supplied source material describes the product as local decision context for GTM messaging.",
          "applies_to": ["PMM", 5],
          "evidence": ["source_notes"],
          "avoid": [false],
          "confidence": "medium",
          "provenance": ["source_notes: supplied source notes"],
          "status": "needs-review",
          "notes": [123]
        }
      ]
    }
  ],
  "gaps": [],
  "rejected_claims": []
}"#,
        );

        let result = validate_prompt_output_file(&root, &path, None, Some("extract-claims-proof"))
            .expect("validation should return diagnostics");

        for code in [
            "prompt_output_applies_to_type",
            "prompt_output_avoid_type",
            "prompt_output_notes_type",
        ] {
            assert!(
                result["issues"]
                    .as_array()
                    .expect("issues array")
                    .iter()
                    .any(|issue| issue["code"] == code),
                "expected issue code {code}"
            );
        }

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_non_string_schema_fields() {
        let root = temp_pack("non-string-schema-fields");
        let path = write_output(
            &root,
            "claims-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "extract-claims-proof",
  "source_summary": {
    "company_domain": "N/A",
    "company_name": "N/A",
    "person_name": 5,
    "person_title": "N/A",
    "account_name": "N/A",
    "inputs_used": ["source_notes"],
    "confidence": "medium"
  },
  "card_patches": [],
  "gaps": [1],
  "rejected_claims": [
    {
      "claim": "x",
      "reason": "y",
      "source": 5
    }
  ]
}"#,
        );

        let result = validate_prompt_output_file(&root, &path, None, Some("extract-claims-proof"))
            .expect("validation should return diagnostics");

        for code in [
            "prompt_output_source_summary_field_type",
            "prompt_output_gaps_type",
            "prompt_output_rejected_claim_field_missing",
        ] {
            assert!(
                result["issues"]
                    .as_array()
                    .expect("issues array")
                    .iter()
                    .any(|issue| issue["code"] == code),
                "expected issue code {code}"
            );
        }

        let _ = std::fs::remove_dir_all(root);
    }
}
