use crate::models::{
    Card, CardKind, Entry, EntryConstraints, Manifest, PersonaMapping, TargetIdentity,
};
use crate::starter::{starter_manifest, starter_prompts};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const PERSONAS: [&str; 2] = ["Buyer", "Operator"];

pub(crate) fn target_manifest(
    name: &str,
    slug: &str,
    template: &str,
    target: &TargetIdentity,
) -> Manifest {
    let mut manifest = starter_manifest(name, slug, template);
    manifest.description = Some(format!(
        "Evidence-gated messaging decisions for {}. Product claims, ICP detail, and proof remain gaps until sources support them.",
        target.name
    ));
    manifest.target = Some(target.clone());
    manifest.personas = strings(&PERSONAS);
    manifest.target_personas = strings(&PERSONAS);
    manifest.operator_roles = vec!["Operator".to_string()];
    manifest.persona_mappings = vec![
        PersonaMapping {
            persona: "Buyer".to_string(),
            title_keywords: vec!["buyer".to_string(), "decision maker".to_string()],
        },
        PersonaMapping {
            persona: "Operator".to_string(),
            title_keywords: vec!["operator".to_string(), "owner".to_string()],
        },
    ];
    if let Some(contract) = manifest
        .lead_input_requirements
        .value_contracts
        .get_mut("segment")
    {
        contract.enum_values = vec!["target-segment".to_string()];
        contract.description = Some(
            "Temporary neutral segment used until reviewed target evidence defines the ICP."
                .to_string(),
        );
    }
    manifest
        .lead_input_requirements
        .attribute_definitions
        .clear();
    if let Some(profile) = manifest.profile.as_mut() {
        profile.context_dimensions =
            BTreeMap::from([("segment".to_string(), vec!["target-segment".to_string()])]);
        profile.context_dimension_dependencies.clear();
    }
    manifest
        .cards
        .retain(|card| card.id != "portfolio-examples");
    for mapping in manifest.primitive_map.values_mut() {
        mapping.cards.retain(|card| card != "portfolio-examples");
        mapping.evals.retain(|eval| {
            matches!(
                eval.as_str(),
                "target-route"
                    | "fit-insufficient-context"
                    | "account-context-missing"
                    | "internal-control-plane-rejected"
            )
        });
    }
    if let Some(mapping) = manifest.primitive_map.get_mut("evals") {
        mapping.evals = vec![
            "target-route".to_string(),
            "fit-insufficient-context".to_string(),
            "account-context-missing".to_string(),
            "internal-control-plane-rejected".to_string(),
        ];
    }
    if let Some(mapping) = manifest.primitive_map.get_mut("gaps") {
        mapping.evals = vec![
            "fit-insufficient-context".to_string(),
            "account-context-missing".to_string(),
        ];
    }
    if let Some(mapping) = manifest.primitive_map.get_mut("source-signals") {
        mapping.evals = vec!["account-context-missing".to_string()];
    }
    for card in &mut manifest.cards {
        card.personas = strings(&PERSONAS);
        card.tags
            .retain(|tag| !matches!(tag.as_str(), "clay" | "deepline"));
    }
    for job in &mut manifest.jobs {
        job.description = Some(match job.id.as_str() {
            "create-or-improve-gtm-pack" => format!(
                "Author reviewed messaging decisions for {} without inheriting starter or prior-target language.",
                target.name
            ),
            "prospect-fit-or-brief" => format!(
                "Normalize supplied prospect context and check fit against reviewed {} evidence.",
                target.name
            ),
            "outbound-copy-brief" => format!(
                "Produce grounded {} copy guidance only after fit, proof, and boundaries are supported.",
                target.name
            ),
            _ => {
                "Check structural validity, target isolation, gaps, and eval coverage.".to_string()
            }
        });
    }
    manifest.profile_eval.required_categories = vec![
        "insufficient-context".to_string(),
        "unsafe-output".to_string(),
        "job-routing".to_string(),
        "account-context-missing".to_string(),
    ];
    manifest.profile_eval.activation.status = Some("needs-review".to_string());
    manifest.profile_eval.activation.summary = Some(format!(
        "Target identity for {} is resolved; commercial claims and ICP detail remain evidence gaps.",
        target.name
    ));
    manifest.provenance.notes = vec![
        format!("External positioning target: {}.", target.name),
        "Internal pack, CLI, schema, prompt, card, and eval terms are authoring vocabulary only."
            .to_string(),
        "Agents must convert unsupported target claims into gaps instead of starter filler."
            .to_string(),
    ];
    manifest
}

