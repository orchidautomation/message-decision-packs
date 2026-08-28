use crate::models::{Manifest, Prospect, ValueContract};
use crate::utils::resolve_pack_persona_label;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Validate the profile-neutral value against the manifest view.  Adapters are
/// responsible for deciding which public fields enter `DecisionInput`; this
/// function intentionally has no persona or product-profile behavior.
pub(crate) fn decision_input_contract_violations(
    requirements: &crate::decision_input::RequirementsView<'_>,
    input: &crate::decision_input::DecisionInput,
) -> Vec<ContractViolation> {
    let mut violations = Vec::new();
    for field in requirements.required_fields() {
        let present = if field == "signals" {
            !input.signals().is_empty()
        } else {
            input.field(field).is_some_and(meaningful_json_value)
        };
        if !present {
            violations.push(required_violation("decision_input", field, field));
        }
    }
    for field in requirements.required_signal_fields() {
        // Legacy GTM semantics require every signal to carry every declared
        // field.  `all` is intentionally false for an empty signal list.
        if input.signals().is_empty()
            || !input
                .signals()
                .iter()
                .all(|signal| signal.get(field).is_some_and(meaningful_json_value))
        {
            violations.push(required_violation(
                "signal",
                field,
                &format!("signals/{field}"),
            ));
        }
    }
    for name in requirements.required_attributes() {
        if !input
            .attributes()
            .get(name)
            .is_some_and(meaningful_json_value)
        {
            violations.push(required_violation(
                "attribute",
                name,
                &format!("attributes/{name}"),
            ));
        }
    }
    violations.extend(decision_input_value_contract_violations(
        requirements,
        input,
        "decision_input",
        "",
        true,
    ));
    violations
}

