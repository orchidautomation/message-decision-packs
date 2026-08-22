use crate::artifact_hash::{pack_content_sha256, sha256_hex};
use crate::commands::prompt_output::validate_prompt_output_file_with_lineage_inputs;
use crate::commands::requirements::requirements;
use crate::commands::source_binding::validate_source_binding_file;
use crate::constants::{
    COLLECTED_ATTEMPT_RESULTS_CONTRACT_V2, NORMALIZED_DECISION_INPUT_CONTRACT_V2,
    REQUIREMENTS_CONTRACT_V2, SOURCE_ATTEMPT_REQUEST_CONTRACT_V2, SOURCE_BINDING_CONTRACT_V2,
};
use crate::pack_io::{read_manifest, resolve_pack_path};
use crate::value_contracts::{valid_date, valid_date_time};
use anyhow::{Context, Result, anyhow};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CHAIN_CONTRACT: &str = "mdp.synthetic-v2-chain.v1";
const CHAIN_FILES: [(&str, &str); 4] = [
    ("source-binding", "source-binding.json"),
    ("source-attempt-request", "source-attempt-request.json"),
    (
        "collected-attempt-results",
        "collected-attempt-results.json",
    ),
    ("normalized-input", "normalized-input.json"),
];

#[derive(Clone)]
struct ChainFile {
    name: &'static str,
    filename: &'static str,
    value: Value,
    bytes: Vec<u8>,
    sha256: String,
}

