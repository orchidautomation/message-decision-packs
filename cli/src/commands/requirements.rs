use crate::cli::SchemaTarget;
use crate::commands::health::validate_pack;
use crate::commands::schemas::schema;
use crate::constants::{NORMALIZED_DECISION_INPUT_CONTRACT, REQUIREMENTS_CONTRACT};
use crate::models::{
    DecisionInputAttemptStatus, DecisionInputAttribute, DecisionInputCondition,
    DecisionInputConditionOperator, DecisionInputContract, DecisionInputDisposition,
    DecisionInputRequirement, DecisionInputSourceClass, Manifest, ValueContract,
};
use crate::pack_io::read_manifest;
use anyhow::{Result, anyhow};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) fn requirements(root: &Path, job_id: &str) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let validation = validate_pack(root)?;
    if validation["valid"] != true {
        return Ok(json!({
            "contract": REQUIREMENTS_CONTRACT,
            "status": "invalid",
            "valid": false,
            "available": false,
            "pack": pack_summary(&manifest),
            "job": {
                "id": job_id,
                "input_contracts": [],
                "decision_input_contracts": []
            },
            "decision_input_contracts": [],
            "diagnostics": validation["issues"]
        }));
    }
    let job = manifest
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .ok_or_else(|| anyhow!("unknown profile job {job_id}"))?;
    let selected_input_contracts = job
        .input_contracts
        .iter()
        .map(|id| {
            manifest
                .input_contracts
                .iter()
                .find(|contract| contract.id == *id)
                .ok_or_else(|| anyhow!("job {job_id} references missing input contract {id}"))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut selected_ids = job
        .decision_input_contracts
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for input_contract in &selected_input_contracts {
        selected_ids.extend(input_contract.decision_input_contracts.iter().cloned());
    }
    let selected_contracts = selected_ids
        .iter()
        .map(|id| {
            manifest
                .decision_input_contracts
                .iter()
                .find(|contract| contract.id == *id)
                .ok_or_else(|| {
                    anyhow!("job {job_id} references missing decision input contract {id}")
                })
        })
        .collect::<Result<Vec<_>>>()?;
    if selected_contracts.is_empty() {
        return Ok(json!({
            "contract": REQUIREMENTS_CONTRACT,
            "status": "unavailable",
            "valid": true,
            "available": false,
            "pack": pack_summary(&manifest),
            "job": {
                "id": &job.id,
                "skill_id": &job.skill_id,
                "input_contracts": &job.input_contracts
            },
            "decision_input_contracts": [],
            "diagnostics": [{
                "code": "decision_input_contract_not_bound",
                "severity": "info",
                "message": "This job has no decision input contract. Existing fit/readiness behavior remains available through lead_input_requirements."
            }]
        }));
    }

    let compiled_contracts = selected_contracts
        .iter()
        .map(|contract| compile_contract(contract))
        .collect::<Vec<_>>();
    let normalized_schema = normalized_envelope_schema(job_id, &selected_contracts);
    let source_attempt_schema = source_attempt_request_schema(job_id, &selected_contracts);
    Ok(json!({
        "contract": REQUIREMENTS_CONTRACT,
        "status": "ready",
        "valid": true,
        "available": true,
        "pack": pack_summary(&manifest),
        "job": {
            "id": &job.id,
            "skill_id": &job.skill_id,
            "input_contracts": &job.input_contracts,
            "resolved_input_contracts": selected_input_contracts,
            "decision_input_contracts": selected_ids
        },
        "decision_input_contracts": compiled_contracts,
        "source_attempt_request_schema": source_attempt_schema,
        "normalized_output_schema": normalized_schema,
        "normalized_prospect_schema": schema(SchemaTarget::Prospect),
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
        "diagnostics": []
    }))
}

fn pack_summary(manifest: &Manifest) -> Value {
    json!({
        "id": &manifest.id,
        "name": &manifest.name,
        "version": &manifest.version,
        "format": &manifest.format
    })
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
    json!({
        "id": &contract.id,
        "version": &contract.version,
        "description": &contract.description,
        "normalization": &contract.normalization,
        "source_classes": source_classes,
        "attempt_statuses": DecisionInputAttemptStatus::ALL,
        "attributes": attributes
    })
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
                DecisionInputDisposition::Gap,
            ),
            (
                DecisionInputAttemptStatus::Error,
                DecisionInputDisposition::Gap,
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
    behavior
}

fn source_attempt_request_schema(job_id: &str, contracts: &[&DecisionInputContract]) -> Value {
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
                "required": ["attempt_id", "attribute_id", "source_class"],
                "additionalProperties": false,
                "properties": {
                    "attempt_id": {"type": "string", "pattern": "\\S"},
                    "attribute_id": {"const": &attribute.id},
                    "source_class": {"enum": &attribute.source_classes},
                    "source_locator": {"type": "string"},
                    "requested_at": {"type": "string", "format": "date-time"}
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
        "required": ["contract", "job_id", "decision_input_contracts", "attempts"],
        "additionalProperties": false,
        "properties": {
            "contract": {"const": "mdp.source-attempt-request.v1"},
            "job_id": {"const": job_id},
            "decision_input_contracts": {
                "type": "array",
                "const": contract_versions
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

fn normalized_envelope_schema(job_id: &str, contracts: &[&DecisionInputContract]) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    let mut ready_outcome_guards = Vec::new();
    for contract in contracts {
        for attribute in &contract.attributes {
            properties.insert(attribute.id.clone(), attempt_result_schema(attribute));
            required.push(Value::String(attribute.id.clone()));
            if let Some(guard) = ready_outcome_guard(attribute) {
                ready_outcome_guards.push(guard);
            }
            if let Some(guard) = applies_when_ready_outcome_guard(attribute) {
                ready_outcome_guards.push(guard);
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
    let mut required = vec!["status"];
    if attribute.provenance.required {
        required.push("provenance");
    }
    if attribute.confidence.required {
        required.push("confidence");
    }
    if attribute.freshness.required {
        required.push("freshness");
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
        "required": required,
        "additionalProperties": false,
        "properties": {
            "status": {"enum": DecisionInputAttemptStatus::ALL},
            "value": value_contract_json_schema(&attribute.value),
            "provenance": {
                "type": "array",
                "minItems": if attribute.provenance.required { 1 } else { 0 },
                "items": {
                    "type": "object",
                    "required": provenance_required,
                    "additionalProperties": false,
                    "properties": {
                        "attempt_id": {"type": "string", "pattern": "\\S"},
                        "source_class": {"enum": &attribute.source_classes},
                        "source_locator": {"type": "string"},
                        "observed_at": {"type": "string", "format": "date-time"},
                        "excerpt": {"type": "string"}
                    }
                }
            },
            "confidence": {
                "type": "integer",
                "minimum": confidence_minimum,
                "maximum": 100
            },
            "freshness": {
                "type": "object",
                "required": freshness_required,
                "additionalProperties": false,
                "properties": {
                    "observed_at": {"type": "string", "format": "date-time"},
                    "age_days": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": freshness_maximum
                    }
                }
            },
            "error": {"type": "string"}
        },
        "allOf": [{
            "if": {"properties": {"status": {"const": "observed"}}},
            "then": {"required": ["value"]},
            "else": {"not": {"required": ["value"]}}
        }]
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
    Value::Object(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        DecisionInputConfidencePolicy, DecisionInputFreshnessPolicy, DecisionInputProvenanceField,
        DecisionInputProvenancePolicy, DecisionInputSensitivity, DecisionInputSourceClass,
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
    fn optional_provider_error_remains_explicit_but_does_not_block() {
        let attribute = DecisionInputAttribute {
            requirement: DecisionInputRequirement::Optional,
            ..DecisionInputAttribute::default()
        };

        assert_eq!(
            effective_status_behavior(&attribute)[&DecisionInputAttemptStatus::Error],
            DecisionInputDisposition::Gap
        );
        assert!(
            ready_outcome_guard(&attribute).is_none(),
            "optional gaps must not prevent a ready normalization outcome"
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

        assert_eq!(
            schema["required"],
            json!(["status", "provenance", "confidence", "freshness"])
        );
        assert_eq!(schema["properties"]["provenance"]["minItems"], 1);
        assert_eq!(schema["properties"]["confidence"]["minimum"], 90);
        assert_eq!(
            schema["properties"]["freshness"]["properties"]["age_days"]["maximum"],
            30
        );
        assert_eq!(
            schema["properties"]["freshness"]["required"],
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
            unknown_allowed_schema["properties"]["freshness"]["required"],
            json!(["observed_at"])
        );
        draft202012::validate(&unknown_allowed_schema, &missing_age)
            .expect("allow_unknown should preserve acceptance when age_days is absent");
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
        draft202012::validate(&compiled["source_attempt_request_schema"], &request)
            .expect("exact source-attempt fixture should satisfy the compiled schema");
        draft202012::validate(&compiled["normalized_output_schema"], &response)
            .expect("exact normalized response fixture should satisfy the compiled schema");

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
            draft202012::validate(&schema, &invalid_ready).expect(
                "applied conditional not_applicable remains valid with a non-ready outcome",
            );
        }
    }

    #[test]
    fn conditional_not_applicable_remains_ready_when_condition_does_not_apply() {
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
        let conditional_attribute = contract
            .attributes
            .iter_mut()
            .find(|candidate| candidate.id == "current_working_country")
            .expect("fixture contract should include current_working_country");
        conditional_attribute.applies_when = vec![DecisionInputCondition {
            attribute: "person_title".to_string(),
            operator: DecisionInputConditionOperator::Equals,
            values: vec!["Chief Financial Officer".to_string()],
        }];

        let schema = normalized_envelope_schema("prospect-fit-or-brief", &[&contract]);
        let mut ready = response;
        let current_country = ready["attributes"]["current_working_country"]
            .as_object_mut()
            .expect("current_working_country should be an object");
        current_country.insert("status".to_string(), json!("not_applicable"));
        current_country.remove("value");
        draft202012::validate(&schema, &ready)
            .expect("conditional not_applicable should stay ready when applies_when is false");
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
