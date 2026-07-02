use crate::models::{Manifest, Prospect, ValueContract};
use crate::utils::resolve_pack_persona_label;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

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

    for (field, contract) in &manifest.lead_input_requirements.value_contracts {
        if field == "persona" {
            if let Some(persona) = persona_contract_value.as_deref() {
                validate_value(
                    field,
                    &Value::String(persona.to_string()),
                    contract,
                    field,
                    "prospect",
                    &mut violations,
                );
            } else if contract.required {
                violations.push(required_violation("prospect", field, field));
            }
        } else if let Some(value) = prospect_field_value(prospect, field) {
            validate_value(field, &value, contract, field, "prospect", &mut violations);
        } else if contract.required {
            violations.push(required_violation("prospect", field, field));
        }
    }

    collect_attribute_contract_violations(
        &manifest.lead_input_requirements.attribute_definitions,
        manifest.lead_input_requirements.allow_undeclared_attributes,
        &prospect.attributes.iter().collect::<Vec<_>>(),
        "attributes",
        &mut violations,
    );

    violations
}

pub(crate) fn normalized_prospect_contract_violations(
    manifest: &Manifest,
    prospect: &Map<String, Value>,
    path: &str,
) -> Vec<ContractViolation> {
    let mut violations = Vec::new();
    let explicit_persona = prospect
        .get("persona")
        .and_then(Value::as_str)
        .and_then(present_str);
    let persona_contract_value = explicit_persona.and_then(|persona| {
        resolve_pack_persona_label(manifest, persona, "normalized_prospect.persona")
            .map(|resolution| resolution.persona)
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

    for (field, contract) in &manifest.lead_input_requirements.value_contracts {
        if field == "persona" {
            if let Some(persona) = persona_contract_value.as_deref() {
                validate_value(
                    field,
                    &Value::String(persona.to_string()),
                    contract,
                    &format!("{path}/{field}"),
                    "prospect",
                    &mut violations,
                );
            } else if let Some(value) = prospect
                .get(field)
                .filter(|value| meaningful_json_value(value))
            {
                validate_value(
                    field,
                    value,
                    contract,
                    &format!("{path}/{field}"),
                    "prospect",
                    &mut violations,
                );
            } else if contract.required {
                violations.push(required_violation(
                    "prospect",
                    field,
                    &format!("{path}/{field}"),
                ));
            }
        } else if let Some(value) = prospect
            .get(field)
            .filter(|value| meaningful_json_value(value))
        {
            validate_value(
                field,
                value,
                contract,
                &format!("{path}/{field}"),
                "prospect",
                &mut violations,
            );
        } else if contract.required {
            violations.push(required_violation(
                "prospect",
                field,
                &format!("{path}/{field}"),
            ));
        }
    }

    let attributes = prospect
        .get("attributes")
        .and_then(Value::as_object)
        .map(|attributes| attributes.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    collect_attribute_contract_violations(
        &manifest.lead_input_requirements.attribute_definitions,
        manifest.lead_input_requirements.allow_undeclared_attributes,
        &attributes,
        &format!("{path}/attributes"),
        &mut violations,
    );

    violations
}

fn prospect_field_value(prospect: &Prospect, field: &str) -> Option<Value> {
    match field {
        "name" => present_str(&prospect.name).map(|value| Value::String(value.to_string())),
        "title" => present_str(&prospect.title).map(|value| Value::String(value.to_string())),
        "company" => present_str(&prospect.company).map(|value| Value::String(value.to_string())),
        "company_domain" => prospect
            .company_domain
            .as_deref()
            .and_then(present_str)
            .map(|value| Value::String(value.to_string())),
        "source_kind" => prospect
            .source_kind
            .as_deref()
            .and_then(present_str)
            .map(|value| Value::String(value.to_string())),
        "synthetic" => Some(Value::Bool(prospect.synthetic)),
        "linkedin_url" => prospect
            .linkedin_url
            .as_deref()
            .and_then(present_str)
            .map(|value| Value::String(value.to_string())),
        "company_url" => prospect
            .company_url
            .as_deref()
            .and_then(present_str)
            .map(|value| Value::String(value.to_string())),
        "background" => prospect
            .background
            .as_deref()
            .and_then(present_str)
            .map(|value| Value::String(value.to_string())),
        "trigger" => prospect
            .trigger
            .as_deref()
            .and_then(present_str)
            .map(|value| Value::String(value.to_string())),
        "persona" => prospect
            .persona
            .as_deref()
            .and_then(present_str)
            .map(|value| Value::String(value.to_string())),
        "segment" => prospect
            .segment
            .as_deref()
            .and_then(present_str)
            .map(|value| Value::String(value.to_string())),
        _ => None,
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

fn valid_date(value: &str) -> bool {
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

fn valid_date_time(value: &str) -> bool {
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
}
