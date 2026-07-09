use crate::models::{CardKind, QualificationGates};
use crate::pack_io::{read_cards_by_id_or_kind, read_manifest, read_prospect};
use crate::routing::{entry_context, entry_route, select_cards};
use crate::utils::slugify;
use crate::utils::{
    normalize_supplied_company_domain, prospect_haystack_with_persona, resolve_persona,
    resolve_persona_label, routable_persona,
};
use crate::value_contracts::prospect_contract_violations;
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(crate) fn route(
    root: &Path,
    persona: &str,
    job: &str,
    include_entries: bool,
    include_eval_fixture: bool,
) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let persona_resolution = resolve_persona_label(&manifest, persona);
    let resolved_persona = routable_persona(persona, &persona_resolution);
    let selected = select_cards(&manifest, Some(resolved_persona), Some(job));
    let load_order: Vec<String> = selected
        .iter()
        .filter_map(|v| v["path"].as_str().map(str::to_string))
        .collect();
    let mut payload = json!({
        "persona": resolved_persona,
        "requested_persona": persona,
        "persona_resolution": persona_resolution,
        "job": job,
        "route": selected,
        "decision_trace": [
            "manifest loaded",
            "persona resolved through pack-owned mappings when available",
            "resolved persona matched against card metadata",
            "job keywords matched against card descriptions and tags",
            "base policy cards included for guardrails"
        ],
        "load_order": load_order
    });
    if include_entries || include_eval_fixture {
        let routed_entries = entry_route(root, &manifest, resolved_persona, job)?;
        if include_eval_fixture {
            payload["eval_fixture"] =
                eval_fixture(persona, resolved_persona, job, &payload, &routed_entries);
        }
        if include_entries {
            payload["entry_route"] = json!(routed_entries);
        }
    }
    Ok(payload)
}

fn eval_fixture(
    requested_persona: &str,
    persona: &str,
    job: &str,
    route_output: &Value,
    routed_entries: &Value,
) -> Value {
    let expected_titles: Vec<Value> = routed_entries["matches"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry["status"].as_str() == Some("required"))
        .filter_map(|entry| entry["title"].as_str().map(|title| json!(title)))
        .take(8)
        .collect();
    json!({
        "id": slugify(&format!("{persona}-{job}")),
        "command": "route",
        "persona": persona,
        "requested_persona": requested_persona,
        "job": job,
        "expect_load_order_contains": route_output["load_order"],
        "expect_entry_titles_contains": expected_titles,
        "notes": [
            "Generated from current route output. Review before committing so evals encode desired behavior, not accidental noise.",
            "Add expect_load_order_excludes or expect_entry_titles_excludes for known wrong-route regressions."
        ]
    })
}

pub(crate) fn fit(root: &Path, prospect_path: &Path) -> Result<Value> {
    let prospect = read_prospect(prospect_path)?;
    fit_prospect(root, prospect)
}

pub(crate) fn fit_prospect(root: &Path, mut prospect: crate::models::Prospect) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let company_domain_normalization = normalize_company_domain_for_fit(&mut prospect);
    let fit_cards = read_cards_by_id_or_kind(root, "fit-rules", CardKind::FitRules)?;
    let mut matches = Vec::new();
    let mut disqualifiers = Vec::new();
    let persona_resolution = resolve_persona(&manifest, &prospect);
    let resolved_persona_for_fit = persona_resolution
        .fit_usable
        .then_some(persona_resolution.persona.as_str());
    let haystack = prospect_haystack_with_persona(&prospect, resolved_persona_for_fit);
    let context = fit_context(
        &manifest,
        &prospect,
        &persona_resolution,
        company_domain_normalization,
    );

    for fit_card in &fit_cards {
        for entry in &fit_card.entries {
            let entry_text = format!("{} {}", entry.title, entry.body).to_lowercase();
            let applies = entry.applies_to.iter().any(|candidate| {
                haystack.contains(&candidate.to_lowercase())
                    || prospect
                        .segment
                        .as_ref()
                        .map(|s| s.eq_ignore_ascii_case(candidate))
                        .unwrap_or(false)
            });
            let keyword_match = entry_text
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|token| token.len() >= 5)
                .any(|token| haystack.contains(token));
            if entry.avoid.is_empty() && (applies || keyword_match) {
                matches.push(json!({"id": entry.id, "title": entry.title, "reason": if applies { "segment/persona match" } else { "keyword match" }}));
            }
            for avoid in &entry.avoid {
                if contains_guardrail_term(&haystack, avoid) {
                    disqualifiers
                        .push(json!({"entry_id": entry.id, "term": avoid, "title": entry.title}));
                }
            }
        }
    }

    let status = if !disqualifiers.is_empty() {
        "disqualified"
    } else if !context["ready"].as_bool().unwrap_or(false) {
        "insufficient-context"
    } else if !matches.is_empty() {
        "fit"
    } else {
        "insufficient-context"
    };
    Ok(json!({
        "contract": "mdp.fit.v0",
        "prospect": prospect,
        "persona_resolution": persona_resolution,
        "status": status,
        "context": context,
        "matches": matches,
        "disqualifiers": disqualifiers,
        "decision": match status {
            "fit" => "Proceed to route/brief with stated assumptions.",
            "disqualified" => "Do not draft outbound copy unless the user overrides the disqualifier.",
            _ => "Ask for more context before drafting.",
        }
    }))
}

fn fit_context(
    manifest: &crate::models::Manifest,
    prospect: &crate::models::Prospect,
    persona_resolution: &crate::utils::PersonaResolution,
    company_domain_normalization: Value,
) -> Value {
    let has_trigger = prospect
        .trigger
        .as_ref()
        .is_some_and(|value| present(value));
    let has_persona = persona_resolution.fit_usable;
    let has_segment = prospect
        .segment
        .as_ref()
        .is_some_and(|value| present(value));
    let has_background = prospect
        .background
        .as_ref()
        .is_some_and(|value| present(value));
    let has_signal = !prospect.signals.is_empty();
    let has_source = prospect
        .signals
        .iter()
        .any(|signal| signal.source.as_ref().is_some_and(|value| present(value)));

    let mut missing_requirements = Vec::new();
    let mut invalid_requirements = Vec::new();
    if company_domain_normalization["status"].as_str() == Some("invalid") {
        invalid_requirements.push(json!({
            "scope": "prospect",
            "field": company_domain_normalization["field"].clone(),
            "path": company_domain_normalization["field"].clone(),
            "reason": company_domain_normalization["reason"].clone()
        }));
    }
    collect_attribute_issues(prospect, &mut invalid_requirements);
    let effective_persona = persona_resolution
        .fit_usable
        .then_some(persona_resolution.persona.as_str());
    for violation in prospect_contract_violations(manifest, prospect, effective_persona) {
        invalid_requirements.push(json!({
            "scope": violation.scope,
            "field": violation.field,
            "path": violation.path,
            "reason": violation.reason
        }));
    }

    for field in &manifest.lead_input_requirements.required_fields {
        if !prospect_field_present(field, prospect, persona_resolution) {
            missing_requirements.push(json!({
                "scope": "prospect",
                "field": field,
                "path": field,
                "reason": "required by manifest.lead_input_requirements.required_fields"
            }));
        }
    }

    for field in &manifest.lead_input_requirements.required_signal_fields {
        if prospect.signals.is_empty() {
            missing_requirements.push(json!({
                "scope": "signal",
                "field": field,
                "path": "signals",
                "reason": "required signal field cannot be checked because prospect.signals is empty"
            }));
            continue;
        }
        for (index, signal) in prospect.signals.iter().enumerate() {
            if !signal_field_present(field, signal) {
                missing_requirements.push(json!({
                    "scope": "signal",
                    "field": field,
                    "path": format!("signals[{index}].{field}"),
                    "reason": "required by manifest.lead_input_requirements.required_signal_fields"
                }));
            }
        }
    }

    for attribute in &manifest.lead_input_requirements.required_attributes {
        if !attribute_present(&prospect.attributes, attribute) {
            missing_requirements.push(json!({
                "scope": "attribute",
                "field": attribute,
                "path": format!("attributes.{attribute}"),
                "reason": "required by manifest.lead_input_requirements.required_attributes"
            }));
        }
    }

    let qualification_gate = qualification_gate_context(&manifest.qualification_gates, prospect);
    if let Some(gate) = manifest.qualification_gates.as_ref() {
        collect_qualification_gate_requirements(
            gate,
            &qualification_gate,
            &mut missing_requirements,
            &mut invalid_requirements,
        );
    }

    let mut missing = BTreeSet::new();
    for requirement in missing_requirements
        .iter()
        .chain(invalid_requirements.iter())
    {
        if let Some(path) = requirement["path"].as_str() {
            if path.starts_with("signals[") && path.ends_with(".source") {
                missing.insert("signals.source".to_string());
            } else {
                missing.insert(path.to_string());
            }
        }
    }
    json!({
        "ready": missing_requirements.is_empty() && invalid_requirements.is_empty(),
        "lead_input_requirements": &manifest.lead_input_requirements,
        "has_trigger": has_trigger,
        "has_persona": has_persona,
        "has_segment": has_segment,
        "has_background": has_background,
        "has_signals": has_signal,
        "has_signal_source": has_source,
        "qualification_gate": qualification_gate,
        "normalization": {
            "company_domain": company_domain_normalization
        },
        "missing": missing.into_iter().collect::<Vec<_>>(),
        "missing_requirements": missing_requirements,
        "invalid_requirements": invalid_requirements
    })
}