pub(crate) fn target_cards(target: &TargetIdentity) -> Vec<(&'static str, Card)> {
    let target_name = target.name.as_str();
    let mut cards = vec![
        card(
            "personas.yaml",
            "personas",
            CardKind::Personas,
            "Target personas",
            "Neutral persona placeholders pending reviewed target evidence.",
            "persona-evidence-gap",
            "Persona evidence required",
            &format!(
                "Buyer and operator roles for {target_name} are unresolved. Replace these neutral placeholders only when supplied sources support named personas."
            ),
        ),
        card(
            "positioning.yaml",
            "positioning",
            CardKind::Positioning,
            "Target positioning",
            &format!("External positioning boundary for {target_name}."),
            "target-identity",
            &format!("Position {target_name}"),
            &format!(
                "Prospect-facing positioning must be about {target_name}. Category, value proposition, capabilities, and differentiation remain unknown until cited sources support them."
            ),
        ),
        card(
            "fit-rules.yaml",
            "fit-rules",
            CardKind::FitRules,
            "Fit rules",
            "Evidence-gated fit and no-message policy.",
            "no-evidence-no-fit",
            "No fit decision without evidence",
            &format!(
                "Do not infer fit for {target_name} from a generic title, industry, or tool mention. Return insufficient context until the pack contains reviewed segment, persona, trigger, and disqualifier evidence."
            ),
        ),
        card(
            "signals.yaml",
            "signals",
            CardKind::Signals,
            "Signals",
            "Source and confidence rules for target-specific evidence.",
            "source-backed-signals",
            "Require source-backed signals",
            &format!(
                "Signals used for {target_name} fit or messaging must retain source, confidence, and freshness. Weak observations stay hypotheses."
            ),
        ),
        card(
            "pains.yaml",
            "pains",
            CardKind::Pains,
            "Pains",
            "Buyer pain candidates supported by target evidence.",
            "pain-evidence-gap",
            "Pain evidence required",
            &format!(
                "No buyer pain is approved for {target_name} yet. Add pains only when a cited source supports the problem and affected persona."
            ),
        ),
        card(
            "claims.yaml",
            "claims",
            CardKind::Claims,
            "Claims",
            "Approved target claims and proof requirements.",
            "no-approved-claims",
            "No approved claims yet",
            &format!(
                "No capability, outcome, customer, integration, pricing, security, or performance claim is approved for {target_name} until a source is added and reviewed."
            ),
        ),
        card(
            "motions.yaml",
            "motions",
            CardKind::Motions,
            "Motions",
            "Allowed work before target evidence is complete.",
            "research-before-outreach",
            "Research before outreach",
            &format!(
                "For {target_name}, gather and review source evidence before creating an outbound brief. A resolved target name alone does not authorize a message."
            ),
        ),
        card(
            "channel-policies.yaml",
            "channel-policies",
            CardKind::ChannelPolicies,
            "Channel policies",
            "Neutral channel boundary pending target evidence.",
            "evidence-before-channel-copy",
            "Evidence before channel copy",
            &format!(
                "Do not draft email, social, call-prep, or agent-brief content for {target_name} until fit, claims, and channel rules are supported."
            ),
        ),
        card(
            "hooks.yaml",
            "hooks",
            CardKind::Hooks,
            "Hooks",
            "Target-aware hooks supported by evidence.",
            "hook-evidence-gap",
            "Hook evidence required",
            &format!(
                "No hook is approved for {target_name} yet. Do not reuse a starter angle or a prior target's category language."
            ),
        ),
        card(
            "ctas.yaml",
            "ctas",
            CardKind::Ctas,
            "CTA rules",
            "Ask and reply-path boundaries.",
            "low-friction-after-fit",
            "Low-friction ask after fit",
            &format!(
                "After {target_name} fit and evidence are reviewed, prefer one low-friction reply-path question. Do not manufacture urgency or default to a calendar request."
            ),
        ),
        card(
            "avoid-rules.yaml",
            "avoid-rules",
            CardKind::AvoidRules,
            "Avoid rules",
            "Target isolation and unsupported-claim boundaries.",
            "external-target-only",
            "Keep authoring vocabulary internal",
            &format!(
                "Prospect-facing content must position {target_name}. Do not position the pack tooling, CLI, schemas, prompts, cards, eval machinery, a starter example, or a prior target as the product being sold."
            ),
        ),
        card(
            "output-rules.yaml",
            "output-rules",
            CardKind::OutputRules,
            "Output rules",
            "Global evidence and target-isolation rules.",
            "no-filler",
            "No inherited filler",
            &format!(
                "Generated {target_name} content must use supported target language or state an explicit gap. Never smooth missing evidence with demo copy, generic outcomes, or old personas."
            ),
        ),
        card(
            "copy-patterns.yaml",
            "copy-patterns",
            CardKind::CopyPatterns,
            "Copy patterns",
            "Neutral structure for later evidence-backed copy.",
            "evidence-gap-pattern",
            "Evidence then gap",
            &format!(
                "When {target_name} evidence is incomplete, return the observed source fact, the unresolved gap, and the next evidence needed. Do not draft persuasive copy."
            ),
        ),
        card(
            "objections.yaml",
            "objections",
            CardKind::Objections,
            "Objections",
            "Target-specific objections supported by evidence.",
            "objection-evidence-gap",
            "Objection evidence required",
            &format!(
                "No objection response is approved for {target_name} yet. Add alternatives and response logic only from supplied or cited sources."
            ),
        ),
        card(
            "gaps.yaml",
            "gaps",
            CardKind::Gaps,
            "Known gaps",
            "Durable target evidence gaps.",
            "target-foundation-gaps",
            "Target foundation evidence missing",
            &format!(
                "Collect reviewed sources for {target_name} category, product boundaries, ICP, personas, pains, proof, objections, channel policy, hooks, and CTA rules before activation."
            ),
        ),
    ];
    if let Some((_, avoid_rules)) = cards.iter_mut().find(|(_, card)| card.id == "avoid-rules") {
        avoid_rules.entries[0].avoid = target.internal_terms.clone();
    }
    cards
}

