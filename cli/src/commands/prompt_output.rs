use crate::constants::{DEFAULT_DIR, PROMPT_OUTPUT_CONTRACT, SOURCE_AUDIT_CONTRACT};
use crate::models::{CardKind, Manifest, PromptFile};
use crate::pack_io::{read_card, read_manifest, read_prompt, resolve_pack_path};
use crate::runtime_context::validate_runtime_context;
use crate::utils::{normalize_supplied_company_domain, resolve_pack_persona_label};
use crate::value_contracts::normalized_prospect_contract_violations;
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
pub(crate) fn validate_prompt_output_file(
    root: &Path,
    file: &Path,
    prompt_path: Option<&Path>,
    prompt_id: Option<&str>,
) -> Result<Value> {
    validate_prompt_output_file_with_source_audit(root, file, prompt_path, prompt_id, None)
}

pub(crate) fn validate_prompt_output_file_with_source_audit(
    root: &Path,
    file: &Path,
    prompt_path: Option<&Path>,
    prompt_id: Option<&str>,
    source_audit_path: Option<&Path>,
) -> Result<Value> {
    if prompt_path.is_some() && prompt_id.is_some() {
        return Err(anyhow!("pass at most one of --prompt and --prompt-id"));
    }

    let prompt = resolve_prompt(root, prompt_path, prompt_id)?;
    let artifact_path = display_path(file);
    let raw = fs::read_to_string(file)?;
    let mut issues = Vec::new();
    let source_audit = match source_audit_path {
        Some(path) => match read_source_audit_file(path) {
            Ok(value) => Some((value, display_path(path))),
            Err(err) => {
                issues.push(issue(
                    "source_audit_parse_failed",
                    "error",
                    display_path(path),
                    err.to_string(),
                ));
                None
            }
        },
        None => None,
    };
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

    validate_prompt_output_parsed(
        root,
        &prompt,
        &output,
        &artifact_path,
        issues,
        source_audit
            .as_ref()
            .map(|(value, path)| (value, path.as_str())),
    )
}

#[allow(dead_code)]
pub(crate) fn validate_prompt_output_value(
    root: &Path,
    output: &Value,
    artifact_path: &str,
    prompt_path: Option<&Path>,
    prompt_id: Option<&str>,
) -> Result<Value> {
    validate_prompt_output_value_with_source_audit(
        root,
        output,
        artifact_path,
        prompt_path,
        prompt_id,
        None,
        None,
    )
}

pub(crate) fn validate_prompt_output_value_with_source_audit(
    root: &Path,
    output: &Value,
    artifact_path: &str,
    prompt_path: Option<&Path>,
    prompt_id: Option<&str>,
    source_audit: Option<&Value>,
    source_audit_path: Option<&str>,
) -> Result<Value> {
    if prompt_path.is_some() && prompt_id.is_some() {
        return Err(anyhow!("pass at most one of prompt or prompt_id"));
    }

    let prompt = resolve_prompt(root, prompt_path, prompt_id)?;
    validate_prompt_output_parsed(
        root,
        &prompt,
        output,
        artifact_path,
        Vec::new(),
        source_audit.map(|value| (value, source_audit_path.unwrap_or("source_audit"))),
    )
}

fn validate_prompt_output_parsed(
    root: &Path,
    prompt: &PromptFile,
    output: &Value,
    artifact_path: &str,
    mut issues: Vec<Value>,
    source_audit: Option<(&Value, &str)>,
) -> Result<Value> {
    let manifest = read_manifest(root)?;
    validate_output_against_prompt(&manifest, prompt, output, artifact_path, &mut issues);
    validate_card_collisions(root, prompt, output, artifact_path, &mut issues)?;
    let source_audit = match source_audit {
        Some((value, source_audit_path)) => validate_source_audit(
            root,
            prompt,
            value,
            source_audit_path,
            output,
            artifact_path,
            &mut issues,
        )?,
        None => None,
    };

    let mut result = json!({
        "valid": issues.is_empty(),
        "file": artifact_path,
        "prompt": prompt_summary(prompt, root),
        "issues": issues
    });
    if let Some(source_audit) = source_audit {
        if let Some(object) = result.as_object_mut() {
            object.insert("source_audit".to_string(), source_audit.summary());
        }
    }
    Ok(result)
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

const MAX_SOURCE_AUDIT_SNIPPET_CHARS: usize = 1000;

#[derive(Debug)]
struct SourceAuditIndex {
    refs: BTreeMap<String, SourceAuditRef>,
    audited_inputs: BTreeSet<String>,
}

impl SourceAuditIndex {
    fn summary(&self) -> Value {
        json!({
            "contract": SOURCE_AUDIT_CONTRACT,
            "ref_count": self.refs.len(),
            "audited_inputs": self.audited_inputs.iter().cloned().collect::<Vec<_>>()
        })
    }
}

#[derive(Debug)]
struct SourceAuditRef {
    source_id: String,
    snippet: String,
}

#[derive(Debug)]
struct PromptSourceRef {
    path: String,
    reference: String,
}

fn read_source_audit_file(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path)?;
    serde_json::from_str::<Value>(&raw)
        .map_err(|_| anyhow!("source audit file must contain valid JSON"))
}

