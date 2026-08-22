use crate::artifact_hash::sha256_hex;
use crate::commands::prompt_output::validate_prompt_output_file_with_lineage_inputs;
use crate::commands::requirements::requirements;
use crate::commands::source_binding::validate_source_binding_file;
use crate::constants::{
    COLLECTED_ATTEMPT_RESULTS_CONTRACT_V2, DEFAULT_DIR, NORMALIZED_DECISION_INPUT_CONTRACT_V2,
    REQUIREMENTS_CONTRACT_V2, SOURCE_ATTEMPT_REQUEST_CONTRACT_V2, SOURCE_BINDING_CONTRACT_V2,
    SYNTHETIC_V2_CHAIN_CONTRACT,
};
use anyhow::{Context, Result, anyhow};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SOURCE_BINDING_FILE: &str = "source-binding.json";
const SOURCE_ATTEMPT_REQUEST_FILE: &str = "source-attempt-request.json";
const COLLECTED_ATTEMPT_RESULTS_FILE: &str = "collected-attempt-results.json";
const NORMALIZED_INPUT_FILE: &str = "normalized-input.json";

#[derive(Clone)]
struct CandidateFile {
    logical_name: &'static str,
    path: PathBuf,
    bytes: Vec<u8>,
}

#[derive(Clone)]
struct SourceRecord {
    contract_id: String,
    attribute_id: String,
    attempt_id: String,
    source_locator: String,
    observed_at: String,
    status: String,
    value: Option<Value>,
    state: Value,
}

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
    if !valid_utc_timestamp(as_of) {
        return Ok(refusal(
            "synthetic_chain_as_of_invalid",
            "--as-of must be a UTC timestamp in YYYY-MM-DDTHH:MM:SSZ form",
        ));
    }
    if force && !apply {
        return Ok(refusal(
            "synthetic_chain_force_requires_apply",
            "--force is valid only with --apply",
        ));
    }

    let root = root
        .canonicalize()
        .with_context(|| format!("canonicalizing pack root {}", root.display()))?;
    let output_dir = absolute_path(out_dir)?;
    let input_dir = input_dir.map(absolute_path).transpose()?;
    if let Some(reason) = invalid_output_location(&root, &output_dir, input_dir.as_deref()) {
        return Ok(refusal("synthetic_chain_output_location_invalid", &reason));
    }

    let compiled = requirements(&root, job_id)?;
    if compiled["available"] != true {
        return Ok(refusal(
            "synthetic_chain_requirements_unavailable",
            "the selected job does not compile an available Decision Input Contract",
        ));
    }
    if compiled["contract"] != REQUIREMENTS_CONTRACT_V2
        || !compiled["decision_input_contracts"]
            .as_array()
            .is_some_and(|contracts| {
                contracts.iter().any(|contract| {
                    contract["signal_projections"]
                        .as_array()
                        .is_some_and(|items| !items.is_empty())
                })
            })
    {
        return Ok(refusal(
            "synthetic_chain_v2_required",
            "rebind-synthetic-chain requires an available signal-aware mdp.requirements.v2 job",
        ));
    }
    let (binding, request, results, normalized, effective_as_of, mode) =
        if let Some(input_dir) = input_dir.as_deref() {
            let (input_binding, input_request, input_results, input_normalized) =
                read_input_chain(input_dir)?;
            if let Some(reason) = reject_non_synthetic_chain(
                &input_binding,
                &input_request,
                &input_results,
                &input_normalized,
            ) {
                return Ok(refusal("synthetic_chain_non_synthetic_provenance", &reason));
            }
            if let Some(reason) = reject_input_identity(
                job_id,
                &compiled,
                &input_binding,
                &input_request,
                &input_results,
                &input_normalized,
            ) {
                return Ok(refusal("synthetic_chain_input_identity_invalid", &reason));
            }
            let binding = build_binding(&compiled, job_id, seed)?;
            let effective_as_of = input_request["as_of"]
                .as_str()
                .filter(|value| valid_utc_timestamp(value))
                .unwrap_or(as_of)
                .to_string();
            let request = rebind_request(input_request, &binding, &compiled, job_id)?;
            let results = rebind_results(input_results, &binding, &request, &compiled, job_id)?;
            let normalized = rebind_normalized(
                input_normalized,
                &binding,
                &request,
                &results,
                &compiled,
                job_id,
            )?;
            (
                binding,
                request,
                results,
                normalized,
                effective_as_of,
                "rebind",
            )
        } else {
            let (binding, request, results, normalized) =
                build_fresh_chain(&compiled, job_id, as_of, seed)?;
            (
                binding,
                request,
                results,
                normalized,
                as_of.to_string(),
                "fresh",
            )
        };

    let candidates = serialize_candidates(&output_dir, &binding, &request, &results, &normalized)?;
    let stage_dir = staging_dir()?;
    let staged = stage_candidates(&stage_dir, &candidates)?;
    let validation = validate_staged_chain(&root, job_id, &stage_dir, &staged)?;
    if validation["source_binding"]["valid"] != true || validation["prompt_output"]["valid"] != true
    {
        let mut result = base_result(
            &compiled,
            job_id,
            &effective_as_of,
            seed,
            mode,
            dry_run || !apply,
        );
        result["valid"] = json!(false);
        result["status"] = json!("blocked");
        result["refusal"] = json!({
            "code": "synthetic_chain_validation_failed",
            "message": "staged synthetic chain failed the existing source-binding or prompt-output validator"
        });
        result["validation"] = validation;
        result["files"] = json!(file_metadata(&candidates));
        cleanup_stage(&stage_dir);
        return Ok(result);
    }

    let write_plan = plan_writes(&candidates, apply, force)?;
    let blocked = write_plan.iter().any(|item| item["action"] == "blocked");
    let mut result = base_result(
        &compiled,
        job_id,
        &effective_as_of,
        seed,
        mode,
        dry_run || !apply,
    );
    result["validation"] = validation;
    result["write_plan"] = json!(write_plan);
    result["files"] = json!(file_metadata(&candidates));
    if blocked {
        result["valid"] = json!(false);
        result["status"] = json!("blocked");
        result["refusal"] = json!({
            "code": "synthetic_chain_write_conflict",
            "message": "changed destination files require --apply --force; no files were written"
        });
        cleanup_stage(&stage_dir);
        return Ok(result);
    }

    if apply {
        if let Err(error) = apply_writes(&candidates, &write_plan) {
            result["valid"] = json!(false);
            result["status"] = json!("blocked");
            result["refusal"] = json!({
                "code": "synthetic_chain_write_failed",
                "message": error.to_string()
            });
            cleanup_stage(&stage_dir);
            return Ok(result);
        }
        result["status"] = json!("applied");
        result["applied"] = json!(true);
    } else {
        result["status"] = json!("dry-run");
        result["applied"] = json!(false);
    }
    cleanup_stage(&stage_dir);
    Ok(result)
}

