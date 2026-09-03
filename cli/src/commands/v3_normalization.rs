//! Profile-neutral semantic normalization v3 (MDP-287).
//!
//! The v3 wire is the first normalized decision-input contract whose
//! canonical `normalized_input` payload is profile neutral. The model emits a
//! bounded semantic-only provider payload (`classifications`, `gaps`,
//! `rejected_claims`) bound to one canonical classification taxonomy per
//! classified attribute. The host seals every host-owned field on top of the
//! validated semantic payload before downstream deterministic evaluation.
//!
//! This module owns:
//!
//! - The semantic provider payload types and JSON Schema.
//! - The sealed v3 envelope JSON Schema (canonical contract).
//! - Validation that rejects host-field injection, unknown enums, mixed
//!   v3/legacy representations, malformed payloads, and incompatible
//!   structured-output schemas.
//! - Deterministic projection to the private neutral `normalized_input`
//!   surface used by `decision_input`.
//!
//! v0/v1/v2 envelopes remain readable through their existing validators.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use crate::constants::{
    CLASSIFICATION_TAXONOMY_CONTRACT_V3, NORMALIZED_DECISION_INPUT_CONTRACT_V3,
    NORMALIZED_SEMANTIC_PROVIDER_SCHEMA_REF_V3, REQUIREMENTS_HASH_LABEL_V3,
    TAXONOMY_SET_HASH_LABEL_V3, V3_AMBIGUITY_POLICY_HUMAN_REVIEW, V3_BASIS_MAX_CHARS_HARD_LIMIT,
    V3_CLASSIFICATION_STATUS_AMBIGUOUS, V3_CLASSIFICATION_STATUS_CLASSIFIED,
    V3_CLASSIFICATION_STATUS_NO_MATCH, V3_CLASSIFICATION_STATUS_UNSUPPORTED,
    V3_CONFLICT_POLICY_HUMAN_REVIEW, V3_IDENTIFIER_MAX_LEN, V3_MAX_CLASSIFICATIONS_PER_ENVELOPE,
    V3_MAX_DERIVED_FROM_PER_CLASSIFICATION, V3_MAX_GAPS_PER_ENVELOPE,
    V3_MAX_REJECTED_CLAIMS_PER_ENVELOPE, V3_MAX_TAXONOMY_CONTRIBUTORS, V3_MAX_TAXONOMY_VALUES,
    V3_NO_MATCH_POLICY_GAP,
};
use crate::models::{
    ClassificationTaxonomy as ClassificationTaxonomyV3, NORMALIZATION_HOST_ENVELOPE_CONTRACT,
    NORMALIZATION_HOST_ENVELOPE_OWNED_FIELDS,
};
use crate::run_contracts::DiagnosticDetailV1;

#[cfg(test)]
use crate::models::{
    ClassificationAmbiguityPolicy, ClassificationConflictPolicy, ClassificationMinimumEvidence,
    ClassificationNoMatchPolicy, ClassificationTaxonomyValue as ClassificationTaxonomyValueV3,
    DecisionInputSourceClass,
};

// =============================================================================
// Closed enums and bounded identifiers for the v3 semantic payload.
// =============================================================================

/// Closed set of classification statuses the provider may emit. Anything
/// outside this set fails the v3 envelope before deterministic evaluation.
pub(crate) const V3_CLASSIFICATION_STATUSES: &[&str] = &[
    V3_CLASSIFICATION_STATUS_CLASSIFIED,
    V3_CLASSIFICATION_STATUS_AMBIGUOUS,
    V3_CLASSIFICATION_STATUS_NO_MATCH,
    V3_CLASSIFICATION_STATUS_UNSUPPORTED,
];

// =============================================================================
// Semantic-only provider payload.
// =============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticClassificationV3 {
    pub(crate) status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) value: Option<String>,
    pub(crate) taxonomy_id: String,
    pub(crate) taxonomy_version: String,
    pub(crate) derived_from: Vec<String>,
    pub(crate) basis: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticGapV3 {
    pub(crate) attribute: String,
    pub(crate) reason: String,
    #[serde(default)]
    pub(crate) derived_from: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) taxonomy_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticRejectedClaimV3 {
    pub(crate) claim: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticProviderPayloadV3 {
    pub(crate) classifications: BTreeMap<String, SemanticClassificationV3>,
    #[serde(default)]
    pub(crate) gaps: Vec<SemanticGapV3>,
    #[serde(default)]
    pub(crate) rejected_claims: Vec<SemanticRejectedClaimV3>,
}

// =============================================================================
// Issues produced by v3 validation.
// =============================================================================

/// Stable, machine-readable diagnostic produced by v3 validators. The CLI
/// projects these into actionable diagnostics and `--v3` audit output.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct V3Issue {
    pub(crate) code: &'static str,
    pub(crate) path: String,
    pub(crate) expected: String,
    pub(crate) observed: String,
}

impl V3Issue {
    fn new(code: &'static str, path: impl Into<String>, expected: &str, observed: &str) -> Self {
        Self {
            code,
            path: path.into(),
            expected: expected.into(),
            observed: observed.into(),
        }
    }
}

const V3_DIAGNOSTIC_PATH_MAX_CHARS: usize = 256;
const V3_DIAGNOSTIC_TOKEN_MAX_CHARS: usize = 96;

/// Convert the first local JSON Schema error into a bounded, content-free
/// diagnostic. `ValidationError::to_string()` is intentionally not used:
/// it can include a rejected value, a property name supplied by the model,
/// or provider-specific implementation text.
pub(crate) fn v3_schema_error_detail(
    schema: &Value,
    value: &Value,
    code: &str,
) -> DiagnosticDetailV1 {
    if let Some(detail) = jsonschema::draft202012::new(schema)
        .ok()
        .and_then(|validator| {
            validator.iter_errors(value).next().map(|error| {
                let (path, expected, observed) = schema_error_fields(&error);
                DiagnosticDetailV1 {
                    code: safe_diagnostic_token(code, "v3-schema-invalid"),
                    path,
                    expected: expected.into(),
                    observed: observed.into(),
                }
            })
        })
    {
        detail
    } else {
        DiagnosticDetailV1 {
            code: safe_diagnostic_token(code, "v3-schema-invalid"),
            path: "$".into(),
            expected: "schema-constraint".into(),
            observed: "unavailable".into(),
        }
    }
}

/// JSON Schema reports an `anyOf` failure at the branch boundary and stores
/// the useful type/required/additional-property failure in its context. Walk
/// that context for a more actionable category while keeping all values and
/// schema-library prose out of the public detail record.
fn schema_error_fields(
    error: &jsonschema::ValidationError<'_>,
) -> (String, &'static str, &'static str) {
    match error.kind() {
        jsonschema::error::ValidationErrorKind::AnyOf { context }
        | jsonschema::error::ValidationErrorKind::OneOfNotValid { context }
        | jsonschema::error::ValidationErrorKind::OneOfMultipleValid { context } => context
            .iter()
            .flatten()
            .map(schema_error_fields)
            .find(|(_, expected, _)| *expected != "semantic-branch")
            .unwrap_or_else(|| schema_error_leaf_fields(error)),
        jsonschema::error::ValidationErrorKind::PropertyNames { error: nested } => {
            schema_error_fields(nested)
        }
        _ => schema_error_leaf_fields(error),
    }
}

fn schema_error_leaf_fields(
    error: &jsonschema::ValidationError<'_>,
) -> (String, &'static str, &'static str) {
    let keyword = error.kind().keyword();
    let observed = if keyword == "required" {
        "missing"
    } else if keyword == "additionalProperties" {
        "undeclared-field"
    } else {
        json_value_kind(error.instance().as_ref())
    };
    (
        safe_json_pointer_path(error.instance_path().as_str()),
        schema_keyword_expectation(keyword),
        observed,
    )
}