fn validate_source_audit(
    root: &Path,
    prompt: &PromptFile,
    value: &Value,
    source_audit_path: &str,
    output: &Value,
    output_path: &str,
    issues: &mut Vec<Value>,
) -> Result<Option<SourceAuditIndex>> {
    let Some(audit) = value.as_object() else {
        issues.push(issue(
            "source_audit_type",
            "error",
            source_audit_path,
            "source_audit must be a JSON object",
        ));
        return Ok(None);
    };

    validate_json_object_keys(
        audit,
        &["contract", "refs"],
        source_audit_path,
        "source_audit_unknown_field",
        issues,
    );

    if value["contract"].as_str() != Some(SOURCE_AUDIT_CONTRACT) {
        issues.push(issue(
            "source_audit_contract_mismatch",
            "error",
            format!("{source_audit_path}#/contract"),
            format!("source_audit contract must be {SOURCE_AUDIT_CONTRACT}"),
        ));
    }

    let declared_inputs = prompt
        .inputs
        .iter()
        .map(|input| input.name.clone())
        .collect::<BTreeSet<_>>();
    let source_ids = match read_source_ledger_ids(root) {
        Ok(source_ids) => Some(source_ids),
        Err(err) => {
            issues.push(issue(
                "source_audit_sources_read_failed",
                "error",
                root.join(DEFAULT_DIR)
                    .join("sources.yaml")
                    .display()
                    .to_string(),
                err.to_string(),
            ));
            None
        }
    };

    let Some(refs) = value["refs"].as_array() else {
        issues.push(issue(
            "source_audit_refs_type",
            "error",
            format!("{source_audit_path}#/refs"),
            "source_audit.refs must be an array",
        ));
        return Ok(None);
    };
    if refs.is_empty() {
        issues.push(issue(
            "source_audit_refs_empty",
            "error",
            format!("{source_audit_path}#/refs"),
            "source_audit.refs must include at least one audited source reference",
        ));
    }

    let mut index = SourceAuditIndex {
        refs: BTreeMap::new(),
        audited_inputs: BTreeSet::new(),
    };

    for (ref_index, audit_ref) in refs.iter().enumerate() {
        let ref_path = format!("{source_audit_path}#/refs/{ref_index}");
        let Some(audit_ref) = audit_ref.as_object() else {
            issues.push(issue(
                "source_audit_ref_type",
                "error",
                &ref_path,
                "source_audit refs must be objects",
            ));
            continue;
        };
        validate_json_object_keys(
            audit_ref,
            &["ref", "source_id", "locator", "snippet", "confidence"],
            &ref_path,
            "source_audit_ref_unknown_field",
            issues,
        );

        let source_ref = audit_ref
            .get("ref")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        let source_id = audit_ref
            .get("source_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        let snippet = audit_ref
            .get("snippet")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        for (field, field_value) in [
            ("ref", source_ref),
            ("source_id", source_id),
            ("snippet", snippet),
        ] {
            if field_value.is_empty() {
                issues.push(issue(
                    "source_audit_ref_field_missing",
                    "error",
                    format!("{ref_path}/{field}"),
                    format!("source_audit refs must include non-empty {field}"),
                ));
            }
        }
        if source_ref.is_empty() {
            continue;
        }
        if !references_declared_input(source_ref, &declared_inputs) {
            issues.push(issue(
                "source_audit_ref_undeclared",
                "error",
                format!("{ref_path}/ref"),
                format!("source_audit ref {source_ref} does not match a declared prompt input"),
            ));
        }
        if let Some(source_ids) = &source_ids {
            if !source_id.is_empty() && !source_ids.contains(source_id) {
                issues.push(issue(
                    "source_audit_source_id_missing",
                    "error",
                    format!("{ref_path}/source_id"),
                    format!(
                        "source_audit source_id {source_id} does not exist in .mdp/sources.yaml"
                    ),
                ));
            }
        }
        if snippet.chars().count() > MAX_SOURCE_AUDIT_SNIPPET_CHARS {
            issues.push(issue(
                "source_audit_snippet_too_long",
                "error",
                format!("{ref_path}/snippet"),
                format!(
                    "source_audit snippets must be at most {MAX_SOURCE_AUDIT_SNIPPET_CHARS} characters"
                ),
            ));
        }
        if index
            .refs
            .insert(
                source_ref.to_string(),
                SourceAuditRef {
                    source_id: source_id.to_string(),
                    snippet: snippet.to_string(),
                },
            )
            .is_some()
        {
            issues.push(issue(
                "source_audit_ref_duplicate",
                "error",
                format!("{ref_path}/ref"),
                format!("duplicate source_audit ref {source_ref}"),
            ));
        }
        index
            .audited_inputs
            .insert(reference_input_root(source_ref).to_string());
    }

    validate_prompt_output_refs_against_source_audit(
        output,
        output_path,
        &index,
        &declared_inputs,
        issues,
    );
    Ok(Some(index))
}

fn read_source_ledger_ids(root: &Path) -> Result<BTreeSet<String>> {
    let path = root.join(DEFAULT_DIR).join("sources.yaml");
    let raw = fs::read_to_string(&path)?;
    let value: Value = serde_yaml::from_str(&raw)?;
    Ok(value["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|source| source["id"].as_str().map(str::to_string))
        .collect())
}

fn validate_prompt_output_refs_against_source_audit(
    output: &Value,
    output_path: &str,
    audit: &SourceAuditIndex,
    declared_inputs: &BTreeSet<String>,
    issues: &mut Vec<Value>,
) {
    for source_ref in collect_prompt_output_source_refs(output, output_path) {
        let (base_ref, snippet_ref) = split_source_reference(&source_ref.reference);
        if base_ref.is_empty() {
            continue;
        }
        let input_root = reference_input_root(&base_ref);
        if !declared_inputs.contains(input_root) || source_audit_exempt_input(input_root) {
            continue;
        }
        let Some(audit_ref) = audit.refs.get(&base_ref) else {
            issues.push(issue(
                "prompt_output_source_ref_missing",
                "error",
                source_ref.path,
                format!("source reference {base_ref} is not present in source_audit.refs"),
            ));
            continue;
        };
        if let Some(snippet_ref) = snippet_ref {
            if !contains_normalized_snippet(&audit_ref.snippet, &snippet_ref) {
                issues.push(issue(
                    "prompt_output_source_snippet_missing",
                    "error",
                    source_ref.path,
                    format!(
                        "source reference {base_ref} cites a snippet that is not present in source_audit ref from source_id {}",
                        audit_ref.source_id
                    ),
                ));
            }
        }
    }
}

fn source_audit_exempt_input(input_root: &str) -> bool {
    matches!(
        input_root,
        "existing_pack_context" | "runtime_context" | "source_audit"
    )
}

fn collect_prompt_output_source_refs(output: &Value, output_path: &str) -> Vec<PromptSourceRef> {
    let mut refs = Vec::new();
    if let Some(card_patches) = output.get("card_patches").and_then(Value::as_array) {
        for (patch_index, patch) in card_patches.iter().enumerate() {
            if let Some(entries) = patch.get("entries").and_then(Value::as_array) {
                for (entry_index, entry) in entries.iter().enumerate() {
                    collect_source_ref_array(
                        entry.get("evidence"),
                        &format!(
                            "{output_path}#/card_patches/{patch_index}/entries/{entry_index}/evidence"
                        ),
                        &mut refs,
                    );
                    collect_source_ref_array(
                        entry.get("provenance"),
                        &format!(
                            "{output_path}#/card_patches/{patch_index}/entries/{entry_index}/provenance"
                        ),
                        &mut refs,
                    );
                }
            }
        }
    }
    if let Some(signals) = output
        .pointer("/normalized_prospect/signals")
        .and_then(Value::as_array)
    {
        for (index, signal) in signals.iter().enumerate() {
            if let Some(reference) = signal.get("source").and_then(Value::as_str) {
                refs.push(PromptSourceRef {
                    path: format!("{output_path}#/normalized_prospect/signals/{index}/source"),
                    reference: reference.to_string(),
                });
            }
        }
    }
    if let Some(reference) = output
        .pointer("/normalization_trace/persona/source")
        .and_then(Value::as_str)
    {
        refs.push(PromptSourceRef {
            path: format!("{output_path}#/normalization_trace/persona/source"),
            reference: reference.to_string(),
        });
    }
    collect_source_ref_array(
        output.pointer("/normalization_trace/preserved_raw_fields"),
        &format!("{output_path}#/normalization_trace/preserved_raw_fields"),
        &mut refs,
    );
    if let Some(missing_required) = output
        .pointer("/normalization_trace/missing_required")
        .and_then(Value::as_array)
    {
        for (index, item) in missing_required.iter().enumerate() {
            let Some(item) = item.as_object() else {
                continue;
            };
            for field in ["path", "source_evidence"] {
                if let Some(reference) = item.get(field).and_then(Value::as_str) {
                    refs.push(PromptSourceRef {
                        path: format!(
                            "{output_path}#/normalization_trace/missing_required/{index}/{field}"
                        ),
                        reference: reference.to_string(),
                    });
                }
            }
        }
    }
    if let Some(rejected_claims) = output.get("rejected_claims").and_then(Value::as_array) {
        for (index, claim) in rejected_claims.iter().enumerate() {
            if let Some(reference) = claim.get("source").and_then(Value::as_str) {
                refs.push(PromptSourceRef {
                    path: format!("{output_path}#/rejected_claims/{index}/source"),
                    reference: reference.to_string(),
                });
            }
        }
    }
    refs
}

fn collect_source_ref_array(value: Option<&Value>, path: &str, refs: &mut Vec<PromptSourceRef>) {
    let Some(items) = value.and_then(Value::as_array) else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        if let Some(reference) = item.as_str() {
            refs.push(PromptSourceRef {
                path: format!("{path}/{index}"),
                reference: reference.to_string(),
            });
        }
    }
}

fn split_source_reference(reference: &str) -> (String, Option<String>) {
    let reference = reference.trim();
    let Some((base, snippet)) = reference.split_once(':') else {
        return (reference.to_string(), None);
    };
    let base = base.trim();
    let snippet = snippet.trim();
    if base.is_empty() || snippet.is_empty() {
        return (reference.to_string(), None);
    }
    (base.to_string(), Some(snippet.to_string()))
}

fn reference_input_root(reference: &str) -> &str {
    let reference = reference.trim();
    let end = reference
        .char_indices()
        .find_map(|(index, character)| {
            if character == '.' || character == '[' || character == ':' {
                Some(index)
            } else {
                None
            }
        })
        .unwrap_or(reference.len());
    &reference[..end]
}

fn contains_normalized_snippet(source: &str, expected: &str) -> bool {
    normalize_snippet(source).contains(&normalize_snippet(expected))
}

fn normalize_snippet(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
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
        validate_normalized_opportunity_alias(
            manifest,
            output.get("normalized_prospect"),
            output.get("normalized_opportunity"),
            &format!("{path}#/normalized_opportunity"),
            issues,
        );
        validate_normalization_trace(
            output.get("normalization_trace"),
            &format!("{path}#/normalization_trace"),
            issues,
        );
        validate_fit_readiness_against_manifest(
            manifest,
            output,
            &format!("{path}#/normalization_trace/fit_readiness"),
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
                format!("source_summary.inputs_used must use declared prompt input names only; {input_name} is not declared"),
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

fn validate_normalized_opportunity_alias(
    manifest: &Manifest,
    normalized_prospect: Option<&Value>,
    normalized_opportunity: Option<&Value>,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let Some(alias) = normalized_opportunity else {
        return;
    };

    if !manifest
        .profile
        .as_ref()
        .is_some_and(|profile| profile.id == "proposal")
    {
        issues.push(issue(
            "prompt_output_normalized_opportunity_profile",
            "error",
            path,
            "normalized_opportunity is a proposal-profile alias; use normalized_prospect for non-proposal normalization outputs",
        ));
    }

    if !alias.is_object() {
        issues.push(issue(
            "prompt_output_normalized_opportunity_type",
            "error",
            path,
            "normalized_opportunity must be an object when provided",
        ));
        return;
    }

    let Some(prospect) = normalized_prospect else {
        return;
    };
    if alias != prospect {
        issues.push(issue(
            "prompt_output_normalized_opportunity_mismatch",
            "error",
            path,
            "normalized_opportunity must exactly match normalized_prospect; it is a proposal-readable alias, not a separate core opportunity object",
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
    validate_plain_string_array(
        trace.get("preserved_raw_fields"),
        &format!("{path}/preserved_raw_fields"),
        "prompt_output_normalization_trace_field_type",
        "normalization_trace.preserved_raw_fields must be an array of strings",
        issues,
    );
    validate_missing_required_trace(
        trace.get("missing_required"),
        &format!("{path}/missing_required"),
        issues,
    );
}

fn validate_fit_readiness_against_manifest(
    manifest: &Manifest,
    output: &Value,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let Some(prospect) = output.get("normalized_prospect").and_then(Value::as_object) else {
        return;
    };
    let Some(fit_readiness) = output
        .pointer("/normalization_trace/fit_readiness")
        .and_then(Value::as_object)
    else {
        return;
    };

    let ready_value = fit_readiness.get("ready_for_mdp_fit");
    let Some(ready_for_mdp_fit) = ready_value.and_then(Value::as_bool) else {
        issues.push(issue(
            "prompt_output_fit_readiness_ready_type",
            "error",
            format!("{path}/ready_for_mdp_fit"),
            "normalization_trace.fit_readiness.ready_for_mdp_fit must be a boolean",
        ));
        return;
    };

    if !ready_for_mdp_fit {
        return;
    }

    for field in &manifest.lead_input_requirements.required_fields {
        if !normalized_prospect_field_present(manifest, prospect, field) {
            issues.push(issue(
                "prompt_output_fit_readiness_missing_required_field",
                "error",
                format!("{path}/ready_for_mdp_fit"),
                format!(
                    "ready_for_mdp_fit is true, but normalized_prospect is missing required field {field} from manifest lead_input_requirements.required_fields"
                ),
            ));
        }
    }

    for field in &manifest.lead_input_requirements.required_signal_fields {
        let Some(signals) = prospect.get("signals").and_then(Value::as_array) else {
            issues.push(issue(
                "prompt_output_fit_readiness_missing_required_signal_field",
                "error",
                format!("{path}/ready_for_mdp_fit"),
                format!(
                    "ready_for_mdp_fit is true, but normalized_prospect.signals is missing required signal field {field}"
                ),
            ));
            continue;
        };
        if signals.is_empty() {
            issues.push(issue(
                "prompt_output_fit_readiness_missing_required_signal_field",
                "error",
                format!("{path}/ready_for_mdp_fit"),
                format!(
                    "ready_for_mdp_fit is true, but normalized_prospect.signals is empty and required signal field {field} cannot be checked"
                ),
            ));
            continue;
        }
        for (index, signal) in signals.iter().enumerate() {
            if !normalized_signal_field_present(signal, field) {
                issues.push(issue(
                    "prompt_output_fit_readiness_missing_required_signal_field",
                    "error",
                    format!("{path}/ready_for_mdp_fit"),
                    format!(
                        "ready_for_mdp_fit is true, but normalized_prospect.signals[{index}] is missing required field {field}"
                    ),
                ));
            }
        }
    }

    for attribute in &manifest.lead_input_requirements.required_attributes {
        if !normalized_attribute_present(prospect, attribute) {
            issues.push(issue(
                "prompt_output_fit_readiness_missing_required_attribute",
                "error",
                format!("{path}/ready_for_mdp_fit"),
                format!(
                    "ready_for_mdp_fit is true, but normalized_prospect.attributes is missing required attribute {attribute} from manifest lead_input_requirements.required_attributes"
                ),
            ));
        }
    }
}

fn normalized_prospect_field_present(
    manifest: &Manifest,
    prospect: &serde_json::Map<String, Value>,
    field: &str,
) -> bool {
    match field {
        "persona" => normalized_persona_present(manifest, prospect),
        "signals" => prospect
            .get("signals")
            .and_then(Value::as_array)
            .is_some_and(|signals| !signals.is_empty()),
        "synthetic" => true,
        _ => prospect.get(field).is_some_and(meaningful_json_value),
    }
}

fn normalized_persona_present(
    manifest: &Manifest,
    prospect: &serde_json::Map<String, Value>,
) -> bool {
    prospect
        .get("persona")
        .and_then(Value::as_str)
        .is_some_and(|persona| {
            present(persona)
                && resolve_pack_persona_label(manifest, persona, "normalized_prospect.persona")
                    .is_some()
        })
        || prospect
            .get("title")
            .and_then(Value::as_str)
            .is_some_and(|title| {
                present(title)
                    && resolve_pack_persona_label(manifest, title, "normalized_prospect.title")
                        .is_some()
            })
}

fn normalized_signal_field_present(signal: &Value, field: &str) -> bool {
    signal
        .as_object()
        .and_then(|signal| signal.get(field))
        .is_some_and(meaningful_json_value)
}

fn normalized_attribute_present(
    prospect: &serde_json::Map<String, Value>,
    attribute: &str,
) -> bool {
    prospect
        .get("attributes")
        .and_then(Value::as_object)
        .and_then(|attributes| attributes.get(attribute))
        .is_some_and(meaningful_json_value)
}

fn meaningful_json_value(value: &Value) -> bool {
    match value {
        Value::String(value) => present(value),
        Value::Number(_) | Value::Bool(_) => true,
        _ => false,
    }
}

fn present(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && !value.eq_ignore_ascii_case("n/a")
}

fn validate_missing_required_trace(value: Option<&Value>, path: &str, issues: &mut Vec<Value>) {
    let Some(items) = value.and_then(Value::as_array) else {
        issues.push(issue(
            "prompt_output_normalization_trace_field_type",
            "error",
            path,
            "normalization_trace.missing_required must be an array",
        ));
        return;
    };

    for (index, item) in items.iter().enumerate() {
        let item_path = format!("{path}/{index}");
        if item.as_str().is_some() {
            continue;
        }
        let Some(item) = item.as_object() else {
            issues.push(issue(
                "prompt_output_missing_required_item_type",
                "error",
                &item_path,
                "missing_required entries must be strings or objects with field and reason",
            ));
            continue;
        };
        validate_json_object_keys(
            item,
            &["field", "path", "reason", "source_evidence"],
            &item_path,
            "prompt_output_missing_required_unknown_field",
            issues,
        );
        for field in ["field", "reason"] {
            if item
                .get(field)
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                issues.push(issue(
                    "prompt_output_missing_required_field_missing",
                    "error",
                    format!("{item_path}/{field}"),
                    format!("missing_required object entries must include non-empty {field}"),
                ));
            }
        }
        for field in ["path", "source_evidence"] {
            if let Some(value) = item.get(field) {
                if value.as_str().is_none_or(|value| value.trim().is_empty()) {
                    issues.push(issue(
                        "prompt_output_missing_required_field_type",
                        "error",
                        format!("{item_path}/{field}"),
                        format!("missing_required.{field} must be a non-empty string when present"),
                    ));
                }
            }
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
                format!("reference {reference} must start with a declared prompt input name"),
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
        fields.push("normalized_opportunity");
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

    fn temp_pack_with_template(name: &str, template: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-prompt-output-{name}-{nonce}"));
        init_pack(&root, "Example Message Pack", template, true, false)
            .expect("starter pack should initialize");
        root
    }

    fn temp_pack(name: &str) -> PathBuf {
        temp_pack_with_template(name, "gtm")
    }

    fn write_output(root: &Path, name: &str, body: &str) -> PathBuf {
        let path = root.join(name);
        std::fs::write(&path, body).expect("output fixture should be writable");
        path
    }

    fn write_json_output(root: &Path, name: &str, body: &Value) -> PathBuf {
        write_output(
            root,
            name,
            &serde_json::to_string_pretty(body).expect("output fixture should serialize"),
        )
    }

    fn proposal_opportunity_alias_output() -> Value {
        let normalized = json!({
            "name": "N/A",
            "title": "N/A",
            "company": "Example Public Services Agency",
            "company_domain": "public-services.example",
            "source_kind": "synthetic-example",
            "synthetic": true,
            "background": "Neutral synthetic opportunity context says a public services buyer needs local-first review of supplied requirements, proof gaps, and bid/no-bid risks.",
            "trigger": "Synthetic public services opportunity needs bid/no-bid and compliance review before proposal drafting.",
            "persona": "Proposal Lead",
            "segment": "public-services-review",
            "attributes": {
                "opportunity_stage": "bid-no-bid",
                "pursuit_decision": "needs-more-info",
                "source_safety": "synthetic"
            },
            "signals": [
                {
                    "id": "public-services-review-context",
                    "title": "Public services review context supplied",
                    "source": "raw_opportunity.summary",
                    "confidence": "medium",
                    "freshness": "synthetic",
                    "state_as": "supplied"
                }
            ]
        });
        json!({
            "contract": "mdp.prompt-output.v0",
            "prompt_id": "normalize-opportunity",
            "source_summary": {
                "company_domain": "public-services.example",
                "company_name": "Example Public Services Agency",
                "person_name": "N/A",
                "person_title": "N/A",
                "account_name": "Neutral proposal contract fixture",
                "inputs_used": ["raw_opportunity", "existing_pack_context", "source_kind"],
                "confidence": "medium"
            },
            "normalized_prospect": normalized.clone(),
            "normalized_opportunity": normalized,
            "normalization_trace": {
                "persona": {
                    "source": "existing_pack_context.personas",
                    "matched_keywords": ["bid/no-bid"],
                    "confidence": "high",
                    "needs_review": false
                },
                "fit_readiness": {
                    "ready_for_mdp_fit": true
                },
                "preserved_raw_fields": ["raw_opportunity.summary", "source_kind"],
                "missing_required": []
            },
            "card_patches": [],
            "gaps": [],
            "rejected_claims": []
        })
    }

    #[test]
    fn validate_accepts_proposal_source_audit_refs() {
        let root = temp_pack_with_template("proposal-source-audit-valid", "proposal");
        let output_path = write_output(
            &root,
            "normalize-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "normalize-opportunity",
  "source_summary": {
    "company_domain": "public-services.example",
    "company_name": "Example Public Services Agency",
    "person_name": "N/A",
    "person_title": "N/A",
    "account_name": "Neutral proposal contract fixture",
    "inputs_used": ["raw_opportunity", "existing_pack_context", "source_kind", "source_audit"],
    "confidence": "medium"
  },
  "normalized_prospect": {
    "name": "N/A",
    "title": "N/A",
    "company": "Example Public Services Agency",
    "company_domain": "public-services.example",
    "source_kind": "synthetic-example",
    "synthetic": true,
    "background": "Neutral synthetic opportunity context says a public services buyer needs local-first review of supplied requirements, proof gaps, and bid/no-bid risks.",
    "trigger": "Synthetic public services opportunity needs bid/no-bid and compliance review before proposal drafting.",
    "persona": "Proposal Lead",
    "segment": "public-services-review",
    "attributes": {
      "opportunity_stage": "bid-no-bid",
      "pursuit_decision": "needs-more-info",
      "source_safety": "synthetic"
    },
    "signals": [
      {
        "id": "public-services-review-context",
        "title": "Public services review context supplied",
        "source": "raw_opportunity.summary: service request intake, status notifications, reporting, and training",
        "confidence": "medium",
        "freshness": "synthetic",
        "state_as": "supplied"
      },
      {
        "id": "due-date-supplied",
        "title": "Proposal due date supplied",
        "source": "raw_opportunity.due_date",
        "confidence": "medium",
        "freshness": "synthetic",
        "state_as": "supplied"
      }
    ]
  },
  "normalization_trace": {
    "persona": {
      "source": "existing_pack_context.personas",
      "matched_keywords": ["bid/no-bid"],
      "confidence": "high",
      "needs_review": false
    },
    "fit_readiness": {
      "has_customer_or_agency": true,
      "has_due_date": true,
      "has_requirement_signal": true,
      "has_review_mode": true,
      "has_signal_source": true,
      "ready_for_mdp_fit": true
    },
    "preserved_raw_fields": [
      "raw_opportunity.customer",
      "raw_opportunity.summary",
      "raw_opportunity.due_date",
      "raw_opportunity.review_mode",
      "source_kind"
    ],
    "missing_required": []
  },
  "card_patches": [],
  "gaps": [],
  "rejected_claims": []
}"#,
        );
        let audit_path = write_output(
            &root,
            "source-audit.json",
            r#"{
  "contract": "mdp.source-audit.v0",
  "refs": [
    {
      "ref": "raw_opportunity.customer",
      "source_id": "synthetic-rfp-summary",
      "locator": "synthetic-rfp-summary#customer",
      "snippet": "Example Public Services Agency issued a fictional public services modernization RFP."
    },
    {
      "ref": "raw_opportunity.summary",
      "source_id": "synthetic-rfp-summary",
      "locator": "synthetic-rfp-summary#summary",
      "snippet": "The fictional RFP asks for service request intake, status notifications, reporting, and training."
    },
    {
      "ref": "raw_opportunity.due_date",
      "source_id": "synthetic-rfp-summary",
      "locator": "synthetic-rfp-summary#due-date",
      "snippet": "The response is due in six weeks in the synthetic scenario."
    },
    {
      "ref": "raw_opportunity.review_mode",
      "source_id": "synthetic-rfp-summary",
      "locator": "synthetic-rfp-summary#review-mode",
      "snippet": "The supplied synthetic review mode is bid/no-bid."
    },
    {
      "ref": "source_kind",
      "source_id": "synthetic-rfp-summary",
      "locator": "synthetic-rfp-summary#source-kind",
      "snippet": "synthetic-example"
    }
  ]
}"#,
        );

        let result = validate_prompt_output_file_with_source_audit(
            &root,
            &output_path,
            None,
            Some("normalize-opportunity"),
            Some(&audit_path),
        )
        .expect("validation should return diagnostics");

        assert_eq!(result["valid"], true);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_proposal_source_audit_missing_ref_and_snippet_mismatch() {
        let root = temp_pack_with_template("proposal-source-audit-invalid", "proposal");
        let output_path = write_output(
            &root,
            "normalize-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "normalize-opportunity",
  "source_summary": {
    "company_domain": "public-services.example",
    "company_name": "Example Public Services Agency",
    "person_name": "N/A",
    "person_title": "N/A",
    "account_name": "Neutral proposal contract fixture",
    "inputs_used": ["raw_opportunity", "existing_pack_context", "source_kind", "source_audit"],
    "confidence": "medium"
  },
  "normalized_prospect": {
    "name": "N/A",
    "title": "N/A",
    "company": "Example Public Services Agency",
    "company_domain": "public-services.example",
    "source_kind": "synthetic-example",
    "synthetic": true,
    "background": "Neutral synthetic opportunity context says a public services buyer needs local-first review.",
    "trigger": "Synthetic public services opportunity needs bid/no-bid review before drafting.",
    "persona": "Proposal Lead",
    "segment": "public-services-review",
    "attributes": {
      "opportunity_stage": "bid-no-bid",
      "pursuit_decision": "needs-more-info",
      "source_safety": "synthetic"
    },
    "signals": [
      {
        "id": "invented-integration",
        "title": "Invented integration requirement",
        "source": "raw_opportunity.summary: mandatory live-chat integration",
        "confidence": "medium",
        "freshness": "synthetic",
        "state_as": "supplied"
      },
      {
        "id": "missing-ref",
        "title": "Nonexistent requirement source",
        "source": "raw_opportunity.unlisted_requirement",
        "confidence": "medium",
        "freshness": "synthetic",
        "state_as": "supplied"
      }
    ]
  },
  "normalization_trace": {
    "persona": {
      "source": "existing_pack_context.personas",
      "matched_keywords": ["bid/no-bid"],
      "confidence": "high",
      "needs_review": false
    },
    "fit_readiness": {
      "ready_for_mdp_fit": true
    },
    "preserved_raw_fields": [
      "raw_opportunity.customer",
      "raw_opportunity.summary",
      "raw_opportunity.due_date",
      "raw_opportunity.review_mode",
      "source_kind"
    ],
    "missing_required": []
  },
  "card_patches": [],
  "gaps": [],
  "rejected_claims": []
}"#,
        );
        let audit_path = write_output(
            &root,
            "source-audit.json",
            r#"{
  "contract": "mdp.source-audit.v0",
  "refs": [
    {
      "ref": "raw_opportunity.customer",
      "source_id": "synthetic-rfp-summary",
      "locator": "synthetic-rfp-summary#customer",
      "snippet": "Example Public Services Agency issued a fictional public services modernization RFP."
    },
    {
      "ref": "raw_opportunity.summary",
      "source_id": "synthetic-rfp-summary",
      "locator": "synthetic-rfp-summary#summary",
      "snippet": "The fictional RFP asks for service request intake, status notifications, reporting, and training."
    },
    {
      "ref": "raw_opportunity.due_date",
      "source_id": "synthetic-rfp-summary",
      "locator": "synthetic-rfp-summary#due-date",
      "snippet": "The response is due in six weeks in the synthetic scenario."
    },
    {
      "ref": "raw_opportunity.review_mode",
      "source_id": "synthetic-rfp-summary",
      "locator": "synthetic-rfp-summary#review-mode",
      "snippet": "The supplied synthetic review mode is bid/no-bid."
    },
    {
      "ref": "source_kind",
      "source_id": "synthetic-rfp-summary",
      "locator": "synthetic-rfp-summary#source-kind",
      "snippet": "synthetic-example"
    }
  ]
}"#,
        );

        let result = validate_prompt_output_file_with_source_audit(
            &root,
            &output_path,
            None,
            Some("normalize-opportunity"),
            Some(&audit_path),
        )
        .expect("validation should return diagnostics");
        let codes: Vec<&str> = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter_map(|issue| issue["code"].as_str())
            .collect();

        assert_eq!(result["valid"], false);
        assert!(codes.contains(&"prompt_output_source_snippet_missing"));
        assert!(codes.contains(&"prompt_output_source_ref_missing"));

        let _ = std::fs::remove_dir_all(root);
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
    fn validate_accepts_structured_missing_required_trace() {
        let root = temp_pack("structured-missing-required");
        let path = write_output(
            &root,
            "normalize-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "normalize-prospect-row",
  "source_summary": {
    "company_domain": "northstarcloud.com",
    "company_name": "Northstar Cloud",
    "person_name": "N/A",
    "person_title": "N/A",
    "account_name": "Northstar Cloud",
    "inputs_used": ["raw_row", "company_domain", "existing_pack_context", "source_kind"],
    "confidence": "medium"
  },
  "normalized_prospect": {
    "name": "N/A",
    "title": "N/A",
    "company": "Northstar Cloud",
    "company_domain": "northstarcloud.com",
    "source_kind": "synthetic-example",
    "synthetic": true,
    "segment": "agent-assisted GTM",
    "trigger": "standardizing prospect qualification data",
    "signals": [
      {
        "id": "qualification-data-standardization",
        "title": "Standardizing prospect qualification data",
        "source": "raw_row.account_note"
      }
    ]
  },
  "normalization_trace": {
    "persona": {
      "source": "N/A",
      "matched_keywords": [],
      "confidence": "unknown",
      "needs_review": true
    },
    "fit_readiness": {
      "has_company_domain": true,
      "has_persona": false,
      "has_segment": true,
      "has_signal_source": true,
      "has_signals": true,
      "has_trigger": true,
      "ready_for_mdp_fit": false,
      "ready_for_brief": false,
      "no_draft_reason": "No person name or title was present in the source row; provide a reviewed contact before drafting."
    },
    "preserved_raw_fields": ["raw_row.company", "raw_row.account_note", "company_domain"],
    "missing_required": [
      {
        "field": "name",
        "path": "normalized_prospect.name",
        "reason": "not_available_in_source",
        "source_evidence": "Raw row contained account context but no named person."
      },
      {
        "field": "title",
        "path": "normalized_prospect.title",
        "reason": "not_available_in_source",
        "source_evidence": "Raw row contained account context but no person title."
      },
      {
        "field": "persona",
        "reason": "not_extractable_without_person",
        "source_evidence": "No reviewed person or role was supplied."
      }
    ]
  },
  "card_patches": [],
  "gaps": [
    "No person name or title was present in the source row; provide a reviewed contact before drafting."
  ],
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
    fn validate_rejects_missing_ready_for_mdp_fit_boolean() {
        let root = temp_pack("missing-ready-bool");
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
    "trigger": "testing readiness",
    "signals": [
      {
        "id": "readiness-test",
        "title": "Readiness test",
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
                .any(|issue| issue["code"] == "prompt_output_fit_readiness_ready_type")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_true_fit_readiness_when_required_attribute_missing() {
        let root = temp_pack("missing-required-readiness-attribute");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace(
                "  required_attributes: []",
                "  required_attributes:\n  - fiscal_year",
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
    "trigger": "testing readiness",
    "signals": [
      {
        "id": "readiness-test",
        "title": "Readiness test",
        "source": "raw_row.note"
      }
    ]
  },
  "normalization_trace": {
    "persona": {},
    "fit_readiness": {
      "ready_for_mdp_fit": true
    },
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
                .any(|issue| issue["code"]
                    == "prompt_output_fit_readiness_missing_required_attribute")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_malformed_structured_missing_required_trace() {
        let root = temp_pack("bad-structured-missing-required");
        let path = write_output(
            &root,
            "normalize-output.json",
            r#"{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "normalize-prospect-row",
  "source_summary": {
    "company_domain": "northstarcloud.com",
    "company_name": "Northstar Cloud",
    "person_name": "N/A",
    "person_title": "N/A",
    "account_name": "Northstar Cloud",
    "inputs_used": ["raw_row"],
    "confidence": "medium"
  },
  "normalized_prospect": {
    "name": "N/A",
    "title": "N/A",
    "company": "Northstar Cloud"
  },
  "normalization_trace": {
    "persona": {},
    "fit_readiness": {},
    "preserved_raw_fields": ["raw_row.company"],
    "missing_required": [
      {
        "field": "name",
        "source_evidence": "Raw row contained no named person."
      }
    ]
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
                .any(|issue| issue["code"] == "prompt_output_missing_required_field_missing")
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
    "fit_readiness": {
      "ready_for_mdp_fit": true
    },
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
    "fit_readiness": {
      "ready_for_mdp_fit": true
    },
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
    "fit_readiness": {
      "ready_for_mdp_fit": true
    },
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

    #[test]
    fn validate_accepts_matching_proposal_opportunity_alias() {
        let root = temp_pack_with_template("proposal-opportunity-alias", "proposal");
        let output = proposal_opportunity_alias_output();
        let path = write_json_output(&root, "normalize-opportunity-output.json", &output);

        let result = validate_prompt_output_file(&root, &path, None, Some("normalize-opportunity"))
            .expect("validation should return diagnostics");

        assert_eq!(result["valid"], true);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_mismatched_proposal_opportunity_alias() {
        let root = temp_pack_with_template("proposal-opportunity-alias-mismatch", "proposal");
        let mut output = proposal_opportunity_alias_output();
        output["normalized_opportunity"]["company"] = Value::String("Different Agency".into());
        let path = write_json_output(&root, "normalize-opportunity-output.json", &output);

        let result = validate_prompt_output_file(&root, &path, None, Some("normalize-opportunity"))
            .expect("validation should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "prompt_output_normalized_opportunity_mismatch")
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
