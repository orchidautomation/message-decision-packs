use crate::models::{JobSelectorContract, Manifest, Prospect};
use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) type ContextScope = BTreeMap<String, Vec<String>>;

/// Selector declarations and runtime requests are intentionally bounded.  The
/// same limits are reflected in the exported JSON Schema so a provider cannot
/// smuggle an unbounded selector surface past the local resolver.
pub(crate) const MAX_SELECTOR_DIMENSIONS: usize = 32;
pub(crate) const MAX_SELECTOR_VALUES_PER_DIMENSION: usize = 16;
pub(crate) const MAX_REQUIRED_SELECTOR_DIMENSIONS: usize = 16;

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
    pub(crate) predicates: Vec<ScopePredicateResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ScopePredicateResult {
    pub(crate) dimension: String,
    pub(crate) status: &'static str,
    pub(crate) allowed: Vec<String>,
    pub(crate) selected: Vec<String>,
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
        if !parsed.contains_key(&dimension) && parsed.len() >= MAX_SELECTOR_DIMENSIONS {
            return Err(anyhow!(
                "invalid --scope: selector input exceeds the bounded limit of {MAX_SELECTOR_DIMENSIONS} dimensions"
            ));
        }
        let values = parsed.entry(dimension.clone()).or_default();
        if values.len() >= MAX_SELECTOR_VALUES_PER_DIMENSION && !values.contains(&value) {
            return Err(anyhow!(
                "invalid --scope: dimension {dimension} exceeds the bounded limit of {MAX_SELECTOR_VALUES_PER_DIMENSION} selected values"
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

/// Resolve runtime selectors and apply the optional selector contract declared
/// by a canonical job.  Legacy jobs (and packs without a selector contract)
/// retain the existing profile-only behavior.  A declared contract is
/// fail-closed: malformed declarations, unknown dimensions/values, and
/// missing required dimensions become explicit scope issues rather than being
/// guessed from job prose.
pub(crate) fn resolve_runtime_scope_for_job(
    manifest: &Manifest,
    job_id: Option<&str>,
    requested: ContextScope,
) -> ScopeResolution {
    let mut resolution = resolve_runtime_scope(manifest, requested);
    let Some(contract) = job_id
        .and_then(|job_id| manifest.jobs.iter().find(|job| job.id == job_id))
        .and_then(|job| job.selector_contract.as_ref())
    else {
        return resolution;
    };
    apply_selector_contract(manifest, contract, &mut resolution);
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

pub(crate) fn scope_from_prospect_for_job(
    manifest: &Manifest,
    prospect: &Prospect,
    job_id: Option<&str>,
) -> ScopeResolution {
    let mut resolution = scope_from_prospect(manifest, prospect);
    let Some(contract) = job_id
        .and_then(|job_id| manifest.jobs.iter().find(|job| job.id == job_id))
        .and_then(|job| job.selector_contract.as_ref())
    else {
        return resolution;
    };
    apply_selector_contract(manifest, contract, &mut resolution);
    resolution
}

/// Stable machine-readable selector declaration attached to route receipts.
/// The legacy shape is explicit so consumers can distinguish a certified
/// structured selector contract from compatibility-only token routing.
pub(crate) fn selector_contract_for_job(manifest: &Manifest, job_id: Option<&str>) -> Value {
    let Some(contract) = job_id
        .and_then(|job_id| manifest.jobs.iter().find(|job| job.id == job_id))
        .and_then(|job| job.selector_contract.as_ref())
    else {
        return json!({
            "status": "legacy-compatible",
            "contract": Value::Null,
            "required": [],
            "dimensions": {}
        });
    };
    json!({
        "status": "declared",
        "contract": contract.contract,
        "required": contract.required,
        "dimensions": contract.dimensions
    })
}

fn apply_selector_contract(
    manifest: &Manifest,
    contract: &JobSelectorContract,
    resolution: &mut ScopeResolution,
) {
    const CONTRACT: &str = "mdp.job-selectors.v1";
    if contract.contract != CONTRACT {
        resolution.issues.push(ScopeIssue {
            code: "scope_job_selector_contract_invalid",
            dimension: "job".to_string(),
            value: Some(contract.contract.clone()),
            reason: format!("job selector contract must declare contract {CONTRACT}"),
        });
    }
    if contract.dimensions.len() > MAX_SELECTOR_DIMENSIONS {
        resolution.issues.push(ScopeIssue {
            code: "scope_job_selector_contract_invalid",
            dimension: "job".to_string(),
            value: None,
            reason: format!(
                "job selector contracts may declare at most {MAX_SELECTOR_DIMENSIONS} dimensions"
            ),
        });
    }
    if contract.required.len() > MAX_REQUIRED_SELECTOR_DIMENSIONS {
        resolution.issues.push(ScopeIssue {
            code: "scope_job_selector_contract_invalid",
            dimension: "job".to_string(),
            value: None,
            reason: format!(
                "job selector contracts may require at most {MAX_REQUIRED_SELECTOR_DIMENSIONS} dimensions"
            ),
        });
    }

    let profile_dimensions = manifest
        .profile
        .as_ref()
        .map(|profile| &profile.context_dimensions);
    let empty = ContextScope::new();
    let profile_dimensions = profile_dimensions.unwrap_or(&empty);

    let mut seen_dimensions = BTreeSet::new();
    for (dimension, allowed_values) in &contract.dimensions {
        let normalized_dimension = normalize_runtime_identifier(dimension);
        if dimension != &normalized_dimension
            || !valid_declared_identifier(&normalized_dimension)
            || !seen_dimensions.insert(normalized_dimension)
        {
            resolution.issues.push(ScopeIssue {
                code: "scope_job_selector_contract_invalid",
                dimension: dimension.clone(),
                value: None,
                reason: "job selector dimensions must be unique normalized identifiers".to_string(),
            });
            continue;
        }
        let Some((declared_dimension, profile_values)) = profile_dimensions
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(dimension))
        else {
            resolution.issues.push(ScopeIssue {
                code: "scope_job_selector_contract_invalid",
                dimension: dimension.clone(),
                value: None,
                reason: format!(
                    "job selector dimension {dimension} is not declared by profile.context_dimensions"
                ),
            });
            continue;
        };
        if allowed_values.is_empty() {
            resolution.issues.push(ScopeIssue {
                code: "scope_job_selector_contract_invalid",
                dimension: declared_dimension.clone(),
                value: None,
                reason: "job selector dimensions must declare at least one value".to_string(),
            });
        }
        if allowed_values.len() > MAX_SELECTOR_VALUES_PER_DIMENSION {
            resolution.issues.push(ScopeIssue {
                code: "scope_job_selector_contract_invalid",
                dimension: declared_dimension.clone(),
                value: None,
                reason: format!(
                    "job selector dimensions may declare at most {MAX_SELECTOR_VALUES_PER_DIMENSION} values"
                ),
            });
        }
        let mut seen = BTreeSet::new();
        for value in allowed_values {
            let normalized = normalize_runtime_identifier(value);
            if value != &normalized
                || !valid_declared_identifier(&normalized)
                || !seen.insert(normalized.clone())
            {
                resolution.issues.push(ScopeIssue {
                    code: "scope_job_selector_contract_invalid",
                    dimension: declared_dimension.clone(),
                    value: Some(value.clone()),
                    reason: format!(
                        "job selector value {value} must be a unique normalized identifier"
                    ),
                });
            } else if !profile_values
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(&normalized))
            {
                resolution.issues.push(ScopeIssue {
                    code: "scope_job_selector_contract_invalid",
                    dimension: declared_dimension.clone(),
                    value: Some(value.clone()),
                    reason: format!(
                        "job selector value {value} is not declared for profile dimension {declared_dimension}"
                    ),
                });
            }
        }

        if let Some(selected_values) = resolution.selected.get(declared_dimension) {
            for selected in selected_values {
                if !allowed_values
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(selected))
                {
                    resolution.issues.push(ScopeIssue {
                        code: "scope_job_selector_value_mismatch",
                        dimension: declared_dimension.clone(),
                        value: Some(selected.clone()),
                        reason: format!(
                            "selected {declared_dimension} value {selected} is outside the job selector contract"
                        ),
                    });
                }
            }
        }
    }

    let mut required = BTreeSet::new();
    for dimension in &contract.required {
        let normalized = normalize_runtime_identifier(dimension);
        if dimension != &normalized
            || !valid_declared_identifier(&normalized)
            || !required.insert(normalized.clone())
        {
            resolution.issues.push(ScopeIssue {
                code: "scope_job_selector_contract_invalid",
                dimension: dimension.clone(),
                value: None,
                reason: format!(
                    "required job selector dimensions must be unique normalized identifiers"
                ),
            });
            continue;
        }
        let Some((declared_dimension, _)) = contract
            .dimensions
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(&normalized))
        else {
            resolution.issues.push(ScopeIssue {
                code: "scope_job_selector_contract_invalid",
                dimension: normalized,
                value: None,
                reason: "required selector dimension must be declared in dimensions".to_string(),
            });
            continue;
        };
        if !resolution
            .selected
            .keys()
            .any(|selected| selected.eq_ignore_ascii_case(declared_dimension))
        {
            resolution.issues.push(ScopeIssue {
                code: "scope_dimension_missing",
                dimension: declared_dimension.clone(),
                value: None,
                reason: format!(
                    "job selector contract requires a selected {declared_dimension} value"
                ),
            });
        }
    }
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
    if is_explicit_universal(entry_scope) {
        return ScopeMatch {
            compatible: true,
            issues: Vec::new(),
            predicates: vec![ScopePredicateResult {
                dimension: "universal".to_string(),
                status: "not-applicable",
                allowed: vec!["true".to_string()],
                selected: Vec::new(),
            }],
        };
    }
    if entry_scope.is_empty() {
        return ScopeMatch {
            compatible: true,
            issues: Vec::new(),
            predicates: Vec::new(),
        };
    }
    if !resolution.is_valid() {
        return ScopeMatch {
            compatible: false,
            issues: resolution.issues.clone(),
            predicates: entry_scope
                .iter()
                .map(|(dimension, allowed)| ScopePredicateResult {
                    dimension: dimension.clone(),
                    status: "unknown",
                    allowed: allowed.clone(),
                    selected: resolution
                        .selected
                        .get(dimension)
                        .cloned()
                        .unwrap_or_default(),
                })
                .collect(),
        };
    }

    let mut issues = Vec::new();
    let mut predicates = Vec::new();
    for (dimension, allowed_values) in entry_scope {
        let Some(selected_values) = resolution.selected.get(dimension) else {
            issues.push(ScopeIssue {
                code: "scope_dimension_missing",
                dimension: dimension.clone(),
                value: None,
                reason: format!("entry requires a selected {dimension} value"),
            });
            predicates.push(ScopePredicateResult {
                dimension: dimension.clone(),
                status: "missing",
                allowed: allowed_values.clone(),
                selected: Vec::new(),
            });
            continue;
        };
        let matches = allowed_values.iter().any(|allowed| {
            selected_values
                .iter()
                .any(|selected| selected.eq_ignore_ascii_case(allowed))
        });
        predicates.push(ScopePredicateResult {
            dimension: dimension.clone(),
            status: if matches { "match" } else { "mismatch" },
            allowed: allowed_values.clone(),
            selected: selected_values.clone(),
        });
        if !matches {
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
        predicates,
    }
}

/// Match an entry's structured applicability against the selected job's
/// closed selector declaration.  A profile-valid dimension that the job did
/// not declare is unconstrained (and therefore emits `not-applicable` rather
/// than a false missing-value rejection).  Legacy jobs deliberately use the
/// original profile-only matcher above.
pub(crate) fn match_entry_scope_for_job(
    manifest: &Manifest,
    job_id: Option<&str>,
    resolution: &ScopeResolution,
    entry_scope: &ContextScope,
) -> ScopeMatch {
    let contract = job_id
        .and_then(|job_id| manifest.jobs.iter().find(|job| job.id == job_id))
        .and_then(|job| job.selector_contract.as_ref());
    let Some(contract) = contract else {
        return match_entry_scope(resolution, entry_scope);
    };
    if is_explicit_universal(entry_scope) {
        return ScopeMatch {
            compatible: true,
            issues: Vec::new(),
            predicates: vec![ScopePredicateResult {
                dimension: "universal".to_string(),
                status: "not-applicable",
                allowed: vec!["true".to_string()],
                selected: Vec::new(),
            }],
        };
    }
    if entry_scope.is_empty() {
        return ScopeMatch {
            compatible: true,
            issues: Vec::new(),
            predicates: Vec::new(),
        };
    }

    let profile_dimensions = manifest
        .profile
        .as_ref()
        .map(|profile| &profile.context_dimensions);
    let empty = ContextScope::new();
    let profile_dimensions = profile_dimensions.unwrap_or(&empty);
    let resolution_invalid = !resolution.is_valid();
    let mut issues = if resolution_invalid {
        resolution.issues.clone()
    } else {
        Vec::new()
    };
    let mut predicates = Vec::new();

    for (dimension, entry_values) in entry_scope {
        let profile_dimension = profile_dimensions
            .keys()
            .find(|candidate| candidate.eq_ignore_ascii_case(dimension));
        let Some(profile_dimension) = profile_dimension else {
            issues.push(ScopeIssue {
                code: "scope_dimension_unknown",
                dimension: dimension.clone(),
                value: None,
                reason: format!(
                    "entry scope dimension {dimension} is not declared by profile.context_dimensions"
                ),
            });
            predicates.push(ScopePredicateResult {
                dimension: dimension.clone(),
                status: "unknown",
                allowed: entry_values.clone(),
                selected: selected_scope_values(resolution, dimension),
            });
            continue;
        };

        let Some((contract_dimension, contract_values)) = contract
            .dimensions
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(dimension))
        else {
            // The profile knows this dimension, but the selected job does not
            // declare it.  Approved v1 semantics treat that axis as
            // unconstrained rather than inventing a missing-state rejection.
            predicates.push(ScopePredicateResult {
                dimension: profile_dimension.clone(),
                status: "not-applicable",
                allowed: entry_values.clone(),
                selected: selected_scope_values(resolution, profile_dimension),
            });
            continue;
        };

        let selected = selected_scope_values(resolution, contract_dimension);
        let mut entry_value_mismatch = false;
        for value in entry_values {
            if !contract_values
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(value))
            {
                entry_value_mismatch = true;
                issues.push(ScopeIssue {
                    code: "scope_job_selector_value_mismatch",
                    dimension: contract_dimension.clone(),
                    value: Some(value.clone()),
                    reason: format!(
                        "entry selector value {value} is outside the selected job selector contract"
                    ),
                });
            }
        }

        if resolution_invalid {
            predicates.push(ScopePredicateResult {
                dimension: contract_dimension.clone(),
                status: "unknown",
                allowed: entry_values.clone(),
                selected,
            });
            continue;
        }
        if selected.is_empty() {
            issues.push(ScopeIssue {
                code: "scope_dimension_missing",
                dimension: contract_dimension.clone(),
                value: None,
                reason: format!("entry requires a selected {contract_dimension} value"),
            });
            predicates.push(ScopePredicateResult {
                dimension: contract_dimension.clone(),
                status: if entry_value_mismatch {
                    "mismatch"
                } else {
                    "missing"
                },
                allowed: entry_values.clone(),
                selected,
            });
            continue;
        }

        let matches = entry_values.iter().any(|allowed| {
            selected
                .iter()
                .any(|selected| selected.eq_ignore_ascii_case(allowed))
        });
        predicates.push(ScopePredicateResult {
            dimension: contract_dimension.clone(),
            status: if entry_value_mismatch || !matches {
                "mismatch"
            } else {
                "match"
            },
            allowed: entry_values.clone(),
            selected: selected.clone(),
        });
        if !matches {
            issues.push(ScopeIssue {
                code: "scope_value_mismatch",
                dimension: contract_dimension.clone(),
                value: Some(selected.join(",")),
                reason: format!(
                    "selected {contract_dimension} values [{}] do not match entry values [{}]",
                    selected.join(", "),
                    entry_values.join(", ")
                ),
            });
        }
    }

    ScopeMatch {
        compatible: issues.is_empty(),
        issues,
        predicates,
    }
}