fn build_fresh_chain(
    compiled: &Value,
    job_id: &str,
    as_of: &str,
    seed: u64,
) -> Result<(Value, Value, Value, Value)> {
    let binding = build_binding(compiled, job_id, seed)?;
    let contracts = compiled["decision_input_contracts"]
        .as_array()
        .ok_or_else(|| anyhow!("compiled requirements omitted decision_input_contracts"))?;
    let mut records = Vec::new();
    let mut attributes = Map::new();
    let mut normalized_prospect = json!({
        "name": "Avery Example",
        "title": "Synthetic Example Contact",
        "company": "Example Expansion Account",
        "company_domain": "example.invalid",
        "source_kind": "synthetic-example",
        "synthetic": true,
        "attributes": {}
    });
    let mut ordinal = 1usize;
    for contract in contracts {
        let contract_id = contract["id"].as_str().unwrap_or_default();
        for attribute in contract["attributes"].as_array().into_iter().flatten() {
            let attribute_id = attribute["id"].as_str().unwrap_or_default();
            if !attribute["source_classes"]
                .as_array()
                .is_some_and(|classes| classes.iter().any(|class| class == "synthetic_fixture"))
            {
                return Err(anyhow!(
                    "synthetic_chain_attribute_source_unsupported: {contract_id}#{attribute_id}"
                ));
            }
            let value = deterministic_value(attribute, as_of, seed, ordinal)?;
            let applies = applies_when(attribute, &attributes);
            let status = if applies {
                "observed"
            } else {
                "not_applicable"
            };
            let attempt_id = format!("synthetic-attempt-{ordinal:03}");
            let locator = format!("opaque:synthetic:{contract_id}:{attribute_id}:{seed}");
            let state = attempt_state(
                attribute,
                status,
                value.clone(),
                &attempt_id,
                &locator,
                as_of,
                ordinal,
            );
            if status == "observed" {
                set_output_path(
                    &mut normalized_prospect,
                    attribute["output_path"].as_str(),
                    &value,
                );
            }
            attributes.insert(attribute_id.to_string(), state.clone());
            records.push(SourceRecord {
                contract_id: contract_id.to_string(),
                attribute_id: attribute_id.to_string(),
                attempt_id,
                source_locator: locator,
                observed_at: as_of.to_string(),
                status: status.to_string(),
                value: (status == "observed").then_some(value),
                state,
            });
            ordinal += 1;
        }
    }
    let request = build_request(compiled, job_id, &binding, as_of, &records);
    let request_bytes = json_bytes(&request)?;
    let request_sha = sha256_hex(&request_bytes);
    let results = build_results(compiled, job_id, &binding, &request_sha, &records);
    let results_bytes = json_bytes(&results)?;
    let results_sha = sha256_hex(&results_bytes);
    let normalized = build_normalized(
        compiled,
        job_id,
        &binding,
        &sha256_hex(&json_bytes(&binding)?),
        &request_sha,
        &results_sha,
        &attributes,
        normalized_prospect,
        &records,
        as_of,
    )?;
    Ok((binding, request, results, normalized))
}

