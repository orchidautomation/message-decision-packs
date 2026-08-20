use crate::artifact_hash::{canonical_json_sha256, pack_content_sha256};
use crate::cli::SchemaTarget;
use crate::commands::health::{profile_activation_decision, validate_pack};
use crate::commands::schemas::schema;
use crate::commands::schemas::signal_observation_v2_schema;
use crate::commands::source_binding::{
    source_binding_schema_v2, source_lineage_version_matrix, validate_source_binding_v2,
};
use crate::constants::{
    COLLECTED_ATTEMPT_RESULTS_CONTRACT, COLLECTED_ATTEMPT_RESULTS_CONTRACT_V2,
    NORMALIZED_DECISION_INPUT_CONTRACT, NORMALIZED_DECISION_INPUT_CONTRACT_V2,
    REQUIREMENTS_CONTRACT, REQUIREMENTS_CONTRACT_V2, SOURCE_ATTEMPT_REQUEST_CONTRACT_V2,
};
use crate::model_steps::{MODEL_STEP_RESOLUTION_V1, resolve_model_steps};
use crate::models::{
    DecisionInputAttemptStatus, DecisionInputAttribute, DecisionInputCondition,
    DecisionInputConditionOperator, DecisionInputContract, DecisionInputDisposition,
    DecisionInputRequirement, DecisionInputSourceClass, InputContract, Manifest, ProfileJob,
    ValueContract,
};
use crate::pack_io::{read_canonical_prompt_by_id, read_manifest, resolve_pack_path};
use crate::product_foundation::{
    apply_validation_errors_for_job, resolution_json, resolve_product_foundation_for_pack,
    validation_errors_block_job, validation_issues_for_job,
};
use crate::value_contracts::{canonical_values_equal, valid_date, valid_date_time};
use anyhow::{Result, anyhow};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) struct ResolvedJobDecisionInputs<'a> {
    pub(crate) input_contracts: Vec<&'a InputContract>,
    pub(crate) decision_input_contracts: Vec<&'a DecisionInputContract>,
}

