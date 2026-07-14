use crate::models::{Manifest, Prospect};
use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) type ContextScope = BTreeMap<String, Vec<String>>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct ScopeResolution {
    pub(crate) requested: ContextScope,
    pub(crate) selected: ContextScope,
    pub(crate) issues: Vec<ScopeIssue>,
}

impl ScopeResolution {
    pub(crate) fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ScopeIssue {
    pub(crate) code: &'static str,
    pub(crate) dimension: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) value: Option<String>,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ScopeMatch {
    pub(crate) compatible: bool,
    pub(crate) issues: Vec<ScopeIssue>,
}

pub(crate) fn parse_scope_selectors(selectors: &[String]) -> Result<ContextScope> {
    let mut parsed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for selector in selectors {
        if selector.matches('=').count() != 1 {
            return Err(anyhow!(
                "invalid --scope {selector:?}; expected exactly one dimension=value pair"
            ));
        }
        let (dimension, value) = selector
            .split_once('=')
            .ok_or_else(|| anyhow!("invalid --scope {selector:?}; expected dimension=value"))?;
        let dimension = normalize_runtime_identifier(dimension);
        let value = normalize_runtime_identifier(value);
        if dimension.is_empty() || value.is_empty() {
            return Err(anyhow!(
                "invalid --scope {selector:?}; dimension and value must both be non-empty"
            ));
        }
        let values = parsed.entry(dimension.clone()).or_default();
        if !values.is_empty() && !values.contains(&value) {
            return Err(anyhow!(
                "invalid --scope: dimension {dimension} has multiple selected values; V1 accepts one value per dimension"
            ));
        }
        values.insert(value);
    }
    Ok(parsed
        .into_iter()
        .map(|(dimension, values)| (dimension, values.into_iter().collect()))
        .collect())
}

pub(crate) fn resolve_runtime_scope(
    manifest: &Manifest,
    requested: ContextScope,
) -> ScopeResolution {
    let Some(profile) = manifest.profile.as_ref() else {
        return resolve_against_dimensions(None, requested);
    };
    let mut resolution = resolve_against_dimensions(Some(&profile.context_dimensions), requested);
    apply_dependencies(&mut resolution, &profile.context_dimension_dependencies);
    resolution
}

pub(crate) fn scope_from_prospect(manifest: &Manifest, prospect: &Prospect) -> ScopeResolution {
    let Some(profile) = manifest.profile.as_ref() else {
        return ScopeResolution::default();
    };
    let mut requested = ContextScope::new();
    let mut issues = Vec::new();

    for dimension in profile.context_dimensions.keys() {
        if dimension == "segment" {
            let attribute_segment = prospect.attributes.get("segment");
            if let (Some(segment), Some(attribute)) = (&prospect.segment, attribute_segment) {
                if attribute.as_str().is_none_or(|value| {
                    normalize_runtime_identifier(value) != normalize_runtime_identifier(segment)
                }) {
                    issues.push(ScopeIssue {
                        code: "scope_segment_conflict",
                        dimension: dimension.clone(),
                        value: attribute.as_str().map(str::to_string),
                        reason: "prospect.segment is authoritative and conflicts with attributes.segment"
                            .to_string(),
                    });
                }
            }
            if let Some(segment) = prospect.segment.as_deref() {
                requested.insert(
                    dimension.clone(),
                    vec![normalize_runtime_identifier(segment)],
                );
                continue;
            }
        }

        let Some(value) = prospect.attributes.get(dimension) else {
            continue;
        };
        match scalar_scope_value(value) {
            Some(value) if !value.is_empty() => {
                requested.insert(dimension.clone(), vec![value]);
            }
            Some(_) => issues.push(ScopeIssue {
                code: "scope_attribute_empty",
                dimension: dimension.clone(),
                value: None,
                reason: format!("prospect attribute {dimension} must be a non-empty string"),
            }),
            None => issues.push(ScopeIssue {
                code: "scope_attribute_type_invalid",
                dimension: dimension.clone(),
                value: None,
                reason: format!("prospect attribute {dimension} must be a scalar string"),
            }),
        }
    }

    let mut resolution = resolve_against_dimensions(Some(&profile.context_dimensions), requested);
    resolution.issues.splice(0..0, issues);
    apply_dependencies(&mut resolution, &profile.context_dimension_dependencies);
    resolution
}

fn apply_dependencies(
    resolution: &mut ScopeResolution,
    dependencies: &BTreeMap<String, Vec<String>>,
) {
    for (dimension, required_dimensions) in dependencies {
        if !resolution.selected.contains_key(dimension) {
            continue;
        }
        for required_dimension in required_dimensions {
            if !resolution.selected.contains_key(required_dimension) {
                resolution.issues.push(ScopeIssue {
                    code: "scope_dependency_missing",
                    dimension: dimension.clone(),
                    value: Some(required_dimension.clone()),
                    reason: format!(
                        "selected dimension {dimension} requires selected dimension {required_dimension}"
                    ),
                });
            }
        }
    }
}

pub(crate) fn match_entry_scope(
    resolution: &ScopeResolution,
    entry_scope: &ContextScope,
) -> ScopeMatch {
    if entry_scope.is_empty() {
        return ScopeMatch {
            compatible: true,
            issues: Vec::new(),
        };
    }
    if !resolution.is_valid() {
        return ScopeMatch {
            compatible: false,
            issues: resolution.issues.clone(),
        };
    }

    let mut issues = Vec::new();
    for (dimension, allowed_values) in entry_scope {
        let Some(selected_values) = resolution.selected.get(dimension) else {
            issues.push(ScopeIssue {
                code: "scope_dimension_missing",
                dimension: dimension.clone(),
                value: None,
                reason: format!("entry requires a selected {dimension} value"),
            });
            continue;
        };
        if !allowed_values.iter().any(|allowed| {
            selected_values
                .iter()
                .any(|selected| selected.eq_ignore_ascii_case(allowed))
        }) {
            issues.push(ScopeIssue {
                code: "scope_value_mismatch",
                dimension: dimension.clone(),
                value: Some(selected_values.join(",")),
                reason: format!(
                    "selected {dimension} values [{}] do not match entry values [{}]",
                    selected_values.join(", "),
                    allowed_values.join(", ")
                ),
            });
        }
    }

    ScopeMatch {
        compatible: issues.is_empty(),
        issues,
    }
}

fn resolve_against_dimensions(
    dimensions: Option<&ContextScope>,
    requested: ContextScope,
) -> ScopeResolution {
    let mut selected: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut issues = Vec::new();
    let empty = ContextScope::new();
    let dimensions = dimensions.unwrap_or(&empty);

    for (requested_dimension, requested_values) in &requested {
        let Some((dimension, allowed_values)) = dimensions
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(requested_dimension))
        else {
            issues.push(ScopeIssue {
                code: "scope_dimension_unknown",
                dimension: requested_dimension.clone(),
                value: None,
                reason: format!(
                    "scope dimension {requested_dimension} is not declared by profile.context_dimensions"
                ),
            });
            continue;
        };

        for requested_value in requested_values {
            if let Some(value) = allowed_values
                .iter()
                .find(|candidate| candidate.eq_ignore_ascii_case(requested_value))
            {
                selected
                    .entry(dimension.clone())
                    .or_default()
                    .insert(value.clone());
            } else {
                issues.push(ScopeIssue {
                    code: "scope_value_unknown",
                    dimension: dimension.clone(),
                    value: Some(requested_value.clone()),
                    reason: format!(
                        "scope value {requested_value} is not declared for dimension {dimension}"
                    ),
                });
            }
        }
    }