fn build_binding(compiled: &Value, job_id: &str, seed: u64) -> Result<Value> {
    let mut projection_bindings = Vec::new();
    for contract in compiled["decision_input_contracts"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let contract_id = contract["id"].as_str().unwrap_or_default();
        for projection in contract["signal_projections"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let projection_id = projection["id"].as_str().unwrap_or_default();
            let contributors = projection["contributor_attribute_ids"].clone();
            let contributor_supported = contributors.as_array().is_some_and(|ids| {
                ids.iter().all(|id| {
                    contract["attributes"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|attribute| {
                            attribute["id"] == *id
                                && attribute["source_classes"]
                                    .as_array()
                                    .is_some_and(|classes| {
                                        classes.iter().any(|class| class == "synthetic_fixture")
                                    })
                        })
                })
            });
            if !contributor_supported {
                return Err(anyhow!(
                    "synthetic_chain_projection_source_unsupported: {contract_id}#{projection_id}"
                ));
            }
            projection_bindings.push(json!({
                "decision_input_contract_id": contract_id,
                "projection_id": projection_id,
                "qualified_projection_id": format!("{contract_id}#{projection_id}"),
                "contributor_attribute_ids": contributors,
                "source": {
                    "logical_source_id": format!("synthetic:{projection_id}"),
                    "source_class": "synthetic_fixture",
                    "acquisition_mode": "fixture",
                    "upstream_reference": format!("opaque:synthetic:{contract_id}:{projection_id}:{seed}")
                }
            }));
        }
    }
    Ok(json!({
        "contract": SOURCE_BINDING_CONTRACT_V2,
        "binding_release": "synthetic-mdp-adapter-v2",
        "job_id": job_id,
        "pack": {
            "id": compiled["pack"]["id"],
            "version": compiled["pack"]["version"],
            "sha256": compiled["pack"]["sha256"]
        },
        "requirements": {
            "contract": REQUIREMENTS_CONTRACT_V2,
            "sha256": compiled["requirements_sha256"],
            "decision_input_contracts": contract_versions(compiled)
        },
        "normalization_release": "synthetic-mdp-normalizer-v2",
        "adapter": {"profile": "synthetic_mdp_adapter", "version": "2.0.0"},
        "transformation": {"id": "identity_v2"},
        "projection_bindings": projection_bindings
    }))
}

fn build_request(
    compiled: &Value,
    job_id: &str,
    binding: &Value,
    as_of: &str,
    records: &[SourceRecord],
) -> Value {
    json!({
        "contract": SOURCE_ATTEMPT_REQUEST_CONTRACT_V2,
        "job_id": job_id,
        "decision_input_contracts": contract_versions(compiled),
        "source_binding_sha256": sha256_hex(&json_bytes(binding).expect("binding should serialize")),
        "as_of": as_of,
        "attempts": records.iter().map(|record| json!({
            "attempt_id": record.attempt_id,
            "attribute_id": record.attribute_id,
            "source_class": "synthetic_fixture",
            "source_locator": record.source_locator,
            "requested_at": as_of,
            "decision_input_contract_id": record.contract_id
        })).collect::<Vec<_>>()
    })
}

fn build_results(
    compiled: &Value,
    job_id: &str,
    binding: &Value,
    request_sha: &str,
    records: &[SourceRecord],
) -> Value {
    let binding_sha = sha256_hex(&json_bytes(binding).expect("binding should serialize"));
    json!({
        "contract": COLLECTED_ATTEMPT_RESULTS_CONTRACT_V2,
        "job_id": job_id,
        "decision_input_contracts": contract_versions(compiled),
        "source_attempt_request_sha256": request_sha,
        "source_binding_sha256": binding_sha,
        "attributes": records.iter().map(|record| (record.attribute_id.clone(), record.state.clone())).collect::<Map<_, _>>(),
        "attempt_results": records.iter().map(|record| {
            let mut result = record.state.clone();
            result["attempt_id"] = json!(record.attempt_id);
            result["decision_input_contract_id"] = json!(record.contract_id);
            result["attribute_id"] = json!(record.attribute_id);
            result["source_class"] = json!("synthetic_fixture");
            result["source_locator"] = json!(record.source_locator);
            result["observed_at"] = json!(record.observed_at);
            result
        }).collect::<Vec<_>>()
    })
}