/// Project a semantic validator issue without copying the issue's expected or
/// observed strings. Those strings are useful to local Rust callers but may
/// contain taxonomy values, evidence identifiers, or other model-controlled
/// content that does not belong in a public run receipt.
pub(crate) fn v3_issue_diagnostic_detail(issue: &V3Issue, code: &str) -> DiagnosticDetailV1 {
    DiagnosticDetailV1 {
        code: safe_diagnostic_token(code, "v3-semantic-output-invalid"),
        path: safe_v3_issue_path(&issue.path),
        expected: issue_expected_category(issue.code).into(),
        observed: issue_observed_category(issue.code).into(),
    }
}

/// Build a detail record for a static host rejection whose validator did not
/// produce a `V3Issue`. Callers may provide only already-categorical labels.
pub(crate) fn v3_static_diagnostic_detail(
    code: &str,
    path: &str,
    expected: &str,
    observed: &str,
) -> DiagnosticDetailV1 {
    DiagnosticDetailV1 {
        code: safe_diagnostic_token(code, "v3-semantic-output-invalid"),
        path: safe_v3_issue_path(path),
        expected: safe_diagnostic_token(expected, "schema-constraint"),
        observed: safe_diagnostic_token(observed, "invalid"),
    }
}

fn safe_diagnostic_token(value: &str, fallback: &str) -> String {
    let mut token = value
        .chars()
        .take(V3_DIAGNOSTIC_TOKEN_MAX_CHARS)
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while token.contains("--") {
        token = token.replace("--", "-");
    }
    let token = token.trim_matches('-').to_string();
    if token.is_empty() {
        fallback.into()
    } else {
        token
    }
}

fn json_value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn schema_keyword_expectation(keyword: &str) -> &'static str {
    match keyword {
        "required" => "required-property",
        "type" => "json-type",
        "enum" => "allowed-value",
        "const" => "exact-value",
        "additionalProperties" | "unevaluatedProperties" => "declared-field-only",
        "anyOf" | "oneOf" => "semantic-branch",
        "minItems" | "maxItems" | "minLength" | "maxLength" | "minProperties" | "maxProperties"
        | "uniqueItems" => "bounded-value",
        _ => "schema-constraint",
    }
}

fn issue_expected_category(code: &str) -> &'static str {
    match code {
        "v3_classification_invalid_status" | "v3_classification_unknown_value" => "allowed-value",
        "v3_classification_missing_value" => "required-property",
        "v3_classification_forbidden_value" => "property-absent",
        "v3_classification_missing_derived_from" => "required-evidence",
        "v3_classification_derived_from_overflow" | "v3_classification_envelope_overflow" => {
            "bounded-value"
        }
        "v3_classification_unknown_taxonomy" => "selected-taxonomy",
        "v3_classification_unknown_evidence_ref" | "v3_gap_unknown_evidence_ref" => {
            "collected-attempt-id"
        }
        "v3_classification_unknown_attribute" | "v3_gap_unknown_attribute" => {
            "compiled-attribute-id"
        }
        "v3_classification_basis_empty" | "v3_rejected_claim_empty" => "non-empty-string",
        "v3_classification_basis_too_long" => "bounded-string",
        "v3_semantic_payload_malformed" => "semantic-object",
        "v3_output_not_object" | "v3_envelope_not_object" => "json-object",
        _ => "schema-constraint",
    }
}

fn issue_observed_category(code: &str) -> &'static str {
    match code {
        "v3_classification_missing_value" | "v3_classification_missing_derived_from" => "missing",
        "v3_classification_forbidden_value" => "present",
        "v3_classification_basis_empty" | "v3_rejected_claim_empty" => "empty",
        "v3_classification_derived_from_overflow"
        | "v3_classification_envelope_overflow"
        | "v3_classification_basis_too_long" => "over-limit",
        "v3_semantic_payload_malformed" => "malformed",
        "v3_output_not_object" | "v3_envelope_not_object" => "non-object",
        "v3_classification_unknown_taxonomy"
        | "v3_classification_unknown_value"
        | "v3_classification_unknown_evidence_ref"
        | "v3_classification_unknown_attribute"
        | "v3_gap_unknown_evidence_ref"
        | "v3_gap_unknown_attribute"
        | "v3_classification_invalid_status" => "unrecognized",
        _ => "invalid",
    }
}

fn safe_json_pointer_path(pointer: &str) -> String {
    let segments = pointer
        .strip_prefix('/')
        .unwrap_or(pointer)
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.replace("~1", "/").replace("~0", "~"))
        .collect::<Vec<_>>();
    safe_path_segments(&segments)
}

fn safe_v3_issue_path(path: &str) -> String {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut chars = path.strip_prefix('$').unwrap_or(path).chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '.' => {
                if !current.is_empty() {
                    segments.push(std::mem::take(&mut current));
                }
            }
            '[' => {
                if !current.is_empty() {
                    segments.push(std::mem::take(&mut current));
                }
                let mut bracket = String::new();
                while let Some(next) = chars.next() {
                    if next == ']' {
                        break;
                    }
                    bracket.push(next);
                }
                if !bracket.is_empty() {
                    segments.push(bracket);
                }
            }
            _ => current.push(character),
        }
    }
    if !current.is_empty() {
        segments.push(current);
    }
    safe_path_segments(&segments)
}

fn safe_path_segments(segments: &[String]) -> String {
    let mut output = String::from("$");
    for (index, segment) in segments.iter().enumerate() {
        let safe = if index > 0
            && segments.get(index - 1).map(String::as_str) == Some("classifications")
        {
            // Classification map keys originate in provider output. Keep the
            // structural path but not an attacker-controlled key, including
            // keys made only from digits that could otherwise look like an
            // array index.
            "*"
        } else if segment.chars().all(|character| character.is_ascii_digit()) && segment.len() <= 6
        {
            segment.as_str()
        } else if is_safe_path_field(segment) {
            segment.as_str()
        } else {
            "*"
        };
        output.push('/');
        output.push_str(safe);
        if output.chars().count() >= V3_DIAGNOSTIC_PATH_MAX_CHARS {
            output = output.chars().take(V3_DIAGNOSTIC_PATH_MAX_CHARS).collect();
            break;
        }
    }
    output
}

fn is_safe_path_field(segment: &str) -> bool {
    matches!(
        segment,
        "contract"
            | "job_id"
            | "decision_input_contracts"
            | "normalization"
            | "requirements_sha256"
            | "taxonomy_set_sha256"
            | "source_binding_sha256"
            | "source_attempt_request_sha256"
            | "collected_attempt_results_sha256"
            | "invocation_receipt_sha256"
            | "attributes"
            | "classifications"
            | "signal_observations"
            | "normalized_input"
            | "fields"
            | "signals"
            | "gaps"
            | "rejected_claims"
            | "outcome"
            | "status"
            | "value"
            | "taxonomy_id"
            | "taxonomy_version"
            | "derived_from"
            | "basis"
            | "attribute"
            | "reason"
            | "claim"
    )
}

/// Reasons a v3 envelope seals successfully but the deterministic decision
/// evaluator should not produce a ready route. The runtime uses these to
/// build the deterministic read paths without consuming semantic authority.
#[allow(dead_code)] // Reserved for the deterministic decision integration lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum V3DeterministicReadiness {
    /// Sealed envelope is ready for downstream evaluation.
    Ready,
    /// Classification marked ambiguous or no-match: ready route must wait.
    ProceedWithoutDecision,
    /// Some classifications marked unsupported or inputs are missing.
    InsufficientContext,
}