fn selected_scope_values(resolution: &ScopeResolution, dimension: &str) -> Vec<String> {
    resolution
        .selected
        .iter()
        .find(|(selected_dimension, _)| selected_dimension.eq_ignore_ascii_case(dimension))
        .map(|(_, values)| values.clone())
        .unwrap_or_default()
}

/// A universal predicate is explicit rather than inferred from an empty map.
/// It is reserved for the structured selector contract and is intentionally
/// not a profile context dimension.
pub(crate) fn is_explicit_universal(scope: &ContextScope) -> bool {
    scope.len() == 1
        && scope
            .get("universal")
            .is_some_and(|values| values.len() == 1 && values[0] == "true")
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
    use crate::models::{
        JobSelectorContract, LeadInputRequirements, Policy, Profile, ProfileEval, ProfileJob,
        Provenance,
    };

    fn manifest() -> Manifest {
        Manifest {
            format: "mdp.v0".to_string(),
            id: "test".to_string(),
            name: "Test".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            target: None,
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
                product_foundation: None,
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
            decision_input_contracts: vec![],
            classification_taxonomies: vec![],
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
        let multi = parse_scope_selectors(&[
            "product=platform-a".to_string(),
            "product=platform-b".to_string(),
        ])
        .expect("bounded multi-value selectors should be accepted");
        assert_eq!(multi["product"], vec!["platform-a", "platform-b"]);
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
    fn selector_input_is_bounded_by_dimensions_and_values() {
        let dimensions = (0..=MAX_SELECTOR_DIMENSIONS)
            .map(|index| format!("dimension-{index}=value"))
            .collect::<Vec<_>>();
        let error = parse_scope_selectors(&dimensions)
            .expect_err("selector inputs must cap the number of dimensions");
        assert!(error.to_string().contains("dimensions"));

        let values = (0..=MAX_SELECTOR_VALUES_PER_DIMENSION)
            .map(|index| format!("product=value-{index}"))
            .collect::<Vec<_>>();
        let error = parse_scope_selectors(&values)
            .expect_err("selector inputs must cap values per dimension");
        assert!(error.to_string().contains("selected values"));
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
        let matched = match_entry_scope(&resolution, &entry_scope);
        assert!(matched.compatible);
        assert_eq!(matched.predicates.len(), 2);
        assert!(
            matched
                .predicates
                .iter()
                .all(|predicate| predicate.status == "match")
        );
    }

    #[test]
    fn multi_value_runtime_scope_matches_by_intersection() {
        let resolution = resolve_runtime_scope(
            &manifest(),
            BTreeMap::from([(
                "product".to_string(),
                vec!["platform-a".to_string(), "platform-b".to_string()],
            )]),
        );
        let entry_scope = BTreeMap::from([("product".to_string(), vec!["platform-b".to_string()])]);
        let matched = match_entry_scope(&resolution, &entry_scope);
        assert!(matched.compatible);
        assert_eq!(
            matched.predicates[0].selected,
            vec!["platform-a", "platform-b"]
        );
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

    #[test]
    fn job_selector_contract_requires_declared_dimensions_and_values() {
        let mut manifest = manifest();
        manifest.jobs.push(ProfileJob {
            id: "scoped-job".to_string(),
            selector_contract: Some(JobSelectorContract {
                contract: "mdp.job-selectors.v1".to_string(),
                required: vec!["product".to_string()],
                dimensions: BTreeMap::from([(
                    "product".to_string(),
                    vec!["platform-a".to_string()],
                )]),
            }),
            ..ProfileJob::default()
        });

        let missing =
            resolve_runtime_scope_for_job(&manifest, Some("scoped-job"), ContextScope::new());
        assert!(missing.issues.iter().any(|issue| {
            issue.code == "scope_dimension_missing" && issue.dimension == "product"
        }));

        let outside = resolve_runtime_scope_for_job(
            &manifest,
            Some("scoped-job"),
            BTreeMap::from([("product".to_string(), vec!["platform-b".to_string()])]),
        );
        assert!(
            outside
                .issues
                .iter()
                .any(|issue| issue.code == "scope_job_selector_value_mismatch")
        );
    }

    #[test]
    fn undeclared_profile_dimension_is_not_applicable_for_the_selected_job() {
        let mut manifest = manifest();
        manifest.jobs.push(ProfileJob {
            id: "scoped-job".to_string(),
            selector_contract: Some(JobSelectorContract {
                contract: "mdp.job-selectors.v1".to_string(),
                required: vec!["product".to_string()],
                dimensions: BTreeMap::from([(
                    "product".to_string(),
                    vec!["platform-a".to_string()],
                )]),
            }),
            ..ProfileJob::default()
        });
        let resolution = resolve_runtime_scope_for_job(
            &manifest,
            Some("scoped-job"),
            BTreeMap::from([("product".to_string(), vec!["platform-a".to_string()])]),
        );
        let matched = match_entry_scope_for_job(
            &manifest,
            Some("scoped-job"),
            &resolution,
            &BTreeMap::from([(
                "capability".to_string(),
                vec!["developer-surface".to_string()],
            )]),
        );
        assert!(matched.compatible);
        assert_eq!(matched.predicates[0].status, "not-applicable");
    }

    #[test]
    fn structured_dimensions_require_and_match_across_the_contract() {
        let mut manifest = manifest();
        manifest.jobs.push(ProfileJob {
            id: "scoped-job".to_string(),
            selector_contract: Some(JobSelectorContract {
                contract: "mdp.job-selectors.v1".to_string(),
                required: vec!["product".to_string(), "segment".to_string()],
                dimensions: BTreeMap::from([
                    (
                        "product".to_string(),
                        vec!["platform-a".to_string(), "platform-b".to_string()],
                    ),
                    ("segment".to_string(), vec!["enterprise".to_string()]),
                ]),
            }),
            ..ProfileJob::default()
        });
        let resolution = resolve_runtime_scope_for_job(
            &manifest,
            Some("scoped-job"),
            BTreeMap::from([
                ("product".to_string(), vec!["platform-b".to_string()]),
                ("segment".to_string(), vec!["enterprise".to_string()]),
            ]),
        );
        let matched = match_entry_scope_for_job(
            &manifest,
            Some("scoped-job"),
            &resolution,
            &BTreeMap::from([
                ("product".to_string(), vec!["platform-b".to_string()]),
                ("segment".to_string(), vec!["enterprise".to_string()]),
            ]),
        );
        assert!(matched.compatible);

        let mismatch = match_entry_scope_for_job(
            &manifest,
            Some("scoped-job"),
            &resolution,
            &BTreeMap::from([
                ("product".to_string(), vec!["platform-a".to_string()]),
                ("segment".to_string(), vec!["mid-market".to_string()]),
            ]),
        );
        assert!(!mismatch.compatible);
        assert!(mismatch
            .predicates
            .iter()
            .any(|predicate| predicate.dimension == "segment" && predicate.status == "mismatch"));
    }

    #[test]
    fn legacy_job_without_selector_contract_remains_profile_compatible() {
        let resolution = resolve_runtime_scope_for_job(
            &manifest(),
            Some("legacy-job"),
            BTreeMap::from([("product".to_string(), vec!["platform-a".to_string()])]),
        );
        assert!(resolution.is_valid());
    }

    #[test]
    fn explicit_universal_predicate_is_not_inferred_from_empty_scope() {
        let resolution = resolve_runtime_scope(&manifest(), ContextScope::new());
        let implicit = match_entry_scope(&resolution, &ContextScope::new());
        assert!(implicit.compatible);
        assert!(implicit.predicates.is_empty());

        let explicit = match_entry_scope(
            &resolution,
            &BTreeMap::from([("universal".to_string(), vec!["true".to_string()])]),
        );
        assert!(explicit.compatible);
        assert!(is_explicit_universal(&BTreeMap::from([(
            "universal".to_string(),
            vec!["true".to_string()],
        )])));
        assert_eq!(explicit.predicates[0].status, "not-applicable");
    }
}