fn qualification_gate_context(
    gate: &Option<QualificationGates>,
    prospect: &crate::models::Prospect,
) -> Value {
    let source_backed_indexes = prospect
        .signals
        .iter()
        .enumerate()
        .filter(|(_, signal)| signal.source.as_ref().is_some_and(|value| present(value)))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let source_backed_count = source_backed_indexes.len();
    let fit_signal_indexes = source_backed_indexes
        .iter()
        .copied()
        .filter(|index| signal_text_matches(&prospect.signals[*index], FIT_SIGNAL_TERMS))
        .collect::<Vec<_>>();
    let why_now_signal_indexes = source_backed_indexes
        .iter()
        .copied()
        .filter(|index| signal_text_matches(&prospect.signals[*index], WHY_NOW_SIGNAL_TERMS))
        .collect::<Vec<_>>();
    let person_resolution_indexes = source_backed_indexes
        .iter()
        .copied()
        .filter(|index| signal_text_matches(&prospect.signals[*index], PERSON_RESOLUTION_TERMS))
        .collect::<Vec<_>>();
    let public_person_url = prospect
        .linkedin_url
        .as_deref()
        .is_some_and(public_person_url_present);
    let person_resolution = present(&prospect.name)
        && present(&prospect.title)
        && (public_person_url || !person_resolution_indexes.is_empty());

    json!({
        "enabled": gate.is_some(),
        "person_resolution": {
            "required": gate.as_ref().is_some_and(|gate| gate.require_person_resolution),
            "resolved": person_resolution,
            "public_person_url": public_person_url,
            "signal_indexes": person_resolution_indexes,
        },
        "signals": {
            "source_backed_count": source_backed_count,
            "source_backed_indexes": source_backed_indexes,
            "fit_signal_indexes": fit_signal_indexes,
            "why_now_signal_indexes": why_now_signal_indexes,
        },
        "fail_policy": gate
            .as_ref()
            .and_then(|gate| gate.fail_policy.as_ref())
            .map(|_| "insufficient_context")
            .unwrap_or("insufficient_context")
    })
}

fn collect_qualification_gate_requirements(
    gate: &QualificationGates,
    gate_context: &Value,
    missing_requirements: &mut Vec<Value>,
    invalid_requirements: &mut Vec<Value>,
) {
    if gate.require_person_resolution
        && !gate_context["person_resolution"]["resolved"]
            .as_bool()
            .unwrap_or(false)
    {
        missing_requirements.push(qualification_issue(
            "person_resolution",
            "qualification_gates.require_person_resolution",
            "requires public person-level resolution with name, title, and a person-scoped public URL or source-backed person-resolution signal",
        ));
    }

    let source_backed_count = gate_context["signals"]["source_backed_count"]
        .as_u64()
        .unwrap_or(0) as usize;
    if gate
        .signals
        .min
        .is_some_and(|minimum| source_backed_count < minimum)
    {
        missing_requirements.push(qualification_issue(
            "signals",
            "qualification_gates.signals.min",
            &format!(
                "requires at least {} source-backed signal(s); found {source_backed_count}",
                gate.signals.min.unwrap_or_default()
            ),
        ));
    }
    if gate
        .signals
        .max
        .is_some_and(|maximum| source_backed_count > maximum)
    {
        invalid_requirements.push(qualification_issue(
            "signals",
            "qualification_gates.signals.max",
            &format!(
                "requires at most {} source-backed signal(s); found {source_backed_count}",
                gate.signals.max.unwrap_or_default()
            ),
        ));
    }
    if gate.signals.require_fit_signal
        && gate_context["signals"]["fit_signal_indexes"]
            .as_array()
            .map(Vec::is_empty)
            .unwrap_or(true)
    {
        missing_requirements.push(qualification_issue(
            "fit_signal",
            "qualification_gates.signals.require_fit_signal",
            "requires at least one source-backed fit signal tied to role, persona, account, ICP, category, or signal fit",
        ));
    }
    if gate.signals.require_why_now_signal
        && gate_context["signals"]["why_now_signal_indexes"]
            .as_array()
            .map(Vec::is_empty)
            .unwrap_or(true)
    {
        missing_requirements.push(qualification_issue(
            "why_now_signal",
            "qualification_gates.signals.require_why_now_signal",
            "requires at least one source-backed why-now signal tied to trigger, timing, priority, change, launch, hiring, demand, or opportunity",
        ));
    }
}

fn qualification_issue(field: &str, path: &str, reason: &str) -> Value {
    json!({
        "scope": "qualification_gate",
        "field": field,
        "path": path,
        "reason": reason
    })
}

const FIT_SIGNAL_TERMS: &[&str] = &[
    "fit",
    "icp",
    "persona",
    "role",
    "title",
    "ownership",
    "owner",
    "account",
    "category",
    "search",
    "seo",
    "aeo",
    "agency",
    "content",
    "brand",
    "pr",
];

const WHY_NOW_SIGNAL_TERMS: &[&str] = &[
    "why-now",
    "why now",
    "trigger",
    "timing",
    "fresh",
    "current",
    "recent",
    "change",
    "launch",
    "hiring",
    "priority",
    "demand",
    "opportunity",
    "initiative",
    "project",
    "moment",
    "interest",
    "mentions",
];

const PERSON_RESOLUTION_TERMS: &[&str] = &[
    "person",
    "profile",
    "role resolution",
    "person-role",
    "named",
    "linkedin.com/in/",
    "author",
    "bio",
    "team",
];

fn signal_text_matches(signal: &crate::models::Signal, terms: &[&str]) -> bool {
    let text = format!(
        "{}\n{}\n{}",
        signal.id,
        signal.title,
        signal.source.as_deref().unwrap_or("")
    )
    .to_lowercase();
    let tokens = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    terms.iter().any(|term| {
        if term.chars().all(|c| c.is_ascii_alphanumeric()) {
            tokens.iter().any(|token| token == term)
        } else {
            text.contains(term)
        }
    })
}

fn public_person_url_present(url: &str) -> bool {
    let value = url.trim().to_lowercase();
    present(&value)
        && (value.contains("linkedin.com/in/")
            || value.contains("/author/")
            || value.contains("/team/")
            || value.contains("/people/")
            || value.contains("/person/")
            || value.contains("/bio/"))
}

fn normalize_company_domain_for_fit(prospect: &mut crate::models::Prospect) -> Value {
    if let Some(input) = prospect
        .company_domain
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    {
        return match normalize_supplied_company_domain(&input) {
            Ok(canonical) => {
                let changed = canonical != input;
                prospect.company_domain = Some(canonical.clone());
                json!({
                    "status": "normalized",
                    "field": "company_domain",
                    "input": input,
                    "value": canonical,
                    "changed": changed
                })
            }
            Err(err) => json!({
                "status": "invalid",
                "field": "company_domain",
                "input": input,
                "reason": err.to_string()
            }),
        };
    }

    if let Some(input) = prospect
        .company_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    {
        return match normalize_supplied_company_domain(&input) {
            Ok(canonical) => {
                prospect.company_domain = Some(canonical.clone());
                json!({
                    "status": "normalized",
                    "field": "company_url",
                    "input": input,
                    "value": canonical,
                    "changed": true
                })
            }
            Err(err) => json!({
                "status": "invalid",
                "field": "company_url",
                "input": input,
                "reason": err.to_string()
            }),
        };
    }

    json!({
        "status": "missing",
        "field": "company_domain",
        "reason": "no supplied company_domain or domain-like company_url"
    })
}

fn prospect_field_present(
    field: &str,
    prospect: &crate::models::Prospect,
    persona_resolution: &crate::utils::PersonaResolution,
) -> bool {
    match field {
        "name" => present(&prospect.name),
        "title" => present(&prospect.title),
        "company" => present(&prospect.company),
        "company_domain" => prospect
            .company_domain
            .as_ref()
            .is_some_and(|value| present(value)),
        "source_kind" => prospect
            .source_kind
            .as_ref()
            .is_some_and(|value| present(value)),
        "synthetic" => true,
        "linkedin_url" => prospect
            .linkedin_url
            .as_ref()
            .is_some_and(|value| present(value)),
        "company_url" => prospect
            .company_url
            .as_ref()
            .is_some_and(|value| present(value)),
        "background" => prospect
            .background
            .as_ref()
            .is_some_and(|value| present(value)),
        "trigger" => prospect
            .trigger
            .as_ref()
            .is_some_and(|value| present(value)),
        "persona" => persona_resolution.fit_usable,
        "segment" => prospect
            .segment
            .as_ref()
            .is_some_and(|value| present(value)),
        "signals" => !prospect.signals.is_empty(),
        _ => false,
    }
}

fn signal_field_present(field: &str, signal: &crate::models::Signal) -> bool {
    match field {
        "id" => present(&signal.id),
        "title" => present(&signal.title),
        "source" => signal.source.as_ref().is_some_and(|value| present(value)),
        "confidence" => signal
            .confidence
            .as_ref()
            .is_some_and(|value| present(value)),
        "freshness" => signal
            .freshness
            .as_ref()
            .is_some_and(|value| present(value)),
        "state_as" => signal.state_as.as_ref().is_some_and(|value| present(value)),
        _ => false,
    }
}

fn collect_attribute_issues(
    prospect: &crate::models::Prospect,
    invalid_requirements: &mut Vec<Value>,
) {
    if prospect.attributes.len() > 25 {
        invalid_requirements.push(json!({
            "scope": "attribute",
            "field": "attributes",
            "path": "attributes",
            "reason": "attributes must contain at most 25 reviewed metadata keys"
        }));
    }
    for (key, value) in &prospect.attributes {
        if !valid_attribute_key(key) {
            invalid_requirements.push(json!({
                "scope": "attribute",
                "field": key,
                "path": format!("attributes.{key}"),
                "reason": "attribute keys must start with a letter and contain only letters, numbers, underscores, or hyphens"
            }));
        }
        if !supported_attribute_value(value) {
            invalid_requirements.push(json!({
                "scope": "attribute",
                "field": key,
                "path": format!("attributes.{key}"),
                "reason": "attribute values must be strings, numbers, or booleans; use signals with sources for evidence"
            }));
        }
    }
}

fn attribute_present(attributes: &std::collections::BTreeMap<String, Value>, key: &str) -> bool {
    attributes
        .get(key)
        .is_some_and(|value| meaningful_json_value(value))
}

fn meaningful_json_value(value: &Value) -> bool {
    if let Some(value) = value.as_str() {
        return present(value);
    }
    value.is_number() || value.is_boolean()
}

fn supported_attribute_value(value: &Value) -> bool {
    value.is_string() || value.is_number() || value.is_boolean()
}

fn valid_attribute_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && key.len() <= 64
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn present(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "n/a" | "na" | "unknown" | "unknown contact" | "unknown role"
        )
}

