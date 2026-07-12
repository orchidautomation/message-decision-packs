use crate::constants::DEFAULT_DIR;
use crate::models::{CardKind, Entry, Manifest};
use crate::pack_io::{read_card, resolve_pack_path};
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
    let details = route_entry_details(root, manifest, persona, job, false, scope)?;
    let blocked = !details.scope_ready(scope);

    Ok(json!({
        "contract": "mdp.entry-route.v0",
        "status": if blocked { "blocked" } else { "ready" },
        "persona": persona,
        "job": job,
        "scope": scope,
        "portfolio_sensitive": details.portfolio_sensitive,
        "matches": details.matches,
        "gaps": details.gaps,
        "policy": if details.portfolio_sensitive { "Use matched bounded entries only. Shared card paths are not scope-filtered drafting context. Resolve missing or invalid scope before drafting." } else { "Load matched entries first. Treat entry metadata as advisory context, not enforced CLI constraints. Load the full card only when an entry is ambiguous, missing, or a guardrail card needs complete review." }
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
    let details = route_entry_details(root, manifest, persona, job, true, scope)?;
    let scope_blocked = !details.scope_ready(scope);
    if !draft_ready || scope_blocked {
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
            "reason": if scope_blocked { "portfolio scope is missing or invalid" } else { "draft_status no-draft" },
            "persona": persona,
            "job": job,
            "scope": scope,
            "portfolio_sensitive": details.portfolio_sensitive,
            "source_load_order": if details.portfolio_sensitive { Vec::<Value>::new() } else { load_order.clone() },
            "entries": entries,
            "gaps": details.gaps,
            "full_card_required": [],
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
        "source_load_order": if details.portfolio_sensitive { Vec::<Value>::new() } else { load_order.clone() },
        "entries": details.context_entries,
        "gaps": details.gaps,
        "full_card_required": details.full_card_required,
        "summary": {
            "card_count": load_order.len(),
            "entry_count": entry_count,
            "required_entry_count": required_entry_count,
            "supporting_entry_count": entry_count.saturating_sub(required_entry_count),
            "guardrail_entry_count": guardrail_entry_count
        },
        "policy": if details.portfolio_sensitive { "Use scope-filtered context.entries only. Shared full cards are not scope-safe drafting context. Treat entry metadata as advisory context, not enforced CLI constraints." } else { "Use context.entries first. Treat entry metadata as advisory context, not enforced CLI constraints. Open full_card_required paths only when present, or when the user asks for a full pack/card audit." }
    }))
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
            if (matched || guardrail) && !scope_match.compatible {
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
        "reason": reason
    })
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
    use crate::models::{CardRef, LeadInputRequirements, Policy, Provenance};

    fn manifest(max_cards_per_route: usize) -> Manifest {
        Manifest {
            format: "mdp.v0".to_string(),
            id: "test".to_string(),
            name: "Test".to_string(),
            version: "0.1.0".to_string(),
            description: None,
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
}