fn build_normalized(
    compiled: &Value,
    job_id: &str,
    binding: &Value,
    binding_sha: &str,
    request_sha: &str,
    results_sha: &str,
    attributes: &Map<String, Value>,
    normalized_prospect: Value,
    records: &[SourceRecord],
    as_of: &str,
) -> Result<Value> {
    let mut observations = Vec::new();
    for (index, contract) in compiled["decision_input_contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
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
            let contributor = contributors
                .iter()
                .find_map(|id| {
                    let id = id.as_str()?;
                    records.iter().find(|record| {
                        record.contract_id == contract_id
                            && record.attribute_id == id
                            && record.status == "observed"
                    })
                })
                .ok_or_else(|| {
                    anyhow!(
                        "synthetic_chain_projection_value_missing: {contract_id}#{projection_id}"
                    )
                })?;
            let value = contributor.value.clone().ok_or_else(|| {
                anyhow!("synthetic_chain_projection_value_missing: {contract_id}#{projection_id}")
            })?;
            if jsonschema::draft202012::validate(&projection["value"], &value).is_err() {
                return Err(anyhow!(
                    "synthetic_chain_projection_value_unsupported: {contract_id}#{projection_id}"
                ));
            }
            observations.push(json!({
                "contract": "mdp.signal-observation.v2",
                "id": format!("synthetic-observation-{:03}", index + observations.len() + 1),
                "contract_id": contract_id,
                "projection_id": projection_id,
                "qualified_projection_id": format!("{contract_id}#{projection_id}"),
                "kind": projection["kind"],
                "roles": projection["roles"],
                "value": value,
                "contributor_attribute_ids": contributors,
                "attempt_ids": [contributor.attempt_id],
                "source_class": "synthetic_fixture",
                "source_locator": contributor.source_locator,
                "observed_at": as_of,
                "confidence": 100,
                "receipt": {
                    "source_binding_sha256": binding_sha,
                    "source_attempt_request_sha256": request_sha,
                    "collected_results_sha256": results_sha
                }
            }));
        }
    }
    let normalization = compiled["decision_input_contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|contract| {
            json!({
                "contract_id": contract["id"],
                "prompt": contract["normalization"]["prompt"],
                "prompt_version": contract["normalization"]["prompt_version"]
            })
        })
        .collect::<Vec<_>>();
    let _ = binding;
    Ok(json!({
        "contract": NORMALIZED_DECISION_INPUT_CONTRACT_V2,
        "job_id": job_id,
        "decision_input_contracts": compiled["decision_input_contracts"].as_array().into_iter().flatten().filter_map(|contract| contract["id"].as_str()).collect::<Vec<_>>(),
        "normalization": normalization,
        "source_binding_sha256": binding_sha,
        "source_attempt_request_sha256": request_sha,
        "collected_attempt_results_sha256": results_sha,
        "attributes": attributes,
        "normalized_prospect": normalized_prospect,
        "outcome": "ready",
        "draft_allowed": false,
        "signal_observations": observations
    }))
}

fn rebind_request(
    mut request: Value,
    binding: &Value,
    compiled: &Value,
    job_id: &str,
) -> Result<Value> {
    request["contract"] = json!(SOURCE_ATTEMPT_REQUEST_CONTRACT_V2);
    request["job_id"] = json!(job_id);
    request["decision_input_contracts"] = json!(contract_versions(compiled));
    request["source_binding_sha256"] = json!(sha256_hex(&json_bytes(binding)?));
    Ok(request)
}

fn rebind_results(
    mut results: Value,
    binding: &Value,
    request: &Value,
    compiled: &Value,
    job_id: &str,
) -> Result<Value> {
    results["contract"] = json!(COLLECTED_ATTEMPT_RESULTS_CONTRACT_V2);
    results["job_id"] = json!(job_id);
    results["decision_input_contracts"] = json!(contract_versions(compiled));
    results["source_attempt_request_sha256"] = json!(sha256_hex(&json_bytes(request)?));
    results["source_binding_sha256"] = json!(sha256_hex(&json_bytes(binding)?));
    Ok(results)
}

