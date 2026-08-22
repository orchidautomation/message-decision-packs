use crate::artifact_hash::{canonical_json_bytes, canonical_json_sha256, sha256_hex};
use crate::commands::health::{profile_activation_decision, validate_pack};
use crate::commands::schemas::routed_context_schema;
use crate::constants::{DEFAULT_DIR, ROUTED_CONTEXT_CONTRACT};
use crate::models::{CardKind, Entry, JobContextBudget, Manifest};
use crate::pack_io::{read_card, resolve_pack_path};
use crate::product_foundation::{
    ProductFoundationResolution, apply_validation_errors_for_job, resolution_json,
    resolve_product_foundation_for_pack, validation_errors_block_job,
};
use crate::runtime_context::current_runtime_context;
use crate::scope::{ContextScope, ScopeResolution, match_entry_scope, resolve_runtime_scope};
use crate::utils::declared_persona_labels;
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

struct EntryRouteDetails {
    matches: Vec<Value>,
    context_entries: Vec<Value>,
    gaps: Vec<Value>,
    full_card_required: Vec<Value>,
    excluded: Vec<Value>,
    route_card_cap: Value,
    portfolio_sensitive: bool,
    compatible_scoped_entry_count: usize,
    scoped_decision_candidate_count: usize,
    compatible_scoped_decision_count: usize,
    allocation: Value,
}

