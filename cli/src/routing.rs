use crate::artifact_hash::{canonical_json_bytes, sha256_hex};
use crate::commands::health::{profile_activation_decision, validate_pack};
use crate::constants::{DEFAULT_DIR, ROUTED_CONTEXT_CONTRACT};
use crate::models::{CardKind, Entry, Manifest};
use crate::pack_io::{read_card, resolve_pack_path};
use crate::product_foundation::{
    ProductFoundationResolution, apply_validation_errors_for_job, resolution_json,
    resolve_product_foundation_for_pack, validation_errors_block_job,
};
use crate::runtime_context::current_runtime_context;
use crate::scope::{ScopeResolution, match_entry_scope};
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

struct EntryRouteDetails {
    matches: Vec<Value>,
    context_entries: Vec<Value>,
    gaps: Vec<Value>,
    full_card_required: Vec<Value>,
    excluded: Vec<Value>,
    portfolio_sensitive: bool,
    compatible_scoped_entry_count: usize,
    scoped_decision_candidate_count: usize,
    compatible_scoped_decision_count: usize,
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

pub(crate) fn select_cards(
    manifest: &Manifest,
    persona: Option<&str>,
    job: Option<&str>,
) -> Vec<Value> {
    let persona_lower = persona.map(|p| p.to_lowercase());
    let job_tokens = tokens(job.unwrap_or(""));
    let is_message_job = is_message_job(&job_tokens);
    let mut selected = Vec::new();
    let mut candidates = Vec::new();

    for card in &manifest.cards {
        if is_base_guardrail(&card.kind) {
            selected.push(json!({"id": card.id, "kind": card.kind, "path": format!("{DEFAULT_DIR}/{}", card.path), "reason": "base guardrail", "description": card.description}));
        }
    }

    for (index, card) in manifest.cards.iter().enumerate() {
        if is_base_guardrail(&card.kind) {
            continue;
        }
        let persona_match = persona_lower
            .as_ref()
            .map(|p| {
                card.personas
                    .iter()
                    .any(|candidate| candidate.to_lowercase() == *p)
                    || card.description.to_lowercase().contains(p)
            })
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
            break;
        }
        selected.push(card);
    }
    selected
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
    apply_selection_authority(&mut details.context_entries, &product_foundation_load_order);
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
    apply_selection_authority(&mut details.context_entries, &product_foundation_load_order);
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
) -> Result<Value> {
    let Some(job) = manifest.jobs.iter().find(|job| job.id == job_id) else {
        return Ok(json!({
            "status": "unassessed",
            "context_sha256": Value::Null,
            "budget": Value::Null,
            "excluded": excluded,
            "diagnostics": ["canonical_job_not_declared"]
        }));
    };
    let Some(budget) = job.context_budget.as_ref() else {
        return Ok(json!({
            "status": "unassessed",
            "context_sha256": Value::Null,
            "budget": Value::Null,
            "excluded": excluded,
            "diagnostics": ["context_budget_not_declared"]
        }));
    };

    let selected_count = selected_authority_count(model_visible_projection);
    let canonical_context = canonical_json_bytes(model_visible_projection)?;
    let actual_bytes = canonical_context.len();
    let mut diagnostics = Vec::new();
    if !full_card_required.is_empty() {
        diagnostics.push("full_card_fallback_required");
    }
    if selected_count > budget.max_entries {
        diagnostics.push("context_entry_budget_exceeded");
    }
    if actual_bytes > budget.max_bytes {
        diagnostics.push("context_byte_budget_exceeded");
    }
    Ok(json!({
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
        "diagnostics": diagnostics
    }))
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
        } else if matches!(
            card_kind,
            Some(CardKind::Ctas | CardKind::CopyPatterns | CardKind::ChannelPolicies)
        ) {
            ("output_requirement", Some("output_requirement"))
        } else if matches!(card_kind, Some(CardKind::Claims)) {
            ("evidence_dependency", Some("evidence_dependency"))
        } else {
            ("persona_or_job_match", None)
        };
        entry["selection_class"] = json!(selection_class);
        if let Some(reason_code) = reason_code
            && let Some(reason_codes) = entry["reason_codes"].as_array_mut()
            && !reason_codes.iter().any(|value| value == reason_code)
        {
            reason_codes.push(json!(reason_code));
        }
    }
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
    let selected = select_cards(manifest, Some(persona), Some(job));
    let selected_ids: BTreeSet<String> = selected
        .iter()
        .filter_map(|value| value["id"].as_str().map(str::to_string))
        .collect();
    let persona_lower = persona.to_lowercase();
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
            let entry_text = format!(
                "{} {} {}",
                entry.title,
                entry.body,
                entry.applies_to.join(" ")
            )
            .to_lowercase();
            let applies = entry
                .applies_to
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(persona));
            let entry_tokens = tokens(&entry_text);
            let job_match = token_overlap(&job_tokens, &entry_tokens);
            let persona_match = entry_text.contains(&persona_lower);
            let entry_allowed =
                entry_policy_compatible(&card.kind, manifest, &job_tokens, &entry_tokens);
            let matched = !(matches!(card.kind, CardKind::ChannelPolicies) && !job_match)
                && entry_allowed
                && (applies || job_match || persona_match);
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
        portfolio_sensitive,
        compatible_scoped_entry_count,
        scoped_decision_candidate_count,
        compatible_scoped_decision_count,
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
    use crate::models::{CardRef, LeadInputRequirements, Policy, Provenance};
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
        root
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
        })
        .expect("budget should serialize");
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
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
    fn opted_in_context_exposes_stable_minimality_digest_and_safe_exclusions() {
        let root = temp_pack("minimality-ready");
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
    fn context_budget_overflow_blocks_without_dropping_guardrails() {
        let root = temp_pack("minimality-overflow");
        set_context_budget(&root, "outbound-copy-brief", 1, 1_000_000);
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
}