// =============================================================================
// Canonical v3 schema constructors.
// =============================================================================

/// JSON Schema for the bounded semantic-only provider payload. Holds only the
/// three fixed semantic fields. Anything else in provider output is an
/// injection that the host rejects before sealing.
pub(crate) fn v3_semantic_provider_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Semantic Normalization v3 Provider Payload",
        "type": "object",
        "additionalProperties": false,
        "required": ["classifications", "gaps", "rejected_claims"],
        "properties": {
            "classifications": {
                "type": "object",
                "maxProperties": V3_MAX_CLASSIFICATIONS_PER_ENVELOPE,
                "additionalProperties": v3_classification_object_schema()
            },
            "gaps": {
                "type": "array",
                "maxItems": V3_MAX_GAPS_PER_ENVELOPE,
                "items": v3_gap_object_schema()
            },
            "rejected_claims": {
                "type": "array",
                "maxItems": V3_MAX_REJECTED_CLAIMS_PER_ENVELOPE,
                "items": v3_rejected_claim_object_schema()
            }
        }
    })
}

fn v3_classification_object_schema() -> Value {
    let common = json!({
        "taxonomy_id": {
            "type": "string",
            "pattern": "^[A-Za-z][A-Za-z0-9_-]*$",
            "minLength": 1,
            "maxLength": V3_IDENTIFIER_MAX_LEN
        },
        "taxonomy_version": {
            "type": "string",
            "minLength": 1,
            "maxLength": V3_IDENTIFIER_MAX_LEN
        },
        "derived_from": {
            "type": "array",
            "minItems": 1,
            "maxItems": V3_MAX_DERIVED_FROM_PER_CLASSIFICATION,
            "uniqueItems": true,
            "items": {
                "type": "string",
                "minLength": 1,
                "maxLength": V3_IDENTIFIER_MAX_LEN
            }
        },
        "basis": {
            "type": "string",
            "minLength": 1,
            "maxLength": V3_BASIS_MAX_CHARS_HARD_LIMIT
        }
    });
    json!({
        "anyOf": [
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["status", "value", "taxonomy_id", "taxonomy_version", "derived_from", "basis"],
                "properties": {
                    "status": { "type": "string", "const": V3_CLASSIFICATION_STATUS_CLASSIFIED },
                    "value": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": V3_IDENTIFIER_MAX_LEN
                    },
                    "taxonomy_id": common["taxonomy_id"].clone(),
                    "taxonomy_version": common["taxonomy_version"].clone(),
                    "derived_from": common["derived_from"].clone(),
                    "basis": common["basis"].clone()
                }
            },
            {
                "type": "object",
                "additionalProperties": false,
                "required": ["status", "taxonomy_id", "taxonomy_version", "derived_from", "basis"],
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": [
                            V3_CLASSIFICATION_STATUS_AMBIGUOUS,
                            V3_CLASSIFICATION_STATUS_NO_MATCH,
                            V3_CLASSIFICATION_STATUS_UNSUPPORTED
                        ]
                    },
                    "taxonomy_id": common["taxonomy_id"].clone(),
                    "taxonomy_version": common["taxonomy_version"].clone(),
                    "derived_from": common["derived_from"].clone(),
                    "basis": common["basis"].clone()
                }
            }
        ]
    })
}

fn v3_gap_object_schema() -> Value {
    let attribute = json!({
        "type": "string",
        "minLength": 1,
        "maxLength": V3_IDENTIFIER_MAX_LEN
    });
    let reason = json!({
        "type": "string",
        "minLength": 1,
        "maxLength": V3_IDENTIFIER_MAX_LEN
    });
    let derived_from = json!({
        "type": "array",
        "maxItems": V3_MAX_DERIVED_FROM_PER_CLASSIFICATION,
        "uniqueItems": true,
        "items": {
            "type": "string",
            "minLength": 1,
            "maxLength": V3_IDENTIFIER_MAX_LEN
        }
    });
    let taxonomy_id = json!({
        "type": "string",
        "pattern": "^[A-Za-z][A-Za-z0-9_-]*$",
        "minLength": 1,
        "maxLength": V3_IDENTIFIER_MAX_LEN
    });
    let branch = |extra: Vec<(&str, Value)>| {
        let mut properties = Map::from_iter([
            ("attribute".into(), attribute.clone()),
            ("reason".into(), reason.clone()),
        ]);
        let mut required = vec![json!("attribute"), json!("reason")];
        for (name, schema) in extra {
            properties.insert(name.into(), schema);
            required.push(json!(name));
        }
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": required,
            "properties": properties
        })
    };
    json!({
        "anyOf": [
            branch(vec![]),
            branch(vec![("derived_from", derived_from.clone())]),
            branch(vec![("taxonomy_id", taxonomy_id.clone())]),
            branch(vec![("derived_from", derived_from), ("taxonomy_id", taxonomy_id)])
        ]
    })
}

fn v3_rejected_claim_object_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["claim", "reason"],
        "properties": {
            "claim": { "type": "string", "minLength": 1 },
            "reason": { "type": "string", "minLength": 1 }
        }
    })
}

/// Canonical sealed v3 envelope JSON Schema. Differs from the provider
/// payload in that the host owns every envelope field; the schema enforces
/// the disjoint authority.
pub(crate) fn v3_sealed_envelope_schema() -> Value {
    let sha256 = json!({
        "type": "string",
        "pattern": "^[0-9a-f]{64}$",
        "minLength": 64,
        "maxLength": 64
    });
    let normalization_entry = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["contract_id", "prompt", "prompt_version"],
        "properties": {
            "contract_id": { "type": "string", "minLength": 1 },
            "prompt": { "type": "string", "minLength": 1 },
            "prompt_version": { "type": "string", "minLength": 1 },
            "prompt_sha256": sha256.clone()
        }
    });
    let attributes = json!({
        "type": "object",
        "additionalProperties": { "type": "object" }
    });
    let projected_attributes = json!({
        "type": "object",
        "additionalProperties": {
            "type": ["string", "number", "integer", "boolean", "array"]
        }
    });
    let normalized_input = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["fields", "signals", "attributes"],
        "properties": {
            "fields": { "type": "object" },
            "signals": {
                "type": "array",
                "items": { "type": "object" }
            },
            "attributes": projected_attributes
        }
    });
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Normalized Decision Input v3 Sealed Envelope",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "contract",
            "job_id",
            "decision_input_contracts",
            "normalization",
            "requirements_sha256",
            "taxonomy_set_sha256",
            "source_binding_sha256",
            "source_attempt_request_sha256",
            "collected_attempt_results_sha256",
            "invocation_receipt_sha256",
            "attributes",
            "classifications",
            "signal_observations",
            "normalized_input",
            "gaps",
            "rejected_claims",
            "outcome"
        ],
        "properties": {
            "contract": { "const": NORMALIZED_DECISION_INPUT_CONTRACT_V3 },
            "job_id": { "type": "string", "minLength": 1 },
            "decision_input_contracts": {
                "type": "array",
                "minItems": 1,
                "items": { "type": "string", "minLength": 1 }
            },
            "normalization": {
                "type": "array",
                "minItems": 1,
                "items": normalization_entry
            },
            "requirements_sha256": sha256.clone(),
            "taxonomy_set_sha256": sha256.clone(),
            "source_binding_sha256": sha256.clone(),
            "source_attempt_request_sha256": sha256.clone(),
            "collected_attempt_results_sha256": sha256.clone(),
            "invocation_receipt_sha256": sha256.clone(),
            "attributes": attributes,
            "classifications": {
                "type": "object",
                "maxProperties": V3_MAX_CLASSIFICATIONS_PER_ENVELOPE,
                "additionalProperties": v3_classification_object_schema()
            },
            "signal_observations": {
                "type": "array",
                "items": { "type": "object" }
            },
            "normalized_input": normalized_input,
            "gaps": {
                "type": "array",
                "maxItems": V3_MAX_GAPS_PER_ENVELOPE,
                "items": v3_gap_object_schema()
            },
            "rejected_claims": {
                "type": "array",
                "maxItems": V3_MAX_REJECTED_CLAIMS_PER_ENVELOPE,
                "items": v3_rejected_claim_object_schema()
            },
            "outcome": { "type": "string" }
        }
    })
}