pub(crate) fn resolve_job_decision_inputs<'a>(
    manifest: &'a Manifest,
    job: &'a ProfileJob,
) -> Result<ResolvedJobDecisionInputs<'a>> {
    let input_contracts = job
        .input_contracts
        .iter()
        .map(|id| {
            manifest
                .input_contracts
                .iter()
                .find(|contract| contract.id == *id)
                .ok_or_else(|| anyhow!("job {} references missing input contract {id}", job.id))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut decision_input_ids = job
        .decision_input_contracts
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for input_contract in &input_contracts {
        decision_input_ids.extend(input_contract.decision_input_contracts.iter().cloned());
    }
    let decision_input_contracts = decision_input_ids
        .iter()
        .map(|id| {
            manifest
                .decision_input_contracts
                .iter()
                .find(|contract| contract.id == *id)
                .ok_or_else(|| {
                    anyhow!(
                        "job {} references missing decision input contract {id}",
                        job.id
                    )
                })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ResolvedJobDecisionInputs {
        input_contracts,
        decision_input_contracts,
    })
}

pub(crate) fn requirements(root: &Path, job_id: &str) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let job = manifest
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .ok_or_else(|| anyhow!("unknown profile job {job_id}"))?;
    let pack_sha256 = pack_content_sha256(root)?;
    let validation = validate_pack(root)?;
    let profile_activation = profile_activation_decision(
        &validation,
        manifest.profile_eval.blocks_activation(),
        Some(job_id),
    );
    let activation_blocked = profile_activation["status"] == "blocked";
    let model_task = compile_model_task(root, job);
    let model_steps = match resolve_model_steps(root, &manifest, job) {
        Ok(resolution) => serde_json::to_value(resolution)?,
        Err(error) => json!({
            "contract": MODEL_STEP_RESOLUTION_V1,
            "job_id": job.id,
            "status": "blocked",
            "steps": [],
            "diagnostics": [{
                "code": "model_step_resolution_failed",
                "severity": "error",
                "message": error.to_string()
            }]
        }),
    };
    let mut product_foundation_resolution =
        resolve_product_foundation_for_pack(root, &manifest, job_id);
    if validation["valid"] != true {
        let product_foundation = match product_foundation_resolution.as_mut() {
            Ok(mut resolution) => {
                if let Some(issues) = validation["issues"].as_array() {
                    apply_validation_errors_for_job(&mut resolution, &manifest, issues);
                }
                resolution_json(&resolution)
            }
            Err(_) => blocked_product_foundation_resolution(job_id),
        };
        let validation_issues = product_foundation_resolution.as_ref().map_or_else(
            |_| validation["issues"].as_array().cloned().unwrap_or_default(),
            |resolution| {
                validation_issues_for_job(
                    &manifest,
                    resolution,
                    validation["issues"]
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                )
            },
        );
        let validation_blocks_job = product_foundation_resolution
            .as_ref()
            .map_or(true, |resolution| {
                validation_errors_block_job(&manifest, resolution, &validation_issues)
            });
        if validation_blocks_job {
            return finalize_requirements(json!({
                "contract": REQUIREMENTS_CONTRACT,
                "status": "invalid",
                "valid": false,
                "available": false,
                "pack": pack_summary(&manifest, &pack_sha256),
                "job": {
                    "id": &job.id,
                    "skill_id": &job.skill_id,
                    "input_contracts": &job.input_contracts,
                    "decision_input_contracts": &job.decision_input_contracts
                },
                "product_foundation": product_foundation,
                "profile_activation": profile_activation,
                "model_task": model_task,
                "model_steps": model_steps,
                "decision_input_contracts": [],
                "diagnostics": validation_issues
            }));
        }
    }
    let product_foundation = resolution_json(&product_foundation_resolution?);
    let resolved_inputs = resolve_job_decision_inputs(&manifest, job)?;
    let selected_input_contracts = resolved_inputs.input_contracts;
    let selected_contracts = resolved_inputs.decision_input_contracts;
    let selected_ids = selected_contracts
        .iter()
        .map(|contract| contract.id.clone())
        .collect::<BTreeSet<_>>();
    if selected_contracts.is_empty() && model_task.is_null() {
        let model_steps_blocked = model_steps["status"] == "blocked";
        let mut model_step_diagnostics = model_steps["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| {
                vec![json!({
                    "code": "decision_input_contract_not_bound",
                    "severity": "info",
                    "message": "This job has no Decision Input contract. Existing fit/readiness behavior remains available through lead_input_requirements."
                })]
            });
        model_step_diagnostics.extend(
            profile_activation["diagnostics"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
        );
        return finalize_requirements(json!({
            "contract": REQUIREMENTS_CONTRACT,
            "status": if model_steps_blocked || activation_blocked { "blocked" } else { "unavailable" },
            "valid": true,
            "available": false,
            "draft_allowed": false,
            "pack": pack_summary(&manifest, &pack_sha256),
            "job": {
                "id": &job.id,
                "skill_id": &job.skill_id,
                "input_contracts": &job.input_contracts,
                "resolved_input_contracts": selected_input_contracts
            },
            "product_foundation": product_foundation,
            "profile_activation": profile_activation,
            "model_task": model_task,
            "model_steps": model_steps,
            "decision_input_contracts": [],
            "diagnostics": model_step_diagnostics
        }));
    }

    if selected_contracts.is_empty() {
        let foundation_blocked = product_foundation["status"] == "blocked";
        let model_task_blocked = model_task["status"] == "blocked";
        let model_steps_blocked = model_steps["status"] == "blocked";
        let drafting_blocked =
            foundation_blocked || activation_blocked || model_task_blocked || model_steps_blocked;
        let mut diagnostics = product_foundation["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        diagnostics.extend(
            model_steps["diagnostics"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
        );
        diagnostics.extend(
            profile_activation["diagnostics"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
        );
        return finalize_requirements(json!({
            "contract": REQUIREMENTS_CONTRACT,
            "status": if drafting_blocked { "blocked" } else { "ready" },
            "valid": true,
            "available": false,
            "model_task_available": true,
            "draft_allowed": !drafting_blocked,
            "pack": pack_summary(&manifest, &pack_sha256),
            "job": {
                "id": &job.id,
                "skill_id": &job.skill_id,
                "input_contracts": &job.input_contracts,
                "resolved_input_contracts": selected_input_contracts
            },
            "product_foundation": product_foundation,
            "profile_activation": profile_activation,
            "model_task": model_task,
            "model_steps": model_steps,
            "decision_input_contracts": [],
            "diagnostics": diagnostics
        }));
    }

    let compiled_contracts = selected_contracts
        .iter()
        .map(|contract| compile_contract(contract))
        .collect::<Vec<_>>();
    let normalized_schema = normalized_envelope_schema(job_id, &selected_contracts);
    let source_attempt_schema = source_attempt_request_schema(job_id, &selected_contracts);
    let collected_results_schema = collected_attempt_results_schema(job_id, &selected_contracts);
    let signal_aware = selected_contracts
        .iter()
        .any(|contract| !contract.signal_projections.is_empty());
    let foundation_blocked = product_foundation["status"] == "blocked";
    let model_task_blocked = model_task["status"] == "blocked";
    let model_steps_blocked = model_steps["status"] == "blocked";
    let drafting_blocked =
        foundation_blocked || activation_blocked || model_task_blocked || model_steps_blocked;
    let mut diagnostics = product_foundation["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    diagnostics.extend(
        model_steps["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
    );
    diagnostics.extend(
        profile_activation["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
    );
    let mut response = json!({
        "contract": REQUIREMENTS_CONTRACT,
        "status": if drafting_blocked { "blocked" } else { "ready" },
        "valid": true,
        "available": true,
        "pack": pack_summary(&manifest, &pack_sha256),
        "job": {
            "id": &job.id,
            "skill_id": &job.skill_id,
            "input_contracts": &job.input_contracts,
            "resolved_input_contracts": selected_input_contracts,
            "decision_input_contracts": selected_ids
        },
        "product_foundation": product_foundation,
        "profile_activation": profile_activation,
        "model_task": model_task,
        "model_steps": model_steps,
        "decision_input_contracts": compiled_contracts,
        "source_attempt_request_schema": source_attempt_schema,
        "collected_attempt_results_schema": collected_results_schema,
        "normalized_output_schema": normalized_schema,
        "normalized_prospect_schema": schema(SchemaTarget::Prospect),
        "normalized_prospect_unbound_policy": {
            "allowed_fields": ["source_kind", "synthetic"],
            "reason": "Only non-decision provenance/safety markers may be present without a Decision Input output_path. All identity, fit, routing, signal, and attribute values require an explicit output_path."
        },
        "semantic_validation": {
            "command": if signal_aware {
                "mdp --json validate-prompt-output --dir PACK_ROOT --prompt BOUND_NORMALIZATION_PROMPT --source-binding SOURCE_BINDING.json --source-attempt-request SOURCE_ATTEMPT_REQUEST.json --collected-attempt-results COLLECTED_ATTEMPT_RESULTS.json --file NORMALIZED_INPUT.json"
            } else {
                "mdp --json validate-prompt-output --dir PACK_ROOT --prompt BOUND_NORMALIZATION_PROMPT --source-attempt-request SOURCE_ATTEMPT_REQUEST.json --collected-attempt-results COLLECTED_ATTEMPT_RESULTS.json --file NORMALIZED_INPUT.json"
            },
            "checks": [
                "exact-compiled-schema",
                if signal_aware { "bound-source-binding" } else { "scalar-v1-no-source-binding" },
                "bound-source-attempt-request",
                "bound-collected-attempt-results",
                "bound-normalization-prompt",
                "trusted-freshness",
                "attribute-output-path-consistency",
                "no-unbound-prospect-fields",
                if signal_aware { "exact-signal-projection-and-conflict-policy" } else { "scalar-v1-compatibility" }
            ]
        },
        "no_draft_policy": {
            "copy_allowed_only_when": "ready",
            "blocked_outcomes": [
                "insufficient-context",
                "disqualified",
                "human-review",
                "malformed",
                "provider-error"
            ],
            "reason": "MDP decision context may compile before drafting, but missing or unsafe decision inputs must never be converted into draft assumptions."
        },
        "boundaries": {
            "mdp_owns": ["requirements", "validation", "fit", "routing", "brief", "gaps", "optional-output-check"],
            "customer_or_host_owns": ["source-collection", "provider-access", "normalization-model-call", "copy-generation", "sequencing"],
            "network_calls": false,
            "model_calls": false
        },
        "diagnostics": diagnostics
    });
    if signal_aware {
        response["contract"] = json!(REQUIREMENTS_CONTRACT_V2);
        response["runtime_contract_version"] = json!("v2");
        response["source_binding_schema"] = source_binding_schema_v2();
        response["contract_version_matrix"] = source_lineage_version_matrix();
    }
    if drafting_blocked {
        response["draft_allowed"] = json!(false);
    }
    finalize_requirements(response)
}

fn blocked_product_foundation_resolution(job_id: &str) -> Value {
    json!({
        "job_id": job_id,
        "status": "blocked",
        "selected_facets": [],
        "optional_facet_ids": [],
        "excluded_facet_ids": [],
        "untriggered_facet_ids": [],
        "diagnostics": [{
            "code": "product_foundation_resolution_failed",
            "severity": "error",
            "path": ".mdp/manifest.yaml#/cards",
            "message": "product foundation could not be resolved from declared card authority"
        }]
    })
}

pub(crate) fn validation_has_only_foundation_errors(validation: &Value) -> bool {
    let errors = validation["issues"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|issue| issue["severity"] == "error")
        .collect::<Vec<_>>();
    !errors.is_empty()
        && errors.iter().all(|issue| {
            issue["code"].as_str().is_some_and(|code| {
                code.starts_with("product_foundation_")
                    || code.starts_with("profile_job_product_foundation_")
                    || code.starts_with("manifest_product_foundation_")
                    || code.starts_with("manifest_profile_job_product_foundation_")
            })
        })
}

#[cfg(test)]
pub(crate) fn validate_normalized_decision_input(
    root: &Path,
    output: &Value,
    artifact_path: &str,
    resolved_prompt_path: &Path,
    source_attempt_request: Option<(&Value, &str, &str)>,
    collected_attempt_results: Option<(&Value, &str, &str)>,
) -> Result<Vec<Value>> {
    let validation = validate_normalized_decision_input_with_projection(
        root,
        output,
        artifact_path,
        resolved_prompt_path,
        None,
        source_attempt_request,
        collected_attempt_results,
        None,
    )?;
    Ok(validation
        .issues
        .into_iter()
        .filter(|issue| issue["code"] != "decision_input_source_binding_missing")
        .collect())
}

pub(crate) struct NormalizedDecisionInputValidation {
    pub(crate) issues: Vec<Value>,
    pub(crate) signal_projection: Option<Value>,
}

pub(crate) fn validate_normalized_decision_input_with_projection(
    root: &Path,
    output: &Value,
    artifact_path: &str,
    resolved_prompt_path: &Path,
    source_binding: Option<(&Value, &str, &str)>,
    source_attempt_request: Option<(&Value, &str, &str)>,
    collected_attempt_results: Option<(&Value, &str, &str)>,
    normalized_output_sha256: Option<&str>,
) -> Result<NormalizedDecisionInputValidation> {
    let Some(job_id) = output["job_id"].as_str() else {
        return Ok(NormalizedDecisionInputValidation {
            issues: vec![decision_input_issue(
                "decision_input_job_id_missing",
                artifact_path,
                "normalized decision input must include a string job_id",
            )],
            signal_projection: None,
        });
    };
    let compiled = match requirements(root, job_id) {
        Ok(compiled) => compiled,
        Err(_) => {
            return Ok(NormalizedDecisionInputValidation {
                issues: vec![decision_input_issue(
                    "decision_input_job_unknown",
                    format!("{artifact_path}#/job_id"),
                    "normalized decision input references an unknown pack job",
                )],
                signal_projection: None,
            });
        }
    };
    if compiled["available"] != true {
        return Ok(NormalizedDecisionInputValidation {
            issues: vec![decision_input_issue(
                "decision_input_requirements_unavailable",
                artifact_path,
                "the referenced job does not compile an available decision-input contract",
            )],
            signal_projection: None,
        });
    }
    if jsonschema::draft202012::validate(&compiled["normalized_output_schema"], output).is_err() {
        return Ok(NormalizedDecisionInputValidation {
            issues: vec![decision_input_issue(
                "decision_input_schema_mismatch",
                artifact_path,
                "normalized decision input does not satisfy the exact compiled job schema",
            )],
            signal_projection: None,
        });
    }

    let mut issues = Vec::new();
    validate_bound_normalization_prompt(
        root,
        &compiled,
        resolved_prompt_path,
        artifact_path,
        &mut issues,
    );
    let source_attempt_index = validate_source_attempt_request(
        &compiled,
        output,
        artifact_path,
        source_attempt_request,
        &mut issues,
    );
    validate_collected_attempt_results(
        &compiled,
        output,
        artifact_path,
        source_attempt_request.map(|(_, _, sha256)| sha256),
        collected_attempt_results,
        &mut issues,
    );
    validate_no_unbound_prospect_fields(&compiled, output, artifact_path, &mut issues);
    for contract in compiled["decision_input_contracts"]
        .as_array()
        .into_iter()
        .flatten()
    {
        for attribute in contract["attributes"].as_array().into_iter().flatten() {
            let Some(attribute_id) = attribute["id"].as_str() else {
                continue;
            };
            let attempt = &output["attributes"][attribute_id];
            let Some(output_path) = attribute["output_path"].as_str() else {
                continue;
            };
            let projected = value_at_output_path(&output["normalized_prospect"], output_path);
            if attempt["status"].as_str() == Some("observed") && projected != attempt.get("value") {
                issues.push(decision_input_issue(
                    "decision_input_projection_mismatch",
                    format!(
                        "{artifact_path}#/normalized_prospect/{}",
                        output_path.replace('.', "/")
                    ),
                    format!(
                        "observed attribute {attribute_id} must equal its declared normalized_prospect output_path {output_path}"
                    ),
                ));
            } else if attempt["status"].as_str() != Some("observed")
                && projected.is_some_and(meaningful_projected_value)
            {
                issues.push(decision_input_issue(
                    "decision_input_unobserved_projection_present",
                    format!(
                        "{artifact_path}#/normalized_prospect/{}",
                        output_path.replace('.', "/")
                    ),
                    format!(
                        "non-observed attribute {attribute_id} must leave its declared normalized_prospect output_path {output_path} absent or neutral"
                    ),
                ));
            }
            validate_observed_value_format(attribute, attempt, artifact_path, &mut issues);
            validate_attribute_attempt_receipts(
                attribute,
                attempt,
                artifact_path,
                source_attempt_index.as_ref(),
                &mut issues,
            );
        }
    }
    let signal_projection = if output["contract"] == NORMALIZED_DECISION_INPUT_CONTRACT_V2 {
        validate_signal_observations(
            &compiled,
            output,
            artifact_path,
            source_binding,
            source_attempt_request,
            collected_attempt_results,
            normalized_output_sha256,
            &mut issues,
        )
    } else {
        None
    };
    Ok(NormalizedDecisionInputValidation {
        issues,
        signal_projection,
    })
}

fn validate_observed_value_format(
    attribute: &Value,
    attempt: &Value,
    artifact_path: &str,
    issues: &mut Vec<Value>,
) {
    if attempt["status"].as_str() != Some("observed") {
        return;
    }
    let Some(format) = attribute["value"]["format"].as_str() else {
        return;
    };
    let Some(value) = attempt["value"].as_str() else {
        return;
    };
    let valid = match format {
        "date" => valid_date(value),
        "date-time" => valid_date_time(value),
        _ => true,
    };
    if !valid {
        let attribute_id = attribute["id"].as_str().unwrap_or("<unknown>");
        issues.push(decision_input_issue(
            "decision_input_observed_value_format_invalid",
            format!("{artifact_path}#/attributes/{attribute_id}/value"),
            format!("observed attribute {attribute_id} must use valid {format} format"),
        ));
    }
}

struct SourceAttemptIndex<'a> {
    attempts: BTreeMap<&'a str, &'a Value>,
    as_of_seconds: i64,
}

fn validate_bound_normalization_prompt(
    root: &Path,
    compiled: &Value,
    resolved_prompt_path: &Path,
    artifact_path: &str,
    issues: &mut Vec<Value>,
) {
    for contract in compiled["decision_input_contracts"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let Some(bound_prompt) = contract["normalization"]["prompt"].as_str() else {
            continue;
        };
        let Ok(expected_path) = resolve_pack_path(root, bound_prompt) else {
            issues.push(decision_input_issue(
                "decision_input_bound_prompt_unresolvable",
                artifact_path,
                "the compiled normalization prompt path cannot be resolved inside the pack",
            ));
            continue;
        };
        let selected = resolved_prompt_path
            .canonicalize()
            .unwrap_or_else(|_| resolved_prompt_path.to_path_buf());
        let expected = expected_path.canonicalize().unwrap_or(expected_path);
        if selected != expected {
            issues.push(decision_input_issue(
                "decision_input_prompt_binding_mismatch",
                artifact_path,
                "the selected prompt is not the exact normalization prompt bound to the compiled job",
            ));
        }
    }
}

fn validate_source_attempt_request<'a>(
    compiled: &Value,
    output: &Value,
    artifact_path: &str,
    source_attempt_request: Option<(&'a Value, &str, &str)>,
    issues: &mut Vec<Value>,
) -> Option<SourceAttemptIndex<'a>> {
    let Some((request, request_path, request_sha256)) = source_attempt_request else {
        issues.push(decision_input_issue(
            "decision_input_source_attempt_request_missing",
            artifact_path,
            "decision-input normalization validation requires the exact source-attempt request file",
        ));
        return None;
    };
    if output["source_attempt_request_sha256"].as_str() != Some(request_sha256) {
        issues.push(decision_input_issue(
            "decision_input_source_attempt_request_hash_mismatch",
            format!("{artifact_path}#/source_attempt_request_sha256"),
            "normalized decision input is not bound to the exact supplied source-attempt request",
        ));
    }
    if jsonschema::draft202012::validate(&compiled["source_attempt_request_schema"], request)
        .is_err()
    {
        issues.push(decision_input_issue(
            "decision_input_source_attempt_request_schema_mismatch",
            request_path,
            "source-attempt request does not satisfy the exact compiled job schema",
        ));
        return None;
    }
    let Some(as_of) = request["as_of"]
        .as_str()
        .and_then(parse_utc_timestamp_seconds)
    else {
        issues.push(decision_input_issue(
            "decision_input_source_attempt_as_of_invalid",
            format!("{request_path}#/as_of"),
            "source-attempt request as_of must be a valid UTC timestamp",
        ));
        return None;
    };
    let mut attempts = BTreeMap::new();
    for (index, attempt) in request["attempts"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let Some(attempt_id) = attempt["attempt_id"].as_str() else {
            continue;
        };
        if attempts.insert(attempt_id, attempt).is_some() {
            issues.push(decision_input_issue(
                "decision_input_source_attempt_id_duplicate",
                format!("{request_path}#/attempts/{index}/attempt_id"),
                "source-attempt request attempt_id values must be unique",
            ));
        }
        let Some(requested_at) = attempt["requested_at"]
            .as_str()
            .and_then(parse_utc_timestamp_seconds)
        else {
            issues.push(decision_input_issue(
                "decision_input_source_attempt_requested_at_invalid",
                format!("{request_path}#/attempts/{index}/requested_at"),
                "source-attempt requested_at must be a valid UTC timestamp",
            ));
            continue;
        };
        if requested_at > as_of {
            issues.push(decision_input_issue(
                "decision_input_source_attempt_requested_at_future",
                format!("{request_path}#/attempts/{index}/requested_at"),
                "source-attempt requested_at must not be later than the trusted as_of timestamp",
            ));
        }
    }
    Some(SourceAttemptIndex {
        attempts,
        as_of_seconds: as_of,
    })
}

fn validate_collected_attempt_results(
    compiled: &Value,
    output: &Value,
    artifact_path: &str,
    source_attempt_request_sha256: Option<&str>,
    collected_attempt_results: Option<(&Value, &str, &str)>,
    issues: &mut Vec<Value>,
) {
    let Some((results, results_path, results_sha256)) = collected_attempt_results else {
        issues.push(decision_input_issue(
            "decision_input_collected_attempt_results_missing",
            artifact_path,
            "decision-input normalization validation requires the exact collected attempt-results ledger",
        ));
        return;
    };
    if jsonschema::draft202012::validate(&compiled["collected_attempt_results_schema"], results)
        .is_err()
    {
        issues.push(decision_input_issue(
            "decision_input_collected_attempt_results_schema_mismatch",
            results_path,
            "collected attempt results do not satisfy the exact compiled job schema",
        ));
        return;
    }
    if results["source_attempt_request_sha256"].as_str() != source_attempt_request_sha256 {
        issues.push(decision_input_issue(
            "decision_input_collected_attempt_request_hash_mismatch",
            format!("{results_path}#/source_attempt_request_sha256"),
            "collected attempt results are not bound to the exact supplied source-attempt request",
        ));
    }
    if output["collected_attempt_results_sha256"].as_str() != Some(results_sha256) {
        issues.push(decision_input_issue(
            "decision_input_collected_attempt_results_hash_mismatch",
            format!("{artifact_path}#/collected_attempt_results_sha256"),
            "normalized decision input is not bound to the exact supplied collected attempt-results ledger",
        ));
    }
    for attribute in compiled["decision_input_contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|contract| contract["attributes"].as_array().into_iter().flatten())
    {
        let Some(attribute_id) = attribute["id"].as_str() else {
            continue;
        };
        let normalized = &output["attributes"][attribute_id];
        let collected = &results["attributes"][attribute_id];
        for field in ["status", "provenance", "confidence", "freshness", "error"] {
            if normalized.get(field) != collected.get(field) {
                issues.push(decision_input_issue(
                    "decision_input_collected_attempt_results_mismatch",
                    format!("{artifact_path}#/attributes/{attribute_id}/{field}"),
                    format!(
                        "normalized {field} for {attribute_id} must exactly match the collected attempt-results ledger"
                    ),
                ));
            }
        }
        if collected["status"].as_str() == Some("observed") {
            let expected_value =
                canonicalize_collected_value(&collected["value"], &attribute["value"]);
            if normalized["value"] == expected_value {
                continue;
            }
            issues.push(decision_input_issue(
                "decision_input_canonical_collected_value_changed",
                format!("{artifact_path}#/attributes/{attribute_id}/value"),
                format!(
                    "normalized value for {attribute_id} must equal the collected value after only trim, case-fold, and space/underscore-to-hyphen enum canonicalization"
                ),
            ));
        }
    }
}

fn canonicalize_collected_value(value: &Value, value_contract: &Value) -> Value {
    if jsonschema::draft202012::validate(value_contract, value).is_ok()
        || !value_contract["enum"].is_array()
    {
        return value.clone();
    }
    let Some(raw) = value.as_str() else {
        return value.clone();
    };
    Value::String(
        raw.trim()
            .chars()
            .flat_map(char::to_lowercase)
            .map(|character| {
                if character.is_whitespace() || character == '_' {
                    '-'
                } else {
                    character
                }
            })
            .collect(),
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_signal_observations(
    compiled: &Value,
    output: &Value,
    artifact_path: &str,
    source_binding: Option<(&Value, &str, &str)>,
    source_attempt_request: Option<(&Value, &str, &str)>,
    collected_attempt_results: Option<(&Value, &str, &str)>,
    normalized_output_sha256: Option<&str>,
    issues: &mut Vec<Value>,
) -> Option<Value> {
    let initial_issue_count = issues.len();
    let Some((binding, binding_path, binding_sha256)) = source_binding else {
        issues.push(decision_input_issue(
            "decision_input_source_binding_missing",
            artifact_path,
            "signal-aware normalization validation requires the exact mdp.source-binding.v2 artifact",
        ));
        return None;
    };
    if jsonschema::draft202012::validate(&compiled["source_binding_schema"], binding).is_err() {
        issues.push(decision_input_issue(
            "decision_input_source_binding_schema_mismatch",
            binding_path,
            "source binding does not satisfy the exact compiled signal-aware schema",
        ));
        return None;
    }
    match validate_source_binding_v2(compiled, binding, binding_path) {
        Ok(validation) if validation["valid"] == true => {}
        Ok(validation) => {
            issues.extend(
                validation["diagnostics"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            );
        }
        Err(error) => {
            issues.push(decision_input_issue(
                "decision_input_source_binding_semantic_validation_failed",
                binding_path,
                format!("source binding semantic validation failed: {error}"),
            ));
            return None;
        }
    }
    if output["source_binding_sha256"].as_str() != Some(binding_sha256) {
        issues.push(decision_input_issue(
            "decision_input_source_binding_hash_mismatch",
            format!("{artifact_path}#/source_binding_sha256"),
            "normalized decision input is not bound to the exact supplied source binding",
        ));
    }
    let request_sha256 = source_attempt_request.map(|(_, _, sha)| sha);
    let results_sha256 = collected_attempt_results.map(|(_, _, sha)| sha);
    if let Some((request, request_path, _)) = source_attempt_request
        && request["source_binding_sha256"].as_str() != Some(binding_sha256)
    {
        issues.push(decision_input_issue(
            "decision_input_request_source_binding_hash_mismatch",
            format!("{request_path}#/source_binding_sha256"),
            "source-attempt request is not bound to the exact supplied source binding",
        ));
    }
    if let Some((results, results_path, _)) = collected_attempt_results
        && results["source_binding_sha256"].as_str() != Some(binding_sha256)
    {
        issues.push(decision_input_issue(
            "decision_input_results_source_binding_hash_mismatch",
            format!("{results_path}#/source_binding_sha256"),
            "collected attempt results are not bound to the exact supplied source binding",
        ));
    }

    let mut projection_index = BTreeMap::new();
    for contract in compiled["decision_input_contracts"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let Some(contract_id) = contract["id"].as_str() else {
            continue;
        };
        for projection in contract["signal_projections"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let Some(projection_id) = projection["id"].as_str() else {
                continue;
            };
            projection_index.insert(
                format!("{contract_id}#{projection_id}"),
                (contract, projection),
            );
        }
    }
    let binding_index = binding["projection_bindings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| Some((item["qualified_projection_id"].as_str()?.to_string(), item)))
        .collect::<BTreeMap<_, _>>();
    let request_attempts = source_attempt_request
        .map(|(request, _, _)| {
            request["attempts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|attempt| Some((attempt["attempt_id"].as_str()?.to_string(), attempt)))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let request_as_of = source_attempt_request.and_then(|(request, _, _)| {
        request["as_of"]
            .as_str()
            .and_then(parse_utc_timestamp_seconds)
    });
    let mut attempt_results = BTreeMap::new();
    if let Some((results, results_path, _)) = collected_attempt_results {
        for (index, result) in results["attempt_results"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let Some(id) = result["attempt_id"].as_str() else {
                continue;
            };
            if attempt_results.insert(id.to_string(), result).is_some() {
                issues.push(decision_input_issue(
                    "decision_input_collected_attempt_id_duplicate",
                    format!("{results_path}#/attempt_results/{index}/attempt_id"),
                    "collected attempt result IDs must be unique",
                ));
            }
            let result_path = format!("{results_path}#/attempt_results/{index}");
            let Some(request_attempt) = request_attempts.get(id) else {
                issues.push(decision_input_issue(
                    "decision_input_collected_attempt_unknown",
                    format!("{result_path}/attempt_id"),
                    "collected attempt result is absent from the exact source-attempt request",
                ));
                continue;
            };
            for field in [
                "decision_input_contract_id",
                "attribute_id",
                "source_class",
                "source_locator",
            ] {
                if result[field] != request_attempt[field] {
                    issues.push(decision_input_issue(
                        "decision_input_collected_attempt_request_mismatch",
                        format!("{result_path}/{field}"),
                        format!("collected attempt result {field} must match its exact source-attempt request"),
                    ));
                }
            }
            let observed_at = result["observed_at"]
                .as_str()
                .and_then(parse_utc_timestamp_seconds);
            let requested_at = request_attempt["requested_at"]
                .as_str()
                .and_then(parse_utc_timestamp_seconds);
            if observed_at
                .zip(requested_at)
                .is_some_and(|(observed, requested)| observed < requested)
                || observed_at
                    .zip(request_as_of)
                    .is_some_and(|(observed, as_of)| observed > as_of)
            {
                issues.push(decision_input_issue(
                    "decision_input_collected_attempt_observed_at_out_of_bounds",
                    format!("{result_path}/observed_at"),
                    "collected attempt observed_at must fall between requested_at and the trusted request as_of",
                ));
            }
        }
    }

    let mut observation_ids = BTreeSet::new();
    let mut attempt_projection_use = BTreeMap::<String, String>::new();
    let mut valid_observations = Vec::new();
    for (index, observation) in output["signal_observations"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let path = format!("{artifact_path}#/signal_observations/{index}");
        let Some(observation_id) = observation["id"].as_str() else {
            continue;
        };
        if !observation_ids.insert(observation_id.to_string()) {
            issues.push(decision_input_issue(
                "decision_input_signal_observation_id_duplicate",
                format!("{path}/id"),
                "signal observation IDs must be unique",
            ));
            continue;
        }
        let qualified = observation["qualified_projection_id"]
            .as_str()
            .unwrap_or_default();
        let Some((contract, projection)) = projection_index.get(qualified).copied() else {
            issues.push(decision_input_issue(
                "decision_input_signal_projection_unknown",
                format!("{path}/qualified_projection_id"),
                "signal observation references an undeclared compiled projection",
            ));
            continue;
        };
        let expected_contract_id = contract["id"].as_str().unwrap_or_default();
        let expected_projection_id = projection["id"].as_str().unwrap_or_default();
        if observation["contract_id"] != expected_contract_id
            || observation["projection_id"] != expected_projection_id
        {
            issues.push(decision_input_issue(
                "decision_input_signal_projection_identity_mismatch",
                &path,
                "contract_id, projection_id, and qualified_projection_id must identify one compiled projection",
            ));
        }
        for (field, code) in [
            ("kind", "decision_input_signal_kind_mismatch"),
            ("roles", "decision_input_signal_roles_mismatch"),
            (
                "contributor_attribute_ids",
                "decision_input_signal_contributors_mismatch",
            ),
        ] {
            if observation[field] != projection[field] {
                issues.push(decision_input_issue(
                    code,
                    format!("{path}/{field}"),
                    format!("signal observation {field} must exactly echo pack-owned projection authority"),
                ));
            }
        }
        let Some(mapping) = binding_index.get(qualified).copied() else {
            issues.push(decision_input_issue(
                "decision_input_signal_binding_projection_missing",
                format!("{binding_path}#/projection_bindings"),
                "source binding omits the observation's compiled projection",
            ));
            continue;
        };
        if mapping["contributor_attribute_ids"] != observation["contributor_attribute_ids"] {
            issues.push(decision_input_issue(
                "decision_input_signal_binding_contributors_mismatch",
                format!("{path}/contributor_attribute_ids"),
                "observation contributors must exactly match the source-binding mapping",
            ));
        }
        if mapping["source"]["source_class"] != observation["source_class"] {
            issues.push(decision_input_issue(
                "decision_input_signal_source_class_mismatch",
                format!("{path}/source_class"),
                "observation source class must exactly match its source-binding mapping",
            ));
        }
        for (field, expected, code) in [
            (
                "source_binding_sha256",
                Some(binding_sha256),
                "decision_input_signal_source_binding_hash_mismatch",
            ),
            (
                "source_attempt_request_sha256",
                request_sha256,
                "decision_input_signal_request_hash_mismatch",
            ),
            (
                "collected_results_sha256",
                results_sha256,
                "decision_input_signal_results_hash_mismatch",
            ),
        ] {
            if observation["receipt"][field].as_str() != expected {
                issues.push(decision_input_issue(
                    code,
                    format!("{path}/receipt/{field}"),
                    format!("signal observation {field} must bind the exact supplied artifact"),
                ));
            }
        }
        if !safe_signal_value(&observation["value"]) {
            issues.push(decision_input_issue(
                "decision_input_signal_value_unsafe",
                format!("{path}/value"),
                "signal values must be bounded and contain no control characters",
            ));
        }
        let value_contract = serde_json::from_value::<ValueContract>(projection["value"].clone())
            .unwrap_or_default();
        let contributor_ids = observation["contributor_attribute_ids"]
            .as_array()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let mut observation_supported = true;
        for attempt_id in observation["attempt_ids"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if let Some(previous) =
                attempt_projection_use.insert(attempt_id.to_string(), qualified.to_string())
                && previous != qualified
            {
                issues.push(decision_input_issue(
                    "decision_input_signal_attempt_reuse_undeclared",
                    format!("{path}/attempt_ids"),
                    "one collected attempt cannot support multiple projections without an authored reuse declaration",
                ));
                observation_supported = false;
            }
            let Some(result) = attempt_results.get(attempt_id).copied() else {
                issues.push(decision_input_issue(
                    "decision_input_signal_attempt_unknown",
                    format!("{path}/attempt_ids"),
                    "signal observation references an attempt absent from collected attempt results",
                ));
                observation_supported = false;
                continue;
            };
            if result["status"] != "observed" {
                issues.push(decision_input_issue(
                    "decision_input_signal_attempt_ineligible_status",
                    format!("{path}/attempt_ids"),
                    "only observed collected attempts may support a first-class signal",
                ));
                observation_supported = false;
            }
            if result["decision_input_contract_id"] != observation["contract_id"]
                || !contributor_ids.contains(&&result["attribute_id"])
            {
                issues.push(decision_input_issue(
                    "decision_input_signal_attempt_contributor_mismatch",
                    format!("{path}/attempt_ids"),
                    "collected attempt does not belong to the declared contract and contributor set",
                ));
                observation_supported = false;
            }
            if !canonical_values_equal(&value_contract, &result["value"], &observation["value"]) {
                issues.push(decision_input_issue(
                    "decision_input_signal_value_mismatch",
                    format!("{path}/value"),
                    "signal value must equal its collected attempt value under the projection's typed canonical equality",
                ));
                observation_supported = false;
            }
            for (field, code) in [
                (
                    "source_class",
                    "decision_input_signal_source_class_mismatch",
                ),
                (
                    "source_locator",
                    "decision_input_signal_source_locator_mismatch",
                ),
                ("observed_at", "decision_input_signal_observed_at_mismatch"),
                ("confidence", "decision_input_signal_confidence_mismatch"),
            ] {
                if result[field] != observation[field] {
                    issues.push(decision_input_issue(
                        code,
                        format!("{path}/{field}"),
                        format!("signal {field} must exactly match its collected attempt result"),
                    ));
                    observation_supported = false;
                }
            }
        }
        if observation_supported {
            valid_observations.push(observation.clone());
        }
    }

    valid_observations.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let mut grouped = BTreeMap::<String, Vec<Value>>::new();
    for observation in &valid_observations {
        grouped
            .entry(
                observation["qualified_projection_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            )
            .or_default()
            .push(observation.clone());
    }
    let mut logical_signals = Vec::new();
    let mut status = "lineage-validated";
    for (qualified, (_, projection)) in &projection_index {
        let observations = grouped.remove(qualified).unwrap_or_default();
        let value_contract = serde_json::from_value::<ValueContract>(projection["value"].clone())
            .unwrap_or_default();
        let mut values = Vec::<(Value, Vec<String>)>::new();
        for observation in &observations {
            let value = &observation["value"];
            let observation_id = observation["id"].as_str().unwrap_or_default().to_string();
            if let Some((_, ids)) = values
                .iter_mut()
                .find(|(existing, _)| canonical_values_equal(&value_contract, existing, value))
            {
                ids.push(observation_id);
            } else {
                values.push((value.clone(), vec![observation_id]));
            }
        }
        let conflict = values.len() > 1;
        if conflict {
            let any_disqualifies = projection["conflict_policy"] == "any-disqualifies"
                && projection["roles"]
                    .as_array()
                    .is_some_and(|roles| roles.contains(&json!("disqualifier")));
            if !any_disqualifies {
                status = "human-review";
                if output["outcome"] == "ready" {
                    issues.push(decision_input_issue(
                        "decision_input_signal_conflict_ready",
                        format!("{artifact_path}#/outcome"),
                        "ready output is forbidden while require-agreement observations conflict",
                    ));
                }
            } else if output["outcome"] != "disqualified" {
                issues.push(decision_input_issue(
                    "decision_input_signal_disqualifier_outcome_mismatch",
                    format!("{artifact_path}#/outcome"),
                    "any-disqualifies conflict may resolve only to a disqualified outcome",
                ));
            } else {
                status = "disqualified";
            }
        }
        let logical_count = values.len();
        let min = projection["cardinality"]["min"].as_u64().unwrap_or(0) as usize;
        let max = projection["cardinality"]["max"].as_u64().unwrap_or(0) as usize;
        if logical_count < min || logical_count > max {
            issues.push(decision_input_issue(
                "decision_input_signal_cardinality_mismatch",
                format!("{artifact_path}#/signal_observations"),
                format!("projection {qualified} has {logical_count} logical values outside [{min}, {max}]"),
            ));
        }
        for (_, observation_ids) in values {
            logical_signals.push(json!({
                "qualified_projection_id": qualified,
                "kind": projection["kind"],
                "roles": projection["roles"],
                "observation_ids": observation_ids
            }));
        }
    }
    logical_signals.sort_by(|left, right| {
        left["qualified_projection_id"]
            .as_str()
            .cmp(&right["qualified_projection_id"].as_str())
            .then_with(|| {
                left["observation_ids"]
                    .to_string()
                    .cmp(&right["observation_ids"].to_string())
            })
    });
    let safe_observations = valid_observations
        .iter()
        .map(|observation| {
            json!({
                "id": observation["id"],
                "qualified_projection_id": observation["qualified_projection_id"],
                "kind": observation["kind"],
                "roles": observation["roles"],
                "contributor_attribute_ids": observation["contributor_attribute_ids"],
                "attempt_ids": observation["attempt_ids"],
                "source_class": observation["source_class"],
                "observed_at": observation["observed_at"],
                "confidence": observation["confidence"],
                "receipt": observation["receipt"]
            })
        })
        .collect::<Vec<_>>();
    let mut receipt = json!({
        "contract": "mdp.signal-projection-decision-receipt.v1",
        "status": if issues.len() == initial_issue_count { status } else { "blocked" },
        "draft_allowed": false,
        "source_binding_sha256": binding_sha256,
        "source_attempt_request_sha256": request_sha256,
        "collected_attempt_results_sha256": results_sha256,
        "observations": safe_observations,
        "logical_signals": logical_signals,
        "trust_boundary": "lineage identity only; does not attest host authenticity or source truth"
    });
    if let Some(sha256) = normalized_output_sha256 {
        receipt["normalized_output_sha256"] = json!(sha256);
    }
    Some(receipt)
}

fn safe_signal_value(value: &Value) -> bool {
    match value {
        Value::String(value) => {
            value.len() <= 4096 && !value.chars().any(|character| character.is_control())
        }
        Value::Number(_) | Value::Bool(_) => true,
        _ => false,
    }
}

fn validate_no_unbound_prospect_fields(
    compiled: &Value,
    output: &Value,
    artifact_path: &str,
    issues: &mut Vec<Value>,
) {
    let output_paths = compiled["decision_input_contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|contract| contract["attributes"].as_array().into_iter().flatten())
        .filter_map(|attribute| attribute["output_path"].as_str())
        .collect::<BTreeSet<_>>();
    let Some(prospect) = output["normalized_prospect"].as_object() else {
        return;
    };
    let allowed_unbound = compiled["normalized_prospect_unbound_policy"]["allowed_fields"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for (field, value) in prospect {
        if allowed_unbound.contains(field.as_str()) {
            continue;
        }
        if field == "attributes" {
            if let Some(attributes) = value.as_object() {
                for (attribute, value) in attributes {
                    let path = format!("attributes.{attribute}");
                    if !output_paths.contains(path.as_str()) && meaningful_projected_value(value) {
                        issues.push(decision_input_issue(
                            "decision_input_unbound_prospect_field",
                            format!(
                                "{artifact_path}#/normalized_prospect/attributes/{attribute}"
                            ),
                            format!(
                                "normalized prospect field {path} is not backed by a declared decision-input output_path"
                            ),
                        ));
                    }
                }
            }
            continue;
        }
        let bound = output_paths.iter().any(|output_path| {
            *output_path == field || output_path.starts_with(&format!("{field}."))
        });
        if !bound && meaningful_projected_value(value) {
            issues.push(decision_input_issue(
                "decision_input_unbound_prospect_field",
                format!("{artifact_path}#/normalized_prospect/{field}"),
                format!(
                    "normalized prospect field {field} is not backed by a declared decision-input output_path"
                ),
            ));
        }
    }
}

fn validate_attribute_attempt_receipts(
    attribute: &Value,
    attempt: &Value,
    artifact_path: &str,
    source_attempt_index: Option<&SourceAttemptIndex<'_>>,
    issues: &mut Vec<Value>,
) {
    let Some(attribute_id) = attribute["id"].as_str() else {
        return;
    };
    let mut provenance_observed_at_seconds = Vec::new();
    if let Some(provenance) = attempt["provenance"].as_array() {
        for (index, receipt) in provenance.iter().enumerate() {
            let receipt_path =
                format!("{artifact_path}#/attributes/{attribute_id}/provenance/{index}");
            if let Some(source_attempt_index) = source_attempt_index {
                if let Some(attempt_id) = receipt["attempt_id"].as_str() {
                    if let Some(source_attempt) = source_attempt_index.attempts.get(attempt_id) {
                        if source_attempt["attribute_id"].as_str() != Some(attribute_id) {
                            issues.push(decision_input_issue(
                                "decision_input_provenance_attempt_attribute_mismatch",
                                format!("{receipt_path}/attempt_id"),
                                "provenance attempt_id belongs to a different decision-input attribute",
                            ));
                        }
                        if receipt.get("source_class").is_some()
                            && receipt["source_class"] != source_attempt["source_class"]
                        {
                            issues.push(decision_input_issue(
                                "decision_input_provenance_source_class_mismatch",
                                format!("{receipt_path}/source_class"),
                                "provenance source_class must match its bound source attempt",
                            ));
                        }
                        if receipt.get("source_locator").is_some()
                            && receipt["source_locator"] != source_attempt["source_locator"]
                        {
                            issues.push(decision_input_issue(
                                "decision_input_provenance_source_locator_mismatch",
                                format!("{receipt_path}/source_locator"),
                                "provenance source_locator must match its bound source attempt",
                            ));
                        }
                    } else {
                        issues.push(decision_input_issue(
                            "decision_input_provenance_attempt_unknown",
                            format!("{receipt_path}/attempt_id"),
                            "provenance attempt_id is not present in the supplied source-attempt request",
                        ));
                    }
                }
            }
            if let Some(observed_at) = receipt["observed_at"].as_str() {
                let Some(observed_at_seconds) = parse_utc_timestamp_seconds(observed_at) else {
                    issues.push(decision_input_issue(
                        "decision_input_provenance_observed_at_invalid",
                        format!("{receipt_path}/observed_at"),
                        "provenance observed_at must be a valid UTC timestamp",
                    ));
                    continue;
                };
                provenance_observed_at_seconds.push(observed_at_seconds);
                if let Some(source_attempt_index) = source_attempt_index {
                    if observed_at_seconds > source_attempt_index.as_of_seconds {
                        issues.push(decision_input_issue(
                            "decision_input_provenance_observed_at_future",
                            format!("{receipt_path}/observed_at"),
                            "provenance observed_at must not be later than the trusted as_of timestamp",
                        ));
                    }
                }
            }
        }
    }
    let Some(freshness) = attempt["freshness"].as_object() else {
        return;
    };
    let Some(observed_at) = freshness.get("observed_at").and_then(Value::as_str) else {
        return;
    };
    let freshness_path = format!("{artifact_path}#/attributes/{attribute_id}/freshness");
    let Some(observed_at_seconds) = parse_utc_timestamp_seconds(observed_at) else {
        issues.push(decision_input_issue(
            "decision_input_freshness_observed_at_invalid",
            format!("{freshness_path}/observed_at"),
            "freshness observed_at must be a valid UTC timestamp",
        ));
        return;
    };
    let expected_freshness_seconds = provenance_observed_at_seconds.into_iter().max();
    let Some(source_attempt_index) = source_attempt_index else {
        return;
    };
    if observed_at_seconds > source_attempt_index.as_of_seconds {
        issues.push(decision_input_issue(
            "decision_input_freshness_observed_at_future",
            format!("{freshness_path}/observed_at"),
            "freshness observed_at must not be later than the trusted as_of timestamp",
        ));
        return;
    }
    if let Some(expected_freshness_seconds) = expected_freshness_seconds {
        if observed_at_seconds != expected_freshness_seconds {
            issues.push(decision_input_issue(
                "decision_input_freshness_provenance_timestamp_mismatch",
                format!("{freshness_path}/observed_at"),
                "freshness observed_at must equal the latest bound provenance observed_at timestamp",
            ));
            return;
        }
    }
    let derived_age = ((source_attempt_index.as_of_seconds - observed_at_seconds) / 86_400) as u64;
    if let Some(max_age_days) = attribute["freshness"]["max_age_days"].as_u64() {
        if derived_age > max_age_days {
            issues.push(decision_input_issue(
                "decision_input_freshness_age_over_limit",
                format!("{freshness_path}/observed_at"),
                "freshness observed_at is older than the maximum age allowed by the compiled decision-input contract",
            ));
        }
    }
    if let Some(age_days) = freshness.get("age_days").and_then(Value::as_u64) {
        if age_days != derived_age {
            issues.push(decision_input_issue(
                "decision_input_freshness_age_mismatch",
                format!("{freshness_path}/age_days"),
                "freshness age_days must equal the age derived from observed_at and the trusted source-attempt as_of timestamp",
            ));
        }
    }
}

fn meaningful_projected_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(value) => {
            let value = value.trim();
            !value.is_empty() && !value.eq_ignore_ascii_case("n/a")
        }
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        _ => true,
    }
}

fn parse_utc_timestamp_seconds(value: &str) -> Option<i64> {
    if !value.is_ascii()
        || value.len() != 20
        || &value[4..5] != "-"
        || &value[7..8] != "-"
        || &value[10..11] != "T"
        || &value[13..14] != ":"
        || &value[16..17] != ":"
        || !value.ends_with('Z')
    {
        return None;
    }
    let date = &value[..10];
    if !valid_date(date) {
        return None;
    }
    let year = value[..4].parse::<i64>().ok()?;
    let month = value[5..7].parse::<i64>().ok()?;
    let day = value[8..10].parse::<i64>().ok()?;
    let hour = value[11..13].parse::<i64>().ok()?;
    let minute = value[14..16].parse::<i64>().ok()?;
    let second = value[17..19].parse::<i64>().ok()?;
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let adjusted_year = year - i64::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn value_at_output_path<'a>(prospect: &'a Value, output_path: &str) -> Option<&'a Value> {
    output_path
        .split('.')
        .try_fold(prospect, |current, segment| current.get(segment))
}

fn decision_input_issue(code: &str, path: impl Into<String>, message: impl Into<String>) -> Value {
    json!({
        "code": code,
        "severity": "error",
        "path": path.into(),
        "message": message.into()
    })
}

fn pack_summary(manifest: &Manifest, sha256: &str) -> Value {
    json!({
        "id": &manifest.id,
        "name": &manifest.name,
        "version": &manifest.version,
        "format": &manifest.format,
        "sha256": sha256
    })
}

fn compile_model_task(root: &Path, job: &ProfileJob) -> Value {
    let Some(binding) = job.model_task.as_ref() else {
        return Value::Null;
    };
    let resolved = match read_canonical_prompt_by_id(root, &binding.prompt) {
        Ok(resolved) => resolved,
        Err(error) => {
            return json!({
                "status": "blocked",
                "kind": binding.kind,
                "prompt": binding.prompt,
                "diagnostics": [{
                    "code": "profile_job_model_task_prompt_read_failed",
                    "severity": "error",
                    "message": error.to_string()
                }]
            });
        }
    };
    let Some((path, prompt)) = resolved else {
        return json!({
            "status": "blocked",
            "kind": binding.kind,
            "prompt": binding.prompt,
            "diagnostics": [{
                "code": "profile_job_model_task_prompt_missing",
                "severity": "error",
                "message": "declared model-task prompt is missing"
            }]
        });
    };
    let prompt_value = match serde_json::to_value(&prompt) {
        Ok(value) => value,
        Err(error) => {
            return json!({
                "status": "blocked",
                "kind": binding.kind,
                "prompt": binding.prompt,
                "diagnostics": [{
                    "code": "profile_job_model_task_prompt_compile_failed",
                    "severity": "error",
                    "message": error.to_string()
                }]
            });
        }
    };
    let prompt_sha256 = match canonical_json_sha256(&prompt_value) {
        Ok(sha256) => sha256,
        Err(error) => {
            return json!({
                "status": "blocked",
                "kind": binding.kind,
                "prompt": binding.prompt,
                "diagnostics": [{
                    "code": "profile_job_model_task_prompt_hash_failed",
                    "severity": "error",
                    "message": error.to_string()
                }]
            });
        }
    };
    json!({
        "status": "ready",
        "kind": binding.kind,
        "prompt_id": prompt.id,
        "prompt_version": prompt.version,
        "prompt_path": path.strip_prefix(root).unwrap_or(&path).display().to_string(),
        "prompt_sha256": prompt_sha256,
        "context_budget": job.context_budget,
        "routed_context_required": prompt.inputs.iter().any(|input| input.required && input.name == "routed_context"),
        "declared_inputs": prompt.inputs,
        "instructions": {
            "instructions": prompt.instructions,
            "role": prompt.role,
            "objective": prompt.objective,
            "procedure": prompt.procedure,
            "selection_rules": prompt.selection_rules,
            "ambiguity_policy": prompt.ambiguity_policy,
            "provenance_policy": prompt.provenance_policy,
            "evidence_policy": prompt.evidence_policy,
            "negative_examples": prompt.negative_examples,
            "final_checklist": prompt.final_checklist
        },
        "output_contract": prompt.output_contract,
        "host_boundary": {
            "executor": "customer-selected-host",
            "mdp_role": "compile-and-validate",
            "model_call_included": false
        }
    })
}

fn finalize_requirements(mut value: Value) -> Result<Value> {
    if value["model_task"].is_null()
        && let Some(object) = value.as_object_mut()
    {
        object.remove("model_task");
    }
    let sha256 = canonical_json_sha256(&value)?;
    value["requirements_sha256"] = json!(sha256);
    Ok(value)
}

fn compile_contract(contract: &DecisionInputContract) -> Value {
    let attributes = contract
        .attributes
        .iter()
        .map(|attribute| {
            let mut value =
                serde_json::to_value(attribute).expect("decision input attribute should serialize");
            value["status_behavior"] = serde_json::to_value(effective_status_behavior(attribute))
                .expect("status behavior should serialize");
            value
        })
        .collect::<Vec<_>>();
    let source_classes = contract
        .source_classes
        .iter()
        .map(|source_class| {
            json!({
                "id": source_class,
                "public_research_allowed": matches!(source_class, DecisionInputSourceClass::PublicWeb),
                "requires_customer_or_operator_access": matches!(
                    source_class,
                    DecisionInputSourceClass::CustomerSystem | DecisionInputSourceClass::ReviewedInternal
                )
            })
        })
        .collect::<Vec<_>>();
    let mut compiled = json!({
        "id": &contract.id,
        "version": &contract.version,
        "description": &contract.description,
        "normalization": &contract.normalization,
        "source_classes": source_classes,
        "attempt_statuses": DecisionInputAttemptStatus::ALL,
        "attributes": attributes
    });
    if !contract.signal_projections.is_empty() {
        compiled["signal_projections"] = serde_json::to_value(&contract.signal_projections)
            .expect("signal projections should serialize");
    }
    compiled
}

fn signal_aware(contracts: &[&DecisionInputContract]) -> bool {
    contracts
        .iter()
        .any(|contract| !contract.signal_projections.is_empty())
}

fn effective_status_behavior(
    attribute: &DecisionInputAttribute,
) -> BTreeMap<DecisionInputAttemptStatus, DecisionInputDisposition> {
    let mut behavior = match attribute.requirement {
        DecisionInputRequirement::Required => BTreeMap::from([
            (
                DecisionInputAttemptStatus::Observed,
                DecisionInputDisposition::Accept,
            ),
            (
                DecisionInputAttemptStatus::NotFound,
                DecisionInputDisposition::Gap,
            ),
            (
                DecisionInputAttemptStatus::NotApplicable,
                DecisionInputDisposition::Gap,
            ),
            (
                DecisionInputAttemptStatus::Blocked,
                DecisionInputDisposition::HumanReview,
            ),
            (
                DecisionInputAttemptStatus::Error,
                DecisionInputDisposition::HumanReview,
            ),
        ]),
        DecisionInputRequirement::Optional => BTreeMap::from([
            (
                DecisionInputAttemptStatus::Observed,
                DecisionInputDisposition::Accept,
            ),
            (
                DecisionInputAttemptStatus::NotFound,
                DecisionInputDisposition::Accept,
            ),
            (
                DecisionInputAttemptStatus::NotApplicable,
                DecisionInputDisposition::Accept,
            ),
            (
                DecisionInputAttemptStatus::Blocked,
                DecisionInputDisposition::HumanReview,
            ),
            (
                DecisionInputAttemptStatus::Error,
                DecisionInputDisposition::HumanReview,
            ),
        ]),
        DecisionInputRequirement::Conditional => BTreeMap::from([
            (
                DecisionInputAttemptStatus::Observed,
                DecisionInputDisposition::Accept,
            ),
            (
                DecisionInputAttemptStatus::NotFound,
                DecisionInputDisposition::Gap,
            ),
            (
                DecisionInputAttemptStatus::NotApplicable,
                DecisionInputDisposition::Accept,
            ),
            (
                DecisionInputAttemptStatus::Blocked,
                DecisionInputDisposition::HumanReview,
            ),
            (
                DecisionInputAttemptStatus::Error,
                DecisionInputDisposition::HumanReview,
            ),
        ]),
        DecisionInputRequirement::HardGate => BTreeMap::new(),
    };
    behavior.extend(attribute.status_behavior.clone());
    let fail_closed_statuses = match attribute.requirement {
        DecisionInputRequirement::Required => vec![
            DecisionInputAttemptStatus::NotFound,
            DecisionInputAttemptStatus::NotApplicable,
            DecisionInputAttemptStatus::Blocked,
            DecisionInputAttemptStatus::Error,
        ],
        DecisionInputRequirement::Conditional => vec![
            DecisionInputAttemptStatus::NotFound,
            DecisionInputAttemptStatus::Blocked,
            DecisionInputAttemptStatus::Error,
        ],
        DecisionInputRequirement::Optional => vec![
            DecisionInputAttemptStatus::Blocked,
            DecisionInputAttemptStatus::Error,
        ],
        DecisionInputRequirement::HardGate => Vec::new(),
    };
    for status in fail_closed_statuses {
        let disposition = behavior
            .get(&status)
            .expect("every non-hard-gate status has a compiled disposition");
        let permits_ready = matches!(
            disposition,
            DecisionInputDisposition::Accept | DecisionInputDisposition::Evaluate
        ) || (attribute.requirement == DecisionInputRequirement::Optional
            && *disposition == DecisionInputDisposition::Gap);
        if permits_ready {
            behavior.insert(
                status,
                match status {
                    DecisionInputAttemptStatus::Blocked | DecisionInputAttemptStatus::Error => {
                        DecisionInputDisposition::HumanReview
                    }
                    DecisionInputAttemptStatus::NotFound
                    | DecisionInputAttemptStatus::NotApplicable => DecisionInputDisposition::Gap,
                    DecisionInputAttemptStatus::Observed => unreachable!(
                        "observed values are never clamped by fail-closed status policy"
                    ),
                },
            );
        }
    }
    behavior
}

fn source_attempt_request_schema(job_id: &str, contracts: &[&DecisionInputContract]) -> Value {
    if signal_aware(contracts) {
        return source_attempt_request_schema_v2(job_id, contracts);
    }
    source_attempt_request_schema_v1(job_id, contracts)
}

fn source_attempt_request_schema_v1(job_id: &str, contracts: &[&DecisionInputContract]) -> Value {
    let attribute_ids = contracts
        .iter()
        .flat_map(|contract| {
            contract
                .attributes
                .iter()
                .map(|attribute| attribute.id.clone())
        })
        .collect::<Vec<_>>();
    let attempt_item_variants = contracts
        .iter()
        .flat_map(|contract| contract.attributes.iter())
        .map(|attribute| {
            json!({
                "type": "object",
                "required": [
                    "attempt_id",
                    "attribute_id",
                    "source_class",
                    "source_locator",
                    "requested_at"
                ],
                "additionalProperties": false,
                "properties": {
                    "attempt_id": {"type": "string", "pattern": "\\S"},
                    "attribute_id": {"const": &attribute.id},
                    "source_class": {"enum": &attribute.source_classes},
                    "source_locator": {"type": "string", "pattern": "\\S"},
                    "requested_at": {
                        "type": "string",
                        "format": "date-time",
                        "pattern": "^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}Z$"
                    }
                }
            })
        })
        .collect::<Vec<_>>();
    let contract_versions = contracts
        .iter()
        .map(|contract| {
            json!({
                "id": contract.id,
                "version": contract.version
            })
        })
        .collect::<Vec<_>>();
    let attempted_complete = attribute_ids
        .iter()
        .map(|attribute_id| {
            json!({
                "contains": {
                    "type": "object",
                    "required": ["attribute_id"],
                    "properties": {
                        "attribute_id": {"const": attribute_id}
                    }
                },
                "minContains": 1
            })
        })
        .collect::<Vec<_>>();
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Source Attempt Request v1",
        "type": "object",
        "required": ["contract", "job_id", "decision_input_contracts", "as_of", "attempts"],
        "additionalProperties": false,
        "properties": {
            "contract": {"const": "mdp.source-attempt-request.v1"},
            "job_id": {"const": job_id},
            "decision_input_contracts": {
                "type": "array",
                "const": contract_versions
            },
            "as_of": {
                "type": "string",
                "format": "date-time",
                "pattern": "^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}Z$",
                "description": "Trusted UTC timestamp supplied by the host and used for deterministic freshness checks."
            },
            "attempts": {
                "type": "array",
                "minItems": attribute_ids.len(),
                "allOf": attempted_complete,
                "items": {
                    "oneOf": attempt_item_variants
                }
            }
        }
    })
}

fn source_attempt_request_schema_v2(job_id: &str, contracts: &[&DecisionInputContract]) -> Value {
    let mut schema = source_attempt_request_schema_v1(job_id, contracts);
    schema["title"] = json!("MDP Source Attempt Request v2");
    schema["properties"]["contract"]["const"] = json!(SOURCE_ATTEMPT_REQUEST_CONTRACT_V2);
    schema["required"]
        .as_array_mut()
        .expect("required should be an array")
        .insert(3, json!("source_binding_sha256"));
    schema["properties"]["source_binding_sha256"] = sha256_schema(
        "SHA-256 of the exact validated mdp.source-binding.v2 artifact used to create this request.",
    );
    let qualified_attributes = contracts
        .iter()
        .flat_map(|contract| {
            contract
                .attributes
                .iter()
                .map(|attribute| (contract.id.as_str(), attribute.id.as_str()))
        })
        .collect::<Vec<_>>();
    for (variant, (contract_id, _)) in schema["properties"]["attempts"]["items"]["oneOf"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .zip(qualified_attributes.iter())
    {
        variant["required"]
            .as_array_mut()
            .expect("attempt required should be an array")
            .insert(1, json!("decision_input_contract_id"));
        variant["properties"]["decision_input_contract_id"] = json!({"const": *contract_id});
    }
    schema["properties"]["attempts"]["allOf"] = json!(
        qualified_attributes
            .iter()
            .map(|(contract_id, attribute_id)| json!({
                "contains": {
                    "type": "object",
                    "required": ["decision_input_contract_id", "attribute_id"],
                    "properties": {
                        "decision_input_contract_id": {"const": contract_id},
                        "attribute_id": {"const": attribute_id}
                    }
                },
                "minContains": 1
            }))
            .collect::<Vec<_>>()
    );
    schema
}

fn collected_attempt_results_schema(job_id: &str, contracts: &[&DecisionInputContract]) -> Value {
    if signal_aware(contracts) {
        let mut schema = collected_attempt_results_schema_v1(job_id, contracts);
        schema["title"] = json!("MDP Collected Attempt Results v2");
        schema["properties"]["contract"]["const"] = json!(COLLECTED_ATTEMPT_RESULTS_CONTRACT_V2);
        schema["required"]
            .as_array_mut()
            .expect("required should be an array")
            .insert(3, json!("source_binding_sha256"));
        schema["properties"]["source_binding_sha256"] = sha256_schema(
            "SHA-256 of the exact source binding already bound by the source-attempt request.",
        );
        schema["required"]
            .as_array_mut()
            .expect("required should be an array")
            .push(json!("attempt_results"));
        let variants = contracts
            .iter()
            .flat_map(|contract| {
                contract.attributes.iter().map(move |attribute| {
                    let mut variant = attempt_result_schema(attribute);
                    let object = variant
                        .as_object_mut()
                        .expect("attempt result schema object");
                    let required = object["required"]
                        .as_array_mut()
                        .expect("attempt result required array");
                    for field in [
                        "attempt_id",
                        "decision_input_contract_id",
                        "attribute_id",
                        "source_class",
                        "source_locator",
                        "observed_at",
                    ] {
                        required.push(json!(field));
                    }
                    object["properties"]["attempt_id"] = signal_identifier_schema();
                    object["properties"]["decision_input_contract_id"] =
                        json!({"const": contract.id});
                    object["properties"]["attribute_id"] = json!({"const": attribute.id});
                    object["properties"]["source_class"] =
                        json!({"enum": attribute.source_classes});
                    object["properties"]["source_locator"] = signal_locator_schema();
                    object["properties"]["observed_at"] = json!({
                        "type": "string", "format": "date-time", "maxLength": 64,
                        "pattern": "^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}Z$"
                    });
                    variant
                })
            })
            .collect::<Vec<_>>();
        schema["properties"]["attempt_results"] = json!({
            "type": "array",
            "minItems": 1,
            "maxItems": crate::models::MAX_SIGNAL_OBSERVATIONS_PER_ENVELOPE,
            "items": {"oneOf": variants}
        });
        return schema;
    }
    collected_attempt_results_schema_v1(job_id, contracts)
}

fn signal_identifier_schema() -> Value {
    json!({
        "type": "string", "minLength": 1,
        "maxLength": crate::models::MAX_SIGNAL_IDENTIFIER_LEN,
        "pattern": "^[A-Za-z0-9][A-Za-z0-9._:-]*$"
    })
}

fn signal_locator_schema() -> Value {
    json!({
        "type": "string", "minLength": 1,
        "maxLength": crate::models::MAX_SIGNAL_LOCATOR_LEN,
        "pattern": "^[^\\u0000-\\u001F\\u007F]+$",
        "not": {"pattern": "^[A-Za-z][A-Za-z0-9+.-]*://"}
    })
}

fn collected_attempt_results_schema_v1(
    job_id: &str,
    contracts: &[&DecisionInputContract],
) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for contract in contracts {
        for attribute in &contract.attributes {
            properties.insert(
                attribute.id.clone(),
                collected_attempt_result_schema(attribute),
            );
            required.push(Value::String(attribute.id.clone()));
        }
    }
    let contract_versions = contracts
        .iter()
        .map(|contract| {
            json!({
                "id": contract.id,
                "version": contract.version
            })
        })
        .collect::<Vec<_>>();
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Collected Attempt Results v1",
        "type": "object",
        "required": [
            "contract",
            "job_id",
            "decision_input_contracts",
            "source_attempt_request_sha256",
            "attributes"
        ],
        "additionalProperties": false,
        "properties": {
            "contract": {"const": COLLECTED_ATTEMPT_RESULTS_CONTRACT},
            "job_id": {"const": job_id},
            "decision_input_contracts": {
                "type": "array",
                "const": contract_versions
            },
            "source_attempt_request_sha256": {
                "type": "string",
                "pattern": "^[a-f0-9]{64}$"
            },
            "attributes": {
                "type": "object",
                "required": required,
                "additionalProperties": false,
                "properties": properties
            }
        }
    })
}

fn collected_attempt_result_schema(attribute: &DecisionInputAttribute) -> Value {
    let mut schema = attempt_result_schema(attribute);
    schema["properties"]["value"] = json!({
        "description": "Raw host-collected value. The bound normalizer may canonicalize it only into the attribute's declared normalized value contract."
    });
    schema
}

fn normalized_envelope_schema(job_id: &str, contracts: &[&DecisionInputContract]) -> Value {
    if signal_aware(contracts) {
        let mut schema = normalized_envelope_schema_v1(job_id, contracts);
        schema["title"] = json!("MDP Normalized Decision Input v2");
        schema["properties"]["contract"]["const"] = json!(NORMALIZED_DECISION_INPUT_CONTRACT_V2);
        schema["required"]
            .as_array_mut()
            .expect("required should be an array")
            .insert(4, json!("source_binding_sha256"));
        schema["required"]
            .as_array_mut()
            .expect("required should be an array")
            .push(json!("signal_observations"));
        schema["properties"]["source_binding_sha256"] = sha256_schema(
            "SHA-256 of the exact source binding bound through request and collected results.",
        );
        let mut observations = signal_observation_v2_schema();
        observations
            .as_object_mut()
            .expect("observation schema object")
            .remove("$schema");
        observations
            .as_object_mut()
            .expect("observation schema object")
            .remove("title");
        schema["properties"]["signal_observations"] = json!({
            "type": "array",
            "maxItems": crate::models::MAX_SIGNAL_OBSERVATIONS_PER_ENVELOPE,
            "items": observations
        });
        return schema;
    }
    normalized_envelope_schema_v1(job_id, contracts)
}

fn normalized_envelope_schema_v1(job_id: &str, contracts: &[&DecisionInputContract]) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    let mut ready_outcome_guards = Vec::new();
    for contract in contracts {
        let attribute_domains = contract
            .attributes
            .iter()
            .map(|attribute| {
                (
                    attribute.id.as_str(),
                    attribute
                        .value
                        .enum_values
                        .iter()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for attribute in &contract.attributes {
            properties.insert(attribute.id.clone(), attempt_result_schema(attribute));
            required.push(Value::String(attribute.id.clone()));
            if let Some(guard) = ready_outcome_guard(attribute) {
                ready_outcome_guards.push(guard);
            }
            if let Some(guard) = applies_when_ready_outcome_guard(attribute) {
                ready_outcome_guards.push(guard);
            }
            if let Some(guard) = conditional_applicability_state_guard(attribute) {
                ready_outcome_guards.push(guard);
            }
            if attribute.applies_when.iter().any(|condition| {
                (condition.operator == DecisionInputConditionOperator::Exists
                    && !condition.values.is_empty())
                    || (condition.operator != DecisionInputConditionOperator::Exists
                        && attribute_domains
                            .get(condition.attribute.as_str())
                            .filter(|domain| !domain.is_empty())
                            .is_some_and(|domain| {
                                condition
                                    .values
                                    .iter()
                                    .any(|value| !domain.contains(value.as_str()))
                            }))
            }) {
                ready_outcome_guards.push(json!(false));
            }
        }
    }
    let normalization_receipts = contracts
        .iter()
        .map(|contract| {
            json!({
                "contract_id": contract.id,
                "prompt": contract.normalization.prompt,
                "prompt_version": contract.normalization.prompt_version
            })
        })
        .collect::<Vec<_>>();
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Normalized Decision Input v1",
        "type": "object",
        "required": [
            "contract",
            "job_id",
            "decision_input_contracts",
            "normalization",
            "source_attempt_request_sha256",
            "collected_attempt_results_sha256",
            "attributes",
            "normalized_prospect",
            "outcome",
            "draft_allowed"
        ],
        "additionalProperties": false,
        "allOf": ready_outcome_guards,
        "properties": {
            "contract": {"const": NORMALIZED_DECISION_INPUT_CONTRACT},
            "job_id": {"const": job_id},
            "decision_input_contracts": {
                "type": "array",
                "const": contracts.iter().map(|contract| contract.id.as_str()).collect::<Vec<_>>()
            },
            "normalization": {
                "type": "array",
                "const": normalization_receipts
            },
            "source_attempt_request_sha256": {
                "type": "string",
                "pattern": "^[a-f0-9]{64}$",
                "description": "SHA-256 of the exact source-attempt request file validated with this normalized output."
            },
            "collected_attempt_results_sha256": {
                "type": "string",
                "pattern": "^[a-f0-9]{64}$",
                "description": "SHA-256 of the exact collected attempt-results ledger validated with this normalized output."
            },
            "attributes": {
                "type": "object",
                "required": required,
                "additionalProperties": false,
                "properties": properties
            },
            "normalized_prospect": schema(SchemaTarget::Prospect),
            "outcome": {
                "enum": [
                    "ready",
                    "insufficient-context",
                    "disqualified",
                    "human-review",
                    "malformed",
                    "provider-error"
                ]
            },
            "draft_allowed": {"const": false, "description": "Normalization never drafts. A downstream copy step may run only after deterministic MDP evaluation returns ready."}
        }
    })
}

fn sha256_schema(description: &str) -> Value {
    json!({
        "type": "string",
        "pattern": "^[a-f0-9]{64}$",
        "description": description
    })
}

fn ready_outcome_guard(attribute: &DecisionInputAttribute) -> Option<Value> {
    let blocking_statuses = effective_status_behavior(attribute)
        .into_iter()
        .filter_map(|(status, disposition)| {
            let ready_permitted = match disposition {
                DecisionInputDisposition::Accept | DecisionInputDisposition::Evaluate => true,
                DecisionInputDisposition::Gap => {
                    attribute.requirement == DecisionInputRequirement::Optional
                }
                DecisionInputDisposition::Block
                | DecisionInputDisposition::Disqualify
                | DecisionInputDisposition::HumanReview => false,
            };
            (!ready_permitted).then_some(status)
        })
        .collect::<Vec<_>>();
    if blocking_statuses.is_empty() {
        return None;
    }

    let mut attribute_guard = Map::new();
    attribute_guard.insert(
        attribute.id.clone(),
        json!({
            "type": "object",
            "required": ["status"],
            "properties": {
                "status": {"enum": blocking_statuses}
            }
        }),
    );
    Some(json!({
        "if": {
            "required": ["attributes"],
            "properties": {
                "attributes": {
                    "type": "object",
                    "required": [&attribute.id],
                    "properties": attribute_guard
                }
            }
        },
        "then": {
            "properties": {
                "outcome": {"not": {"const": "ready"}}
            }
        }
    }))
}

fn applies_when_ready_outcome_guard(attribute: &DecisionInputAttribute) -> Option<Value> {
    if attribute.requirement != DecisionInputRequirement::Conditional
        || attribute.applies_when.is_empty()
    {
        return None;
    }

    let mut attribute_guards = attribute
        .applies_when
        .iter()
        .map(applies_when_condition_schema)
        .collect::<Vec<_>>();
    let mut not_applicable_guard = Map::new();
    not_applicable_guard.insert(
        attribute.id.clone(),
        json!({
            "type": "object",
            "required": ["status"],
            "properties": {
                "status": {"const": DecisionInputAttemptStatus::NotApplicable}
            }
        }),
    );
    attribute_guards.push(json!({
        "required": [&attribute.id],
        "properties": not_applicable_guard
    }));

    Some(json!({
        "if": {
            "required": ["attributes"],
            "properties": {
                "attributes": {
                    "type": "object",
                    "allOf": attribute_guards
                }
            }
        },
        "then": {
            "properties": {
                "outcome": {"not": {"const": "ready"}}
            }
        }
    }))
}

fn conditional_applicability_state_guard(attribute: &DecisionInputAttribute) -> Option<Value> {
    if attribute.requirement != DecisionInputRequirement::Conditional
        || attribute.applies_when.is_empty()
    {
        return None;
    }
    let conditions = attribute
        .applies_when
        .iter()
        .map(applies_when_condition_schema)
        .collect::<Vec<_>>();
    let target_status = |status: Value| {
        let mut properties = Map::new();
        properties.insert(
            attribute.id.clone(),
            json!({
                "type": "object",
                "required": ["status"],
                "properties": {"status": status}
            }),
        );
        json!({
            "type": "object",
            "required": [&attribute.id],
            "properties": properties
        })
    };
    Some(json!({
        "if": {
            "required": ["attributes"],
            "properties": {
                "attributes": {
                    "type": "object",
                    "allOf": conditions
                }
            }
        },
        "then": {
            "properties": {
                "attributes": target_status(json!({"not": {"const": DecisionInputAttemptStatus::NotApplicable}}))
            }
        },
        "else": {
            "properties": {
                "attributes": target_status(json!({"const": DecisionInputAttemptStatus::NotApplicable}))
            }
        }
    }))
}

fn applies_when_condition_schema(condition: &DecisionInputCondition) -> Value {
    let observed_status = json!({"const": DecisionInputAttemptStatus::Observed});
    let condition_state = match condition.operator {
        DecisionInputConditionOperator::Exists => json!({
            "type": "object",
            "required": ["status"],
            "properties": {
                "status": observed_status
            }
        }),
        DecisionInputConditionOperator::Equals => {
            let Some(value) = condition.values.first() else {
                return json!(false);
            };
            json!({
                "type": "object",
                "required": ["status", "value"],
                "properties": {
                    "status": observed_status,
                    "value": {"const": value}
                }
            })
        }
        DecisionInputConditionOperator::NotEquals => json!({
            "type": "object",
            "required": ["status", "value"],
            "properties": {
                "status": observed_status,
                "value": {"not": {"enum": &condition.values}}
            }
        }),
        DecisionInputConditionOperator::In => {
            if condition.values.is_empty() {
                return json!(false);
            }
            json!({
                "type": "object",
                "required": ["status", "value"],
                "properties": {
                    "status": observed_status,
                    "value": {"enum": &condition.values}
                }
            })
        }
    };

    let mut properties = Map::new();
    properties.insert(condition.attribute.clone(), condition_state);
    json!({
        "required": [&condition.attribute],
        "properties": properties
    })
}

fn attempt_result_schema(attribute: &DecisionInputAttribute) -> Value {
    let mut observed_required = vec!["value"];
    if attribute.provenance.required {
        observed_required.push("provenance");
    }
    if attribute.confidence.required {
        observed_required.push("confidence");
    }
    if attribute.freshness.required {
        observed_required.push("freshness");
    }
    let provenance_required = attribute
        .provenance
        .required_fields
        .iter()
        .map(|field| serde_json::to_value(field).expect("provenance field should serialize"))
        .collect::<Vec<_>>();
    let confidence_minimum = attribute.confidence.minimum.unwrap_or(0);
    let freshness_maximum = attribute.freshness.max_age_days.unwrap_or(u32::MAX);
    let freshness_required = if attribute.freshness.required {
        if attribute.freshness.allow_unknown {
            vec!["observed_at"]
        } else {
            vec!["observed_at", "age_days"]
        }
    } else {
        Vec::new()
    };

    json!({
        "type": "object",
        "required": ["status"],
        "additionalProperties": false,
        "properties": {
            "status": {"enum": DecisionInputAttemptStatus::ALL},
            "value": value_contract_json_schema(&attribute.value),
            "provenance": {
                "type": "array",
                "minItems": 0,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "attempt_id": {"type": "string", "pattern": "\\S"},
                        "source_class": {"enum": &attribute.source_classes},
                        "source_locator": {"type": "string"},
                        "observed_at": {
                            "type": "string",
                            "format": "date-time",
                            "pattern": "^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}Z$"
                        },
                        "excerpt": {"type": "string"}
                    }
                }
            },
            "confidence": {
                "type": "integer",
                "minimum": 0,
                "maximum": 100
            },
            "freshness": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "observed_at": {
                        "type": "string",
                        "format": "date-time",
                        "pattern": "^\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}Z$"
                    },
                    "age_days": {
                        "type": "integer",
                        "minimum": 0
                    }
                }
            },
            "error": {"type": "string", "pattern": "\\S"}
        },
        "allOf": [
            {
                "if": {
                    "required": ["status"],
                    "properties": {"status": {"const": "observed"}}
                },
                "then": {
                    "required": observed_required,
                    "not": {"required": ["error"]},
                    "properties": {
                        "provenance": {
                            "minItems": if attribute.provenance.required { 1 } else { 0 },
                            "items": {"required": provenance_required}
                        },
                        "confidence": {"minimum": confidence_minimum},
                        "freshness": {
                            "required": freshness_required,
                            "properties": {
                                "age_days": {"maximum": freshness_maximum}
                            }
                        }
                    }
                },
                "else": {"not": {"required": ["value"]}}
            },
            {
                "if": {
                    "required": ["status"],
                    "properties": {"status": {"const": "error"}}
                },
                "then": {"required": ["error"]},
                "else": {"not": {"required": ["error"]}}
            }
        ]
    })
}

fn value_contract_json_schema(contract: &ValueContract) -> Value {
    let mut schema = Map::new();
    schema.insert(
        "type".to_string(),
        Value::String(
            contract
                .value_type
                .clone()
                .unwrap_or_else(|| "string".to_string()),
        ),
    );
    if let Some(format) = &contract.format {
        schema.insert("format".to_string(), Value::String(format.clone()));
    }
    if !contract.enum_values.is_empty() {
        schema.insert(
            "enum".to_string(),
            Value::Array(
                contract
                    .enum_values
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
    if contract.value_type.as_deref().unwrap_or("string") == "string" {
        schema.insert("pattern".to_string(), Value::String("\\S".to_string()));
    }
    Value::Object(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use crate::commands::routing::fit_normalized;
    use crate::models::{
        DecisionInputConfidencePolicy, DecisionInputDecisionEffect, DecisionInputFreshnessPolicy,
        DecisionInputProvenanceField, DecisionInputProvenancePolicy, DecisionInputSensitivity,
        DecisionInputSignalCardinality, DecisionInputSignalConflictPolicy,
        DecisionInputSignalProjection, DecisionInputSignalRole, DecisionInputSourceClass,
        ValueContract,
    };
    use jsonschema::draft202012;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn clay_example_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("CLI crate should have a repository parent")
            .join("examples/clay-audiences-self-serve-enterprise-expansion")
    }

    fn signal_aware_contract() -> DecisionInputContract {
        let manifest = read_manifest(&clay_example_root()).expect("clay manifest should load");
        let mut contract = manifest.decision_input_contracts[0].clone();
        contract.signal_projections.clear();
        contract
            .signal_projections
            .push(DecisionInputSignalProjection {
                id: "buying-window".to_string(),
                kind: "profile_buying_window".to_string(),
                roles: vec![DecisionInputSignalRole::WhyNow],
                contributor_attribute_ids: vec!["last_meaningful_touch".to_string()],
                value: ValueContract {
                    value_type: Some("string".to_string()),
                    format: Some("date-time".to_string()),
                    ..ValueContract::default()
                },
                cardinality: DecisionInputSignalCardinality { min: 0, max: 4 },
                conflict_policy: DecisionInputSignalConflictPolicy::RequireAgreement,
                decision_effects: vec![
                    DecisionInputDecisionEffect::Brief,
                    DecisionInputDecisionEffect::NoDraft,
                ],
            });
        contract
    }

    fn signal_aware_clay_example(name: &str) -> PathBuf {
        let root = temporary_clay_example(name);
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should read");
        let mut manifest: serde_yaml::Value = serde_yaml::from_str(&raw).expect("manifest parses");
        manifest["decision_input_contracts"][0]["signal_projections"] =
            serde_yaml::Value::Sequence(Vec::new());
        manifest["decision_input_contracts"][0]["normalization"]["normalized_schema_ref"] =
            serde_yaml::Value::String("mdp.normalized-decision-input.v2".to_string());
        manifest["decision_input_contracts"][0]["signal_projections"] = serde_yaml::from_str(
            r#"
- id: buying-window
  kind: profile_buying_window
  roles: [why-now]
  contributor_attribute_ids: [last_meaningful_touch]
  value: {type: string, format: date-time}
  cardinality: {min: 0, max: 4}
  conflict_policy: require-agreement
  decision_effects: [brief, no-draft]
"#,
        )
        .expect("projection yaml parses");
        std::fs::write(&manifest_path, serde_yaml::to_string(&manifest).unwrap()).unwrap();
        let prompt_path = root.join(".mdp/prompts/normalize-prospect.yaml");
        let prompt = std::fs::read_to_string(&prompt_path)
            .expect("normalization prompt should read")
            .replace(
                "mdp.normalized-decision-input.v1",
                "mdp.normalized-decision-input.v2",
            );
        std::fs::write(prompt_path, prompt).unwrap();
        root
    }

    #[test]
    fn scalar_runtime_schema_characterization_stays_v1() {
        let root = scalar_clay_example("scalar-runtime-characterization");
        let compiled = requirements(&root, "prospect-fit-or-brief")
            .expect("scalar requirements should compile");

        assert_eq!(
            canonical_json_sha256(&compiled["source_attempt_request_schema"]).unwrap(),
            "81793f07da26d4e83a3dde7ab79dc1305d155092766422a18fef18eb14de883c"
        );
        assert_eq!(
            canonical_json_sha256(&compiled["collected_attempt_results_schema"]).unwrap(),
            "2adf1bc04d6f3edf1d108487cb80770439efe340023c3516d0cf8b094e7a19a4"
        );
        assert_eq!(
            canonical_json_sha256(&compiled["normalized_output_schema"]).unwrap(),
            "b1fcb6541565f6da0eeeb3a75dd4eeae9e6d328e980c9f8cc36170da09ab86ef"
        );
        assert!(compiled.get("runtime_contract_version").is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn signal_aware_runtime_schemas_bind_source_binding_and_exclude_output_self_hash() {
        let contract = signal_aware_contract();
        let contracts = vec![&contract];
        let request = source_attempt_request_schema("signal-job", &contracts);
        let results = collected_attempt_results_schema("signal-job", &contracts);
        let normalized = normalized_envelope_schema("signal-job", &contracts);

        assert_eq!(
            request["properties"]["contract"]["const"],
            "mdp.source-attempt-request.v2"
        );
        assert!(
            request["required"]
                .as_array()
                .unwrap()
                .contains(&json!("source_binding_sha256"))
        );
        assert!(
            results["required"]
                .as_array()
                .unwrap()
                .contains(&json!("source_binding_sha256"))
        );
        assert!(
            normalized["required"]
                .as_array()
                .unwrap()
                .contains(&json!("source_binding_sha256"))
        );
        assert_eq!(
            results["properties"]["contract"]["const"],
            "mdp.collected-attempt-results.v2"
        );
        assert_eq!(
            normalized["properties"]["contract"]["const"],
            "mdp.normalized-decision-input.v2"
        );
        assert!(
            normalized["properties"]
                .get("normalized_output_sha256")
                .is_none()
        );
        assert_eq!(
            normalized["properties"]["signal_observations"]["maxItems"],
            crate::models::MAX_SIGNAL_OBSERVATIONS_PER_ENVELOPE
        );
        assert!(
            results["required"]
                .as_array()
                .unwrap()
                .contains(&json!("attempt_results")),
            "v2 collected results need one record per attempt so conflicting observations can be proven"
        );
        assert_eq!(
            results["properties"]["attempt_results"]["maxItems"],
            crate::models::MAX_SIGNAL_OBSERVATIONS_PER_ENVELOPE
        );
    }

    #[test]
    fn signal_projection_validator_accepts_matching_receipts_and_preserves_conflicts() {
        let fixture = signal_projection_fixture("projection-valid");
        let validation = validate_normalized_decision_input_with_projection(
            &fixture.root,
            &fixture.output,
            "normalized.json",
            &fixture.prompt_path,
            Some((&fixture.binding, "binding.json", &fixture.binding_sha256)),
            Some((&fixture.request, "request.json", &fixture.request_sha256)),
            Some((&fixture.results, "results.json", &fixture.results_sha256)),
            Some("d".repeat(64).as_str()),
        )
        .expect("signal validation should run");

        assert!(validation.issues.is_empty(), "{:#?}", validation.issues);
        let receipt = validation
            .signal_projection
            .expect("v2 validation should emit a post-validation projection receipt");
        assert_eq!(receipt["status"], "lineage-validated");
        assert_eq!(receipt["logical_signals"].as_array().unwrap().len(), 1);
        assert_eq!(
            receipt["logical_signals"][0]["observation_ids"],
            json!(["obs-a", "obs-b"])
        );
        assert_eq!(receipt["observations"].as_array().unwrap().len(), 2);
        assert!(receipt["observations"][0].get("value").is_none());
        assert!(receipt["observations"][0].get("source_locator").is_none());
        assert!(receipt["logical_signals"][0].get("value").is_none());
        assert_eq!(receipt["normalized_output_sha256"], "d".repeat(64));
        assert!(fixture.output.get("normalized_output_sha256").is_none());

        let mut conflict_fixture = signal_projection_fixture("projection-conflict");
        conflict_fixture.results["attempt_results"][1]["value"] = json!("2026-07-21T12:00:00Z");
        conflict_fixture.results_sha256 = canonical_json_sha256(&conflict_fixture.results).unwrap();
        conflict_fixture.output["collected_attempt_results_sha256"] =
            json!(&conflict_fixture.results_sha256);
        for observation in conflict_fixture.output["signal_observations"]
            .as_array_mut()
            .unwrap()
        {
            observation["receipt"]["collected_results_sha256"] =
                json!(&conflict_fixture.results_sha256);
        }
        conflict_fixture.output["signal_observations"][0]["value"] = json!("2026-07-21T12:00:00Z");
        conflict_fixture.output["outcome"] = json!("human-review");
        let conflict_validation = validate_normalized_decision_input_with_projection(
            &conflict_fixture.root,
            &conflict_fixture.output,
            "normalized.json",
            &conflict_fixture.prompt_path,
            Some((
                &conflict_fixture.binding,
                "binding.json",
                &conflict_fixture.binding_sha256,
            )),
            Some((
                &conflict_fixture.request,
                "request.json",
                &conflict_fixture.request_sha256,
            )),
            Some((
                &conflict_fixture.results,
                "results.json",
                &conflict_fixture.results_sha256,
            )),
            None,
        )
        .expect("conflict validation should run");
        assert!(
            conflict_validation.issues.is_empty(),
            "{:#?}",
            conflict_validation.issues
        );
        let conflict_receipt = conflict_validation.signal_projection.unwrap();
        assert_eq!(conflict_receipt["status"], "human-review");
        assert_eq!(
            conflict_receipt["logical_signals"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            conflict_receipt["observations"].as_array().unwrap().len(),
            2
        );

        conflict_fixture.output["outcome"] = json!("ready");
        let ready_conflict = validate_normalized_decision_input_with_projection(
            &conflict_fixture.root,
            &conflict_fixture.output,
            "normalized.json",
            &conflict_fixture.prompt_path,
            Some((
                &conflict_fixture.binding,
                "binding.json",
                &conflict_fixture.binding_sha256,
            )),
            Some((
                &conflict_fixture.request,
                "request.json",
                &conflict_fixture.request_sha256,
            )),
            Some((
                &conflict_fixture.results,
                "results.json",
                &conflict_fixture.results_sha256,
            )),
            None,
        )
        .unwrap();
        assert!(
            ready_conflict
                .issues
                .iter()
                .any(|issue| issue["code"] == "decision_input_signal_conflict_ready")
        );
    }

    #[test]
    fn signal_projection_validator_blocks_absent_required_projection() {
        let mut fixture = signal_projection_fixture("projection-required-absent");
        let manifest_path = fixture.root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should read");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["decision_input_contracts"][0]["signal_projections"][0]["cardinality"]["min"] =
            serde_yaml::Value::Number(1.into());
        std::fs::write(&manifest_path, serde_yaml::to_string(&manifest).unwrap()).unwrap();
        fixture.output["signal_observations"] = json!([]);

        let validation = validate_normalized_decision_input_with_projection(
            &fixture.root,
            &fixture.output,
            "normalized.json",
            &fixture.prompt_path,
            Some((&fixture.binding, "binding.json", &fixture.binding_sha256)),
            Some((&fixture.request, "request.json", &fixture.request_sha256)),
            Some((&fixture.results, "results.json", &fixture.results_sha256)),
            None,
        )
        .expect("signal validation should run");

        assert!(validation.issues.iter().any(|issue| {
            issue["code"] == "decision_input_signal_cardinality_mismatch"
                && issue["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("has 0 logical values outside [1, 4]"))
        }));
        assert_eq!(validation.signal_projection.unwrap()["status"], "blocked");
        let _ = std::fs::remove_dir_all(fixture.root);
    }

    #[test]
    fn signal_projection_validator_fails_closed_on_independent_lineage_mismatches() {
        let fixture = signal_projection_fixture("projection-mismatch");
        for (label, field, expected) in [
            (
                "binding hash",
                "source_binding_sha256",
                "decision_input_signal_source_binding_hash_mismatch",
            ),
            (
                "request hash",
                "source_attempt_request_sha256",
                "decision_input_signal_request_hash_mismatch",
            ),
            (
                "results hash",
                "collected_results_sha256",
                "decision_input_signal_results_hash_mismatch",
            ),
        ] {
            let mut output = fixture.output.clone();
            output["signal_observations"][0]["receipt"][field] = json!("0".repeat(64));
            let validation = validate_normalized_decision_input_with_projection(
                &fixture.root,
                &output,
                "normalized.json",
                &fixture.prompt_path,
                Some((&fixture.binding, "binding.json", &fixture.binding_sha256)),
                Some((&fixture.request, "request.json", &fixture.request_sha256)),
                Some((&fixture.results, "results.json", &fixture.results_sha256)),
                None,
            )
            .expect("mismatch validation should run");
            assert!(
                validation
                    .issues
                    .iter()
                    .any(|issue| issue["code"] == expected),
                "{label} should emit {expected}: {:#?}",
                validation.issues
            );
        }
    }

    #[test]
    fn signal_projection_validator_binds_attempt_results_to_the_exact_request() {
        let mut fixture = signal_projection_fixture("projection-attempt-request-drift");
        fixture.results["attempt_results"][0]["source_locator"] = json!("opaque:drift");
        fixture.output["signal_observations"][1]["source_locator"] = json!("opaque:drift");
        fixture.results_sha256 = canonical_json_sha256(&fixture.results).unwrap();
        fixture.output["collected_attempt_results_sha256"] = json!(&fixture.results_sha256);
        for observation in fixture.output["signal_observations"]
            .as_array_mut()
            .unwrap()
        {
            observation["receipt"]["collected_results_sha256"] = json!(&fixture.results_sha256);
        }

        let validation = validate_normalized_decision_input_with_projection(
            &fixture.root,
            &fixture.output,
            "normalized.json",
            &fixture.prompt_path,
            Some((&fixture.binding, "binding.json", &fixture.binding_sha256)),
            Some((&fixture.request, "request.json", &fixture.request_sha256)),
            Some((&fixture.results, "results.json", &fixture.results_sha256)),
            None,
        )
        .expect("signal validation should run");

        assert!(
            validation.issues.iter().any(|issue| {
                issue["code"] == "decision_input_collected_attempt_request_mismatch"
                    && issue["path"] == "results.json#/attempt_results/0/source_locator"
            }),
            "{:#?}",
            validation.issues
        );
        assert_eq!(validation.signal_projection.unwrap()["status"], "blocked");
        let _ = std::fs::remove_dir_all(fixture.root);
    }

    struct SignalProjectionFixture {
        root: PathBuf,
        prompt_path: PathBuf,
        binding: Value,
        binding_sha256: String,
        request: Value,
        request_sha256: String,
        results: Value,
        results_sha256: String,
        output: Value,
    }

    fn signal_projection_fixture(name: &str) -> SignalProjectionFixture {
        signal_projection_fixture_from_root(signal_aware_clay_example(name))
    }

    fn signal_projection_fixture_with_legacy_signal_requirements(
        name: &str,
    ) -> SignalProjectionFixture {
        let root = signal_aware_clay_example(name);
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should read");
        let mut manifest: serde_yaml::Value = serde_yaml::from_str(&raw).expect("manifest parses");
        manifest["lead_input_requirements"]["required_fields"]
            .as_sequence_mut()
            .expect("required fields should be a sequence")
            .push(serde_yaml::Value::String("signals".to_string()));
        manifest["lead_input_requirements"]["required_signal_fields"]
            .as_sequence_mut()
            .expect("required signal fields should be a sequence")
            .push(serde_yaml::Value::String("source".to_string()));
        std::fs::write(&manifest_path, serde_yaml::to_string(&manifest).unwrap())
            .expect("manifest should be writable");
        signal_projection_fixture_from_root(root)
    }

    fn signal_projection_fixture_from_root(root: PathBuf) -> SignalProjectionFixture {
        let compiled = requirements(&root, "prospect-fit-or-brief").unwrap();
        let contract_id = compiled["decision_input_contracts"][0]["id"].clone();
        let contract_version = compiled["decision_input_contracts"][0]["version"].clone();
        let binding = json!({
            "contract": "mdp.source-binding.v2",
            "binding_release": "synthetic-binding-v2",
            "job_id": "prospect-fit-or-brief",
            "pack": {"id": compiled["pack"]["id"], "version": compiled["pack"]["version"], "sha256": compiled["pack"]["sha256"]},
            "requirements": {
                "contract": compiled["contract"],
                "sha256": compiled["requirements_sha256"],
                "decision_input_contracts": [{"id": contract_id, "version": contract_version}]
            },
            "normalization_release": "synthetic-normalizer-v2",
            "adapter": {"profile": "synthetic_adapter", "version": "2.0.0"},
            "transformation": {"id": "identity_v2"},
            "projection_bindings": [{
                "decision_input_contract_id": contract_id,
                "projection_id": "buying-window",
                "qualified_projection_id": format!("{}#buying-window", contract_id.as_str().unwrap()),
                "contributor_attribute_ids": ["last_meaningful_touch"],
                "source": {"logical_source_id": "synthetic_touch", "source_class": "synthetic_fixture", "acquisition_mode": "fixture", "upstream_reference": "opaque:touch"}
            }]
        });
        let binding_sha256 = canonical_json_sha256(&binding).unwrap();

        let mut request: Value = serde_json::from_slice(
            &std::fs::read(root.join("fixtures/source-attempt-request.json")).unwrap(),
        )
        .unwrap();
        request["contract"] = json!("mdp.source-attempt-request.v2");
        request["source_binding_sha256"] = json!(&binding_sha256);
        for attempt in request["attempts"].as_array_mut().unwrap() {
            attempt["decision_input_contract_id"] = contract_id.clone();
            attempt["source_locator"] = json!(format!(
                "opaque:{}",
                attempt["attempt_id"].as_str().unwrap()
            ));
        }
        let duplicate_attempt = {
            let mut attempt = request["attempts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["attribute_id"] == "last_meaningful_touch")
                .unwrap()
                .clone();
            attempt["attempt_id"] = json!("synthetic-attempt-099");
            attempt["source_locator"] = json!("opaque:synthetic-attempt-099");
            attempt
        };
        request["attempts"]
            .as_array_mut()
            .unwrap()
            .push(duplicate_attempt);
        let request_sha256 = canonical_json_sha256(&request).unwrap();

        let mut results: Value = serde_json::from_slice(
            &std::fs::read(root.join("fixtures/collected-attempt-results.json")).unwrap(),
        )
        .unwrap();
        results["contract"] = json!("mdp.collected-attempt-results.v2");
        results["source_binding_sha256"] = json!(&binding_sha256);
        results["source_attempt_request_sha256"] = json!(&request_sha256);
        for result in results["attributes"].as_object_mut().unwrap().values_mut() {
            if let Some(provenance) = result["provenance"].as_array_mut() {
                for receipt in provenance {
                    receipt["source_locator"] = json!(format!(
                        "opaque:{}",
                        receipt["attempt_id"].as_str().unwrap()
                    ));
                }
            }
        }
        results["attempt_results"] = json!([
            {
                "attempt_id": "synthetic-attempt-007", "decision_input_contract_id": contract_id,
                "attribute_id": "last_meaningful_touch", "status": "observed",
                "value": "2026-07-20T12:00:00Z", "source_class": "synthetic_fixture",
                "source_locator": "opaque:synthetic-attempt-007", "observed_at": "2026-07-29T12:00:00Z",
                "confidence": 100,
                "provenance": [{"attempt_id": "synthetic-attempt-007", "source_class": "synthetic_fixture", "source_locator": "opaque:synthetic-attempt-007", "observed_at": "2026-07-29T12:00:00Z"}],
                "freshness": {"observed_at": "2026-07-29T12:00:00Z", "age_days": 0}
            },
            {
                "attempt_id": "synthetic-attempt-099", "decision_input_contract_id": contract_id,
                "attribute_id": "last_meaningful_touch", "status": "observed",
                "value": "2026-07-20T12:00:00Z", "source_class": "synthetic_fixture",
                "source_locator": "opaque:synthetic-attempt-099", "observed_at": "2026-07-29T12:00:00Z",
                "confidence": 100,
                "provenance": [{"attempt_id": "synthetic-attempt-099", "source_class": "synthetic_fixture", "source_locator": "opaque:synthetic-attempt-099", "observed_at": "2026-07-29T12:00:00Z"}],
                "freshness": {"observed_at": "2026-07-29T12:00:00Z", "age_days": 0}
            }
        ]);
        let results_sha256 = canonical_json_sha256(&results).unwrap();

        let mut output: Value = serde_json::from_slice(
            &std::fs::read(root.join("fixtures/normalized-response-ready.json")).unwrap(),
        )
        .unwrap();
        output["contract"] = json!("mdp.normalized-decision-input.v2");
        output["source_binding_sha256"] = json!(&binding_sha256);
        output["source_attempt_request_sha256"] = json!(&request_sha256);
        output["collected_attempt_results_sha256"] = json!(&results_sha256);
        output["attributes"] = results["attributes"].clone();
        output["signal_observations"] = json!([
            signal_observation(
                "obs-b",
                "synthetic-attempt-099",
                "opaque:synthetic-attempt-099",
                &contract_id,
                &binding_sha256,
                &request_sha256,
                &results_sha256
            ),
            signal_observation(
                "obs-a",
                "synthetic-attempt-007",
                "opaque:synthetic-attempt-007",
                &contract_id,
                &binding_sha256,
                &request_sha256,
                &results_sha256
            )
        ]);
        SignalProjectionFixture {
            root: root.clone(),
            prompt_path: root.join(".mdp/prompts/normalize-prospect.yaml"),
            binding,
            binding_sha256,
            request,
            request_sha256,
            results,
            results_sha256,
            output,
        }
    }

    #[test]
    fn signal_aware_fit_consumes_lineage_validated_observations_for_legacy_signal_requirements() {
        let mut fixture =
            signal_projection_fixture_with_legacy_signal_requirements("fit-v2-signal-authority");
        let normalized_path = fixture.root.join("normalized.json");
        let binding_path = fixture.root.join("binding.json");
        let request_path = fixture.root.join("request.json");
        let results_path = fixture.root.join("results.json");
        let write_json = |path: &Path, value: &Value| {
            let bytes = serde_json::to_vec(value).unwrap();
            std::fs::write(path, &bytes).expect("fixture should be writable");
            crate::artifact_hash::sha256_hex(&bytes)
        };
        let binding_sha256 = write_json(&binding_path, &fixture.binding);
        fixture.request["source_binding_sha256"] = json!(&binding_sha256);
        let request_sha256 = write_json(&request_path, &fixture.request);
        fixture.results["source_binding_sha256"] = json!(&binding_sha256);
        fixture.results["source_attempt_request_sha256"] = json!(&request_sha256);
        let results_sha256 = write_json(&results_path, &fixture.results);
        fixture.output["source_binding_sha256"] = json!(&binding_sha256);
        fixture.output["source_attempt_request_sha256"] = json!(&request_sha256);
        fixture.output["collected_attempt_results_sha256"] = json!(&results_sha256);
        for observation in fixture.output["signal_observations"]
            .as_array_mut()
            .expect("signal observations should be an array")
        {
            observation["receipt"]["source_binding_sha256"] = json!(&binding_sha256);
            observation["receipt"]["source_attempt_request_sha256"] = json!(&request_sha256);
            observation["receipt"]["collected_results_sha256"] = json!(&results_sha256);
        }
        write_json(&normalized_path, &fixture.output);
        let result = fit_normalized(
            &fixture.root,
            &normalized_path,
            &fixture.prompt_path,
            &binding_path,
            &request_path,
            &results_path,
            Some("prospect-fit-or-brief"),
        )
        .expect("lineage-validated v2 fit should succeed");

        assert_eq!(result["status"], "fit");
        assert_eq!(result["context"]["has_signals"], true);
        assert_eq!(result["context"]["has_signal_source"], true);
        assert!(
            result["context"]["missing"]
                .as_array()
                .expect("missing requirements")
                .iter()
                .all(|item| item != "signals" && item != "signals.source")
        );
        assert_eq!(
            result["signal_authority"]["authority_class"],
            "lineage-validated"
        );

        let _ = std::fs::remove_dir_all(fixture.root);
    }

    fn signal_observation(
        id: &str,
        attempt_id: &str,
        locator: &str,
        contract_id: &Value,
        binding_sha256: &str,
        request_sha256: &str,
        results_sha256: &str,
    ) -> Value {
        json!({
            "contract": "mdp.signal-observation.v2", "id": id,
            "contract_id": contract_id, "projection_id": "buying-window",
            "qualified_projection_id": format!("{}#buying-window", contract_id.as_str().unwrap()),
            "kind": "profile_buying_window", "roles": ["why-now"],
            "value": "2026-07-20T12:00:00Z", "contributor_attribute_ids": ["last_meaningful_touch"],
            "attempt_ids": [attempt_id], "source_class": "synthetic_fixture", "source_locator": locator,
            "observed_at": "2026-07-29T12:00:00Z", "confidence": 100,
            "receipt": {"source_binding_sha256": binding_sha256, "source_attempt_request_sha256": request_sha256, "collected_results_sha256": results_sha256}
        })
    }

    #[test]
    fn signal_aware_requirements_publish_v2_matrix_and_binding_schema() {
        let root = signal_aware_clay_example("v2-requirements");
        let compiled = requirements(&root, "prospect-fit-or-brief")
            .expect("signal-aware requirements should compile");

        assert_eq!(compiled["contract"], "mdp.requirements.v2", "{compiled:#}");
        assert_eq!(compiled["runtime_contract_version"], "v2");
        assert_eq!(
            compiled["source_binding_schema"]["properties"]["contract"]["const"],
            "mdp.source-binding.v2"
        );
        assert_eq!(
            compiled["contract_version_matrix"]["signal_aware_v2"]["normalized_output"],
            "mdp.normalized-decision-input.v2"
        );
        assert!(
            compiled["requirements_sha256"]
                .as_str()
                .is_some_and(|sha| sha.len() == 64)
        );
        let _ = std::fs::remove_dir_all(root);
    }

    fn copy_tree(source: &Path, destination: &Path) {
        std::fs::create_dir_all(destination).expect("fixture destination should be created");
        for entry in std::fs::read_dir(source).expect("fixture source should be readable") {
            let entry = entry.expect("fixture entry should be readable");
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if source_path.is_dir() {
                copy_tree(&source_path, &destination_path);
            } else {
                std::fs::copy(&source_path, &destination_path).expect("fixture file should copy");
            }
        }
    }

    fn temporary_clay_example(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-clay-{name}-{nonce}"));
        copy_tree(&clay_example_root(), &root);
        root
    }

    fn generated_gtm_pack(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-gtm-{name}-{nonce}"));
        crate::commands::init::init_pack_targeted(
            &root,
            "Basic MDP Template",
            "gtm",
            &crate::commands::init::TargetInitOptions::default(),
            true,
            false,
        )
        .expect("generated GTM pack should initialize");
        root
    }

    fn scalar_clay_example(name: &str) -> PathBuf {
        let root = temporary_clay_example(name);
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should read");
        let mut manifest: serde_yaml::Value = serde_yaml::from_str(&raw).expect("manifest parses");
        manifest["decision_input_contracts"][0]["signal_projections"] =
            serde_yaml::Value::Sequence(Vec::new());
        manifest["decision_input_contracts"][0]["normalization"]["normalized_schema_ref"] =
            serde_yaml::Value::String(NORMALIZED_DECISION_INPUT_CONTRACT.to_string());
        std::fs::write(&manifest_path, serde_yaml::to_string(&manifest).unwrap()).unwrap();
        let prompt_path = root.join(".mdp/prompts/normalize-prospect.yaml");
        let prompt = std::fs::read_to_string(&prompt_path)
            .expect("normalization prompt should read")
            .replace(
                NORMALIZED_DECISION_INPUT_CONTRACT_V2,
                NORMALIZED_DECISION_INPUT_CONTRACT,
            );
        let mut prompt: serde_yaml::Value = serde_yaml::from_str(&prompt).unwrap();
        let example = prompt["output_contract"]["example"]
            .as_mapping_mut()
            .unwrap();
        example.remove(&serde_yaml::Value::String(
            "source_binding_sha256".to_string(),
        ));
        example.remove(&serde_yaml::Value::String(
            "signal_observations".to_string(),
        ));
        if let Some(required) = prompt["output_contract"]["required_top_level"].as_sequence_mut() {
            required.retain(|field| {
                !matches!(
                    field.as_str(),
                    Some(
                        "source_binding_sha256"
                            | "source_attempt_request_sha256"
                            | "collected_attempt_results_sha256"
                            | "signal_observations"
                    )
                )
            });
        }
        std::fs::write(prompt_path, serde_yaml::to_string(&prompt).unwrap()).unwrap();
        root
    }

    fn add_product_foundation(root: &Path, selected_entry_id: &str) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile"]["product_foundation"] = serde_yaml::from_str(&format!(
            r#"
facets:
  - id: selected-identity
    kind: product_identity
    entries:
      - card_id: positioning
        entry_id: {selected_entry_id}
  - id: optional-gap
    kind: gaps
    gaps:
      - card_id: gaps
        entry_id: target-foundation-gaps
"#
        ))
        .expect("foundation should parse");
        manifest["jobs"][0]["product_foundation"] = serde_yaml::from_str(
            r#"
required:
  - selected-identity
optional:
  - optional-gap
"#,
        )
        .expect("binding should parse");
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    #[test]
    fn requirements_compiles_exact_job_owned_prompt_package() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-job-owned-prompt-{nonce}"));
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter should initialize");

        let compiled =
            requirements(&root, "outbound-copy-brief").expect("requirements should compile");

        assert_eq!(compiled["model_task"]["status"], "ready");
        assert_eq!(
            compiled["model_steps"]["contract"],
            "mdp.model-step-resolution.v1"
        );
        assert_eq!(compiled["model_steps"]["status"], "ready");
        assert_eq!(
            compiled["model_steps"]["steps"][0]["phase"],
            "normalization"
        );
        assert_eq!(compiled["model_steps"]["steps"][1]["phase"], "generation");
        assert_eq!(
            compiled["model_steps"]["steps"][1]["step_id"],
            "model:outbound-copy-brief/generation"
        );
        assert_eq!(compiled["model_task"]["kind"], "generation");
        assert_eq!(compiled["model_task"]["prompt_version"], "2");
        assert_eq!(
            compiled["model_task"]["host_boundary"]["model_call_included"],
            false
        );
        assert!(
            compiled["model_task"]["prompt_sha256"]
                .as_str()
                .is_some_and(|value| value.len() == 64)
        );
        assert_eq!(
            compiled["model_task"]["declared_inputs"][0]["producer"],
            "pack"
        );
        assert_eq!(
            compiled["model_task"]["instructions"]["instructions"][0],
            "Use only declared inputs and the exact mdp.routed-context.v1 authority for this job."
        );
        assert_eq!(
            compiled["job"]["resolved_input_contracts"][0]["prompt"],
            "prompts/normalize-prospect.yaml"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    fn clay_validation_fixture() -> (PathBuf, Value, Value, String, PathBuf) {
        let root = scalar_clay_example("legacy-validation");
        let request_raw = std::fs::read(root.join("fixtures/source-attempt-request.json"))
            .expect("source-attempt fixture bytes should load");
        let mut request: Value =
            serde_json::from_slice(&request_raw).expect("source-attempt fixture should parse");
        request["contract"] = json!("mdp.source-attempt-request.v1");
        request
            .as_object_mut()
            .unwrap()
            .remove("source_binding_sha256");
        for attempt in request["attempts"].as_array_mut().unwrap() {
            attempt
                .as_object_mut()
                .unwrap()
                .remove("decision_input_contract_id");
        }
        let request_bytes = serde_json::to_vec_pretty(&request).unwrap();
        let request_sha256 = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(&request_bytes))
        };
        let mut response: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("fixtures/normalized-response-ready.json"))
                .expect("normalized response fixture should load"),
        )
        .expect("normalized response fixture should parse");
        response["contract"] = json!(NORMALIZED_DECISION_INPUT_CONTRACT);
        response["source_attempt_request_sha256"] = json!(&request_sha256);
        response
            .as_object_mut()
            .unwrap()
            .remove("source_binding_sha256");
        response
            .as_object_mut()
            .unwrap()
            .remove("signal_observations");
        let prompt_path = root.join(".mdp/prompts/normalize-prospect.yaml");
        (root, request, response, request_sha256, prompt_path)
    }

    fn semantic_issue_codes(
        root: &Path,
        request: &Value,
        response: &Value,
        request_sha256: &str,
        prompt_path: &Path,
    ) -> BTreeSet<String> {
        let results_raw = std::fs::read(root.join("fixtures/collected-attempt-results.json"))
            .expect("collected-results fixture bytes should load");
        let mut results: Value =
            serde_json::from_slice(&results_raw).expect("collected-results fixture should parse");
        results["contract"] = json!(COLLECTED_ATTEMPT_RESULTS_CONTRACT);
        results["source_attempt_request_sha256"] = json!(request_sha256);
        results
            .as_object_mut()
            .unwrap()
            .remove("source_binding_sha256");
        results.as_object_mut().unwrap().remove("attempt_results");
        results["attributes"] = response["attributes"].clone();
        let results_bytes =
            serde_json::to_vec_pretty(&results).expect("collected results should serialize");
        let results_sha256 = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(results_bytes))
        };
        let mut bound_response = response.clone();
        bound_response["collected_attempt_results_sha256"] = json!(&results_sha256);
        validate_normalized_decision_input(
            root,
            &bound_response,
            "synthetic-response",
            prompt_path,
            Some((request, "synthetic-request", request_sha256)),
            Some((&results, "synthetic-results", &results_sha256)),
        )
        .expect("semantic validation should run")
        .into_iter()
        .filter_map(|issue| issue["code"].as_str().map(str::to_string))
        .collect()
    }

    #[test]
    fn required_attributes_compile_attempted_complete_status_behavior() {
        let attribute = DecisionInputAttribute {
            id: "company_domain".to_string(),
            question: "What is the canonical company domain?".to_string(),
            output_path: "company_domain".to_string(),
            requirement: DecisionInputRequirement::Required,
            source_classes: vec![DecisionInputSourceClass::UserProvided],
            sensitivity: DecisionInputSensitivity::Public,
            ..DecisionInputAttribute::default()
        };

        let behavior = effective_status_behavior(&attribute);

        assert_eq!(
            behavior[&DecisionInputAttemptStatus::Observed],
            DecisionInputDisposition::Accept
        );
        assert_eq!(
            behavior[&DecisionInputAttemptStatus::NotFound],
            DecisionInputDisposition::Gap
        );
        assert_eq!(
            behavior[&DecisionInputAttemptStatus::Error],
            DecisionInputDisposition::HumanReview
        );
    }

    #[test]
    fn optional_provider_failures_require_human_review() {
        let attribute = DecisionInputAttribute {
            requirement: DecisionInputRequirement::Optional,
            ..DecisionInputAttribute::default()
        };

        assert_eq!(
            effective_status_behavior(&attribute)[&DecisionInputAttemptStatus::NotFound],
            DecisionInputDisposition::Accept
        );
        assert_eq!(
            effective_status_behavior(&attribute)[&DecisionInputAttemptStatus::Blocked],
            DecisionInputDisposition::HumanReview
        );
        assert_eq!(
            effective_status_behavior(&attribute)[&DecisionInputAttemptStatus::Error],
            DecisionInputDisposition::HumanReview
        );
        assert!(
            ready_outcome_guard(&attribute).is_some(),
            "optional provider and access failures must prevent a ready outcome"
        );
    }

    #[test]
    fn required_fail_open_override_cannot_compile_a_ready_schema() {
        let (root, _request, mut response, _request_sha256, _prompt_path) =
            clay_validation_fixture();
        let mut manifest = read_manifest(&root).expect("Clay example manifest should load");
        let contract = manifest
            .decision_input_contracts
            .iter_mut()
            .find(|contract| contract.id == "clay.audiences.self_serve_enterprise_expansion")
            .expect("Clay decision-input contract should exist");
        let attribute = contract
            .attributes
            .iter_mut()
            .find(|attribute| attribute.id == "company_domain")
            .expect("Clay contract should include company_domain");
        attribute.status_behavior.insert(
            DecisionInputAttemptStatus::NotFound,
            DecisionInputDisposition::Accept,
        );
        assert_eq!(
            effective_status_behavior(attribute)[&DecisionInputAttemptStatus::NotFound],
            DecisionInputDisposition::Gap,
            "unsafe required overrides must be clamped fail-closed in compiled contracts"
        );
        let contracts = vec![&*contract];
        let compiled_schema = normalized_envelope_schema("prospect-fit-or-brief", &contracts);

        response["attributes"]["company_domain"] = json!({"status": "not_found"});
        response["normalized_prospect"]
            .as_object_mut()
            .expect("normalized prospect should be an object")
            .remove("company_domain");

        assert!(
            draft202012::validate(&compiled_schema, &response).is_err(),
            "required not_found evidence must never validate with outcome ready"
        );
    }

    #[test]
    fn normalized_attribute_schema_enforces_declared_evidence_policy() {
        let attribute = DecisionInputAttribute {
            source_classes: vec![DecisionInputSourceClass::CustomerSystem],
            provenance: DecisionInputProvenancePolicy {
                required: true,
                required_fields: vec![
                    DecisionInputProvenanceField::AttemptId,
                    DecisionInputProvenanceField::SourceClass,
                    DecisionInputProvenanceField::SourceLocator,
                    DecisionInputProvenanceField::ObservedAt,
                ],
            },
            confidence: DecisionInputConfidencePolicy {
                required: true,
                minimum: Some(90),
            },
            freshness: DecisionInputFreshnessPolicy {
                required: true,
                max_age_days: Some(30),
                allow_unknown: false,
            },
            ..DecisionInputAttribute::default()
        };

        let schema = attempt_result_schema(&attribute);

        assert_eq!(schema["required"], json!(["status"]));
        assert_eq!(
            schema["allOf"][0]["then"]["required"],
            json!(["value", "provenance", "confidence", "freshness"])
        );
        assert_eq!(
            schema["allOf"][0]["then"]["properties"]["provenance"]["minItems"],
            1
        );
        assert_eq!(
            schema["allOf"][0]["then"]["properties"]["confidence"]["minimum"],
            90
        );
        assert_eq!(
            schema["allOf"][0]["then"]["properties"]["freshness"]["properties"]["age_days"]["maximum"],
            30
        );
        assert_eq!(
            schema["allOf"][0]["then"]["properties"]["freshness"]["required"],
            json!(["observed_at", "age_days"])
        );

        let valid = json!({
            "status": "observed",
            "value": "current",
            "provenance": [{
                "attempt_id": "synthetic-attempt-001",
                "source_class": "customer_system",
                "source_locator": "synthetic://requirements-test",
                "observed_at": "2026-07-29T12:00:00Z"
            }],
            "confidence": 100,
            "freshness": {
                "observed_at": "2026-07-29T12:00:00Z",
                "age_days": 30
            }
        });
        draft202012::validate(&schema, &valid)
            .expect("known freshness at the maximum age should validate");

        let mut missing_age = valid.clone();
        missing_age["freshness"]
            .as_object_mut()
            .expect("freshness should be an object")
            .remove("age_days");
        assert!(
            draft202012::validate(&schema, &missing_age).is_err(),
            "required known freshness must include age_days"
        );

        let mut over_limit = valid.clone();
        over_limit["freshness"]["age_days"] = json!(31);
        assert!(
            draft202012::validate(&schema, &over_limit).is_err(),
            "freshness older than max_age_days must be rejected"
        );

        let mut unknown_allowed_attribute = attribute.clone();
        unknown_allowed_attribute.freshness.allow_unknown = true;
        let unknown_allowed_schema = attempt_result_schema(&unknown_allowed_attribute);
        assert_eq!(
            unknown_allowed_schema["allOf"][0]["then"]["properties"]["freshness"]["required"],
            json!(["observed_at"])
        );
        draft202012::validate(&unknown_allowed_schema, &missing_age)
            .expect("allow_unknown should preserve acceptance when age_days is absent");

        for status in ["not_found", "not_applicable", "blocked"] {
            draft202012::validate(&schema, &json!({"status": status}))
                .unwrap_or_else(|error| panic!("clean {status} must validate: {error}"));
        }
        assert!(
            draft202012::validate(&schema, &json!({"status": "error"})).is_err(),
            "error status must include a non-blank error"
        );
        draft202012::validate(
            &schema,
            &json!({"status": "error", "error": "synthetic provider failure"}),
        )
        .expect("clean provider error should validate without observation evidence");
        assert!(
            draft202012::validate(
                &schema,
                &json!({"status": "blocked", "error": "wrong status for error detail"})
            )
            .is_err(),
            "error detail must be exclusive to error status"
        );
    }

    #[test]
    fn clay_example_fixtures_cover_the_versioned_contract_exactly() {
        let root = clay_example_root();
        let manifest = read_manifest(&root).expect("Clay example manifest should load");
        let contract = manifest
            .decision_input_contracts
            .first()
            .expect("Clay example should declare one decision input contract");
        let expected_attributes = contract
            .attributes
            .iter()
            .map(|attribute| attribute.id.as_str())
            .collect::<BTreeSet<_>>();
        let request: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("fixtures/source-attempt-request.json"))
                .expect("source-attempt fixture should load"),
        )
        .expect("source-attempt fixture should be valid JSON");
        let requested_attributes = request["attempts"]
            .as_array()
            .expect("attempts should be an array")
            .iter()
            .filter_map(|attempt| attempt["attribute_id"].as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(requested_attributes, expected_attributes);
        assert_eq!(
            request["decision_input_contracts"][0],
            json!({"id": contract.id, "version": contract.version})
        );

        let response: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("fixtures/normalized-response-ready.json"))
                .expect("normalized response fixture should load"),
        )
        .expect("normalized response fixture should be valid JSON");
        let prompt =
            crate::pack_io::read_prompt(&root.join(".mdp/prompts/normalize-prospect.yaml"))
                .expect("bound normalization prompt should load");
        assert_eq!(
            prompt.output_contract.contract,
            NORMALIZED_DECISION_INPUT_CONTRACT_V2
        );
        assert_eq!(
            prompt.output_contract.output_kind.as_deref(),
            Some("decision-input-normalization")
        );
        assert_eq!(
            prompt.output_contract.schema_ref.as_deref(),
            Some(NORMALIZED_DECISION_INPUT_CONTRACT_V2)
        );
        assert_eq!(
            prompt.output_contract.example["contract"], NORMALIZED_DECISION_INPUT_CONTRACT_V2,
            "the illustrative prompt example must stay on the selected v2 contract"
        );
        assert_eq!(prompt.output_contract.example["draft_allowed"], false);
        let normalized_attributes = response["attributes"]
            .as_object()
            .expect("normalized attributes should be an object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(normalized_attributes, expected_attributes);
        assert_eq!(response["draft_allowed"], false);
        assert_eq!(response["outcome"], "ready");
        assert_eq!(
            response["normalization"][0]["prompt_version"],
            contract.normalization.prompt_version
        );

        let compiled =
            requirements(&root, "prospect-fit-or-brief").expect("requirements should compile");
        assert_eq!(
            compiled["normalized_prospect_unbound_policy"]["allowed_fields"],
            json!(["source_kind", "synthetic"]),
            "compiler must explicitly limit unbound fields to non-decision provenance markers"
        );
        draft202012::validate(&compiled["source_attempt_request_schema"], &request)
            .expect("exact source-attempt fixture should satisfy the compiled schema");
        draft202012::validate(&compiled["normalized_output_schema"], &response)
            .expect("exact normalized response fixture should satisfy the compiled schema");
        let prompt_path = root.join(".mdp/prompts/normalize-prospect.yaml");
        let request_raw = std::fs::read(root.join("fixtures/source-attempt-request.json"))
            .expect("source-attempt fixture bytes should load");
        let request_sha256 = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(request_raw))
        };
        let results_raw = std::fs::read(root.join("fixtures/collected-attempt-results.json"))
            .expect("collected-results fixture bytes should load");
        let results: Value = serde_json::from_slice(&results_raw)
            .expect("collected-results fixture should be valid JSON");
        let results_sha256 = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(results_raw))
        };
        draft202012::validate(&compiled["collected_attempt_results_schema"], &results)
            .expect("exact collected-results fixture should satisfy the compiled schema");
        assert!(
            validate_normalized_decision_input(
                &root,
                &response,
                "synthetic-response",
                &prompt_path,
                Some((&request, "synthetic-request", &request_sha256)),
                Some((&results, "synthetic-results", &results_sha256)),
            )
            .expect("semantic validation should run")
            .is_empty(),
            "ready fixture should preserve every observed output-path projection"
        );
        let mut projection_mismatch = response.clone();
        projection_mismatch["normalized_prospect"]["name"] = json!("Different Synthetic Person");
        assert!(
            validate_normalized_decision_input(
                &root,
                &projection_mismatch,
                "synthetic-response",
                &prompt_path,
                Some((&request, "synthetic-request", &request_sha256)),
                Some((&results, "synthetic-results", &results_sha256)),
            )
            .expect("semantic validation should run")
            .iter()
            .any(|issue| issue["code"] == "decision_input_projection_mismatch"),
            "semantic validation must reject observed attribute/output-path disagreement"
        );

        let mut raw_results = results.clone();
        raw_results["attributes"]["customer_motion"]["value"] = json!("Self Serve");
        let raw_results_bytes =
            serde_json::to_vec_pretty(&raw_results).expect("raw results should serialize");
        let raw_results_sha256 = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(&raw_results_bytes))
        };
        let mut canonicalized_response = response.clone();
        canonicalized_response["collected_attempt_results_sha256"] = json!(&raw_results_sha256);
        assert!(
            validate_normalized_decision_input(
                &root,
                &canonicalized_response,
                "synthetic-response",
                &prompt_path,
                Some((&request, "synthetic-request", &request_sha256)),
                Some((&raw_results, "synthetic-raw-results", &raw_results_sha256)),
            )
            .expect("semantic validation should run")
            .is_empty(),
            "normalization may canonicalize raw values into the declared normalized value contract"
        );

        let mut canonical_results = results.clone();
        canonical_results["attributes"]["customer_motion"]["value"] = json!("sales-assisted");
        let canonical_results_bytes = serde_json::to_vec_pretty(&canonical_results)
            .expect("canonical results should serialize");
        let canonical_results_sha256 = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(&canonical_results_bytes))
        };
        let mut reversed_hard_gate = response.clone();
        reversed_hard_gate["collected_attempt_results_sha256"] = json!(&canonical_results_sha256);
        assert!(
            validate_normalized_decision_input(
                &root,
                &reversed_hard_gate,
                "synthetic-response",
                &prompt_path,
                Some((&request, "synthetic-request", &request_sha256)),
                Some((
                    &canonical_results,
                    "synthetic-canonical-results",
                    &canonical_results_sha256,
                )),
            )
            .expect("semantic validation should run")
            .iter()
            .any(|issue| issue["code"] == "decision_input_canonical_collected_value_changed"),
            "normalization must not change one already-valid hard-gate value into another"
        );

        let mut nonsense_results = results.clone();
        nonsense_results["attributes"]["customer_motion"]["value"] = json!("nonsense");
        let nonsense_results_bytes = serde_json::to_vec_pretty(&nonsense_results)
            .expect("nonsense results should serialize");
        let nonsense_results_sha256 = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(&nonsense_results_bytes))
        };
        let mut invented_hard_gate = response.clone();
        invented_hard_gate["collected_attempt_results_sha256"] = json!(&nonsense_results_sha256);
        assert!(
            validate_normalized_decision_input(
                &root,
                &invented_hard_gate,
                "synthetic-response",
                &prompt_path,
                Some((&request, "synthetic-request", &request_sha256)),
                Some((
                    &nonsense_results,
                    "synthetic-nonsense-results",
                    &nonsense_results_sha256,
                )),
            )
            .expect("semantic validation should run")
            .iter()
            .any(|issue| issue["code"] == "decision_input_canonical_collected_value_changed"),
            "normalization must not turn an unknown raw value into a valid hard-gate value"
        );

        let mut blocked_results = results.clone();
        let blocked_company = blocked_results["attributes"]["company_domain"]
            .as_object_mut()
            .expect("company-domain result should be an object");
        blocked_company.insert("status".to_string(), json!("blocked"));
        blocked_company.remove("value");
        let blocked_results_raw =
            serde_json::to_vec_pretty(&blocked_results).expect("blocked results should serialize");
        let blocked_results_sha256 = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(&blocked_results_raw))
        };
        let mut fabricated_observed = response.clone();
        fabricated_observed["collected_attempt_results_sha256"] = json!(&blocked_results_sha256);
        assert!(
            validate_normalized_decision_input(
                &root,
                &fabricated_observed,
                "synthetic-response",
                &prompt_path,
                Some((&request, "synthetic-request", &request_sha256)),
                Some((
                    &blocked_results,
                    "synthetic-blocked-results",
                    &blocked_results_sha256,
                )),
            )
            .expect("semantic validation should run")
            .iter()
            .any(|issue| issue["code"] == "decision_input_collected_attempt_results_mismatch"),
            "normalization must not turn a host-recorded blocked result into observed/ready"
        );

        let mut unbound_routing = response.clone();
        unbound_routing["normalized_prospect"]["segment"] = json!("invented-segment");
        unbound_routing["normalized_prospect"]["signals"] = json!([{
            "id": "invented-signal",
            "title": "Invented routing signal"
        }]);
        assert!(
            validate_normalized_decision_input(
                &root,
                &unbound_routing,
                "synthetic-response",
                &prompt_path,
                Some((&request, "synthetic-request", &request_sha256)),
                Some((&results, "synthetic-results", &results_sha256)),
            )
            .expect("semantic validation should run")
            .iter()
            .any(|issue| issue["code"] == "decision_input_unbound_prospect_field"),
            "normalization must reject segment and signal fields without declared output paths"
        );

        let mut forbidden_source = request.clone();
        let attempt = forbidden_source["attempts"]
            .as_array_mut()
            .expect("attempts should remain an array")
            .iter_mut()
            .find(|attempt| attempt["attribute_id"] == "do_not_contact")
            .expect("fixture should include the do-not-contact hard gate");
        attempt["source_class"] = json!("public_web");
        assert!(
            draft202012::validate(
                &compiled["source_attempt_request_schema"],
                &forbidden_source
            )
            .is_err(),
            "per-attribute source policy must reject a contract-wide but forbidden source"
        );

        let mut wrong_prompt_version = response.clone();
        wrong_prompt_version["normalization"][0]["prompt_version"] = json!("wrong.v9");
        assert!(
            draft202012::validate(&compiled["normalized_output_schema"], &wrong_prompt_version)
                .is_err(),
            "normalization receipts must use the exact compiled prompt version"
        );

        let mut empty_normalization = response.clone();
        empty_normalization["normalization"] = json!([]);
        assert!(
            draft202012::validate(&compiled["normalized_output_schema"], &empty_normalization)
                .is_err(),
            "every selected contract must have one exact normalization receipt"
        );

        let mut blocked_with_stale_value = response.clone();
        blocked_with_stale_value["attributes"]["do_not_contact"]["status"] = json!("blocked");
        assert!(
            draft202012::validate(
                &compiled["normalized_output_schema"],
                &blocked_with_stale_value
            )
            .is_err(),
            "non-observed attempts must not retain a stale normalized value"
        );

        let mut blocked_ready = response.clone();
        let blocked_gate = blocked_ready["attributes"]["do_not_contact"]
            .as_object_mut()
            .expect("do-not-contact should be an object");
        blocked_gate.insert("status".to_string(), json!("blocked"));
        blocked_gate.remove("value");
        assert!(
            draft202012::validate(&compiled["normalized_output_schema"], &blocked_ready).is_err(),
            "a readiness-blocking status must forbid a ready outcome"
        );

        blocked_ready["outcome"] = json!("human-review");
        draft202012::validate(&compiled["normalized_output_schema"], &blocked_ready)
            .expect("a non-ready outcome should remain valid for a blocked hard gate");

        for (status, result) in [
            ("blocked", json!({"status": "blocked"})),
            (
                "error",
                json!({
                    "status": "error",
                    "error": "Synthetic provider failure"
                }),
            ),
        ] {
            let mut optional_failure = response.clone();
            optional_failure["attributes"]["employee_band"] = result;
            optional_failure["normalized_prospect"]["attributes"]
                .as_object_mut()
                .expect("normalized attributes should be an object")
                .remove("employee_band");
            assert!(
                draft202012::validate(&compiled["normalized_output_schema"], &optional_failure)
                    .is_err(),
                "optional {status} must not certify a ready normalization outcome"
            );
            optional_failure["outcome"] = json!("human-review");
            draft202012::validate(&compiled["normalized_output_schema"], &optional_failure)
                .unwrap_or_else(|error| {
                    panic!("optional {status} should permit human-review: {error}")
                });
        }
    }

    #[test]
    fn decision_input_semantics_bind_source_attempts_prompt_and_freshness() {
        let (root, request, response, request_sha256, prompt_path) = clay_validation_fixture();

        assert!(
            semantic_issue_codes(&root, &request, &response, &request_sha256, &prompt_path,)
                .is_empty()
        );

        let mut unknown_attempt = response.clone();
        unknown_attempt["attributes"]["company_name"]["provenance"][0]["attempt_id"] =
            json!("synthetic-attempt-unknown");
        assert!(
            semantic_issue_codes(
                &root,
                &request,
                &unknown_attempt,
                &request_sha256,
                &prompt_path,
            )
            .contains("decision_input_provenance_attempt_unknown")
        );

        let attempt_id_only_root = scalar_clay_example("attempt-id-only-provenance");
        let manifest_path = attempt_id_only_root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replacen(
                "required_fields:\n      - attempt_id\n      - source_class\n      - source_locator\n      - observed_at",
                "required_fields:\n      - attempt_id",
                1,
            ),
        )
        .expect("manifest should be writable");
        let mut unknown_attempt_without_observed_at = response.clone();
        let company_receipt =
            unknown_attempt_without_observed_at["attributes"]["company_name"]["provenance"][0]
                .as_object_mut()
                .expect("company provenance receipt should be an object");
        company_receipt.insert("attempt_id".to_string(), json!("synthetic-attempt-unknown"));
        company_receipt.remove("observed_at");
        assert!(
            semantic_issue_codes(
                &attempt_id_only_root,
                &request,
                &unknown_attempt_without_observed_at,
                &request_sha256,
                &attempt_id_only_root.join(".mdp/prompts/normalize-prospect.yaml"),
            )
            .contains("decision_input_provenance_attempt_unknown"),
            "attempt_id binding must not depend on observed_at being present"
        );
        let _ = std::fs::remove_dir_all(attempt_id_only_root);

        let mut wrong_attribute_attempt = response.clone();
        wrong_attribute_attempt["attributes"]["company_name"]["provenance"][0]["attempt_id"] =
            response["attributes"]["company_domain"]["provenance"][0]["attempt_id"].clone();
        assert!(
            semantic_issue_codes(
                &root,
                &request,
                &wrong_attribute_attempt,
                &request_sha256,
                &prompt_path,
            )
            .contains("decision_input_provenance_attempt_attribute_mismatch")
        );

        let mut wrong_hash = response.clone();
        wrong_hash["source_attempt_request_sha256"] =
            json!("0000000000000000000000000000000000000000000000000000000000000000");
        assert!(
            semantic_issue_codes(&root, &request, &wrong_hash, &request_sha256, &prompt_path,)
                .contains("decision_input_source_attempt_request_hash_mismatch")
        );

        let wrong_prompt = root.join(".mdp/prompts/not-the-bound-prompt.yaml");
        assert!(
            semantic_issue_codes(&root, &request, &response, &request_sha256, &wrong_prompt,)
                .contains("decision_input_prompt_binding_mismatch")
        );

        let mut wrong_age = response.clone();
        wrong_age["attributes"]["last_meaningful_touch"]["freshness"]["age_days"] = json!(9);
        assert!(
            semantic_issue_codes(&root, &request, &wrong_age, &request_sha256, &prompt_path,)
                .contains("decision_input_freshness_age_mismatch")
        );

        let mut future_freshness = response.clone();
        future_freshness["attributes"]["last_meaningful_touch"]["freshness"]["observed_at"] =
            json!("2026-07-30T12:00:00Z");
        assert!(
            semantic_issue_codes(
                &root,
                &request,
                &future_freshness,
                &request_sha256,
                &prompt_path,
            )
            .contains("decision_input_freshness_observed_at_future")
        );

        let mut stale_provenance_with_fresh_metadata = response.clone();
        stale_provenance_with_fresh_metadata["attributes"]["person_title"]["provenance"][0]["observed_at"] =
            json!("2020-01-01T00:00:00Z");
        assert!(
            semantic_issue_codes(
                &root,
                &request,
                &stale_provenance_with_fresh_metadata,
                &request_sha256,
                &prompt_path,
            )
            .contains("decision_input_freshness_provenance_timestamp_mismatch"),
            "freshness must derive from the bound evidence timestamp"
        );

        let mut known_stale_without_age = response.clone();
        known_stale_without_age["attributes"]["employee_band"]["provenance"][0]["observed_at"] =
            json!("2020-01-01T00:00:00Z");
        known_stale_without_age["attributes"]["employee_band"]["freshness"]["observed_at"] =
            json!("2020-01-01T00:00:00Z");
        known_stale_without_age["attributes"]["employee_band"]["freshness"]
            .as_object_mut()
            .expect("employee_band freshness should be an object")
            .remove("age_days");
        assert!(
            semantic_issue_codes(
                &root,
                &request,
                &known_stale_without_age,
                &request_sha256,
                &prompt_path,
            )
            .contains("decision_input_freshness_age_over_limit"),
            "allow_unknown must not bypass a deterministically stale observed_at timestamp"
        );

        let mut future_business_date = response.clone();
        future_business_date["attributes"]["last_meaningful_touch"]["value"] =
            json!("2027-07-20T12:00:00Z");
        future_business_date["normalized_prospect"]["attributes"]["last_meaningful_touch"] =
            json!("2027-07-20T12:00:00Z");
        assert!(
            semantic_issue_codes(
                &root,
                &request,
                &future_business_date,
                &request_sha256,
                &prompt_path,
            )
            .is_empty(),
            "freshness should derive from provenance, not future business date values"
        );

        let historical_date_root = scalar_clay_example("historical-business-date");
        let manifest_path = historical_date_root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace("format: date-time", "format: date"),
        )
        .expect("date contract fixture should be writable");
        let mut historical_business_date = response;
        historical_business_date["attributes"]["last_meaningful_touch"]["value"] =
            json!("1999-01-01");
        historical_business_date["normalized_prospect"]["attributes"]["last_meaningful_touch"] =
            json!("1999-01-01");
        assert!(
            semantic_issue_codes(
                &historical_date_root,
                &request,
                &historical_business_date,
                &request_sha256,
                &historical_date_root.join(".mdp/prompts/normalize-prospect.yaml"),
            )
            .is_empty(),
            "freshness should derive from provenance, not historical business date values"
        );
        let _ = std::fs::remove_dir_all(historical_date_root);
    }

    #[test]
    fn decision_input_semantics_reject_invalid_observed_calendar_values() {
        let (root, request, response, request_sha256, prompt_path) = clay_validation_fixture();

        let mut invalid_date_time = response.clone();
        invalid_date_time["attributes"]["last_meaningful_touch"]["value"] =
            json!("2026-02-30T10:00:00Z");
        invalid_date_time["normalized_prospect"]["attributes"]["last_meaningful_touch"] =
            json!("2026-02-30T10:00:00Z");
        assert!(
            semantic_issue_codes(
                &root,
                &request,
                &invalid_date_time,
                &request_sha256,
                &prompt_path,
            )
            .contains("decision_input_observed_value_format_invalid")
        );

        let date_root = scalar_clay_example("invalid-observed-date");
        let manifest_path = date_root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace("format: date-time", "format: date"),
        )
        .expect("date contract fixture should be writable");
        let mut invalid_date = response;
        invalid_date["attributes"]["last_meaningful_touch"]["value"] = json!("2026-02-30");
        invalid_date["normalized_prospect"]["attributes"]["last_meaningful_touch"] =
            json!("2026-02-30");
        assert!(
            semantic_issue_codes(
                &date_root,
                &request,
                &invalid_date,
                &request_sha256,
                &date_root.join(".mdp/prompts/normalize-prospect.yaml"),
            )
            .contains("decision_input_observed_value_format_invalid")
        );

        let _ = std::fs::remove_dir_all(date_root);
    }

    #[test]
    fn provenance_attempt_binding_does_not_depend_on_observed_at() {
        let attribute = json!({"id": "company_name"});
        let attempt = json!({
            "status": "observed",
            "provenance": [{"attempt_id": "synthetic-attempt-unknown"}]
        });
        let known_attempt = json!({
            "attempt_id": "synthetic-attempt-known",
            "attribute_id": "company_name",
            "source_class": "synthetic_fixture",
            "source_locator": "synthetic://known"
        });
        let mut attempts = BTreeMap::new();
        attempts.insert("synthetic-attempt-known", &known_attempt);
        let source_attempt_index = SourceAttemptIndex {
            attempts,
            as_of_seconds: parse_utc_timestamp_seconds("2026-07-29T12:00:00Z")
                .expect("fixture timestamp should parse"),
        };
        let mut issues = Vec::new();

        validate_attribute_attempt_receipts(
            &attribute,
            &attempt,
            "synthetic-response",
            Some(&source_attempt_index),
            &mut issues,
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue["code"] == "decision_input_provenance_attempt_unknown")
        );
    }

    #[test]
    fn decision_input_semantics_reject_malformed_times_unobserved_values_and_blank_strings() {
        let (root, request, response, request_sha256, prompt_path) = clay_validation_fixture();

        let mut malformed_provenance = response.clone();
        malformed_provenance["attributes"]["company_name"]["provenance"][0]["observed_at"] =
            json!("not-a-timestamp");
        assert!(
            semantic_issue_codes(
                &root,
                &request,
                &malformed_provenance,
                &request_sha256,
                &prompt_path,
            )
            .contains("decision_input_schema_mismatch")
        );

        let mut malformed_freshness = response.clone();
        malformed_freshness["attributes"]["last_meaningful_touch"]["freshness"]["observed_at"] =
            json!("2026-99-99T12:00:00Z");
        assert!(
            semantic_issue_codes(
                &root,
                &request,
                &malformed_freshness,
                &request_sha256,
                &prompt_path,
            )
            .contains("decision_input_freshness_observed_at_invalid")
        );

        let mut unobserved_projection = response.clone();
        let attempt = unobserved_projection["attributes"]["employee_band"]
            .as_object_mut()
            .expect("employee band attempt should be an object");
        attempt.insert("status".to_string(), json!("not_found"));
        for field in ["value", "provenance", "confidence", "freshness"] {
            attempt.remove(field);
        }
        assert!(
            semantic_issue_codes(
                &root,
                &request,
                &unobserved_projection,
                &request_sha256,
                &prompt_path,
            )
            .contains("decision_input_unobserved_projection_present")
        );

        let mut blank_string = response.clone();
        blank_string["attributes"]["company_name"]["value"] = json!("   ");
        blank_string["normalized_prospect"]["company"] = json!("   ");
        assert!(
            semantic_issue_codes(
                &root,
                &request,
                &blank_string,
                &request_sha256,
                &prompt_path,
            )
            .contains("decision_input_schema_mismatch")
        );
    }

    #[test]
    fn utc_timestamp_parser_rejects_invalid_calendar_and_clock_values() {
        assert_eq!(parse_utc_timestamp_seconds("1970-01-01T00:00:00Z"), Some(0));
        for invalid in [
            "2026-02-30T12:00:00Z",
            "2026-07-29T24:00:00Z",
            "2026-07-29T12:60:00Z",
            "2026-07-29T12:00:60Z",
            "2026-07-29T12:00:00+00:00",
            "not-a-timestamp",
        ] {
            assert_eq!(
                parse_utc_timestamp_seconds(invalid),
                None,
                "{invalid} must be rejected"
            );
        }
    }

    #[test]
    fn conditional_applies_when_guards_block_not_applicable_ready_outcomes() {
        let root = clay_example_root();
        let manifest = read_manifest(&root).expect("Clay example manifest should load");
        let mut contract = manifest
            .decision_input_contracts
            .first()
            .expect("Clay example should declare one decision input contract")
            .clone();
        let response: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("fixtures/normalized-response-ready.json"))
                .expect("normalized response fixture should load"),
        )
        .expect("normalized response fixture should be valid JSON");

        let cases = [
            (
                DecisionInputConditionOperator::Exists,
                "person_title",
                Vec::<String>::new(),
            ),
            (
                DecisionInputConditionOperator::Equals,
                "person_title",
                vec!["VP Revenue Operations".to_string()],
            ),
            (
                DecisionInputConditionOperator::NotEquals,
                "person_title",
                vec!["Not This Title".to_string()],
            ),
            (
                DecisionInputConditionOperator::In,
                "person_title",
                vec![
                    "VP Revenue Operations".to_string(),
                    "Chief Revenue Officer".to_string(),
                ],
            ),
        ];

        for (operator, attribute, values) in cases {
            let conditional_attribute = contract
                .attributes
                .iter_mut()
                .find(|candidate| candidate.id == "current_working_country")
                .expect("fixture contract should include current_working_country");
            conditional_attribute.applies_when = vec![DecisionInputCondition {
                attribute: attribute.to_string(),
                operator,
                values,
            }];

            let schema = normalized_envelope_schema("prospect-fit-or-brief", &[&contract]);
            let mut invalid_ready = response.clone();
            let current_country = invalid_ready["attributes"]["current_working_country"]
                .as_object_mut()
                .expect("current_working_country should be an object");
            current_country.insert("status".to_string(), json!("not_applicable"));
            current_country.remove("value");
            assert!(
                draft202012::validate(&schema, &invalid_ready).is_err(),
                "applied conditional attributes must not validate as ready when marked not_applicable"
            );

            invalid_ready["outcome"] = json!("insufficient-context");
            assert!(
                draft202012::validate(&schema, &invalid_ready).is_err(),
                "applied conditional attributes must never be marked not_applicable"
            );
        }
    }

    #[test]
    fn compiled_schema_fails_closed_on_out_of_domain_applicability_values() {
        let root = clay_example_root();
        let manifest = read_manifest(&root).expect("Clay example manifest should load");
        let base_contract = manifest
            .decision_input_contracts
            .first()
            .expect("Clay example should declare one decision input contract")
            .clone();
        let response: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("fixtures/normalized-response-ready.json"))
                .expect("normalized response fixture should load"),
        )
        .expect("normalized response fixture should be valid JSON");

        for (operator, values) in [
            (
                DecisionInputConditionOperator::Exists,
                vec!["ignored-value".to_string()],
            ),
            (
                DecisionInputConditionOperator::Equals,
                vec!["not-a-real-enum-value".to_string()],
            ),
            (
                DecisionInputConditionOperator::NotEquals,
                vec!["not-a-real-enum-value".to_string()],
            ),
            (
                DecisionInputConditionOperator::In,
                vec!["not-a-real-enum-value".to_string()],
            ),
        ] {
            let mut contract = base_contract.clone();
            let condition = &mut contract
                .attributes
                .iter_mut()
                .find(|attribute| attribute.id == "latest_support_context")
                .expect("Clay example should include latest_support_context")
                .applies_when[0];
            condition.operator = operator.clone();
            condition.values = values;

            let compiled = normalized_envelope_schema("prospect-fit-or-brief", &[&contract]);

            assert!(
                compiled["allOf"]
                    .as_array()
                    .expect("compiled guards should be an array")
                    .contains(&json!(false)),
                "{operator:?} out-of-domain operands must make the compiled contract unsatisfiable"
            );
            assert!(
                draft202012::validate(&compiled, &response).is_err(),
                "{operator:?} out-of-domain operands must reject every normalized envelope"
            );
        }
    }

    #[test]
    fn false_conditional_predicates_require_not_applicable_for_every_operator() {
        let root = clay_example_root();
        let manifest = read_manifest(&root).expect("Clay example manifest should load");
        let base_contract = manifest
            .decision_input_contracts
            .first()
            .expect("Clay example should declare one decision input contract")
            .clone();
        let response: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("fixtures/normalized-response-ready.json"))
                .expect("normalized response fixture should load"),
        )
        .expect("normalized response fixture should be valid JSON");
        let cases = [
            (DecisionInputConditionOperator::Exists, Vec::<String>::new()),
            (
                DecisionInputConditionOperator::Equals,
                vec!["Chief Financial Officer".to_string()],
            ),
            (
                DecisionInputConditionOperator::NotEquals,
                vec!["VP Revenue Operations".to_string()],
            ),
            (
                DecisionInputConditionOperator::In,
                vec!["Chief Financial Officer".to_string()],
            ),
        ];

        for (operator, values) in cases {
            let mut contract = base_contract.clone();
            let conditional_attribute = contract
                .attributes
                .iter_mut()
                .find(|candidate| candidate.id == "current_working_country")
                .expect("fixture contract should include current_working_country");
            conditional_attribute.applies_when = vec![DecisionInputCondition {
                attribute: "person_title".to_string(),
                operator: operator.clone(),
                values,
            }];
            let schema = normalized_envelope_schema("prospect-fit-or-brief", &[&contract]);
            let mut false_condition = response.clone();
            false_condition["outcome"] = json!("insufficient-context");
            if operator == DecisionInputConditionOperator::Exists {
                let dependency = false_condition["attributes"]["person_title"]
                    .as_object_mut()
                    .expect("person_title should be an object");
                dependency.insert("status".to_string(), json!("not_found"));
                dependency.remove("value");
            }

            assert!(
                draft202012::validate(&schema, &false_condition).is_err(),
                "{operator:?} false predicate must reject an observed conditional value"
            );

            let target = false_condition["attributes"]["current_working_country"]
                .as_object_mut()
                .expect("current_working_country should be an object");
            target.insert("status".to_string(), json!("not_applicable"));
            target.remove("value");
            draft202012::validate(&schema, &false_condition).unwrap_or_else(|error| {
                panic!("{operator:?} false predicate must accept not_applicable: {error}")
            });
        }
    }

    #[test]
    fn requirements_fails_closed_when_pack_validation_fails() {
        let root = temporary_clay_example("invalid-prompt");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replacen(
                "prompt: prompts/normalize-prospect.yaml",
                "prompt: prompts/missing-normalization-prompt.yaml",
                1,
            ),
        )
        .expect("manifest should be writable");

        let compiled =
            requirements(&root, "prospect-fit-or-brief").expect("requirements should respond");
        assert_eq!(compiled["valid"], false);
        assert_eq!(compiled["status"], "invalid");
        assert_eq!(compiled["available"], false);
        assert_eq!(compiled["product_foundation"]["status"], "unassessed");
        assert!(
            compiled["diagnostics"]
                .as_array()
                .expect("diagnostics should be an array")
                .iter()
                .any(|issue| issue["code"] == "decision_input_normalization_prompt_missing")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_pack_preserves_json_when_foundation_cards_are_unreadable() {
        let root = temporary_clay_example("missing-foundation-card");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["cards"][0]["path"] =
            serde_yaml::Value::String("cards/missing-declared-card.yaml".to_string());
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let compiled =
            requirements(&root, "prospect-fit-or-brief").expect("invalid pack should respond");

        assert_eq!(compiled["valid"], false);
        assert_eq!(compiled["status"], "invalid");
        assert_eq!(compiled["available"], false);
        assert_eq!(compiled["product_foundation"]["status"], "blocked");
        assert!(
            compiled["product_foundation"]["diagnostics"]
                .as_array()
                .expect("foundation diagnostics should be an array")
                .iter()
                .any(|diagnostic| diagnostic["code"] == "product_foundation_resolution_failed")
        );
        assert!(
            compiled["diagnostics"]
                .as_array()
                .expect("validation diagnostics should be an array")
                .iter()
                .any(|diagnostic| diagnostic["code"] == "card_read_failed")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn generated_briefs_and_traces_do_not_stale_requirements() {
        let root = temporary_clay_example("generated-artifacts");
        let before = requirements(&root, "prospect-fit-or-brief")
            .expect("requirements should compile before generated artifacts");

        std::fs::create_dir_all(root.join(".mdp/briefs"))
            .expect("brief output directory should be created");
        std::fs::create_dir_all(root.join(".mdp/traces"))
            .expect("trace output directory should be created");
        std::fs::write(root.join(".mdp/briefs/prospect.json"), "{}\n")
            .expect("generated brief should be writable");
        std::fs::write(root.join(".mdp/traces/run.json"), "{}\n")
            .expect("generated trace should be writable");

        let after = requirements(&root, "prospect-fit-or-brief")
            .expect("requirements should compile after generated artifacts");
        assert_eq!(after["pack"]["sha256"], before["pack"]["sha256"]);
        assert_eq!(after["requirements_sha256"], before["requirements_sha256"]);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn job_input_contract_ids_keep_the_same_shape_when_available() {
        let root = clay_example_root();
        let compiled =
            requirements(&root, "prospect-fit-or-brief").expect("requirements should compile");

        assert!(
            compiled["job"]["input_contracts"]
                .as_array()
                .expect("input contracts should be an array")
                .iter()
                .all(Value::is_string)
        );
        assert!(
            compiled["job"]["resolved_input_contracts"]
                .as_array()
                .expect("resolved input contracts should be an array")
                .iter()
                .all(Value::is_object)
        );
        assert_eq!(compiled["product_foundation"]["status"], "unassessed");
    }

    #[test]
    fn requirements_exposes_complete_selected_foundation_without_optional_leakage() {
        let root = temporary_clay_example("selected-foundation");
        add_product_foundation(&root, "target-identity");

        let compiled =
            requirements(&root, "prospect-fit-or-brief").expect("requirements should compile");

        assert_eq!(compiled["product_foundation"]["status"], "ready");
        assert_eq!(
            compiled["product_foundation"]["selected_facets"][0]["id"],
            "selected-identity"
        );
        assert_eq!(
            compiled["product_foundation"]["selected_facets"][0]["entry_refs"][0],
            json!({"card_id": "positioning", "entry_id": "target-identity"})
        );
        assert!(
            compiled["product_foundation"]["selected_facets"][0]["entries"][0]["body"]
                .as_str()
                .is_some_and(|body| !body.is_empty())
        );
        assert_eq!(
            compiled["product_foundation"]["optional_facet_ids"],
            json!(["optional-gap"])
        );
        assert_eq!(
            compiled["product_foundation"]["selected_facets"]
                .as_array()
                .expect("selected facets")
                .len(),
            1
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn requirements_preserves_blocked_resolution_for_foundation_only_invalidity() {
        let root = temporary_clay_example("invalid-foundation");
        add_product_foundation(&root, "missing-entry");

        let compiled =
            requirements(&root, "prospect-fit-or-brief").expect("requirements should respond");

        assert_eq!(compiled["valid"], false);
        assert_eq!(compiled["status"], "invalid");
        assert_eq!(compiled["product_foundation"]["status"], "blocked");
        assert_eq!(
            compiled["product_foundation"]["selected_facets"][0]["entry_refs"][0],
            json!({"card_id": "positioning", "entry_id": "missing-entry"})
        );
        assert!(
            compiled["product_foundation"]["diagnostics"]
                .as_array()
                .expect("foundation diagnostics")
                .iter()
                .any(|diagnostic| {
                    diagnostic["code"] == "product_foundation_selected_reference_dangling"
                })
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn requirements_selected_job_ignores_other_job_foundation_errors() {
        let root = temporary_clay_example("unrelated-foundation-error");
        add_product_foundation(&root, "target-identity");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["jobs"][1]["product_foundation"] = serde_yaml::from_str(
            r#"
required:
  - selected-identity
conditional:
  - facet_id: selected-identity
    when:
      fact: unsupported_fact
      equals: outbound-copy-brief
"#,
        )
        .expect("foundation binding should parse");
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let compiled =
            requirements(&root, "prospect-fit-or-brief").expect("requirements should compile");

        assert_eq!(compiled["valid"], true);
        assert_ne!(compiled["status"], "invalid");
        assert_eq!(compiled["available"], true);
        assert_eq!(compiled["product_foundation"]["status"], "ready");
        assert!(
            compiled["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .all(|diagnostic| {
                    diagnostic["code"] != "product_foundation_condition_fact_unknown"
                })
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn requirements_unknown_job_preserves_error_contract() {
        let root = clay_example_root();

        let error = requirements(&root, "write something persuasive")
            .expect_err("unknown canonical job should remain an error");

        assert_eq!(
            error.to_string(),
            "unknown profile job write something persuasive"
        );
    }

    #[test]
    fn generated_gtm_jobs_share_the_signal_aware_prospect_contract() {
        let root = generated_gtm_pack("generated-dic");

        for job_id in [
            "prospect-fit-or-brief",
            "outbound-copy-brief",
            "outbound-copy-review",
        ] {
            let compiled = requirements(&root, job_id).expect("requirements should compile");
            assert_eq!(compiled["valid"], true, "{job_id}");
            assert_eq!(compiled["available"], true, "{job_id}");
            assert_eq!(compiled["runtime_contract_version"], "v2", "{job_id}");
            assert_eq!(
                compiled["normalized_output_schema"]["properties"]["contract"]["const"],
                "mdp.normalized-decision-input.v2",
                "{job_id}"
            );
            assert_eq!(
                compiled["decision_input_contracts"][0]["id"],
                "gtm.prospect-context"
            );
            assert_eq!(compiled["decision_input_contracts"][0]["version"], "1.0.0");
            assert!(
                compiled["requirements_sha256"]
                    .as_str()
                    .is_some_and(|receipt| receipt.len() == 64),
                "{job_id}"
            );
        }

        let scenarios: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("examples/decision-input-scenarios.json"))
                .expect("scenario fixture should load"),
        )
        .expect("scenario fixture should be valid JSON");
        let ids = scenarios["scenarios"]
            .as_array()
            .expect("scenarios should be an array")
            .iter()
            .filter_map(|scenario| scenario["id"].as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            ids,
            BTreeSet::from([
                "attempted-complete",
                "insufficient",
                "disqualified",
                "human-review",
                "malformed",
                "provider-error",
            ])
        );
        assert!(
            scenarios["scenarios"]
                .as_array()
                .expect("scenarios should be an array")
                .iter()
                .all(|scenario| scenario["draft_allowed"] == false)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_unbound_job_does_not_infer_a_contract_from_legacy_or_prompt_fields() {
        let root = generated_gtm_pack("legacy-unbound");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should read");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["decision_input_contracts"] = serde_yaml::Value::Sequence(Vec::new());
        manifest["input_contracts"][0]["decision_input_contracts"] =
            serde_yaml::Value::Sequence(Vec::new());
        for job in manifest["jobs"]
            .as_sequence_mut()
            .expect("jobs should be an array")
        {
            job["decision_input_contracts"] = serde_yaml::Value::Sequence(Vec::new());
        }
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should write");

        let compiled = requirements(&root, "prospect-fit-or-brief")
            .expect("legacy requirements should respond");
        assert_eq!(compiled["valid"], true);
        assert_eq!(compiled["available"], false);
        assert_eq!(compiled["status"], "unavailable");
        assert_eq!(compiled["decision_input_contracts"], json!([]));
        assert!(
            compiled["diagnostics"]
                .as_array()
                .expect("diagnostics should be an array")
                .iter()
                .any(|diagnostic| diagnostic["code"] == "decision_input_contract_not_bound")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn blocked_foundation_preserves_contracts_but_blocks_drafting() {
        let root = temporary_clay_example("selected-gap");
        add_product_foundation(&root, "target-identity");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["jobs"][0]["product_foundation"]["required"] =
            serde_yaml::from_str("- selected-identity\n- optional-gap\n")
                .expect("required facets should parse");
        manifest["jobs"][0]["product_foundation"]["optional"] =
            serde_yaml::from_str("[]\n").expect("optional facets should parse");
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let compiled =
            requirements(&root, "prospect-fit-or-brief").expect("requirements should compile");

        assert_eq!(compiled["valid"], true);
        assert_eq!(compiled["available"], true);
        assert_eq!(compiled["status"], "blocked");
        assert_eq!(compiled["draft_allowed"], false);
        assert_eq!(compiled["product_foundation"]["status"], "blocked");
        assert!(
            compiled["decision_input_contracts"]
                .as_array()
                .is_some_and(|contracts| !contracts.is_empty())
        );
        assert!(compiled["normalized_output_schema"].is_object());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn computed_profile_activation_blocks_drafting_without_hiding_contracts() {
        let root = temporary_clay_example("computed-activation-blocked");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile_eval"]["activation"]["status"] =
            serde_yaml::Value::String("ready".to_string());
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
        let eval_path = root.join(".mdp/evals/account-context-missing.yaml");
        let raw = std::fs::read_to_string(&eval_path).expect("eval should be readable");
        std::fs::write(
            &eval_path,
            raw.replace("category: account-context-missing", "category: proceed"),
        )
        .expect("eval should be writable");

        let compiled =
            requirements(&root, "prospect-fit-or-brief").expect("requirements should compile");

        assert_eq!(compiled["valid"], true);
        assert_eq!(compiled["available"], true);
        assert_eq!(compiled["status"], "blocked");
        assert_eq!(compiled["draft_allowed"], false);
        assert_eq!(compiled["profile_activation"]["status"], "blocked");
        assert!(compiled["normalized_output_schema"].is_object());
        assert!(
            compiled["diagnostics"]
                .as_array()
                .expect("diagnostics should be an array")
                .iter()
                .any(|diagnostic| diagnostic["code"] == "profile_eval_category_missing")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn profile_activation_blocks_drafting_without_hiding_contracts() {
        let root = temporary_clay_example("activation-blocked");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile_eval"]["activation"]["status"] =
            serde_yaml::Value::String("needs-review".to_string());
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let compiled =
            requirements(&root, "prospect-fit-or-brief").expect("requirements should compile");

        assert_eq!(compiled["valid"], true);
        assert_eq!(compiled["available"], true);
        assert_eq!(compiled["status"], "blocked");
        assert_eq!(compiled["draft_allowed"], false);
        assert_eq!(compiled["profile_activation"]["status"], "blocked");
        assert!(compiled["normalized_output_schema"].is_object());
        assert!(
            compiled["diagnostics"]
                .as_array()
                .expect("diagnostics should be an array")
                .iter()
                .any(|diagnostic| diagnostic["code"] == "profile_activation_not_ready")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn clay_example_expected_outcomes_are_complete_and_no_draft() {
        let root = clay_example_root();
        let outcomes: Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("fixtures/expected-outcomes.json"))
                .expect("expected-outcomes fixture should load"),
        )
        .expect("expected-outcomes fixture should be valid JSON");
        let cases = outcomes["cases"]
            .as_array()
            .expect("expected outcomes should be an array");
        let ids = cases
            .iter()
            .filter_map(|case| case["expected_outcome"].as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            ids,
            BTreeSet::from([
                "ready",
                "lineage-validated",
                "blocked",
                "insufficient-context",
                "disqualified",
                "human-review",
                "malformed",
                "provider-error",
            ])
        );
        assert!(cases.iter().all(|case| case["draft_allowed"] == false));
    }
}