pub(crate) fn synthetic_v2_chain_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Synthetic v2 Chain Result",
        "type": "object",
        "additionalProperties": false,
        "required": ["contract", "valid", "status", "mode", "job_id", "pack", "files", "validation"],
        "properties": {
            "contract": {"const": CHAIN_CONTRACT},
            "valid": {"type": "boolean"},
            "status": {"enum": ["ready", "dry-run", "applied", "unchanged", "refused", "blocked"]},
            "mode": {"enum": ["fresh", "rebind"]},
            "job_id": {"type": ["string", "null"], "minLength": 1},
            "pack": {"type": ["object", "null"], "additionalProperties": false, "required": ["id", "version", "sha256"], "properties": {
                "id": {"type": "string"}, "version": {"type": "string"}, "sha256": {"type": "string", "pattern": "^[a-f0-9]{64}$"}
            }},
            "deterministic": {"type": "object"},
            "files": {"type": "array", "items": {"type": "object"}},
            "validation": {"type": "object"},
            "error": {"type": "object"}
        }
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn rebind_synthetic_chain(
    root: &Path,
    job_id: &str,
    out_dir: &Path,
    input_dir: Option<&Path>,
    as_of: &str,
    seed: u64,
    dry_run: bool,
    apply: bool,
    force: bool,
) -> Result<Value> {
    if dry_run && apply {
        return Ok(refusal(
            "invalid_argument",
            "--dry-run and --apply are mutually exclusive",
            "fresh",
        ));
    }
    if force && !apply {
        return Ok(refusal(
            "invalid_argument",
            "--force requires --apply",
            "fresh",
        ));
    }
    if !valid_timestamp(as_of) {
        return Ok(refusal(
            "synthetic_chain_as_of_invalid",
            "--as-of must be an exact UTC timestamp",
            "fresh",
        ));
    }

    let root = root
        .canonicalize()
        .with_context(|| format!("resolving pack root {}", root.display()))?;
    let input_dir = input_dir.map(Path::to_path_buf);
    let mode = if input_dir.is_some() {
        "rebind"
    } else {
        "fresh"
    };
    let input_dir_canonical = match input_dir.as_deref() {
        Some(path) => Some(
            path.canonicalize()
                .with_context(|| format!("resolving input directory {}", path.display()))?,
        ),
        None => None,
    };
    let output_check = check_output_directory(&root, out_dir, input_dir_canonical.as_deref());
    if let Err(error) = output_check {
        return Ok(refusal(
            "output-directory-inside-pack",
            &error.to_string(),
            mode,
        ));
    }

    let compiled = requirements(&root, job_id)?;
    if compiled["contract"] != REQUIREMENTS_CONTRACT_V2
        || compiled["available"] != true
        || compiled["decision_input_contracts"]
            .as_array()
            .is_none_or(|items| {
                items
                    .iter()
                    .all(|item| item["signal_projections"].is_null())
            })
    {
        return Ok(refusal(
            "synthetic_chain_v2_required",
            "the selected job must compile an available signal-aware mdp.requirements.v2 contract",
            mode,
        ));
    }
    let manifest = read_manifest(&root)?;
    let pack_sha256 = pack_content_sha256(&root)?;
    let pack = json!({"id": manifest.id, "version": manifest.version, "sha256": pack_sha256});
    let requirements_sha256 = compiled["requirements_sha256"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let chain = if let Some(input_dir) = input_dir_canonical.as_deref() {
        let inputs = read_input_chain(input_dir)?;
        if let Err(error) = validate_synthetic_input(&inputs, job_id, &compiled) {
            return Ok(refusal(error.code, &error.message, mode));
        }
        match rebind_chain(&inputs, &compiled, &pack, &requirements_sha256, job_id) {
            Ok(chain) => chain,
            Err(error) => {
                return Ok(refusal(
                    "synthetic_chain_rebind_failed",
                    &error.to_string(),
                    mode,
                ));
            }
        }
    } else {
        match build_fresh_chain(&compiled, &pack, &requirements_sha256, job_id, as_of, seed) {
            Ok(chain) => chain,
            Err(error) => {
                return Ok(refusal(
                    "synthetic_chain_recipe_unsupported",
                    &error.to_string(),
                    mode,
                ));
            }
        }
    };

    let validation = stage_and_validate(&root, job_id, &compiled, &chain)?;
    let validation_ok = validation["source_binding"]["valid"] == true
        && validation["prompt_output"]["valid"] == true;
    let mut result = base_result(
        &compiled, &pack, job_id, mode, as_of, seed, &chain, validation,
    );
    if !validation_ok {
        result["status"] = json!("blocked");
        result["valid"] = json!(false);
        return Ok(result);
    }

    let out_dir = out_dir.to_path_buf();
    let mut file_plans = plan_destination_files(&out_dir, &chain, apply, force)?;
    let has_blocked = file_plans.iter().any(|item| item["action"] == "blocked");
    if has_blocked {
        result["status"] = json!("blocked");
        result["valid"] = json!(false);
        result["error"] = json!({"code": "synthetic_chain_write_conflict", "message": "changed destination files require --apply --force"});
        result["files"] = Value::Array(std::mem::take(&mut file_plans));
        return Ok(result);
    }
    let all_unchanged = file_plans.iter().all(|item| item["action"] == "unchanged");
    result["files"] = Value::Array(file_plans.clone());
    if !apply || all_unchanged {
        result["status"] = json!(if all_unchanged {
            "unchanged"
        } else {
            "dry-run"
        });
        result["valid"] = json!(true);
        return Ok(result);
    }
    apply_destination_files(&out_dir, &chain, &file_plans, force)?;
    result["status"] = json!("applied");
    result["valid"] = json!(true);
    Ok(result)
}

fn base_result(
    compiled: &Value,
    pack: &Value,
    job_id: &str,
    mode: &str,
    as_of: &str,
    seed: u64,
    chain: &[ChainFile],
    validation: Value,
) -> Value {
    json!({
        "contract": CHAIN_CONTRACT,
        "valid": false,
        "status": "blocked",
        "mode": mode,
        "job_id": job_id,
        "pack": pack,
        "requirements": {"contract": compiled["contract"], "sha256": compiled["requirements_sha256"]},
        "deterministic": {"as_of": as_of, "seed": seed, "input_mode": mode},
        "files": chain.iter().map(|file| json!({"name": file.name, "filename": file.filename, "bytes": file.bytes.len(), "sha256": file.sha256})).collect::<Vec<_>>(),
        "validation": validation
    })
}

fn build_fresh_chain(
    compiled: &Value,
    pack: &Value,
    requirements_sha256: &str,
    job_id: &str,
    as_of: &str,
    seed: u64,
) -> Result<Vec<ChainFile>> {
    let contracts = compiled["decision_input_contracts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let contract_versions = contracts
        .iter()
        .map(|contract| json!({"id": contract["id"], "version": contract["version"]}))
        .collect::<Vec<_>>();
    let mut attributes = Vec::new();
    let mut values = BTreeMap::<String, Value>::new();
    let mut active = BTreeMap::<String, bool>::new();
    for contract in &contracts {
        let contract_id = contract["id"].as_str().unwrap_or_default();
        for attribute in contract["attributes"].as_array().into_iter().flatten() {
            let attribute_id = attribute["id"].as_str().unwrap_or_default();
            if !attribute["source_classes"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|item| item == "synthetic_fixture")
            {
                return Err(anyhow!(
                    "synthetic_chain_unsupported_source_class: attribute {attribute_id} does not allow synthetic_fixture"
                ));
            }
            let value = safe_value(attribute, job_id, seed);
            values.insert(format!("{contract_id}#{attribute_id}"), value);
        }
    }
    for contract in &contracts {
        let contract_id = contract["id"].as_str().unwrap_or_default();
        for attribute in contract["attributes"].as_array().into_iter().flatten() {
            let attribute_id = attribute["id"].as_str().unwrap_or_default();
            let qualified = format!("{contract_id}#{attribute_id}");
            let applies = condition_is_active(attribute, &values, &active, contract_id);
            active.insert(qualified.clone(), applies);
            let status = if applies {
                "observed"
            } else {
                "not_applicable"
            };
            attributes.push((
                contract_id.to_string(),
                attribute.clone(),
                status.to_string(),
                values[&qualified].clone(),
            ));
        }
    }

    let mut projection_bindings = Vec::new();
    for contract in &contracts {
        let contract_id = contract["id"].as_str().unwrap_or_default();
        for projection in contract["signal_projections"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let projection_id = projection["id"].as_str().unwrap_or_default();
            let contributors = projection["contributor_attribute_ids"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let source_ok = contributors.iter().all(|id| {
                contract["attributes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|attr| attr["id"] == *id)
                    .is_some_and(|_attr| {
                        active
                            .get(&format!(
                                "{contract_id}#{}",
                                id.as_str().unwrap_or_default()
                            ))
                            .copied()
                            .unwrap_or(false)
                    })
            });
            if !source_ok {
                return Err(anyhow!(
                    "synthetic_chain_projection_unavailable: projection {contract_id}#{projection_id} has no observed synthetic contributor"
                ));
            }
            let source_class = contributors
                .iter()
                .find_map(|id| {
                    contract["attributes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|attr| attr["id"] == *id)
                        .and_then(|attr| attr["source_classes"].as_array())
                        .and_then(|classes| {
                            classes.iter().find(|class| *class == "synthetic_fixture")
                        })
                        .cloned()
                })
                .unwrap_or_else(|| json!("synthetic_fixture"));
            projection_bindings.push(json!({
                "decision_input_contract_id": contract_id,
                "projection_id": projection_id,
                "qualified_projection_id": format!("{contract_id}#{projection_id}"),
                "contributor_attribute_ids": contributors,
                "source": {
                    "logical_source_id": format!("synthetic_{projection_id}"),
                    "source_class": source_class,
                    "acquisition_mode": "fixture",
                    "upstream_reference": format!("opaque:synthetic:{job_id}:{projection_id}:{seed}")
                }
            }));
        }
    }
    let binding = json!({
        "contract": SOURCE_BINDING_CONTRACT_V2,
        "binding_release": "synthetic-fixture-v2",
        "job_id": job_id,
        "pack": pack,
        "requirements": {"contract": REQUIREMENTS_CONTRACT_V2, "sha256": requirements_sha256, "decision_input_contracts": contract_versions},
        "normalization_release": "synthetic-normalizer-v2",
        "adapter": {"profile": "synthetic_fixture", "version": "2.0.0"},
        "transformation": {"id": "identity_v2"},
        "projection_bindings": projection_bindings
    });
    let binding_bytes = serialize(&binding)?;
    let binding_sha256 = sha256_hex(&binding_bytes);

    let mut attempts = Vec::new();
    let mut primary_attempts = BTreeMap::<String, String>::new();
    let mut next_attempt = 1usize;
    for (contract_id, attribute, status, value) in &attributes {
        let attribute_id = attribute["id"].as_str().unwrap_or_default();
        let attempt_id = format!("synthetic-{seed}-{next_attempt:03}");
        next_attempt += 1;
        primary_attempts.insert(format!("{contract_id}#{attribute_id}"), attempt_id.clone());
        attempts.push(json!({"attempt_id": attempt_id, "attribute_id": attribute_id, "source_class": "synthetic_fixture", "source_locator": format!("opaque:synthetic:{job_id}:{attribute_id}:{seed}"), "requested_at": as_of, "decision_input_contract_id": contract_id}));
        let _ = (status, value);
    }
    let mut projection_attempts = BTreeMap::<String, Vec<String>>::new();
    for contract in &contracts {
        let contract_id = contract["id"].as_str().unwrap_or_default();
        for projection in contract["signal_projections"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let qualified = format!(
                "{contract_id}#{}",
                projection["id"].as_str().unwrap_or_default()
            );
            let mut ids = Vec::new();
            let min = projection["cardinality"]["min"]
                .as_u64()
                .unwrap_or(1)
                .max(1) as usize;
            for contributor in projection["contributor_attribute_ids"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                for _ in 0..min {
                    let attempt_id = format!("synthetic-{seed}-{next_attempt:03}");
                    next_attempt += 1;
                    let locator =
                        format!("opaque:synthetic:{job_id}:{contributor}:{seed}:{attempt_id}");
                    attempts.push(json!({"attempt_id": attempt_id, "attribute_id": contributor, "source_class": "synthetic_fixture", "source_locator": locator, "requested_at": as_of, "decision_input_contract_id": contract_id}));
                    ids.push(attempt_id);
                }
            }
            projection_attempts.insert(qualified, ids);
        }
    }
    let request = json!({"contract": SOURCE_ATTEMPT_REQUEST_CONTRACT_V2, "job_id": job_id, "decision_input_contracts": contract_versions, "source_binding_sha256": binding_sha256, "as_of": as_of, "attempts": attempts});
    let request_bytes = serialize(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);

    let mut result_attributes = Map::new();
    let mut attempt_results = Vec::new();
    for (contract_id, attribute, status, value) in &attributes {
        let attribute_id = attribute["id"].as_str().unwrap_or_default();
        let attempt_id = primary_attempts[&format!("{contract_id}#{attribute_id}")].clone();
        let locator = format!("opaque:synthetic:{job_id}:{attribute_id}:{seed}");
        let result = attribute_result(attribute, status, value, &attempt_id, &locator, as_of)?;
        result_attributes.insert(attribute_id.to_string(), result.clone());
        attempt_results.push(attempt_result_item(
            contract_id,
            attribute_id,
            &result,
            &attempt_id,
            &locator,
            as_of,
        ));
    }
    for contract in &contracts {
        let contract_id = contract["id"].as_str().unwrap_or_default();
        for projection in contract["signal_projections"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let qualified = format!(
                "{contract_id}#{}",
                projection["id"].as_str().unwrap_or_default()
            );
            let contributor = projection["contributor_attribute_ids"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_str)
                .unwrap_or_default();
            let attr = contract["attributes"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|item| item["id"] == contributor)
                .ok_or_else(|| anyhow!("missing contributor {contributor}"))?;
            for attempt_id in projection_attempts.get(&qualified).into_iter().flatten() {
                let locator =
                    format!("opaque:synthetic:{job_id}:{contributor}:{seed}:{attempt_id}");
                let result = attribute_result(
                    attr,
                    "observed",
                    &values[&format!("{contract_id}#{contributor}")],
                    attempt_id,
                    &locator,
                    as_of,
                )?;
                attempt_results.push(attempt_result_item(
                    contract_id,
                    contributor,
                    &result,
                    attempt_id,
                    &locator,
                    as_of,
                ));
            }
        }
    }
    let results = json!({"contract": COLLECTED_ATTEMPT_RESULTS_CONTRACT_V2, "job_id": job_id, "decision_input_contracts": contract_versions, "source_attempt_request_sha256": request_sha256, "source_binding_sha256": binding_sha256, "attributes": result_attributes, "attempt_results": attempt_results});
    let results_bytes = serialize(&results)?;
    let results_sha256 = sha256_hex(&results_bytes);

    let mut prospect = json!({"name": "Avery Example", "title": "Example Operator", "company": "Example Expansion Account", "company_domain": "example-expansion.invalid", "source_kind": "synthetic-example", "synthetic": true, "attributes": {}});
    let mut normalized_attributes = Map::new();
    for (contract_id, attribute, status, _) in &attributes {
        let attribute_id = attribute["id"].as_str().unwrap_or_default();
        let result = result_attributes[attribute_id].clone();
        normalized_attributes.insert(attribute_id.to_string(), result.clone());
        if status == "observed" {
            set_path(
                &mut prospect,
                attribute["output_path"].as_str().unwrap_or_default(),
                result["value"].clone(),
            )?;
        }
        let _ = contract_id;
    }
    let mut observations = Vec::new();
    let mut observation_number = 1usize;
    for contract in &contracts {
        let contract_id = contract["id"].as_str().unwrap_or_default();
        for projection in contract["signal_projections"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let projection_id = projection["id"].as_str().unwrap_or_default();
            let qualified = format!("{contract_id}#{projection_id}");
            let contributor = projection["contributor_attribute_ids"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_str)
                .unwrap_or_default();
            let value = values[&format!("{contract_id}#{contributor}")].clone();
            for attempt_id in &projection_attempts[&qualified] {
                let locator =
                    format!("opaque:synthetic:{job_id}:{contributor}:{seed}:{attempt_id}");
                observations.push(json!({"contract": "mdp.signal-observation.v2", "id": format!("obs-{seed}-{observation_number}"), "contract_id": contract_id, "projection_id": projection_id, "qualified_projection_id": qualified, "kind": projection["kind"], "roles": projection["roles"], "value": value, "contributor_attribute_ids": projection["contributor_attribute_ids"], "attempt_ids": [attempt_id], "source_class": "synthetic_fixture", "source_locator": locator, "observed_at": as_of, "confidence": 100, "receipt": {"source_binding_sha256": binding_sha256, "source_attempt_request_sha256": request_sha256, "collected_results_sha256": results_sha256}}));
                observation_number += 1;
            }
        }
    }
    let normalized = json!({"contract": NORMALIZED_DECISION_INPUT_CONTRACT_V2, "job_id": job_id, "decision_input_contracts": contracts.iter().map(|contract| contract["id"].clone()).collect::<Vec<_>>(), "normalization": contracts.iter().map(|contract| json!({"contract_id": contract["id"], "prompt": contract["normalization"]["prompt"], "prompt_version": contract["normalization"]["prompt_version"]})).collect::<Vec<_>>(), "source_attempt_request_sha256": request_sha256, "collected_attempt_results_sha256": results_sha256, "source_binding_sha256": binding_sha256, "attributes": normalized_attributes, "normalized_prospect": prospect, "outcome": "ready", "draft_allowed": false, "signal_observations": observations});
    make_chain(
        binding,
        binding_bytes,
        request,
        request_bytes,
        results,
        results_bytes,
        normalized,
    )
}

fn rebind_chain(
    inputs: &[Value; 4],
    compiled: &Value,
    pack: &Value,
    requirements_sha256: &str,
    job_id: &str,
) -> Result<Vec<ChainFile>> {
    let mut binding = inputs[0].clone();
    binding["job_id"] = json!(job_id);
    binding["pack"] = pack.clone();
    binding["requirements"] = json!({"contract": REQUIREMENTS_CONTRACT_V2, "sha256": requirements_sha256, "decision_input_contracts": compiled["decision_input_contracts"].as_array().into_iter().flatten().map(|contract| json!({"id": contract["id"], "version": contract["version"]})).collect::<Vec<_>>()});
    let binding_bytes = serialize(&binding)?;
    let binding_sha256 = sha256_hex(&binding_bytes);
    let mut request = inputs[1].clone();
    request["job_id"] = json!(job_id);
    request["source_binding_sha256"] = json!(binding_sha256);
    let request_bytes = serialize(&request)?;
    let request_sha256 = sha256_hex(&request_bytes);
    let mut results = inputs[2].clone();
    results["job_id"] = json!(job_id);
    results["source_binding_sha256"] = json!(binding_sha256);
    results["source_attempt_request_sha256"] = json!(request_sha256);
    let results_bytes = serialize(&results)?;
    let results_sha256 = sha256_hex(&results_bytes);
    let mut normalized = inputs[3].clone();
    normalized["job_id"] = json!(job_id);
    normalized["source_binding_sha256"] = json!(binding_sha256);
    normalized["source_attempt_request_sha256"] = json!(request_sha256);
    normalized["collected_attempt_results_sha256"] = json!(results_sha256);
    if let Some(observations) = normalized["signal_observations"].as_array_mut() {
        for observation in observations {
            observation["receipt"]["source_binding_sha256"] = json!(binding_sha256);
            observation["receipt"]["source_attempt_request_sha256"] = json!(request_sha256);
            observation["receipt"]["collected_results_sha256"] = json!(results_sha256);
        }
    }
    make_chain(
        binding,
        binding_bytes,
        request,
        request_bytes,
        results,
        results_bytes,
        normalized,
    )
}

fn make_chain(
    binding: Value,
    binding_bytes: Vec<u8>,
    request: Value,
    request_bytes: Vec<u8>,
    results: Value,
    results_bytes: Vec<u8>,
    normalized: Value,
) -> Result<Vec<ChainFile>> {
    let normalized_bytes = serialize(&normalized)?;
    Ok(vec![
        chain_file(0, binding, binding_bytes),
        chain_file(1, request, request_bytes),
        chain_file(2, results, results_bytes),
        chain_file(3, normalized, normalized_bytes),
    ])
}

fn chain_file(index: usize, value: Value, bytes: Vec<u8>) -> ChainFile {
    ChainFile {
        name: CHAIN_FILES[index].0,
        filename: CHAIN_FILES[index].1,
        sha256: sha256_hex(&bytes),
        bytes,
        value,
    }
}

fn attribute_result(
    attribute: &Value,
    status: &str,
    value: &Value,
    attempt_id: &str,
    locator: &str,
    as_of: &str,
) -> Result<Value> {
    let mut result = json!({"status": status, "provenance": [{"attempt_id": attempt_id, "source_class": "synthetic_fixture", "source_locator": locator, "observed_at": as_of}], "confidence": 100, "freshness": {"observed_at": as_of, "age_days": 0}});
    if status == "observed" {
        result["value"] = value.clone();
    }
    if attribute["provenance"]["required_fields"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|field| field == "excerpt")
    {
        result["provenance"][0]["excerpt"] = json!(format!(
            "Synthetic fixture value for {}.",
            attribute["id"].as_str().unwrap_or("attribute")
        ));
    }
    Ok(result)
}

fn attempt_result_item(
    contract_id: &str,
    attribute_id: &str,
    result: &Value,
    attempt_id: &str,
    locator: &str,
    as_of: &str,
) -> Value {
    let mut item = result.clone();
    item["attempt_id"] = json!(attempt_id);
    item["decision_input_contract_id"] = json!(contract_id);
    item["attribute_id"] = json!(attribute_id);
    item["source_class"] = json!("synthetic_fixture");
    item["source_locator"] = json!(locator);
    item["observed_at"] = json!(as_of);
    item
}

fn safe_value(attribute: &Value, job_id: &str, seed: u64) -> Value {
    let contract = &attribute["value"];
    if let Some(first) = contract["enum"].as_array().and_then(|items| items.first()) {
        return first.clone();
    }
    match contract["type"].as_str().unwrap_or("string") {
        "boolean" => json!(true),
        "integer" => json!(1),
        "number" => json!(1),
        _ => match contract["format"].as_str() {
            Some("date") => json!("2026-01-01"),
            Some("date-time") => json!("2026-01-01T00:00:00Z"),
            _ => {
                let id = attribute["id"].as_str().unwrap_or("value");
                if id == "company_name" || attribute["output_path"] == "company" {
                    json!("Example Expansion Account")
                } else if id == "company_domain" || attribute["output_path"] == "company_domain" {
                    json!("example-expansion.invalid")
                } else if id == "person_name" || attribute["output_path"] == "name" {
                    json!("Avery Example")
                } else if id == "person_title" || attribute["output_path"] == "title" {
                    json!("VP Revenue Operations")
                } else if id == "expansion_trigger_summary" || attribute["output_path"] == "trigger"
                {
                    json!("Synthetic usage threshold reached in the reviewed fixture.")
                } else {
                    json!(format!("Synthetic example {job_id} {id} {seed}"))
                }
            }
        },
    }
}

fn condition_is_active(
    attribute: &Value,
    values: &BTreeMap<String, Value>,
    active: &BTreeMap<String, bool>,
    contract_id: &str,
) -> bool {
    attribute["applies_when"]
        .as_array()
        .into_iter()
        .flatten()
        .all(|condition| {
            let key = format!(
                "{contract_id}#{}",
                condition["attribute"].as_str().unwrap_or_default()
            );
            let exists = active.get(&key).copied().unwrap_or(false);
            match condition["operator"].as_str().unwrap_or("exists") {
                "exists" => exists,
                "equals" => {
                    exists
                        && values.get(&key).is_some_and(|current| {
                            condition["values"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .any(|value| value == current)
                        })
                }
                "not_equals" => {
                    !exists
                        || values.get(&key).is_none_or(|current| {
                            condition["values"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .all(|value| value != current)
                        })
                }
                "in" => {
                    exists
                        && values.get(&key).is_some_and(|current| {
                            condition["values"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .any(|value| value == current)
                        })
                }
                _ => false,
            }
        })
}

fn set_path(root: &mut Value, path: &str, value: Value) -> Result<()> {
    let mut current = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("normalized prospect must be an object"))?;
    let segments = path
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return Err(anyhow!("decision input output_path is empty"));
    }
    for segment in &segments[..segments.len() - 1] {
        let entry = current
            .entry((*segment).to_string())
            .or_insert_with(|| json!({}));
        current = entry
            .as_object_mut()
            .ok_or_else(|| anyhow!("output_path {path} is not object-shaped"))?;
    }
    current.insert(segments[segments.len() - 1].to_string(), value);
    Ok(())
}

fn stage_and_validate(
    root: &Path,
    job_id: &str,
    compiled: &Value,
    chain: &[ChainFile],
) -> Result<Value> {
    let stage = std::env::temp_dir().join(format!(
        "mdp-synthetic-chain-{}-{}",
        std::process::id(),
        nonce()
    ));
    fs::create_dir_all(&stage)?;
    let paths = CHAIN_FILES.map(|(_, filename)| stage.join(filename));
    for (file, path) in chain.iter().zip(paths.iter()) {
        fs::write(path, &file.bytes)?;
    }
    let binding_validation = validate_source_binding_file(root, job_id, &paths[0])?;
    let prompt_path = compiled["decision_input_contracts"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|contract| contract["normalization"]["prompt"].as_str())
        .map(|path| resolve_pack_path(root, path))
        .transpose()?;
    let prompt_validation = validate_prompt_output_file_with_lineage_inputs(
        root,
        &paths[3],
        prompt_path.as_deref(),
        None,
        None,
        Some(&paths[0]),
        Some(&paths[1]),
        Some(&paths[2]),
        None,
        None,
    )?;
    let schema_diagnostics = json!({
        "request": jsonschema::draft202012::validate(&compiled["source_attempt_request_schema"], &chain[1].value).err().map(|error| error.to_string()),
        "results": jsonschema::draft202012::validate(&compiled["collected_attempt_results_schema"], &chain[2].value).err().map(|error| error.to_string()),
        "normalized": jsonschema::draft202012::validate(&compiled["normalized_output_schema"], &chain[3].value).err().map(|error| error.to_string())
    });
    let _ = fs::remove_dir_all(&stage);
    Ok(
        json!({"source_binding": binding_validation, "prompt_output": prompt_validation, "compiled_contract": compiled["contract"], "schema_diagnostics": schema_diagnostics}),
    )
}

fn plan_destination_files(
    out_dir: &Path,
    chain: &[ChainFile],
    apply: bool,
    force: bool,
) -> Result<Vec<Value>> {
    let mut plans = Vec::new();
    for file in chain {
        let path = out_dir.join(file.filename);
        let existing = fs::read(&path).ok();
        let (action, backup) = match existing {
            None => ("create", Value::Null),
            Some(bytes) if bytes == file.bytes => ("unchanged", Value::Null),
            Some(bytes) if apply && force => (
                "overwrite-with-backup",
                json!(backup_path(&path, &sha256_hex(&bytes))?),
            ),
            Some(_) => ("blocked", Value::Null),
        };
        plans.push(json!({"name": file.name, "filename": file.filename, "path": path.display().to_string(), "bytes": file.bytes.len(), "sha256": file.sha256, "action": action, "backup": backup, "would_write": action != "unchanged" && action != "blocked"}));
    }
    Ok(plans)
}

fn apply_destination_files(
    out_dir: &Path,
    chain: &[ChainFile],
    plans: &[Value],
    force: bool,
) -> Result<()> {
    fs::create_dir_all(out_dir)?;
    let mut changed = Vec::<(PathBuf, Option<PathBuf>)>::new();
    for plan in plans {
        if plan["action"] == "unchanged" {
            continue;
        }
        let filename = plan["filename"].as_str().unwrap_or_default();
        let target = out_dir.join(filename);
        let candidate = chain
            .iter()
            .find(|file| file.filename == filename)
            .ok_or_else(|| anyhow!("missing staged file {filename}"))?;
        let backup = if target.exists() {
            if !force {
                return Err(anyhow!(
                    "synthetic_chain_write_conflict: {} requires --force",
                    target.display()
                ));
            }
            let path = plan["backup"]
                .as_str()
                .map(PathBuf::from)
                .ok_or_else(|| anyhow!("missing backup plan"))?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&target, &path)?;
            Some(path)
        } else {
            None
        };
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp = target.with_extension(format!("json.tmp-{}", nonce()));
        let mut file = fs::File::create(&temp)?;
        file.write_all(&candidate.bytes)?;
        file.sync_all().ok();
        if let Err(error) = fs::rename(&temp, &target) {
            let _ = fs::remove_file(&temp);
            rollback_files(&changed);
            return Err(error.into());
        }
        changed.push((target, backup));
    }
    Ok(())
}

fn rollback_files(changed: &[(PathBuf, Option<PathBuf>)]) {
    for (target, backup) in changed.iter().rev() {
        if let Some(backup) = backup {
            let _ = fs::rename(backup, target);
        } else {
            let _ = fs::remove_file(target);
        }
    }
}

fn backup_path(target: &Path, digest: &str) -> Result<PathBuf> {
    let parent = target
        .parent()
        .ok_or_else(|| anyhow!("destination has no parent"))?
        .join(".mdp-synthetic-backups");
    let filename = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact.json");
    let base = parent.join(format!("{filename}.{digest}.bak"));
    if !base.exists() {
        return Ok(base);
    }
    for index in 1..1000 {
        let candidate = parent.join(format!("{filename}.{digest}.{index}.bak"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(anyhow!(
        "synthetic_chain_backup_collision: no recoverable backup path available for {}",
        target.display()
    ))
}

fn read_input_chain(input_dir: &Path) -> Result<[Value; 4]> {
    let mut values = Vec::new();
    for (_, filename) in CHAIN_FILES {
        let path = input_dir.join(filename);
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("reading synthetic chain input {}", path.display()))?;
        values.push(
            serde_json::from_str(&raw)
                .with_context(|| format!("parsing synthetic chain input {}", path.display()))?,
        );
    }
    values
        .try_into()
        .map_err(|_| anyhow!("expected four synthetic chain files"))
}

struct Refusal {
    code: &'static str,
    message: String,
}

fn validate_synthetic_input(
    inputs: &[Value; 4],
    job_id: &str,
    compiled: &Value,
) -> std::result::Result<(), Refusal> {
    let expected = [
        SOURCE_BINDING_CONTRACT_V2,
        SOURCE_ATTEMPT_REQUEST_CONTRACT_V2,
        COLLECTED_ATTEMPT_RESULTS_CONTRACT_V2,
        NORMALIZED_DECISION_INPUT_CONTRACT_V2,
    ];
    let mut saw_source_class = false;
    for (index, input) in inputs.iter().enumerate() {
        if input["contract"] != expected[index] {
            return Err(Refusal {
                code: "synthetic_chain_mixed_version",
                message: format!("{} must be {}", CHAIN_FILES[index].1, expected[index]),
            });
        }
        if input["job_id"] != job_id {
            return Err(Refusal {
                code: "synthetic_chain_job_mismatch",
                message: format!("{} is not bound to job {job_id}", CHAIN_FILES[index].1),
            });
        }
        scan_provenance(input, &mut saw_source_class)?;
    }
    if !saw_source_class {
        return Err(Refusal {
            code: "synthetic_chain_provenance_missing",
            message: "synthetic chain has no explicit synthetic source provenance".to_string(),
        });
    }
    if inputs[3]["normalized_prospect"]["synthetic"] != true
        || inputs[3]["normalized_prospect"]["source_kind"] != "synthetic-example"
    {
        return Err(Refusal {
            code: "synthetic_chain_provenance_not_synthetic",
            message: "normalized input must explicitly mark synthetic-example provenance"
                .to_string(),
        });
    }
    if compiled["contract"] != REQUIREMENTS_CONTRACT_V2 {
        return Err(Refusal {
            code: "synthetic_chain_v2_required",
            message: "rebind requires an available v2 job".to_string(),
        });
    }
    Ok(())
}

fn scan_provenance(value: &Value, saw_source_class: &mut bool) -> std::result::Result<(), Refusal> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if key == "source_class" {
                    *saw_source_class = true;
                    if child != "synthetic_fixture" {
                        return Err(Refusal {
                            code: "synthetic_chain_real_provenance",
                            message: "rebind refuses non-synthetic source_class provenance"
                                .to_string(),
                        });
                    }
                }
                if matches!(key.as_str(), "source_locator" | "upstream_reference") {
                    let Some(reference) = child.as_str() else {
                        return Err(Refusal {
                            code: "synthetic_chain_provenance_ambiguous",
                            message: "provenance locator must be an opaque string".to_string(),
                        });
                    };
                    if reference.contains("://")
                        || reference.starts_with('/')
                        || reference.starts_with('~')
                        || reference.chars().any(char::is_control)
                    {
                        return Err(Refusal { code: "synthetic_chain_unsafe_locator", message: "rebind refuses URL, absolute, home-relative, or control-character locators".to_string() });
                    }
                }
                if key == "synthetic" && child != true {
                    return Err(Refusal {
                        code: "synthetic_chain_provenance_not_synthetic",
                        message: "rebind refuses an explicit synthetic=false marker".to_string(),
                    });
                }
                if matches!(
                    key.as_str(),
                    "provider"
                        | "provider_name"
                        | "credential"
                        | "credential_id"
                        | "token"
                        | "api_key"
                        | "customer_id"
                        | "private_path"
                        | "raw_source"
                ) {
                    return Err(Refusal {
                        code: "synthetic_chain_private_provenance",
                        message: format!(
                            "rebind refuses private or provider provenance field {key}"
                        ),
                    });
                }
                scan_provenance(child, saw_source_class)?;
            }
        }
        Value::Array(items) => {
            for child in items {
                scan_provenance(child, saw_source_class)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_output_directory(root: &Path, out_dir: &Path, input_dir: Option<&Path>) -> Result<()> {
    let root = root.canonicalize()?;
    let candidate = if out_dir.exists() {
        out_dir.canonicalize()?
    } else {
        out_dir.to_path_buf()
    };
    if candidate == root
        || candidate.starts_with(&root)
        || input_dir.is_some_and(|input| candidate == input || candidate.starts_with(input))
    {
        return Err(anyhow!(
            "output directory must be external to the active pack and input chain"
        ));
    }
    Ok(())
}

fn serialize(value: &Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn refusal(code: &'static str, message: &str, mode: &str) -> Value {
    json!({"contract": CHAIN_CONTRACT, "valid": false, "status": "refused", "mode": mode, "job_id": Value::Null, "pack": Value::Null, "files": [], "validation": {}, "error": {"code": code, "message": message}})
}

fn valid_timestamp(value: &str) -> bool {
    value.is_ascii()
        && value.len() == 20
        && value.ends_with('Z')
        && valid_date(&value[..10])
        && valid_date_time(value)
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clay_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("cli has a repository parent")
            .join("examples/clay-audiences-self-serve-enterprise-expansion")
    }

    #[test]
    fn fresh_chain_is_deterministic_and_schema_valid() {
        let root = clay_root();
        let compiled = requirements(&root, "prospect-fit-or-brief").expect("requirements compile");
        let manifest = read_manifest(&root).expect("manifest reads");
        let pack = json!({"id": manifest.id, "version": manifest.version, "sha256": pack_content_sha256(&root).expect("pack hashes")});
        let left = build_fresh_chain(
            &compiled,
            &pack,
            compiled["requirements_sha256"].as_str().unwrap(),
            "prospect-fit-or-brief",
            "2026-01-01T00:00:00Z",
            0,
        )
        .expect("chain builds");
        let right = build_fresh_chain(
            &compiled,
            &pack,
            compiled["requirements_sha256"].as_str().unwrap(),
            "prospect-fit-or-brief",
            "2026-01-01T00:00:00Z",
            0,
        )
        .expect("chain rebuilds");
        assert_eq!(
            left.iter().map(|file| &file.bytes).collect::<Vec<_>>(),
            right.iter().map(|file| &file.bytes).collect::<Vec<_>>()
        );
        assert!(
            jsonschema::draft202012::validate(&compiled["source_binding_schema"], &left[0].value)
                .is_ok()
        );
        assert!(
            jsonschema::draft202012::validate(
                &compiled["source_attempt_request_schema"],
                &left[1].value
            )
            .is_ok()
        );
        assert!(
            jsonschema::draft202012::validate(
                &compiled["collected_attempt_results_schema"],
                &left[2].value
            )
            .is_ok()
        );
        assert!(
            jsonschema::draft202012::validate(
                &compiled["normalized_output_schema"],
                &left[3].value
            )
            .is_ok()
        );
    }

    #[test]
    fn rebind_rejects_non_synthetic_source_class() {
        let root = clay_root();
        let compiled = requirements(&root, "prospect-fit-or-brief").expect("requirements compile");
        let manifest = read_manifest(&root).expect("manifest reads");
        let pack = json!({"id": manifest.id, "version": manifest.version, "sha256": pack_content_sha256(&root).expect("pack hashes")});
        let chain = build_fresh_chain(
            &compiled,
            &pack,
            compiled["requirements_sha256"].as_str().unwrap(),
            "prospect-fit-or-brief",
            "2026-01-01T00:00:00Z",
            0,
        )
        .expect("chain builds");
        let mut inputs: [Value; 4] = chain
            .iter()
            .map(|file| file.value.clone())
            .collect::<Vec<_>>()
            .try_into()
            .expect("four files");
        inputs[0]["projection_bindings"][0]["source"]["source_class"] = json!("public_web");
        let error = validate_synthetic_input(&inputs, "prospect-fit-or-brief", &compiled)
            .expect_err("real provenance must refuse");
        assert_eq!(error.code, "synthetic_chain_real_provenance");
    }
}