impl EntryRouteDetails {
    fn scope_ready(&self, scope: &ScopeResolution) -> bool {
        let compatible_requirement_met = if self.scoped_decision_candidate_count > 0 {
            self.compatible_scoped_decision_count > 0
        } else {
            self.compatible_scoped_entry_count > 0
        };
        !self.portfolio_sensitive
            || (!scope.selected.is_empty() && scope.is_valid() && compatible_requirement_met)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MessageLifecycle {
    Initial,
    FollowUp,
}

pub(crate) const ROUTE_CARD_CAP_DIAGNOSTIC: &str = "route_card_cap_excluded_applicable";
const ROUTE_CARD_CAP_REASON: &str = "max_cards_per_route_reached";

struct CardSelection {
    selected: Vec<Value>,
    route_card_cap: Value,
}

pub(crate) fn select_cards(
    manifest: &Manifest,
    persona: Option<&str>,
    job: Option<&str>,
) -> Vec<Value> {
    select_cards_with_diagnostics(manifest, persona, job).selected
}

fn select_cards_with_diagnostics(
    manifest: &Manifest,
    persona: Option<&str>,
    job: Option<&str>,
) -> CardSelection {
    let job_tokens = tokens(job.unwrap_or(""));
    let is_message_job = is_message_job(&job_tokens);
    let mut selected = Vec::new();
    let mut candidates = Vec::new();
    let mut excluded_cards = Vec::new();

    for card in &manifest.cards {
        if is_base_guardrail(&card.kind) {
            selected.push(json!({"id": card.id, "kind": card.kind, "path": format!("{DEFAULT_DIR}/{}", card.path), "reason": "base guardrail", "description": card.description}));
        }
    }

    for (index, card) in manifest.cards.iter().enumerate() {
        if is_base_guardrail(&card.kind) {
            continue;
        }
        let persona_match = persona
            .map(|requested| selector_matches_persona(&card.personas, requested))
            .unwrap_or(false);
        let job_match = !job_tokens.is_empty()
            && (token_overlap(&job_tokens, &tokens(&card.description))
                || card
                    .tags
                    .iter()
                    .any(|tag| token_overlap(&job_tokens, &tokens(tag))));
        if persona_match || job_match {
            let reason = match (persona_match, job_match) {
                (true, true) => "persona and job/tag match",
                (true, false) => "persona match",
                (false, true) => "job/tag match",
                (false, false) => "matched",
            };
            candidates.push((
                card_priority(&card.kind, is_message_job),
                index,
                json!({"id": card.id, "kind": card.kind, "path": format!("{DEFAULT_DIR}/{}", card.path), "reason": reason, "description": card.description}),
            ));
        }
    }

    candidates.sort_by_key(|(priority, index, _)| (*priority, *index));
    for (_, _, card) in candidates {
        if selected.len() >= manifest.policy.max_cards_per_route {
            excluded_cards.push(json!({
                "id": card["id"],
                "kind": card["kind"],
                "reason": ROUTE_CARD_CAP_REASON
            }));
            continue;
        }
        selected.push(card);
    }

    let diagnostics = if excluded_cards.is_empty() {
        Vec::new()
    } else {
        vec![json!(ROUTE_CARD_CAP_DIAGNOSTIC)]
    };
    let route_card_cap = json!({
        "status": if excluded_cards.is_empty() { "ready" } else { "blocked" },
        "max_cards_per_route": manifest.policy.max_cards_per_route,
        "selected_cards": selected.iter().map(card_identity).collect::<Vec<_>>(),
        "excluded_cards": excluded_cards,
        "diagnostics": diagnostics
    });

    CardSelection {
        selected,
        route_card_cap,
    }
}

fn card_identity(card: &Value) -> Value {
    json!({
        "id": card["id"],
        "kind": card["kind"]
    })
}

#[cfg(test)]
pub(crate) fn narrow_starter_route_candidates_for_tests(root: &Path) {
    let manifest_path = root.join(DEFAULT_DIR).join("manifest.yaml");
    let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
    let mut manifest: serde_yaml::Value =
        serde_yaml::from_str(&raw).expect("manifest should parse");
    for card in manifest["cards"].as_sequence_mut().expect("cards") {
        if matches!(card["id"].as_str(), Some("gaps") | Some("objections")) {
            card["description"] = serde_yaml::Value::String(
                "Unrelated synthetic route-cap fixture card.".to_string(),
            );
            card["tags"] = serde_yaml::Value::Sequence(Vec::new());
        }
        if matches!(
            card["id"].as_str(),
            Some("gaps")
                | Some("objections")
                | Some("portfolio-examples")
                | Some("channel-policies")
        ) {
            card["personas"] =
                serde_yaml::from_str("- Route Cap Excluded\n").expect("nonmatching persona");
        }
    }
    manifest["target_personas"]
        .as_sequence_mut()
        .expect("target personas")
        .push(serde_yaml::Value::String("Route Cap Excluded".to_string()));
    // Keep the synthetic route-cap fixtures on the original exact-cap contract
    // while the shipped starter can leave one additional slot for scoped cards.
    manifest["policy"]["max_cards_per_route"] = serde_yaml::Value::Number(13.into());
    std::fs::write(
        manifest_path,
        serde_yaml::to_string(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");
}

#[cfg(test)]
pub(crate) fn add_supplemental_persona_card_for_tests(root: &Path) {
    use crate::models::{Card, CardRef};

    let manifest_path = root.join(DEFAULT_DIR).join("manifest.yaml");
    let mut manifest = crate::pack_io::read_manifest(root).expect("manifest should load");
    manifest.cards.push(CardRef {
        id: "supplemental-personas".to_string(),
        path: "cards/supplemental-personas.yaml".to_string(),
        kind: CardKind::Personas,
        description: "Synthetic supplemental persona guardrail with neutral applicability."
            .to_string(),
        personas: Vec::new(),
        tags: Vec::new(),
    });
    std::fs::write(
        manifest_path,
        serde_yaml::to_string(&manifest).expect("manifest should serialize"),
    )
    .expect("manifest should be writable");

    let card = Card {
        id: "supplemental-personas".to_string(),
        kind: CardKind::Personas,
        title: "Supplemental Personas".to_string(),
        description: "Synthetic supplemental persona guardrail.".to_string(),
        personas: Vec::new(),
        tags: Vec::new(),
        entries: vec![Entry {
            id: "neutral-persona".to_string(),
            title: "Neutral synthetic persona".to_string(),
            body: "Synthetic neutral applicability entry for route-cap regression coverage."
                .to_string(),
            applies_to: Vec::new(),
            scope: BTreeMap::new(),
            evidence: vec!["mdp-1-education-thesis".to_string()],
            avoid: Vec::new(),
            exact_paragraphs: None,
            constraints: Default::default(),
            metadata: BTreeMap::new(),
        }],
    };
    std::fs::write(
        root.join(DEFAULT_DIR)
            .join("cards/supplemental-personas.yaml"),
        serde_yaml::to_string(&card).expect("card should serialize"),
    )
    .expect("card should be writable");
}

fn is_message_job(job_tokens: &[String]) -> bool {
    [
        "copy", "outbound", "linkedin", "email", "message", "brief", "cta", "ask", "reply",
    ]
    .iter()
    .any(|token| job_tokens.iter().any(|candidate| candidate == token))
}

fn is_base_guardrail(kind: &CardKind) -> bool {
    matches!(
        kind,
        CardKind::Personas | CardKind::AvoidRules | CardKind::OutputRules
    )
}

fn card_priority(kind: &CardKind, is_message_job: bool) -> usize {
    if is_message_job {
        match kind {
            CardKind::Personas | CardKind::AvoidRules | CardKind::OutputRules => 0,
            CardKind::FitRules => 5,
            CardKind::Positioning => 10,
            CardKind::Pains => 20,
            CardKind::Signals => 25,
            CardKind::Hooks => 30,
            CardKind::Claims => 35,
            CardKind::CopyPatterns => 40,
            CardKind::Ctas => 45,
            CardKind::ChannelPolicies => 50,
            CardKind::Objections => 60,
            CardKind::Motions => 70,
            CardKind::Gaps => 80,
        }
    } else {
        match kind {
            CardKind::Personas | CardKind::AvoidRules | CardKind::OutputRules => 0,
            CardKind::FitRules => 5,
            CardKind::Positioning => 10,
            CardKind::Motions => 20,
            CardKind::Signals => 30,
            CardKind::Pains => 40,
            CardKind::Claims => 50,
            CardKind::ChannelPolicies => 60,
            CardKind::Objections => 70,
            CardKind::Hooks => 80,
            CardKind::CopyPatterns => 90,
            CardKind::Ctas => 100,
            CardKind::Gaps => 110,
        }
    }
}

pub(crate) fn entry_route(
    root: &Path,
    manifest: &Manifest,
    persona: &str,
    job: &str,
) -> Result<Value> {
    entry_route_scoped(root, manifest, persona, job, &ScopeResolution::default())
}

/// Deterministic generation-time preflight that evaluates every declared
/// canonical job that carries a `context_budget` against every relevant
/// manifest persona using the default (unfiltered) portfolio scope. It fails
/// when any route's selected entry count or canonical byte size exceeds the
/// declared budget, and reports selected/excluded counts, reason-code
/// distributions, and the largest contributing cards without leaking entry
/// bodies. Universal guardrails and product-foundation requirements remain
/// selected; the preflight never truncates or ranks them away to satisfy a
/// budget. Legacy packs without a declared context budget remain
/// budget-unassessed, but cap-caused authority loss is still a blocking
/// preflight diagnostic.
pub(crate) fn route_budget_preflight(root: &Path, manifest: &Manifest) -> Result<Value> {
    let scope = ScopeResolution::default();
    let declared_personas = declared_persona_labels(manifest);
    let mut routes = Vec::new();
    let mut overflow_count = 0usize;
    let mut route_card_cap_exclusion_count = 0usize;
    let mut near_budget_count = 0usize;
    let mut unassessed_generation_count = 0usize;

    for job in &manifest.jobs {
        let Some(budget) = job.context_budget.as_ref() else {
            if job.model_task.is_some() {
                unassessed_generation_count += 1;
            }
            if declared_personas.is_empty() {
                routes.push(json!({
                    "persona": Value::Null,
                    "job": job.id,
                    "status": "unassessed",
                    "reason": "context_budget_not_declared",
                    "budget": Value::Null,
                    "selected_count": Value::Null,
                    "excluded_count": Value::Null,
                    "diagnostics": ["context_budget_not_declared"],
                    "reason_distribution": {},
                    "excluded_reason_distribution": {},
                    "largest_contributing_cards": [],
                    "route_card_cap": Value::Null
                }));
            } else {
                for persona in &declared_personas {
                    let route_card_cap =
                        select_cards_with_diagnostics(manifest, Some(persona), Some(&job.id))
                            .route_card_cap;
                    let route_card_cap_blocked = route_card_cap["status"] == "blocked";
                    if route_card_cap_blocked {
                        route_card_cap_exclusion_count += 1;
                    }
                    let diagnostics = if route_card_cap_blocked {
                        json!(["context_budget_not_declared", ROUTE_CARD_CAP_DIAGNOSTIC])
                    } else {
                        json!(["context_budget_not_declared"])
                    };
                    routes.push(json!({
                        "persona": persona,
                        "job": job.id,
                        "status": if route_card_cap_blocked { "blocked" } else { "unassessed" },
                        "reason": "context_budget_not_declared",
                        "budget": Value::Null,
                        "selected_count": Value::Null,
                        "excluded_count": Value::Null,
                        "diagnostics": diagnostics,
                        "reason_distribution": {},
                        "excluded_reason_distribution": {},
                        "largest_contributing_cards": [],
                        "route_card_cap": route_card_cap
                    }));
                }
            }
            continue;
        };
        for persona in &declared_personas {
            let route = entry_route_scoped(root, manifest, persona, &job.id, &scope)?;
            let route_card_cap = route["route_card_cap"].clone();
            let route_card_cap_blocked = route_card_cap["status"] == "blocked";
            if route_card_cap_blocked {
                route_card_cap_exclusion_count += 1;
            }
            let minimality = &route["minimality"];
            let budget_value = &minimality["budget"];
            let max_entries = budget.max_entries;
            let max_bytes = budget.max_bytes;
            let actual_entries = budget_value["actual_entries"].as_u64().unwrap_or(0) as usize;
            let actual_bytes = budget_value["actual_bytes"].as_u64().unwrap_or(0) as usize;
            let selected_count = minimality["selected_count"].as_u64().unwrap_or(0) as usize;
            let excluded_count = minimality["excluded_count"].as_u64().unwrap_or(0) as usize;
            // The preflight is a budget gate. Runtime blocks such as
            // full-card fallback remain owned by `route --entries` and
            // `brief --context` minimality so legacy fail-closed behavior is
            // preserved; only budget overflow and near-budget signals surface
            // here so generation handoff can narrow applicability.
            let entry_overflow = actual_entries > max_entries;
            let byte_overflow = actual_bytes > max_bytes;
            let near_entries = max_entries > 0
                && actual_entries > 0
                && !entry_overflow
                && actual_entries * 100 >= max_entries * 90;
            let near_bytes = max_bytes > 0
                && actual_bytes > 0
                && !byte_overflow
                && actual_bytes * 100 >= max_bytes * 90;
            if entry_overflow || byte_overflow {
                overflow_count += 1;
            }
            let mut diagnostics: Vec<Value> = Vec::new();
            if entry_overflow {
                diagnostics.push(json!("context_entry_budget_exceeded"));
            }
            if byte_overflow {
                diagnostics.push(json!("context_byte_budget_exceeded"));
            }
            if near_entries || near_bytes {
                near_budget_count += 1;
                diagnostics.push(json!("near_context_budget"));
            }
            if route_card_cap_blocked {
                diagnostics.push(json!(ROUTE_CARD_CAP_DIAGNOSTIC));
            }
            let status = if entry_overflow || byte_overflow || route_card_cap_blocked {
                "blocked"
            } else {
                "ready"
            };
            let reason_distribution = route_reason_distribution(&route);
            let excluded_reason_distribution = route_excluded_reason_distribution(minimality);
            let largest_contributing_cards = minimality["largest_contributing_cards"].clone();
            let mut route_receipt = json!({
                "persona": persona,
                "job": job.id,
                "status": status,
                "budget": {
                    "max_entries": max_entries,
                    "max_bytes": max_bytes,
                    "actual_entries": actual_entries,
                    "actual_bytes": actual_bytes
                },
                "selected_count": selected_count,
                "excluded_count": excluded_count,
                "diagnostics": diagnostics,
                "reason_distribution": reason_distribution,
                "excluded_reason_distribution": excluded_reason_distribution,
                "largest_contributing_cards": largest_contributing_cards,
                "context_sha256": minimality["context_sha256"].clone(),
                "route_card_cap": route_card_cap
            });
            if !minimality["allocation"].is_null() {
                route_receipt["allocation"] = minimality["allocation"].clone();
            }
            routes.push(route_receipt);
        }
    }

    let valid = overflow_count == 0 && route_card_cap_exclusion_count == 0;
    Ok(json!({
        "contract": "mdp.route-budget.v0",
        "valid": valid,
        "pack_id": manifest.id,
        "scope": "default",
        "route_count": routes.len(),
        "overflow_count": overflow_count,
        "route_card_cap_exclusion_count": route_card_cap_exclusion_count,
        "near_budget_count": near_budget_count,
        "unassessed_generation_count": unassessed_generation_count,
        "routes": routes
    }))
}

fn route_reason_distribution(route: &Value) -> Value {
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    for entry in route["matches"].as_array().into_iter().flatten() {
        if let Some(reason) = entry["reason"].as_str() {
            *counts.entry(reason).or_default() += 1;
        }
    }
    let mut distribution = serde_json::Map::new();
    for (reason, count) in counts {
        distribution.insert(reason.to_string(), json!(count));
    }
    Value::Object(distribution)
}

fn route_excluded_reason_distribution(minimality: &Value) -> Value {
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    for entry in minimality["excluded"].as_array().into_iter().flatten() {
        if let Some(reason_code) = entry["reason_code"].as_str() {
            *counts.entry(reason_code).or_default() += 1;
        }
    }
    let mut distribution = serde_json::Map::new();
    for (reason_code, count) in counts {
        distribution.insert(reason_code.to_string(), json!(count));
    }
    Value::Object(distribution)
}

pub(crate) fn entry_route_scoped(
    root: &Path,
    manifest: &Manifest,
    persona: &str,
    job: &str,
    scope: &ScopeResolution,
) -> Result<Value> {
    let validation = validate_pack(root)?;
    let mut product_foundation = resolve_product_foundation_for_pack(root, manifest, job)?;
    let validation_issues = validation["issues"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    apply_validation_errors_for_job(&mut product_foundation, manifest, validation_issues);
    let validation_blocked =
        validation_errors_block_job(manifest, &product_foundation, validation_issues);
    let profile_activation = profile_activation_decision(
        &validation,
        manifest.profile_eval.blocks_activation(),
        Some(job),
    );
    let profile_activation_blocked = profile_activation["status"] == "blocked";
    let mut details = route_entry_details(root, manifest, persona, job, true, scope)?;
    let product_foundation_load_order = foundation_load_order(&product_foundation);
    let context_budget = manifest
        .jobs
        .iter()
        .find(|candidate| candidate.id == job)
        .and_then(|candidate| candidate.context_budget.as_ref());
    if optional_quota_enabled(context_budget) {
        apply_selection_authority(&mut details.context_entries, &product_foundation_load_order);
        allocate_context_entries(&mut details, context_budget);
    }
    let (policy, model_visible_projection) = routed_context_projection(
        job,
        persona,
        scope,
        &product_foundation,
        &product_foundation_load_order,
        &details,
    );
    let minimality = context_minimality(
        manifest,
        job,
        &model_visible_projection,
        &details.full_card_required,
        &details.excluded,
        &details.route_card_cap,
        &details.allocation,
    )?;
    let blocked = validation_blocked
        || profile_activation_blocked
        || !details.scope_ready(scope)
        || product_foundation.blocks_activation()
        || minimality["status"] == "blocked";

    Ok(json!({
        "contract": "mdp.entry-route.v0",
        "status": if blocked { "blocked" } else { "ready" },
        "persona": persona,
        "job": job,
        "scope": scope,
        "portfolio_sensitive": details.portfolio_sensitive,
        "product_foundation": resolution_json(&product_foundation),
        "product_foundation_load_order": product_foundation_load_order,
        "profile_activation": profile_activation,
        "matches": details.matches,
        "gaps": details.gaps,
        "route_card_cap": details.route_card_cap,
        "minimality": minimality,
        "policy": policy
    }))
}

pub(crate) fn entry_context_scoped(
    root: &Path,
    manifest: &Manifest,
    persona: &str,
    job: &str,
    draft_ready: bool,
    scope: &ScopeResolution,
) -> Result<Value> {
    let runtime_context = current_runtime_context()?;
    entry_context_with_runtime_scoped(
        root,
        manifest,
        persona,
        job,
        draft_ready,
        &runtime_context,
        scope,
    )
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum RoutedContextValidationKind {
    Schema,
    Contract,
    Job,
    Scope,
    Canonical,
    NotCompiled,
    ReadinessBlocked,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct RoutedContextValidationError {
    kind: RoutedContextValidationKind,
}

impl RoutedContextValidationError {
    fn new(kind: RoutedContextValidationKind) -> Self {
        Self { kind }
    }

    pub(crate) fn kind(self) -> RoutedContextValidationKind {
        self.kind
    }
}

impl fmt::Display for RoutedContextValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            RoutedContextValidationKind::Schema
            | RoutedContextValidationKind::Contract
            | RoutedContextValidationKind::Job
            | RoutedContextValidationKind::Scope
            | RoutedContextValidationKind::Canonical
            | RoutedContextValidationKind::NotCompiled => "routed-context-invalid",
            RoutedContextValidationKind::ReadinessBlocked => "draft-readiness-blocked",
        })
    }
}

impl std::error::Error for RoutedContextValidationError {}

#[derive(Debug)]
pub(crate) struct RoutedContextValidation {
    pub(crate) sha256: String,
}

pub(crate) fn validate_routed_context_bytes_for_job(
    root: &Path,
    manifest: &Manifest,
    bytes: &[u8],
    job: &str,
) -> std::result::Result<RoutedContextValidation, RoutedContextValidationError> {
    let value = serde_json::from_slice::<Value>(bytes)
        .map_err(|_| RoutedContextValidationError::new(RoutedContextValidationKind::Schema))?;
    validate_routed_context_value_for_job(root, manifest, &value, &sha256_hex(bytes), job)
}

pub(crate) fn validate_routed_context_value_for_job(
    root: &Path,
    manifest: &Manifest,
    value: &Value,
    raw_sha256: &str,
    job: &str,
) -> std::result::Result<RoutedContextValidation, RoutedContextValidationError> {
    if value
        .get("contract")
        .and_then(Value::as_str)
        .is_some_and(|contract| contract != ROUTED_CONTEXT_CONTRACT)
    {
        return Err(RoutedContextValidationError::new(
            RoutedContextValidationKind::Contract,
        ));
    }
    jsonschema::draft202012::validate(&routed_context_schema(), value)
        .map_err(|_| RoutedContextValidationError::new(RoutedContextValidationKind::Schema))?;
    if value["contract"] != ROUTED_CONTEXT_CONTRACT {
        return Err(RoutedContextValidationError::new(
            RoutedContextValidationKind::Contract,
        ));
    }
    if value["job"].as_str() != Some(job) {
        return Err(RoutedContextValidationError::new(
            RoutedContextValidationKind::Job,
        ));
    }
    let requested_scope =
        serde_json::from_value::<ContextScope>(value["scope"]["requested"].clone())
            .map_err(|_| RoutedContextValidationError::new(RoutedContextValidationKind::Scope))?;
    let scope = resolve_runtime_scope(manifest, requested_scope);
    if serde_json::to_value(&scope).ok().as_ref() != Some(&value["scope"]) {
        return Err(RoutedContextValidationError::new(
            RoutedContextValidationKind::Scope,
        ));
    }
    let canonical_sha256 = canonical_json_sha256(value)
        .map_err(|_| RoutedContextValidationError::new(RoutedContextValidationKind::Canonical))?;
    if canonical_sha256 != raw_sha256 {
        return Err(RoutedContextValidationError::new(
            RoutedContextValidationKind::Canonical,
        ));
    }
    let persona = value["persona"]
        .as_str()
        .ok_or_else(|| RoutedContextValidationError::new(RoutedContextValidationKind::Schema))?;
    let compiled = entry_context_scoped(root, manifest, persona, job, true, &scope)
        .map_err(|_| RoutedContextValidationError::new(RoutedContextValidationKind::NotCompiled))?;
    if compiled["status"] != "ready" || compiled["model_context"].is_null() {
        return Err(RoutedContextValidationError::new(
            RoutedContextValidationKind::ReadinessBlocked,
        ));
    }
    if compiled["model_context"] != *value {
        return Err(RoutedContextValidationError::new(
            RoutedContextValidationKind::NotCompiled,
        ));
    }
    Ok(RoutedContextValidation {
        sha256: canonical_sha256,
    })
}

pub(crate) fn entry_context_with_runtime_scoped(
    root: &Path,
    manifest: &Manifest,
    persona: &str,
    job: &str,
    draft_ready: bool,
    runtime_context: &Value,
    scope: &ScopeResolution,
) -> Result<Value> {
    let load_order: Vec<Value> = select_cards(manifest, Some(persona), Some(job))
        .iter()
        .filter_map(|value| value["path"].as_str().map(|path| json!(path)))
        .collect();
    let validation = validate_pack(root)?;
    let mut product_foundation = resolve_product_foundation_for_pack(root, manifest, job)?;
    let validation_issues = validation["issues"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    apply_validation_errors_for_job(&mut product_foundation, manifest, validation_issues);
    let validation_blocked =
        validation_errors_block_job(manifest, &product_foundation, validation_issues);
    let profile_activation = profile_activation_decision(
        &validation,
        manifest.profile_eval.blocks_activation(),
        Some(job),
    );
    let profile_activation_blocked = profile_activation["status"] == "blocked";
    let mut details = route_entry_details(root, manifest, persona, job, true, scope)?;
    let scope_blocked = !details.scope_ready(scope);
    let product_foundation_load_order = foundation_load_order(&product_foundation);
    let context_budget = manifest
        .jobs
        .iter()
        .find(|candidate| candidate.id == job)
        .and_then(|candidate| candidate.context_budget.as_ref());
    if optional_quota_enabled(context_budget) {
        apply_selection_authority(&mut details.context_entries, &product_foundation_load_order);
        allocate_context_entries(&mut details, context_budget);
    }
    let foundation_blocked = product_foundation.blocks_activation();
    let (ready_policy, model_visible_projection) = routed_context_projection(
        job,
        persona,
        scope,
        &product_foundation,
        &product_foundation_load_order,
        &details,
    );
    let minimality = context_minimality(
        manifest,
        job,
        &model_visible_projection,
        &details.full_card_required,
        &details.excluded,
        &details.route_card_cap,
        &details.allocation,
    )?;
    let minimality_blocked = minimality["status"] == "blocked";
    if validation_blocked
        || profile_activation_blocked
        || !draft_ready
        || scope_blocked
        || foundation_blocked
        || minimality_blocked
    {
        let blocked_reason = if validation_blocked {
            "pack validation failed for this job"
        } else if scope_blocked {
            "portfolio scope is missing or invalid"
        } else if foundation_blocked {
            "selected product foundation authority is blocked"
        } else if profile_activation["blocker_codes"]
            .as_array()
            .is_some_and(|codes| {
                codes
                    .iter()
                    .any(|code| code == "profile_activation_not_ready")
            })
        {
            "profile activation requires review or is blocked"
        } else if profile_activation_blocked {
            "computed profile activation is blocked"
        } else if details.route_card_cap["status"] == "blocked" {
            "route card cap excluded applicable authority"
        } else if minimality_blocked {
            "minimal context contract is blocked"
        } else {
            "draft_status no-draft"
        };
        let entries: Vec<Value> = if scope_blocked {
            details
                .context_entries
                .into_iter()
                .filter(|entry| entry["selection"] == "guardrail")
                .collect()
        } else {
            Vec::new()
        };
        let required_entry_count = entries
            .iter()
            .filter(|entry| entry["status"].as_str() == Some("required"))
            .count();
        let guardrail_entry_count = entries.len();
        let entry_count = entries.len();
        return Ok(json!({
            "contract": "mdp.context.v0",
            "status": "blocked",
            "runtime_context": runtime_context,
            "reason": blocked_reason,
            "persona": persona,
            "job": job,
            "scope": scope,
            "portfolio_sensitive": details.portfolio_sensitive,
            "product_foundation": resolution_json(&product_foundation),
            "product_foundation_load_order": product_foundation_load_order,
            "profile_activation": profile_activation,
            "source_load_order": if details.portfolio_sensitive { Vec::<Value>::new() } else { load_order.clone() },
            "route_card_cap": details.route_card_cap.clone(),
            "entries": entries,
            "gaps": details.gaps,
            "full_card_required": [],
            "minimality": minimality,
            "model_context": Value::Null,
            "summary": {
                "card_count": load_order.len(),
                "entry_count": entry_count,
                "required_entry_count": required_entry_count,
                "supporting_entry_count": entry_count.saturating_sub(required_entry_count),
                "guardrail_entry_count": guardrail_entry_count
            },
            "policy": if scope_blocked { "Do not draft until portfolio scope is resolved. Global bounded guardrails may be inspected, but shared card paths are not scope-filtered context." } else { "Do not draft from bounded context when draft_status is no-draft. Entry metadata is advisory context only." }
        }));
    }

    let required_entry_count = details
        .context_entries
        .iter()
        .filter(|entry| entry["status"].as_str() == Some("required"))
        .count();
    let guardrail_entry_count = details
        .context_entries
        .iter()
        .filter(|entry| entry["selection"].as_str() == Some("guardrail"))
        .count();
    let entry_count = details.context_entries.len();

    Ok(json!({
        "contract": "mdp.context.v0",
        "status": "ready",
        "runtime_context": runtime_context,
        "persona": persona,
        "job": job,
        "scope": scope,
        "portfolio_sensitive": details.portfolio_sensitive,
        "product_foundation": resolution_json(&product_foundation),
        "product_foundation_load_order": product_foundation_load_order,
        "profile_activation": profile_activation,
        "source_load_order": if details.portfolio_sensitive { Vec::<Value>::new() } else { load_order.clone() },
        "route_card_cap": details.route_card_cap.clone(),
        "entries": details.context_entries,
        "gaps": details.gaps,
        "full_card_required": details.full_card_required,
        "minimality": minimality,
        "model_context": model_visible_projection,
        "summary": {
            "card_count": load_order.len(),
            "entry_count": entry_count,
            "required_entry_count": required_entry_count,
            "supporting_entry_count": entry_count.saturating_sub(required_entry_count),
            "guardrail_entry_count": guardrail_entry_count
        },
        "policy": ready_policy
    }))
}

fn routed_context_projection(
    job: &str,
    persona: &str,
    scope: &ScopeResolution,
    product_foundation: &ProductFoundationResolution,
    product_foundation_load_order: &[Value],
    details: &EntryRouteDetails,
) -> (&'static str, Value) {
    let policy = if details.portfolio_sensitive {
        "Use scope-filtered context.entries only. Shared full cards are not scope-safe drafting context. Treat entry metadata as advisory context, not enforced CLI constraints."
    } else {
        "Use context.entries only. Canonical jobs must not open undeclared cards or whole-card fallbacks."
    };
    let projection = json!({
        "contract": ROUTED_CONTEXT_CONTRACT,
        "job": job,
        "persona": persona,
        "scope": scope,
        "product_foundation": resolution_json(product_foundation),
        "product_foundation_load_order": product_foundation_load_order,
        "entries": details.context_entries,
        "gaps": details.gaps,
        "policy": policy
    });
    (policy, projection)
}

fn context_minimality(
    manifest: &Manifest,
    job_id: &str,
    model_visible_projection: &Value,
    full_card_required: &[Value],
    excluded: &[Value],
    route_card_cap: &Value,
    allocation: &Value,
) -> Result<Value> {
    let route_card_cap_blocked = route_card_cap["status"] == "blocked";
    let Some(job) = manifest.jobs.iter().find(|job| job.id == job_id) else {
        let mut diagnostics = vec![json!("canonical_job_not_declared")];
        if route_card_cap_blocked {
            diagnostics.push(json!(ROUTE_CARD_CAP_DIAGNOSTIC));
        }
        return Ok(json!({
            "status": if route_card_cap_blocked { "blocked" } else { "unassessed" },
            "context_sha256": Value::Null,
            "budget": Value::Null,
            "excluded": excluded,
            "diagnostics": diagnostics
        }));
    };
    let Some(budget) = job.context_budget.as_ref() else {
        let mut diagnostics = vec![json!("context_budget_not_declared")];
        if route_card_cap_blocked {
            diagnostics.push(json!(ROUTE_CARD_CAP_DIAGNOSTIC));
        }
        return Ok(json!({
            "status": if route_card_cap_blocked { "blocked" } else { "unassessed" },
            "context_sha256": Value::Null,
            "budget": Value::Null,
            "excluded": excluded,
            "diagnostics": diagnostics
        }));
    };

    let selected_count = selected_authority_count(model_visible_projection);
    let canonical_context = canonical_json_bytes(model_visible_projection)?;
    let actual_bytes = canonical_context.len();
    let largest_contributing_cards = largest_contributing_cards(model_visible_projection);
    let mut diagnostics = Vec::new();
    if !full_card_required.is_empty() {
        diagnostics.push("full_card_fallback_required");
    }
    if route_card_cap_blocked {
        diagnostics.push(ROUTE_CARD_CAP_DIAGNOSTIC);
    }
    if selected_count > budget.max_entries {
        diagnostics.push("context_entry_budget_exceeded");
    }
    if actual_bytes > budget.max_bytes {
        diagnostics.push("context_byte_budget_exceeded");
    }
    let mut receipt = json!({
        "status": if diagnostics.is_empty() { "ready" } else { "blocked" },
        "context_sha256": sha256_hex(&canonical_context),
        "budget": {
            "max_entries": budget.max_entries,
            "max_bytes": budget.max_bytes,
            "actual_entries": selected_count,
            "actual_bytes": actual_bytes
        },
        "selected_count": selected_count,
        "excluded_count": excluded.len(),
        "excluded": excluded,
        "largest_contributing_cards": largest_contributing_cards,
        "diagnostics": diagnostics
    });
    if !allocation.is_null() {
        receipt["allocation"] = allocation.clone();
    }
    Ok(receipt)
}

fn selected_authority_count(model_visible_projection: &Value) -> usize {
    let mut selected = BTreeSet::new();
    for entry in model_visible_projection["entries"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if let (Some(card_id), Some(entry_id)) =
            (entry["card_id"].as_str(), entry["entry_id"].as_str())
        {
            selected.insert((card_id, entry_id));
        }
    }
    for reference in model_visible_projection["product_foundation_load_order"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if let (Some(card_id), Some(entry_id)) = (
            reference["card_id"].as_str(),
            reference["entry_id"].as_str(),
        ) {
            selected.insert((card_id, entry_id));
        }
    }
    selected.len()
}

/// Groups the model-visible routed context by contributing card and reports the
/// canonical JSON byte size of each card's selected entries. Entry bodies are
/// never included in the returned diagnostics; only stable identifiers, kinds,
/// counts, and byte sizes escape. The byte size is the canonical JSON size of
/// that card's entry slice, not the marginal contribution to the full context,
/// so callers can identify which cards dominate a declared budget.
fn largest_contributing_cards(model_visible_projection: &Value) -> Vec<Value> {
    let Some(entries) = model_visible_projection["entries"].as_array() else {
        return Vec::new();
    };
    let mut groups: BTreeMap<String, (Value, Vec<&Value>)> = BTreeMap::new();
    for entry in entries {
        let Some(card_id) = entry["card_id"].as_str() else {
            continue;
        };
        let card_kind = entry["card_kind"].clone();
        groups
            .entry(card_id.to_string())
            .or_insert_with(|| (card_kind, Vec::new()))
            .1
            .push(entry);
    }
    let mut contributions = Vec::new();
    for (card_id, (card_kind, card_entries)) in groups {
        let slice = Value::Array(card_entries.into_iter().cloned().collect());
        let bytes = canonical_json_bytes(&slice)
            .map(|bytes| bytes.len())
            .unwrap_or(0);
        contributions.push(json!({
            "card_id": card_id,
            "card_kind": card_kind,
            "entry_count": slice.as_array().map(Vec::len).unwrap_or(0),
            "canonical_bytes": bytes
        }));
    }
    contributions.sort_by(|left, right| {
        right["canonical_bytes"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&left["canonical_bytes"].as_u64().unwrap_or(0))
            .then_with(|| {
                left["card_id"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(right["card_id"].as_str().unwrap_or(""))
            })
    });
    contributions.truncate(8);
    contributions
}

fn optional_quota_enabled(budget: Option<&JobContextBudget>) -> bool {
    budget.is_some_and(|budget| !budget.optional_kind_quotas.is_empty())
}

fn apply_selection_authority(entries: &mut [Value], foundation_load_order: &[Value]) {
    let foundation_refs = foundation_load_order
        .iter()
        .filter_map(|reference| {
            Some((
                reference["card_id"].as_str()?,
                reference["entry_id"].as_str()?,
            ))
        })
        .collect::<BTreeSet<_>>();
    for entry in entries {
        let reference = (entry["card_id"].as_str(), entry["entry_id"].as_str());
        let card_kind = serde_json::from_value::<CardKind>(entry["card_kind"].clone()).ok();
        let (selection_class, reason_code) = if entry["selection"] == "guardrail" {
            ("universal_guardrail", None)
        } else if reference
            .0
            .zip(reference.1)
            .is_some_and(|reference| foundation_refs.contains(&reference))
        {
            (
                "product_foundation_requirement",
                Some("product_foundation_requirement"),
            )
        } else if matches!(card_kind, Some(CardKind::Gaps)) {
            ("gap_requirement", Some("gap_requirement"))
        } else if entry["evidence"]
            .as_array()
            .is_some_and(|evidence| !evidence.is_empty())
        {
            ("evidence_dependency", Some("evidence_dependency"))
        } else if matches!(card_kind, Some(CardKind::ChannelPolicies)) {
            ("output_requirement", Some("output_requirement"))
        } else if entry["metadata"]["required"] == true {
            ("output_requirement", Some("output_requirement"))
        } else {
            ("persona_or_job_match", None)
        };
        entry["selection_class"] = json!(selection_class);
        entry["status"] = json!(if selection_class == "persona_or_job_match" {
            "supporting"
        } else {
            "required"
        });
        if let Some(reason_code) = reason_code
            && let Some(reason_codes) = entry["reason_codes"].as_array_mut()
            && !reason_codes.iter().any(|value| value == reason_code)
        {
            reason_codes.push(json!(reason_code));
        }
    }
}

fn allocate_context_entries(
    details: &mut EntryRouteDetails,
    budget: Option<&crate::models::JobContextBudget>,
) {
    let quotas = budget
        .map(|budget| {
            budget
                .optional_kind_quotas
                .iter()
                .filter_map(|(name, limit)| {
                    let kind = serde_json::from_value::<CardKind>(json!(name)).ok()?;
                    kind.optional_quota_allowed().then_some((kind, *limit))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut required = Vec::new();
    let mut optional = Vec::new();
    for entry in std::mem::take(&mut details.context_entries) {
        if entry["status"] == "required" {
            required.push(entry);
        } else {
            optional.push(entry);
        }
    }

    let mut selected_optional = Vec::new();
    let mut optional_selected_by_kind: BTreeMap<CardKind, usize> = BTreeMap::new();
    let mut optional_excluded_by_kind: BTreeMap<CardKind, usize> = BTreeMap::new();
    for entry in optional {
        let Some(card_kind) = serde_json::from_value::<CardKind>(entry["card_kind"].clone()).ok()
        else {
            selected_optional.push(entry);
            continue;
        };
        let selected_count = optional_selected_by_kind
            .entry(card_kind.clone())
            .or_default();
        let quota = quotas.get(&card_kind).copied();
        if quota.is_some_and(|quota| *selected_count >= quota) {
            details.excluded.push(json!({
                "card_id": entry["card_id"],
                "card_kind": entry["card_kind"],
                "entry_id": entry["entry_id"],
                "reason_code": "optional_kind_quota_exceeded"
            }));
            *optional_excluded_by_kind.entry(card_kind).or_default() += 1;
        } else {
            *selected_count += 1;
            selected_optional.push(entry);
        }
    }

    let mut required_by_kind = BTreeMap::new();
    for entry in &required {
        if let Ok(card_kind) = serde_json::from_value::<CardKind>(entry["card_kind"].clone()) {
            *required_by_kind.entry(card_kind.name()).or_insert(0usize) += 1;
        }
    }
    let mut quota_receipts = BTreeMap::new();
    for (card_kind, max_optional_entries) in quotas {
        quota_receipts.insert(
            card_kind.name(),
            json!({
                "max_optional_entries": max_optional_entries,
                "reserved_count": required_by_kind.get(card_kind.name()).copied().unwrap_or(0),
                "optional_selected_count": optional_selected_by_kind.get(&card_kind).copied().unwrap_or(0),
                "optional_excluded_count": optional_excluded_by_kind.get(&card_kind).copied().unwrap_or(0)
            }),
        );
    }
    details.context_entries = required.into_iter().chain(selected_optional).collect();
    details.allocation = json!({
        "strategy": "required-first",
        "required_count": details.context_entries.iter().filter(|entry| entry["status"] == "required").count(),
        "optional_selected_count": optional_selected_by_kind.values().sum::<usize>(),
        "optional_excluded_count": optional_excluded_by_kind.values().sum::<usize>(),
        "required_by_kind": required_by_kind,
        "quotas": quota_receipts
    });
}

fn foundation_load_order(resolution: &ProductFoundationResolution) -> Vec<Value> {
    let mut load_order = Vec::new();
    for facet in &resolution.selected_facets {
        for reference in &facet.entry_refs {
            load_order.push(json!({
                "facet_id": facet.id,
                "classification": facet.classification,
                "reference_kind": "entry",
                "card_id": reference.card_id,
                "entry_id": reference.entry_id
            }));
        }
        for reference in &facet.gap_refs {
            load_order.push(json!({
                "facet_id": facet.id,
                "classification": facet.classification,
                "reference_kind": "gap",
                "card_id": reference.card_id,
                "entry_id": reference.entry_id
            }));
        }
    }
    load_order
}

fn route_entry_details(
    root: &Path,
    manifest: &Manifest,
    persona: &str,
    job: &str,
    include_context: bool,
    scope: &ScopeResolution,
) -> Result<EntryRouteDetails> {
    let selection = select_cards_with_diagnostics(manifest, Some(persona), Some(job));
    let selected = selection.selected;
    let route_card_cap = selection.route_card_cap;
    let selected_ids: BTreeSet<String> = selected
        .iter()
        .filter_map(|value| value["id"].as_str().map(str::to_string))
        .collect();
    let job_tokens = tokens(job);
    let mut matches = Vec::new();
    let mut context_entries = Vec::new();
    let mut gaps = Vec::new();
    let mut full_card_required = Vec::new();
    let mut excluded = Vec::new();
    let mut portfolio_sensitive = false;
    let mut compatible_scoped_entry_count = 0usize;
    let mut scoped_decision_candidate_count = 0usize;
    let mut compatible_scoped_decision_count = 0usize;

    for card_ref in &manifest.cards {
        if !selected_ids.contains(&card_ref.id) {
            continue;
        }
        let card = read_card(&resolve_pack_path(root, &card_ref.path)?)?;
        let display_path = format!("{DEFAULT_DIR}/{}", card_ref.path);
        let mut card_match_count = 0usize;
        let mut selected_entry_count = 0usize;

        for entry in &card.entries {
            let entry_text = format!("{} {}", entry.title, entry.body).to_lowercase();
            let applies = selector_matches_persona(&entry.applies_to, persona);
            let entry_tokens = tokens(&entry_text);
            let job_match = token_overlap(&job_tokens, &entry_tokens);
            let entry_allowed =
                entry_policy_compatible(&card.kind, manifest, &job_tokens, &entry_tokens);
            let matched = !(matches!(card.kind, CardKind::ChannelPolicies) && !job_match)
                && entry_allowed
                && (applies || job_match);
            let guardrail = is_context_guardrail(&card.kind, entry);
            let scope_match = match_entry_scope(scope, &entry.scope);
            if !entry_allowed {
                excluded.push(excluded_entry(
                    &card.id,
                    &card.kind,
                    entry,
                    "policy_incompatible",
                ));
            } else if !(matched || guardrail) {
                excluded.push(excluded_entry(
                    &card.id,
                    &card.kind,
                    entry,
                    "not_applicable",
                ));
            }
            if (matched || guardrail) && !entry.scope.is_empty() {
                portfolio_sensitive = true;
                if scope_match.compatible {
                    compatible_scoped_entry_count += 1;
                }
                if matched && !guardrail {
                    scoped_decision_candidate_count += 1;
                    if scope_match.compatible {
                        compatible_scoped_decision_count += 1;
                    }
                }
            }
            if entry_allowed && (matched || guardrail) && !scope_match.compatible {
                excluded.push(excluded_entry(
                    &card.id,
                    &card.kind,
                    entry,
                    "scope_incompatible",
                ));
                for issue in scope_match.issues {
                    gaps.push(json!({
                        "card_id": card.id,
                        "entry_id": entry.id,
                        "title": entry.title,
                        "reason": issue.code,
                        "dimension": issue.dimension,
                        "value": issue.value,
                        "detail": issue.reason
                    }));
                }
                continue;
            }

            if matched {
                card_match_count += 1;
                matches.push(entry_summary(
                    &card.id,
                    &card.kind,
                    entry,
                    match_reason(applies, job_match),
                ));
            }
            if include_context && (matched || guardrail) {
                selected_entry_count += 1;
                context_entries.push(entry_context_value(
                    &card.id,
                    &card.kind,
                    &display_path,
                    entry,
                    if guardrail { "guardrail" } else { "matched" },
                    if matched {
                        match_reason(applies, job_match)
                    } else {
                        guardrail_reason(&card.kind)
                    },
                ));
            }
        }
        if card_match_count == 0 {
            gaps.push(json!({
                "card_id": card.id,
                "path": display_path,
                "reason": "card routed, but no entry matched persona/job cleanly"
            }));
        }
        if include_context && selected_entry_count == 0 {
            full_card_required.push(json!({
                "card_id": card.id,
                "card_kind": card.kind,
                "path": display_path,
                "reason": "routed card had no bounded entries; open full card only if this card is needed for the task"
            }));
        }
    }

    if portfolio_sensitive {
        full_card_required.clear();
    }

    Ok(EntryRouteDetails {
        matches,
        context_entries,
        gaps,
        full_card_required,
        excluded,
        route_card_cap,
        portfolio_sensitive,
        compatible_scoped_entry_count,
        scoped_decision_candidate_count,
        compatible_scoped_decision_count,
        allocation: Value::Null,
    })
}

fn entry_summary(card_id: &str, card_kind: &CardKind, entry: &Entry, reason: &str) -> Value {
    json!({
        "card_id": card_id,
        "card_kind": card_kind,
        "entry_id": entry.id,
        "title": entry.title,
        "status": entry_status(card_kind),
        "reason": reason,
        "metadata": entry.metadata,
        "evidence_count": entry.evidence.len(),
        "avoid_count": entry.avoid.len(),
        "constraints": entry.constraints,
        "scope": entry.scope
    })
}

fn entry_context_value(
    card_id: &str,
    card_kind: &CardKind,
    card_path: &str,
    entry: &Entry,
    selection: &str,
    reason: &str,
) -> Value {
    let reason_code = match selection {
        "guardrail" => guardrail_reason_code(card_kind),
        _ if reason == "persona applies" => "persona_applicability",
        _ if reason == "entry job match" => "job_match",
        _ => "persona_text_match",
    };
    json!({
        "card_id": card_id,
        "card_kind": card_kind,
        "card_path": card_path,
        "entry_id": entry.id,
        "title": entry.title,
        "body": entry.body,
        "applies_to": entry.applies_to,
        "scope": entry.scope,
        "evidence": entry.evidence,
        "avoid": entry.avoid,
        "exact_paragraphs": entry.exact_paragraphs,
        "constraints": entry.constraints,
        "metadata": entry.metadata,
        "status": entry_status(card_kind),
        "selection": selection,
        "selection_class": if selection == "guardrail" { "universal_guardrail" } else { "persona_or_job_match" },
        "reason": reason,
        "reason_codes": [reason_code]
    })
}

fn excluded_entry(card_id: &str, card_kind: &CardKind, entry: &Entry, reason_code: &str) -> Value {
    json!({
        "card_id": card_id,
        "card_kind": card_kind,
        "entry_id": entry.id,
        "reason_code": reason_code
    })
}

fn guardrail_reason_code(card_kind: &CardKind) -> &'static str {
    match card_kind {
        CardKind::FitRules => "fit_guardrail",
        CardKind::OutputRules => "output_rule_guardrail",
        _ => "avoid_rule_guardrail",
    }
}

fn entry_status(card_kind: &CardKind) -> &'static str {
    if matches!(
        card_kind,
        CardKind::AvoidRules
            | CardKind::OutputRules
            | CardKind::FitRules
            | CardKind::Claims
            | CardKind::Positioning
            | CardKind::ChannelPolicies
    ) {
        "required"
    } else {
        "supporting"
    }
}

fn match_reason(applies: bool, job_match: bool) -> &'static str {
    if applies {
        "persona applies"
    } else if job_match {
        "entry job match"
    } else {
        "persona text match"
    }
}

fn is_context_guardrail(card_kind: &CardKind, entry: &Entry) -> bool {
    matches!(card_kind, CardKind::AvoidRules | CardKind::OutputRules)
        || (matches!(card_kind, CardKind::FitRules) && !entry.avoid.is_empty())
}

fn guardrail_reason(card_kind: &CardKind) -> &'static str {
    if matches!(card_kind, CardKind::FitRules) {
        "fit guardrail included"
    } else if matches!(card_kind, CardKind::OutputRules) {
        "output-rule guardrail included"
    } else {
        "avoid-rule guardrail included"
    }
}

pub(crate) fn tokens(input: &str) -> Vec<String> {
    input
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| token.len() >= 2)
        .map(str::to_string)
        .collect()
}

pub(crate) fn token_overlap(left: &[String], right: &[String]) -> bool {
    left.iter()
        .any(|token| right.iter().any(|other| other == token))
}

pub(crate) fn selector_is_universal(values: &[String]) -> bool {
    values.iter().all(|value| value.trim().is_empty())
}

pub(crate) fn selector_matches_persona(values: &[String], persona: &str) -> bool {
    selector_is_universal(values)
        || values.iter().any(|value| {
            let candidate = value.trim();
            !candidate.is_empty() && candidate.eq_ignore_ascii_case(persona.trim())
        })
}

fn entry_policy_compatible(
    card_kind: &CardKind,
    manifest: &Manifest,
    job_tokens: &[String],
    entry_tokens: &[String],
) -> bool {
    if matches!(card_kind, CardKind::ChannelPolicies) {
        channel_compatible(&manifest.supported_channels, job_tokens, entry_tokens)
            && lifecycle_compatible(job_tokens, entry_tokens)
    } else {
        true
    }
}

fn lifecycle_compatible(job_tokens: &[String], entry_tokens: &[String]) -> bool {
    match (lifecycle_stage(job_tokens), lifecycle_stage(entry_tokens)) {
        (Some(job_stage), Some(entry_stage)) => job_stage == entry_stage,
        (Some(_), None) => true,
        (None, Some(MessageLifecycle::FollowUp)) => false,
        (None, Some(MessageLifecycle::Initial)) | (None, None) => true,
    }
}

fn lifecycle_stage(tokens: &[String]) -> Option<MessageLifecycle> {
    if has_token(tokens, "followup") || (has_token(tokens, "follow") && has_token(tokens, "up")) {
        Some(MessageLifecycle::FollowUp)
    } else if has_token(tokens, "initial")
        || has_token(tokens, "opener")
        || has_token(tokens, "opening")
        || (has_token(tokens, "first") && has_token(tokens, "touch"))
    {
        Some(MessageLifecycle::Initial)
    } else {
        None
    }
}

fn channel_compatible(
    supported_channels: &[String],
    job_tokens: &[String],
    entry_tokens: &[String],
) -> bool {
    let job_channels = message_channels(supported_channels, job_tokens);
    let entry_channels = message_channels(supported_channels, entry_tokens);
    if job_channels.is_empty() || entry_channels.is_empty() {
        return true;
    }
    job_channels
        .iter()
        .any(|channel| entry_channels.contains(channel))
}

fn message_channels(supported_channels: &[String], tokens: &[String]) -> BTreeSet<String> {
    let mut channels = BTreeSet::new();
    for channel in supported_channels {
        let channel_tokens = tokens_for_channel(channel);
        if !channel_tokens.is_empty()
            && channel_tokens
                .iter()
                .all(|channel_token| has_token(tokens, channel_token))
        {
            channels.insert(channel.to_lowercase());
        }
    }
    channels
}

fn tokens_for_channel(channel: &str) -> Vec<String> {
    tokens(channel)
}

fn has_token(tokens: &[String], needle: &str) -> bool {
    tokens.iter().any(|token| token == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use crate::models::{CardRef, Entry, LeadInputRequirements, Policy, Provenance};
    use crate::pack_io::read_manifest;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_pack(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-routing-{name}-{nonce}"));
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        narrow_starter_route_candidates_for_tests(&root);
        root
    }

    fn set_max_cards_per_route(root: &Path, max_cards: usize) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["policy"]["max_cards_per_route"] = serde_yaml::Value::Number(max_cards.into());
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    fn add_second_supplemental_persona_card(root: &Path) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        let card_ref = manifest["cards"]
            .as_sequence()
            .expect("cards")
            .iter()
            .find(|card| card["id"] == "supplemental-personas")
            .cloned()
            .expect("first supplemental card");
        let mut second_ref = card_ref;
        second_ref["id"] = serde_yaml::Value::String("supplemental-personas-2".to_string());
        second_ref["path"] =
            serde_yaml::Value::String("cards/supplemental-personas-2.yaml".to_string());
        manifest["cards"]
            .as_sequence_mut()
            .expect("cards")
            .push(second_ref);
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let first_path = root.join(".mdp/cards/supplemental-personas.yaml");
        let raw = std::fs::read_to_string(&first_path).expect("first card should be readable");
        let mut card: serde_yaml::Value = serde_yaml::from_str(&raw).expect("card should parse");
        card["id"] = serde_yaml::Value::String("supplemental-personas-2".to_string());
        std::fs::write(
            root.join(".mdp/cards/supplemental-personas-2.yaml"),
            serde_yaml::to_string(&card).expect("card should serialize"),
        )
        .expect("card should be writable");
    }

    fn add_selected_foundation_gap(root: &Path) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile"]["product_foundation"]["facets"][0]["gaps"] =
            serde_yaml::from_str("- card_id: gaps\n  entry_id: missing-company-proof\n")
                .expect("gap reference should parse");
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    fn remove_required_eval_category(root: &Path) {
        for entry in std::fs::read_dir(root.join(".mdp/evals")).expect("evals should be readable") {
            let path = entry.expect("eval entry should load").path();
            let raw = std::fs::read_to_string(&path).expect("eval should be readable");
            std::fs::write(
                path,
                raw.replace("category: prompt-output-validation", "category: proceed"),
            )
            .expect("eval should be writable");
        }
    }

    fn set_profile_activation(root: &Path, status: &str) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile_eval"]["activation"]["status"] =
            serde_yaml::Value::String(status.to_string());
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    fn set_foundation_facet_kind(root: &Path, kind: &str) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile"]["product_foundation"]["facets"][0]["kind"] =
            serde_yaml::Value::String(kind.to_string());
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    fn duplicate_foundation_entry(root: &Path) {
        let card_path = root.join(".mdp/cards/positioning.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("card should be readable");
        let mut card: serde_yaml::Value = serde_yaml::from_str(&raw).expect("card should parse");
        let first_entry = card["entries"][0].clone();
        card["entries"]
            .as_sequence_mut()
            .expect("card entries")
            .push(first_entry);
        std::fs::write(
            card_path,
            serde_yaml::to_string(&card).expect("card should serialize"),
        )
        .expect("card should be writable");
    }

    fn set_unrelated_job_condition_fact(root: &Path, fact: &str) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["jobs"][1]["product_foundation"]["conditional"] = serde_yaml::from_str(&format!(
            "- facet_id: known-gaps\n  when:\n    fact: {fact}\n    equals: outbound-copy-brief\n"
        ))
        .expect("conditional should parse");
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    fn duplicate_job_id(root: &Path) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["jobs"][1]["id"] = manifest["jobs"][0]["id"].clone();
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    fn set_context_budget(root: &Path, job_id: &str, max_entries: usize, max_bytes: usize) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        let jobs = manifest["jobs"].as_sequence_mut().expect("jobs");
        let job = jobs
            .iter_mut()
            .find(|job| job["id"].as_str() == Some(job_id))
            .expect("job should exist");
        job["context_budget"] = serde_yaml::to_value(crate::models::JobContextBudget {
            max_entries,
            max_bytes,
            optional_kind_quotas: BTreeMap::new(),
        })
        .expect("budget should serialize");
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    fn set_optional_kind_quotas(root: &Path, job_id: &str, quotas: &[(&str, usize)]) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        let job = manifest["jobs"]
            .as_sequence_mut()
            .expect("jobs")
            .iter_mut()
            .find(|job| job["id"].as_str() == Some(job_id))
            .expect("job should exist");
        let quota_map = quotas
            .iter()
            .map(|(kind, limit)| ((*kind).to_string(), *limit))
            .collect::<BTreeMap<_, _>>();
        job["context_budget"]["optional_kind_quotas"] =
            serde_yaml::to_value(quota_map).expect("quotas should serialize");
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    fn add_evidence_backed_entries(root: &Path, card_id: &str) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let manifest_raw = std::fs::read_to_string(&manifest_path).expect("manifest should load");
        let manifest: serde_yaml::Value =
            serde_yaml::from_str(&manifest_raw).expect("manifest should parse");
        let card_ref = manifest["cards"]
            .as_sequence()
            .expect("cards")
            .iter()
            .find(|card| card["id"] == card_id)
            .expect("card reference should exist");
        let card_path = root.join(".mdp").join(
            card_ref["path"]
                .as_str()
                .expect("card path should be a string"),
        );
        let raw = std::fs::read_to_string(&card_path).expect("card should load");
        let mut card: serde_yaml::Value = serde_yaml::from_str(&raw).expect("card should parse");
        let template = card["entries"][0].clone();
        for suffix in ["one", "two"] {
            let mut entry = template.clone();
            entry["id"] = serde_yaml::Value::String(format!("{card_id}-evidence-{suffix}"));
            entry["title"] =
                serde_yaml::Value::String(format!("Evidence-backed {card_id} {suffix}"));
            entry["body"] = serde_yaml::Value::String(
                "Evidence-backed PMM outbound context for deterministic allocation coverage."
                    .to_string(),
            );
            entry["applies_to"] = serde_yaml::from_str("- PMM\n").expect("persona selector");
            entry["evidence"] =
                serde_yaml::from_str("- mdp-reference-contract\n").expect("evidence reference");
            card["entries"]
                .as_sequence_mut()
                .expect("entries")
                .push(entry);
        }
        std::fs::write(
            card_path,
            serde_yaml::to_string(&card).expect("card should serialize"),
        )
        .expect("card should write");
    }

    fn make_card_entries_optional_for_tests(root: &Path, card_id: &str) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let manifest_raw = std::fs::read_to_string(&manifest_path).expect("manifest should load");
        let manifest: serde_yaml::Value =
            serde_yaml::from_str(&manifest_raw).expect("manifest should parse");
        let card_ref = manifest["cards"]
            .as_sequence()
            .expect("cards")
            .iter()
            .find(|card| card["id"] == card_id)
            .expect("card reference should exist");
        let card_path = root.join(".mdp").join(
            card_ref["path"]
                .as_str()
                .expect("card path should be a string"),
        );
        let raw = std::fs::read_to_string(&card_path).expect("card should load");
        let mut card: serde_yaml::Value = serde_yaml::from_str(&raw).expect("card should parse");
        for entry in card["entries"].as_sequence_mut().expect("entries") {
            if let Some(object) = entry.as_mapping_mut() {
                object.remove(serde_yaml::Value::String("evidence".to_string()));
                object.remove(serde_yaml::Value::String("metadata".to_string()));
            }
        }
        std::fs::write(
            card_path,
            serde_yaml::to_string(&card).expect("card should serialize"),
        )
        .expect("card should write");
    }

    fn add_supplemental_persona_card(root: &Path) {
        add_supplemental_persona_card_for_tests(root);
    }

    fn narrow_route_candidates_for_exact_cap(root: &Path) {
        narrow_starter_route_candidates_for_tests(root);
    }

    fn manifest(max_cards_per_route: usize) -> Manifest {
        Manifest {
            format: "mdp.v0".to_string(),
            id: "test".to_string(),
            name: "Test".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            target: None,
            profile: None,
            personas: vec!["PMM".to_string()],
            target_personas: vec![],
            operator_roles: vec![],
            supported_channels: vec![
                "linkedin".to_string(),
                "email".to_string(),
                "call-prep".to_string(),
                "partner-intro".to_string(),
            ],
            persona_mappings: vec![],
            lead_input_requirements: LeadInputRequirements::default(),
            qualification_gates: None,
            required_primitives: Vec::new(),
            primitive_map: std::collections::BTreeMap::new(),
            decision_input_contracts: Vec::new(),
            input_contracts: Vec::new(),
            jobs: Vec::new(),
            profile_eval: crate::models::ProfileEval::default(),
            cards: vec![
                CardRef {
                    id: "personas".to_string(),
                    path: "cards/personas.yaml".to_string(),
                    kind: CardKind::Personas,
                    description: "Personas".to_string(),
                    personas: vec!["PMM".to_string()],
                    tags: vec!["persona".to_string()],
                },
                CardRef {
                    id: "avoid-rules".to_string(),
                    path: "cards/avoid-rules.yaml".to_string(),
                    kind: CardKind::AvoidRules,
                    description: "Avoid".to_string(),
                    personas: vec!["PMM".to_string()],
                    tags: vec!["avoid".to_string()],
                },
                CardRef {
                    id: "output-rules".to_string(),
                    path: "cards/output-rules.yaml".to_string(),
                    kind: CardKind::OutputRules,
                    description: "Output rules".to_string(),
                    personas: vec!["PMM".to_string()],
                    tags: vec!["style".to_string()],
                },
                CardRef {
                    id: "ctas".to_string(),
                    path: "cards/ctas.yaml".to_string(),
                    kind: CardKind::Ctas,
                    description: "CTA policy".to_string(),
                    personas: vec!["PMM".to_string()],
                    tags: vec!["cta".to_string()],
                },
                CardRef {
                    id: "motions".to_string(),
                    path: "cards/motions.yaml".to_string(),
                    kind: CardKind::Motions,
                    description: "Motions".to_string(),
                    personas: vec!["PMM".to_string()],
                    tags: vec!["motion".to_string()],
                },
            ],
            policy: Policy {
                progressive_disclosure: true,
                load_manifest_first: true,
                max_cards_per_route,
                json_contract: "mdp.cli.v0".to_string(),
                no_auth_required: true,
            },
            provenance: Provenance {
                owner: "local".to_string(),
                created_by: "test".to_string(),
                notes: vec![],
            },
        }
    }

    #[test]
    fn select_cards_keeps_base_guardrails_and_message_priority() {
        let selected = select_cards(&manifest(5), Some("PMM"), Some("linkedin outbound copy"));
        let ids: Vec<&str> = selected
            .iter()
            .filter_map(|card| card["id"].as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["personas", "avoid-rules", "output-rules", "ctas", "motions"]
        );
    }

    #[test]
    fn selector_helpers_treat_empty_and_blank_values_as_universal() {
        assert!(selector_is_universal(&[]));
        assert!(selector_is_universal(&["".to_string(), "  ".to_string()]));
        assert!(!selector_is_universal(&[
            "  PMM  ".to_string(),
            "".to_string()
        ]));
        assert!(selector_matches_persona(
            &["  pMm  ".to_string(), "".to_string()],
            "PMM"
        ));
        assert!(!selector_matches_persona(
            &["  pMm  ".to_string(), "".to_string()],
            "Buyer"
        ));
        assert!(selector_matches_persona(
            &["".to_string(), "  ".to_string()],
            "Buyer"
        ));
        let mixed = vec!["".to_string(), "PMM".to_string()];
        assert!(selector_matches_persona(&mixed, "PMM"));
        assert!(!selector_matches_persona(&mixed, "Buyer"));
        assert_eq!(mixed, vec!["".to_string(), "PMM".to_string()]);
    }

    #[test]
    fn universal_card_and_entry_route_without_prose_persona_inference() {
        let root = temp_pack("universal-gap-routing");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["personas"]
            .as_sequence_mut()
            .expect("personas")
            .push(serde_yaml::Value::String("Buyer".to_string()));
        manifest["target_personas"]
            .as_sequence_mut()
            .expect("target personas")
            .push(serde_yaml::Value::String("Buyer".to_string()));
        manifest["policy"]["max_cards_per_route"] = serde_yaml::Value::Number(100.into());
        let gaps_ref = manifest["cards"]
            .as_sequence_mut()
            .expect("cards")
            .iter_mut()
            .find(|card| card["id"] == "gaps")
            .expect("gaps card ref");
        gaps_ref["personas"] = serde_yaml::Value::Sequence(Vec::new());
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let card_path = root.join(".mdp/cards/gaps.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("card should be readable");
        let mut card: serde_yaml::Value = serde_yaml::from_str(&raw).expect("card should parse");
        card["personas"] = serde_yaml::Value::Sequence(Vec::new());
        card["entries"] = serde_yaml::from_str(
            r#"
- id: unresolved-public-authority
  title: Neutral unresolved authority
  body: This synthetic gap has no actor words in its prose and is reachable through structured emptiness.
  applies_to: []
  evidence: []
  avoid: []
- id: scoped-comparison
  title: PMM-only comparison
  body: This synthetic comparison remains limited to one declared persona.
  applies_to:
  - PMM
  evidence: []
  avoid: []
"#,
        )
        .expect("fixture entries should parse");
        std::fs::write(
            &card_path,
            serde_yaml::to_string(&card).expect("card should serialize"),
        )
        .expect("card should be writable");

        let manifest = read_manifest(&root).expect("manifest should load");
        let details = route_entry_details(
            &root,
            &manifest,
            "Buyer",
            "neutral review",
            true,
            &ScopeResolution::default(),
        )
        .expect("route details should compile");

        assert!(
            details
                .matches
                .iter()
                .any(|entry| entry["entry_id"] == "unresolved-public-authority")
        );
        assert!(
            details
                .context_entries
                .iter()
                .any(|entry| entry["entry_id"] == "unresolved-public-authority")
        );
        assert!(!details.excluded.iter().any(|entry| {
            entry["entry_id"] == "unresolved-public-authority"
                && entry["reason_code"] == "not_applicable"
        }));
        assert!(
            details
                .excluded
                .iter()
                .any(|entry| entry["entry_id"] == "scoped-comparison"
                    && entry["reason_code"] == "not_applicable")
        );
        assert_eq!(
            details
                .context_entries
                .iter()
                .find(|entry| entry["entry_id"] == "unresolved-public-authority")
                .expect("universal context entry")["applies_to"],
            json!([])
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn opted_in_context_exposes_stable_minimality_digest_and_safe_exclusions() {
        let root = temp_pack("minimality-ready");
        narrow_route_candidates_for_exact_cap(&root);
        set_context_budget(&root, "outbound-copy-brief", 100, 1_000_000);
        let manifest = read_manifest(&root).expect("manifest should load");
        let scope = ScopeResolution::default();

        let first =
            entry_context_scoped(&root, &manifest, "PMM", "outbound-copy-brief", true, &scope)
                .expect("context should compile");
        let second =
            entry_context_scoped(&root, &manifest, "PMM", "outbound-copy-brief", true, &scope)
                .expect("context should replay");

        assert_eq!(first["minimality"]["status"], "ready");
        assert_eq!(
            first["minimality"]["context_sha256"],
            second["minimality"]["context_sha256"]
        );
        assert_eq!(
            first["minimality"]["context_sha256"]
                .as_str()
                .expect("digest")
                .len(),
            64
        );
        assert!(
            first["entries"]
                .as_array()
                .expect("entries")
                .iter()
                .all(|entry| entry["reason_codes"].is_array())
        );
        let excluded = first["minimality"]["excluded"]
            .as_array()
            .expect("excluded");
        assert!(excluded.iter().all(|entry| entry.get("body").is_none()));
        let unique_excluded = excluded
            .iter()
            .map(|entry| {
                (
                    entry["card_id"].as_str().expect("card id"),
                    entry["entry_id"].as_str().expect("entry id"),
                )
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(unique_excluded.len(), excluded.len());
        assert_eq!(first["minimality"]["excluded_count"], excluded.len());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn required_first_allocator_bounds_optional_kinds_without_displacing_authority() {
        let root = temp_pack("required-first-allocation");
        make_card_entries_optional_for_tests(&root, "hooks");
        set_context_budget(&root, "outbound-copy-brief", 100, 1_000_000);
        set_optional_kind_quotas(
            &root,
            "outbound-copy-brief",
            &[("hooks", 1), ("pains", 1), ("ctas", 1)],
        );
        let manifest = read_manifest(&root).expect("manifest should load");
        let first = entry_context_scoped(
            &root,
            &manifest,
            "PMM",
            "outbound-copy-brief",
            true,
            &ScopeResolution::default(),
        )
        .expect("context should compile");
        let second = entry_context_scoped(
            &root,
            &manifest,
            "PMM",
            "outbound-copy-brief",
            true,
            &ScopeResolution::default(),
        )
        .expect("context should replay");
        let route = entry_route_scoped(
            &root,
            &manifest,
            "PMM",
            "outbound-copy-brief",
            &ScopeResolution::default(),
        )
        .expect("route should compile");

        assert_eq!(first["status"], "ready");
        assert_eq!(
            first["minimality"]["allocation"]["strategy"],
            "required-first"
        );
        assert_eq!(
            first["minimality"]["allocation"],
            second["minimality"]["allocation"]
        );
        assert_eq!(
            first["minimality"]["context_sha256"],
            second["minimality"]["context_sha256"]
        );
        assert_eq!(
            first["minimality"]["allocation"],
            route["minimality"]["allocation"]
        );
        let preflight = route_budget_preflight(&root, &manifest).expect("preflight should compile");
        let preflight_route = preflight["routes"]
            .as_array()
            .expect("routes")
            .iter()
            .find(|candidate| {
                candidate["persona"] == "PMM" && candidate["job"] == "outbound-copy-brief"
            })
            .expect("PMM outbound-copy-brief preflight route");
        assert_eq!(
            preflight_route["allocation"],
            first["minimality"]["allocation"]
        );
        assert!(
            first["minimality"]["allocation"]["optional_excluded_count"]
                .as_u64()
                .is_some_and(|count| count > 0)
        );
        assert!(first["minimality"]["allocation"]["quotas"].is_object());
        let entries = first["entries"].as_array().expect("entries");
        let required = entries
            .iter()
            .filter(|entry| entry["status"] == "required")
            .collect::<Vec<_>>();
        assert_eq!(
            first["minimality"]["allocation"]["required_count"],
            required.len()
        );
        assert!(
            first["minimality"]["excluded"]
                .as_array()
                .expect("excluded")
                .iter()
                .any(|entry| entry["reason_code"] == "optional_kind_quota_exceeded")
        );
        assert!(
            first["minimality"]["excluded"]
                .as_array()
                .expect("excluded")
                .iter()
                .all(|entry| entry.get("body").is_none() && entry.get("evidence").is_none())
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_context_preserves_classification_and_receipt_shape_without_quotas() {
        let root = temp_pack("legacy-context-compatibility");
        set_context_budget(&root, "outbound-copy-brief", 100, 1_000_000);
        let manifest = read_manifest(&root).expect("manifest should load");
        let scope = ScopeResolution::default();
        let route = entry_route_scoped(&root, &manifest, "PMM", "outbound-copy-brief", &scope)
            .expect("legacy route should compile");
        let context =
            entry_context_scoped(&root, &manifest, "PMM", "outbound-copy-brief", true, &scope)
                .expect("legacy context should compile");

        for entry in context["entries"].as_array().expect("context entries") {
            let card_kind =
                serde_json::from_value::<CardKind>(entry["card_kind"].clone()).expect("card kind");
            let expected_class = if entry["selection"] == "guardrail" {
                "universal_guardrail"
            } else {
                "persona_or_job_match"
            };
            assert_eq!(entry["status"], entry_status(&card_kind));
            assert_eq!(entry["selection_class"], expected_class);
        }
        assert!(route["minimality"].get("allocation").is_none());
        assert!(context["minimality"].get("allocation").is_none());
        assert_eq!(route["minimality"], context["minimality"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn evidence_backed_non_claim_entries_are_reserved_before_quotas() {
        let root = temp_pack("evidence-backed-reservations");
        for card_id in ["pains", "copy-patterns", "motions", "signals"] {
            add_evidence_backed_entries(&root, card_id);
        }
        set_context_budget(&root, "outbound-copy-brief", 100, 1_000_000);
        set_optional_kind_quotas(
            &root,
            "outbound-copy-brief",
            &[
                ("pains", 1),
                ("copy-patterns", 1),
                ("motions", 1),
                ("signals", 1),
            ],
        );
        let manifest = read_manifest(&root).expect("manifest should load");
        let context = entry_context_scoped(
            &root,
            &manifest,
            "PMM",
            "outbound-copy-brief",
            true,
            &ScopeResolution::default(),
        )
        .expect("context should compile");
        let entries = context["entries"].as_array().expect("entries");
        for card_id in ["pains", "copy-patterns", "motions", "signals"] {
            for suffix in ["one", "two"] {
                let entry_id = format!("{card_id}-evidence-{suffix}");
                let entry = entries
                    .iter()
                    .find(|entry| entry["entry_id"] == entry_id)
                    .expect("evidence-backed entry should remain selected");
                assert_eq!(entry["status"], "required");
                assert_eq!(entry["selection_class"], "evidence_dependency");
                assert!(
                    !context["minimality"]["excluded"]
                        .as_array()
                        .expect("excluded")
                        .iter()
                        .any(|excluded| excluded["entry_id"] == entry_id)
                );
            }
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn context_budget_overflow_blocks_without_dropping_guardrails() {
        let root = temp_pack("minimality-overflow");
        set_context_budget(&root, "outbound-copy-brief", 1, 1_000_000);
        set_optional_kind_quotas(
            &root,
            "outbound-copy-brief",
            &[("hooks", 1), ("pains", 1), ("ctas", 1)],
        );
        let manifest = read_manifest(&root).expect("manifest should load");

        let context = entry_context_scoped(
            &root,
            &manifest,
            "PMM",
            "outbound-copy-brief",
            true,
            &ScopeResolution::default(),
        )
        .expect("context should compile");

        assert_eq!(context["status"], "blocked");
        assert_eq!(context["minimality"]["status"], "blocked");
        assert!(
            context["minimality"]["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|code| code == "context_entry_budget_exceeded")
        );
        assert_eq!(context["minimality"]["budget"]["max_entries"], 1);
        assert!(
            context["minimality"]["allocation"]["required_count"]
                .as_u64()
                .expect("required count")
                > 1
        );
        assert!(
            context["minimality"]["budget"]["actual_entries"]
                .as_u64()
                .expect("actual entries")
                > 1
        );
        assert!(context["entries"].as_array().expect("entries").is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn context_byte_budget_overflow_blocks_and_hides_model_context() {
        let root = temp_pack("minimality-byte-overflow");
        set_context_budget(&root, "outbound-copy-brief", 100, 1);
        let manifest = read_manifest(&root).expect("manifest should load");

        let context = entry_context_scoped(
            &root,
            &manifest,
            "PMM",
            "outbound-copy-brief",
            true,
            &ScopeResolution::default(),
        )
        .expect("context should compile");

        assert_eq!(context["status"], "blocked");
        assert_eq!(context["model_context"], Value::Null);
        assert!(
            context["minimality"]["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|code| code == "context_byte_budget_exceeded")
        );
        assert_eq!(context["minimality"]["budget"]["max_bytes"], 1);
        assert!(context["entries"].as_array().expect("entries").is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn full_card_fallback_blocks_and_hides_model_context() {
        let root = temp_pack("minimality-full-card");
        set_context_budget(&root, "outbound-copy-brief", 100, 1_000_000);
        let card_path = root.join(".mdp/cards/personas.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("card should load");
        let mut card: serde_yaml::Value = serde_yaml::from_str(&raw).expect("card should parse");
        for entry in card["entries"].as_sequence_mut().expect("entries") {
            entry["title"] = serde_yaml::Value::String("Unrelated audience".into());
            entry["body"] = serde_yaml::Value::String("Unrelated audience context".into());
            entry["applies_to"] =
                serde_yaml::from_str("- GTM Engineering\n").expect("applicability should parse");
        }
        std::fs::write(
            &card_path,
            serde_yaml::to_string(&card).expect("card should serialize"),
        )
        .expect("card should write");
        let manifest = read_manifest(&root).expect("manifest should load");

        let context = entry_context_scoped(
            &root,
            &manifest,
            "PMM",
            "outbound-copy-brief",
            true,
            &ScopeResolution::default(),
        )
        .expect("context should compile");

        assert_eq!(context["status"], "blocked");
        assert_eq!(context["model_context"], Value::Null);
        assert!(
            context["minimality"]["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|code| code == "full_card_fallback_required")
        );
        assert!(context["entries"].as_array().expect("entries").is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn route_and_context_share_the_same_minimality_digest() {
        let root = temp_pack("minimality-parity");
        narrow_route_candidates_for_exact_cap(&root);
        set_context_budget(&root, "outbound-copy-brief", 100, 1_000_000);
        let manifest = read_manifest(&root).expect("manifest should load");
        let scope = ScopeResolution::default();

        let route = entry_route_scoped(&root, &manifest, "PMM", "outbound-copy-brief", &scope)
            .expect("route should compile");
        let context =
            entry_context_scoped(&root, &manifest, "PMM", "outbound-copy-brief", true, &scope)
                .expect("context should compile");

        assert_eq!(route["minimality"]["status"], "ready");
        assert_eq!(
            route["minimality"]["context_sha256"],
            context["minimality"]["context_sha256"]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn declared_persona_selector_routes_case_insensitively_and_preserves_authored_value() {
        let root = temp_pack("declared-persona-selector");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest_value: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest_value["personas"]
            .as_sequence_mut()
            .expect("manifest personas")
            .push(serde_yaml::Value::String("Buyer".into()));
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest_value).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let card_path = root.join(".mdp/cards/personas.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("card should be readable");
        let mut card: serde_yaml::Value = serde_yaml::from_str(&raw).expect("card should parse");
        card["entries"][0]["title"] = serde_yaml::Value::String("Unrelated audience".into());
        card["entries"][0]["body"] =
            serde_yaml::Value::String("No persona words appear in this prose.".into());
        card["entries"][0]["applies_to"] =
            serde_yaml::from_str("- buyer\n").expect("applicability should parse");
        std::fs::write(
            &card_path,
            serde_yaml::to_string(&card).expect("card should serialize"),
        )
        .expect("card should be writable");

        let manifest = read_manifest(&root).expect("manifest should load");
        let details = route_entry_details(
            &root,
            &manifest,
            "BUYER",
            "outbound-copy-brief",
            true,
            &ScopeResolution::default(),
        )
        .expect("route details should compile");
        let selected = details
            .context_entries
            .iter()
            .find(|entry| entry["card_id"] == "personas" && entry["entry_id"] == "gtm-engineering")
            .expect("declared selector should remain reachable");
        assert_eq!(selected["reason_codes"], json!(["persona_applicability"]));
        assert_eq!(selected["applies_to"], json!(["buyer"]));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn select_cards_respects_route_card_limit_after_base_cards() {
        let selected = select_cards(&manifest(2), Some("PMM"), Some("linkedin outbound copy"));
        let ids: Vec<&str> = selected
            .iter()
            .filter_map(|card| card["id"].as_str())
            .collect();
        assert_eq!(ids, vec!["personas", "avoid-rules", "output-rules"]);
    }

    #[test]
    fn route_card_cap_blocks_applicable_authority_displaced_by_supplemental_base_card() {
        let root = temp_pack("route-card-cap-pressure");
        let scope = ScopeResolution::default();
        narrow_route_candidates_for_exact_cap(&root);
        let manifest = read_manifest(&root).expect("manifest should load");

        let exact_cap_route =
            entry_route_scoped(&root, &manifest, "PMM", "outbound-copy-brief", &scope)
                .expect("exact-cap route should compile");
        assert_eq!(exact_cap_route["status"], "ready", "{exact_cap_route}");
        assert_eq!(exact_cap_route["route_card_cap"]["status"], "ready");
        assert_eq!(exact_cap_route["route_card_cap"]["max_cards_per_route"], 13);
        assert_eq!(
            exact_cap_route["route_card_cap"]["selected_cards"]
                .as_array()
                .expect("selected cards")
                .len(),
            13
        );
        assert!(
            exact_cap_route["route_card_cap"]["selected_cards"]
                .as_array()
                .expect("selected cards")
                .iter()
                .any(|card| card["id"] == "motions" && card["kind"] == "motions")
        );
        assert_eq!(
            exact_cap_route["route_card_cap"]["excluded_cards"],
            json!([])
        );

        add_supplemental_persona_card(&root);
        let manifest = read_manifest(&root).expect("manifest should reload");
        let displaced_route =
            entry_route_scoped(&root, &manifest, "PMM", "outbound-copy-brief", &scope)
                .expect("cap-pressure route should compile");

        assert_eq!(displaced_route["status"], "blocked");
        assert_eq!(displaced_route["minimality"]["status"], "blocked");
        assert_eq!(
            displaced_route["minimality"]["diagnostics"],
            json!([ROUTE_CARD_CAP_DIAGNOSTIC])
        );
        let cap_receipt = &displaced_route["route_card_cap"];
        assert_eq!(cap_receipt["status"], "blocked");
        assert_eq!(cap_receipt["max_cards_per_route"], 13);
        assert_eq!(cap_receipt["selected_cards"].as_array().unwrap().len(), 13);
        assert!(
            cap_receipt["selected_cards"]
                .as_array()
                .unwrap()
                .iter()
                .any(|card| card["id"] == "supplemental-personas" && card["kind"] == "personas")
        );
        assert!(
            !cap_receipt["selected_cards"]
                .as_array()
                .unwrap()
                .iter()
                .any(|card| card["id"] == "motions")
        );
        assert_eq!(
            cap_receipt["excluded_cards"],
            json!([{
                "id": "motions",
                "kind": "motions",
                "reason": "max_cards_per_route_reached"
            }])
        );
        assert_eq!(
            cap_receipt["diagnostics"],
            json!([ROUTE_CARD_CAP_DIAGNOSTIC])
        );
        assert!(!cap_receipt.to_string().contains("body"));

        let route_command =
            crate::commands::routing::route(&root, "PMM", "outbound-copy-brief", false, false)
                .expect("route command should compile");
        assert_eq!(route_command["draft_status"], "blocked");
        assert_eq!(route_command["route_card_cap"], cap_receipt.clone());

        let brief_command =
            crate::commands::briefs::emit_brief(&root, "PMM", None, Some("outbound-copy-brief"))
                .expect("brief command should compile");
        assert_eq!(brief_command["draft_status"], "blocked");
        assert_eq!(brief_command["route_card_cap"], cap_receipt.clone());
        assert_eq!(
            brief_command["context"]["route_card_cap"],
            cap_receipt.clone()
        );
        assert!(
            brief_command["context"]["entries"]
                .as_array()
                .expect("brief context entries")
                .is_empty()
        );

        let context =
            entry_context_scoped(&root, &manifest, "PMM", "outbound-copy-brief", true, &scope)
                .expect("cap-pressure context should compile");
        assert_eq!(context["status"], "blocked");
        assert_eq!(
            context["reason"],
            "route card cap excluded applicable authority"
        );
        assert_eq!(context["route_card_cap"], cap_receipt.clone());
        assert!(context["entries"].as_array().expect("entries").is_empty());

        let preflight = route_budget_preflight(&root, &manifest).expect("preflight should compile");

        assert_eq!(preflight["valid"], false);
        assert!(
            preflight["route_card_cap_exclusion_count"]
                .as_u64()
                .unwrap()
                > 0
        );
        let preflight_route = preflight["routes"]
            .as_array()
            .expect("routes")
            .iter()
            .find(|route| route["persona"] == "PMM" && route["job"] == "outbound-copy-brief")
            .expect("PMM outbound-copy-brief route should be present");
        assert_eq!(preflight_route["status"], "blocked");
        assert!(
            preflight_route["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|diagnostic| diagnostic == ROUTE_CARD_CAP_DIAGNOSTIC)
        );
        assert_eq!(preflight_route["route_card_cap"], cap_receipt.clone());

        let strict_preflight =
            crate::commands::routing::route_budget_preflight_command(&root, true)
                .expect("strict preflight should compile");
        assert_eq!(strict_preflight["valid"], false);
        assert_eq!(strict_preflight["strict"]["enabled"], true);
        assert_eq!(
            strict_preflight["routes"]
                .as_array()
                .expect("strict routes")
                .iter()
                .find(|route| {
                    route["persona"] == "PMM" && route["job"] == "outbound-copy-brief"
                })
                .expect("strict PMM outbound-copy-brief route")["route_card_cap"],
            cap_receipt.clone()
        );

        let mut no_budget_manifest = read_manifest(&root).expect("manifest should reload");
        no_budget_manifest
            .jobs
            .iter_mut()
            .find(|job| job.id == "outbound-copy-brief")
            .expect("outbound-copy-brief job should be present")
            .context_budget = None;
        let no_budget_preflight = route_budget_preflight(&root, &no_budget_manifest)
            .expect("no-budget preflight should compile");
        assert_eq!(no_budget_preflight["valid"], false);
        assert!(
            no_budget_preflight["route_card_cap_exclusion_count"]
                .as_u64()
                .unwrap()
                > 0
        );
        let no_budget_route = no_budget_preflight["routes"]
            .as_array()
            .expect("routes")
            .iter()
            .find(|route| route["persona"] == "PMM" && route["job"] == "outbound-copy-brief")
            .expect("no-budget PMM outbound-copy-brief route should be present");
        assert_eq!(no_budget_route["status"], "blocked");
        assert_eq!(no_budget_route["budget"], Value::Null);
        assert_eq!(no_budget_route["route_card_cap"], cap_receipt.clone());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn starter_route_preserves_13_14_15_card_pressure_contract() {
        let root = temp_pack("starter-pressure");
        add_supplemental_persona_card(&root);
        add_second_supplemental_persona_card(&root);
        for file_name in ["supplemental-personas.yaml", "supplemental-personas-2.yaml"] {
            let path = root.join(".mdp/cards").join(file_name);
            let raw = std::fs::read_to_string(&path).expect("supplemental card should load");
            let mut card: serde_yaml::Value = serde_yaml::from_str(&raw).expect("card parses");
            card["entries"][0]["applies_to"] =
                serde_yaml::from_str("- PMM\n").expect("persona selector should parse");
            std::fs::write(
                path,
                serde_yaml::to_string(&card).expect("card should serialize"),
            )
            .expect("supplemental card should write");
        }
        let scope = ScopeResolution::default();
        let mut statuses = Vec::new();
        for max_cards in [13, 14, 15] {
            set_max_cards_per_route(&root, max_cards);
            let manifest = read_manifest(&root).expect("manifest should load");
            let route = entry_route_scoped(&root, &manifest, "PMM", "outbound-copy-brief", &scope)
                .expect("starter route should compile");
            statuses.push((
                max_cards,
                route["status"].clone(),
                route["route_card_cap"]["status"].clone(),
                route["route_card_cap"]["selected_cards"]
                    .as_array()
                    .map(Vec::len),
            ));
        }
        assert_eq!(statuses[0].1, "blocked");
        assert_eq!(statuses[0].2, "blocked");
        assert_eq!(statuses[1].1, "blocked");
        assert_eq!(statuses[1].2, "blocked");
        assert_eq!(statuses[2].1, "ready");
        assert_eq!(statuses[2].2, "ready");
        assert_eq!(statuses[2].3, Some(15));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn lifecycle_gate_defaults_generic_message_jobs_to_initial_entries() {
        let generic_job = tokens("linkedin outbound copy");
        let initial_entry = tokens("LinkedIn initial touch");
        let follow_up_entry = tokens("LinkedIn follow up");

        assert!(lifecycle_compatible(&generic_job, &initial_entry));
        assert!(!lifecycle_compatible(&generic_job, &follow_up_entry));
    }

    #[test]
    fn lifecycle_gate_separates_initial_and_follow_up_entries() {
        let initial_job = tokens("initial email outbound message");
        let follow_up_job = tokens("email follow up message");
        let initial_entry = tokens("Email initial touch");
        let follow_up_entry = tokens("Email follow up");

        assert!(lifecycle_compatible(&initial_job, &initial_entry));
        assert!(!lifecycle_compatible(&initial_job, &follow_up_entry));
        assert!(!lifecycle_compatible(&follow_up_job, &initial_entry));
        assert!(lifecycle_compatible(&follow_up_job, &follow_up_entry));
    }

    #[test]
    fn channel_gate_excludes_wrong_channel_policy_entries() {
        let email_job = tokens("initial email outbound message");
        let linkedin_job = tokens("linkedin follow up message");
        let email_entry = tokens("Email initial touch");
        let linkedin_entry = tokens("LinkedIn follow up");

        let supported_channels = ["linkedin".to_string(), "email".to_string()];

        assert!(channel_compatible(
            &supported_channels,
            &email_job,
            &email_entry
        ));
        assert!(!channel_compatible(
            &supported_channels,
            &email_job,
            &linkedin_entry
        ));
        assert!(channel_compatible(
            &supported_channels,
            &linkedin_job,
            &linkedin_entry
        ));
        assert!(!channel_compatible(
            &supported_channels,
            &linkedin_job,
            &email_entry
        ));
    }

    #[test]
    fn channel_gate_uses_manifest_supported_custom_channels() {
        let supported_channels = ["partner-intro".to_string(), "email".to_string()];
        let job = tokens("partner intro outbound message");
        let partner_entry = tokens("Partner intro");
        let email_entry = tokens("Initial email");

        assert!(channel_compatible(
            &supported_channels,
            &job,
            &partner_entry
        ));
        assert!(!channel_compatible(&supported_channels, &job, &email_entry));
    }

    #[test]
    fn entry_outputs_preserve_advisory_metadata() {
        let entry = Entry {
            id: "custom".to_string(),
            title: "Custom annotation".to_string(),
            body: "Use this entry for custom context.".to_string(),
            applies_to: vec!["PMM".to_string()],
            scope: std::collections::BTreeMap::new(),
            evidence: vec![],
            avoid: vec![],
            exact_paragraphs: None,
            constraints: Default::default(),
            metadata: [(
                "segment_hint".to_string(),
                Value::String("enterprise".to_string()),
            )]
            .into_iter()
            .collect(),
        };

        let value = entry_context_value(
            "hooks",
            &CardKind::Hooks,
            ".mdp/cards/hooks.yaml",
            &entry,
            "matched",
            "entry job match",
        );

        assert_eq!(value["metadata"]["segment_hint"], "enterprise");
    }

    #[test]
    fn selected_foundation_gap_blocks_route_and_drafting_context() {
        let root = temp_pack("foundation-gap");
        add_selected_foundation_gap(&root);
        let manifest = read_manifest(&root).expect("manifest should load");
        let scope = ScopeResolution::default();

        let route = entry_route_scoped(&root, &manifest, "PMM", "prospect-fit-or-brief", &scope)
            .expect("entry route should resolve");
        let context = entry_context_scoped(
            &root,
            &manifest,
            "PMM",
            "prospect-fit-or-brief",
            true,
            &scope,
        )
        .expect("entry context should resolve");

        for output in [&route, &context] {
            assert_eq!(output["status"], "blocked");
            assert_eq!(output["product_foundation"]["status"], "blocked");
            assert!(
                output["product_foundation"]["diagnostics"]
                    .as_array()
                    .expect("foundation diagnostics")
                    .iter()
                    .any(|diagnostic| diagnostic["code"]
                        == "product_foundation_selected_facet_has_gaps")
            );
            assert!(
                output["product_foundation_load_order"]
                    .as_array()
                    .expect("foundation load order")
                    .iter()
                    .any(|reference| {
                        reference["reference_kind"] == "gap"
                            && reference["card_id"] == "gaps"
                            && reference["entry_id"] == "missing-company-proof"
                    })
            );
        }
        assert_eq!(
            context["reason"],
            "selected product foundation authority is blocked"
        );
        assert!(
            context["entries"]
                .as_array()
                .expect("context entries")
                .is_empty()
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_foundation_blocks_route_and_drafting_context() {
        let root = temp_pack("invalid-foundation");
        set_foundation_facet_kind(&root, "unknown-foundation-kind");
        let manifest = read_manifest(&root).expect("manifest should load");
        let scope = ScopeResolution::default();

        let route = entry_route_scoped(&root, &manifest, "PMM", "prospect-fit-or-brief", &scope)
            .expect("entry route should resolve");
        let context = entry_context_scoped(
            &root,
            &manifest,
            "PMM",
            "prospect-fit-or-brief",
            true,
            &scope,
        )
        .expect("entry context should resolve");

        for output in [&route, &context] {
            assert_eq!(output["status"], "blocked");
            assert_eq!(output["product_foundation"]["status"], "blocked");
            assert!(
                output["product_foundation"]["diagnostics"]
                    .as_array()
                    .expect("foundation diagnostics")
                    .iter()
                    .any(|diagnostic| diagnostic["code"]
                        == "product_foundation_facet_kind_unknown")
            );
        }
        assert_eq!(context["reason"], "pack validation failed for this job");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unrelated_job_foundation_validation_does_not_block_selected_job() {
        let root = temp_pack("unrelated-invalid-foundation");
        narrow_route_candidates_for_exact_cap(&root);
        set_unrelated_job_condition_fact(&root, "unknown-fact");
        let manifest = read_manifest(&root).expect("manifest should load");

        let route = entry_route_scoped(
            &root,
            &manifest,
            "PMM",
            "prospect-fit-or-brief",
            &ScopeResolution::default(),
        )
        .expect("entry route should resolve");

        assert_eq!(route["status"], "ready");
        assert_eq!(route["product_foundation"]["status"], "ready");
        assert!(
            route["product_foundation"]["diagnostics"]
                .as_array()
                .expect("foundation diagnostics")
                .is_empty()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn non_foundation_validation_error_has_specific_context_reason() {
        let root = temp_pack("non-foundation-validation");
        duplicate_job_id(&root);
        let manifest = read_manifest(&root).expect("manifest should load");

        let context = entry_context_scoped(
            &root,
            &manifest,
            "PMM",
            "prospect-fit-or-brief",
            true,
            &ScopeResolution::default(),
        )
        .expect("entry context should resolve");

        assert_eq!(context["status"], "blocked");
        assert_eq!(context["reason"], "pack validation failed for this job");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ambiguous_foundation_entry_blocks_route_instead_of_selecting_last_duplicate() {
        let root = temp_pack("ambiguous-foundation-entry");
        duplicate_foundation_entry(&root);
        let manifest = read_manifest(&root).expect("manifest should load");
        let route = entry_route_scoped(
            &root,
            &manifest,
            "PMM",
            "prospect-fit-or-brief",
            &ScopeResolution::default(),
        )
        .expect("entry route should resolve");

        assert_eq!(route["status"], "blocked");
        assert_eq!(route["product_foundation"]["status"], "blocked");
        assert!(
            route["product_foundation"]["diagnostics"]
                .as_array()
                .expect("foundation diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["code"] == "product_foundation_entry_ambiguous")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn computed_activation_veto_blocks_route_and_context() {
        let root = temp_pack("computed-activation-veto");
        remove_required_eval_category(&root);
        let manifest = read_manifest(&root).expect("manifest should load");
        let scope = ScopeResolution::default();

        let route = entry_route_scoped(&root, &manifest, "PMM", "prospect-fit-or-brief", &scope)
            .expect("route should resolve");
        let context = entry_context_scoped(
            &root,
            &manifest,
            "PMM",
            "prospect-fit-or-brief",
            true,
            &scope,
        )
        .expect("context should resolve");

        assert_eq!(route["status"], "blocked");
        assert_eq!(route["profile_activation"]["status"], "blocked");
        assert_eq!(context["status"], "blocked");
        assert_eq!(context["reason"], "computed profile activation is blocked");
        assert_eq!(context["profile_activation"]["status"], "blocked");
        assert_eq!(context["entries"], json!([]));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn needs_review_profile_blocks_route_and_drafting_context() {
        let root = temp_pack("needs-review");
        set_profile_activation(&root, "needs-review");
        let manifest = read_manifest(&root).expect("manifest should load");
        let scope = ScopeResolution::default();

        let route = entry_route_scoped(&root, &manifest, "PMM", "prospect-fit-or-brief", &scope)
            .expect("entry route should resolve");
        let context = entry_context_scoped(
            &root,
            &manifest,
            "PMM",
            "prospect-fit-or-brief",
            true,
            &scope,
        )
        .expect("entry context should resolve");

        assert_eq!(route["status"], "blocked");
        assert_eq!(context["status"], "blocked");
        assert_eq!(context["product_foundation"]["status"], "ready");
        assert_eq!(
            context["reason"],
            "profile activation requires review or is blocked"
        );
        assert!(
            !context["product_foundation_load_order"]
                .as_array()
                .expect("foundation load order")
                .is_empty()
        );
        assert!(
            context["entries"]
                .as_array()
                .expect("context entries")
                .is_empty()
        );

        let _ = std::fs::remove_dir_all(root);
    }

    fn add_buyer_persona_and_case_studies(root: &Path, count: usize) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["personas"]
            .as_sequence_mut()
            .expect("personas")
            .push(serde_yaml::Value::String("Buyer".to_string()));
        manifest["target_personas"]
            .as_sequence_mut()
            .expect("target personas")
            .push(serde_yaml::Value::String("Buyer".to_string()));
        manifest["cards"].as_sequence_mut().expect("cards").push(
            serde_yaml::to_value(crate::models::CardRef {
                id: "buyer-case-studies".to_string(),
                path: "cards/buyer-case-studies.yaml".to_string(),
                kind: CardKind::Claims,
                description: "Synthetic Buyer case studies for route-budget preflight.".to_string(),
                personas: vec!["Buyer".to_string()],
                tags: vec!["buyer".to_string(), "route-budget".to_string()],
            })
            .expect("card ref should serialize"),
        );
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let card_path = root.join(".mdp/cards/buyer-case-studies.yaml");
        let mut lines = vec![
            "id: buyer-case-studies".to_string(),
            "kind: claims".to_string(),
            "title: Synthetic Buyer case studies".to_string(),
            "description: Synthetic Buyer case studies for route-budget preflight.".to_string(),
            "personas:".to_string(),
            "- Buyer".to_string(),
            "tags:".to_string(),
            "- buyer".to_string(),
            "- route-budget".to_string(),
            "entries:".to_string(),
        ];
        for index in 1..=count {
            lines.push(format!("- id: buyer-case-{index:03}"));
            lines.push(format!("  title: Buyer case study {index}"));
            let body = format!(
                "Buyer context note {index}: a synthetic persona-scoped entry used only to exercise route-budget preflight without asserting any real customer outcome, certification, compliance status, or past performance."
            );
            lines.push(format!(
                "  body: {}",
                serde_yaml::to_string(&body)
                    .expect("body should serialize")
                    .trim()
            ));
            lines.push("  applies_to:".to_string());
            lines.push("  - Buyer".to_string());
            lines.push("  evidence: []".to_string());
            lines.push("  avoid: []".to_string());
        }
        std::fs::write(card_path, lines.join("\n") + "\n").expect("card should be writable");
    }

    #[test]
    fn preflight_fails_when_persona_wide_applicability_overflows_budget() {
        let root = temp_pack("route-budget-overflow");
        add_buyer_persona_and_case_studies(&root, 99);
        let manifest = read_manifest(&root).expect("manifest should load");

        let preflight = route_budget_preflight(&root, &manifest).expect("preflight should compile");

        assert_eq!(preflight["contract"], "mdp.route-budget.v0");
        assert_eq!(preflight["valid"], false);
        assert_eq!(preflight["overflow_count"], 3);
        let buyer_brief = preflight["routes"]
            .as_array()
            .expect("routes")
            .iter()
            .find(|route| route["persona"] == "Buyer" && route["job"] == "outbound-copy-brief")
            .expect("buyer outbound-copy-brief route should be present");
        assert_eq!(buyer_brief["status"], "blocked");
        assert_eq!(buyer_brief["budget"]["max_entries"], 64);
        assert_eq!(buyer_brief["budget"]["max_bytes"], 65536);
        assert!(
            buyer_brief["budget"]["actual_entries"]
                .as_u64()
                .expect("actual entries")
                > 64
        );
        assert!(
            buyer_brief["budget"]["actual_bytes"]
                .as_u64()
                .expect("actual bytes")
                > 65536
        );
        assert!(
            buyer_brief["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|value| value == "context_entry_budget_exceeded")
        );
        assert!(
            buyer_brief["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|value| value == "context_byte_budget_exceeded")
        );
        assert_eq!(buyer_brief["reason_distribution"]["persona applies"], 99);
        let largest = buyer_brief["largest_contributing_cards"]
            .as_array()
            .expect("largest contributing cards");
        assert_eq!(largest[0]["card_id"], "buyer-case-studies");
        assert_eq!(largest[0]["entry_count"], 99);
        assert!(
            largest
                .iter()
                .all(|card| card.get("body").is_none() && card.get("entries").is_none())
        );
        assert!(
            preflight["routes"]
                .as_array()
                .expect("routes")
                .iter()
                .flat_map(|route| route["largest_contributing_cards"]
                    .as_array()
                    .into_iter()
                    .flatten())
                .all(|card| card.get("body").is_none())
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn route_budget_preflight_uses_all_declared_persona_sources() {
        let root = temp_pack("route-budget-persona-sources");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["target_personas"] =
            serde_yaml::from_str("- ' Buyer '\n- buyer\n").expect("target personas");
        manifest["operator_roles"] =
            serde_yaml::from_str("- Operator\n- ' operator '\n").expect("operator roles");
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let manifest = read_manifest(&root).expect("manifest should load");
        let preflight = route_budget_preflight(&root, &manifest).expect("preflight should compile");
        let personas: BTreeSet<&str> = preflight["routes"]
            .as_array()
            .expect("routes")
            .iter()
            .filter_map(|route| route["persona"].as_str())
            .collect();
        assert!(personas.contains("PMM"));
        assert!(personas.contains(" Buyer "));
        assert!(personas.contains("Operator"));
        assert_eq!(personas.len(), 5);
        assert_eq!(preflight["route_count"], 15);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn preflight_passes_when_narrow_applicability_fits_budget() {
        let root = temp_pack("route-budget-ready");
        add_buyer_persona_and_case_studies(&root, 5);
        let manifest = read_manifest(&root).expect("manifest should load");

        let preflight = route_budget_preflight(&root, &manifest).expect("preflight should compile");

        assert_eq!(preflight["valid"], true);
        assert_eq!(preflight["overflow_count"], 0);
        let buyer_brief = preflight["routes"]
            .as_array()
            .expect("routes")
            .iter()
            .find(|route| route["persona"] == "Buyer" && route["job"] == "outbound-copy-brief")
            .expect("buyer outbound-copy-brief route should be present");
        assert_eq!(buyer_brief["status"], "ready");
        assert!(
            buyer_brief["budget"]["actual_entries"]
                .as_u64()
                .expect("actual entries")
                <= 64
        );
        assert!(
            buyer_brief["budget"]["actual_bytes"]
                .as_u64()
                .expect("actual bytes")
                <= 65536
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn preflight_reports_excluded_reason_distribution_without_bodies() {
        let root = temp_pack("route-budget-distribution");
        add_buyer_persona_and_case_studies(&root, 5);
        let manifest = read_manifest(&root).expect("manifest should load");

        let preflight = route_budget_preflight(&root, &manifest).expect("preflight should compile");
        let buyer_brief = preflight["routes"]
            .as_array()
            .expect("routes")
            .iter()
            .find(|route| route["persona"] == "Buyer" && route["job"] == "outbound-copy-brief")
            .expect("buyer outbound-copy-brief route should be present");
        let excluded_distribution = buyer_brief["excluded_reason_distribution"]
            .as_object()
            .expect("excluded reason distribution");
        assert!(!excluded_distribution.is_empty());
        assert!(
            preflight["routes"]
                .as_array()
                .expect("routes")
                .iter()
                .flat_map(|route| route["largest_contributing_cards"]
                    .as_array()
                    .into_iter()
                    .flatten())
                .all(|card| card.get("body").is_none())
        );
        assert!(
            preflight.to_string().contains("not_applicable")
                || preflight.to_string().contains("policy_incompatible")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn preflight_preserves_legacy_jobs_without_a_context_budget() {
        let root = temp_pack("route-budget-legacy");
        // The starter pack declares context budgets on all three jobs. Strip
        // one budget to model a legacy job that must remain runtime fail-closed.
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest readable");
        let mut value: serde_yaml::Value = serde_yaml::from_str(&raw).expect("manifest parses");
        for job in value["jobs"].as_sequence_mut().expect("jobs") {
            if job["id"].as_str() == Some("prospect-fit-or-brief") {
                job["context_budget"] = serde_yaml::Value::Null;
            }
        }
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&value).expect("manifest serializes"),
        )
        .expect("manifest writable");
        let manifest = read_manifest(&root).expect("manifest should load");

        let preflight = route_budget_preflight(&root, &manifest).expect("preflight should compile");

        assert_eq!(preflight["valid"], true);
        let legacy = preflight["routes"]
            .as_array()
            .expect("routes")
            .iter()
            .find(|route| route["job"] == "prospect-fit-or-brief")
            .expect("legacy route should be present");
        assert_eq!(legacy["status"], "unassessed");
        assert_eq!(legacy["budget"], Value::Null);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn minimality_reports_largest_contributing_cards_without_entry_bodies() {
        let root = temp_pack("route-budget-minimality");
        add_buyer_persona_and_case_studies(&root, 5);
        let manifest = read_manifest(&root).expect("manifest should load");

        let route = entry_route_scoped(
            &root,
            &manifest,
            "Buyer",
            "outbound-copy-brief",
            &ScopeResolution::default(),
        )
        .expect("route should compile");
        let minimality = &route["minimality"];
        let largest = minimality["largest_contributing_cards"]
            .as_array()
            .expect("largest contributing cards");
        assert!(!largest.is_empty());
        assert!(largest[0].get("card_id").is_some());
        assert!(largest[0].get("canonical_bytes").is_some());
        assert!(largest[0].get("entry_count").is_some());
        assert!(largest.iter().all(|card| card.get("body").is_none()));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn routed_context_validator_accepts_exact_emitted_bytes_and_rejects_drift() {
        let root = temp_pack("routed-context-validator");
        let manifest = read_manifest(&root).expect("manifest should load");
        let output =
            crate::commands::briefs::emit_brief(&root, "PMM", None, Some("outbound-copy-brief"))
                .expect("brief should emit");
        let context = output["context"]["model_context"].clone();
        let bytes = canonical_json_bytes(&context).expect("context should canonicalize");
        let valid =
            validate_routed_context_bytes_for_job(&root, &manifest, &bytes, "outbound-copy-brief")
                .expect("exact producer bytes should validate");
        assert_eq!(valid.sha256, sha256_hex(&bytes));

        let mut wrong_contract = context.clone();
        wrong_contract["contract"] = json!("mdp.routed-context.v0");
        let wrong_contract_bytes =
            canonical_json_bytes(&wrong_contract).expect("wrong contract should serialize");
        assert_eq!(
            validate_routed_context_bytes_for_job(
                &root,
                &manifest,
                &wrong_contract_bytes,
                "outbound-copy-brief",
            )
            .expect_err("wrong contract must fail closed")
            .kind(),
            RoutedContextValidationKind::Contract
        );

        let malformed_bytes = br#"{\"contract\":\"mdp.routed-context.v1\"}"#;
        assert_eq!(
            validate_routed_context_bytes_for_job(
                &root,
                &manifest,
                malformed_bytes,
                "outbound-copy-brief",
            )
            .expect_err("schema-invalid context must fail closed")
            .kind(),
            RoutedContextValidationKind::Schema
        );

        let mut wrong_job = context.clone();
        wrong_job["job"] = json!("prospect-fit-or-brief");
        let wrong_job_bytes = canonical_json_bytes(&wrong_job).expect("wrong job should serialize");
        assert_eq!(
            validate_routed_context_bytes_for_job(
                &root,
                &manifest,
                &wrong_job_bytes,
                "outbound-copy-brief",
            )
            .expect_err("wrong job must fail closed")
            .kind(),
            RoutedContextValidationKind::Job
        );

        let pretty_bytes = serde_json::to_vec_pretty(&context).expect("context should serialize");
        assert_eq!(
            validate_routed_context_bytes_for_job(
                &root,
                &manifest,
                &pretty_bytes,
                "outbound-copy-brief",
            )
            .expect_err("non-canonical bytes must fail closed")
            .kind(),
            RoutedContextValidationKind::Canonical
        );

        let mut changed = context;
        changed["entries"][0]["body"] = json!("synthetic changed authority");
        let changed_bytes =
            canonical_json_bytes(&changed).expect("changed context should serialize");
        assert_eq!(
            validate_routed_context_bytes_for_job(
                &root,
                &manifest,
                &changed_bytes,
                "outbound-copy-brief",
            )
            .expect_err("changed context must fail closed")
            .kind(),
            RoutedContextValidationKind::NotCompiled
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