/// Schema for the authored classification taxonomy. Packs publish this so the
/// compiler can read it back. U1 (MDP-286) owns the manifest authoring
/// surface. This module only exposes the schema to keep U3 clean.
pub(crate) fn v3_classification_taxonomy_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Classification Taxonomy v3",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "id", "version", "output_attribute", "contributor_attribute_ids",
            "source_classes", "minimum_evidence", "basis_max_chars",
            "ambiguity_policy", "no_match_policy", "conflict_policy", "values"
        ],
        "properties": {
            "id": { "type": "string", "pattern": "^[A-Za-z][A-Za-z0-9_-]*$" },
            "version": { "type": "string", "minLength": 1, "maxLength": V3_IDENTIFIER_MAX_LEN },
            "output_attribute": { "type": "string", "pattern": "^[A-Za-z][A-Za-z0-9_-]*$", "maxLength": V3_IDENTIFIER_MAX_LEN },
            "contributor_attribute_ids": {
                "type": "array",
                "minItems": 1,
                "maxItems": V3_MAX_TAXONOMY_CONTRIBUTORS,
                "uniqueItems": true,
                "items": { "type": "string", "pattern": "^[A-Za-z][A-Za-z0-9_-]*$", "maxLength": V3_IDENTIFIER_MAX_LEN }
            },
            "source_classes": {
                "type": "array",
                "minItems": 1,
                "maxItems": 5,
                "uniqueItems": true,
                "items": { "enum": ["user_provided", "customer_system", "reviewed_internal", "public_web", "synthetic_fixture"] }
            },
            "minimum_evidence": {
                "type": "object",
                "additionalProperties": false,
                "required": ["observed_contributors"],
                "properties": {
                    "observed_contributors": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": V3_MAX_TAXONOMY_CONTRIBUTORS
                    }
                }
            },
            "basis_max_chars": { "type": "integer", "minimum": 1, "maximum": V3_BASIS_MAX_CHARS_HARD_LIMIT },
            "ambiguity_policy": { "const": V3_AMBIGUITY_POLICY_HUMAN_REVIEW },
            "no_match_policy": { "const": V3_NO_MATCH_POLICY_GAP },
            "conflict_policy": { "const": V3_CONFLICT_POLICY_HUMAN_REVIEW },
            "values": {
                "type": "array",
                "minItems": 1,
                "maxItems": V3_MAX_TAXONOMY_VALUES,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["value", "definition"],
                    "properties": {
                        "value": { "type": "string", "minLength": 1 },
                        "definition": { "type": "string", "minLength": 1 },
                        "positive_indicators": { "type": "array", "uniqueItems": true, "items": { "type": "string", "minLength": 1 } },
                        "exclusions": { "type": "array", "uniqueItems": true, "items": { "type": "string", "minLength": 1 } }
                    }
                }
            }
        }
    })
}

// =============================================================================
// Provider output hardening: reject host-field injection and mixed v3/legacy.
// =============================================================================

/// Reject provider output that tries to set host-owned top-level fields or
/// that carries any of the legacy v0-v2 aliases. Both cases fail closed.
pub(crate) fn reject_host_field_injection(provider_output: &Value) -> Result<(), V3Issue> {
    let object = provider_output
        .as_object()
        .ok_or_else(|| V3Issue::new("v3_output_not_object", "$", "object", "non-object"))?;
    if object.contains_key("normalized_prospect") {
        return Err(V3Issue::new(
            "v3_legacy_alias_paired_with_v3",
            "$.normalized_prospect",
            "absent or v3-input only",
            "present",
        ));
    }
    if object.contains_key("normalized_opportunity") {
        return Err(V3Issue::new(
            "v3_legacy_alias_paired_with_v3",
            "$.normalized_opportunity",
            "absent or v3-input only",
            "present",
        ));
    }
    for forbidden in [
        "outcome",
        "draft_allowed",
        "prompt_id",
        "prompt_sha256",
        "invocation_receipt_sha256",
        "context_sha256",
    ] {
        if object.contains_key(forbidden) {
            return Err(V3Issue::new(
                "v3_provider_authority_field",
                format!("$.{forbidden}"),
                "absent",
                "present",
            ));
        }
    }
    for field in NORMALIZATION_HOST_ENVELOPE_OWNED_FIELDS {
        if object.contains_key(*field) {
            return Err(V3Issue::new(
                "v3_host_owned_field_injection",
                format!("$.{field}"),
                "absent",
                "present",
            ));
        }
    }
    Ok(())
}

/// OpenAI's strict schema subset does not carry `uniqueItems` through the
/// provider projection. Attempt references are set-like semantic evidence,
/// so remove repeated string IDs before applying the canonical schema. Keep
/// malformed non-string entries untouched so the schema still rejects them.
pub(crate) fn normalize_v3_semantic_reference_arrays(value: &mut Value) {
    if let Some(classifications) = value
        .get_mut("classifications")
        .and_then(Value::as_object_mut)
    {
        for classification in classifications.values_mut() {
            deduplicate_v3_reference_array(classification.get_mut("derived_from"));
        }
    }
    if let Some(gaps) = value.get_mut("gaps").and_then(Value::as_array_mut) {
        for gap in gaps {
            deduplicate_v3_reference_array(gap.get_mut("derived_from"));
        }
    }
}

fn deduplicate_v3_reference_array(value: Option<&mut Value>) {
    let Some(Value::Array(items)) = value else {
        return;
    };
    let mut seen = std::collections::BTreeSet::new();
    items.retain(|item| {
        item.as_str()
            .is_none_or(|attempt_id| seen.insert(attempt_id.to_owned()))
    });
}

// =============================================================================
// Validators for v3 sealed input.
// =============================================================================