/// Shared scalar/attribute checks used by both compatibility adapters.  The
/// caller supplies the legacy diagnostic scope/path so the public rendering
/// remains unchanged while the value being checked is the neutral input.
pub(crate) fn decision_input_value_contract_violations(
    requirements: &crate::decision_input::RequirementsView<'_>,
    input: &crate::decision_input::DecisionInput,
    scope: &'static str,
    path_prefix: &str,
    include_required: bool,
) -> Vec<ContractViolation> {
    let mut violations = Vec::new();
    for (name, contract) in requirements.value_contracts() {
        if name == "persona" {
            continue;
        }
        if let Some(value) = input
            .field(name)
            .filter(|value| meaningful_json_value(value))
        {
            validate_value(
                name,
                value,
                contract,
                &join_path(path_prefix, name),
                scope,
                &mut violations,
            );
        } else if include_required && contract.required {
            violations.push(required_violation(
                scope,
                name,
                &join_path(path_prefix, name),
            ));
        }
    }
    let attributes = input.attributes().iter().collect::<Vec<_>>();
    collect_attribute_contract_violations(
        requirements.attribute_definitions(),
        requirements.allow_undeclared_attributes(),
        &attributes,
        &join_path(path_prefix, "attributes"),
        &mut violations,
    );
    violations
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContractViolation {
    pub(crate) code: &'static str,
    pub(crate) scope: &'static str,
    pub(crate) field: String,
    pub(crate) path: String,
    pub(crate) reason: String,
}

pub(crate) const PROSPECT_CONTRACT_FIELDS: &[&str] = &[
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
];

pub(crate) fn prospect_contract_violations(
    manifest: &Manifest,
    prospect: &Prospect,
    effective_persona: Option<&str>,
) -> Vec<ContractViolation> {
    let Ok(input) = crate::decision_input::from_gtm_prospect(prospect) else {
        // The closed adapter owns wire validity.  Do not fall back to a
        // second interpretation of an input that the adapter rejected.
        return Vec::new();
    };
    let mut violations = Vec::new();
    let explicit_persona = prospect.persona.as_deref().and_then(present_str);
    let persona_contract_value = explicit_persona
        .and_then(|persona| {
            resolve_pack_persona_label(manifest, persona, "prospect.persona")
                .map(|resolution| resolution.persona)
        })
        .or_else(|| explicit_persona.map(str::to_string))
        .or_else(|| effective_persona.and_then(present_str).map(str::to_string));

    if let Some(persona) = explicit_persona {
        if resolve_pack_persona_label(manifest, persona, "prospect.persona").is_none() {
            violations.push(ContractViolation {
                code: "value_contract_persona_unrecognized",
                scope: "prospect",
                field: "persona".to_string(),
                path: "persona".to_string(),
                reason: format!(
                    "persona must match a pack-owned persona or persona_mappings alias; received {persona}; allowed personas: {}",
                    allowed_personas(manifest)
                ),
            });
        }
    }

    violations.extend(
        crate::value_contracts::decision_input_value_contract_violations(
            &crate::decision_input::requirements_view(&manifest.lead_input_requirements),
            &input,
            "prospect",
            "",
            true,
        ),
    );
    if let Some(contract) = manifest
        .lead_input_requirements
        .value_contracts
        .get("persona")
    {
        if let Some(persona) = persona_contract_value.as_deref() {
            validate_value(
                "persona",
                &Value::String(persona.to_string()),
                contract,
                "persona",
                "prospect",
                &mut violations,
            );
        } else if contract.required {
            violations.push(required_violation("prospect", "persona", "persona"));
        }
    }

    violations
}

pub(crate) fn normalized_prospect_contract_violations(
    manifest: &Manifest,
    prospect: &Map<String, Value>,
    path: &str,
) -> Vec<ContractViolation> {
    let input = if manifest
        .profile
        .as_ref()
        .is_some_and(|profile| profile.id == "proposal")
    {
        let mut output = Map::new();
        output.insert(
            "normalized_prospect".to_string(),
            Value::Object(prospect.clone()),
        );
        crate::decision_input::from_proposal_output(&output)
    } else {
        crate::decision_input::from_gtm_normalized(prospect)
    };
    let Ok(input) = input else {
        return Vec::new();
    };
    let mut violations = Vec::new();
    let explicit_persona = prospect
        .get("persona")
        .and_then(Value::as_str)
        .and_then(present_str);
    let persona_contract_value = explicit_persona
        .and_then(|persona| {
            resolve_pack_persona_label(manifest, persona, "normalized_prospect.persona")
                .map(|resolution| resolution.persona)
        })
        .or_else(|| {
            prospect
                .get("title")
                .and_then(Value::as_str)
                .and_then(present_str)
                .and_then(|title| {
                    resolve_pack_persona_label(manifest, title, "normalized_prospect.title")
                        .map(|resolution| resolution.persona)
                })
        });

    if let Some(persona) = explicit_persona {
        if resolve_pack_persona_label(manifest, persona, "normalized_prospect.persona").is_none() {
            violations.push(ContractViolation {
                code: "value_contract_persona_unrecognized",
                scope: "prospect",
                field: "persona".to_string(),
                path: format!("{path}/persona"),
                reason: format!(
                    "normalized_prospect.persona must match a pack-owned persona or persona_mappings alias; received {persona}; allowed personas: {}",
                    allowed_personas(manifest)
                ),
            });
        }
    }

    violations.extend(
        crate::value_contracts::decision_input_value_contract_violations(
            &crate::decision_input::requirements_view(&manifest.lead_input_requirements),
            &input,
            "prospect",
            path,
            true,
        ),
    );
    // Preserve the normalized-prospect path for persona diagnostics, including
    // the legacy title-to-persona compatibility projection.
    if let Some(contract) = manifest
        .lead_input_requirements
        .value_contracts
        .get("persona")
    {
        if let Some(persona) = persona_contract_value.as_deref() {
            validate_value(
                "persona",
                &Value::String(persona.to_string()),
                contract,
                &format!("{path}/persona"),
                "prospect",
                &mut violations,
            );
        } else if contract.required {
            violations.push(required_violation(
                "prospect",
                "persona",
                &format!("{path}/persona"),
            ));
        }
    }

    violations
}

fn join_path(prefix: &str, field: &str) -> String {
    if prefix.is_empty() {
        field.to_string()
    } else {
        format!("{prefix}/{field}")
    }
}

fn collect_attribute_contract_violations(
    definitions: &BTreeMap<String, ValueContract>,
    allow_undeclared: bool,
    attributes: &[(&String, &Value)],
    path: &str,
    violations: &mut Vec<ContractViolation>,
) {
    for (name, contract) in definitions {
        if let Some((_, value)) = attributes
            .iter()
            .find(|(key, _)| key.as_str() == name.as_str())
            .filter(|(_, value)| meaningful_json_value(value))
        {
            validate_value(
                name,
                value,
                contract,
                &format!("{path}/{name}"),
                "attribute",
                violations,
            );
        } else if contract.required {
            violations.push(required_violation(
                "attribute",
                name,
                &format!("{path}/{name}"),
            ));
        }
    }

    if !allow_undeclared {
        for (key, _) in attributes {
            if !definitions.contains_key(key.as_str()) {
                violations.push(ContractViolation {
                    code: "value_contract_attribute_undeclared",
                    scope: "attribute",
                    field: key.to_string(),
                    path: format!("{path}/{key}"),
                    reason: format!(
                        "attribute {key} is not declared in manifest lead_input_requirements.attribute_definitions"
                    ),
                });
            }
        }
    }
}

fn validate_value(
    field: &str,
    value: &Value,
    contract: &ValueContract,
    path: &str,
    scope: &'static str,
    violations: &mut Vec<ContractViolation>,
) {
    if let Some(expected_type) = contract.value_type.as_deref() {
        if !value_matches_type(value, expected_type) {
            violations.push(ContractViolation {
                code: "value_contract_type_mismatch",
                scope,
                field: field.to_string(),
                path: path.to_string(),
                reason: format!("expected {expected_type} value for {field}"),
            });
            return;
        }
    }

    if !contract.enum_values.is_empty() {
        let Some(value) = value.as_str() else {
            violations.push(ContractViolation {
                code: "value_contract_enum_type_mismatch",
                scope,
                field: field.to_string(),
                path: path.to_string(),
                reason: format!("expected string enum value for {field}"),
            });
            return;
        };
        if !contract.enum_values.iter().any(|allowed| allowed == value) {
            violations.push(ContractViolation {
                code: "value_contract_enum_mismatch",
                scope,
                field: field.to_string(),
                path: path.to_string(),
                reason: format!(
                    "{field} must be one of {}; received {value}",
                    contract.enum_values.join(", ")
                ),
            });
            return;
        }
    }

    if let Some(format) = contract.format.as_deref() {
        let Some(value) = value.as_str() else {
            violations.push(ContractViolation {
                code: "value_contract_format_type_mismatch",
                scope,
                field: field.to_string(),
                path: path.to_string(),
                reason: format!("expected string value for {field} format {format}"),
            });
            return;
        };
        let valid = match format {
            "date" => valid_date(value),
            "date-time" => valid_date_time(value),
            _ => true,
        };
        if !valid {
            violations.push(ContractViolation {
                code: "value_contract_format_mismatch",
                scope,
                field: field.to_string(),
                path: path.to_string(),
                reason: format!("{field} must use {format} format; received {value}"),
            });
        }
    }
}

fn value_matches_type(value: &Value, expected_type: &str) -> bool {
    match expected_type {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "boolean" => value.is_boolean(),
        _ => true,
    }
}

pub(crate) fn canonical_values_equal(
    contract: &ValueContract,
    left: &Value,
    right: &Value,
) -> bool {
    match contract.value_type.as_deref() {
        Some("string") => left
            .as_str()
            .zip(right.as_str())
            .is_some_and(|(a, b)| a == b),
        Some("boolean") => left
            .as_bool()
            .zip(right.as_bool())
            .is_some_and(|(a, b)| a == b),
        Some("integer") => canonical_integer(left)
            .zip(canonical_integer(right))
            .is_some_and(|(a, b)| a == b),
        Some("number") => canonical_number(left)
            .zip(canonical_number(right))
            .is_some_and(|(a, b)| canonical_numbers_equal(a, b)),
        Some(_) => false,
        None => left == right,
    }
}

fn canonical_integer(value: &Value) -> Option<i128> {
    value
        .as_i64()
        .map(i128::from)
        .or_else(|| value.as_u64().map(i128::from))
}

#[derive(Clone, Copy)]
enum CanonicalNumber {
    Integer(i128),
    Float(f64),
}

fn canonical_number(value: &Value) -> Option<CanonicalNumber> {
    canonical_integer(value)
        .map(CanonicalNumber::Integer)
        .or_else(|| value.as_f64().map(CanonicalNumber::Float))
}

fn canonical_numbers_equal(left: CanonicalNumber, right: CanonicalNumber) -> bool {
    match (left, right) {
        (CanonicalNumber::Integer(left), CanonicalNumber::Integer(right)) => left == right,
        (CanonicalNumber::Float(left), CanonicalNumber::Float(right)) => left == right,
        (CanonicalNumber::Integer(integer), CanonicalNumber::Float(float))
        | (CanonicalNumber::Float(float), CanonicalNumber::Integer(integer)) => {
            float.is_finite()
                && float.fract() == 0.0
                && (float as i128) == integer
                && (integer as f64) == float
        }
    }
}

fn required_violation(scope: &'static str, field: &str, path: &str) -> ContractViolation {
    ContractViolation {
        code: "value_contract_required_missing",
        scope,
        field: field.to_string(),
        path: path.to_string(),
        reason: format!("{field} is required by manifest lead_input_requirements contract"),
    }
}

fn meaningful_json_value(value: &Value) -> bool {
    match value {
        Value::String(value) => present_str(value).is_some(),
        Value::Number(_) | Value::Bool(_) => true,
        _ => false,
    }
}

fn present_str(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty() && !value.eq_ignore_ascii_case("n/a")).then_some(value)
}

