use crate::commands::requirements::requirements;
use crate::constants::{
    REQUIREMENTS_CONTRACT, SOURCE_BINDING_CONTRACT, SOURCE_BINDING_VALIDATION_CONTRACT,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceBinding {
    contract: String,
    binding_release: String,
    job_id: String,
    pack: PackPin,
    requirements: RequirementsPin,
    normalization_release: String,
    #[serde(rename = "status_translation")]
    _status_translation: StatusTranslation,
    bindings: Vec<AttributeBinding>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackPin {
    id: String,
    version: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequirementsPin {
    contract: String,
    sha256: String,
    decision_input_contracts: Vec<ContractPin>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct ContractPin {
    id: String,
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusTranslation {
    #[serde(rename = "missing_value")]
    _missing_value: String,
    #[serde(rename = "null_value")]
    _null_value: String,
    #[serde(rename = "empty_string")]
    _empty_string: String,
    #[serde(rename = "whitespace_only")]
    _whitespace_only: String,
    #[serde(rename = "false_value")]
    _false_value: String,
    #[serde(rename = "zero_value")]
    _zero_value: String,
    #[serde(rename = "inapplicable")]
    _inapplicable: String,
    #[serde(rename = "inaccessible")]
    _inaccessible: String,
    #[serde(rename = "unmapped")]
    _unmapped: String,
    #[serde(rename = "runtime_failure")]
    _runtime_failure: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AttributeBinding {
    decision_input_contract_id: String,
    attribute_id: String,
    requirement: String,
    source: BindingSource,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingSource {
    #[serde(rename = "field_key")]
    _field_key: String,
    source_class: String,
    #[serde(rename = "system_of_record")]
    _system_of_record: String,
    #[serde(rename = "acquisition_mode")]
    _acquisition_mode: String,
}

#[derive(Debug)]
struct ExpectedAttribute {
    requirement: String,
    source_classes: BTreeSet<String>,
}

pub(crate) fn validate_source_binding_file(
    root: &Path,
    job_id: &str,
    file: &Path,
) -> Result<Value> {
    if binding_is_inside_pack(root, file) {
        let compiled = requirements(root, job_id)?;
        return Ok(validation_result(
            false,
            compiled["available"] == true,
            "invalid",
            &compiled,
            vec![issue(
                "source_binding_inside_pack",
                file.display().to_string(),
                "source bindings are integration-owned artifacts and must remain outside .mdp",
            )],
        ));
    }
    let raw = fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let value: Value =
        serde_json::from_str(&raw).with_context(|| format!("parsing {}", file.display()))?;
    validate_source_binding_value(root, job_id, &value, &file.display().to_string())
}

fn binding_is_inside_pack(root: &Path, file: &Path) -> bool {
    let Ok(pack_root) = root.join(crate::constants::DEFAULT_DIR).canonicalize() else {
        return false;
    };
    let Ok(binding_path) = file.canonicalize() else {
        return false;
    };
    binding_path.starts_with(pack_root)
}

fn validate_source_binding_value(
    root: &Path,
    job_id: &str,
    value: &Value,
    artifact_path: &str,
) -> Result<Value> {
    let compiled = requirements(root, job_id)?;
    if compiled["available"] != true {
        return Ok(validation_result(
            false,
            false,
            "unavailable",
            &compiled,
            vec![issue(
                "source_binding_requirements_unavailable",
                artifact_path,
                "the selected job does not compile an available Decision Input Contract",
            )],
        ));
    }

    if let Err(error) = jsonschema::draft202012::validate(&source_binding_schema(), value) {
        return Ok(validation_result(
            false,
            true,
            "invalid",
            &compiled,
            vec![issue(
                "source_binding_schema_invalid",
                artifact_path,
                format!("source binding does not match mdp.source-binding.v1: {error}"),
            )],
        ));
    }
    let binding: SourceBinding =
        serde_json::from_value(value.clone()).context("deserializing validated source binding")?;
    let mut diagnostics = Vec::new();

    check_equal(
        &mut diagnostics,
        "source_binding_contract_mismatch",
        format!("{artifact_path}#/contract"),
        &binding.contract,
        SOURCE_BINDING_CONTRACT,
        "source binding contract",
    );
    check_equal(
        &mut diagnostics,
        "source_binding_job_mismatch",
        format!("{artifact_path}#/job_id"),
        &binding.job_id,
        job_id,
        "job id",
    );
    check_equal(
        &mut diagnostics,
        "source_binding_pack_id_mismatch",
        format!("{artifact_path}#/pack/id"),
        &binding.pack.id,
        compiled["pack"]["id"].as_str().unwrap_or_default(),
        "pack id",
    );
    check_equal(
        &mut diagnostics,
        "source_binding_pack_version_mismatch",
        format!("{artifact_path}#/pack/version"),
        &binding.pack.version,
        compiled["pack"]["version"].as_str().unwrap_or_default(),
        "pack version",
    );
    check_equal(
        &mut diagnostics,
        "source_binding_pack_sha256_mismatch",
        format!("{artifact_path}#/pack/sha256"),
        &binding.pack.sha256,
        compiled["pack"]["sha256"].as_str().unwrap_or_default(),
        "portable pack digest",
    );
    check_equal(
        &mut diagnostics,
        "source_binding_requirements_contract_mismatch",
        format!("{artifact_path}#/requirements/contract"),
        &binding.requirements.contract,
        REQUIREMENTS_CONTRACT,
        "requirements contract",
    );
    check_equal(
        &mut diagnostics,
        "source_binding_requirements_sha256_mismatch",
        format!("{artifact_path}#/requirements/sha256"),
        &binding.requirements.sha256,
        compiled["requirements_sha256"].as_str().unwrap_or_default(),
        "requirements digest",
    );

    let expected_contracts = compiled["decision_input_contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|contract| {
            Some(ContractPin {
                id: contract["id"].as_str()?.to_string(),
                version: contract["version"].as_str()?.to_string(),
            })
        })
        .collect::<BTreeSet<_>>();
    let received_contracts = binding
        .requirements
        .decision_input_contracts
        .into_iter()
        .collect::<BTreeSet<_>>();
    if expected_contracts != received_contracts {
        diagnostics.push(issue(
            "source_binding_contract_receipts_mismatch",
            format!("{artifact_path}#/requirements/decision_input_contracts"),
            "Decision Input Contract ID/version receipts must exactly match the selected job",
        ));
    }

    let expected_attributes = expected_attributes(&compiled);
    let mut seen = BTreeSet::new();
    for (index, item) in binding.bindings.iter().enumerate() {
        let key = (
            item.decision_input_contract_id.clone(),
            item.attribute_id.clone(),
        );
        let item_path = format!("{artifact_path}#/bindings/{index}");
        if !seen.insert(key.clone()) {
            diagnostics.push(issue(
                "source_binding_duplicate_requirement",
                item_path,
                format!("duplicate binding for {} / {}", key.0, key.1),
            ));
            continue;
        }
        let Some(expected) = expected_attributes.get(&key) else {
            diagnostics.push(issue(
                "source_binding_unknown_requirement",
                item_path,
                format!("unknown binding for {} / {}", key.0, key.1),
            ));
            continue;
        };
        if item.requirement != expected.requirement {
            diagnostics.push(issue(
                "source_binding_requirement_mismatch",
                format!("{item_path}/requirement"),
                format!(
                    "requirement class must be {}; received {}",
                    expected.requirement, item.requirement
                ),
            ));
        }
        if !expected.source_classes.contains(&item.source.source_class) {
            diagnostics.push(issue(
                "source_binding_source_class_incompatible",
                format!("{item_path}/source/source_class"),
                format!(
                    "source class {} is not allowed; expected one of {}",
                    item.source.source_class,
                    expected
                        .source_classes
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
    }
    for key in expected_attributes
        .keys()
        .filter(|key| !seen.contains(*key))
    {
        diagnostics.push(issue(
            "source_binding_requirement_missing",
            format!("{artifact_path}#/bindings"),
            format!("missing binding for {} / {}", key.0, key.1),
        ));
    }

    let valid = diagnostics.is_empty();
    let mut result = validation_result(
        valid,
        true,
        if valid { "ready" } else { "invalid" },
        &compiled,
        diagnostics,
    );
    result["integration_releases"] = json!({
        "binding": binding.binding_release,
        "normalization": binding.normalization_release
    });
    result["coverage"] = json!({
        "required_binding_count": expected_attributes.len(),
        "received_binding_count": binding.bindings.len(),
        "unique_requirement_count": seen.len(),
        "field_key_reuse_allowed": true
    });
    Ok(result)
}

fn expected_attributes(compiled: &Value) -> BTreeMap<(String, String), ExpectedAttribute> {
    let mut expected = BTreeMap::new();
    for contract in compiled["decision_input_contracts"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let contract_id = contract["id"].as_str().unwrap_or_default();
        for attribute in contract["attributes"].as_array().into_iter().flatten() {
            let Some(attribute_id) = attribute["id"].as_str() else {
                continue;
            };
            let source_classes = attribute["source_classes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            expected.insert(
                (contract_id.to_string(), attribute_id.to_string()),
                ExpectedAttribute {
                    requirement: attribute["requirement"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    source_classes,
                },
            );
        }
    }
    expected
}

fn validation_result(
    valid: bool,
    available: bool,
    status: &str,
    compiled: &Value,
    diagnostics: Vec<Value>,
) -> Value {
    json!({
        "contract": SOURCE_BINDING_VALIDATION_CONTRACT,
        "status": status,
        "valid": valid,
        "available": available,
        "pack": compiled["pack"],
        "job": compiled["job"],
        "requirements_sha256": compiled["requirements_sha256"],
        "diagnostics": diagnostics,
        "boundaries": {
            "mdp_owns": ["requirements", "schema", "digests", "validation"],
            "integration_owns": ["source-access", "provider-credentials", "orchestration", "normalization-execution", "field-storage", "record-results"],
            "network_calls": false,
            "model_calls": false
        }
    })
}

fn check_equal(
    diagnostics: &mut Vec<Value>,
    code: &str,
    path: String,
    actual: &str,
    expected: &str,
    label: &str,
) {
    if actual != expected {
        diagnostics.push(issue(
            code,
            path,
            format!("{label} must be {expected}; received {actual}"),
        ));
    }
}

fn issue(code: &str, path: impl Into<String>, message: impl Into<String>) -> Value {
    json!({
        "code": code,
        "severity": "error",
        "path": path.into(),
        "message": message.into()
    })
}

pub(crate) fn source_binding_schema() -> Value {
    let non_blank = || json!({"type": "string", "minLength": 1, "pattern": ".*\\S.*"});
    let sha256 = || json!({"type": "string", "pattern": "^[0-9a-f]{64}$"});
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Source Binding v1",
        "description": "Provider-neutral integration mapping for one exact compiled MDP job. This artifact is integration-owned and must remain outside the pack.",
        "type": "object",
        "additionalProperties": false,
        "required": ["contract", "binding_release", "job_id", "pack", "requirements", "normalization_release", "status_translation", "bindings"],
        "properties": {
            "contract": {"const": SOURCE_BINDING_CONTRACT},
            "binding_release": non_blank(),
            "job_id": non_blank(),
            "pack": {
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "version", "sha256"],
                "properties": {
                    "id": non_blank(),
                    "version": non_blank(),
                    "sha256": sha256()
                }
            },
            "requirements": {
                "type": "object",
                "additionalProperties": false,
                "required": ["contract", "sha256", "decision_input_contracts"],
                "properties": {
                    "contract": {"const": REQUIREMENTS_CONTRACT},
                    "sha256": sha256(),
                    "decision_input_contracts": {
                        "type": "array",
                        "minItems": 1,
                        "uniqueItems": true,
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["id", "version"],
                            "properties": {
                                "id": non_blank(),
                                "version": non_blank()
                            }
                        }
                    }
                }
            },
            "normalization_release": non_blank(),
            "status_translation": {
                "type": "object",
                "additionalProperties": false,
                "required": ["missing_value", "null_value", "empty_string", "whitespace_only", "false_value", "zero_value", "inapplicable", "inaccessible", "unmapped", "runtime_failure"],
                "properties": {
                    "missing_value": {"const": "not_found"},
                    "null_value": {"const": "not_found"},
                    "empty_string": {"const": "not_found"},
                    "whitespace_only": {"const": "not_found"},
                    "false_value": {"const": "observed"},
                    "zero_value": {"const": "observed"},
                    "inapplicable": {"const": "not_applicable"},
                    "inaccessible": {"const": "blocked"},
                    "unmapped": {"const": "blocked"},
                    "runtime_failure": {"const": "error"}
                }
            },
            "bindings": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["decision_input_contract_id", "attribute_id", "requirement", "source"],
                    "properties": {
                        "decision_input_contract_id": non_blank(),
                        "attribute_id": non_blank(),
                        "requirement": {"enum": ["required", "optional", "conditional", "hard-gate"]},
                        "source": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["field_key", "source_class", "system_of_record", "acquisition_mode"],
                            "properties": {
                                "field_key": non_blank(),
                                "source_class": {"enum": ["user_provided", "customer_system", "reviewed_internal", "public_web", "synthetic_fixture"]},
                                "system_of_record": non_blank(),
                                "acquisition_mode": non_blank()
                            }
                        }
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{source_binding_schema, validate_source_binding_value};
    use crate::commands::requirements::requirements;
    use serde_json::{Value, json};
    use std::path::PathBuf;

    #[test]
    fn schema_fixes_status_translation_and_keeps_provider_names_open() {
        let schema = source_binding_schema();
        assert_eq!(
            schema["properties"]["status_translation"]["properties"]["runtime_failure"]["const"],
            "error"
        );
        assert_eq!(
            schema["properties"]["bindings"]["items"]["properties"]["source"]["properties"]["system_of_record"]
                ["type"],
            "string"
        );
    }

    #[test]
    fn complete_binding_validates_and_field_keys_may_repeat() {
        let root = example_root();
        let compiled =
            requirements(&root, "prospect-fit-or-brief").expect("requirements should compile");
        let value = complete_binding(&compiled);
        let result =
            validate_source_binding_value(&root, "prospect-fit-or-brief", &value, "binding.json")
                .expect("binding validation should run");
        assert_eq!(result["valid"], true, "{result:#}");
    }

    #[test]
    fn stale_digest_and_missing_binding_fail_closed() {
        let root = example_root();
        let compiled =
            requirements(&root, "prospect-fit-or-brief").expect("requirements should compile");
        let mut value = complete_binding(&compiled);
        value["pack"]["sha256"] = json!("0".repeat(64));
        value["bindings"].as_array_mut().unwrap().pop();
        let result =
            validate_source_binding_value(&root, "prospect-fit-or-brief", &value, "binding.json")
                .expect("binding validation should run");
        let codes = result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["code"].as_str())
            .collect::<Vec<_>>();
        assert!(codes.contains(&"source_binding_pack_sha256_mismatch"));
        assert!(codes.contains(&"source_binding_requirement_missing"));
    }

    #[test]
    fn duplicate_unknown_and_incompatible_bindings_fail() {
        let root = example_root();
        let compiled =
            requirements(&root, "prospect-fit-or-brief").expect("requirements should compile");
        let mut value = complete_binding(&compiled);
        let duplicate = value["bindings"][0].clone();
        value["bindings"].as_array_mut().unwrap().push(duplicate);
        value["bindings"][1]["attribute_id"] = json!("unknown_attribute");
        value["bindings"][5]["source"]["source_class"] = json!("public_web");

        let result =
            validate_source_binding_value(&root, "prospect-fit-or-brief", &value, "binding.json")
                .expect("binding validation should run");
        let codes = result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["code"].as_str())
            .collect::<Vec<_>>();
        assert!(codes.contains(&"source_binding_duplicate_requirement"));
        assert!(codes.contains(&"source_binding_unknown_requirement"));
        assert!(codes.contains(&"source_binding_source_class_incompatible"));
        assert!(codes.contains(&"source_binding_requirement_missing"));
    }

    #[test]
    fn qualified_keys_keep_same_attribute_id_distinct_across_contracts() {
        let compiled = json!({
            "decision_input_contracts": [
                {
                    "id": "contract.one",
                    "attributes": [{
                        "id": "shared",
                        "requirement": "required",
                        "source_classes": ["user_provided"]
                    }]
                },
                {
                    "id": "contract.two",
                    "attributes": [{
                        "id": "shared",
                        "requirement": "optional",
                        "source_classes": ["customer_system"]
                    }]
                }
            ]
        });
        let expected = super::expected_attributes(&compiled);
        assert_eq!(expected.len(), 2);
        assert!(expected.contains_key(&("contract.one".into(), "shared".into())));
        assert!(expected.contains_key(&("contract.two".into(), "shared".into())));
    }

    #[test]
    fn shipped_adapter_fixtures_validate_against_current_release_pins() {
        let root = example_root();
        for name in [
            "source-binding-clay-adapter.json",
            "source-binding-record-grid.json",
        ] {
            let path = root.join("fixtures").join(name);
            let raw = std::fs::read_to_string(&path).expect("binding fixture should be readable");
            let value: Value =
                serde_json::from_str(&raw).expect("binding fixture should be valid JSON");
            let result =
                validate_source_binding_value(&root, "prospect-fit-or-brief", &value, name)
                    .expect("binding validation should run");
            assert_eq!(result["valid"], true, "{name}: {result:#}");
        }
    }

    #[test]
    fn bindings_inside_pack_are_detected_as_wrong_ownership() {
        let root = example_root();
        assert!(super::binding_is_inside_pack(
            &root,
            &root.join(".mdp/manifest.yaml")
        ));
        assert!(!super::binding_is_inside_pack(
            &root,
            &root.join("fixtures/source-binding-record-grid.json")
        ));
    }

    fn complete_binding(compiled: &Value) -> Value {
        let contracts = compiled["decision_input_contracts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|contract| json!({"id": contract["id"], "version": contract["version"]}))
            .collect::<Vec<_>>();
        let mut bindings = Vec::new();
        for contract in compiled["decision_input_contracts"].as_array().unwrap() {
            for attribute in contract["attributes"].as_array().unwrap() {
                bindings.push(json!({
                    "decision_input_contract_id": contract["id"],
                    "attribute_id": attribute["id"],
                    "requirement": attribute["requirement"],
                    "source": {
                        "field_key": "shared.synthetic.field",
                        "source_class": attribute["source_classes"][0],
                        "system_of_record": "synthetic-record-store",
                        "acquisition_mode": "fixture-read"
                    }
                }));
            }
        }
        json!({
            "contract": "mdp.source-binding.v1",
            "binding_release": "synthetic-binding-1",
            "job_id": compiled["job"]["id"],
            "pack": {
                "id": compiled["pack"]["id"],
                "version": compiled["pack"]["version"],
                "sha256": compiled["pack"]["sha256"]
            },
            "requirements": {
                "contract": "mdp.requirements.v1",
                "sha256": compiled["requirements_sha256"],
                "decision_input_contracts": contracts
            },
            "normalization_release": "synthetic-normalizer-1",
            "status_translation": {
                "missing_value": "not_found",
                "null_value": "not_found",
                "empty_string": "not_found",
                "whitespace_only": "not_found",
                "false_value": "observed",
                "zero_value": "observed",
                "inapplicable": "not_applicable",
                "inaccessible": "blocked",
                "unmapped": "blocked",
                "runtime_failure": "error"
            },
            "bindings": bindings
        })
    }

    fn example_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../examples/clay-audiences-self-serve-enterprise-expansion")
    }
}