pub(crate) fn target_source_ledger(target: &TargetIdentity) -> Value {
    json!({
        "format": "mdp.sources.v0",
        "purpose": format!("Source ledger for {}. Target identity is known; product claims remain unsupported until evidence is added.", target.name),
        "rules": [
            "Add public URLs, user-provided documents, or note identifiers before authoring claims.",
            "Keep direct source claims separate from interpretations.",
            "Represent missing proof as a gap, never inherited filler."
        ],
        "sources": [{
            "id": "target-identity",
            "kind": "operator-intent",
            "locator": "user-supplied target identity",
            "freshness": "current-run",
            "confidence": "high",
            "direct_claims": [format!("The external {} target is {}.", target.kind, target.name)],
            "interpretations": [format!("Use {} as the subject of external positioning; this statement does not prove product capabilities or outcomes.", target.name)],
            "gaps": ["Category, capabilities, ICP, personas, pains, proof, alternatives, and channel policy require reviewed sources."]
        }]
    })
}

pub(crate) fn target_prospect(target: &TargetIdentity) -> Value {
    json!({
        "name": "Example Person",
        "title": "Example Operator",
        "company": "Example Prospect Company",
        "source_kind": "synthetic-example",
        "synthetic": true,
        "background": format!("Neutral synthetic row for testing {} pack wiring; not evidence about a real account.", target.name),
        "persona": "Operator",
        "segment": "target-segment",
        "signals": [],
        "attributes": {}
    })
}