fn rebind_normalized(
    mut normalized: Value,
    binding: &Value,
    request: &Value,
    results: &Value,
    compiled: &Value,
    job_id: &str,
) -> Result<Value> {
    normalized["contract"] = json!(NORMALIZED_DECISION_INPUT_CONTRACT_V2);
    normalized["job_id"] = json!(job_id);
    normalized["decision_input_contracts"] = json!(
        compiled["decision_input_contracts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|contract| contract["id"].as_str())
            .collect::<Vec<_>>()
    );
    let binding_sha = sha256_hex(&json_bytes(binding)?);
    let request_sha = sha256_hex(&json_bytes(request)?);
    let results_sha = sha256_hex(&json_bytes(results)?);
    normalized["source_binding_sha256"] = json!(binding_sha);
    normalized["source_attempt_request_sha256"] = json!(request_sha);
    normalized["collected_attempt_results_sha256"] = json!(results_sha);
    if let Some(observations) = normalized["signal_observations"].as_array_mut() {
        for observation in observations {
            observation["receipt"]["source_binding_sha256"] = json!(binding_sha);
            observation["receipt"]["source_attempt_request_sha256"] = json!(request_sha);
            observation["receipt"]["collected_results_sha256"] = json!(results_sha);
        }
    }
    Ok(normalized)
}

fn deterministic_value(attribute: &Value, as_of: &str, seed: u64, ordinal: usize) -> Result<Value> {
    let id = attribute["id"].as_str().unwrap_or("attribute");
    let value_contract = &attribute["value"];
    if let Some(values) = value_contract["enum"]
        .as_array()
        .filter(|values| !values.is_empty())
    {
        let preferred = match id {
            "customer_motion" => "self-serve",
            "enterprise_eligibility" => "eligible",
            "do_not_contact" => "clear",
            "open_support_escalation" => "escalation-clear",
            "account_owner_state" => "assigned",
            "employee_band" => "201-1000",
            "executive_sponsor" => "identified",
            _ => "",
        };
        if let Some(value) = values
            .iter()
            .find(|value| value.as_str() == Some(preferred))
        {
            return Ok(value.clone());
        }
        return Ok(values[0].clone());
    }
    let value_type = value_contract["type"].as_str().unwrap_or("string");
    match value_type {
        "string" => {
            if value_contract["format"].as_str() == Some("date-time") {
                return Ok(json!(as_of));
            }
            let value = match id {
                "company_name" => "Example Expansion Account".to_string(),
                "company_domain" => "example.invalid".to_string(),
                "person_name" => "Avery Example".to_string(),
                "person_title" => "Synthetic Example Contact".to_string(),
                "expansion_trigger_summary" => {
                    "Synthetic usage threshold reached in the reviewed fixture.".to_string()
                }
                "current_working_country" => "United States".to_string(),
                "latest_support_context" => "Synthetic fixture support context.".to_string(),
                _ => format!("Synthetic example value {seed}-{ordinal} for {id}."),
            };
            Ok(json!(value))
        }
        "number" => Ok(json!(0)),
        "integer" => Ok(json!(0)),
        "boolean" => Ok(json!(false)),
        "array" => Ok(json!([])),
        "object" => Ok(json!({})),
        other => Err(anyhow!("synthetic_chain_value_type_unsupported: {other}")),
    }
}

fn attempt_state(
    attribute: &Value,
    status: &str,
    value: Value,
    attempt_id: &str,
    locator: &str,
    as_of: &str,
    ordinal: usize,
) -> Value {
    let mut state = json!({
        "status": status,
        "provenance": [{
            "attempt_id": attempt_id,
            "source_class": "synthetic_fixture",
            "source_locator": locator,
            "observed_at": as_of
        }],
        "confidence": 100,
        "freshness": {"observed_at": as_of, "age_days": 0}
    });
    if attribute["provenance"]["required_fields"]
        .as_array()
        .is_some_and(|fields| fields.iter().any(|field| field == "excerpt"))
    {
        state["provenance"][0]["excerpt"] = json!(format!(
            "Synthetic fixture value for {}.",
            attribute["id"].as_str().unwrap_or("attribute")
        ));
    }
    if status == "observed" {
        state["value"] = value;
    }
    let _ = ordinal;
    state
}

fn applies_when(attribute: &Value, attributes: &Map<String, Value>) -> bool {
    attribute["applies_when"]
        .as_array()
        .into_iter()
        .flatten()
        .all(|condition| {
            let id = condition["attribute"].as_str().unwrap_or_default();
            let Some(previous) = attributes.get(id) else {
                return false;
            };
            if previous["status"] != "observed" {
                return false;
            }
            match condition["operator"].as_str().unwrap_or("exists") {
                "exists" => true,
                "equals" => condition["values"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|value| value == &previous["value"]),
                "not_equals" => condition["values"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .all(|value| value != &previous["value"]),
                "in" => condition["values"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|value| value == &previous["value"]),
                _ => false,
            }
        })
}

fn set_output_path(target: &mut Value, output_path: Option<&str>, value: &Value) {
    let Some(output_path) = output_path else {
        return;
    };
    let mut current = target;
    let segments = output_path.split('.').collect::<Vec<_>>();
    for segment in &segments[..segments.len().saturating_sub(1)] {
        if !current["attributes"].is_object() && *segment == "attributes" {
            current["attributes"] = json!({});
        }
        if current.get(segment).is_none() {
            current[segment] = json!({});
        }
        current = &mut current[*segment];
    }
    if let Some(last) = segments.last() {
        current[*last] = value.clone();
    }
}

fn contract_versions(compiled: &Value) -> Vec<Value> {
    compiled["decision_input_contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|contract| json!({"id": contract["id"], "version": contract["version"]}))
        .collect()
}

fn serialize_candidates(
    out_dir: &Path,
    binding: &Value,
    request: &Value,
    results: &Value,
    normalized: &Value,
) -> Result<Vec<CandidateFile>> {
    Ok(vec![
        CandidateFile {
            logical_name: SOURCE_BINDING_FILE,
            path: out_dir.join(SOURCE_BINDING_FILE),
            bytes: json_bytes(binding)?,
        },
        CandidateFile {
            logical_name: SOURCE_ATTEMPT_REQUEST_FILE,
            path: out_dir.join(SOURCE_ATTEMPT_REQUEST_FILE),
            bytes: json_bytes(request)?,
        },
        CandidateFile {
            logical_name: COLLECTED_ATTEMPT_RESULTS_FILE,
            path: out_dir.join(COLLECTED_ATTEMPT_RESULTS_FILE),
            bytes: json_bytes(results)?,
        },
        CandidateFile {
            logical_name: NORMALIZED_INPUT_FILE,
            path: out_dir.join(NORMALIZED_INPUT_FILE),
            bytes: json_bytes(normalized)?,
        },
    ])
}

fn json_bytes(value: &Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn stage_candidates(stage_dir: &Path, candidates: &[CandidateFile]) -> Result<Vec<CandidateFile>> {
    fs::create_dir_all(stage_dir)?;
    let staged = candidates
        .iter()
        .map(|candidate| CandidateFile {
            logical_name: candidate.logical_name,
            path: stage_dir.join(candidate.logical_name),
            bytes: candidate.bytes.clone(),
        })
        .collect::<Vec<_>>();
    for candidate in &staged {
        fs::write(&candidate.path, &candidate.bytes)?;
    }
    Ok(staged)
}

fn validate_staged_chain(
    root: &Path,
    job_id: &str,
    stage_dir: &Path,
    staged: &[CandidateFile],
) -> Result<Value> {
    let binding = staged
        .iter()
        .find(|file| file.logical_name == SOURCE_BINDING_FILE)
        .unwrap();
    let request = staged
        .iter()
        .find(|file| file.logical_name == SOURCE_ATTEMPT_REQUEST_FILE)
        .unwrap();
    let results = staged
        .iter()
        .find(|file| file.logical_name == COLLECTED_ATTEMPT_RESULTS_FILE)
        .unwrap();
    let normalized = staged
        .iter()
        .find(|file| file.logical_name == NORMALIZED_INPUT_FILE)
        .unwrap();
    let binding_validation = validate_source_binding_file(root, job_id, &binding.path)?;
    let compiled = requirements(root, job_id)?;
    let prompt_rel = compiled["decision_input_contracts"][0]["normalization"]["prompt"]
        .as_str()
        .ok_or_else(|| anyhow!("normalization prompt is missing"))?;
    let prompt_path = root.join(DEFAULT_DIR).join(prompt_rel);
    let prompt_validation = validate_prompt_output_file_with_lineage_inputs(
        root,
        &normalized.path,
        Some(&prompt_path),
        None,
        None,
        Some(&binding.path),
        Some(&request.path),
        Some(&results.path),
        None,
        None,
    )?;
    let _ = stage_dir;
    Ok(json!({
        "source_binding": validation_receipt(&binding_validation),
        "prompt_output": validation_receipt(&prompt_validation)
    }))
}

fn validation_receipt(value: &Value) -> Value {
    let mut result = json!({"valid": value["valid"], "contract": value["contract"]});
    if let Some(issues) = value.get("issues") {
        result["issues"] = issues.clone();
    }
    if let Some(projection) = value.get("signal_projection") {
        result["signal_projection"] = projection.clone();
    }
    result
}

fn plan_writes(candidates: &[CandidateFile], apply: bool, force: bool) -> Result<Vec<Value>> {
    candidates
        .iter()
        .map(|candidate| {
            let new_sha = sha256_hex(&candidate.bytes);
            let new_size = candidate.bytes.len();
            let (action, old_sha, old_size, backup_path) = if !candidate.path.exists() {
                ("create", Value::Null, Value::Null, Value::Null)
            } else {
                let old = fs::read(&candidate.path)?;
                let old_sha = sha256_hex(&old);
                if old == candidate.bytes {
                    ("unchanged", json!(old_sha), json!(old.len()), Value::Null)
                } else if apply && force {
                    (
                        "overwrite-with-backup",
                        json!(old_sha),
                        json!(old.len()),
                        json!(backup_path(&candidate.path, &old_sha)?),
                    )
                } else {
                    ("blocked", json!(old_sha), json!(old.len()), Value::Null)
                }
            };
            Ok(json!({
                "name": candidate.logical_name,
                "path": candidate.path,
                "action": action,
                "old_byte_count": old_size,
                "old_sha256": old_sha,
                "new_byte_count": new_size,
                "new_sha256": new_sha,
                "backup_path": backup_path,
                "would_write": action == "create" || action == "overwrite-with-backup"
            }))
        })
        .collect()
}

fn apply_writes(candidates: &[CandidateFile], write_plan: &[Value]) -> Result<()> {
    if let Some(parent) = candidates
        .first()
        .and_then(|candidate| candidate.path.parent())
    {
        fs::create_dir_all(parent)?;
    }
    let mut previous = Vec::<(PathBuf, Option<Vec<u8>>)>::new();
    for item in write_plan {
        if item["action"] == "unchanged" {
            continue;
        }
        let path = PathBuf::from(item["path"].as_str().unwrap_or_default());
        if item["action"] == "overwrite-with-backup" {
            let old = fs::read(&path)?;
            let backup = PathBuf::from(item["backup_path"].as_str().unwrap_or_default());
            fs::write(&backup, old)?;
        }
    }
    for candidate in candidates {
        let action = write_plan
            .iter()
            .find(|item| item["name"] == candidate.logical_name)
            .map(|item| item["action"].as_str().unwrap_or_default())
            .unwrap_or("blocked");
        if action == "unchanged" {
            continue;
        }
        let old = if candidate.path.exists() {
            Some(fs::read(&candidate.path)?)
        } else {
            None
        };
        previous.push((candidate.path.clone(), old));
        let temp = candidate.path.with_file_name(format!(
            ".{}.tmp-{}",
            candidate.logical_name,
            unique_suffix()
        ));
        if let Err(error) = (|| -> Result<()> {
            fs::write(&temp, &candidate.bytes)?;
            fs::rename(&temp, &candidate.path)?;
            Ok(())
        })() {
            let _ = fs::remove_file(&temp);
            for (path, old) in previous.iter().rev() {
                match old {
                    Some(bytes) => {
                        let _ = fs::write(path, bytes);
                    }
                    None => {
                        let _ = fs::remove_file(path);
                    }
                }
            }
            return Err(error);
        }
    }
    Ok(())
}

fn file_metadata(candidates: &[CandidateFile]) -> Vec<Value> {
    candidates
        .iter()
        .map(|candidate| {
            json!({
                "name": candidate.logical_name,
                "path": candidate.path,
                "byte_count": candidate.bytes.len(),
                "sha256": sha256_hex(&candidate.bytes)
            })
        })
        .collect()
}

fn base_result(
    compiled: &Value,
    job_id: &str,
    as_of: &str,
    seed: u64,
    mode: &str,
    dry_run: bool,
) -> Value {
    json!({
        "contract": SYNTHETIC_V2_CHAIN_CONTRACT,
        "valid": true,
        "status": "planned",
        "mode": mode,
        "dry_run": dry_run,
        "applied": false,
        "pack": compiled["pack"],
        "requirements": {
            "contract": compiled["contract"],
            "sha256": compiled["requirements_sha256"],
            "decision_input_contracts": contract_versions(compiled)
        },
        "job_id": job_id,
        "deterministic_inputs": {"as_of": as_of, "seed": seed, "mode": mode}
    })
}

fn refusal(code: &str, message: &str) -> Value {
    json!({
        "contract": SYNTHETIC_V2_CHAIN_CONTRACT,
        "valid": false,
        "status": "blocked",
        "refusal": {"code": code, "message": message},
        "write_plan": [],
        "files": []
    })
}

fn read_input_chain(input_dir: &Path) -> Result<(Value, Value, Value, Value)> {
    if !input_dir.is_dir() {
        return Err(anyhow!(
            "input directory does not exist: {}",
            input_dir.display()
        ));
    }
    Ok((
        read_json(&input_dir.join(SOURCE_BINDING_FILE))?,
        read_json(&input_dir.join(SOURCE_ATTEMPT_REQUEST_FILE))?,
        read_json(&input_dir.join(COLLECTED_ATTEMPT_RESULTS_FILE))?,
        read_json(&input_dir.join(NORMALIZED_INPUT_FILE))?,
    ))
}

fn read_json(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
}

fn reject_non_synthetic_chain(
    binding: &Value,
    request: &Value,
    results: &Value,
    normalized: &Value,
) -> Option<String> {
    if normalized["normalized_prospect"]["synthetic"] != true
        || normalized["normalized_prospect"]["source_kind"]
            .as_str()
            .is_none_or(|kind| !kind.starts_with("synthetic"))
    {
        return Some(
            "normalized_prospect must be explicitly marked synthetic with a synthetic source_kind"
                .to_string(),
        );
    }
    let mut source_class_count = 0usize;
    let mut failure = None;
    fn visit(value: &Value, key: Option<&str>, count: &mut usize, failure: &mut Option<String>) {
        if failure.is_some() {
            return;
        }
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    if key == "source_class" {
                        *count += 1;
                        if value != "synthetic_fixture" {
                            *failure = Some(format!(
                                "source_class must be synthetic_fixture, received {value}"
                            ));
                            return;
                        }
                    }
                    if matches!(
                        key.as_str(),
                        "provider" | "credential" | "credentials" | "customer_record"
                    ) {
                        *failure = Some(format!(
                            "private or provider provenance field {key} is not rebindable"
                        ));
                        return;
                    }
                    visit(value, Some(key), count, failure);
                }
            }
            Value::Array(items) => {
                for item in items {
                    visit(item, key, count, failure);
                }
            }
            Value::String(text) => {
                if matches!(key, Some("source_locator" | "upstream_reference"))
                    && unsafe_locator(text)
                {
                    *failure = Some("source locators and upstream references must remain opaque non-URL synthetic identifiers".to_string());
                }
            }
            _ => {}
        }
    }
    for value in [binding, request, results, normalized] {
        visit(value, None, &mut source_class_count, &mut failure);
    }
    failure.or_else(|| {
        (source_class_count == 0)
            .then(|| "chain contains no explicit source_class provenance markers".to_string())
    })
}

fn reject_input_identity(
    job_id: &str,
    compiled: &Value,
    binding: &Value,
    request: &Value,
    results: &Value,
    normalized: &Value,
) -> Option<String> {
    for (name, value, expected) in [
        ("source binding", binding, SOURCE_BINDING_CONTRACT_V2),
        (
            "source request",
            request,
            SOURCE_ATTEMPT_REQUEST_CONTRACT_V2,
        ),
        (
            "collected results",
            results,
            COLLECTED_ATTEMPT_RESULTS_CONTRACT_V2,
        ),
        (
            "normalized input",
            normalized,
            NORMALIZED_DECISION_INPUT_CONTRACT_V2,
        ),
    ] {
        if value["contract"] != expected {
            return Some(format!("{name} must use {expected}"));
        }
        if value["job_id"] != job_id {
            return Some(format!("{name} job_id does not match --job"));
        }
    }
    if binding["pack"]["id"] != compiled["pack"]["id"] {
        return Some(
            "input source binding pack identity does not match the selected pack".to_string(),
        );
    }
    if request["decision_input_contracts"] != json!(contract_versions(compiled)) {
        return Some("input request contract receipts do not match the selected job".to_string());
    }
    None
}

fn invalid_output_location(root: &Path, output: &Path, input: Option<&Path>) -> Option<String> {
    let pack = root.canonicalize().ok()?;
    let mdp = pack.join(DEFAULT_DIR);
    if output == &pack || output.starts_with(&pack) || output.starts_with(&mdp) {
        return Some("--out-dir must be outside the pack root and its .mdp tree".to_string());
    }
    if input.is_some_and(|input| output == input || output.starts_with(input)) {
        return Some("--out-dir must not overlap --input-dir".to_string());
    }
    None
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn staging_dir() -> Result<PathBuf> {
    let suffix = unique_suffix();
    let path = std::env::temp_dir().join(format!("mdp-synthetic-chain-{suffix}"));
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn cleanup_stage(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

fn backup_path(path: &Path, sha: &str) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("destination has no parent"))?;
    let stem = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact");
    let mut candidate = parent.join(format!(".{stem}.backup-{sha}"));
    let mut index = 1;
    while candidate.exists() {
        candidate = parent.join(format!(".{stem}.backup-{sha}-{index}"));
        index += 1;
    }
    Ok(candidate)
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn unsafe_locator(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    value.starts_with('/')
        || value.starts_with("~/")
        || value.contains("\\")
        || lower.contains("://")
        || lower.contains("/users/")
        || lower.contains("private")
        || lower.contains("credential")
        || lower.contains("provider")
}

fn valid_utc_timestamp(value: &str) -> bool {
    value.len() == 20
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value.as_bytes()[10] == b'T'
        && value.as_bytes()[13] == b':'
        && value.as_bytes()[16] == b':'
        && value.ends_with('Z')
        && value.chars().enumerate().all(|(index, character)| {
            [4, 7].contains(&index)
                || [10].contains(&index)
                || [13, 16].contains(&index)
                || [19].contains(&index)
                || character.is_ascii_digit()
        })
}

pub(crate) fn synthetic_chain_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Synthetic Governed v2 Chain Result",
        "type": "object",
        "additionalProperties": true,
        "required": ["contract", "valid", "status"],
        "properties": {
            "contract": {"const": SYNTHETIC_V2_CHAIN_CONTRACT},
            "valid": {"type": "boolean"},
            "status": {"enum": ["planned", "dry-run", "applied", "blocked"]},
            "mode": {"enum": ["fresh", "rebind"]},
            "dry_run": {"type": "boolean"},
            "applied": {"type": "boolean"},
            "job_id": {"type": "string"},
            "deterministic_inputs": {"type": "object"},
            "write_plan": {"type": "array"},
            "validation": {"type": "object"},
            "refusal": {"type": "object", "required": ["code", "message"]}
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emitted_json_hash_includes_trailing_newline() {
        let bytes = json_bytes(&json!({"a": 1})).expect("JSON should serialize");
        assert!(bytes.ends_with(b"\n"));
        assert_ne!(sha256_hex(&bytes), sha256_hex(&bytes[..bytes.len() - 1]));
    }

    #[test]
    fn unsafe_locator_refuses_urls_and_private_paths() {
        assert!(unsafe_locator("https://example.invalid/source"));
        assert!(unsafe_locator("/Users/example/private.json"));
        assert!(!unsafe_locator("opaque:synthetic:fixture:0"));
    }
}