/// Validate a sealed v3 envelope against the canonical schema plus the
/// disjoint authority checks. The runtime calls this after the provider
/// output and host-owned fields are merged.
pub(crate) fn validate_v3_sealed_envelope(value: &Value) -> Result<(), Vec<V3Issue>> {
    let mut issues = Vec::new();
    let Some(object) = value.as_object() else {
        issues.push(V3Issue::new(
            "v3_envelope_not_object",
            "$",
            "object",
            "non-object",
        ));
        return Err(issues);
    };

    if object.get("contract").and_then(Value::as_str) != Some(NORMALIZED_DECISION_INPUT_CONTRACT_V3)
    {
        issues.push(V3Issue::new(
            "v3_envelope_contract_mismatch",
            "$.contract",
            NORMALIZED_DECISION_INPUT_CONTRACT_V3,
            object
                .get("contract")
                .and_then(Value::as_str)
                .unwrap_or("<missing>"),
        ));
    }
    if object.contains_key("normalized_prospect") {
        issues.push(V3Issue::new(
            "v3_legacy_alias_paired_with_v3",
            "$.normalized_prospect",
            "absent",
            "present",
        ));
    }
    if object.contains_key("normalized_opportunity") {
        issues.push(V3Issue::new(
            "v3_legacy_alias_paired_with_v3",
            "$.normalized_opportunity",
            "absent",
            "present",
        ));
    }
    if !object.contains_key("normalized_input") {
        issues.push(V3Issue::new(
            "v3_envelope_missing_neutral_input",
            "$.normalized_input",
            "object",
            "<missing>",
        ));
    }
    let schema = v3_sealed_envelope_schema();
    if jsonschema::draft202012::validate(&schema, value).is_err() {
        let detail = v3_schema_error_detail(&schema, value, "v3-sealed-envelope-schema-mismatch");
        issues.push(V3Issue::new(
            "v3_envelope_schema_mismatch",
            detail.path,
            detail.expected.as_str(),
            detail.observed.as_str(),
        ));
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

/// Validate the semantic payload against the selected classification
/// taxonomies and a trusted set of attempt identifiers. Failures are stable
/// diagnostic codes surfaced to the runtime CLI.
pub(crate) fn validate_v3_semantic_payload(
    payload: &Value,
    selected_taxonomies: &[ClassificationTaxonomyV3],
    observed_attribute_ids: &[String],
    known_attempt_ids: &[String],
) -> Result<(), Vec<V3Issue>> {
    let mut issues = Vec::new();
    if let Err(issue) = reject_host_field_injection(payload) {
        issues.push(issue);
    }

    let semantic = match parse_semantic_payload(payload) {
        Ok(parsed) => parsed,
        Err(error) => {
            issues.push(error);
            return Err(issues);
        }
    };

    if semantic.classifications.len() > V3_MAX_CLASSIFICATIONS_PER_ENVELOPE {
        issues.push(V3Issue::new(
            "v3_classification_envelope_overflow",
            "$.classifications",
            "at most 32 entries",
            semantic.classifications.len().to_string().as_str(),
        ));
    }

    let taxonomy_index: std::collections::HashMap<(String, String), &ClassificationTaxonomyV3> =
        selected_taxonomies
            .iter()
            .map(|taxonomy| ((taxonomy.id.clone(), taxonomy.version.clone()), taxonomy))
            .collect();
    let mut seen_attribute_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for (attribute_id, classification) in &semantic.classifications {
        if !seen_attribute_ids.insert(attribute_id.clone()) {
            issues.push(V3Issue::new(
                "v3_classification_duplicate_attribute",
                format!("$.classifications.{attribute_id}"),
                "unique attribute id",
                "duplicate",
            ));
        }
        if !V3_CLASSIFICATION_STATUSES.contains(&classification.status.as_str()) {
            issues.push(V3Issue::new(
                "v3_classification_invalid_status",
                format!("$.classifications.{attribute_id}.status"),
                "closed enum value",
                classification.status.as_str(),
            ));
        }
        let key = (
            classification.taxonomy_id.clone(),
            classification.taxonomy_version.clone(),
        );
        let Some(taxonomy) = taxonomy_index.get(&key) else {
            issues.push(V3Issue::new(
                "v3_classification_unknown_taxonomy",
                format!("$.classifications.{attribute_id}"),
                "selected taxonomy id + version",
                format!(
                    "{}@{}",
                    classification.taxonomy_id, classification.taxonomy_version
                )
                .as_str(),
            ));
            continue;
        };
        if classification.status == V3_CLASSIFICATION_STATUS_CLASSIFIED {
            if classification.value.is_none() {
                issues.push(V3Issue::new(
                    "v3_classification_missing_value",
                    format!("$.classifications.{attribute_id}.value"),
                    "non-empty value",
                    "<missing>",
                ));
                continue;
            }
            let value = classification.value.as_ref().expect("present above");
            let allowed = taxonomy.canonical_values();
            if !allowed.iter().any(|v| v == value) {
                issues.push(V3Issue::new(
                    "v3_classification_unknown_value",
                    format!("$.classifications.{attribute_id}.value"),
                    &format!("one of {:?}", allowed),
                    value.as_str(),
                ));
                continue;
            }
        } else if classification.value.is_some() {
            issues.push(V3Issue::new(
                "v3_classification_forbidden_value",
                format!("$.classifications.{attribute_id}.value"),
                "<missing>",
                classification.value.as_deref().unwrap_or("<null>"),
            ));
        }
        if classification.derived_from.is_empty() {
            issues.push(V3Issue::new(
                "v3_classification_missing_derived_from",
                format!("$.classifications.{attribute_id}.derived_from"),
                "non-empty",
                "[]",
            ));
        }
        if classification.derived_from.len() > V3_MAX_DERIVED_FROM_PER_CLASSIFICATION {
            issues.push(V3Issue::new(
                "v3_classification_derived_from_overflow",
                format!("$.classifications.{attribute_id}.derived_from"),
                "at most 16 ids",
                classification.derived_from.len().to_string().as_str(),
            ));
        }
        for attempt_id in &classification.derived_from {
            if !known_attempt_ids.iter().any(|id| id == attempt_id) {
                issues.push(V3Issue::new(
                    "v3_classification_unknown_evidence_ref",
                    format!("$.classifications.{attribute_id}.derived_from"),
                    "collected attempt_id",
                    attempt_id.as_str(),
                ));
            }
        }
        if classification.basis.trim().is_empty() {
            issues.push(V3Issue::new(
                "v3_classification_basis_empty",
                format!("$.classifications.{attribute_id}.basis"),
                "non-empty bounded basis",
                "<empty>",
            ));
        } else if classification.basis.chars().count() > taxonomy.basis_max_chars {
            issues.push(V3Issue::new(
                "v3_classification_basis_too_long",
                format!("$.classifications.{attribute_id}.basis"),
                &format!("<= {} chars", taxonomy.basis_max_chars),
                classification.basis.chars().count().to_string().as_str(),
            ));
        }
        if !observed_attribute_ids.iter().any(|id| id == attribute_id) {
            issues.push(V3Issue::new(
                "v3_classification_unknown_attribute",
                format!("$.classifications.{attribute_id}"),
                "compiled decision input attribute id",
                attribute_id.as_str(),
            ));
        }
    }

    for (index, gap) in semantic.gaps.iter().enumerate() {
        if !observed_attribute_ids.iter().any(|id| id == &gap.attribute) {
            issues.push(V3Issue::new(
                "v3_gap_unknown_attribute",
                format!("$.gaps[{index}].attribute"),
                "compiled decision input attribute id",
                gap.attribute.as_str(),
            ));
        }
        for attempt_id in &gap.derived_from {
            if !known_attempt_ids.iter().any(|id| id == attempt_id) {
                issues.push(V3Issue::new(
                    "v3_gap_unknown_evidence_ref",
                    format!("$.gaps[{index}].derived_from"),
                    "collected attempt_id",
                    attempt_id.as_str(),
                ));
            }
        }
    }
    for claim in &semantic.rejected_claims {
        if claim.claim.trim().is_empty() {
            issues.push(V3Issue::new(
                "v3_rejected_claim_empty",
                "$.rejected_claims[*].claim",
                "non-empty",
                "<empty>",
            ));
        }
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

fn parse_semantic_payload(value: &Value) -> Result<SemanticProviderPayloadV3, V3Issue> {
    serde_json::from_value::<SemanticProviderPayloadV3>(value.clone()).map_err(|_| {
        V3Issue::new(
            "v3_semantic_payload_malformed",
            "$",
            "object matching mdp.normalization-semantic-provider.v3",
            "malformed",
        )
    })
}

// =============================================================================
// Deterministic envelope sealer.
// =============================================================================

/// Build an authoritative, host-owned, sealed v3 envelope. Returns the
/// canonical envelope JSON. The runtime never accepts provider output that
/// pre-populates these fields.
pub(crate) struct V3SealInputs<'a> {
    pub(crate) job_id: &'a str,
    pub(crate) decision_input_contract_ids: &'a [String],
    pub(crate) normalization_entries: &'a [Value],
    pub(crate) requirements_sha256: &'a str,
    pub(crate) taxonomy_set_sha256: &'a str,
    pub(crate) source_binding_sha256: &'a str,
    pub(crate) source_attempt_request_sha256: &'a str,
    pub(crate) collected_attempt_results_sha256: &'a str,
    pub(crate) invocation_receipt_sha256: &'a str,
    pub(crate) attributes: &'a Map<String, Value>,
    pub(crate) classifications: &'a Map<String, Value>,
    pub(crate) signal_observations: &'a [Value],
    pub(crate) normalized_input: &'a Map<String, Value>,
    pub(crate) gaps: &'a [Value],
    pub(crate) rejected_claims: &'a [Value],
    pub(crate) outcome: &'a str,
}

pub(crate) fn seal_v3_envelope(inputs: V3SealInputs<'_>) -> Value {
    json!({
        "contract": NORMALIZED_DECISION_INPUT_CONTRACT_V3,
        "job_id": inputs.job_id,
        "decision_input_contracts": inputs.decision_input_contract_ids,
        "normalization": inputs.normalization_entries,
        REQUIREMENTS_HASH_LABEL_V3: inputs.requirements_sha256,
        TAXONOMY_SET_HASH_LABEL_V3: inputs.taxonomy_set_sha256,
        "source_binding_sha256": inputs.source_binding_sha256,
        "source_attempt_request_sha256": inputs.source_attempt_request_sha256,
        "collected_attempt_results_sha256": inputs.collected_attempt_results_sha256,
        "invocation_receipt_sha256": inputs.invocation_receipt_sha256,
        "attributes": Value::Object(inputs.attributes.clone()),
        "classifications": Value::Object(inputs.classifications.clone()),
        "signal_observations": inputs.signal_observations,
        "normalized_input": Value::Object(inputs.normalized_input.clone()),
        "gaps": inputs.gaps,
        "rejected_claims": inputs.rejected_claims,
        "outcome": inputs.outcome,
    })
}

// =============================================================================
// Provider schema preflight: invalid structured-output schemas fail closed
// before any driver invocation. This is the v3 specialization of the
// existing `project_output_schema_for_openai` preflight.
// =============================================================================

/// Project the v3 semantic-provider schema into the canonical OpenAI
/// projection. Returns an unsupported-schema error if the projected schema
/// loses required fields.
#[allow(dead_code)] // Reserved for the v3 provider schema preflight integration.
pub(crate) fn project_v3_semantic_provider_schema_for_openai() -> Result<Value, &'static str> {
    let canonical = v3_semantic_provider_schema();
    let properties = canonical
        .get("properties")
        .and_then(Value::as_object)
        .ok_or("v3-semantic-schema-properties-missing")?;
    // Reject if the contracted field set is missing or empty; the provider
    // would silently emit unsupported payloads.
    let required: Vec<String> = ["classifications", "gaps", "rejected_claims"]
        .iter()
        .map(|field| (*field).to_string())
        .collect();
    for field in &required {
        if !properties.contains_key(field) {
            return Err("v3-semantic-required-field-missing-from-schema");
        }
    }
    let projected_properties = required
        .iter()
        .filter_map(|field| {
            properties
                .get(field)
                .map(|schema| ((*field).clone(), schema.clone()))
        })
        .collect::<Map<_, _>>();
    Ok(json!({
        "type": "object",
        "properties": projected_properties,
        "required": required,
        "additionalProperties": false
    }))
}

// =============================================================================
// Identity helpers.
// =============================================================================

/// Stable identity labels used to attach run receipts and bundle bindings.
/// Kept here so that run bundles know which authority binding a v3 envelope
/// proves without copying private prose.
#[allow(dead_code)] // Reserved for receipt/bundle identity binding in the U3 integration.
pub(crate) fn v3_identity_labels() -> Vec<(&'static str, &'static str)> {
    vec![
        ("contract", NORMALIZED_DECISION_INPUT_CONTRACT_V3),
        (
            "semantic_schema",
            NORMALIZED_SEMANTIC_PROVIDER_SCHEMA_REF_V3,
        ),
        ("taxonomy_contract", CLASSIFICATION_TAXONOMY_CONTRACT_V3),
        ("host_envelope", NORMALIZATION_HOST_ENVELOPE_CONTRACT),
    ]
}

// =============================================================================
// Tests.
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_taxonomy() -> ClassificationTaxonomyV3 {
        ClassificationTaxonomyV3 {
            id: "buyer-persona".into(),
            version: "1".into(),
            output_attribute: "persona".into(),
            contributor_attribute_ids: vec!["person_title".into(), "responsibilities".into()],
            source_classes: vec![DecisionInputSourceClass::SyntheticFixture],
            minimum_evidence: ClassificationMinimumEvidence {
                observed_contributors: 1,
            },
            basis_max_chars: 500,
            ambiguity_policy: ClassificationAmbiguityPolicy::HumanReview,
            no_match_policy: ClassificationNoMatchPolicy::Gap,
            conflict_policy: ClassificationConflictPolicy::HumanReview,
            values: vec![
                ClassificationTaxonomyValueV3 {
                    value: "GTM Systems Owner".into(),
                    definition: "Owns or builds technical GTM systems.".into(),
                    positive_indicators: vec![],
                    exclusions: vec![],
                },
                ClassificationTaxonomyValueV3 {
                    value: "Quota-Carrying Seller".into(),
                    definition: "Quota-carrying seller without system ownership.".into(),
                    positive_indicators: vec![],
                    exclusions: vec![],
                },
            ],
        }
    }

    #[test]
    fn sealed_v3_envelope_rejects_legacy_alias_payload() {
        let mut sealed = seal_v3_envelope(V3SealInputs {
            job_id: "prospect-fit-or-brief",
            decision_input_contract_ids: &["gtm.prospect-context".into()],
            normalization_entries: &[json!({
                "contract_id": "gtm.prospect-context",
                "prompt": "prompts/normalize-prospect.yaml",
                "prompt_version": "gtm-prospect-context.v3",
            })],
            requirements_sha256: &"a".repeat(64),
            taxonomy_set_sha256: &"b".repeat(64),
            source_binding_sha256: &"c".repeat(64),
            source_attempt_request_sha256: &"d".repeat(64),
            collected_attempt_results_sha256: &"e".repeat(64),
            invocation_receipt_sha256: &"f".repeat(64),
            attributes: &Map::new(),
            classifications: &Map::new(),
            signal_observations: &[],
            normalized_input: &Map::from_iter([
                ("fields".into(), json!({})),
                ("signals".into(), json!([])),
                ("attributes".into(), json!({})),
            ]),
            gaps: &[],
            rejected_claims: &[],
            outcome: "ready",
        });
        sealed
            .as_object_mut()
            .unwrap()
            .insert("normalized_prospect".into(), json!({"legacy": true}));
        let result = validate_v3_sealed_envelope(&sealed);
        let error = result.unwrap_err();
        assert!(
            error
                .iter()
                .any(|issue| issue.code == "v3_legacy_alias_paired_with_v3")
        );
    }

    #[test]
    fn sealed_v3_envelope_passes_for_well_built_envelope() {
        let sealed = seal_v3_envelope(V3SealInputs {
            job_id: "prospect-fit-or-brief",
            decision_input_contract_ids: &["gtm.prospect-context".into()],
            normalization_entries: &[json!({
                "contract_id": "gtm.prospect-context",
                "prompt": "prompts/normalize-prospect.yaml",
                "prompt_version": "gtm-prospect-context.v3",
            })],
            requirements_sha256: &"a".repeat(64),
            taxonomy_set_sha256: &"b".repeat(64),
            source_binding_sha256: &"c".repeat(64),
            source_attempt_request_sha256: &"d".repeat(64),
            collected_attempt_results_sha256: &"e".repeat(64),
            invocation_receipt_sha256: &"f".repeat(64),
            attributes: &Map::new(),
            classifications: &Map::new(),
            signal_observations: &[],
            normalized_input: &Map::from_iter([
                ("fields".into(), json!({})),
                ("signals".into(), json!([])),
                ("attributes".into(), json!({})),
            ]),
            gaps: &[],
            rejected_claims: &[],
            outcome: "ready",
        });
        let validated = validate_v3_sealed_envelope(&sealed);
        if let Err(errors) = validated {
            panic!("sealed envelope unexpected errors: {errors:?}");
        }
    }

    #[test]
    fn provider_payload_rejects_host_field_injection() {
        let payload = json!({
            "contract": "mdp.normalized-decision-input.v3",
            "classifications": {}
        });
        let error = reject_host_field_injection(&payload).unwrap_err();
        assert_eq!(error.code, "v3_host_owned_field_injection");
        assert_eq!(error.path, "$.contract");
    }

    #[test]
    fn provider_payload_rejects_mixed_v3_legacy_aliases() {
        for forbidden in ["normalized_prospect", "normalized_opportunity"] {
            let payload = json!({
                "classifications": {},
                forbidden: {"legacy": true}
            });
            let error = reject_host_field_injection(&payload).unwrap_err();
            assert_eq!(error.code, "v3_legacy_alias_paired_with_v3");
        }
    }

    #[test]
    fn provider_payload_rejects_outcome_or_draft_authority() {
        for forbidden in ["outcome", "draft_allowed", "prompt_id", "prompt_sha256"] {
            let payload = json!({
                "classifications": {},
                forbidden: "x"
            });
            let error = reject_host_field_injection(&payload).unwrap_err();
            assert_eq!(error.code, "v3_provider_authority_field");
        }
    }

    #[test]
    fn semantic_payload_rejects_unknown_evidence_ref() {
        let payload = json!({
            "classifications": {
                "persona": {
                    "status": "classified",
                    "value": "GTM Systems Owner",
                    "taxonomy_id": "buyer-persona",
                    "taxonomy_version": "1",
                    "derived_from": ["synthetic-attempt-XXX"],
                    "basis": "title says it"
                }
            }
        });
        let error = validate_v3_semantic_payload(
            &payload,
            &[sample_taxonomy()],
            &["persona".into()],
            &["synthetic-attempt-001".into()],
        )
        .unwrap_err();
        assert!(
            error
                .iter()
                .any(|issue| issue.code == "v3_classification_unknown_evidence_ref")
        );
    }

    #[test]
    fn semantic_payload_rejects_unknown_taxonomy() {
        let payload = json!({
            "classifications": {
                "persona": {
                    "status": "classified",
                    "value": "GTM Systems Owner",
                    "taxonomy_id": "unknown",
                    "taxonomy_version": "1",
                    "derived_from": ["synthetic-attempt-001"],
                    "basis": "title says it"
                }
            }
        });
        let error = validate_v3_semantic_payload(
            &payload,
            &[sample_taxonomy()],
            &["persona".into()],
            &["synthetic-attempt-001".into()],
        )
        .unwrap_err();
        assert!(
            error
                .iter()
                .any(|issue| issue.code == "v3_classification_unknown_taxonomy")
        );
    }

    #[test]
    fn semantic_payload_rejects_unknown_value() {
        let payload = json!({
            "classifications": {
                "persona": {
                    "status": "classified",
                    "value": "NotInTaxonomy",
                    "taxonomy_id": "buyer-persona",
                    "taxonomy_version": "1",
                    "derived_from": ["synthetic-attempt-001"],
                    "basis": "title says it"
                }
            }
        });
        let error = validate_v3_semantic_payload(
            &payload,
            &[sample_taxonomy()],
            &["persona".into()],
            &["synthetic-attempt-001".into()],
        )
        .unwrap_err();
        assert!(
            error
                .iter()
                .any(|issue| issue.code == "v3_classification_unknown_value")
        );
    }

    #[test]
    fn semantic_payload_rejects_unknown_status() {
        let payload = json!({
            "classifications": {
                "persona": {
                    "status": "ready",
                    "value": "GTM Systems Owner",
                    "taxonomy_id": "buyer-persona",
                    "taxonomy_version": "1",
                    "derived_from": ["synthetic-attempt-001"],
                    "basis": "title says it"
                }
            }
        });
        let error = validate_v3_semantic_payload(
            &payload,
            &[sample_taxonomy()],
            &["persona".into()],
            &["synthetic-attempt-001".into()],
        )
        .unwrap_err();
        assert!(
            error
                .iter()
                .any(|issue| issue.code == "v3_classification_invalid_status")
        );
    }

    #[test]
    fn semantic_payload_rejects_basis_overflow() {
        let long_basis: String = std::iter::repeat('x').take(600).collect();
        let payload = json!({
            "classifications": {
                "persona": {
                    "status": "classified",
                    "value": "GTM Systems Owner",
                    "taxonomy_id": "buyer-persona",
                    "taxonomy_version": "1",
                    "derived_from": ["synthetic-attempt-001"],
                    "basis": long_basis
                }
            }
        });
        let error = validate_v3_semantic_payload(
            &payload,
            &[sample_taxonomy()],
            &["persona".into()],
            &["synthetic-attempt-001".into()],
        )
        .unwrap_err();
        assert!(
            error
                .iter()
                .any(|issue| issue.code == "v3_classification_basis_too_long")
        );
    }

    #[test]
    fn semantic_payload_rejects_forbidden_value_when_not_classified() {
        let payload = json!({
            "classifications": {
                "persona": {
                    "status": "ambiguous",
                    "value": "GTM Systems Owner",
                    "taxonomy_id": "buyer-persona",
                    "taxonomy_version": "1",
                    "derived_from": ["synthetic-attempt-001", "synthetic-attempt-002"],
                    "basis": "two candidate titles disagree"
                }
            }
        });
        let error = validate_v3_semantic_payload(
            &payload,
            &[sample_taxonomy()],
            &["persona".into()],
            &[
                "synthetic-attempt-001".into(),
                "synthetic-attempt-002".into(),
            ],
        )
        .unwrap_err();
        assert!(
            error
                .iter()
                .any(|issue| issue.code == "v3_classification_forbidden_value")
        );
    }

    #[test]
    fn semantic_payload_rejects_missing_value_when_classified() {
        let payload = json!({
            "classifications": {
                "persona": {
                    "status": "classified",
                    "taxonomy_id": "buyer-persona",
                    "taxonomy_version": "1",
                    "derived_from": ["synthetic-attempt-001"],
                    "basis": "title says it"
                }
            }
        });
        let error = validate_v3_semantic_payload(
            &payload,
            &[sample_taxonomy()],
            &["persona".into()],
            &["synthetic-attempt-001".into()],
        )
        .unwrap_err();
        assert!(
            error
                .iter()
                .any(|issue| issue.code == "v3_classification_missing_value")
        );
    }

    #[test]
    fn semantic_payload_rejects_empty_derived_from() {
        let payload = json!({
            "classifications": {
                "persona": {
                    "status": "classified",
                    "value": "GTM Systems Owner",
                    "taxonomy_id": "buyer-persona",
                    "taxonomy_version": "1",
                    "derived_from": [],
                    "basis": "title says it"
                }
            }
        });
        let error = validate_v3_semantic_payload(
            &payload,
            &[sample_taxonomy()],
            &["persona".into()],
            &["synthetic-attempt-001".into()],
        )
        .unwrap_err();
        assert!(
            error
                .iter()
                .any(|issue| issue.code == "v3_classification_missing_derived_from")
        );
    }

    #[test]
    fn semantic_payload_rejects_unknown_attribute() {
        let payload = json!({
            "classifications": {
                "bogus": {
                    "status": "classified",
                    "value": "GTM Systems Owner",
                    "taxonomy_id": "buyer-persona",
                    "taxonomy_version": "1",
                    "derived_from": ["synthetic-attempt-001"],
                    "basis": "title says it"
                }
            }
        });
        let error = validate_v3_semantic_payload(
            &payload,
            &[sample_taxonomy()],
            &["persona".into()],
            &["synthetic-attempt-001".into()],
        )
        .unwrap_err();
        assert!(
            error
                .iter()
                .any(|issue| issue.code == "v3_classification_unknown_attribute")
        );
    }

    #[test]
    fn semantic_payload_rejects_malformed_provider_payload() {
        let payload = json!("just-a-string");
        let error = validate_v3_semantic_payload(
            &payload,
            &[sample_taxonomy()],
            &["persona".into()],
            &["synthetic-attempt-001".into()],
        )
        .unwrap_err();
        assert!(
            error
                .iter()
                .any(|issue| issue.code == "v3_semantic_payload_malformed"
                    || issue.code == "v3_output_not_object")
        );
    }

    #[test]
    fn semantic_payload_accepts_classified_valid_status() {
        let payload = json!({
            "classifications": {
                "persona": {
                    "status": "classified",
                    "value": "GTM Systems Owner",
                    "taxonomy_id": "buyer-persona",
                    "taxonomy_version": "1",
                    "derived_from": ["synthetic-attempt-001"],
                    "basis": "title says it"
                }
            },
            "gaps": [],
            "rejected_claims": []
        });
        let result = validate_v3_semantic_payload(
            &payload,
            &[sample_taxonomy()],
            &["persona".into()],
            &["synthetic-attempt-001".into()],
        );
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    #[test]
    fn semantic_payload_accepts_no_match_status_without_value() {
        let payload = json!({
            "classifications": {
                "persona": {
                    "status": "no-match",
                    "taxonomy_id": "buyer-persona",
                    "taxonomy_version": "1",
                    "derived_from": ["synthetic-attempt-001"],
                    "basis": "no role evidence found"
                }
            }
        });
        let result = validate_v3_semantic_payload(
            &payload,
            &[sample_taxonomy()],
            &["persona".into()],
            &["synthetic-attempt-001".into()],
        );
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    #[test]
    fn provider_schema_preflight_returns_projectable_shape() {
        let projected = project_v3_semantic_provider_schema_for_openai().unwrap();
        assert_eq!(projected["type"], "object");
        assert_eq!(
            projected["required"],
            json!(["classifications", "gaps", "rejected_claims"])
        );
        assert_eq!(projected["additionalProperties"], false);
    }

    #[test]
    fn provider_schema_preflight_includes_three_fixed_semantic_fields() {
        let projected = project_v3_semantic_provider_schema_for_openai().unwrap();
        let properties = projected["properties"].as_object().unwrap();
        for field in ["classifications", "gaps", "rejected_claims"] {
            assert!(
                properties.contains_key(field),
                "missing provider property {field}"
            );
        }
    }

    #[test]
    fn provider_schema_projection_preserves_status_and_item_semantics() {
        let projected = project_v3_semantic_provider_schema_for_openai().unwrap();
        let classification = &projected["properties"]["classifications"]["additionalProperties"];
        let branches = classification["anyOf"]
            .as_array()
            .expect("classification must use explicit status branches");
        assert_eq!(branches.len(), 2);
        assert!(
            branches[0]["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field == "value")
        );
        assert!(
            !branches[1]["properties"]
                .as_object()
                .unwrap()
                .contains_key("value")
        );

        for status in ["classified", "ambiguous", "no-match", "unsupported"] {
            let mut classification_value = json!({
                "status": status,
                "taxonomy_id": "buyer-persona",
                "taxonomy_version": "1",
                "derived_from": ["attempt-1"],
                "basis": "synthetic basis"
            });
            if status == "classified" {
                classification_value["value"] = json!("GTM Systems Owner");
            }
            let payload = json!({
                "classifications": {"persona": classification_value},
                "gaps": [{"attribute": "person_title", "reason": "not found"}],
                "rejected_claims": [{"claim": "unsupported claim", "reason": "not evidenced"}]
            });
            jsonschema::draft202012::validate(&projected, &payload).unwrap_or_else(|error| {
                panic!("{status} provider payload should validate: {error}")
            });
        }

        assert!(
            jsonschema::draft202012::validate(
                &projected,
                &json!({
                    "classifications": {"persona": {
                        "status": "ambiguous",
                        "value": "GTM Systems Owner",
                        "taxonomy_id": "buyer-persona",
                        "taxonomy_version": "1",
                        "derived_from": ["attempt-1"],
                        "basis": "synthetic basis"
                    }},
                    "gaps": [],
                    "rejected_claims": []
                })
            )
            .is_err(),
            "non-classified provider status must not require or accept value"
        );

        for field in ["gaps", "rejected_claims"] {
            let items = projected["properties"][field]["items"]
                .as_object()
                .expect("semantic item schema should be explicit");
            assert!(items.contains_key("anyOf") || items.contains_key("required"));
            assert_ne!(
                items,
                &serde_json::Map::from_iter([("type".into(), json!("object"))])
            );
        }
    }

    #[test]
    fn schema_rejection_detail_is_bounded_and_content_free() {
        let schema = v3_semantic_provider_schema();
        let payload = json!({
            "classifications": {},
            "gaps": [{"attribute": 7, "reason": "raw-secret-sentinel"}],
            "rejected_claims": []
        });
        let detail = v3_schema_error_detail(&schema, &payload, "v3-semantic-output-invalid");
        assert_eq!(detail.code, "v3-semantic-output-invalid");
        assert!(detail.path.starts_with("$/"));
        assert!(detail.path.chars().count() <= V3_DIAGNOSTIC_PATH_MAX_CHARS);
        assert_eq!(detail.expected, "json-type");
        assert_eq!(detail.observed, "number");
        assert!(
            !serde_json::to_string(&detail)
                .unwrap()
                .contains("raw-secret-sentinel")
        );
    }

    #[test]
    fn v3_identity_labels_include_contract_semantic_taxonomy_and_envelope() {
        let labels: std::collections::BTreeMap<&'static str, &'static str> =
            v3_identity_labels().into_iter().collect();
        assert_eq!(
            labels.get("contract").copied(),
            Some(NORMALIZED_DECISION_INPUT_CONTRACT_V3)
        );
        assert_eq!(
            labels.get("semantic_schema").copied(),
            Some(NORMALIZED_SEMANTIC_PROVIDER_SCHEMA_REF_V3)
        );
        assert_eq!(
            labels.get("taxonomy_contract").copied(),
            Some(CLASSIFICATION_TAXONOMY_CONTRACT_V3)
        );
        assert_eq!(
            labels.get("host_envelope").copied(),
            Some(NORMALIZATION_HOST_ENVELOPE_CONTRACT)
        );
    }
}