    ScopeResolution {
        requested,
        selected: selected
            .into_iter()
            .map(|(dimension, values)| (dimension, values.into_iter().collect()))
            .collect(),
        issues,
    }
}

fn scalar_scope_value(value: &Value) -> Option<String> {
    value.as_str().map(normalize_runtime_identifier)
}

pub(crate) fn normalize_runtime_identifier(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_separator = false;
    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            previous_separator = false;
        } else if !previous_separator && !normalized.is_empty() {
            normalized.push('-');
            previous_separator = true;
        }
    }
    while normalized.ends_with('-') {
        normalized.pop();
    }
    normalized
}

pub(crate) fn valid_declared_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !value.contains("--")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{LeadInputRequirements, Policy, Profile, ProfileEval, Provenance};

    fn manifest() -> Manifest {
        Manifest {
            format: "mdp.v0".to_string(),
            id: "test".to_string(),
            name: "Test".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            profile: Some(Profile {
                id: "gtm".to_string(),
                label: None,
                version: None,
                context_dimensions: BTreeMap::from([
                    (
                        "product".to_string(),
                        vec!["platform-a".to_string(), "platform-b".to_string()],
                    ),
                    (
                        "capability".to_string(),
                        vec!["developer-surface".to_string()],
                    ),
                    (
                        "segment".to_string(),
                        vec!["enterprise".to_string(), "mid-market".to_string()],
                    ),
                ]),
                context_dimension_dependencies: BTreeMap::from([(
                    "capability".to_string(),
                    vec!["product".to_string()],
                )]),
            }),
            personas: vec![],
            target_personas: vec![],
            operator_roles: vec![],
            supported_channels: vec![],
            persona_mappings: vec![],
            lead_input_requirements: LeadInputRequirements::default(),
            qualification_gates: None,
            required_primitives: vec![],
            primitive_map: BTreeMap::new(),
            input_contracts: vec![],
            jobs: vec![],
            profile_eval: ProfileEval::default(),
            cards: vec![],
            policy: Policy {
                progressive_disclosure: true,
                load_manifest_first: true,
                max_cards_per_route: 10,
                json_contract: "mdp.cli.v0".to_string(),
                no_auth_required: true,
            },
            provenance: Provenance {
                owner: "test".to_string(),
                created_by: "test".to_string(),
                notes: vec![],
            },
        }
    }

    fn prospect() -> Prospect {
        Prospect {
            name: "Taylor".to_string(),
            title: "VP Sales".to_string(),
            company: "Example".to_string(),
            company_domain: None,
            source_kind: None,
            synthetic: true,
            linkedin_url: None,
            company_url: None,
            background: None,
            trigger: None,
            persona: None,
            segment: Some("enterprise".to_string()),
            signals: vec![],
            attributes: BTreeMap::from([(
                "product".to_string(),
                Value::String("platform-a".to_string()),
            )]),
        }
    }

    #[test]
    fn selectors_normalize_and_deduplicate() {
        let scope = parse_scope_selectors(&[
            "Product=Platform-A".to_string(),
            "product=platform-a".to_string(),
        ])
        .unwrap();
        assert_eq!(scope["product"], vec!["platform-a"]);
        let err = parse_scope_selectors(&[
            "product=platform-a".to_string(),
            "product=platform-b".to_string(),
        ])
        .expect_err("multiple runtime values for one dimension should be rejected");
        assert!(err.to_string().contains("multiple selected values"));
        assert!(
            parse_scope_selectors(&["product=platform=a".to_string()])
                .expect_err("multiple separators should be rejected")
                .to_string()
                .contains("exactly one")
        );
        assert!(
            parse_scope_selectors(&["product".to_string()])
                .expect_err("missing separator should be rejected")
                .to_string()
                .contains("exactly one")
        );
    }

    #[test]
    fn matching_is_or_within_and_and_across_dimensions() {
        let resolution = resolve_runtime_scope(
            &manifest(),
            BTreeMap::from([
                ("product".to_string(), vec!["platform-b".to_string()]),
                (
                    "capability".to_string(),
                    vec!["developer-surface".to_string()],
                ),
            ]),
        );
        let entry_scope = BTreeMap::from([
            (
                "product".to_string(),
                vec!["platform-a".to_string(), "platform-b".to_string()],
            ),
            (
                "capability".to_string(),
                vec!["developer-surface".to_string()],
            ),
        ]);
        assert!(match_entry_scope(&resolution, &entry_scope).compatible);
    }

    #[test]
    fn broader_product_entry_matches_narrower_runtime_scope() {
        let resolution = resolve_runtime_scope(
            &manifest(),
            BTreeMap::from([
                ("product".to_string(), vec!["platform-a".to_string()]),
                (
                    "capability".to_string(),
                    vec!["developer-surface".to_string()],
                ),
            ]),
        );
        let entry_scope = BTreeMap::from([("product".to_string(), vec!["platform-a".to_string()])]);
        assert!(match_entry_scope(&resolution, &entry_scope).compatible);
    }

    #[test]
    fn prospect_uses_scalar_attribute_and_top_level_segment() {
        let resolution = scope_from_prospect(&manifest(), &prospect());
        assert!(resolution.is_valid());
        assert_eq!(resolution.selected["product"], vec!["platform-a"]);
        assert_eq!(resolution.selected["segment"], vec!["enterprise"]);
    }

    #[test]
    fn dependent_dimension_requires_its_companion_dimension() {
        let resolution = resolve_runtime_scope(
            &manifest(),
            BTreeMap::from([(
                "capability".to_string(),
                vec!["developer-surface".to_string()],
            )]),
        );
        assert!(!resolution.is_valid());
        assert_eq!(resolution.issues[0].code, "scope_dependency_missing");
    }

    #[test]
    fn top_level_segment_conflict_is_invalid() {
        let mut prospect = prospect();
        prospect.attributes.insert(
            "segment".to_string(),
            Value::String("mid-market".to_string()),
        );
        let resolution = scope_from_prospect(&manifest(), &prospect);
        assert!(!resolution.is_valid());
        assert!(
            resolution
                .issues
                .iter()
                .any(|issue| issue.code == "scope_segment_conflict")
        );
    }

    #[test]
    fn missing_dimension_and_unknown_value_fail_closed() {
        let missing = resolve_runtime_scope(
            &manifest(),
            BTreeMap::from([("product".to_string(), vec!["platform-a".to_string()])]),
        );
        let entry_scope = BTreeMap::from([(
            "capability".to_string(),
            vec!["developer-surface".to_string()],
        )]);
        let result = match_entry_scope(&missing, &entry_scope);
        assert!(!result.compatible);
        assert_eq!(result.issues[0].code, "scope_dimension_missing");

        let unknown = resolve_runtime_scope(
            &manifest(),
            BTreeMap::from([("product".to_string(), vec!["platform-c".to_string()])]),
        );
        assert!(!match_entry_scope(&unknown, &entry_scope).compatible);
        assert_eq!(unknown.issues[0].code, "scope_value_unknown");
    }

    #[test]
    fn declared_identifiers_are_lowercase_kebab_case() {
        assert!(valid_declared_identifier("developer-surface"));
        assert!(!valid_declared_identifier("Developer-Surface"));
        assert!(!valid_declared_identifier("developer_surface"));
        assert!(!valid_declared_identifier("developer--surface"));
    }
}