fn allowed_personas(manifest: &Manifest) -> String {
    manifest
        .personas
        .iter()
        .chain(manifest.target_personas.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn valid_date(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return false;
    }
    let Ok(year) = parts[0].parse::<u32>() else {
        return false;
    };
    let Ok(month) = parts[1].parse::<u32>() else {
        return false;
    };
    let Ok(day) = parts[2].parse::<u32>() else {
        return false;
    };
    if year == 0 || !(1..=12).contains(&month) {
        return false;
    }
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year(year) => 29,
        2 => 28,
        _ => return false,
    };
    (1..=max_day).contains(&day)
}

pub(crate) fn valid_date_time(value: &str) -> bool {
    let Some((date, rest)) = value.split_once('T') else {
        return false;
    };
    if !valid_date(date) {
        return false;
    }
    let time = rest.strip_suffix('Z').unwrap_or(rest);
    let time = time
        .rsplit_once('+')
        .map(|(time, zone)| valid_offset(zone).then_some(time))
        .or_else(|| {
            time.rsplit_once('-')
                .map(|(time, zone)| valid_offset(zone).then_some(time))
        })
        .flatten()
        .unwrap_or(time);
    valid_time(time)
}

fn valid_time(value: &str) -> bool {
    let parts = value.split(':').collect::<Vec<_>>();
    if !(parts.len() == 2 || parts.len() == 3) {
        return false;
    }
    let Ok(hour) = parts[0].parse::<u32>() else {
        return false;
    };
    let Ok(minute) = parts[1].parse::<u32>() else {
        return false;
    };
    let second = if parts.len() == 3 {
        let seconds = parts[2].split('.').next().unwrap_or_default();
        let Ok(second) = seconds.parse::<u32>() else {
            return false;
        };
        second
    } else {
        0
    };
    hour <= 23 && minute <= 59 && second <= 59
}