pub(crate) fn check_claims(
    root: &Path,
    text: Option<&str>,
    file: Option<&Path>,
    subject: Option<&str>,
    persona: Option<&str>,
    job: Option<&str>,
) -> Result<Value> {
    if persona.is_some() != job.is_some() {
        return Err(anyhow!(
            "pass both --persona and --job for route-scoped constraint checks"
        ));
    }
    let raw = match (text, file) {
        (Some(value), None) => value.to_string(),
        (None, Some(path)) => {
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?
        }
        (Some(_), Some(_)) => return Err(anyhow!("pass either --text or --file, not both")),
        (None, None) => return Err(anyhow!("pass --text or --file")),
    };
    let paragraph_count = count_paragraphs(&raw);
    let lower = raw.to_lowercase();
    let claims_cards = read_cards_by_id_or_kind(root, "claims", CardKind::Claims)?;
    let avoid_cards = read_cards_by_id_or_kind(root, "avoid-rules", CardKind::AvoidRules)?;
    let output_rules_cards =
        read_cards_by_id_or_kind(root, "output-rules", CardKind::OutputRules).unwrap_or_default();
    let approved_claim_context = claims_cards
        .iter()
        .flat_map(|card| card.entries.iter())
        .map(|entry| {
            format!(
                "{} {} {}",
                entry.title,
                entry.body,
                entry.evidence.join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let mut matched_claims = Vec::new();
    let mut claim_gaps = Vec::new();
    let mut guardrail_hits = Vec::new();
    let mut constraint_warnings = Vec::new();
    let mut unchecked_constraints = Vec::new();
    let unsupported_claims = unsupported_claims(&lower, &approved_claim_context);
    let mut route_persona_resolution = Value::Null;
    let mut resolved_persona_for_output: Option<String> = None;

    for card in &claims_cards {
        collect_guardrail_hits(&mut guardrail_hits, &lower, &card.id, &card.entries);
        for entry in &card.entries {
            let title = entry.title.to_lowercase();
            let title_match = title.len() > 4 && lower.contains(&title);
            let evidence_missing = entry.evidence.is_empty();
            if title_match {
                matched_claims.push(json!({"id": entry.id, "title": entry.title, "evidence": entry.evidence, "evidence_missing": evidence_missing}));
                if evidence_missing {
                    claim_gaps.push(json!({"id": entry.id, "title": entry.title, "reason": "matched claim has no evidence"}));
                }
            }
        }
    }
    for card in &avoid_cards {
        collect_guardrail_hits(&mut guardrail_hits, &lower, &card.id, &card.entries);
    }
    for card in &output_rules_cards {
        collect_guardrail_hits(&mut guardrail_hits, &lower, &card.id, &card.entries);
        collect_output_structure_hits(&mut guardrail_hits, paragraph_count, &card.entries);
        collect_output_constraint_hits(
            &mut guardrail_hits,
            &mut constraint_warnings,
            &mut unchecked_constraints,
            &raw,
            subject,
            &card.id,
            &card.entries,
        );
    }
    if let (Some(persona), Some(job)) = (persona, job) {
        let manifest = read_manifest(root)?;
        let persona_resolution = resolve_persona_label(&manifest, persona);
        let resolved_persona = routable_persona(persona, &persona_resolution);
        resolved_persona_for_output = Some(resolved_persona.to_string());
        let context = entry_context(root, &manifest, resolved_persona, job, true)?;
        route_persona_resolution = serde_json::to_value(&persona_resolution)?;
        collect_context_constraint_hits(
            &mut guardrail_hits,
            &mut constraint_warnings,
            &mut unchecked_constraints,
            &raw,
            subject,
            &context,
        );
    }
    let valid = guardrail_hits.is_empty() && claim_gaps.is_empty() && unsupported_claims.is_empty();
    Ok(json!({
        "contract": "mdp.claim-check.v0",
        "valid": valid,
        "checked": {
            "subject": subject.is_some(),
            "route_scoped": persona.is_some() && job.is_some(),
            "persona": persona,
            "resolved_persona": resolved_persona_for_output,
            "job": job
        },
        "persona_resolution": route_persona_resolution,
        "matched_claims": matched_claims,
        "claim_gaps": claim_gaps,
        "guardrail_hits": guardrail_hits,
        "constraint_warnings": constraint_warnings,
        "unchecked_constraints": unchecked_constraints,
        "unsupported_claims": unsupported_claims,
        "decision": if valid { "claim-safe" } else { "needs-revision" }
    }))
}

fn collect_guardrail_hits(
    guardrail_hits: &mut Vec<Value>,
    lower: &str,
    card_id: &str,
    entries: &[crate::models::Entry],
) {
    for entry in entries {
        for term in &entry.avoid {
            if contains_guardrail_term(lower, term) {
                guardrail_hits.push(
                    json!({"card_id": card_id, "entry_id": entry.id, "term": term, "title": entry.title}),
                );
            }
        }
    }
}

fn collect_output_structure_hits(
    guardrail_hits: &mut Vec<Value>,
    paragraph_count: usize,
    entries: &[crate::models::Entry],
) {
    for entry in entries {
        if let Some(expected) = entry.exact_paragraphs {
            if paragraph_count != expected {
                guardrail_hits.push(json!({
                    "card_id": "output-rules",
                    "entry_id": entry.id,
                    "title": entry.title,
                    "rule": "exact_paragraphs",
                    "expected": expected,
                    "actual": paragraph_count
                }));
            }
        }
    }
}

fn collect_output_constraint_hits(
    guardrail_hits: &mut Vec<Value>,
    constraint_warnings: &mut Vec<Value>,
    unchecked_constraints: &mut Vec<Value>,
    raw: &str,
    subject: Option<&str>,
    card_id: &str,
    entries: &[crate::models::Entry],
) {
    for entry in entries {
        if entry.constraints.is_empty() {
            continue;
        }
        collect_constraints(
            guardrail_hits,
            constraint_warnings,
            unchecked_constraints,
            raw,
            subject,
            card_id,
            &entry.id,
            &entry.title,
            &json!(entry.constraints),
        );
    }
}

fn collect_context_constraint_hits(
    guardrail_hits: &mut Vec<Value>,
    constraint_warnings: &mut Vec<Value>,
    unchecked_constraints: &mut Vec<Value>,
    raw: &str,
    subject: Option<&str>,
    context: &Value,
) {
    let Some(entries) = context["entries"].as_array() else {
        return;
    };
    for entry in entries {
        if entry["card_kind"].as_str() == Some("output-rules") {
            continue;
        }
        let constraints = &entry["constraints"];
        if constraints
            .as_object()
            .is_none_or(|object| object.is_empty())
        {
            continue;
        }
        collect_constraints(
            guardrail_hits,
            constraint_warnings,
            unchecked_constraints,
            raw,
            subject,
            entry["card_id"].as_str().unwrap_or("unknown"),
            entry["entry_id"].as_str().unwrap_or("unknown"),
            entry["title"].as_str().unwrap_or("Untitled"),
            constraints,
        );
    }
}

fn collect_constraints(
    guardrail_hits: &mut Vec<Value>,
    constraint_warnings: &mut Vec<Value>,
    unchecked_constraints: &mut Vec<Value>,
    raw: &str,
    subject: Option<&str>,
    card_id: &str,
    entry_id: &str,
    title: &str,
    constraints: &Value,
) {
    if let Some(word_count) = constraints.get("word_count") {
        collect_count_constraint(
            guardrail_hits,
            constraint_warnings,
            card_id,
            entry_id,
            title,
            "constraints.word_count",
            count_words(raw),
            word_count,
        );
    }

    if constraints.get("subject_words").is_some() || constraints.get("subject_avoid").is_some() {
        if let Some(subject) = subject {
            if let Some(subject_words) = constraints.get("subject_words") {
                collect_count_constraint(
                    guardrail_hits,
                    constraint_warnings,
                    card_id,
                    entry_id,
                    title,
                    "constraints.subject_words",
                    count_words(subject),
                    subject_words,
                );
            }
            if let Some(avoid) = constraints["subject_avoid"].as_array() {
                let subject_lower = subject.to_lowercase();
                for term in avoid.iter().filter_map(|value| value.as_str()) {
                    if subject_lower.contains(&term.to_lowercase()) {
                        guardrail_hits.push(json!({
                            "card_id": card_id,
                            "entry_id": entry_id,
                            "title": title,
                            "rule": "constraints.subject_avoid",
                            "term": term
                        }));
                    }
                }
            }
        } else {
            unchecked_constraints.push(json!({
                "card_id": card_id,
                "entry_id": entry_id,
                "title": title,
                "rule": "constraints.subject",
                "reason": "No subject was supplied. Pass --subject to check subject word counts and subject avoid literals."
            }));
        }
    }

    if let Some(max_questions) = constraints["max_questions"].as_u64() {
        let actual = raw.chars().filter(|character| *character == '?').count();
        if actual > max_questions as usize {
            guardrail_hits.push(json!({
                "card_id": card_id,
                "entry_id": entry_id,
                "title": title,
                "rule": "constraints.max_questions",
                "expected": {"max": max_questions},
                "actual": actual
            }));
        }
    }

    if constraints["forbid_links"].as_bool() == Some(true) && contains_link(raw) {
        guardrail_hits.push(forbid_hit(
            card_id,
            entry_id,
            title,
            "constraints.forbid_links",
            "draft contains a URL, www. link, or email-like address",
        ));
    }
    if constraints["forbid_html"].as_bool() == Some(true) && contains_html(raw) {
        guardrail_hits.push(forbid_hit(
            card_id,
            entry_id,
            title,
            "constraints.forbid_html",
            "draft contains HTML-like markup",
        ));
    }
    if constraints["forbid_images"].as_bool() == Some(true) {
        if contains_image_reference(raw) {
            guardrail_hits.push(forbid_hit(
                card_id,
                entry_id,
                title,
                "constraints.forbid_images",
                "draft contains an image reference",
            ));
        }
        unchecked_constraints.push(metadata_note(
            card_id,
            entry_id,
            title,
            "constraints.forbid_images",
            "check-claims can inspect text references, but cannot verify embedded images outside the supplied draft text.",
        ));
    }
    if constraints["forbid_attachments"].as_bool() == Some(true) {
        if contains_attachment_reference(raw) {
            guardrail_hits.push(forbid_hit(
                card_id,
                entry_id,
                title,
                "constraints.forbid_attachments",
                "draft contains an attachment reference",
            ));
        }
        unchecked_constraints.push(metadata_note(
            card_id,
            entry_id,
            title,
            "constraints.forbid_attachments",
            "check-claims can inspect text references, but cannot verify actual file attachments outside the supplied draft text.",
        ));
    }
    if constraints["forbid_tracking"].as_bool() == Some(true) {
        if contains_tracking_reference(raw) {
            guardrail_hits.push(forbid_hit(
                card_id,
                entry_id,
                title,
                "constraints.forbid_tracking",
                "draft contains tracking-related parameters or language",
            ));
        }
        unchecked_constraints.push(metadata_note(
            card_id,
            entry_id,
            title,
            "constraints.forbid_tracking",
            "check-claims can inspect draft text, but cannot verify open tracking, click tracking, or tracking pixels configured by a sending surface.",
        ));
    }
}

fn collect_count_constraint(
    guardrail_hits: &mut Vec<Value>,
    constraint_warnings: &mut Vec<Value>,
    card_id: &str,
    entry_id: &str,
    title: &str,
    rule: &str,
    actual: usize,
    constraint: &Value,
) {
    let min = constraint["min"].as_u64().map(|value| value as usize);
    let max = constraint["max"].as_u64().map(|value| value as usize);
    if min.is_some_and(|value| actual < value) || max.is_some_and(|value| actual > value) {
        guardrail_hits.push(json!({
            "card_id": card_id,
            "entry_id": entry_id,
            "title": title,
            "rule": rule,
            "expected": {"min": min, "max": max},
            "actual": actual
        }));
    }

    let target_min = constraint["target_min"]
        .as_u64()
        .map(|value| value as usize);
    let target_max = constraint["target_max"]
        .as_u64()
        .map(|value| value as usize);
    if target_min.is_some_and(|value| actual < value)
        || target_max.is_some_and(|value| actual > value)
    {
        constraint_warnings.push(json!({
            "card_id": card_id,
            "entry_id": entry_id,
            "title": title,
            "rule": format!("{rule}.target"),
            "expected": {"target_min": target_min, "target_max": target_max},
            "actual": actual
        }));
    }
}

fn count_paragraphs(raw: &str) -> usize {
    let mut count = 0;
    let mut in_paragraph = false;
    for line in raw.lines() {
        if line.trim().is_empty() {
            in_paragraph = false;
        } else if !in_paragraph {
            count += 1;
            in_paragraph = true;
        }
    }
    count
}

fn count_words(raw: &str) -> usize {
    raw.split(|character: char| !(character.is_ascii_alphanumeric() || character == '\''))
        .filter(|token| {
            token
                .chars()
                .any(|character| character.is_ascii_alphanumeric())
        })
        .count()
}

fn forbid_hit(card_id: &str, entry_id: &str, title: &str, rule: &str, reason: &str) -> Value {
    json!({
        "card_id": card_id,
        "entry_id": entry_id,
        "title": title,
        "rule": rule,
        "reason": reason
    })
}

fn metadata_note(card_id: &str, entry_id: &str, title: &str, rule: &str, reason: &str) -> Value {
    json!({
        "card_id": card_id,
        "entry_id": entry_id,
        "title": title,
        "rule": rule,
        "reason": reason
    })
}

fn contains_link(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("www.")
        || raw
            .split_whitespace()
            .any(|token| token.contains('@') && token.contains('.'))
}

fn contains_html(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    [
        "<a ",
        "<br",
        "<div",
        "<html",
        "<img",
        "<p>",
        "<p ",
        "<span",
        "<table",
        "<!doctype",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn contains_image_reference(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    lower.contains("![")
        || lower.contains("<img")
        || [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg"]
            .iter()
            .any(|extension| lower.contains(extension))
}

fn contains_attachment_reference(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    contains_any(
        &lower,
        &[
            "attached",
            "attachment",
            "see the file",
            "see file",
            ".pdf",
            ".doc",
            ".docx",
            ".ppt",
            ".pptx",
            ".xls",
            ".xlsx",
            ".csv",
        ],
    )
}

fn contains_tracking_reference(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    contains_any(
        &lower,
        &[
            "utm_",
            "utm-",
            "tracking pixel",
            "open tracking",
            "click tracking",
            "pixel.gif",
        ],
    )
}

fn unsupported_claims(text: &str, approved_context: &str) -> Vec<Value> {
    let mut hits = Vec::new();
    let mut push_hit = |category: &str, trigger: &str, reason: &str| {
        if !approved_context.contains(trigger) {
            hits.push(json!({
                "category": category,
                "trigger": trigger,
                "reason": reason
            }));
        }
    };

    let has_number = text.chars().any(|c| c.is_ascii_digit());
    if ((text.contains('%') || text.contains(" percent") || has_number)
        && contains_actionable_any(
            text,
            &[
                "improves reply rates",
                "improve reply rates",
                "increases reply rates",
                "increase reply rates",
                "books meetings",
                "book meetings",
                "pipeline",
                "revenue",
                "conversion",
                "roi",
            ],
        ))
        || contains_actionable_any(
            text,
            &[
                "doubles reply rates",
                "double reply rates",
                "2x reply rates",
                "2x pipeline",
                "cuts research time in half",
                "cut research time in half",
                "saves hours",
            ],
        )
    {
        push_hit(
            "quantified-outcome",
            "quantified outcome",
            "Quantified performance or ROI claims require explicit approved evidence.",
        );
    }
    if contains_actionable_any(text, &["guarantee", "guarantees", "guaranteed"])
        && contains_actionable_any(text, &["meeting", "meetings", "reply", "replies"])
    {
        push_hit(
            "quantified-outcome",
            "guarantee",
            "Guaranteed meetings, replies, or outcomes are unsupported unless explicitly approved.",
        );
    }
    if contains_actionable_any(
        text,
        &[
            "integrates with",
            "integrate with",
            "integration with",
            "connects to",
            "connect to",
            "syncs with",
            "sync with",
            "pushes to",
            "push to",
            "writes to",
            "write to",
            "works inside",
            "native integration",
            "crm integration",
        ],
    ) && contains_actionable_any(
        text,
        &["salesforce", "hubspot", "outreach", "salesloft", "crm"],
    ) {
        push_hit(
            "integration",
            "integration",
            "Integration claims require an approved product capability claim.",
        );
    }
    if contains_actionable_any(
        text,
        &[
            "soc 2",
            "soc2",
            "hipaa",
            "gdpr",
            "cmmc",
            "fedramp",
            "compliant",
            "secure",
            "security certified",
            "security-approved",
            "security approved",
            "security-ready",
            "security ready",
            "compliance certified",
            "compliance-approved",
            "compliance approved",
            "compliance approval",
            "compliance-ready",
            "compliance ready",
            "handles compliance",
            "handle compliance",
            "manages compliance",
            "manage compliance",
            "approved for procurement",
        ],
    ) {
        push_hit(
            "compliance-security",
            "compliance/security",
            "Compliance and security claims require explicit approved evidence.",
        );
    }
    if contains_actionable_any(
        text,
        &[
            "trusted by",
            "used by",
            "customers already use",
            "customers rely on",
            "teams already use",
            "customer adoption",
            "production adoption",
            "production use",
            "production rollout",
            "customer deployment",
            "live customer",
            "deployed in production",
            "in production with customers",
            "validated adoption",
            "design partner",
            "design partners",
            "paid pilot",
            "paid pilots",
            "customers include",
            "customer like",
            "customers like",
        ],
    ) {
        push_hit(
            "customer-name",
            "customer proof",
            "Customer-name and social-proof claims require explicit approved source context.",
        );
    }
    if contains_actionable_any(
        text,
        &[
            "arr conversion",
            "workshop conversion",
            "workshops converted",
            "market validated",
            "market validation",
            "validated by the market",
        ],
    ) {
        push_hit(
            "commercial-traction",
            "commercial traction",
            "Commercial traction and market-validation claims require explicit approved source context.",
        );
    }
    if contains_actionable_any(
        text,
        &[
            "updates crm",
            "update crm",
            "writes to crm",
            "write to crm",
            "send emails",
            "sends emails",
            "send linkedin",
            "sends linkedin",
            "auto-send",
            "auto-sends",
            "auto sends",
            "autosend",
            "sequence prospects",
            "sequencer",
            "books meetings",
            "book meetings",
            "launches campaigns",
            "launch campaigns",
            "sends for you",
            "send for you",
            "owns follow-up",
            "own follow-up",
            "owns the follow-up",
            "own the follow-up",
            "autonomously sends",
        ],
    ) {
        push_hit(
            "execution-crm-sending",
            "execution",
            "MDP stops at pack, route, and brief; execution claims require a separate exact-action tool.",
        );
    }
    if contains_actionable_any(
        text,
        &[
            "bypass legal",
            "bypasses legal",
            "bypass procurement",
            "bypasses procurement",
            "skip legal",
            "skips legal",
            "skip procurement",
            "skips procurement",
            "no legal review",
            "no procurement review",
            "approval not needed",
            "legal approved",
            "procurement approved",
        ],
    ) {
        push_hit(
            "legal-procurement-bypass",
            "legal/procurement bypass",
            "Legal, procurement, and approval-bypass claims require explicit reviewed context.",
        );
    }
    if contains_actionable_any(
        text,
        &[
            "ai can own the response",
            "ai owns the response",
            "ai decides",
            "ai-approved",
            "ai approved",
            "autonomously writes",
            "fully automated writing",
            "fully automated proposal",
            "hands-free drafting",
            "hands free drafting",
        ],
    ) {
        push_hit(
            "ai-authoritative",
            "AI authoritative",
            "AI-authoritative language must not replace human review or approved source context.",
        );
    }
    if contains_actionable_any(
        text,
        &[
            "replaces proposal management",
            "replace proposal management",
            "replaces compliance review",
            "replace compliance review",
            "rfp automation platform",
            "automates rfp responses",
            "automate rfp responses",
            "responds to rfps",
            "respond to rfps",
            "submits proposals",
            "submit proposals",
            "proposal platform replacement",
            "fully automated proposal writing",
        ],
    ) {
        push_hit(
            "rfp-platform-replacement",
            "RFP platform replacement",
            "RFP/proposal platform replacement claims are outside MDP's local decision-context boundary.",
        );
    }
    if contains_actionable_any(
        text,
        &[
            "i loved your recent linkedin post",
            "i loved your post",
            "your recent linkedin post",
            "noticed your podcast",
            "saw your webinar",
            "i watched your webinar",
            "checked out your profile",
        ],
    ) {
        push_hit(
            "fake-personalization",
            "fake personalization",
            "Personalization claims need supplied source context and should not be invented from a thin row.",
        );
    }
    if contains_actionable_any(
        text,
        &[
            "best-in-class",
            "battle-tested",
            "field-tested",
            "enterprise-ready",
            "operator-approved",
            "expert-approved",
        ],
    ) {
        push_hit(
            "weak-trust",
            "weak trust",
            "Weak trust and approval claims require explicit approved evidence.",
        );
    }

    hits
}

fn contains_guardrail_term(text: &str, term: &str) -> bool {
    let needle = term.trim().to_lowercase();
    if needle.is_empty() {
        return false;
    }

    text.match_indices(&needle).any(|(start, _)| {
        let end = start + needle.len();
        has_phrase_boundaries(text, start, end, &needle)
            && !is_obviously_negated_match(text, start, &needle)
            && !is_coordinated_negation_match(text, start)
            && !is_disclaimed_match(text, start)
    })
}

fn has_phrase_boundaries(text: &str, start: usize, end: usize, needle: &str) -> bool {
    let starts_with_word = needle.chars().next().is_some_and(is_word_char);
    let ends_with_word = needle.chars().next_back().is_some_and(is_word_char);
    let previous_ok = !starts_with_word
        || text[..start]
            .chars()
            .next_back()
            .is_none_or(|value| !is_word_char(value));
    let next_ok = !ends_with_word
        || text[end..]
            .chars()
            .next()
            .is_none_or(|value| !is_word_char(value));

    previous_ok && next_ok
}

fn is_obviously_negated_match(text: &str, start: usize, needle: &str) -> bool {
    if !negation_eligible(needle) {
        return false;
    }
    let words: Vec<&str> = text[..start]
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '\''))
        .filter(|word| !word.is_empty())
        .collect();
    let window_start = words.len().saturating_sub(7);
    let window = &words[window_start..];

    for index in (0..window.len()).rev() {
        if !is_negator(window[index]) {
            continue;
        }
        let bridge = &window[index + 1..];
        return bridge.is_empty()
            || bridge.iter().all(|word| is_negation_bridge(word))
            || is_negated_coordination_bridge(bridge);
    }

    false
}

fn negation_eligible(needle: &str) -> bool {
    needle.chars().any(|c| c.is_ascii_alphabetic())
        && !needle.contains("://")
        && !needle.contains("www.")
        && !needle.contains("utm_")
        && !needle.contains('<')
}

fn is_negator(word: &str) -> bool {
    matches!(
        word,
        "not"
            | "no"
            | "never"
            | "cannot"
            | "cant"
            | "can't"
            | "dont"
            | "don't"
            | "doesnt"
            | "doesn't"
            | "didnt"
            | "didn't"
            | "wont"
            | "won't"
    )
}

fn is_negation_bridge(word: &str) -> bool {
    matches!(word, "a" | "an" | "the" | "any" | "to" | "be" | "as")
}

fn is_negated_coordination_bridge(bridge: &[&str]) -> bool {
    bridge.len() <= 4
        && bridge
            .last()
            .is_some_and(|word| matches!(*word, "and" | "or" | "nor"))
        && !bridge
            .iter()
            .any(|word| matches!(*word, "only" | "just" | "merely"))
}

fn contains_actionable_any(text: &str, needles: &[&str]) -> bool {
    needles
        .iter()
        .any(|needle| contains_guardrail_term(text, needle))
}

fn is_coordinated_negation_match(text: &str, start: usize) -> bool {
    let prefix = &text[..start];
    let sentence_prefix =
        match prefix.rfind(|character| matches!(character, '.' | '!' | '?' | '\n' | ';')) {
            Some(index) => &prefix[index + 1..],
            None => prefix,
        };

    let Some((marker, marker_start)) = [
        "does not ",
        "do not ",
        "did not ",
        "doesn't ",
        "don't ",
        "cannot ",
        "can't ",
        "not ",
        "no ",
        "without ",
    ]
    .iter()
    .filter_map(|marker| sentence_prefix.rfind(marker).map(|start| (*marker, start)))
    .max_by_key(|(_, start)| *start) else {
        return false;
    };

    let after_marker = &sentence_prefix[marker_start + marker.len()..];
    if contains_any(after_marker, &[" but ", " however ", " although "]) {
        return false;
    }
    let bridge_words = after_marker
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '\''))
        .filter(|word| !word.is_empty())
        .count();

    bridge_words <= 10
        && (after_marker.contains(',')
            || after_marker.contains(" and ")
            || after_marker.contains(" or "))
}

fn is_disclaimed_match(text: &str, start: usize) -> bool {
    let prefix = &text[..start];
    let sentence_prefix =
        match prefix.rfind(|character| matches!(character, '.' | '!' | '?' | '\n' | ';')) {
            Some(index) => &prefix[index + 1..],
            None => prefix,
        };

    let disclaimer_phrases = [
        "does not claim",
        "do not claim",
        "doesn't claim",
        "not claim",
        "without claiming",
        "cannot verify",
        "can't verify",
        "does not verify",
        "do not verify",
        "requires approved evidence for",
        "requires evidence for",
        "requires review for",
        "requires human review for",
        "not approved for",
        "not intended to",
        "not a replacement for",
    ];
    disclaimer_phrases.iter().any(|phrase| {
        sentence_prefix.rfind(phrase).is_some_and(|phrase_start| {
            let after_phrase = &sentence_prefix[phrase_start + phrase.len()..];
            !contains_any(after_phrase, &[" but ", " however ", " although "])
        })
    })
}

fn is_word_char(value: char) -> bool {
    value.is_ascii_alphanumeric()
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use crate::pack_io::read_card_by_id;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_pack(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-{name}-{nonce}"));
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        root
    }

    fn rename_manifest_card_ids(root: &Path, replacements: &[(&str, &str)]) {
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let mut manifest =
            std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        for (from, to) in replacements {
            manifest = manifest.replace(&format!("- id: {from}\n"), &format!("- id: {to}\n"));
        }
        std::fs::write(manifest_path, manifest).expect("manifest should be writable");
    }

    fn add_qualification_gate(root: &Path, min: usize, max: usize) {
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let gate = format!(
            "qualification_gates:\n  require_person_resolution: true\n  signals:\n    min: {min}\n    max: {max}\n    require_fit_signal: true\n    require_why_now_signal: true\n  fail_policy: insufficient_context\n"
        );
        let updated = raw.replacen(
            "required_primitives:\n",
            &format!("{gate}required_primitives:\n"),
            1,
        );
        std::fs::write(manifest_path, updated).expect("manifest should be writable");
    }

    fn rename_card_id(root: &Path, card_path: &str, from: &str, to: &str) {
        let path = root.join(".mdp").join("cards").join(card_path);
        let raw = std::fs::read_to_string(&path).expect("card should be readable");
        std::fs::write(
            path,
            raw.replacen(&format!("id: {from}\n"), &format!("id: {to}\n"), 1),
        )
        .expect("card should be writable");
    }

    fn add_initial_email_word_count_constraint(root: &Path) {
        let path = root.join(".mdp").join("cards").join("output-rules.yaml");
        let raw = std::fs::read_to_string(&path).expect("output rules should be readable");
        std::fs::write(
            path,
            raw.replace(
                "- id: no-fake-personalization",
                "  constraints:\n    word_count:\n      min: 50\n      max: 125\n- id: no-fake-personalization",
            ),
        )
        .expect("output rules should be writable");
    }

    #[test]
    fn route_preserves_skill_load_order_contract() {
        let root = temp_pack("route-contract");

        let result = route(
            &root,
            "GTM Engineering",
            "linkedin outbound copy",
            false,
            false,
        )
        .expect("route should succeed");
        let load_order: Vec<&str> = result["load_order"]
            .as_array()
            .expect("load_order should be an array")
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("load_order entries should be strings")
            })
            .collect();

        assert_eq!(
            &load_order[..3],
            &[
                ".mdp/cards/personas.yaml",
                ".mdp/cards/avoid-rules.yaml",
                ".mdp/cards/output-rules.yaml",
            ]
        );
        assert!(load_order.contains(&".mdp/cards/ctas.yaml"));
        assert!(load_order.len() <= 13);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn linkedin_entry_route_excludes_email_follow_up_and_call_prep_entries() {
        let root = temp_pack("linkedin-entry-route");

        let result = route(&root, "PMM", "linkedin outbound copy", true, false)
            .expect("route should succeed");
        let titles: Vec<&str> = result["entry_route"]["matches"]
            .as_array()
            .expect("entry matches array")
            .iter()
            .filter_map(|entry| entry["title"].as_str())
            .collect();

        assert!(titles.contains(&"LinkedIn initial touch"));
        assert!(!titles.contains(&"LinkedIn follow-up"));
        assert!(!titles.contains(&"Email initial touch"));
        assert!(!titles.contains(&"Email follow-up"));
        assert!(!titles.contains(&"Call prep"));
        assert!(
            result["entry_route"]["matches"]
                .as_array()
                .expect("entry matches array")
                .iter()
                .all(|entry| entry.get("body").is_none())
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn initial_email_entry_route_excludes_follow_up_and_linkedin_entries() {
        let root = temp_pack("email-initial-entry-route");

        let result = route(&root, "PMM", "initial email outbound message", true, false)
            .expect("route should succeed");
        let titles: Vec<&str> = result["entry_route"]["matches"]
            .as_array()
            .expect("entry matches array")
            .iter()
            .filter_map(|entry| entry["title"].as_str())
            .collect();

        assert!(titles.contains(&"Email initial touch"));
        assert!(!titles.contains(&"Email follow-up"));
        assert!(!titles.contains(&"LinkedIn initial touch"));
        assert!(!titles.contains(&"LinkedIn follow-up"));
        assert!(!titles.contains(&"Call prep"));
        let initial_email = result["entry_route"]["matches"]
            .as_array()
            .expect("entry matches array")
            .iter()
            .find(|entry| entry["entry_id"] == "email-initial-touch")
            .expect("initial email entry should route");
        assert_eq!(initial_email["constraints"]["word_count"]["min"], 50);
        assert_eq!(initial_email["constraints"]["subject_words"]["max"], 6);
        assert_eq!(initial_email["constraints"]["max_questions"], 1);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn route_resolves_manifest_persona_alias_before_card_and_entry_routing() {
        let root = temp_pack("route-persona-alias");

        let result = route(
            &root,
            "Growth Engineer",
            "agent brief for enriched row",
            true,
            true,
        )
        .expect("route should succeed");
        let ids: Vec<&str> = result["route"]
            .as_array()
            .expect("route array")
            .iter()
            .filter_map(|entry| entry["id"].as_str())
            .collect();
        let titles: Vec<&str> = result["entry_route"]["matches"]
            .as_array()
            .expect("entry matches array")
            .iter()
            .filter_map(|entry| entry["title"].as_str())
            .collect();

        assert_eq!(result["requested_persona"], "Growth Engineer");
        assert_eq!(result["persona"], "GTM Engineering");
        assert_eq!(
            result["persona_resolution"]["source"],
            "manifest.persona_mappings.title_keywords"
        );
        assert_eq!(result["persona_resolution"]["resolved"], true);
        assert!(ids.contains(&"fit-rules"));
        assert!(ids.contains(&"signals"));
        assert!(titles.contains(&"Agent brief"));
        assert_eq!(
            result["eval_fixture"]["requested_persona"],
            "Growth Engineer"
        );
        assert_eq!(result["eval_fixture"]["persona"], "GTM Engineering");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn linkedin_follow_up_entry_route_excludes_initial_and_email_entries() {
        let root = temp_pack("linkedin-follow-up-entry-route");

        let result = route(&root, "PMM", "linkedin follow up message", true, false)
            .expect("route should succeed");
        let titles: Vec<&str> = result["entry_route"]["matches"]
            .as_array()
            .expect("entry matches array")
            .iter()
            .filter_map(|entry| entry["title"].as_str())
            .collect();

        assert!(titles.contains(&"LinkedIn follow-up"));
        assert!(!titles.contains(&"LinkedIn initial touch"));
        assert!(!titles.contains(&"Email initial touch"));
        assert!(!titles.contains(&"Email follow-up"));
        assert!(!titles.contains(&"Call prep"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unknown_task_does_not_match_ask_substring() {
        let root = temp_pack("route-token-boundary");

        let result =
            route(&root, "Unknown", "task hygiene", false, false).expect("route should succeed");
        let ids: Vec<&str> = result["route"]
            .as_array()
            .expect("route array")
            .iter()
            .filter_map(|entry| entry["id"].as_str())
            .collect();

        assert_eq!(ids, vec!["personas", "avoid-rules", "output-rules"]);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_gate_allows_generated_prospect() {
        let root = temp_pack("fit-contract");
        let prospect_path = root.join("examples").join("clay-row.json");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["contract"], "mdp.fit.v0");
        assert_eq!(result["status"], "fit");
        assert_eq!(result["prospect"]["company_domain"], "example.com");
        assert!(result["matches"].as_array().expect("matches array").len() > 0);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_gate_uses_fit_rule_kind_when_canonical_id_is_absent() {
        let root = temp_pack("fit-kind-fallback");
        rename_manifest_card_ids(&root, &[("fit-rules", "qualification-rules")]);
        let prospect_path = root.join("examples").join("clay-row.json");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "fit");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_canonicalizes_supplied_company_domain_urls() {
        let root = temp_pack("fit-domain-canonical");
        let prospect_path = root.join("examples").join("domain-url.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "company_domain": "https://www.example.com/path?x=1",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents",
  "signals": [{"id": "agent-gtm-workflow", "title": "Building multi-agent GTM workflow", "source": "example row"}]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["prospect"]["company_domain"], "example.com");
        assert_eq!(
            result["context"]["normalization"]["company_domain"]["status"],
            "normalized"
        );
        assert_eq!(result["status"], "fit");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_qualification_gate_requires_public_person_resolution() {
        let root = temp_pack("fit-qualification-person");
        add_qualification_gate(&root, 1, 3);
        let prospect_path = root.join("examples").join("missing-person-resolution.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "persona": "PMM",
  "segment": "agent-assisted GTM",
  "trigger": "Recent launch creates urgency for message context",
  "signals": [
    {"id": "fit-signal", "title": "Strong fit signal for account category", "source": "public account page"},
    {"id": "why-now-signal", "title": "Recent launch trigger creates a why-now opportunity", "source": "public launch post"}
  ]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "insufficient-context");
        assert!(
            result["context"]["missing_requirements"]
                .as_array()
                .expect("missing requirements")
                .iter()
                .any(|issue| issue["scope"] == "qualification_gate"
                    && issue["field"] == "person_resolution")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_qualification_gate_rejects_placeholder_person_resolution() {
        let root = temp_pack("fit-qualification-placeholder-person");
        add_qualification_gate(&root, 1, 3);
        let prospect_path = root
            .join("examples")
            .join("placeholder-person-resolution.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Unknown Contact",
  "title": "Unknown Role",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "linkedin_url": "https://www.linkedin.com/in/unknown-contact",
  "persona": "PMM",
  "segment": "agent-assisted GTM",
  "trigger": "Recent launch creates urgency for message context",
  "signals": [
    {"id": "fit-signal", "title": "Strong fit signal for account category", "source": "public account page"},
    {"id": "why-now-signal", "title": "Recent launch trigger creates a why-now opportunity", "source": "public launch post"}
  ]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "insufficient-context");
        assert!(
            result["context"]["missing_requirements"]
                .as_array()
                .expect("missing requirements")
                .iter()
                .any(|issue| issue["scope"] == "qualification_gate"
                    && issue["field"] == "person_resolution")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_qualification_gate_requires_source_backed_fit_signal() {
        let root = temp_pack("fit-qualification-fit-signal");
        add_qualification_gate(&root, 1, 3);
        let prospect_path = root.join("examples").join("missing-fit-signal.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "linkedin_url": "https://www.linkedin.com/in/taylor-lee",
  "persona": "PMM",
  "segment": "agent-assisted GTM",
  "trigger": "Recent launch creates urgency for message context",
  "signals": [
    {"id": "why-now-signal", "title": "Recent launch trigger creates a why-now opportunity", "source": "public launch post"}
  ]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "insufficient-context");
        assert!(
            result["context"]["missing_requirements"]
                .as_array()
                .expect("missing requirements")
                .iter()
                .any(|issue| issue["field"] == "fit_signal")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_qualification_gate_requires_source_backed_why_now_signal() {
        let root = temp_pack("fit-qualification-why-now");
        add_qualification_gate(&root, 1, 3);
        let prospect_path = root.join("examples").join("missing-why-now-signal.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "linkedin_url": "https://www.linkedin.com/in/taylor-lee",
  "persona": "PMM",
  "segment": "agent-assisted GTM",
  "trigger": "Recent launch creates urgency for message context",
  "signals": [
    {"id": "fit-signal", "title": "Strong fit signal for account category", "source": "public account page"}
  ]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "insufficient-context");
        assert!(
            result["context"]["missing_requirements"]
                .as_array()
                .expect("missing requirements")
                .iter()
                .any(|issue| issue["field"] == "why_now_signal")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_qualification_gate_accepts_valid_source_backed_signals() {
        let root = temp_pack("fit-qualification-valid");
        add_qualification_gate(&root, 1, 3);
        let prospect_path = root.join("examples").join("qualified.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "linkedin_url": "https://www.linkedin.com/in/taylor-lee",
  "persona": "PMM",
  "segment": "agent-assisted GTM",
  "trigger": "Recent launch creates urgency for message context",
  "signals": [
    {"id": "fit-signal", "title": "Strong fit signal for account category", "source": "public account page"},
    {"id": "why-now-signal", "title": "Recent launch trigger creates a why-now opportunity", "source": "public launch post"}
  ]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "fit");
        assert_eq!(result["context"]["qualification_gate"]["enabled"], true);
        assert_eq!(
            result["context"]["qualification_gate"]["signals"]["source_backed_count"],
            2
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_qualification_gate_reports_signal_count_bounds() {
        let root = temp_pack("fit-qualification-counts");
        add_qualification_gate(&root, 2, 2);
        let prospect_path = root.join("examples").join("too-many-signals.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "linkedin_url": "https://www.linkedin.com/in/taylor-lee",
  "persona": "PMM",
  "segment": "agent-assisted GTM",
  "trigger": "Recent launch creates urgency for message context",
  "signals": [
    {"id": "fit-signal", "title": "Strong fit signal for account category", "source": "public account page"},
    {"id": "why-now-signal", "title": "Recent launch trigger creates a why-now opportunity", "source": "public launch post"},
    {"id": "extra-fit-signal", "title": "Extra fit signal", "source": "public article"}
  ]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "insufficient-context");
        assert!(
            result["context"]["invalid_requirements"]
                .as_array()
                .expect("invalid requirements")
                .iter()
                .any(|issue| issue["path"] == "qualification_gates.signals.max")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_without_qualification_gate_remains_compatible() {
        let root = temp_pack("fit-no-qualification-gate");
        let prospect_path = root.join("examples").join("no-gate.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "persona": "PMM",
  "segment": "agent-assisted GTM",
  "trigger": "Recent launch creates urgency for message context",
  "signals": [
    {"id": "generic", "title": "Generic context", "source": "public account page"}
  ]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "fit");
        assert_eq!(result["context"]["qualification_gate"]["enabled"], false);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_reports_missing_required_attributes() {
        let root = temp_pack("fit-required-attribute");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let mut manifest =
            std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        manifest = manifest.replace(
            "required_attributes: []",
            "required_attributes:\n  - fiscal_year",
        );
        std::fs::write(&manifest_path, manifest).expect("manifest should be writable");

        let result =
            fit(&root, &root.join("examples").join("clay-row.json")).expect("fit should succeed");

        assert_eq!(result["status"], "insufficient-context");
        assert!(
            result["context"]["missing"]
                .as_array()
                .expect("missing array")
                .iter()
                .any(|value| value == "attributes.fiscal_year")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_reports_invalid_pack_owned_segment_value() {
        let root = temp_pack("fit-invalid-segment-contract");
        let prospect_path = root.join("examples").join("invalid-segment.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "GTM Engineering Lead",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "persona": "GTM Engineering",
  "segment": "enterprise SaaS",
  "trigger": "standardizing outbound context across agents",
  "signals": [{"id": "agent-gtm-workflow", "title": "Building multi-agent GTM workflow", "source": "example row"}]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "insufficient-context");
        assert!(
            result["context"]["invalid_requirements"]
                .as_array()
                .expect("invalid requirements array")
                .iter()
                .any(|issue| issue["path"] == "segment"
                    && issue["reason"]
                        .as_str()
                        .is_some_and(|reason| reason.contains("agent-assisted GTM")))
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_reports_invalid_mapped_persona_contract_value() {
        let root = temp_pack("fit-invalid-mapped-persona-contract");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace(
                "  value_contracts:\n    segment:",
                "  value_contracts:\n    persona:\n      type: string\n      enum:\n      - GTM Engineering\n      description: Pack-owned persona labels accepted for fit.\n    segment:",
            ),
        )
        .expect("manifest should be writable");
        let prospect_path = root.join("examples").join("mapped-persona-contract.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Product Marketing Lead",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents",
  "signals": [{"id": "agent-gtm-workflow", "title": "Building multi-agent GTM workflow", "source": "example row"}]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["persona_resolution"]["persona"], "PMM");
        assert_eq!(result["status"], "insufficient-context");
        assert!(
            result["context"]["invalid_requirements"]
                .as_array()
                .expect("invalid requirements array")
                .iter()
                .any(|issue| issue["path"] == "persona"
                    && issue["reason"].as_str().is_some_and(|reason| reason
                        .contains("GTM Engineering")
                        && reason.contains("PMM")))
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_accepts_pack_persona_alias_for_persona_contract() {
        let root = temp_pack("fit-persona-alias-contract");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace(
                "  value_contracts:\n    segment:",
                "  value_contracts:\n    persona:\n      type: string\n      enum:\n      - GTM Engineering\n      description: Pack-owned persona labels accepted for fit.\n    segment:",
            ),
        )
        .expect("manifest should be writable");
        let prospect_path = root.join("examples").join("persona-alias-contract.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Revenue Operations Lead",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "persona": "revops",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents",
  "signals": [{"id": "agent-gtm-workflow", "title": "Building multi-agent GTM workflow", "source": "example row"}]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["persona_resolution"]["persona"], "GTM Engineering");
        assert_eq!(result["status"], "fit");
        assert!(
            result["context"]["invalid_requirements"]
                .as_array()
                .expect("invalid requirements array")
                .is_empty()
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_reports_invalid_supplied_company_domain() {
        let root = temp_pack("fit-invalid-domain");
        let prospect_path = root.join("examples").join("invalid-domain.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "company_domain": "ExampleCo Inc",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents",
  "signals": [{"id": "agent-gtm-workflow", "title": "Building multi-agent GTM workflow", "source": "example row"}]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "insufficient-context");
        assert_eq!(
            result["context"]["invalid_requirements"][0]["path"],
            "company_domain"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_reports_invalid_supplied_company_url() {
        let root = temp_pack("fit-invalid-company-url");
        let prospect_path = root.join("examples").join("invalid-company-url.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "company_url": "https://",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents",
  "signals": [{"id": "agent-gtm-workflow", "title": "Building multi-agent GTM workflow", "source": "example row"}]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "insufficient-context");
        assert_eq!(
            result["context"]["normalization"]["company_domain"]["status"],
            "invalid"
        );
        assert_eq!(
            result["context"]["invalid_requirements"][0]["path"],
            "company_url"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_rejects_unknown_top_level_prospect_fields() {
        let root = temp_pack("fit-unknown-prospect-field");
        let prospect_path = root.join("examples").join("unknown-field.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "territory": "enterprise",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents",
  "signals": [{"id": "agent-gtm-workflow", "title": "Building multi-agent GTM workflow", "source": "example row"}]
}"#,
        )
        .expect("prospect should be writable");

        let err = fit(&root, &prospect_path).expect_err("unknown field should fail");
        let message = err.to_string();

        assert!(message.contains("prospect_unknown_field"));
        assert!(message.contains("attributes.territory"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_rejects_unknown_signal_fields() {
        let root = temp_pack("fit-unknown-signal-field");
        let prospect_path = root.join("examples").join("unknown-signal-field.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents",
  "signals": [{"id": "agent-gtm-workflow", "title": "Building multi-agent GTM workflow", "source": "example row", "url": "https://example.com"}]
}"#,
        )
        .expect("prospect should be writable");

        let err = fit(&root, &prospect_path).expect_err("unknown signal field should fail");
        let message = err.to_string();

        assert!(message.contains("prospect_signal_unknown_field"));
        assert!(message.contains("signals[].source"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_gate_uses_manifest_persona_mapping_for_titles() {
        let root = temp_pack("fit-persona-mapping");
        let prospect_path = root.join("examples").join("demand-gen.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Director of Demand Gen",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents",
  "signals": [{"id": "agent-gtm-workflow", "title": "Building multi-agent GTM workflow", "source": "example row"}]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "fit");
        assert_eq!(result["persona_resolution"]["persona"], "PMM");
        assert_eq!(
            result["persona_resolution"]["source"],
            "manifest.persona_mappings.title_keywords"
        );
        assert_eq!(result["context"]["missing"].as_array().unwrap().len(), 0);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_gate_still_requires_persona_when_no_mapping_matches() {
        let root = temp_pack("fit-no-persona-mapping");
        let prospect_path = root.join("examples").join("chief-of-staff.json");
        std::fs::write(
            &prospect_path,
            r#"{
  "name": "Taylor Lee",
  "title": "Chief of Staff",
  "company": "ExampleCo",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents",
  "signals": [{"id": "agent-gtm-workflow", "title": "Building multi-agent GTM workflow", "source": "example row"}]
}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "insufficient-context");
        assert_eq!(result["persona_resolution"]["source"], "fallback");
        assert!(
            result["context"]["missing"]
                .as_array()
                .expect("missing context array")
                .iter()
                .any(|item| item == "persona")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn route_can_emit_eval_fixture_scaffold_from_actual_matches() {
        let root = temp_pack("route-eval-fixture");

        let result = route(&root, "PMM", "linkedin outbound copy", false, true)
            .expect("route should succeed");

        assert_eq!(result["eval_fixture"]["command"], "route");
        assert_eq!(result["eval_fixture"]["persona"], "PMM");
        assert_eq!(result["eval_fixture"]["job"], "linkedin outbound copy");
        assert!(
            result["eval_fixture"]["expect_load_order_contains"]
                .as_array()
                .expect("expected load order should be an array")
                .iter()
                .any(|path| path == ".mdp/cards/ctas.yaml")
        );
        assert!(
            result["eval_fixture"]["notes"][0]
                .as_str()
                .expect("fixture note should be a string")
                .contains("Review before committing")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_gate_rejects_thin_gtm_title_without_context() {
        let root = temp_pack("fit-thin");
        let prospect_path = root.join("examples").join("thin.json");
        std::fs::write(
            &prospect_path,
            r#"{"name":"Taylor Lee","title":"GTM Engineering Lead","company":"ExampleCo"}"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "insufficient-context");
        assert!(
            result["context"]["missing"]
                .as_array()
                .expect("missing context array")
                .iter()
                .any(|item| item == "trigger")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fit_gate_allows_negated_execution_boundaries() {
        let root = temp_pack("fit-negated-execution");
        let prospect_path = root.join("examples").join("negated-execution.json");
        std::fs::write(
            &prospect_path,
            r#"{
                "name": "Jordan Smith",
                "title": "GTM Engineering Lead",
                "company": "ExampleCo",
                "company_domain": "example.com",
                "company_url": "https://example.com",
                "persona": "GTM Engineering",
                "segment": "agent-assisted GTM",
                "source_kind": "synthetic-example",
                "synthetic": true,
                "background": "building repeatable agent-assisted GTM workflows across supplied rows and review steps",
                "trigger": "Needs message context and explicitly says do not auto-send the campaign",
                "signals": [
                    {
                        "id": "review-boundary",
                        "title": "Review workflow, not auto-send",
                        "source": "synthetic example row"
                    }
                ]
            }"#,
        )
        .expect("prospect should be writable");

        let result = fit(&root, &prospect_path).expect("fit should succeed");

        assert_eq!(result["status"], "fit");
        assert!(
            result["disqualifiers"]
                .as_array()
                .expect("disqualifiers array")
                .is_empty()
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_check_flags_execution_category_drift() {
        let root = temp_pack("claim-contract");

        let result = check_claims(
            &root,
            Some("This turns your messaging pack into an AI SDR."),
            None,
            None,
            None,
            None,
        )
        .expect("claim check should succeed");

        assert_eq!(result["contract"], "mdp.claim-check.v0");
        assert_eq!(result["valid"], false);
        assert!(
            result["guardrail_hits"]
                .as_array()
                .expect("guardrail_hits array")
                .iter()
                .any(|hit| hit["term"] == "AI SDR")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_check_allows_negated_execution_boundaries() {
        let root = temp_pack("claim-negated-execution");

        let result = check_claims(
            &root,
            Some("MDP is a local offline CLI. It does not auto-send or send emails."),
            None,
            None,
            None,
            None,
        )
        .expect("claim check should succeed");

        assert_eq!(result["valid"], true);
        assert!(
            result["unsupported_claims"]
                .as_array()
                .expect("unsupported claims array")
                .is_empty()
        );
        assert!(
            result["guardrail_hits"]
                .as_array()
                .expect("guardrail hits array")
                .is_empty()
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_check_flags_output_rule_terms() {
        let root = temp_pack("output-rule-contract");

        let result = check_claims(
            &root,
            Some("MDP is local — it keeps message context in a pack."),
            None,
            None,
            None,
            None,
        )
        .expect("claim check should succeed");

        assert_eq!(result["contract"], "mdp.claim-check.v0");
        assert_eq!(result["valid"], false);
        assert!(
            result["guardrail_hits"]
                .as_array()
                .expect("guardrail_hits array")
                .iter()
                .any(|hit| hit["card_id"] == "output-rules" && hit["entry_id"] == "no-em-dashes")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_check_uses_card_kinds_when_canonical_ids_are_absent() {
        let root = temp_pack("claim-kind-fallback");
        rename_manifest_card_ids(
            &root,
            &[
                ("claims", "proof-library"),
                ("avoid-rules", "category-boundaries"),
                ("output-rules", "prose-contract"),
            ],
        );
        rename_card_id(&root, "claims.yaml", "claims", "proof-library");
        rename_card_id(
            &root,
            "avoid-rules.yaml",
            "avoid-rules",
            "category-boundaries",
        );
        rename_card_id(&root, "output-rules.yaml", "output-rules", "prose-contract");

        let result = check_claims(
            &root,
            Some("MDP is local — it becomes an AI SDR and guarantees meetings."),
            None,
            None,
            None,
            None,
        )
        .expect("claim check should succeed");

        let guardrail_entry_ids: Vec<&str> = result["guardrail_hits"]
            .as_array()
            .expect("guardrail hits array")
            .iter()
            .filter_map(|hit| hit["entry_id"].as_str())
            .collect();

        assert_eq!(result["valid"], false);
        assert!(guardrail_entry_ids.contains(&"not-execution"));
        assert!(guardrail_entry_ids.contains(&"no-em-dashes"));
        assert!(
            result["unsupported_claims"]
                .as_array()
                .expect("unsupported claims array")
                .iter()
                .any(|claim| claim["trigger"] == "guarantee")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn route_scoped_claim_check_does_not_duplicate_custom_output_rule_constraints() {
        let root = temp_pack("claim-kind-fallback-route-scoped");
        add_initial_email_word_count_constraint(&root);
        rename_manifest_card_ids(&root, &[("output-rules", "prose-contract")]);
        rename_card_id(&root, "output-rules.yaml", "output-rules", "prose-contract");

        let result = check_claims(
            &root,
            Some("Too short."),
            None,
            Some("Proposal note"),
            Some("PMM"),
            Some("initial email outbound copy"),
        )
        .expect("claim check should succeed");

        let output_word_count_hits = result["guardrail_hits"]
            .as_array()
            .expect("guardrail hits array")
            .iter()
            .filter(|hit| {
                hit["card_id"] == "prose-contract"
                    && hit["entry_id"] == "initial-email-shape"
                    && hit["rule"] == "constraints.word_count"
            })
            .count();

        assert_eq!(output_word_count_hits, 1);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_check_flags_exact_paragraph_count_rules() {
        let root = temp_pack("output-rule-paragraph-count");
        let output_rules_path = root.join(".mdp").join("cards").join("output-rules.yaml");
        let raw = std::fs::read_to_string(&output_rules_path).expect("output rules readable");
        std::fs::write(
            &output_rules_path,
            raw.replace(
                "  avoid: []\n- id: no-meta-commentary",
                "  avoid: []\n  exact_paragraphs: 2\n- id: no-meta-commentary",
            ),
        )
        .expect("output rules writable");

        let one_paragraph =
            check_claims(&root, Some("First paragraph only."), None, None, None, None)
                .expect("check should run");
        assert_eq!(one_paragraph["valid"], false);
        assert!(
            one_paragraph["guardrail_hits"]
                .as_array()
                .expect("guardrail_hits array")
                .iter()
                .any(|hit| hit["rule"] == "exact_paragraphs"
                    && hit["expected"] == 2
                    && hit["actual"] == 1)
        );

        let two_paragraphs = check_claims(
            &root,
            Some("First paragraph.\n\nSecond paragraph."),
            None,
            None,
            None,
            None,
        )
        .expect("check should run");
        assert_eq!(two_paragraphs["valid"], true);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn starter_parses_structured_output_constraints() {
        let root = temp_pack("structured-constraints-parse");
        let channel_policies =
            read_card_by_id(&root, "channel-policies").expect("channel policies should read");
        let initial_email = channel_policies
            .entries
            .iter()
            .find(|entry| entry.id == "email-initial-touch")
            .expect("initial email entry should exist");

        assert_eq!(
            initial_email
                .constraints
                .word_count
                .as_ref()
                .expect("word count constraint")
                .min,
            Some(50)
        );
        assert!(initial_email.constraints.forbid_links);
        assert!(
            initial_email
                .constraints
                .subject_avoid
                .iter()
                .any(|term| term == "Re:")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_check_flags_route_scoped_structured_constraint_violations() {
        let root = temp_pack("structured-constraints-check");
        let result = check_claims(
            &root,
            Some("Hi Alex, can we compare notes? See https://example.com? Thanks?"),
            None,
            Some("Re: urgent"),
            Some("PMM"),
            Some("initial email outbound message"),
        )
        .expect("claim check should run");
        let rules: Vec<&str> = result["guardrail_hits"]
            .as_array()
            .expect("guardrail hits array")
            .iter()
            .filter_map(|hit| hit["rule"].as_str())
            .collect();

        assert_eq!(result["valid"], false);
        assert!(rules.contains(&"constraints.word_count"));
        assert!(rules.contains(&"constraints.subject_words"));
        assert!(rules.contains(&"constraints.subject_avoid"));
        assert!(rules.contains(&"constraints.max_questions"));
        assert!(rules.contains(&"constraints.forbid_links"));
        assert!(
            result["unchecked_constraints"]
                .as_array()
                .expect("unchecked constraints array")
                .iter()
                .any(|hit| hit["rule"] == "constraints.forbid_tracking")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_check_reports_target_word_count_as_warning_not_failure() {
        let root = temp_pack("structured-constraints-target");
        let draft = [
            "Alex, saw your team is standardizing outbound context across notes and research.",
            "That usually creates small mismatches between what reps know, what campaigns say, and what agents load before drafting.",
            "MDP keeps the message decisions in a local pack so each workflow can use the same approved context.",
            "Worth comparing notes on who owns that context today?",
        ]
        .join(" ");

        let result = check_claims(
            &root,
            Some(&draft),
            None,
            Some("Context for outbound agents"),
            Some("PMM"),
            Some("initial email outbound message"),
        )
        .expect("claim check should run");

        assert_eq!(result["valid"], true);
        assert!(
            result["constraint_warnings"]
                .as_array()
                .expect("constraint warnings array")
                .iter()
                .any(|hit| hit["rule"] == "constraints.word_count.target")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_check_flags_obvious_unsupported_claims() {
        let root = temp_pack("claim-unsupported");

        for text in [
            "MDP guarantees meetings with enterprise buyers.",
            "MDP improves reply rates by 30%.",
            "MDP integrates with Salesforce and HubSpot.",
            "MDP updates CRM fields after every send.",
            "MDP can auto-send the campaign after the brief is approved.",
            "MDP is SOC 2 compliant and trusted by Acme.",
        ] {
            let result = check_claims(&root, Some(text), None, None, None, None)
                .expect("claim check should succeed");
            assert_eq!(result["valid"], false, "text should fail: {text}");
            assert!(
                result["unsupported_claims"]
                    .as_array()
                    .expect("unsupported_claims array")
                    .len()
                    > 0,
                "text should produce unsupported claim: {text}"
            );
        }

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_check_flags_adversarial_unsupported_claim_variants() {
        let root = temp_pack("claim-adversarial-variants");

        let result = check_claims(
            &root,
            Some("MDP is security-approved, connects to your CRM, books meetings, doubles reply rates, customers rely on it in production, I loved your recent LinkedIn post, bypasses procurement, AI can own the response, replaces proposal management software, and is best-in-class."),
            None,
            None,
            None,
            None,
        )
        .expect("claim check should succeed");

        let categories: Vec<&str> = result["unsupported_claims"]
            .as_array()
            .expect("unsupported claims array")
            .iter()
            .filter_map(|claim| claim["category"].as_str())
            .collect();

        assert_eq!(result["valid"], false);
        for expected in [
            "compliance-security",
            "integration",
            "quantified-outcome",
            "customer-name",
            "execution-crm-sending",
            "legal-procurement-bypass",
            "ai-authoritative",
            "rfp-platform-replacement",
            "fake-personalization",
            "weak-trust",
        ] {
            assert!(
                categories.contains(&expected),
                "missing unsupported category {expected}"
            );
        }

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_check_allows_safe_negated_execution_and_boundary_phrases() {
        let root = temp_pack("claim-safe-negated-boundaries");

        let result = check_claims(
            &root,
            Some("MDP is local-first AI-assisted decision context before drafting with approved claims, evidence, and review rules. It does not send emails, does not connect to Salesforce, does not update CRM records, does not bypass legal, does not replace proposal management software, and does not claim compliance approval."),
            None,
            None,
            None,
            None,
        )
        .expect("claim check should succeed");

        assert_eq!(result["valid"], true);
        assert_eq!(
            result["guardrail_hits"]
                .as_array()
                .expect("guardrail hits array")
                .len(),
            0
        );
        assert_eq!(
            result["unsupported_claims"]
                .as_array()
                .expect("unsupported claims array")
                .len(),
            0
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn claim_check_allows_coordinated_safe_negated_boundaries() {
        let root = temp_pack("claim-safe-coordinated-boundaries");

        let result = check_claims(
            &root,
            Some("MDP is local-first decision context. It does not update CRM records, send emails, or bypass legal review."),
            None,
            None,
            None,
            None,
        )
        .expect("claim check should succeed");

        assert_eq!(result["valid"], true);
        assert_eq!(
            result["guardrail_hits"]
                .as_array()
                .expect("guardrail hits array")
                .len(),
            0
        );
        assert_eq!(
            result["unsupported_claims"]
                .as_array()
                .expect("unsupported claims array")
                .len(),
            0
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