pub(crate) fn target_prompts(
    target: &TargetIdentity,
    include_output_schemas: bool,
) -> Vec<(&'static str, Value)> {
    starter_prompts(include_output_schemas)
        .into_iter()
        .map(|(name, mut prompt)| {
            remove_starter_identifiers(&mut prompt);
            neutralize_prompt_example(&mut prompt, target);
            if let Some(instructions) = prompt
                .get_mut("instructions")
                .and_then(Value::as_array_mut)
            {
                instructions.insert(
                    0,
                    json!(format!(
                        "External target identity is {} ({}). Prospect-facing candidates must position this target, not the pack, CLI, schema, prompt, card, eval, starter, or prior target.",
                        target.name, target.kind
                    )),
                );
                instructions.insert(
                    1,
                    json!("When supplied sources do not support a target-specific candidate, emit a gap instead of inheriting the example or inventing a claim."),
                );
            }
            (name, prompt)
        })
        .collect()
}

pub(crate) fn target_evals(target: &TargetIdentity) -> Vec<(&'static str, Value)> {
    vec![
        (
            "target-route.yaml",
            json!({
                "id": "target-route",
                "command": "route",
                "profile_eval": {"category": "job-routing", "primitives": ["actors", "boundaries", "output-contracts"], "jobs": ["create-or-improve-gtm-pack"]},
                "persona": "Operator",
                "job": format!("create or improve messaging for {}", target.name),
                "expect_load_order_contains": [".mdp/cards/personas.yaml", ".mdp/cards/positioning.yaml"]
            }),
        ),
        (
            "fit-insufficient-context.yaml",
            json!({
                "id": "fit-insufficient-context",
                "command": "fit",
                "profile_eval": {"category": "insufficient-context", "primitives": ["actors", "decision-criteria", "source-signals", "gaps"], "jobs": ["prospect-fit-or-brief"]},
                "prospect": target_prospect(target),
                "expect_status": "insufficient-context"
            }),
        ),
        (
            "account-context-missing.yaml",
            json!({
                "id": "account-context-missing",
                "command": "fit",
                "profile_eval": {"category": "account-context-missing", "primitives": ["source-signals", "gaps"], "jobs": ["prospect-fit-or-brief"]},
                "prospect": {
                    "name": "Example Person",
                    "title": "Example Operator",
                    "company": "Example Prospect Company",
                    "persona": "Operator",
                    "segment": "target-segment",
                    "source_kind": "synthetic-example",
                    "synthetic": true,
                    "signals": [{"id": "unproven", "title": "Unproven signal"}]
                },
                "expect_status": "insufficient-context"
            }),
        ),
        (
            "internal-control-plane-rejected.yaml",
            json!({
                "id": "internal-control-plane-rejected",
                "command": "check-claims",
                "profile_eval": {"category": "unsafe-output", "primitives": ["boundaries", "output-contracts"], "jobs": ["outbound-copy-brief"]},
                "text": "The Message Decision Pack is a local offline decision layer that improves agent handoffs.",
                "expect_valid": false,
                "expect_guardrail_terms_contains": ["Message Decision Pack", "local offline decision layer", "agent handoffs"]
            }),
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn card(
    filename: &'static str,
    id: &str,
    kind: CardKind,
    title: &str,
    description: &str,
    entry_id: &str,
    entry_title: &str,
    body: &str,
) -> (&'static str, Card) {
    (
        filename,
        Card {
            id: id.to_string(),
            kind,
            title: title.to_string(),
            description: description.to_string(),
            personas: strings(&PERSONAS),
            tags: vec!["target-aware".to_string(), "evidence-gated".to_string()],
            entries: vec![Entry {
                id: entry_id.to_string(),
                title: entry_title.to_string(),
                body: body.to_string(),
                applies_to: strings(&PERSONAS),
                scope: BTreeMap::new(),
                evidence: vec!["target-identity".to_string()],
                avoid: Vec::new(),
                exact_paragraphs: None,
                constraints: EntryConstraints::default(),
                metadata: BTreeMap::new(),
            }],
        },
    )
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn neutralize_prompt_example(prompt: &mut Value, target: &TargetIdentity) {
    let evidence_input = prompt
        .get("inputs")
        .and_then(Value::as_object)
        .and_then(|inputs| {
            inputs.keys().find(|key| {
                !matches!(
                    key.as_str(),
                    "existing_pack_context" | "runtime_context" | "source_kind"
                )
            })
        })
        .cloned()
        .unwrap_or_else(|| "source_notes".to_string());
    let Some(example) = prompt
        .get_mut("output_contract")
        .and_then(|contract| contract.get_mut("example"))
    else {
        return;
    };

    if let Some(card_patches) = example
        .get_mut("card_patches")
        .and_then(Value::as_array_mut)
    {
        for patch in card_patches {
            let card_id = patch["card_id"].as_str().unwrap_or("target").to_string();
            let kind = patch["kind"].as_str().unwrap_or(&card_id).to_string();
            patch["entries"] = json!([{
                "id": format!("{card_id}-evidence-gap"),
                "title": "Evidence required",
                "body": format!(
                    "No {} {} candidate is supported by supplied evidence yet. Record the missing evidence as a gap instead of drafting target-specific content.",
                    target.name, kind
                ),
                "applies_to": PERSONAS,
                "evidence": [evidence_input.clone()],
                "avoid": [],
                "confidence": "low",
                "provenance": [format!("{evidence_input}: supplied input reviewed; target-specific support remains missing")],
                "notes": [],
                "status": "needs-review"
            }]);
        }
        if let Some(source_summary) = example
            .get_mut("source_summary")
            .and_then(Value::as_object_mut)
        {
            source_summary.insert("inputs_used".to_string(), json!([evidence_input]));
            source_summary.insert("confidence".to_string(), json!("low"));
        }
    }

    if example.get("normalized_prospect").is_some() {
        example["normalized_prospect"] = target_prospect(target);
        example["source_summary"] = json!({
            "account_name": "Example Prospect Company",
            "company_domain": "N/A",
            "company_name": "Example Prospect Company",
            "confidence": "unknown",
            "inputs_used": ["raw_row"],
            "person_name": "Example Person",
            "person_title": "Example Operator"
        });
        example["normalization_trace"] = json!({
            "fit_readiness": {
                "has_company_domain": false,
                "has_persona": true,
                "has_segment": true,
                "has_signal_source": false,
                "has_signals": false,
                "has_trigger": false,
                "ready_for_mdp_fit": false,
                "ready_for_brief": false
            },
            "missing_required": [
                {"field": "company_domain", "reason": "not_available_in_source", "source_evidence": "Synthetic example intentionally omits a company domain."},
                {"field": "trigger", "reason": "not_available_in_source", "source_evidence": "Synthetic example intentionally omits a target-specific trigger."},
                {"field": "signals", "reason": "not_available_in_source", "source_evidence": "Synthetic example intentionally contains no source-backed signals."}
            ],
            "persona": {
                "confidence": "low",
                "matched_keywords": [],
                "needs_review": true,
                "source": "synthetic-example"
            },
            "preserved_raw_fields": ["raw_row.name", "raw_row.title", "raw_row.company"]
        });
    }
}

fn remove_starter_identifiers(value: &mut Value) {
    match value {
        Value::String(text) => {
            for (from, to) in [
                ("Basic MDP Template", "target messaging scaffold"),
                ("agent-assisted GTM", "target-specific operations"),
                ("local-cli", "target-product"),
                ("agent-plugin", "target-extension"),
                ("example-mdp-demo", "example-target-demo"),
            ] {
                *text = text.replace(from, to);
            }
        }
        Value::Array(values) => {
            for value in values {
                remove_starter_identifiers(value);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                remove_starter_identifiers(value);
            }
        }
        _ => {}
    }
}