fn valid_offset(value: &str) -> bool {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 2 {
        return false;
    }
    let Ok(hour) = parts[0].parse::<u32>() else {
        return false;
    };
    let Ok(minute) = parts[1].parse::<u32>() else {
        return false;
    };
    hour <= 23 && minute <= 59
}

fn leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::LeadInputRequirements;
    use serde_json::json;

    #[test]
    fn date_validation_rejects_invalid_calendar_dates() {
        assert!(valid_date("2026-07-02"));
        assert!(valid_date("2024-02-29"));
        assert!(!valid_date("2025-02-29"));
        assert!(!valid_date("2026-13-02"));
    }

    #[test]
    fn date_time_validation_accepts_basic_rfc3339_shapes() {
        assert!(valid_date_time("2026-07-02T03:45:00Z"));
        assert!(valid_date_time("2026-07-02T03:45:00-04:00"));
        assert!(!valid_date_time("2026-07-02 03:45:00"));
        assert!(!valid_date_time("2026-07-02T25:45:00Z"));
    }

    #[test]
    fn canonical_equality_is_typed_and_does_not_coerce_values() {
        let string_contract = ValueContract {
            value_type: Some("string".to_string()),
            ..ValueContract::default()
        };
        let number_contract = ValueContract {
            value_type: Some("number".to_string()),
            ..ValueContract::default()
        };

        assert!(canonical_values_equal(
            &string_contract,
            &Value::String("7".to_string()),
            &Value::String("7".to_string())
        ));
        assert!(!canonical_values_equal(
            &string_contract,
            &Value::String("7".to_string()),
            &json!(7)
        ));
        assert!(canonical_values_equal(
            &number_contract,
            &json!(7),
            &json!(7.0)
        ));
        assert!(!canonical_values_equal(
            &number_contract,
            &json!(7),
            &Value::String("7".to_string())
        ));
        assert!(!canonical_values_equal(
            &number_contract,
            &json!(9_007_199_254_740_993_u64),
            &json!(9_007_199_254_740_992_f64)
        ));
    }

    #[test]
    fn required_signal_fields_require_every_signal_and_nonempty_signals() {
        let requirements = LeadInputRequirements {
            required_signal_fields: vec!["source".into(), "confidence".into()],
            ..Default::default()
        };
        let view = crate::decision_input::requirements_view(&requirements);
        let partial = crate::decision_input::DecisionInput::new(
            BTreeMap::new(),
            vec![
                json!({"source": "row"}),
                json!({"source": "row", "confidence": "high"}),
            ],
            BTreeMap::new(),
        )
        .unwrap();
        let violations = decision_input_contract_violations(&view, &partial);
        assert_eq!(
            violations
                .iter()
                .filter(|violation| violation.scope == "signal")
                .map(|violation| violation.field.as_str())
                .collect::<Vec<_>>(),
            vec!["confidence"]
        );

        let empty =
            crate::decision_input::DecisionInput::new(BTreeMap::new(), Vec::new(), BTreeMap::new())
                .unwrap();
        assert_eq!(
            decision_input_contract_violations(&view, &empty)
                .iter()
                .filter(|violation| violation.scope == "signal")
                .count(),
            2
        );
    }
}
