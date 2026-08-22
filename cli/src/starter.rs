use crate::constants::{
    FORMAT_VERSION, PROMPT_CARD_PATCH_SCHEMA_REF, PROMPT_FORMAT_V1, PROMPT_FORMAT_VERSION,
    PROMPT_OUTPUT_CONTRACT, PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF,
};
use crate::models::{
    Card, CardKind, CardRef, CountConstraint, DecisionInputAttemptStatus, DecisionInputAttribute,
    DecisionInputConfidencePolicy, DecisionInputContract, DecisionInputDecisionEffect,
    DecisionInputDisposition, DecisionInputFreshnessPolicy, DecisionInputNormalization,
    DecisionInputProvenanceField, DecisionInputProvenancePolicy, DecisionInputRequirement,
    DecisionInputSensitivity, DecisionInputSignalCardinality, DecisionInputSignalConflictPolicy,
    DecisionInputSignalProjection, DecisionInputSignalRole, DecisionInputSourceClass, Entry,
    EntryConstraints, InputContract, LeadInputRequirements, Manifest, PersonaMapping, Policy,
    PrimitiveMapping, ProductFoundationBinding, ProductFoundationEntryRef, ProductFoundationFacet,
    ProductFoundationFacetKind, ProductFoundationRegistry, Profile, ProfileActivation, ProfileEval,
    ProfileJob, Provenance, ValueContract,
};
use crate::runtime_context::runtime_context_schema;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(crate) fn generated_starter_manifest(name: &str, slug: &str, _template: &str) -> Manifest {
    let personas = vec![
        "GTM Engineering".to_string(),
        "PMM".to_string(),
        "PM".to_string(),
    ];
    let value_contracts = BTreeMap::from([
        (
            "segment".to_string(),
            ValueContract {
                value_type: Some("string".to_string()),
                enum_values: vec!["agent-assisted GTM".to_string()],
                description: Some(
                    "Pack-owned segment labels accepted from normalization prompts.".to_string(),
                ),
                ..ValueContract::default()
            },
        ),
        (
            "source_kind".to_string(),
            ValueContract {
                value_type: Some("string".to_string()),
                enum_values: vec![
                    "user-provided-row".to_string(),
                    "csv-row".to_string(),
                    "crm-export-row".to_string(),
                    "clay-row".to_string(),
                    "deepline-row".to_string(),
                    "private-scratch-row".to_string(),
                    "sanitized-example".to_string(),
                    "synthetic-example".to_string(),
                ],
                description: Some(
                    "Provider-neutral source markers accepted from normalization prompts."
                        .to_string(),
                ),
                ..ValueContract::default()
            },
        ),
    ]);
    let attribute_definitions = BTreeMap::from([
        (
            "contact_policy".to_string(),
            ValueContract {
                value_type: Some("string".to_string()),
                enum_values: strings(&["clear", "do-not-contact", "needs-review"]),
                description: Some(
                    "Reviewed host-owned permission state; MDP does not collect or change it."
                        .to_string(),
                ),
                ..ValueContract::default()
            },
        ),
        (
            "fiscal_year".to_string(),
            ValueContract {
                value_type: Some("string".to_string()),
                description: Some(
                    "Optional reviewed account metadata. Keep proof in signals, not attributes."
                        .to_string(),
                ),
                ..ValueContract::default()
            },
        ),
        (
            "product".to_string(),
            ValueContract {
                value_type: Some("string".to_string()),
                enum_values: strings(&["local-cli", "agent-plugin"]),
                description: Some(
                    "Optional pack-owned product context used for entry scope routing.".to_string(),
                ),
                ..ValueContract::default()
            },
        ),
        (
            "capability".to_string(),
            ValueContract {
                value_type: Some("string".to_string()),
                enum_values: strings(&["portfolio-routing", "bounded-context"]),
                description: Some(
                    "Optional capability context; select product with capability.".to_string(),
                ),
                ..ValueContract::default()
            },
        ),
        (
            "solution".to_string(),
            ValueContract {
                value_type: Some("string".to_string()),
                enum_values: strings(&["gtm-messaging", "agent-enablement"]),
                description: Some(
                    "Optional solution context; select product with solution.".to_string(),
                ),
                ..ValueContract::default()
            },
        ),
    ]);
    Manifest {
        format: FORMAT_VERSION.to_string(),
        id: slug.to_string(),
        name: name.to_string(),
        version: "0.1.0".to_string(),
        description: Some("A modular message decision pack for agent-readable ICP, pains, triggers, proof, CTA policy, avoid-rules, output rules, and copy guidance.".to_string()),
        target: None,
        profile: Some(gtm_profile()),
        personas: personas.clone(),
        target_personas: personas,
        operator_roles: vec!["GTM Engineering".to_string(), "PMM".to_string()],
        supported_channels: vec!["linkedin".to_string(), "email".to_string(), "call-prep".to_string(), "agent-brief".to_string()],
        persona_mappings: vec![
            persona_mapping("GTM Engineering", &["gtm engineering", "gtm engineer", "growth engineer", "growth engineering", "revops", "revenue operations", "sales ops", "sales operations", "growth ops"]),
            persona_mapping("PMM", &["pmm", "product marketing", "demand gen", "demand generation", "growth marketing", "messaging", "positioning"]),
            persona_mapping("PM", &["product manager", "head of product", "vp product", "chief product officer"]),
        ],
        lead_input_requirements: LeadInputRequirements {
            required_fields: vec!["name".to_string(), "title".to_string(), "company".to_string(), "company_domain".to_string(), "trigger".to_string(), "persona".to_string(), "segment".to_string(), "signals".to_string()],
            required_signal_fields: vec!["source".to_string()],
            required_attributes: vec!["contact_policy".to_string()],
            value_contracts,
            attribute_definitions,
            allow_undeclared_attributes: true,
        },
        qualification_gates: None,
        required_primitives: gtm_required_primitives(),
        primitive_map: gtm_primitive_map(),
        decision_input_contracts: vec![starter_prospect_decision_input_contract()],
        input_contracts: vec![InputContract {
            id: "prospect".to_string(),
            description: Some(
                "Reviewed prospect/source-row input contract for person, account, and relationship context."
                    .to_string(),
            ),
            schema_ref: Some("mdp.input.prospect.v0".to_string()),
            prompt: Some("prompts/normalize-prospect.yaml".to_string()),
            normalizes: strings(&["account", "person", "relationship"]),
            decision_input_contracts: strings(&["gtm.prospect-context"]),
        }],
        jobs: gtm_profile_jobs(),
        profile_eval: ProfileEval {
            required_categories: strings(&[
                "proceed",
                "insufficient-context",
                "refusal",
                "unsafe-output",
                "job-routing",
                "account-context-present",
                "account-context-missing",
                "account-only-no-draft",
                "prompt-output-validation",
            ]),
            activation: ProfileActivation {
                status: Some("ready".to_string()),
                summary: Some(
                    "Starter GTM profile includes primitive coverage, account-context fixtures, no-draft coverage, and prompt-output validation."
                        .to_string(),
                ),
            },
        },
        cards: vec![
            card_ref("personas", "cards/personas.yaml", CardKind::Personas, "Who the decision pack serves and what each persona needs.", &["GTM Engineering", "PMM", "PM"], &["persona"]),
            card_ref("positioning", "cards/positioning.yaml", CardKind::Positioning, "Category, product boundaries, value pillars, and what this pack is not.", &["GTM Engineering", "PMM", "PM"], &["positioning", "category", "boundary"]),
            card_ref("fit-rules", "cards/fit-rules.yaml", CardKind::FitRules, "ICP, fit, disqualification, and no-message rules.", &["GTM Engineering", "PMM", "PM"], &["fit", "icp", "disqualifier", "no-message"]),
            card_ref("signals", "cards/signals.yaml", CardKind::Signals, "Structured buying signals, triggers, and source interpretation rules.", &["GTM Engineering", "PMM", "PM"], &["signal", "trigger", "source", "clay", "deepline", "linkedin"]),
            card_ref("pains", "cards/pains.yaml", CardKind::Pains, "Buyer pains, triggers, and evidence requirements.", &["PMM", "PM"], &["pain", "trigger"]),
            card_ref("claims", "cards/claims.yaml", CardKind::Claims, "Approved claims and proof requirements an agent may use.", &["PMM", "GTM Engineering"], &["claim", "proof", "evidence"]),
            card_ref("motions", "cards/motions.yaml", CardKind::Motions, "Approved GTM motions and motion boundaries.", &["GTM Engineering", "PMM"], &["motion", "workflow"]),
            card_ref("channel-policies", "cards/channel-policies.yaml", CardKind::ChannelPolicies, "Channel-specific policy for LinkedIn, email, call prep, and agent briefs.", &["GTM Engineering", "PMM"], &["channel", "linkedin", "email", "initial", "follow-up", "call", "prep", "agent", "brief"]),
            card_ref("hooks", "cards/hooks.yaml", CardKind::Hooks, "Messaging hooks that can be reused after evidence checks.", &["PMM"], &["hook", "copy", "message"]),
            card_ref("portfolio-examples", "cards/portfolio-examples.yaml", CardKind::Hooks, "Synthetic portfolio scope demonstration.", &[], &["portfolio", "scope", "example"]),
            card_ref("ctas", "cards/ctas.yaml", CardKind::Ctas, "CTA rules, reply paths, and ask boundaries for outbound copy.", &["PMM", "GTM Engineering"], &["cta", "ask", "reply", "copy", "outbound", "message"]),
            card_ref("avoid-rules", "cards/avoid-rules.yaml", CardKind::AvoidRules, "Claims and categories the agent must avoid.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "avoid"]),
            card_ref("output-rules", "cards/output-rules.yaml", CardKind::OutputRules, "Global style, formatting, and output-structure rules for generated text.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "style", "format"]),
            card_ref("copy-patterns", "cards/copy-patterns.yaml", CardKind::CopyPatterns, "Copy structures and brief patterns for GTM outputs.", &["PMM"], &["copy", "brief", "outbound", "message"]),
            card_ref("objections", "cards/objections.yaml", CardKind::Objections, "Expected objections, category confusion, and approved response logic.", &[], &[]),
            card_ref("gaps", "cards/gaps.yaml", CardKind::Gaps, "Known gaps and open questions agents must surface instead of filling in.", &[], &[]),
        ],
        policy: Policy { progressive_disclosure: true, load_manifest_first: true, max_cards_per_route: 14, json_contract: "mdp.cli.v0".to_string(), no_auth_required: true },
        provenance: Provenance { owner: "local".to_string(), created_by: "mdp init".to_string(), notes: vec!["This pack is guidance and evidence context, not an execution system.".to_string(), "Agents should load only routed cards unless the user asks for a full audit.".to_string()] },
    }
}

pub(crate) fn starter_manifest(name: &str, slug: &str, template: &str) -> Manifest {
    let mut manifest = generated_starter_manifest(name, slug, template);
    manifest.decision_input_contracts.clear();
    for input_contract in &mut manifest.input_contracts {
        input_contract.decision_input_contracts.clear();
    }
    for job in &mut manifest.jobs {
        job.decision_input_contracts.clear();
    }
    manifest
        .lead_input_requirements
        .required_fields
        .retain(|field| field != "company");
    manifest.lead_input_requirements.required_attributes.clear();
    manifest
        .lead_input_requirements
        .attribute_definitions
        .remove("contact_policy");
    manifest
}

fn gtm_profile() -> Profile {
    Profile {
        id: "gtm".to_string(),
        label: Some("GTM Messaging".to_string()),
        version: Some("mdp.profile.v0".to_string()),
        context_dimensions: BTreeMap::from([
            (
                "product".to_string(),
                strings(&["local-cli", "agent-plugin"]),
            ),
            (
                "capability".to_string(),
                strings(&["portfolio-routing", "bounded-context"]),
            ),
            (
                "solution".to_string(),
                strings(&["gtm-messaging", "agent-enablement"]),
            ),
            ("segment".to_string(), strings(&["agent-assisted-gtm"])),
        ]),
        context_dimension_dependencies: BTreeMap::from([
            ("capability".to_string(), strings(&["product"])),
            ("solution".to_string(), strings(&["product"])),
        ]),
        product_foundation: Some(gtm_product_foundation()),
    }
}

fn gtm_required_primitives() -> Vec<String> {
    strings(&[
        "actors",
        "decision-criteria",
        "source-signals",
        "needs-requirements",
        "evidence-proof",
        "boundaries",
        "output-contracts",
        "routing-jobs",
        "gaps",
        "evals",
    ])
}

fn gtm_primitive_map() -> BTreeMap<String, PrimitiveMapping> {
    BTreeMap::from([
        (
            "actors".to_string(),
            primitive_mapping(&["personas"], &[], &["prospect"], &[], &[]),
        ),
        (
            "decision-criteria".to_string(),
            primitive_mapping(&["fit-rules"], &[], &[], &[], &[]),
        ),
        (
            "source-signals".to_string(),
            primitive_mapping(
                &["signals"],
                &["normalize-prospect-row"],
                &["prospect"],
                &[],
                &[
                    "account-context-present",
                    "account-context-missing",
                    "decision-input-contract",
                ],
            ),
        ),
        (
            "needs-requirements".to_string(),
            primitive_mapping(&["pains"], &[], &[], &[], &[]),
        ),
        (
            "evidence-proof".to_string(),
            primitive_mapping(&["claims", "positioning"], &[], &[], &[], &[]),
        ),
        (
            "boundaries".to_string(),
            primitive_mapping(
                &["avoid-rules", "objections", "positioning"],
                &[],
                &[],
                &[],
                &[],
            ),
        ),
        (
            "output-contracts".to_string(),
            primitive_mapping(
                &[
                    "output-rules",
                    "copy-patterns",
                    "ctas",
                    "hooks",
                    "portfolio-examples",
                    "channel-policies",
                ],
                &[],
                &[],
                &[],
                &[],
            ),
        ),
        (
            "routing-jobs".to_string(),
            primitive_mapping(
                &["channel-policies", "motions"],
                &[],
                &[],
                &[
                    "prospect-fit-or-brief",
                    "outbound-copy-brief",
                    "outbound-copy-review",
                ],
                &[],
            ),
        ),
        (
            "gaps".to_string(),
            primitive_mapping(
                &["gaps"],
                &[],
                &[],
                &[],
                &[
                    "fit-insufficient-context",
                    "brief-insufficient-context",
                    "account-context-missing",
                    "account-only-no-draft",
                    "decision-input-contract",
                ],
            ),
        ),
        (
            "evals".to_string(),
            primitive_mapping(
                &[],
                &[],
                &[],
                &[],
                &[
                    "fit-good",
                    "fit-insufficient-context",
                    "fit-disqualified",
                    "claim-check-unsupported",
                    "claim-check-customer-proof",
                    "claim-check-commercial-traction",
                    "claim-check-output-rule",
                    "linkedin-copy-route",
                    "email-initial-route",
                    "call-prep-route",
                    "account-context-present",
                    "account-context-missing",
                    "account-only-no-draft",
                    "decision-input-contract",
                    "portfolio-local-cli-route",
                    "portfolio-codex-plugin-route",
                    "portfolio-missing-scope-route",
                ],
            ),
        ),
    ])
}

fn gtm_profile_jobs() -> Vec<ProfileJob> {
    vec![
        ProfileJob {
            id: "prospect-fit-or-brief".to_string(),
            skill_id: "mdp-gtm-brief".to_string(),
            label: Some("Prospect row to fit decision or brief".to_string()),
            description: Some(
                "Normalize supplied row context, check fit, and route a bounded local brief."
                    .to_string(),
            ),
            required_primitives: strings(&[
                "actors",
                "decision-criteria",
                "source-signals",
                "evidence-proof",
                "boundaries",
                "output-contracts",
                "routing-jobs",
                "gaps",
            ]),
            input_contracts: strings(&["prospect"]),
            decision_input_contracts: Vec::new(),
            product_foundation: Some(foundation_binding(&[
                "product-identity",
                "product-exclusions",
                "actors",
                "operating-context",
                "problems",
                "claims",
                "proof-boundaries",
            ])),
            model_task: None,
            context_budget: Some(crate::models::JobContextBudget {
                max_entries: 48,
                max_bytes: 49_152,
                optional_kind_quotas: BTreeMap::new(),
            }),
        },
        ProfileJob {
            id: "outbound-copy-brief".to_string(),
            skill_id: "mdp-gtm-brief".to_string(),
            label: Some("Outbound copy brief".to_string()),
            description: Some(
                "Produce grounded copy guidance after fit, proof, guardrails, and output contracts are loaded."
                    .to_string(),
            ),
            required_primitives: strings(&[
                "actors",
                "decision-criteria",
                "source-signals",
                "evidence-proof",
                "boundaries",
                "output-contracts",
                "routing-jobs",
                "gaps",
            ]),
            input_contracts: strings(&["prospect"]),
            decision_input_contracts: Vec::new(),
            product_foundation: Some(foundation_binding(&[
                "product-identity",
                "product-exclusions",
                "actors",
                "operating-context",
                "problems",
                "outcomes",
                "differentiators",
                "claims",
                "proof-boundaries",
                "offers",
                "motions",
                "calls-to-action",
                "narrative-posture",
            ])),
            model_task: Some(crate::models::JobModelTask {
                kind: "generation".to_string(),
                prompt: "generate-outbound-copy-v1".to_string(),
            }),
            context_budget: Some(crate::models::JobContextBudget {
                max_entries: 64,
                max_bytes: 65_536,
                optional_kind_quotas: BTreeMap::new(),
            }),
        },
        ProfileJob {
            id: "outbound-copy-review".to_string(),
            skill_id: "mdp-gtm-brief".to_string(),
            label: Some("Supplied outbound copy review".to_string()),
            description: Some(
                "Evaluate a supplied copy draft against fit, proof, guardrails, and output rules."
                    .to_string(),
            ),
            required_primitives: strings(&[
                "actors",
                "decision-criteria",
                "source-signals",
                "evidence-proof",
                "boundaries",
                "output-contracts",
                "routing-jobs",
                "gaps",
            ]),
            input_contracts: strings(&["prospect"]),
            decision_input_contracts: Vec::new(),
            product_foundation: Some(foundation_binding(&[
                "product-identity",
                "product-exclusions",
                "actors",
                "alternatives",
                "claims",
                "proof-boundaries",
                "calls-to-action",
                "narrative-posture",
            ])),
            model_task: Some(crate::models::JobModelTask {
                kind: "review".to_string(),
                prompt: "review-outbound-copy-v1".to_string(),
            }),
            context_budget: Some(crate::models::JobContextBudget {
                max_entries: 64,
                max_bytes: 65_536,
                optional_kind_quotas: BTreeMap::new(),
            }),
        },
    ]
}

fn starter_prospect_decision_input_contract() -> DecisionInputContract {
    let common_sources = vec![
        DecisionInputSourceClass::UserProvided,
        DecisionInputSourceClass::CustomerSystem,
        DecisionInputSourceClass::ReviewedInternal,
        DecisionInputSourceClass::PublicWeb,
        DecisionInputSourceClass::SyntheticFixture,
    ];
    let private_sources = vec![
        DecisionInputSourceClass::UserProvided,
        DecisionInputSourceClass::CustomerSystem,
        DecisionInputSourceClass::ReviewedInternal,
        DecisionInputSourceClass::SyntheticFixture,
    ];
    let required_effects = vec![
        DecisionInputDecisionEffect::Readiness,
        DecisionInputDecisionEffect::Fit,
        DecisionInputDecisionEffect::Routing,
        DecisionInputDecisionEffect::Brief,
        DecisionInputDecisionEffect::Gaps,
        DecisionInputDecisionEffect::NoDraft,
    ];
    let mut attributes = vec![
        starter_decision_input_attribute(
            "company_name",
            "What is the reviewed company or account name?",
            "company",
            ValueContract {
                value_type: Some("string".to_string()),
                ..ValueContract::default()
            },
            DecisionInputRequirement::Required,
            common_sources.clone(),
            DecisionInputSensitivity::CustomerPrivate,
            required_effects.clone(),
        ),
        starter_decision_input_attribute(
            "company_domain",
            "What is the reviewed canonical company domain?",
            "company_domain",
            ValueContract {
                value_type: Some("string".to_string()),
                ..ValueContract::default()
            },
            DecisionInputRequirement::Required,
            common_sources.clone(),
            DecisionInputSensitivity::CustomerPrivate,
            required_effects.clone(),
        ),
        starter_decision_input_attribute(
            "person_name",
            "What is the reviewed name of the intended person?",
            "name",
            ValueContract {
                value_type: Some("string".to_string()),
                ..ValueContract::default()
            },
            DecisionInputRequirement::Required,
            common_sources.clone(),
            DecisionInputSensitivity::PersonalData,
            required_effects.clone(),
        ),
        starter_decision_input_attribute(
            "person_title",
            "What is the person's current reviewed job title?",
            "title",
            ValueContract {
                value_type: Some("string".to_string()),
                ..ValueContract::default()
            },
            DecisionInputRequirement::Required,
            common_sources.clone(),
            DecisionInputSensitivity::PersonalData,
            required_effects.clone(),
        ),
        starter_decision_input_attribute(
            "persona",
            "Which pack-owned persona is supported by the reviewed person context?",
            "persona",
            ValueContract {
                value_type: Some("string".to_string()),
                ..ValueContract::default()
            },
            DecisionInputRequirement::Required,
            common_sources.clone(),
            DecisionInputSensitivity::PersonalData,
            required_effects.clone(),
        ),
        starter_decision_input_attribute(
            "segment",
            "Which pack-owned segment is supported by the reviewed account context?",
            "segment",
            ValueContract {
                value_type: Some("string".to_string()),
                ..ValueContract::default()
            },
            DecisionInputRequirement::Required,
            common_sources.clone(),
            DecisionInputSensitivity::CustomerPrivate,
            required_effects.clone(),
        ),
        starter_decision_input_attribute(
            "trigger",
            "What source-backed event or condition makes this prospect relevant now?",
            "trigger",
            ValueContract {
                value_type: Some("string".to_string()),
                ..ValueContract::default()
            },
            DecisionInputRequirement::Required,
            common_sources,
            DecisionInputSensitivity::CustomerPrivate,
            required_effects.clone(),
        ),
        starter_decision_input_attribute(
            "contact_policy",
            "Does reviewed policy permit this prospect to enter deterministic fit and brief evaluation?",
            "attributes.contact_policy",
            ValueContract {
                value_type: Some("string".to_string()),
                enum_values: strings(&["clear", "do-not-contact", "needs-review"]),
                ..ValueContract::default()
            },
            DecisionInputRequirement::HardGate,
            private_sources,
            DecisionInputSensitivity::Restricted,
            vec![
                DecisionInputDecisionEffect::Readiness,
                DecisionInputDecisionEffect::Fit,
                DecisionInputDecisionEffect::Disqualification,
                DecisionInputDecisionEffect::Routing,
                DecisionInputDecisionEffect::Gaps,
                DecisionInputDecisionEffect::HumanReview,
                DecisionInputDecisionEffect::NoDraft,
            ],
        ),
    ];
    attributes
        .last_mut()
        .expect("contact policy attribute exists")
        .status_behavior = BTreeMap::from([
        (
            DecisionInputAttemptStatus::Observed,
            DecisionInputDisposition::Evaluate,
        ),
        (
            DecisionInputAttemptStatus::NotFound,
            DecisionInputDisposition::Block,
        ),
        (
            DecisionInputAttemptStatus::NotApplicable,
            DecisionInputDisposition::Block,
        ),
        (
            DecisionInputAttemptStatus::Blocked,
            DecisionInputDisposition::HumanReview,
        ),
        (
            DecisionInputAttemptStatus::Error,
            DecisionInputDisposition::HumanReview,
        ),
    ]);

    DecisionInputContract {
        id: "gtm.prospect-context".to_string(),
        version: "1.0.0".to_string(),
        description: Some(
            "Minimum attempted-complete prospect context required before canonical GTM fit, brief, or copy-review work.".to_string(),
        ),
        normalization: DecisionInputNormalization {
            prompt: "prompts/normalize-prospect.yaml".to_string(),
            prompt_version: "gtm-prospect-context.v2".to_string(),
            normalized_schema_ref: "mdp.normalized-decision-input.v2".to_string(),
        },
        source_classes: vec![
            DecisionInputSourceClass::UserProvided,
            DecisionInputSourceClass::CustomerSystem,
            DecisionInputSourceClass::ReviewedInternal,
            DecisionInputSourceClass::PublicWeb,
            DecisionInputSourceClass::SyntheticFixture,
        ],
        attributes,
        signal_projections: vec![
            DecisionInputSignalProjection {
                id: "why-now".to_string(),
                kind: "prospect_trigger".to_string(),
                roles: vec![DecisionInputSignalRole::WhyNow, DecisionInputSignalRole::Fit],
                contributor_attribute_ids: vec!["trigger".to_string()],
                value: ValueContract { value_type: Some("string".to_string()), ..ValueContract::default() },
                cardinality: DecisionInputSignalCardinality { min: 1, max: 8 },
                conflict_policy: DecisionInputSignalConflictPolicy::RequireAgreement,
                decision_effects: required_effects,
            },
            DecisionInputSignalProjection {
                id: "contact-policy".to_string(),
                kind: "contact_policy".to_string(),
                roles: vec![DecisionInputSignalRole::Disqualifier],
                contributor_attribute_ids: vec!["contact_policy".to_string()],
                value: ValueContract {
                    value_type: Some("string".to_string()),
                    enum_values: strings(&["clear", "do-not-contact", "needs-review"]),
                    ..ValueContract::default()
                },
                cardinality: DecisionInputSignalCardinality { min: 1, max: 4 },
                conflict_policy: DecisionInputSignalConflictPolicy::AnyDisqualifies,
                decision_effects: vec![
                    DecisionInputDecisionEffect::Disqualification,
                    DecisionInputDecisionEffect::HumanReview,
                    DecisionInputDecisionEffect::NoDraft,
                ],
            },
        ],
    }
}

fn starter_decision_input_attribute(
    id: &str,
    question: &str,
    output_path: &str,
    value: ValueContract,
    requirement: DecisionInputRequirement,
    source_classes: Vec<DecisionInputSourceClass>,
    sensitivity: DecisionInputSensitivity,
    decision_effects: Vec<DecisionInputDecisionEffect>,
) -> DecisionInputAttribute {
    DecisionInputAttribute {
        id: id.to_string(),
        question: question.to_string(),
        description: None,
        output_path: output_path.to_string(),
        value,
        requirement,
        applies_when: Vec::new(),
        decision_effects,
        source_classes,
        provenance: DecisionInputProvenancePolicy {
            required: true,
            required_fields: vec![
                DecisionInputProvenanceField::AttemptId,
                DecisionInputProvenanceField::SourceClass,
                DecisionInputProvenanceField::SourceLocator,
                DecisionInputProvenanceField::ObservedAt,
            ],
        },
        confidence: DecisionInputConfidencePolicy {
            required: true,
            minimum: Some(80),
        },
        freshness: DecisionInputFreshnessPolicy {
            required: true,
            max_age_days: Some(365),
            allow_unknown: false,
        },
        sensitivity,
        status_behavior: BTreeMap::new(),
    }
}

fn primitive_mapping(
    cards: &[&str],
    prompts: &[&str],
    input_contracts: &[&str],
    jobs: &[&str],
    evals: &[&str],
) -> PrimitiveMapping {
    PrimitiveMapping {
        cards: strings(cards),
        prompts: strings(prompts),
        input_contracts: strings(input_contracts),
        jobs: strings(jobs),
        evals: strings(evals),
    }
}

fn gtm_product_foundation() -> ProductFoundationRegistry {
    ProductFoundationRegistry {
        facets: vec![
            foundation_facet(
                "product-identity",
                ProductFoundationFacetKind::ProductIdentity,
                &[
                    ("positioning", "decision-layer"),
                    ("positioning", "progressive-disclosure"),
                ],
            ),
            foundation_facet(
                "product-exclusions",
                ProductFoundationFacetKind::ProductExclusions,
                &[
                    ("positioning", "not-execution-system"),
                    ("avoid-rules", "not-execution"),
                ],
            ),
            foundation_facet(
                "actors",
                ProductFoundationFacetKind::Actors,
                &[
                    ("personas", "gtm-engineering"),
                    ("personas", "pmm"),
                    ("personas", "pm"),
                ],
            ),
            foundation_facet(
                "operating-context",
                ProductFoundationFacetKind::OperatingContext,
                &[
                    ("signals", "source-row-signal"),
                    ("signals", "linkedin-profile-signal"),
                    ("signals", "company-context-signal"),
                ],
            ),
            foundation_facet(
                "problems",
                ProductFoundationFacetKind::Problems,
                &[
                    ("pains", "agent-context-drift"),
                    ("pains", "handoff-friction"),
                    ("pains", "claim-inconsistency"),
                ],
            ),
            foundation_facet(
                "outcomes",
                ProductFoundationFacetKind::Outcomes,
                &[
                    ("claims", "modular-pack-routing"),
                    ("claims", "local-offline"),
                    ("claims", "versionable-context"),
                ],
            ),
            foundation_facet(
                "differentiators",
                ProductFoundationFacetKind::Differentiators,
                &[
                    ("positioning", "progressive-disclosure"),
                    ("claims", "local-offline"),
                ],
            ),
            foundation_facet(
                "alternatives",
                ProductFoundationFacetKind::Alternatives,
                &[
                    ("objections", "why-not-prompt"),
                    ("objections", "why-not-sequencer"),
                ],
            ),
            foundation_facet(
                "claims",
                ProductFoundationFacetKind::Claims,
                &[
                    ("claims", "modular-pack-routing"),
                    ("claims", "local-offline"),
                    ("claims", "versionable-context"),
                ],
            ),
            foundation_facet(
                "proof-boundaries",
                ProductFoundationFacetKind::ProofBoundaries,
                &[
                    ("avoid-rules", "no-unsourced-claims"),
                    ("fit-rules", "no-context-no-copy"),
                ],
            ),
            foundation_facet(
                "offers",
                ProductFoundationFacetKind::Offers,
                &[
                    ("motions", "copy-brief"),
                    ("motions", "agent-preflight"),
                    ("motions", "source-row-to-brief"),
                ],
            ),
            foundation_facet(
                "motions",
                ProductFoundationFacetKind::Motions,
                &[
                    ("motions", "copy-brief"),
                    ("motions", "agent-preflight"),
                    ("motions", "source-row-to-brief"),
                ],
            ),
            foundation_facet(
                "calls-to-action",
                ProductFoundationFacetKind::CallsToAction,
                &[
                    ("ctas", "soft-ask"),
                    ("ctas", "calendar-second"),
                    ("ctas", "no-false-urgency"),
                    ("ctas", "reply-path"),
                ],
            ),
            foundation_facet(
                "narrative-posture",
                ProductFoundationFacetKind::NarrativePosture,
                &[
                    ("output-rules", "plain-text-by-default"),
                    ("output-rules", "no-fake-personalization"),
                    ("copy-patterns", "claim-gap"),
                ],
            ),
            ProductFoundationFacet {
                id: "known-gaps".to_string(),
                kind: ProductFoundationFacetKind::Gaps,
                entries: Vec::new(),
                gaps: foundation_refs(&[
                    ("gaps", "missing-company-proof"),
                    ("gaps", "unclear-fit"),
                    ("gaps", "hosted-api-not-included"),
                ]),
                conflicts_with: Vec::new(),
            },
        ],
    }
}

pub(crate) fn foundation_binding(required: &[&str]) -> ProductFoundationBinding {
    ProductFoundationBinding {
        required: strings(required),
        conditional: Vec::new(),
        optional: vec!["known-gaps".to_string()],
        excluded: Vec::new(),
    }
}

fn foundation_facet(
    id: &str,
    kind: ProductFoundationFacetKind,
    entries: &[(&str, &str)],
) -> ProductFoundationFacet {
    ProductFoundationFacet {
        id: id.to_string(),
        kind,
        entries: foundation_refs(entries),
        gaps: Vec::new(),
        conflicts_with: Vec::new(),
    }
}

pub(crate) fn foundation_refs(values: &[(&str, &str)]) -> Vec<ProductFoundationEntryRef> {
    values
        .iter()
        .map(|(card_id, entry_id)| ProductFoundationEntryRef {
            card_id: (*card_id).to_string(),
            entry_id: (*entry_id).to_string(),
        })
        .collect()
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

pub(crate) fn starter_cards(_template: &str) -> Vec<(&'static str, Card)> {
    vec![
        ("personas.yaml", card("personas", CardKind::Personas, "Core personas", "The users who author, maintain, and consume the decision pack.", &["GTM Engineering", "PMM", "PM"], &["persona"], vec![
            entry_with_evidence("gtm-engineering", "GTM Engineering", "Needs precise contracts, data boundaries, approved workflows, and machine-readable routing.", &["GTM Engineering"], &["mdp-reference-contract"]),
            entry_with_evidence("pmm", "PMM", "Needs pains, triggers, hooks, proof points, CTA policy, and copy constraints without losing source fidelity.", &["PMM"], &["mdp-reference-contract"]),
            entry_with_evidence("pm", "PM", "Needs product boundaries, roadmap-relevant pain evidence, and clear decisions about what the product is not.", &["PM"], &["mdp-reference-contract"]),
        ])),
        ("positioning.yaml", card("positioning", CardKind::Positioning, "Positioning and boundaries", "Category and product truth that every routed brief should preserve.", &["GTM Engineering", "PMM", "PM"], &["positioning", "category", "boundary"], vec![
            entry_with_evidence("decision-layer", "Versioned decision context for agents", "MDP is versioned decision context for agents. It stores the rules, approved evidence, boundaries, gaps, and job-specific context an agent may use.", &["GTM Engineering", "PMM", "PM"], &["mdp-reference-contract"]),
            entry_with_evidence("not-execution-system", "Not an execution system", "MDP is not an agent runtime, graph database, orchestration framework, persistent memory layer, or universal company graph. It does not call models, send messages, update CRM records, enrich leads, scrape data, sequence outreach, replace workflow execution tools, or prove that a source claim is true.", &["GTM Engineering", "PMM", "PM"], &["mdp-reference-contract"]),
            entry_with_evidence("progressive-disclosure", "Progressive disclosure", "The pack is a small manifest plus modular cards. Agents should load only the cards returned by route or brief commands.", &["GTM Engineering", "PMM"], &["mdp-reference-contract"]),
        ])),
        ("fit-rules.yaml", card("fit-rules", CardKind::FitRules, "Fit rules", "ICP, qualification, disqualification, and no-message rules.", &["GTM Engineering", "PMM", "PM"], &["fit", "icp", "disqualifier", "no-message"], vec![
            entry_with_evidence("good-fit-agent-gtm", "Good fit: agent-assisted GTM", "Use when the account is building GTM workflows with agents, provider-neutral source rows, Codex/Claude Code/OpenCode, or multiple systems that need consistent message context.", &["GTM Engineering", "PMM"], &["mdp-reference-contract", "examples/clay-row.json"]),
            Entry { id: "no-context-no-copy".to_string(), title: "No message without context".to_string(), body: "If the row has no persona, trigger, source, or useful account context, return insufficient-context instead of drafting polished copy.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string()], scope: BTreeMap::new(), evidence: vec!["mdp-reference-contract".to_string()], avoid: vec!["no source".to_string(), "unknown persona".to_string(), "no trigger".to_string()], exact_paragraphs: None, constraints: EntryConstraints::default(), metadata: BTreeMap::new() },
            Entry { id: "bad-fit-sending-only".to_string(), title: "Bad fit: sending-only ask".to_string(), body: "If the request is only to blast, sequence, or auto-send messages without decision context, treat it as out of scope for MDP.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string()], scope: BTreeMap::new(), evidence: vec!["mdp-reference-contract".to_string()], avoid: vec!["blast".to_string(), "auto-send".to_string(), "sequence everyone".to_string()], exact_paragraphs: None, constraints: EntryConstraints::default(), metadata: BTreeMap::new() },
        ])),
        ("signals.yaml", card("signals", CardKind::Signals, "Signals and triggers", "How to interpret source rows, LinkedIn context, source material, and account signals.", &["GTM Engineering", "PMM", "PM"], &["signal", "trigger", "source", "source-row", "csv", "crm", "linkedin"], vec![
            entry_with_evidence("source-row-signal", "Source row signal", "Treat user-provided rows, CSVs, CRM exports, Clay, Deepline, or other supplied row-like inputs as evidence inputs. Preserve source and confidence when present, and state weak signals as hypotheses.", &["GTM Engineering", "PMM"], &["examples/clay-row.json"]),
            entry_with_evidence("linkedin-profile-signal", "LinkedIn profile signal", "Use LinkedIn URLs or profile summaries as context for role, background, and likely priorities. Do not pretend the profile proves a product need by itself.", &["PMM"], &["examples/clay-row.json"]),
            entry_with_evidence("company-context-signal", "Company context signal", "Company website, hiring, funding, product, and stack clues can shape the pain hypothesis when the pack states how to interpret them.", &["PMM", "PM"], &["examples/clay-row.json"]),
        ])),
        ("pains.yaml", card("pains", CardKind::Pains, "Pains and triggers", "Reusable buyer pains with evidence expectations.", &["PMM", "PM"], &["pain", "trigger"], vec![
            entry_with_evidence("agent-context-drift", "Agent context drift", "Agents working on GTM tasks lose product truth when source context, contracts, and approved claims are scattered.", &["PMM", "PM"], &["mdp-reference-contract"]),
            entry_with_evidence("handoff-friction", "Handoff friction", "Teams need a way to give agents enough context to draft or decide without dumping a giant doc into every prompt.", &["GTM Engineering", "PMM"], &["mdp-reference-contract"]),
            entry_with_evidence("claim-inconsistency", "Claim inconsistency", "Different agents or workflows reuse outdated claims, unsupported proof points, or mismatched CTAs when there is no shared pack.", &["PMM"], &["mdp-reference-contract"]),
        ])),
        ("claims.yaml", card("claims", CardKind::Claims, "Approved claims", "Claims an agent may use only when the route and source context support them.", &["PMM", "GTM Engineering"], &["claim", "proof", "evidence"], vec![
            entry_with_evidence("modular-pack-routing", "Versioned decision context", "MDP is versioned decision context for agents. It lets teams store messaging decisions in a manifest plus modular cards so agents load relevant context instead of a giant prompt.", &["PMM", "GTM Engineering"], &["mdp-reference-contract"]),
            entry_with_evidence("local-offline", "Local offline CLI", "MDP is a local/offline standard, CLI, and plugin for modular GTM messaging context.", &["GTM Engineering"], &["mdp-reference-contract"]),
            entry_with_evidence("versionable-context", "Version-declared context", "Each MDP pack declares a version in its manifest alongside the card references for its modular message context.", &["GTM Engineering", "PMM"], &["mdp-pack-manifest"]),
        ])),
        ("motions.yaml", card("motions", CardKind::Motions, "Approved motions", "GTM workflows this pack can support as context.", &["GTM Engineering", "PMM"], &["motion", "workflow"], vec![
            entry_with_evidence("copy-brief", "Copy brief", "Route persona, pain, hook, avoid-rules, CTA policy, and copy-pattern cards to produce a grounded brief, not final unsupervised sending.", &["PMM"], &["mdp-reference-contract"]),
            entry_with_evidence("agent-preflight", "Agent preflight", "Let an agent inspect the pack before doing GTM work and report missing evidence or unsupported claims.", &["GTM Engineering"], &["mdp-reference-contract"]),
            entry_with_evidence("source-row-to-brief", "Source row to brief", "Convert a provider-neutral prospect/source row into a message brief before drafting. Keep source fields as inputs, not as proof of claims.", &["GTM Engineering", "PMM"], &["mdp-reference-contract", "examples/clay-row.json"]),
        ])),
        ("channel-policies.yaml", card("channel-policies", CardKind::ChannelPolicies, "Channel policies", "Channel and lifecycle rules for how routed message decisions should be used.", &["GTM Engineering", "PMM"], &["channel", "linkedin", "email", "initial", "follow-up", "call", "prep", "agent", "brief"], vec![
            entry_with_evidence("linkedin-initial-touch", "LinkedIn initial touch", "For a first LinkedIn touch, use one sourced observation or explicitly labeled hypothesis, one relevant angle, and one low-friction ask. Keep it brief and do not make the first note feel like a full pitch.", &["PMM"], &["mdp-reference-contract"]),
            entry_with_evidence("linkedin-follow-up", "LinkedIn follow-up", "For a later LinkedIn note, reference the earlier outreach lightly, add one new relevance angle or question, and keep the ask low-friction. Do not use guilt, breakup framing, or a bare bump.", &["PMM"], &["mdp-reference-contract"]),
            Entry { id: "email-initial-touch".to_string(), title: "Email initial touch".to_string(), body: "For a first cold email, use the email output rules, one source-backed reason or explicit hypothesis, one approved angle, and one reply path. Keep one soft CTA and one question only. Do not lead with a calendar ask unless fit is strong and the source context supports it. Default to no links, attachments, images, HTML polish, or tracking unless the user explicitly overrides.".to_string(), applies_to: vec!["PMM".to_string()], scope: BTreeMap::new(), evidence: vec!["mdp-reference-contract".to_string()], avoid: vec![], exact_paragraphs: None, constraints: initial_email_constraints(), metadata: BTreeMap::new() },
            entry_with_evidence("email-follow-up", "Email follow-up", "For follow-up email copy, assume a maximum of three follow-up notes after the initial email. Refer back without assuming interest, add one concrete reason, question, angle, or proof gap, and keep the reply path to owner validation or relevance. Do not use bump language, bare bumps, guilt breakup framing, or imply a longer follow-up sequence than the user supplied.", &["PMM"], &["mdp-reference-contract"]),
            entry_with_evidence("call-prep", "Call prep", "Return likely persona, pains, allowed claims, avoid-rules, open questions, and the exact cards loaded. Do not pretend this is CRM history.", &["GTM Engineering", "PMM"], &["mdp-reference-contract"]),
            entry_with_evidence("agent-brief", "Agent brief", "Return fit status, loaded cards, approved claims, avoid-rules, source hypotheses, open gaps, and exact handoff boundaries. Do not send, enrich, or update external systems.", &["GTM Engineering", "PMM"], &["mdp-reference-contract"]),
        ])),
        ("hooks.yaml", card("hooks", CardKind::Hooks, "Hooks", "Starter hook patterns that require local evidence before use.", &["PMM"], &["hook", "copy", "message"], vec![
            entry_with_evidence("manifest-not-monolith", "Manifest, not monolith", "Position the pack as a small manifest plus task-specific cards so agents load the minimum needed context.", &["PMM"], &["mdp-reference-contract"]),
            entry_with_evidence("evidence-before-action", "Evidence before action", "Emphasize that GTM execution should start with source context, contracts, and approval boundaries.", &["PMM"], &["mdp-reference-contract"]),
            entry_with_evidence("one-context-many-agents", "One context, many agents", "Use when the account has Claude Code, Codex, OpenCode, Clay, or other systems that need the same source of messaging truth.", &["PMM", "GTM Engineering"], &["mdp-reference-contract", "examples/clay-row.json"]),
        ])),
        ("portfolio-examples.yaml", card("portfolio-examples", CardKind::Hooks, "Portfolio scope examples", "Synthetic examples showing how product scope filters otherwise agnostic message decisions.", &["PMM"], &["portfolio", "scope", "example"], vec![
            entry("portfolio-scope-is-applicability", "Scope qualifies primitives", "Product, capability, solution, and segment narrow where an entry applies. They do not replace actors, pains, proof, boundaries, hooks, CTAs, or other agnostic primitives.", &["PMM"]),
            scoped_entry("local-cli-angle", "Local CLI portfolio angle", "Lead with local validation, versionable decision context, and deterministic routing for the local CLI product example.", &["PMM"], &[("product", &["local-cli"])]),
            scoped_entry("codex-plugin-angle", "Codex plugin portfolio angle", "Lead with bounded context handoff and workflow guidance for the plugin product example.", &["PMM"], &[("product", &["agent-plugin"])]),
            scoped_entry("portfolio-routing-capability", "Portfolio routing capability angle", "Use structured product-aware routing when the selected local CLI product and portfolio-routing capability are both relevant.", &["PMM"], &[("product", &["local-cli"]), ("capability", &["portfolio-routing"])]),
        ])),
        ("ctas.yaml", card("ctas", CardKind::Ctas, "CTA rules", "Calls to action, reply paths, and ask boundaries for outbound copy.", &["PMM", "GTM Engineering"], &["cta", "ask", "reply", "copy", "outbound", "message"], vec![
            entry_with_evidence("soft-ask", "Soft ask", "Default to a low-friction ask that optimizes for a human reply: compare notes, sanity-check the hypothesis, ask who owns the problem, or ask whether the angle is worth a quick look.", &["PMM", "GTM Engineering"], &["mdp-reference-contract"]),
            entry_with_evidence("calendar-second", "Calendar second", "Do not make the first CTA a calendar booking unless fit is strong, the reason for urgency is sourced, and the channel policy allows it. Use a reply-path question first when fit or ownership is uncertain.", &["PMM", "GTM Engineering"], &["mdp-reference-contract"]),
            entry_with_evidence("no-false-urgency", "No false urgency", "Do not manufacture urgency or imply the prospect has asked for help unless the source row says so.", &["PMM"], &["mdp-reference-contract"]),
            entry_with_evidence("reply-path", "Reply path", "When the best next step is not a meeting, ask a routing question that helps identify the owner, priority, or current workflow.", &["PMM", "GTM Engineering"], &["mdp-reference-contract"]),
        ])),
        ("avoid-rules.yaml", card("avoid-rules", CardKind::AvoidRules, "Avoid rules", "Category and claim boundaries agents must keep intact.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "avoid"], vec![
            Entry { id: "not-execution".to_string(), title: "Do not claim execution".to_string(), body: "Do not describe the decision pack as an agent runtime, graph database, orchestration framework, persistent memory layer, AI SDR, sequencer, CRM, enrichment provider, scraper, BI tool, meeting booker, sender, AI-owned response system, or generic RevOps automation system. Do not claim its hashes prove source truth.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string(), "PM".to_string()], scope: BTreeMap::new(), evidence: vec!["mdp-reference-contract".to_string()], avoid: vec!["agent runtime".to_string(), "graph database".to_string(), "graph engineering platform".to_string(), "orchestration framework".to_string(), "persistent memory layer".to_string(), "memory layer".to_string(), "source truth".to_string(), "universal company graph".to_string(), "proves source truth".to_string(), "AI SDR".to_string(), "sequencer".to_string(), "CRM replacement".to_string(), "generic automation".to_string(), "scraper".to_string(), "update CRM".to_string(), "updates CRM".to_string(), "sends for you".to_string(), "auto-sends".to_string(), "books meetings".to_string(), "launches campaigns".to_string(), "AI can own the response".to_string()], exact_paragraphs: None, constraints: EntryConstraints::default(), metadata: BTreeMap::new() },
            Entry { id: "no-unsourced-claims".to_string(), title: "No unsourced claims".to_string(), body: "Do not add quantified outcomes, integrations, customer names, compliance/security approval, production adoption, design partner, paid pilot, market validation, commercial traction, weak trust claims, fake personalization, RFP/proposal-platform replacement, or product capability claims unless they are present in the claims card or supplied source material.".to_string(), applies_to: vec!["PMM".to_string(), "GTM Engineering".to_string()], scope: BTreeMap::new(), evidence: vec![], avoid: vec!["guaranteed".to_string(), "proven ROI".to_string(), "doubles reply rates".to_string(), "fully automated".to_string(), "connect to your CRM".to_string(), "connects to your CRM".to_string(), "native CRM integration".to_string(), "security-approved".to_string(), "handles compliance".to_string(), "compliance approval".to_string(), "customers already use".to_string(), "customers rely on".to_string(), "customer adoption".to_string(), "design partner".to_string(), "design partners".to_string(), "paid pilot".to_string(), "paid pilots".to_string(), "production adoption".to_string(), "production use".to_string(), "validated adoption".to_string(), "ARR conversion".to_string(), "workshop conversion".to_string(), "workshops converted".to_string(), "market validated".to_string(), "market validation".to_string(), "I loved your recent LinkedIn post".to_string(), "bypasses procurement".to_string(), "bypass legal".to_string(), "replace proposal management software".to_string(), "replaces proposal management software".to_string(), "best-in-class".to_string()], exact_paragraphs: None, constraints: EntryConstraints::default(), metadata: BTreeMap::new() },
        ])),
        ("output-rules.yaml", card("output-rules", CardKind::OutputRules, "Output rules", "Global style, formatting, and output-structure rules generated text must follow.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "style", "format"], vec![
            Entry { id: "no-em-dashes".to_string(), title: "No em dashes".to_string(), body: "Do not use em dashes in generated copy. Use commas, periods, colons, or shorter sentences instead.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string(), "PM".to_string()], scope: BTreeMap::new(), evidence: vec![], avoid: vec!["—".to_string()], exact_paragraphs: None, constraints: EntryConstraints::default(), metadata: BTreeMap::new() },
            Entry { id: "plain-text-by-default".to_string(), title: "Plain text by default".to_string(), body: "For outbound email or LinkedIn copy, default to plain text. Do not include links, attachments, images, HTML, tracking parameters, or decorative formatting unless the user explicitly asks and the pack supports it.".to_string(), applies_to: vec!["PMM".to_string(), "GTM Engineering".to_string()], scope: BTreeMap::new(), evidence: vec![], avoid: vec!["http://".to_string(), "https://".to_string(), "<html".to_string(), "<img".to_string(), "utm_".to_string()], exact_paragraphs: None, constraints: EntryConstraints::default(), metadata: BTreeMap::new() },
            entry("initial-email-shape", "Initial email shape", "When drafting an initial cold email, aim for roughly 90-125 words, use a short non-clickbait subject, and avoid fake Re: or Fwd: framing. Put detailed narrative structure in copy-patterns, not here.", &["PMM"]),
            entry("no-fake-personalization", "No fake personalization", "Do not imply the sender read, watched, met, noticed, or personally researched something unless that source context is present. Use hypotheses when the source signal is weak.", &["PMM", "GTM Engineering"]),
            entry("honor-paragraph-count", "Honor paragraph count", "If the user or pack states a paragraph count, match it exactly. Do not add setup, recap, or explanation paragraphs outside the requested structure.", &["PMM", "GTM Engineering", "PM"]),
            entry("no-meta-commentary", "No meta commentary", "Do not explain why the copy works, describe the structure, or include drafting notes unless the user asks for critique or rationale.", &["PMM", "GTM Engineering", "PM"]),
        ])),
        ("copy-patterns.yaml", card("copy-patterns", CardKind::CopyPatterns, "Copy patterns", "Reusable structures for brief and copy outputs.", &["PMM"], &["copy", "brief", "outbound", "message"], vec![
            entry_with_evidence("brief-contract", "Brief contract", "Return audience, job, loaded cards, decision trace, approved claims, avoid rules, open questions, and draft direction.", &["PMM"], &["mdp-reference-contract"]),
            entry_with_evidence("claim-gap", "Claim gap", "When evidence is missing, write the gap explicitly instead of smoothing over it with generic GTM language.", &["PMM", "PM"], &["mdp-reference-contract"]),
            entry_with_evidence("trigger-hypothesis-proof-gap-angle-cta", "Trigger/hypothesis -> proof gap -> angle -> CTA", "Structure outbound copy as observed trigger or explicit hypothesis, proof gap or missing context, approved MDP angle, and one soft CTA. Mark weak inputs as hypotheses instead of fake personalization.", &["PMM"], &["mdp-reference-contract", "examples/clay-row.json"]),
        ])),
        ("objections.yaml", card("objections", CardKind::Objections, "Objections and alternatives", "Category confusion and response logic for agents to preserve.", &["PMM", "GTM Engineering"], &["objection", "alternative", "response"], vec![
            entry_with_evidence("why-not-prompt", "Why not one giant prompt?", "Explain that MDP favors versioned, testable, progressively loaded cards so agents can fetch only the context needed for the current job.", &["PMM", "GTM Engineering"], &["mdp-reference-contract"]),
            entry_with_evidence("why-not-sequencer", "Why not a sequencer?", "Clarify that MDP stores message decisions and evidence. Sequencers or CRMs may consume outputs, but they are separate execution systems.", &["PMM", "GTM Engineering"], &["mdp-reference-contract"]),
        ])),
        ("gaps.yaml", card("gaps", CardKind::Gaps, "Known gaps", "Durable gaps and open questions agents should surface instead of inventing answers.", &["GTM Engineering", "PMM", "PM"], &["gap", "unknown", "open-question"], vec![
            entry("missing-company-proof", "Missing company-specific proof", "If a prospect/account row lacks concrete source context, ask for source material or state the personalization gap before drafting.", &["PMM", "GTM Engineering"]),
            entry("unclear-fit", "Unclear fit", "If role, segment, or trigger does not map to a fit rule, return insufficient-context instead of forcing a message.", &["GTM Engineering", "PMM"]),
            entry("hosted-api-not-included", "Hosted API not included", "The MVP is local/offline. Do not imply a hosted API exists unless the user has deployed one separately.", &["GTM Engineering", "PMM"]),
        ])),
    ]
}

pub(crate) fn starter_source_ledger(_template: &str) -> Value {
    json!({
        "format": "mdp.sources.v0",
        "purpose": "Source ledger for evidence used in cards. Keep direct source claims separate from interpretation, and preserve gaps instead of inventing proof.",
        "rules": [
            "Add public URLs, user-provided docs, or note identifiers before bulk card writing.",
            "Use direct_claims for facts the source states, and interpretations for how the pack may use them.",
            "Mark confidence and freshness when known.",
            "Do not include private customer data, raw call notes, local browser data, or sensitive local files."
        ],
        "sources": [
            {
                "id": "mdp-reference-contract",
                "kind": "repo-doc",
                "locator": ".mdp/cards/positioning.yaml",
                "freshness": "repo-current",
                "confidence": "high",
                "direct_claims": [
                    "MDP is versioned decision context for agents.",
                    "MDP is a local/offline standard, CLI, and plugin for modular GTM messaging context.",
                    "MDP stores decision context and routing contracts; it is not execution infrastructure.",
                    "The pack is a small manifest plus modular cards. Agents should load only the cards returned by route or brief commands."
                ],
                "interpretations": [
                    "Use this source for category boundaries, not for third-party customer proof."
                ],
                "gaps": []
            },
            {
                "id": "mdp-pack-manifest",
                "kind": "pack-manifest",
                "locator": ".mdp/manifest.yaml",
                "freshness": "pack-current",
                "confidence": "high",
                "direct_claims": [
                    "This MDP pack declares its version and card references in the pack manifest."
                ],
                "interpretations": [
                    "Use this source only for the pack's declared version and manifest-addressed card structure."
                ],
                "gaps": []
            },
            {
                "id": "example-prospect",
                "kind": "synthetic-example",
                "locator": "examples/clay-row.json",
                "freshness": "generated",
                "confidence": "demo-only",
                "direct_claims": [
                    "This row is fictional starter data for exercising fit, route, and brief commands."
                ],
                "interpretations": [
                    "Do not treat the example prospect as a real account, customer, or source of market evidence."
                ],
                "gaps": [
                    "Replace with a real or intentionally sanitized prospect row before production copy work."
                ]
            }
        ]
    })
}

fn eval_profile(category: &str, primitives: &[&str], jobs: &[&str]) -> Value {
    json!({
        "category": category,
        "primitives": strings(primitives),
        "jobs": strings(jobs)
    })
}

pub(crate) fn generated_starter_evals() -> Vec<(&'static str, Value)> {
    vec![
        decision_input_contract_eval(),
        (
            "linkedin-copy-route.yaml",
            json!({
                "id": "linkedin-copy-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "job-routing",
                    &["actors", "routing-jobs", "output-contracts"],
                    &["outbound-copy-brief"]
                ),
                "persona": "PMM",
                "job": "linkedin outbound copy",
                "expect_draft_status": "ready",
                "expect_load_order_contains": [
                    ".mdp/cards/personas.yaml",
                    ".mdp/cards/avoid-rules.yaml",
                    ".mdp/cards/output-rules.yaml",
                    ".mdp/cards/positioning.yaml",
                    ".mdp/cards/claims.yaml",
                    ".mdp/cards/ctas.yaml"
                ],
                "expect_entry_titles_contains": ["LinkedIn initial touch"],
                "expect_entry_titles_excludes": ["LinkedIn follow-up", "Email initial touch", "Email follow-up", "Call prep"]
            }),
        ),
        (
            "linkedin-follow-up-route.yaml",
            json!({
                "id": "linkedin-follow-up-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "job-routing",
                    &["actors", "routing-jobs", "output-contracts"],
                    &["outbound-copy-brief"]
                ),
                "persona": "PMM",
                "job": "linkedin follow up message",
                "expect_draft_status": "ready",
                "expect_load_order_contains": [
                    ".mdp/cards/channel-policies.yaml",
                    ".mdp/cards/copy-patterns.yaml"
                ],
                "expect_entry_titles_contains": ["LinkedIn follow-up"],
                "expect_entry_titles_excludes": ["LinkedIn initial touch", "Email initial touch", "Email follow-up", "Call prep"]
            }),
        ),
        (
            "revops-owner-agent-brief-route.yaml",
            json!({
                "id": "revops-owner-agent-brief-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "job-routing",
                    &["actors", "source-signals", "routing-jobs"],
                    &["prospect-fit-or-brief"]
                ),
                "persona": "GTM Engineering",
                "job": "agent brief for RevOps owner source row",
                "expect_load_order_contains": [
                    ".mdp/cards/personas.yaml",
                    ".mdp/cards/avoid-rules.yaml",
                    ".mdp/cards/output-rules.yaml",
                    ".mdp/cards/fit-rules.yaml",
                    ".mdp/cards/signals.yaml",
                    ".mdp/cards/channel-policies.yaml"
                ],
                "expect_entry_titles_contains": ["Agent brief"]
            }),
        ),
        (
            "founder-gtm-lead-pack-route.yaml",
            json!({
                "id": "founder-gtm-lead-pack-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "job-routing",
                    &["actors", "boundaries", "output-contracts"],
                    &["prospect-fit-or-brief"]
                ),
                "persona": "PM",
                "job": "create or improve GTM pack for founder GTM lead",
                "expect_load_order_contains": [
                    ".mdp/cards/personas.yaml",
                    ".mdp/cards/avoid-rules.yaml",
                    ".mdp/cards/output-rules.yaml",
                    ".mdp/cards/positioning.yaml"
                ]
            }),
        ),
        (
            "email-initial-route.yaml",
            json!({
                "id": "email-initial-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "job-routing",
                    &["actors", "routing-jobs", "output-contracts"],
                    &["outbound-copy-brief"]
                ),
                "persona": "PMM",
                "job": "initial email outbound message",
                "expect_draft_status": "ready",
                "expect_load_order_contains": [
                    ".mdp/cards/channel-policies.yaml",
                    ".mdp/cards/copy-patterns.yaml"
                ],
                "expect_entry_titles_contains": ["Email initial touch"],
                "expect_entry_titles_excludes": ["Email follow-up", "LinkedIn initial touch", "LinkedIn follow-up", "Call prep"]
            }),
        ),
        (
            "email-follow-up-route.yaml",
            json!({
                "id": "email-follow-up-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "job-routing",
                    &["actors", "routing-jobs", "output-contracts"],
                    &["outbound-copy-brief"]
                ),
                "persona": "PMM",
                "job": "email follow up",
                "expect_draft_status": "ready",
                "expect_load_order_contains": [
                    ".mdp/cards/channel-policies.yaml",
                    ".mdp/cards/copy-patterns.yaml"
                ],
                "expect_entry_titles_contains": ["Email follow-up"],
                "expect_entry_titles_excludes": ["Email initial touch", "LinkedIn initial touch", "LinkedIn follow-up", "Call prep"]
            }),
        ),
        (
            "call-prep-route.yaml",
            json!({
                "id": "call-prep-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "job-routing",
                    &["actors", "decision-criteria", "routing-jobs"],
                    &["prospect-fit-or-brief"]
                ),
                "persona": "GTM Engineering",
                "job": "call prep",
                "expect_load_order_contains": [
                    ".mdp/cards/channel-policies.yaml",
                    ".mdp/cards/fit-rules.yaml"
                ],
                "expect_entry_titles_contains": ["Call prep"],
                "expect_entry_titles_excludes": ["LinkedIn initial touch", "LinkedIn follow-up", "Email initial touch", "Email follow-up"]
            }),
        ),
        (
            "account-context-present.yaml",
            json!({
                "id": "account-context-present",
                "command": "fit",
                "job": "prospect-fit-or-brief",
                "profile_eval": eval_profile(
                    "account-context-present",
                    &["actors", "decision-criteria", "source-signals"],
                    &["prospect-fit-or-brief"]
                ),
                "expect_status": "insufficient-context",
                "prospect": {
                    "name": "Alex Rivera",
                    "title": "Revenue Operations Lead",
                    "company": "Northstar Cloud",
                    "company_domain": "northstarcloud.com",
                    "company_url": "https://northstarcloud.com",
                    "background": "supplied row says the account is standardizing qualification data across CRM exports, spreadsheets, and agent-assisted GTM workflows",
                    "trigger": "standardizing prospect qualification data before routing new campaigns",
                    "persona": "GTM Engineering",
                    "segment": "agent-assisted GTM",
                    "source_kind": "synthetic-example",
                    "synthetic": true,
                    "signals": [
                        {
                            "id": "qualification-data-standardization",
                            "title": "Standardizing prospect qualification data",
                            "source": "synthetic account context row",
                            "confidence": "medium",
                            "freshness": "recent",
                            "state_as": "supplied"
                        }
                    ]
                }
            }),
        ),
        (
            "account-context-missing.yaml",
            json!({
                "id": "account-context-missing",
                "command": "fit",
                "job": "prospect-fit-or-brief",
                "profile_eval": eval_profile(
                    "account-context-missing",
                    &["decision-criteria", "source-signals", "gaps"],
                    &["prospect-fit-or-brief"]
                ),
                "expect_status": "insufficient-context",
                "prospect": {
                    "name": "Taylor Lee",
                    "title": "Revenue Operations Lead",
                    "company": "UnknownCo",
                    "persona": "GTM Engineering",
                    "segment": "agent-assisted GTM",
                    "source_kind": "synthetic-example",
                    "synthetic": true,
                    "trigger": "standardizing prospect qualification data",
                    "signals": [
                        {
                            "id": "qualification-data-standardization",
                            "title": "Standardizing prospect qualification data"
                        }
                    ]
                }
            }),
        ),
        (
            "account-only-no-draft.yaml",
            json!({
                "id": "account-only-no-draft",
                "command": "brief",
                "profile_eval": eval_profile(
                    "account-only-no-draft",
                    &["actors", "decision-criteria", "source-signals", "output-contracts", "gaps"],
                    &["outbound-copy-brief"]
                ),
                "channel": "linkedin",
                "job": "outbound-copy-brief",
                "expect_draft_status": "no-draft",
                "prospect": {
                    "name": "N/A",
                    "title": "N/A",
                    "company": "Northstar Cloud",
                    "company_domain": "northstarcloud.com",
                    "company_url": "https://northstarcloud.com",
                    "background": "account-only row says the company is standardizing qualification data across agent-assisted GTM workflows",
                    "trigger": "standardizing prospect qualification data before routing new campaigns",
                    "segment": "agent-assisted GTM",
                    "source_kind": "synthetic-example",
                    "synthetic": true,
                    "signals": [
                        {
                            "id": "qualification-data-standardization",
                            "title": "Standardizing prospect qualification data",
                            "source": "synthetic account-only row",
                            "confidence": "medium",
                            "freshness": "recent",
                            "state_as": "supplied"
                        }
                    ]
                }
            }),
        ),
        (
            "unknown-task-route.yaml",
            json!({
                "id": "unknown-task-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "job-routing",
                    &["actors", "boundaries", "output-contracts"],
                    &["outbound-copy-review"]
                ),
                "persona": "Unknown",
                "job": "task hygiene",
                "expect_load_order_contains": [
                    ".mdp/cards/personas.yaml",
                    ".mdp/cards/avoid-rules.yaml",
                    ".mdp/cards/output-rules.yaml"
                ],
                "expect_load_order_excludes": [
                    ".mdp/cards/ctas.yaml",
                    ".mdp/cards/gaps.yaml"
                ]
            }),
        ),
        (
            "unsupported-persona-route.yaml",
            json!({
                "id": "unsupported-persona-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "job-routing",
                    &["actors", "boundaries", "output-contracts"],
                    &["outbound-copy-brief"]
                ),
                "persona": "Sales Development",
                "job": "linkedin outbound copy",
                "expect_load_order_contains": [
                    ".mdp/cards/personas.yaml",
                    ".mdp/cards/avoid-rules.yaml",
                    ".mdp/cards/output-rules.yaml",
                    ".mdp/cards/ctas.yaml"
                ]
            }),
        ),
        (
            "claim-check-output-rule.yaml",
            json!({
                "id": "claim-check-output-rule",
                "command": "check-claims",
                "profile_eval": eval_profile(
                    "unsafe-output",
                    &["output-contracts", "boundaries"],
                    &["outbound-copy-brief"]
                ),
                "text": "MDP is local — it stores message context in modular cards.",
                "expect_valid": false
            }),
        ),
        (
            "fit-good.yaml",
            json!({
                "id": "fit-good",
                "command": "fit",
                "job": "prospect-fit-or-brief",
                "profile_eval": eval_profile(
                    "proceed",
                    &["actors", "decision-criteria", "source-signals"],
                    &["prospect-fit-or-brief"]
                ),
                "expect_status": "insufficient-context",
                "prospect": starter_prospect("gtm")
            }),
        ),
        (
            "fit-insufficient-context.yaml",
            json!({
                "id": "fit-insufficient-context",
                "command": "fit",
                "job": "prospect-fit-or-brief",
                "profile_eval": eval_profile(
                    "insufficient-context",
                    &["decision-criteria", "source-signals", "gaps"],
                    &["prospect-fit-or-brief"]
                ),
                "expect_status": "insufficient-context",
                "prospect": {
                    "name": "Taylor Lee",
                    "title": "Revenue Operations Lead",
                    "company": "ExampleCo"
                }
            }),
        ),
        (
            "fit-disqualified.yaml",
            json!({
                "id": "fit-disqualified",
                "command": "fit",
                "job": "prospect-fit-or-brief",
                "profile_eval": eval_profile(
                    "refusal",
                    &["decision-criteria", "boundaries"],
                    &["prospect-fit-or-brief"]
                ),
                "expect_status": "disqualified",
                "prospect": {
                    "name": "Jordan Smith",
                    "title": "Growth Lead",
                    "company": "BlastCo",
                    "persona": "GTM Engineering",
                    "segment": "agent-assisted GTM",
                    "trigger": "sequence everyone with auto-send",
                    "signals": [{"id": "sending-only", "title": "Wants auto-send", "source": "example row"}]
                }
            }),
        ),
        (
            "fit-negated-execution-boundary.yaml",
            json!({
                "id": "fit-negated-execution-boundary",
                "command": "fit",
                "job": "prospect-fit-or-brief",
                "profile_eval": eval_profile(
                    "proceed",
                    &["decision-criteria", "boundaries", "source-signals"],
                    &["prospect-fit-or-brief"]
                ),
                "expect_status": "insufficient-context",
                "prospect": {
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
                }
            }),
        ),
        (
            "brief-insufficient-context.yaml",
            json!({
                "id": "brief-insufficient-context",
                "command": "brief",
                "profile_eval": eval_profile(
                    "insufficient-context",
                    &["actors", "source-signals", "output-contracts", "gaps"],
                    &["outbound-copy-brief"]
                ),
                "channel": "linkedin",
                "job": "outbound-copy-brief",
                "expect_draft_status": "no-draft",
                "prospect": {
                    "name": "Taylor Lee",
                    "title": "Revenue Operations Lead",
                    "company": "ExampleCo"
                }
            }),
        ),
        (
            "claim-check-unsupported.yaml",
            json!({
                "id": "claim-check-unsupported",
                "command": "check-claims",
                "profile_eval": eval_profile(
                    "unsafe-output",
                    &["evidence-proof", "boundaries", "output-contracts"],
                    &["outbound-copy-brief"]
                ),
                "text": "MDP guarantees meetings, improves reply rates by 30%, integrates with Salesforce, and updates CRM records.",
                "expect_valid": false
            }),
        ),
        (
            "claim-check-execution-positive.yaml",
            json!({
                "id": "claim-check-execution-positive",
                "command": "check-claims",
                "profile_eval": eval_profile(
                    "unsafe-output",
                    &["evidence-proof", "boundaries", "output-contracts"],
                    &["outbound-copy-brief"]
                ),
                "text": "MDP can auto-send the campaign and send emails after the brief is approved.",
                "expect_valid": false,
                "expect_unsupported_claims_contains": ["execution"]
            }),
        ),
        (
            "claim-check-negated-execution-boundary.yaml",
            json!({
                "id": "claim-check-negated-execution-boundary",
                "command": "check-claims",
                "profile_eval": eval_profile(
                    "proceed",
                    &["evidence-proof", "boundaries", "output-contracts"],
                    &["outbound-copy-brief"]
                ),
                "text": "MDP is not an AI SDR. It does not auto-send. It does not send emails.",
                "expect_valid": true
            }),
        ),
        (
            "claim-check-customer-proof.yaml",
            json!({
                "id": "claim-check-customer-proof",
                "command": "check-claims",
                "profile_eval": eval_profile(
                    "unsafe-output",
                    &["evidence-proof", "boundaries", "output-contracts"],
                    &["outbound-copy-brief"]
                ),
                "text": "The attributes prove MDP customers already use the pack in production use with a design partner and paid pilot.",
                "expect_valid": false,
                "expect_guardrail_terms_contains": [
                    "customers already use",
                    "design partner",
                    "paid pilot",
                    "production use"
                ],
                "expect_unsupported_claims_contains": ["customer proof"]
            }),
        ),
        (
            "claim-check-commercial-traction.yaml",
            json!({
                "id": "claim-check-commercial-traction",
                "command": "check-claims",
                "profile_eval": eval_profile(
                    "unsafe-output",
                    &["evidence-proof", "boundaries", "output-contracts"],
                    &["outbound-copy-brief"]
                ),
                "text": "MDP has validated adoption, ARR conversion, workshops converted into customers, and is market validated.",
                "expect_valid": false,
                "expect_guardrail_terms_contains": [
                    "validated adoption",
                    "ARR conversion",
                    "workshops converted",
                    "market validated"
                ],
                "expect_unsupported_claims_contains": ["commercial traction"]
            }),
        ),
        (
            "claim-check-adversarial-variants.yaml",
            json!({
                "id": "claim-check-adversarial-variants",
                "command": "check-claims",
                "profile_eval": eval_profile(
                    "unsafe-output",
                    &["evidence-proof", "boundaries", "output-contracts"],
                    &["outbound-copy-brief"]
                ),
                "text": "MDP is security-approved, connects to your CRM, books meetings, doubles reply rates, customers rely on it in production, I loved your recent LinkedIn post, bypasses procurement, AI can own the response, replaces proposal management software, and is best-in-class.",
                "expect_valid": false,
                "expect_guardrail_terms_contains": [
                    "security-approved",
                    "connects to your CRM",
                    "books meetings",
                    "doubles reply rates",
                    "customers rely on",
                    "I loved your recent LinkedIn post",
                    "bypasses procurement",
                    "AI can own the response",
                    "replaces proposal management software",
                    "best-in-class"
                ],
                "expect_unsupported_claims_contains": [
                    "compliance-security",
                    "integration",
                    "quantified-outcome",
                    "customer-name",
                    "execution-crm-sending",
                    "legal-procurement-bypass",
                    "ai-authoritative",
                    "rfp-platform-replacement",
                    "fake-personalization",
                    "weak-trust"
                ]
            }),
        ),
        (
            "claim-check-safe-adjacent.yaml",
            json!({
                "id": "claim-check-safe-adjacent",
                "command": "check-claims",
                "profile_eval": eval_profile(
                    "proceed",
                    &["evidence-proof", "boundaries"],
                    &["outbound-copy-review"]
                ),
                "text": "MDP is local-first AI-assisted decision context before drafting with approved claims, evidence, and review rules. It does not send emails, does not connect to your CRM, does not update CRM records, does not bypass legal, does not replace proposal management software, and does not claim compliance approval.",
                "expect_valid": true
            }),
        ),
        (
            "claim-check-category-overreach.yaml",
            json!({
                "id": "claim-check-category-overreach",
                "command": "check-claims",
                "profile_eval": eval_profile(
                    "unsafe-output",
                    &["evidence-proof", "boundaries", "output-contracts"],
                    &["outbound-copy-brief"]
                ),
                "text": "MDP is a graph engineering platform, graph database, agent runtime, memory layer, persistent memory layer, orchestration framework, and universal company graph that proves source truth.",
                "expect_valid": false,
                "expect_guardrail_terms_contains": [
                    "graph engineering platform",
                    "graph database",
                    "agent runtime",
                    "orchestration framework",
                    "persistent memory layer",
                    "memory layer",
                    "source truth",
                    "universal company graph",
                    "proves source truth"
                ]
            }),
        ),
        (
            "claim-check-coordinated-safe-boundary.yaml",
            json!({
                "id": "claim-check-coordinated-safe-boundary",
                "command": "check-claims",
                "profile_eval": eval_profile(
                    "proceed",
                    &["evidence-proof", "boundaries"],
                    &["outbound-copy-review"]
                ),
                "text": "MDP is local-first decision context. It does not update CRM records, send emails, or bypass legal review.",
                "expect_valid": true
            }),
        ),
        (
            "claim-check-approved.yaml",
            json!({
                "id": "claim-check-approved",
                "command": "check-claims",
                "profile_eval": eval_profile(
                    "proceed",
                    &["evidence-proof", "boundaries"],
                    &["outbound-copy-review"]
                ),
                "text": "MDP is versioned decision context for agents. It is a local offline CLI, and each pack declares a version in its manifest alongside modular card references.",
                "expect_valid": true
            }),
        ),
        (
            "portfolio-local-cli-route.yaml",
            json!({
                "id": "portfolio-local-cli-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "job-routing",
                    &["actors", "output-contracts", "routing-jobs"],
                    &["outbound-copy-brief"]
                ),
                "persona": "PMM",
                "job": "portfolio scope example",
                "expect_draft_status": "ready",
                "scope": ["product=local-cli"],
                "expect_entry_titles_contains": ["Scope qualifies primitives", "Local CLI portfolio angle"],
                "expect_entry_titles_excludes": ["Codex plugin portfolio angle", "Portfolio routing capability angle"]
            }),
        ),
        (
            "portfolio-codex-plugin-route.yaml",
            json!({
                "id": "portfolio-codex-plugin-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "job-routing",
                    &["actors", "output-contracts", "routing-jobs"],
                    &["outbound-copy-brief"]
                ),
                "persona": "PMM",
                "job": "portfolio scope example",
                "expect_draft_status": "ready",
                "scope": ["product=agent-plugin"],
                "expect_entry_titles_contains": ["Scope qualifies primitives", "Codex plugin portfolio angle"],
                "expect_entry_titles_excludes": ["Local CLI portfolio angle", "Portfolio routing capability angle"]
            }),
        ),
        (
            "portfolio-missing-scope-route.yaml",
            json!({
                "id": "portfolio-missing-scope-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "insufficient-context",
                    &["actors", "output-contracts", "routing-jobs", "gaps"],
                    &["outbound-copy-brief"]
                ),
                "persona": "PMM",
                "job": "portfolio scope example",
                "expect_draft_status": "blocked",
                "expect_entry_gap_reasons_contains": ["scope_dimension_missing"]
            }),
        ),
        (
            "portfolio-local-cli-brief.yaml",
            json!({
                "id": "portfolio-local-cli-brief",
                "command": "brief",
                "profile_eval": eval_profile(
                    "proceed",
                    &["actors", "source-signals", "output-contracts", "routing-jobs"],
                    &["prospect-fit-or-brief", "outbound-copy-brief"]
                ),
                "channel": "linkedin",
                "job": "outbound-copy-brief",
                "prospect": portfolio_eval_prospect("local-cli"),
                "expect_draft_status": "no-draft"
            }),
        ),
        (
            "portfolio-agent-plugin-brief.yaml",
            json!({
                "id": "portfolio-agent-plugin-brief",
                "command": "brief",
                "profile_eval": eval_profile(
                    "proceed",
                    &["actors", "source-signals", "output-contracts", "routing-jobs"],
                    &["prospect-fit-or-brief", "outbound-copy-brief"]
                ),
                "channel": "linkedin",
                "job": "outbound-copy-brief",
                "prospect": portfolio_eval_prospect("agent-plugin"),
                "expect_draft_status": "no-draft"
            }),
        ),
        (
            "portfolio-missing-scope-brief.yaml",
            json!({
                "id": "portfolio-missing-scope-brief",
                "command": "brief",
                "profile_eval": eval_profile(
                    "insufficient-context",
                    &["actors", "source-signals", "output-contracts", "routing-jobs", "gaps"],
                    &["prospect-fit-or-brief", "outbound-copy-brief"]
                ),
                "channel": "linkedin",
                "job": "outbound-copy-brief",
                "prospect": portfolio_eval_prospect_without_product(),
                "expect_draft_status": "no-draft"
            }),
        ),
    ]
}

pub(crate) fn starter_evals() -> Vec<(&'static str, Value)> {
    generated_starter_evals()
        .into_iter()
        .map(|(filename, mut eval)| {
            match eval["id"].as_str() {
                Some("decision-input-contract") => {
                    eval["expect_available"] = json!(false);
                    let object = eval.as_object_mut().expect("eval fixture is an object");
                    object.remove("expect_runtime_contract_version");
                    object.remove("expect_attempt_statuses");
                    object.remove("expect_no_draft_outcomes");
                }
                Some("fit-good")
                | Some("fit-negated-execution-boundary")
                | Some("account-context-present") => {
                    eval["expect_status"] = json!("fit");
                    eval.as_object_mut()
                        .expect("eval fixture is an object")
                        .remove("job");
                }
                Some("fit-insufficient-context") | Some("account-context-missing") => {
                    eval["expect_status"] = json!("insufficient-context");
                    eval.as_object_mut()
                        .expect("eval fixture is an object")
                        .remove("job");
                }
                Some("fit-disqualified") => {
                    eval["expect_status"] = json!("disqualified");
                    eval.as_object_mut()
                        .expect("eval fixture is an object")
                        .remove("job");
                }
                Some("brief-insufficient-context") | Some("account-only-no-draft") => {
                    eval["job"] = json!("linkedin outbound copy");
                }
                Some("linkedin-copy-route")
                | Some("linkedin-follow-up-route")
                | Some("email-initial-route")
                | Some("email-follow-up-route")
                | Some("portfolio-local-cli-route")
                | Some("portfolio-codex-plugin-route") => {
                    eval["expect_draft_status"] = json!("ready");
                }
                Some("portfolio-local-cli-brief") | Some("portfolio-agent-plugin-brief") => {
                    eval["job"] = json!("portfolio scope example");
                    eval["expect_draft_status"] = json!("ready");
                }
                Some("portfolio-missing-scope-brief") => {
                    eval["job"] = json!("portfolio scope example");
                }
                _ => {}
            }
            (filename, eval)
        })
        .collect()
}

pub(crate) fn decision_input_contract_eval() -> (&'static str, Value) {
    (
        "decision-input-contract.yaml",
        json!({
            "id": "decision-input-contract",
            "command": "requirements",
            "profile_eval": eval_profile(
                "prompt-output-validation",
                &["source-signals", "decision-criteria", "boundaries", "gaps"],
                &["prospect-fit-or-brief"]
            ),
            "job": "prospect-fit-or-brief",
            "expect_available": true,
            "expect_runtime_contract_version": "v2",
            "expect_attempt_statuses": [
                "observed", "not_found", "not_applicable", "blocked", "error"
            ],
            "expect_no_draft_outcomes": [
                "insufficient-context", "disqualified", "human-review", "malformed", "provider-error"
            ]
        }),
    )
}

pub(crate) fn decision_input_scenarios() -> Value {
    json!({
        "contract": "mdp.decision-input-scenarios.v1",
        "synthetic": true,
        "decision_input_contract": "gtm.prospect-context@1.0.0",
        "runtime_contract_version": "v2",
        "normalized_schema_ref": "mdp.normalized-decision-input.v2",
        "note": "Provider-neutral expected outcomes only. Collection and provider execution remain host-owned.",
        "attempt_statuses": ["observed", "not_found", "not_applicable", "blocked", "error"],
        "scenarios": [
            {
                "id": "attempted-complete",
                "attempted_complete": true,
                "attempt_statuses": ["observed", "not_applicable"],
                "expected_outcome": "ready",
                "draft_allowed": false
            },
            {
                "id": "insufficient",
                "attempted_complete": true,
                "attempt_statuses": ["observed", "not_found"],
                "expected_outcome": "insufficient-context",
                "draft_allowed": false
            },
            {
                "id": "disqualified",
                "attempted_complete": true,
                "attempt_statuses": ["observed"],
                "expected_outcome": "disqualified",
                "draft_allowed": false
            },
            {
                "id": "human-review",
                "attempted_complete": true,
                "attempt_statuses": ["observed", "blocked"],
                "expected_outcome": "human-review",
                "draft_allowed": false
            },
            {
                "id": "malformed",
                "attempted_complete": false,
                "attempt_statuses": [],
                "expected_outcome": "malformed",
                "draft_allowed": false
            },
            {
                "id": "provider-error",
                "attempted_complete": true,
                "attempt_statuses": ["observed", "error"],
                "expected_outcome": "provider-error",
                "draft_allowed": false
            }
        ]
    })
}

fn portfolio_eval_prospect(product: &str) -> Value {
    let mut prospect = portfolio_eval_prospect_without_product();
    prospect["attributes"] = json!({"product": product});
    prospect
}

fn portfolio_eval_prospect_without_product() -> Value {
    json!({
        "name": "Alex Rivera",
        "title": "Revenue Operations Lead",
        "company": "ExampleCo",
        "company_domain": "example.com",
        "persona": "GTM Engineering",
        "segment": "agent-assisted GTM",
        "trigger": "standardizing outbound context before agents draft or route campaign briefs",
        "source_kind": "synthetic-example",
        "synthetic": true,
        "signals": [{
            "id": "portfolio-context-standardization",
            "title": "Standardizing portfolio messaging context",
            "source": "synthetic portfolio eval fixture",
            "confidence": "medium",
            "freshness": "recent",
            "state_as": "supplied"
        }]
    })
}

pub(crate) fn generated_starter_prompts(
    include_output_schemas: bool,
) -> Vec<(&'static str, Value)> {
    vec![
        (
            "normalize-prospect.yaml",
            prospect_normalization_prompt_contract(include_output_schemas),
        ),
        (
            "generate-outbound-copy.yaml",
            outbound_model_task_prompt(
                "outbound-copy-brief",
                "generate-outbound-copy-v1",
                "generation",
            ),
        ),
        (
            "review-outbound-copy.yaml",
            outbound_model_task_prompt("outbound-copy-review", "review-outbound-copy-v1", "review"),
        ),
        (
            "icp-persona.yaml",
            prompt_contract(
                "extract-icp-persona",
                "Extract ICP and persona candidates",
                "Turns supplied person, company, and account context into reviewable persona and ICP entries.",
                &["personas", "fit-rules"],
                &["prompt", "icp", "persona", "fit"],
                "Identify likely operator or buyer personas, account traits, and fit rules. If the input does not support a persona, company segment, or fit rule, emit a gap entry instead of guessing.",
                json!([
                    {
                        "card_id": "personas",
                        "kind": "personas",
                        "entries": [
                            prompt_entry(
                                "persona-gtm-ops",
                                "GTM operations",
                                "Supplied person, company, or account data suggests a team responsible for keeping outbound context and messaging rules consistent across agent-assisted workflows.",
                                &["PMM", "GTM Engineering"],
                                &["company_data"],
                                &[],
                                "low",
                                &["company_data: supplied company data"],
                                "needs-review"
                            )
                        ]
                    },
                    {
                        "card_id": "fit-rules",
                        "kind": "fit-rules",
                        "entries": [
                            prompt_entry(
                                "fit-agent-assisted-gtm",
                                "Possible fit: agent-assisted GTM",
                                "Use only if supplied sources show the company is building or standardizing agent-assisted GTM workflows.",
                                &["PMM", "GTM Engineering"],
                                &["company_data"],
                                &["no source", "no GTM workflow signal"],
                                "low",
                                &["company_data: supplied company data"],
                                "needs-review"
                            )
                        ]
                    }
                ]),
                &["company_data"],
                &[],
                include_output_schemas,
            ),
        ),
        (
            "pains.yaml",
            prompt_contract(
                "extract-pains",
                "Extract pain candidates",
                "Turns supplied person, company, and account context into reviewable pain and trigger entries.",
                &["pains"],
                &["prompt", "pain", "trigger"],
                "Extract pains only when the source material supports the problem. Phrase weak inferences as hypotheses and preserve missing evidence as gaps.",
                json!([
                    {
                        "card_id": "pains",
                        "kind": "pains",
                        "entries": [
                            prompt_entry(
                                "pain-context-drift",
                                "Possible pain: context drift",
                                "The supplied material suggests messaging decisions may be scattered across tools or agents, creating context drift risk.",
                                &["PMM", "GTM Engineering"],
                                &["company_data"],
                                &[],
                                "low",
                                &["company_data: supplied company data"],
                                "needs-review"
                            )
                        ]
                    }
                ]),
                &["company_data"],
                &[],
                include_output_schemas,
            ),
        ),
        (
            "hooks.yaml",
            prompt_contract(
                "extract-hooks",
                "Extract hook candidates",
                "Turns supplied person, company, and account context into sourced hook candidates for later message work.",
                &["hooks"],
                &["prompt", "hook", "angle"],
                "Create hooks only as reusable message angles, not final copy. Each hook must tie back to a source-backed signal or be marked as a gap.",
                json!([
                    {
                        "card_id": "hooks",
                        "kind": "hooks",
                        "entries": [
                            prompt_entry(
                                "hook-standardize-agent-context",
                                "Standardize agent context",
                                "Use when supplied context shows the company has multiple GTM tools or agents that need the same messaging truth.",
                                &["PMM"],
                                &["company_data"],
                                &[],
                                "low",
                                &["company_data: supplied company data"],
                                "needs-review"
                            )
                        ]
                    }
                ]),
                &["company_data"],
                &[],
                include_output_schemas,
            ),
        ),
        (
            "claims-proof.yaml",
            prompt_contract(
                "extract-claims-proof",
                "Extract claims and proof candidates",
                "Turns supplied person, company, account, and source material into reviewable claims without upgrading unsupported statements.",
                &["claims"],
                &["prompt", "claim", "proof", "evidence"],
                "Extract only claims directly supported by supplied source material. Put unsupported or quantified claims in rejected_claims, not card_patches.",
                json!([
                    {
                        "card_id": "claims",
                        "kind": "claims",
                        "entries": [
                            prompt_entry(
                                "claim-local-decision-context",
                                "Local decision context",
                                "Supplied source material describes the product as local decision context for GTM messaging.",
                                &["PMM", "GTM Engineering"],
                                &["source_notes"],
                                &[],
                                "medium",
                                &["source_notes: supplied source notes"],
                                "needs-review"
                            )
                        ]
                    }
                ]),
                &["source_notes"],
                &[],
                include_output_schemas,
            ),
        ),
        (
            "fit-rules.yaml",
            prompt_contract(
                "extract-fit-rules",
                "Extract fit and disqualification rules",
                "Turns supplied person, company, and account context into reviewable fit, no-message, and disqualification entries.",
                &["fit-rules"],
                &["prompt", "fit", "icp", "disqualifier"],
                "Separate positive fit signals from disqualifiers. If source material only supports a sending or scraping ask, mark it as out of scope for MDP.",
                json!([
                    {
                        "card_id": "fit-rules",
                        "kind": "fit-rules",
                        "entries": [
                            prompt_entry(
                                "fit-needs-message-context",
                                "Good fit: needs message context",
                                "Use when supplied context shows the account needs shared messaging decisions across agents, workflows, or teams.",
                                &["PMM", "GTM Engineering"],
                                &["company_data"],
                                &[],
                                "low",
                                &["company_data: supplied company data"],
                                "needs-review"
                            )
                        ]
                    }
                ]),
                &["company_data"],
                &[],
                include_output_schemas,
            ),
        ),
        (
            "avoid-rules.yaml",
            prompt_contract(
                "extract-avoid-rules",
                "Extract avoid rules",
                "Turns supplied person, company, and account context into reviewable category, claim, and wording guardrails.",
                &["avoid-rules"],
                &["prompt", "avoid", "guardrail"],
                "Extract avoid rules that prevent category confusion, unsafe claims, or unsupported copy. Do not turn product aspirations into approved claims.",
                json!([
                    {
                        "card_id": "avoid-rules",
                        "kind": "avoid-rules",
                        "entries": [
                            prompt_entry(
                                "avoid-unsupported-outcomes",
                                "Avoid unsupported outcomes",
                                "Do not claim quantified outcomes, customer proof, integrations, or execution capabilities unless supplied sources directly support them.",
                                &["PMM", "GTM Engineering"],
                                &["source_notes"],
                                &["guaranteed", "proven ROI", "auto-send"],
                                "medium",
                                &["source_notes: supplied source notes"],
                                "needs-review"
                            )
                        ]
                    }
                ]),
                &["source_notes"],
                &[],
                include_output_schemas,
            ),
        ),
        (
            "output-rules.yaml",
            prompt_contract(
                "extract-output-rules",
                "Extract output rules",
                "Turns supplied style guidance, editorial preferences, and channel constraints into reviewable output-rule entries.",
                &["output-rules"],
                &["prompt", "style", "format", "guardrail"],
                "Extract global style and output-structure rules for generated text. Put forbidden punctuation, phrases, or formats in avoid, and keep structural requirements in the body.",
                json!([
                    {
                        "card_id": "output-rules",
                        "kind": "output-rules",
                        "entries": [
                            prompt_entry(
                                "avoid-em-dashes",
                                "Avoid em dashes",
                                "Do not use em dashes in generated copy; use commas, periods, colons, or shorter sentences instead.",
                                &["PMM", "GTM Engineering"],
                                &["source_notes"],
                                &["—"],
                                "medium",
                                &["source_notes: supplied style guidance"],
                                "needs-review"
                            )
                        ]
                    }
                ]),
                &["source_notes"],
                &[],
                include_output_schemas,
            ),
        ),
        (
            "cta-channel-policy.yaml",
            prompt_contract(
                "extract-cta-channel-policy",
                "Extract CTA and channel policy",
                "Turns supplied person, company, and account context into reviewable CTA rules and channel boundaries.",
                &["ctas", "channel-policies"],
                &["prompt", "cta", "channel"],
                "Extract CTA and channel rules for handoff only. Do not imply sending, sequencing, CRM updates, or external-system execution.",
                json!([
                    {
                        "card_id": "ctas",
                        "kind": "ctas",
                        "entries": [
                            prompt_entry(
                                "cta-routing-question",
                                "Routing question",
                                "When fit is plausible but not proven, ask a routing question that identifies the owner or current workflow before asking for a meeting.",
                                &["PMM"],
                                &["company_data"],
                                &[],
                                "low",
                                &["company_data: supplied company data"],
                                "needs-review"
                            )
                        ]
                    },
                    {
                        "card_id": "channel-policies",
                        "kind": "channel-policies",
                        "entries": [
                            prompt_entry(
                                "channel-agent-brief",
                                "Agent brief",
                                "Return fit status, loaded card candidates, supported claims, avoid rules, and gaps. Do not send or update external systems.",
                                &["GTM Engineering", "PMM"],
                                &["source_notes"],
                                &[],
                                "medium",
                                &["source_notes: supplied source notes"],
                                "needs-review"
                            )
                        ]
                    }
                ]),
                &["company_data", "source_notes"],
                &[],
                include_output_schemas,
            ),
        ),
        (
            "gaps.yaml",
            prompt_contract(
                "extract-gaps",
                "Extract durable gaps",
                "Turns missing or weak person, company, account, or source context into explicit gaps instead of invented pack entries.",
                &["gaps"],
                &["prompt", "gap", "unknown"],
                "List missing data that blocks confident card entries. Prefer gaps over weak claims whenever source support is absent.",
                json!([
                    {
                        "card_id": "gaps",
                        "kind": "gaps",
                        "entries": [
                            prompt_entry(
                                "gap-company-proof",
                                "Missing company proof",
                                "N/A",
                                &["PMM", "GTM Engineering"],
                                &[],
                                &[],
                                "unknown",
                                &[],
                                "gap"
                            )
                        ]
                    }
                ]),
                &[],
                &["Need concrete source material before creating approved claims."],
                include_output_schemas,
            ),
        ),
    ]
}

pub(crate) fn starter_prospect(_template: &str) -> Value {
    json!({
        "name": "Alex Rivera",
        "title": "Revenue Operations Lead",
        "company": "ExampleCo",
        "company_domain": "example.com",
        "source_kind": "synthetic-example",
        "synthetic": true,
        "linkedin_url": "https://www.linkedin.com/in/example-mdp-demo",
        "company_url": "https://example.com",
        "background": "synthetic RevOps owner evaluating repeatable agent-assisted GTM workflows across source rows, Codex, and review notes",
        "trigger": "standardizing outbound context before agents draft or route campaign briefs",
        "persona": "GTM Engineering",
        "segment": "agent-assisted GTM",
        "signals": [
            {
                "id": "revops-owner-context-standardization",
                "title": "RevOps owner standardizing campaign context",
                "source": "synthetic example row",
                "confidence": "medium",
                "freshness": "recent",
                "state_as": "hypothesis"
            }
        ]
    })
}

fn card_ref(
    id: &str,
    path: &str,
    kind: CardKind,
    description: &str,
    personas: &[&str],
    tags: &[&str],
) -> CardRef {
    CardRef {
        id: id.to_string(),
        path: path.to_string(),
        kind,
        description: description.to_string(),
        personas: personas.iter().map(|s| s.to_string()).collect(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
    }
}

fn persona_mapping(persona: &str, title_keywords: &[&str]) -> PersonaMapping {
    PersonaMapping {
        persona: persona.to_string(),
        title_keywords: title_keywords
            .iter()
            .map(|value| value.to_string())
            .collect(),
    }
}

fn card(
    id: &str,
    kind: CardKind,
    title: &str,
    description: &str,
    personas: &[&str],
    tags: &[&str],
    entries: Vec<Entry>,
) -> Card {
    Card {
        id: id.to_string(),
        kind,
        title: title.to_string(),
        description: description.to_string(),
        personas: personas.iter().map(|s| s.to_string()).collect(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
        entries,
    }
}

fn entry(id: &str, title: &str, body: &str, applies_to: &[&str]) -> Entry {
    Entry {
        id: id.to_string(),
        title: title.to_string(),
        body: body.to_string(),
        applies_to: applies_to.iter().map(|s| s.to_string()).collect(),
        scope: BTreeMap::new(),
        evidence: vec![],
        avoid: vec![],
        exact_paragraphs: None,
        constraints: EntryConstraints::default(),
        metadata: BTreeMap::new(),
    }
}

fn scoped_entry(
    id: &str,
    title: &str,
    body: &str,
    applies_to: &[&str],
    scope: &[(&str, &[&str])],
) -> Entry {
    Entry {
        id: id.to_string(),
        title: title.to_string(),
        body: body.to_string(),
        applies_to: strings(applies_to),
        scope: scope
            .iter()
            .map(|(dimension, values)| ((*dimension).to_string(), strings(values)))
            .collect(),
        evidence: vec![],
        avoid: vec![],
        exact_paragraphs: None,
        constraints: EntryConstraints::default(),
        metadata: BTreeMap::new(),
    }
}

fn entry_with_evidence(
    id: &str,
    title: &str,
    body: &str,
    applies_to: &[&str],
    evidence: &[&str],
) -> Entry {
    Entry {
        id: id.to_string(),
        title: title.to_string(),
        body: body.to_string(),
        applies_to: applies_to.iter().map(|s| s.to_string()).collect(),
        scope: BTreeMap::new(),
        evidence: evidence.iter().map(|s| s.to_string()).collect(),
        avoid: vec![],
        exact_paragraphs: None,
        constraints: EntryConstraints::default(),
        metadata: BTreeMap::new(),
    }
}

fn initial_email_constraints() -> EntryConstraints {
    EntryConstraints {
        word_count: Some(CountConstraint {
            min: Some(50),
            max: Some(125),
            target_min: Some(75),
            target_max: Some(110),
        }),
        subject_words: Some(CountConstraint {
            min: Some(3),
            max: Some(6),
            target_min: None,
            target_max: None,
        }),
        subject_avoid: vec![
            "Re:".to_string(),
            "Fwd:".to_string(),
            "urgent".to_string(),
            "quick question".to_string(),
        ],
        max_questions: Some(1),
        forbid_links: true,
        forbid_attachments: true,
        forbid_images: true,
        forbid_html: true,
        forbid_tracking: true,
        proof_output: Default::default(),
    }
}

fn outbound_model_task_prompt(job_id: &str, id: &str, kind: &str) -> Value {
    let is_review = kind == "review";
    let objective = if is_review {
        "Evaluate supplied outbound copy against the exact routed context, selected claims and evidence, CTA, and output constraints."
    } else {
        "Produce one structured outbound copy draft from the exact routed context and declared runtime inputs."
    };
    let artifact_schema = if is_review {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["status", "decision", "issues", "accepted_claim_ids", "accepted_evidence_ids"],
            "properties": {
                "status": {"enum": ["ready", "gap", "refused"]},
                "decision": {"enum": ["approve", "revise", "reject"]},
                "issues": {"type": "array", "items": {"type": "string"}},
                "accepted_claim_ids": {"type": "array", "items": {"type": "string"}},
                "accepted_evidence_ids": {"type": "array", "items": {"type": "string"}}
            }
        })
    } else {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["status", "angle_id", "cta_id", "claim_ids", "evidence_ids", "subject_options", "message_body"],
            "properties": {
                "status": {"enum": ["ready", "gap", "refused"]},
                "angle_id": {"type": "string"},
                "cta_id": {"type": "string"},
                "claim_ids": {"type": "array", "items": {"type": "string"}},
                "evidence_ids": {"type": "array", "items": {"type": "string"}},
                "subject_options": {"type": "array", "items": {"type": "string"}},
                "message_body": {"type": "string"}
            },
            "allOf": [{
                "if": {"properties": {"status": {"const": "ready"}}, "required": ["status"]},
                "then": {
                    "properties": {
                        "angle_id": {"type": "string", "minLength": 1, "not": {"const": "N/A"}},
                        "cta_id": {"type": "string", "minLength": 1, "not": {"const": "N/A"}},
                        "claim_ids": {"type": "array", "minItems": 1, "items": {"type": "string", "minLength": 1}},
                        "evidence_ids": {"type": "array", "minItems": 1, "items": {"type": "string", "minLength": 1}},
                        "subject_options": {"type": "array", "minItems": 1, "items": {"type": "string", "minLength": 1}},
                        "message_body": {"type": "string", "minLength": 1, "not": {"const": "N/A"}}
                    }
                }
            }]
        })
    };
    let example_artifact = if is_review {
        json!({
            "status": "gap",
            "decision": "revise",
            "issues": ["The supplied draft uses a claim that is not selected authority."],
            "accepted_claim_ids": [],
            "accepted_evidence_ids": []
        })
    } else {
        json!({
            "status": "gap",
            "angle_id": "N/A",
            "cta_id": "N/A",
            "claim_ids": [],
            "evidence_ids": [],
            "subject_options": [],
            "message_body": "N/A"
        })
    };
    let mut schema_properties = serde_json::Map::new();
    schema_properties.insert(
        "contract".to_string(),
        json!({"const": PROMPT_OUTPUT_CONTRACT}),
    );
    schema_properties.insert("prompt_id".to_string(), json!({"const": id}));
    schema_properties.insert("job_id".to_string(), json!({"const": job_id}));
    schema_properties.insert("prompt_version".to_string(), json!({"const": "2"}));
    schema_properties.insert(
        "prompt_sha256".to_string(),
        json!({"type": "string", "pattern": "^[0-9a-f]{64}$"}),
    );
    schema_properties.insert(
        "invocation_receipt_sha256".to_string(),
        json!({"type": "string", "pattern": "^[0-9a-f]{64}$"}),
    );
    schema_properties.insert(
        "context_sha256".to_string(),
        json!({"type": "string", "pattern": "^[0-9a-f]{64}$"}),
    );
    let declared_input_names = if is_review {
        json!([
            "routed_context",
            "normalized_prospect",
            "runtime_context",
            "prompt_receipt",
            "invocation_receipt_sha256",
            "supplied_draft"
        ])
    } else {
        json!([
            "routed_context",
            "normalized_prospect",
            "runtime_context",
            "prompt_receipt",
            "invocation_receipt_sha256"
        ])
    };
    schema_properties.insert(
        "source_summary".to_string(),
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["inputs_used"],
            "properties": {
                "inputs_used": {
                    "type": "array",
                    "items": {"enum": declared_input_names}
                }
            }
        }),
    );
    schema_properties.insert(
        "selected_authority".to_string(),
        json!({"type": "array", "items": {"type": "string"}}),
    );
    schema_properties.insert("artifact".to_string(), artifact_schema);
    schema_properties.insert(
        "gaps".to_string(),
        json!({"type": "array", "items": {"type": "string"}}),
    );
    schema_properties.insert(
        "rejected_claims".to_string(),
        json!({"type": "array", "items": {"type": "string"}}),
    );
    json!({
        "format": PROMPT_FORMAT_V1,
        "id": id,
        "version": "2",
        "kind": kind,
        "title": if is_review { "Review outbound copy" } else { "Generate outbound copy" },
        "description": objective,
        "role": if is_review { "Governed outbound copy reviewer" } else { "Governed outbound copy writer" },
        "objective": objective,
        "target_card_kinds": ["positioning", "personas", "pains", "claims", "avoid-rules", "output-rules", "ctas"],
        "tags": ["prompt", "model-task", "outbound", kind],
        "inputs": outbound_model_task_inputs(is_review),
        "instructions": [
            "Use only declared inputs and the exact mdp.routed-context.v1 authority for this job.",
            "Return strict JSON only, preserve exact selected authority identifiers, and echo the exact invocation receipt SHA-256 supplied by the host.",
            "Echo context_sha256 exactly from the SHA-256 recorded for routed_context in prompt_receipt.inputs; do not recalculate or invent it.",
            "If evidence or authority is insufficient, return structured gaps or refusal instead of inventing facts."
        ],
        "procedure": ["Confirm the job, prompt version, declared inputs, and selected authority.", "Apply the pack-owned selection and evidence rules.", "Return the exact governed artifact schema."],
        "selection_rules": ["Choose only angle, CTA, claim, and evidence identifiers present in selected authority.", "Select at most one card-qualified authority reference for each bare artifact identifier.", "Never load the whole pack or borrow authority from another job."],
        "ambiguity_policy": ["Represent missing or conflicting facts in gaps and use a bounded non-success status."],
        "provenance_policy": ["Retain the exact authority identifiers used to produce or review the artifact.", "Treat prompt_receipt as the exact receipt content and echo the separately supplied invocation_receipt_sha256; the receipt cannot contain its own hash.", "Echo context_sha256 exactly from the SHA-256 recorded for routed_context in prompt_receipt.inputs; do not recalculate or invent it."],
        "evidence_policy": ["Do not state a claim unless its selected evidence supports it; generated text must still pass mdp verify-output."],
        "negative_examples": ["Do not invent customer proof, integrations, outcomes, timing, or recipient facts.", "Do not silently choose an undeclared claim or CTA."],
        "final_checklist": ["Output is strict JSON.", "prompt_sha256 matches the host-provided canonical prompt hash.", "invocation_receipt_sha256 exactly echoes the separately supplied host value for the exact prompt_receipt bytes.", "context_sha256 exactly echoes the SHA-256 recorded for routed_context in prompt_receipt.inputs without recalculation or invention.", "All selected identifiers are declared and unambiguous.", "Gaps and rejected claims are explicit.", "Generated copy is substantive before status is ready and remains ready for separate verify-output validation."],
        "output_contract": {
            "contract": PROMPT_OUTPUT_CONTRACT,
            "output_kind": "governed-artifact",
            "strict_json_only": true,
            "required_top_level": ["contract", "prompt_id", "job_id", "prompt_version", "prompt_sha256", "invocation_receipt_sha256", "context_sha256", "source_summary", "selected_authority", "artifact", "gaps", "rejected_claims"],
            "entry_defaults": {"body": "N/A", "applies_to": [], "evidence": [], "avoid": [], "confidence": "unknown", "provenance": []},
            "schema": {"type": "object", "additionalProperties": false, "required": ["contract", "prompt_id", "job_id", "prompt_version", "prompt_sha256", "invocation_receipt_sha256", "context_sha256", "source_summary", "selected_authority", "artifact", "gaps", "rejected_claims"], "properties": schema_properties},
            "example": {"contract": PROMPT_OUTPUT_CONTRACT, "prompt_id": id, "job_id": job_id, "prompt_version": "2", "prompt_sha256": "0000000000000000000000000000000000000000000000000000000000000000", "invocation_receipt_sha256": "0000000000000000000000000000000000000000000000000000000000000000", "context_sha256": "0000000000000000000000000000000000000000000000000000000000000000", "source_summary": {"inputs_used": []}, "selected_authority": [], "artifact": example_artifact, "gaps": if is_review { json!(["Supplied draft needs revision before approval."]) } else { json!(["Insufficient selected evidence for a claim-backed draft."]) }, "rejected_claims": []}
        }
    })
}

fn outbound_model_task_inputs(is_review: bool) -> Value {
    let mut inputs = vec![
        json!({"name": "routed_context", "description": "Exact canonical mdp.routed-context.v1 object compiled for this job.", "required": true, "default": "N/A", "missing_behavior": "Return a gap or refusal; never load unrelated pack entries.", "producer": "pack"}),
        json!({"name": "normalized_prospect", "description": "Validated prospect or account context.", "required": true, "default": "N/A", "missing_behavior": "Return a gap; do not invent a recipient, company, trigger, or persona.", "producer": "prior-step"}),
        json!({"name": "runtime_context", "description": "Optional bounded date and channel metadata.", "required": false, "default": "N/A", "missing_behavior": "Avoid time-sensitive framing that is not supplied.", "producer": "runtime"}),
        json!({"name": "prompt_receipt", "description": "Host-produced mdp.prompt-invocation.v1 receipt binding the canonical prompt and per-input SHA-256 values.", "required": true, "default": "N/A", "missing_behavior": "Return a gap or refusal; never invent prompt or input receipt hashes.", "producer": "host"}),
        json!({"name": "invocation_receipt_sha256", "description": "Host-produced detached SHA-256 of the exact prompt_receipt bytes, supplied separately because a receipt cannot contain its own hash.", "required": true, "default": "N/A", "missing_behavior": "Return a gap or refusal; never calculate or invent the detached receipt hash.", "producer": "host"}),
    ];
    if is_review {
        inputs.push(json!({"name": "supplied_draft", "description": "Copy supplied for review.", "required": true, "default": "N/A", "missing_behavior": "Return a gap when no draft is supplied.", "producer": "host"}));
    }
    Value::Array(inputs)
}

fn legacy_prospect_normalization_prompt_contract(include_output_schemas: bool) -> Value {
    let mut prompt = json!({
        "format": PROMPT_FORMAT_V1,
        "id": "normalize-prospect-row",
        "version": "1",
        "kind": "normalization",
        "title": "Normalize prospect row",
        "description": "Turns a supplied messy person, company, account, CRM, CSV, Clay, Deepline, spreadsheet, or research row into provider-neutral MDP prospect JSON before mdp fit or brief runs.",
        "target_card_kinds": ["personas", "fit-rules", "signals"],
        "tags": ["prompt", "normalization", "prospect", "fit", "routing"],
        "inputs": [
            {
                "name": "raw_row",
                "description": "The full messy source row, note, webhook payload, CSV row, CRM export row, Clay/Deepline row, spreadsheet row, or pasted research record.",
                "required": true,
                "default": "N/A",
                "missing_behavior": "Return gaps and do not create normalized_prospect fields from absent source material.",
                "producer": "source"
            },
            {
                "name": "company_domain",
                "description": "Company domain when available.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Use N/A and do not infer company identity from absent data.",
                "producer": "host"
            },
            {
                "name": "existing_pack_context",
                "description": "Relevant manifest personas, persona_mappings, lead_input_requirements.value_contracts, lead_input_requirements.attribute_definitions, fit rules, signal definitions, avoid-rules, output rules, and source policy from this MDP.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Do not assume pack-owned persona mappings, value domains, fit rules, attributes, or signal names when this field is N/A.",
                "producer": "pack"
            },
            {
                "name": "runtime_context",
                "description": "Optional MDP runtime context with now_utc, date_utc, timezone UTC, and local_time_policy. Use it only for temporal framing; fiscal year, renewal dates, event dates, and campaign windows remain pack-declared or supplied metadata.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Do not infer fiscal years, renewal windows, event timing, or local business calendar facts from missing runtime context.",
                "producer": "runtime"
            },
            {
                "name": "source_kind",
                "description": "Provider-neutral source marker such as user-provided-row, csv-row, crm-export-row, clay-row, deepline-row, private-scratch-row, sanitized-example, or synthetic-example.",
                "required": false,
                "default": "user-provided-row",
                "missing_behavior": "Use user-provided-row unless the caller supplies a more specific source kind.",
                "producer": "host"
            }
        ],
        "instructions": [
            "Use only raw_row, company_domain, existing_pack_context, runtime_context, and source_kind. Do not browse, scrape, enrich, send, sequence, update a CRM, or call external systems from this normalization prompt contract.",
            "Return strict JSON only. Do not wrap the response in markdown, prose, comments, or code fences.",
            "Set normalized_prospect to the exact provider-neutral shape accepted by mdp --json schema prospect: name, title, company, optional company_domain, source_kind, synthetic, linkedin_url, company_url, background, trigger, persona, segment, signals, and bounded attributes.",
            "When company_domain or company_url is supplied, normalize only that supplied domain-like value. Do not infer a domain from company name.",
            "Use runtime_context.now_utc and runtime_context.date_utc only to state when this normalization ran or to compare against explicitly supplied timing metadata. Do not hardcode fiscal year or infer customer-specific calendars from the current date.",
            "When existing_pack_context includes lead_input_requirements.value_contracts, emit only values allowed by those pack-owned enum/type/format contracts for persona, segment, source_kind, and other normalized scalar fields. If the source value is outside the contract, omit the optional field or add a gap instead of inventing a synonym.",
            "Use explicit persona from the row only when it already matches a pack-owned persona. Otherwise use pack-owned persona_mappings from existing_pack_context and emit the canonical persona label; if no pack-owned mapping applies, omit persona and add a gap instead of guessing.",
            "Use attributes only for bounded reviewed metadata such as fiscal_year or segment_tier. Put evidence in signals with source, not in attributes.",
            "Attributes are metadata, not proof. Do not use attributes to substantiate customer adoption, production use, design partners, paid pilots, ARR conversion, market validation, compliance, integrations, or product capability claims.",
            "When existing_pack_context includes lead_input_requirements.attribute_definitions, emit only declared attributes when allow_undeclared_attributes is false, and match declared type, enum, date, or date-time formats. Invalid or unreviewed metadata belongs in gaps or normalization_trace, not attributes.",
            "Preserve uncertainty: weak inferences belong in signal state_as as hypothesis, low confidence, gaps, or normalization_trace.needs_review. Do not smooth away disqualifying execution asks such as scrape contacts, auto-send, sequence everyone, enrich leads, or update CRM.",
            "Keep raw evidence traceable. Each signal should name the supplied source field, note, URL, or row fragment that supports it when available.",
            "Set source_summary.inputs_used to declared prompt input names only, such as raw_row, company_domain, existing_pack_context, runtime_context, or source_kind. Put field paths, source snippets, URLs, and row fragments in signals[].source, normalization_trace.preserved_raw_fields, normalization_trace.missing_required[].source_evidence, or gaps instead.",
            "For non-synthetic rows, use a meaningful source_kind, keep material signals source-backed, and set confidence/freshness from supplied evidence. If source_kind, signal source, confidence, or freshness is vague or inconsistent, mark the row not ready for a draft and emit gaps.",
            "If the input is account-only and lacks person name or title, do not invent a contact. Keep compatibility fields as N/A where the prospect schema requires them, add structured normalization_trace.missing_required entries with field, reason, and source_evidence, add a human-readable gap, and set normalization_trace.fit_readiness.ready_for_mdp_fit and ready_for_brief to false.",
            "Missing-field example: if the row has company but no person title, do not fabricate a title; add {\"field\":\"title\",\"reason\":\"not_available_in_source\",\"source_evidence\":\"Raw row contained no person title.\"} to normalization_trace.missing_required and set ready_for_mdp_fit false.",
            "Invalid-value example: if the row says segment enterprise but value_contracts.segment only allows agent-assisted GTM, do not output segment enterprise; add a gap asking for a reviewed pack segment or manifest update.",
            "Keep card_patches empty. This prompt normalizes runtime prospect input; it does not propose edits to MDP cards."
        ],
        "role": "Provider-neutral prospect normalization analyst",
        "objective": "Convert supplied messy prospect context into the exact bounded prospect JSON accepted by this pack without inventing facts.",
        "procedure": ["Inventory only declared inputs and preserve their source boundaries.", "Normalize values against pack-owned vocabularies and readiness rules.", "Return strict JSON with explicit gaps and normalization trace."],
        "selection_rules": ["Use only pack-owned persona, segment, signal, source-kind, and attribute values.", "Omit or gap any value that cannot be mapped exactly."],
        "ambiguity_policy": ["Preserve weak inferences as hypotheses, low confidence, needs-review, or gaps."],
        "provenance_policy": ["Keep supplied field paths and source notes in signal sources and normalization trace."],
        "evidence_policy": ["Attributes are metadata, not proof; never convert them into adoption, outcome, compliance, or capability claims."],
        "negative_examples": ["Do not invent a person, title, company domain, persona, segment, trigger, or signal.", "Do not browse, scrape, enrich, send, sequence, or update external systems."],
        "final_checklist": ["Output is strict JSON.", "All normalized values satisfy pack vocabulary.", "Missing required context is explicit.", "Fit readiness is false whenever required evidence is absent."],
        "output_contract": {
            "contract": PROMPT_OUTPUT_CONTRACT,
            "output_kind": "prospect-normalization",
            "strict_json_only": true,
            "required_top_level": ["contract", "prompt_id", "source_summary", "normalized_prospect", "normalization_trace", "card_patches", "gaps", "rejected_claims"],
            "entry_defaults": {"body": "N/A", "applies_to": [], "evidence": [], "avoid": [], "confidence": "unknown", "provenance": []},
            "schema_ref": PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF,
            "example": {
                "contract": PROMPT_OUTPUT_CONTRACT,
                "prompt_id": "normalize-prospect-row",
                "source_summary": {"company_domain": "example.com", "company_name": "ExampleCo", "person_name": "Alex Rivera", "person_title": "Revenue Operations Lead", "account_name": "ExampleCo", "inputs_used": ["raw_row", "existing_pack_context"], "confidence": "medium"},
                "normalized_prospect": {
                    "name": "Alex Rivera", "title": "Revenue Operations Lead", "company": "ExampleCo", "company_domain": "example.com", "source_kind": "user-provided-row", "synthetic": false, "company_url": "https://example.com", "background": "Source row says the team is standardizing campaign qualification data across CRM exports, spreadsheets, and research notes.", "trigger": "Standardizing prospect qualification data before routing new campaigns.", "persona": "GTM Engineering", "segment": "agent-assisted GTM", "attributes": {"fiscal_year": "FY2027"},
                    "signals": [{"id": "qualification-data-standardization", "title": "Standardizing prospect qualification data", "source": "raw_row.operations_note", "confidence": "medium", "freshness": "N/A", "state_as": "supplied"}]
                },
                "normalization_trace": {
                    "persona": {"source": "existing_pack_context.persona_mappings", "matched_keywords": ["revenue operations"], "confidence": "medium", "needs_review": false},
                    "fit_readiness": {"has_trigger": true, "has_company_domain": true, "has_persona": true, "has_segment": true, "has_signals": true, "has_signal_source": true, "ready_for_mdp_fit": true},
                    "preserved_raw_fields": ["raw_row.name", "raw_row.title", "raw_row.company", "company_domain", "raw_row.operations_note", "raw_row.fiscal_year"],
                    "missing_required": []
                },
                "card_patches": [], "gaps": [], "rejected_claims": []
            }
        }
    });
    if include_output_schemas {
        prompt["output_contract"]["schema"] = prospect_normalization_output_schema();
    }
    prompt
}

fn prospect_normalization_prompt_contract(include_output_schemas: bool) -> Value {
    let mut prompt = json!({
        "format": PROMPT_FORMAT_V1,
        "id": "normalize-prospect-row",
        "version": "1",
        "kind": "normalization",
        "title": "Normalize prospect row",
        "description": "Turns a supplied messy person, company, account, CRM, CSV, Clay, Deepline, spreadsheet, or research row into provider-neutral MDP prospect JSON before mdp fit or brief runs.",
        "target_card_kinds": ["personas", "fit-rules", "signals"],
        "tags": ["prompt", "normalization", "prospect", "fit", "routing"],
        "inputs": [
            {
                "name": "raw_row",
                "description": "The full messy source row, note, webhook payload, CSV row, CRM export row, Clay/Deepline row, spreadsheet row, or pasted research record.",
                "required": true,
                "default": "N/A",
                "missing_behavior": "Return gaps and do not create normalized_prospect fields from absent source material.",
                "producer": "source"
            },
            {
                "name": "company_domain",
                "description": "Company domain when available.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Use N/A and do not infer company identity from absent data.",
                "producer": "host"
            },
            {
                "name": "existing_pack_context",
                "description": "Relevant manifest personas, persona_mappings, lead_input_requirements.value_contracts, lead_input_requirements.attribute_definitions, fit rules, signal definitions, avoid-rules, output rules, and source policy from this MDP.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Do not assume pack-owned persona mappings, value domains, fit rules, attributes, or signal names when this field is N/A.",
                "producer": "pack"
            },
            {
                "name": "runtime_context",
                "description": "Optional MDP runtime context with now_utc, date_utc, timezone UTC, and local_time_policy. Use it only for temporal framing; fiscal year, renewal dates, event dates, and campaign windows remain pack-declared or supplied metadata.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Do not infer fiscal years, renewal windows, event timing, or local business calendar facts from missing runtime context.",
                "producer": "runtime"
            },
            {
                "name": "source_kind",
                "description": "Provider-neutral source marker such as user-provided-row, csv-row, crm-export-row, clay-row, deepline-row, private-scratch-row, sanitized-example, or synthetic-example.",
                "required": false,
                "default": "user-provided-row",
                "missing_behavior": "Use user-provided-row unless the caller supplies a more specific source kind.",
                "producer": "host"
            }
        ],
        "instructions": [
            "Use only raw_row, company_domain, existing_pack_context, runtime_context, and source_kind. Do not browse, scrape, enrich, send, sequence, update a CRM, or call external systems from this normalization prompt contract.",
            "Return strict JSON only. Do not wrap the response in markdown, prose, comments, or code fences.",
            "Set normalized_prospect to the exact provider-neutral shape accepted by mdp --json schema prospect: name, title, company, optional company_domain, source_kind, synthetic, linkedin_url, company_url, background, trigger, persona, segment, signals, and bounded attributes.",
            "When company_domain or company_url is supplied, normalize only that supplied domain-like value. Do not infer a domain from company name.",
            "Use runtime_context.now_utc and runtime_context.date_utc only to state when this normalization ran or to compare against explicitly supplied timing metadata. Do not hardcode fiscal year or infer customer-specific calendars from the current date.",
            "When existing_pack_context includes lead_input_requirements.value_contracts, emit only values allowed by those pack-owned enum/type/format contracts for persona, segment, source_kind, and other normalized scalar fields. If the source value is outside the contract, omit the optional field or add a gap instead of inventing a synonym.",
            "Use explicit persona from the row only when it already matches a pack-owned persona. Otherwise use pack-owned persona_mappings from existing_pack_context and emit the canonical persona label; if no pack-owned mapping applies, omit persona and add a gap instead of guessing.",
            "Use attributes only for bounded reviewed metadata such as fiscal_year or segment_tier. Put evidence in signals with source, not in attributes.",
            "Attributes are metadata, not proof. Do not use attributes to substantiate customer adoption, production use, design partners, paid pilots, ARR conversion, market validation, compliance, integrations, or product capability claims.",
            "When existing_pack_context includes lead_input_requirements.attribute_definitions, emit only declared attributes when allow_undeclared_attributes is false, and match declared type, enum, date, or date-time formats. Invalid or unreviewed metadata belongs in gaps or normalization_trace, not attributes.",
            "Preserve uncertainty: weak inferences belong in signal state_as as hypothesis, low confidence, gaps, or normalization_trace.needs_review. Do not smooth away disqualifying execution asks such as scrape contacts, auto-send, sequence everyone, enrich leads, or update CRM.",
            "Keep raw evidence traceable. Each signal should name the supplied source field, note, URL, or row fragment that supports it when available.",
            "Set source_summary.inputs_used to declared prompt input names only, such as raw_row, company_domain, existing_pack_context, runtime_context, or source_kind. Put field paths, source snippets, URLs, and row fragments in signals[].source, normalization_trace.preserved_raw_fields, normalization_trace.missing_required[].source_evidence, or gaps instead.",
            "For non-synthetic rows, use a meaningful source_kind, keep material signals source-backed, and set confidence/freshness from supplied evidence. If source_kind, signal source, confidence, or freshness is vague or inconsistent, mark the row not ready for a draft and emit gaps.",
            "If the input is account-only and lacks person name or title, do not invent a contact. Keep compatibility fields as N/A where the prospect schema requires them, add structured normalization_trace.missing_required entries with field, reason, and source_evidence, add a human-readable gap, and set normalization_trace.fit_readiness.ready_for_mdp_fit and ready_for_brief to false.",
            "Missing-field example: if the row has company but no person title, do not fabricate a title; add {\"field\":\"title\",\"reason\":\"not_available_in_source\",\"source_evidence\":\"Raw row contained no person title.\"} to normalization_trace.missing_required and set ready_for_mdp_fit false.",
            "Invalid-value example: if the row says segment enterprise but value_contracts.segment only allows agent-assisted GTM, do not output segment enterprise; add a gap asking for a reviewed pack segment or manifest update.",
            "Keep card_patches empty. This prompt normalizes runtime prospect input; it does not propose edits to MDP cards."
        ],
        "role": "Provider-neutral prospect normalization analyst",
        "objective": "Convert supplied messy prospect context into the exact bounded prospect JSON accepted by this pack without inventing facts.",
        "procedure": ["Inventory only declared inputs and preserve their source boundaries.", "Normalize values against pack-owned vocabularies and readiness rules.", "Return strict JSON with explicit gaps and normalization trace."],
        "selection_rules": ["Use only pack-owned persona, segment, signal, source-kind, and attribute values.", "Omit or gap any value that cannot be mapped exactly."],
        "ambiguity_policy": ["Preserve weak inferences as hypotheses, low confidence, needs-review, or gaps."],
        "provenance_policy": ["Keep supplied field paths and source notes in signal sources and normalization trace."],
        "evidence_policy": ["Attributes are metadata, not proof; never convert them into adoption, outcome, compliance, or capability claims."],
        "negative_examples": ["Do not invent a person, title, company domain, persona, segment, trigger, or signal.", "Do not browse, scrape, enrich, send, sequence, or update external systems."],
        "final_checklist": ["Output is strict JSON.", "All normalized values satisfy pack vocabulary.", "Missing required context is explicit.", "Fit readiness is false whenever required evidence is absent."],
        "output_contract": {
            "contract": PROMPT_OUTPUT_CONTRACT,
            "output_kind": "prospect-normalization",
            "strict_json_only": true,
            "required_top_level": [
                "contract",
                "prompt_id",
                "source_summary",
                "normalized_prospect",
                "normalization_trace",
                "card_patches",
                "gaps",
                "rejected_claims"
            ],
            "entry_defaults": {
                "body": "N/A",
                "applies_to": [],
                "evidence": [],
                "avoid": [],
                "confidence": "unknown",
                "provenance": []
            },
            "schema_ref": PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF,
            "example": {
                "contract": PROMPT_OUTPUT_CONTRACT,
                "prompt_id": "normalize-prospect-row",
                "source_summary": {
                    "company_domain": "example.com",
                    "company_name": "ExampleCo",
                    "person_name": "Alex Rivera",
                    "person_title": "Revenue Operations Lead",
                    "account_name": "ExampleCo",
                    "inputs_used": ["raw_row", "existing_pack_context"],
                    "confidence": "medium"
                },
                "normalized_prospect": {
                    "name": "Alex Rivera",
                    "title": "Revenue Operations Lead",
                    "company": "ExampleCo",
                    "company_domain": "example.com",
                    "source_kind": "user-provided-row",
                    "synthetic": false,
                    "company_url": "https://example.com",
                    "background": "Source row says the team is standardizing campaign qualification data across CRM exports, spreadsheets, and research notes.",
                    "trigger": "Standardizing prospect qualification data before routing new campaigns.",
                    "persona": "GTM Engineering",
                    "segment": "agent-assisted GTM",
                    "attributes": {
                        "fiscal_year": "FY2027"
                    },
                    "signals": [
                        {
                            "id": "qualification-data-standardization",
                            "title": "Standardizing prospect qualification data",
                            "source": "raw_row.operations_note",
                            "confidence": "medium",
                            "freshness": "N/A",
                            "state_as": "supplied"
                        }
                    ]
                },
                "normalization_trace": {
                    "persona": {
                        "source": "existing_pack_context.persona_mappings",
                        "matched_keywords": ["revenue operations"],
                        "confidence": "medium",
                        "needs_review": false
                    },
                    "fit_readiness": {
                        "has_trigger": true,
                        "has_company_domain": true,
                        "has_persona": true,
                        "has_segment": true,
                        "has_signals": true,
                        "has_signal_source": true,
                        "ready_for_mdp_fit": true
                    },
                    "preserved_raw_fields": ["raw_row.name", "raw_row.title", "raw_row.company", "company_domain", "raw_row.operations_note", "raw_row.fiscal_year"],
                    "missing_required": []
                },
                "card_patches": [],
                "gaps": [],
                "rejected_claims": []
            }
        }
    });
    let _ = include_output_schemas;
    prompt["version"] = json!("gtm-prospect-context.v2");
    prompt["title"] = json!("Normalize attempted-complete prospect decision inputs");
    prompt["description"] = json!(
        "Normalizes a host-collected attempted-complete prospect ledger into the signal-aware v2 envelope required before canonical GTM fit, brief, or copy-review work."
    );
    prompt["inputs"] = json!([
        {
            "name": "raw_row",
            "description": "The exact collected-attempt-results ledger for every compiled Decision Input attribute.",
            "required": true,
            "default": "N/A",
            "missing_behavior": "Return malformed with draft_allowed false; do not collect or infer missing data.",
            "producer": "source"
        },
        {
            "name": "decision_input_requirements",
            "description": "The exact mdp.requirements.v2 data object compiled for the selected canonical job.",
            "required": true,
            "default": "N/A",
            "missing_behavior": "Return malformed with draft_allowed false.",
            "producer": "pack"
        },
        {
            "name": "source_binding_sha256",
            "description": "SHA-256 of the exact validated mdp.source-binding.v2 artifact supplied by the host.",
            "required": true,
            "default": "N/A",
            "missing_behavior": "Return malformed with draft_allowed false.",
            "producer": "host"
        },
        {
            "name": "source_attempt_request_sha256",
            "description": "SHA-256 of the exact attempted-complete source request supplied by the host.",
            "required": true,
            "default": "N/A",
            "missing_behavior": "Return malformed with draft_allowed false.",
            "producer": "host"
        },
        {
            "name": "collected_attempt_results_sha256",
            "description": "SHA-256 of raw_row supplied by the host.",
            "required": true,
            "default": "N/A",
            "missing_behavior": "Return malformed with draft_allowed false.",
            "producer": "host"
        }
    ]);
    prompt["instructions"] = json!([
        "Use only raw_row, decision_input_requirements, source_binding_sha256, source_attempt_request_sha256, and collected_attempt_results_sha256. Do not browse, scrape, enrich, send, sequence, mutate CRM records, or call external systems.",
        "Treat decision_input_requirements.normalized_output_schema as binding and return exactly one mdp.normalized-decision-input.v2 object.",
        "Copy the job, contract, normalization, binding, request, and collected-results receipts exactly; never invent or upgrade authority.",
        "Preserve every compiled attribute exactly once with its observed, not_found, not_applicable, blocked, or error status. Never convert blocked or error to absence.",
        "Emit repeated sourced observations only as mdp.signal-observation.v2 records for compiled signal projections, roles, contributors, and attempt receipts. Preserve agreement and conflicts; do not select a positive winner.",
        "Populate normalized_prospect only from observed values through declared output_path mappings. Do not infer a person, domain, persona, segment, trigger, contact policy, or signal.",
        "Set outcome to ready only when compiled readiness and hard-gate policy permit deterministic evaluation. Otherwise use insufficient-context, disqualified, human-review, malformed, or provider-error.",
        "Always set draft_allowed to false. Normalization never drafts or authorizes collection, generation, sending, or external mutation.",
        "Return strict JSON only without markdown, prose, comments, or code fences."
    ]);
    prompt["role"] = json!("Provider-neutral Decision Input normalization analyst");
    prompt["objective"] = json!(
        "Normalize one attempted-complete prospect ledger into the exact governed v2 envelope without adding facts or execution authority."
    );
    prompt["procedure"] = json!([
        "Verify exact contract and lineage receipts.",
        "Preserve every attempted attribute and repeated signal observation.",
        "Apply compiled status and conflict behavior.",
        "Return one no-draft v2 envelope."
    ]);
    prompt["selection_rules"] = json!([
        "Use only compiled attributes and projections.",
        "Accept only declared source classes, value contracts, and output paths."
    ]);
    prompt["ambiguity_policy"] = json!([
        "Unresolved disagreement is human-review and no-draft; never choose a positive winner."
    ]);
    prompt["provenance_policy"] = json!([
        "Retain exact attempt, source, observation, confidence, freshness, request, results, and binding receipts."
    ]);
    prompt["evidence_policy"] = json!([
        "Lineage consistency does not prove host authenticity, authorization, or source truth."
    ]);
    prompt["negative_examples"] = json!([
        "Do not infer a DIC from prompt prose, field names, or lead_input_requirements.",
        "Do not turn not_found, blocked, or error into a safe value."
    ]);
    prompt["final_checklist"] = json!([
        "All compiled attempts are present.",
        "Repeated observations use v2 projections.",
        "No undeclared prospect field is populated.",
        "draft_allowed is false."
    ]);
    prompt["output_contract"] = json!({
        "contract": "mdp.normalized-decision-input.v2",
        "output_kind": "decision-input-normalization",
        "strict_json_only": true,
        "required_top_level": [
            "contract", "job_id", "decision_input_contracts", "normalization",
            "source_binding_sha256", "source_attempt_request_sha256",
            "collected_attempt_results_sha256", "attributes", "signal_observations",
            "normalized_prospect", "outcome", "draft_allowed"
        ],
        "entry_defaults": {
            "body": "N/A",
            "applies_to": [],
            "evidence": [],
            "avoid": [],
            "confidence": "unknown",
            "provenance": []
        },
        "schema_ref": "mdp.normalized-decision-input.v2",
        "example": starter_decision_input_normalization_example()
    });
    prompt
}

fn starter_decision_input_normalization_example() -> Value {
    let observed = |attempt_id: &str, value: Value| {
        json!({
            "status": "observed",
            "value": value,
            "provenance": [{
                "attempt_id": attempt_id,
                "source_class": "synthetic_fixture",
                "source_locator": format!("opaque:{attempt_id}"),
                "observed_at": "2026-01-15T12:00:00Z"
            }],
            "confidence": 100,
            "freshness": {"observed_at": "2026-01-15T12:00:00Z", "age_days": 0}
        })
    };
    json!({
        "contract": "mdp.normalized-decision-input.v2",
        "job_id": "prospect-fit-or-brief",
        "decision_input_contracts": ["gtm.prospect-context"],
        "normalization": [{
            "contract_id": "gtm.prospect-context",
            "prompt": "prompts/normalize-prospect.yaml",
            "prompt_version": "gtm-prospect-context.v2"
        }],
        "source_binding_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "source_attempt_request_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "collected_attempt_results_sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "attributes": {
            "company_name": observed("synthetic-attempt-001", json!("Example Prospect Company")),
            "company_domain": observed("synthetic-attempt-002", json!("example.invalid")),
            "person_name": observed("synthetic-attempt-003", json!("Alex Example")),
            "person_title": observed("synthetic-attempt-004", json!("Revenue Operations Lead")),
            "persona": observed("synthetic-attempt-005", json!("GTM Engineering")),
            "segment": observed("synthetic-attempt-006", json!("agent-assisted GTM")),
            "trigger": observed("synthetic-attempt-007", json!("Synthetic account is reviewing its prospect qualification workflow.")),
            "contact_policy": observed("synthetic-attempt-008", json!("clear"))
        },
        "signal_observations": [
            {
                "contract": "mdp.signal-observation.v2",
                "id": "synthetic-trigger-001",
                "contract_id": "gtm.prospect-context",
                "projection_id": "why-now",
                "qualified_projection_id": "gtm.prospect-context#why-now",
                "kind": "prospect_trigger",
                "roles": ["fit", "why-now"],
                "value": "Synthetic account is reviewing its prospect qualification workflow.",
                "contributor_attribute_ids": ["trigger"],
                "attempt_ids": ["synthetic-attempt-007"],
                "source_class": "synthetic_fixture",
                "source_locator": "opaque:synthetic-attempt-007",
                "observed_at": "2026-01-15T12:00:00Z",
                "confidence": 100,
                "receipt": {
                    "source_binding_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "source_attempt_request_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "collected_results_sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                }
            },
            {
                "contract": "mdp.signal-observation.v2",
                "id": "synthetic-contact-policy-001",
                "contract_id": "gtm.prospect-context",
                "projection_id": "contact-policy",
                "qualified_projection_id": "gtm.prospect-context#contact-policy",
                "kind": "contact_policy",
                "roles": ["disqualifier"],
                "value": "clear",
                "contributor_attribute_ids": ["contact_policy"],
                "attempt_ids": ["synthetic-attempt-008"],
                "source_class": "synthetic_fixture",
                "source_locator": "opaque:synthetic-attempt-008",
                "observed_at": "2026-01-15T12:00:00Z",
                "confidence": 100,
                "receipt": {
                    "source_binding_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "source_attempt_request_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "collected_results_sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                }
            }
        ],
        "normalized_prospect": {
            "name": "Alex Example",
            "title": "Revenue Operations Lead",
            "company": "Example Prospect Company",
            "company_domain": "example.invalid",
            "persona": "GTM Engineering",
            "segment": "agent-assisted GTM",
            "trigger": "Synthetic account is reviewing its prospect qualification workflow.",
            "source_kind": "synthetic-example",
            "synthetic": true,
            "attributes": {"contact_policy": "clear"}
        },
        "outcome": "ready",
        "draft_allowed": false
    })
}

fn prompt_contract(
    id: &str,
    title: &str,
    description: &str,
    target_card_kinds: &[&str],
    tags: &[&str],
    task_instruction: &str,
    card_patches: Value,
    inputs_used: &[&str],
    gaps: &[&str],
    include_output_schemas: bool,
) -> Value {
    let mut prompt = json!({
        "format": PROMPT_FORMAT_VERSION,
        "id": id,
        "title": title,
        "description": description,
        "target_card_kinds": target_card_kinds,
        "tags": tags,
        "inputs": [
            {
                "name": "company_domain",
                "description": "Company domain when available.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Use N/A and do not infer company identity from absent data."
            },
            {
                "name": "company_data",
                "description": "Arbitrary user-provided company, website, firmographic, product, hiring, funding, or research context.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Use N/A, emit gaps, and avoid creating candidate entries from missing context."
            },
            {
                "name": "person_data",
                "description": "Optional user-provided person-level context such as title, role, profile notes, responsibilities, posts, or background.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Use N/A and do not infer role, seniority, priorities, or persona from absent person data."
            },
            {
                "name": "account_data",
                "description": "Optional account-level context such as segment, lifecycle stage, trigger, current workflow, tech stack, or qualification notes.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Use N/A and emit fit or gap entries instead of forcing ICP classification."
            },
            {
                "name": "source_notes",
                "description": "Optional source excerpts, URLs, file references, or user notes.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Use empty evidence arrays unless a supplied source supports the entry."
            },
            {
                "name": "existing_pack_context",
                "description": "Optional existing MDP manifest/card context to prevent duplicate or conflicting entries, including personas, operator roles, fit rules, claims, avoid-rules, output rules, supported channels, and declared value domains.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Do not assume previous pack decisions, personas, channels, claims, or value domains when this field is N/A."
            },
            {
                "name": "runtime_context",
                "description": "Optional MDP runtime context with now_utc, date_utc, timezone UTC, and local_time_policy. Use it only for temporal framing; fiscal year, renewal dates, event dates, and campaign windows remain pack-declared or supplied metadata.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Do not infer fiscal years, renewal windows, event timing, or local business calendar facts from missing runtime context."
            }
        ],
        "instructions": [
            "Use only supplied company_domain, person_data, company_data, account_data, source_notes, existing_pack_context, and runtime_context. Do not browse, scrape, enrich, or call external systems from this extraction prompt contract.",
            task_instruction,
            "Return strict JSON only. Do not wrap the response in markdown, prose, comments, or code fences.",
            "Use existing_pack_context as the source of truth for pack-owned personas, operator roles, card ids, claims, avoid-rules, output rules, supported channels, and value domains. Do not invent new pack labels when the source is weak; emit gaps or needs-review candidates instead.",
            "Use runtime_context.now_utc and runtime_context.date_utc only to state when this extraction ran or to compare against explicitly supplied timing metadata. Do not hardcode fiscal year or infer customer-specific calendars from the current date.",
            "For source_summary.company_domain, use the supplied company_domain or an explicit supplied URL/domain only. Do not infer a domain from company name.",
            "Each card_patches entry must contain candidate MDP entry fields: id, title, body, applies_to, evidence, and avoid. Use constraints for deterministic output limits such as word counts, subject word counts, max questions, or forbidden links/html/tracking when the source explicitly calls for them. Use metadata only for advisory custom annotations that should be preserved for agents but not enforced by the CLI.",
            "Set source_summary.inputs_used to exact declared prompt input names only. Keep source paths, snippets, URLs, and field-level provenance in evidence and provenance. Do not put prospect facts, proof, citations, or raw source excerpts only in metadata.",
            "Each candidate entry must also include confidence, provenance, and status so a reviewer can decide whether it may become a card entry.",
            "Use body N/A, empty arrays, confidence unknown, and status gap when data is missing or weak.",
            "Put unsupported, quantified, customer, integration, compliance, or execution claims in rejected_claims instead of card_patches.",
            "MDP is a local/offline decision and context layer, not a sender, CRM, sequencer, enrichment provider, scraper, BI tool, AI SDR, or generic automation system."
        ],
        "output_contract": {
            "contract": PROMPT_OUTPUT_CONTRACT,
            "strict_json_only": true,
            "required_top_level": [
                "contract",
                "prompt_id",
                "source_summary",
                "card_patches",
                "gaps",
                "rejected_claims"
            ],
            "entry_defaults": {
                "body": "N/A",
                "applies_to": [],
                "evidence": [],
                "avoid": [],
                "confidence": "unknown",
                "provenance": []
            },
            "schema_ref": PROMPT_CARD_PATCH_SCHEMA_REF,
            "example": {
                "contract": PROMPT_OUTPUT_CONTRACT,
                "prompt_id": id,
                "source_summary": {
                    "company_domain": "N/A",
                    "company_name": "N/A",
                    "person_name": "N/A",
                    "person_title": "N/A",
                    "account_name": "N/A",
                    "inputs_used": inputs_used,
                    "confidence": "unknown"
                },
                "card_patches": card_patches,
                "gaps": gaps,
                "rejected_claims": []
            }
        }
    });
    if include_output_schemas {
        prompt["output_contract"]["schema"] = card_patch_output_schema(id, target_card_kinds);
    }
    prompt
}

#[allow(dead_code)]
fn prospect_normalization_output_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP prospect normalization output",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "contract",
            "prompt_id",
            "source_summary",
            "normalized_prospect",
            "normalization_trace",
            "card_patches",
            "gaps",
            "rejected_claims"
        ],
        "properties": {
            "contract": {
                "const": PROMPT_OUTPUT_CONTRACT,
                "description": "Stable MDP prompt output contract identifier."
            },
            "prompt_id": {
                "const": "normalize-prospect-row",
                "description": "The prompt contract that produced this response."
            },
            "source_summary": source_summary_output_schema(),
            "runtime_context": runtime_context_schema(),
            "normalized_prospect": normalized_prospect_output_schema(),
            "normalization_trace": normalization_trace_output_schema(),
            "card_patches": {
                "type": "array",
                "maxItems": 0,
                "description": "Always empty for prospect normalization prompts; this prompt does not edit MDP cards."
            },
            "gaps": string_array_output_schema("Missing source data, weak inferences, or review questions that block confident fit/routing."),
            "rejected_claims": rejected_claims_output_schema()
        }
    })
}

fn card_patch_output_schema(id: &str, target_card_kinds: &[&str]) -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": format!("MDP card patch output: {id}"),
        "type": "object",
        "additionalProperties": false,
        "required": [
            "contract",
            "prompt_id",
            "source_summary",
            "card_patches",
            "gaps",
            "rejected_claims"
        ],
        "properties": {
            "contract": {
                "const": PROMPT_OUTPUT_CONTRACT,
                "description": "Stable MDP prompt output contract identifier."
            },
            "prompt_id": {
                "const": id,
                "description": "The prompt contract that produced this response."
            },
            "source_summary": source_summary_output_schema(),
            "runtime_context": runtime_context_schema(),
            "card_patches": {
                "type": "array",
                "description": "Candidate MDP card entries grouped by target card. These require human review before being copied into cards.",
                "items": card_patch_item_output_schema(target_card_kinds)
            },
            "gaps": string_array_output_schema("Missing source data, weak inferences, or review questions that block stronger candidate entries."),
            "rejected_claims": rejected_claims_output_schema()
        }
    })
}

fn source_summary_output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["company_domain", "company_name", "inputs_used", "confidence"],
        "properties": {
            "company_domain": {
                "type": "string",
                "description": "Supplied company domain, or N/A when absent."
            },
            "company_name": {
                "type": "string",
                "description": "Normalized company name from supplied input, or N/A when absent."
            },
            "person_name": {
                "type": "string",
                "description": "Supplied person name, or N/A when absent."
            },
            "person_title": {
                "type": "string",
                "description": "Supplied person title, or N/A when absent."
            },
            "account_name": {
                "type": "string",
                "description": "Supplied account name, or N/A when absent."
            },
            "inputs_used": string_array_output_schema("Exact declared prompt input names used to create this output. Do not put field paths, URLs, source snippets, or page locators here; use evidence/provenance, signals[].source, or normalization_trace for source locators."),
            "confidence": {
                "enum": ["high", "medium", "low", "unknown"],
                "description": "Overall confidence in the source summary."
            }
        }
    })
}

#[allow(dead_code)]
fn normalized_prospect_output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["name", "title", "company"],
        "properties": {
            "name": {
                "type": "string",
                "description": "Person name from the supplied row. Do not invent a contact."
            },
            "title": {
                "type": "string",
                "description": "Person title from the supplied row. Do not invent a title."
            },
            "company": {
                "type": "string",
                "description": "Company or account name from the supplied row."
            },
            "company_domain": {
                "type": "string",
                "description": "Preferred account routing key when supplied. Normalize URLs/domains such as https://www.apple.com/ to apple.com; do not infer from company name."
            },
            "source_kind": {
                "type": "string",
                "description": "Provider-neutral source marker such as user-provided-row, csv-row, crm-export-row, clay-row, deepline-row, private-scratch-row, sanitized-example, or synthetic-example."
            },
            "synthetic": {
                "type": "boolean",
                "description": "True only for generated or fictional fixtures."
            },
            "linkedin_url": {"type": "string"},
            "company_url": {"type": "string"},
            "background": {
                "type": "string",
                "description": "Short source-backed context that may help fit or brief creation."
            },
            "trigger": {
                "type": "string",
                "description": "Source-backed trigger or reason this row may be relevant."
            },
            "persona": {
                "type": "string",
                "description": "Explicit row persona or pack-owned persona mapping. Omit when unsupported."
            },
            "segment": {
                "type": "string",
                "description": "Source-backed segment or account category."
            },
            "signals": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "title"],
                    "properties": {
                        "id": {"type": "string"},
                        "title": {"type": "string"},
                        "source": {"type": "string"},
                        "confidence": {"enum": ["high", "medium", "low", "unknown"]},
                        "freshness": {"type": "string"},
                        "state_as": {
                            "type": "string",
                            "description": "How to state the signal, such as supplied, observed, or hypothesis."
                        }
                    }
                }
            },
            "attributes": {
                "type": "object",
                "maxProperties": 25,
                "description": "Bounded reviewed metadata for pack-specific routing, such as fiscal_year or segment tier. Use signals with source fields for evidence instead of dumping raw data here.",
                "propertyNames": {"pattern": "^[A-Za-z][A-Za-z0-9_-]{0,63}$"},
                "additionalProperties": {
                    "type": ["string", "number", "integer", "boolean"]
                }
            }
        }
    })
}

#[allow(dead_code)]
fn normalization_trace_output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["persona", "fit_readiness", "preserved_raw_fields", "missing_required"],
        "properties": {
            "persona": {
                "type": "object",
                "description": "How persona was preserved, mapped, omitted, or marked review-needed."
            },
            "fit_readiness": {
                "type": "object",
                "description": "Booleans that tell the caller whether mdp fit has enough context."
            },
            "preserved_raw_fields": string_array_output_schema("Raw row fields preserved in the normalized prospect or trace."),
            "missing_required": missing_required_output_schema()
        }
    })
}

#[allow(dead_code)]
fn missing_required_output_schema() -> Value {
    json!({
        "type": "array",
        "description": "Required prospect fields missing from the supplied row. Prefer structured objects so missing source data is distinguishable from invalid values; legacy string field names remain accepted for compatibility.",
        "items": {
            "oneOf": [
                {"type": "string"},
                {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["field", "reason"],
                    "properties": {
                        "field": {
                            "type": "string",
                            "description": "Missing or non-extractable prospect field, such as name, title, persona, segment, trigger, or signals."
                        },
                        "path": {
                            "type": "string",
                            "description": "Optional output path, such as normalized_prospect.title."
                        },
                        "reason": {
                            "type": "string",
                            "description": "Reason code such as not_available_in_source, not_extractable_from_source, not_extractable_without_person, or invalid_out_of_contract."
                        },
                        "source_evidence": {
                            "type": "string",
                            "description": "Short source-backed explanation, such as Raw row said no named person yet."
                        }
                    }
                }
            ]
        }
    })
}

fn card_patch_item_output_schema(target_card_kinds: &[&str]) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["card_id", "kind", "entries"],
        "properties": {
            "card_id": {
                "type": "string",
                "description": "Target MDP card id for these candidate entries."
            },
            "kind": {
                "enum": target_card_kinds,
                "description": "Target MDP card kind; must be one of this prompt's target_card_kinds."
            },
            "entries": {
                "type": "array",
                "items": candidate_entry_output_schema()
            }
        }
    })
}

fn candidate_entry_output_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "id",
            "title",
            "body",
            "applies_to",
            "evidence",
            "avoid",
            "confidence",
            "provenance",
            "status",
            "notes"
        ],
        "properties": {
            "id": {
                "type": "string",
                "description": "Stable kebab-case candidate entry id."
            },
            "title": {"type": "string"},
            "body": {
                "type": "string",
                "description": "Candidate MDP entry body, or N/A when the source is too weak."
            },
            "applies_to": string_array_output_schema("Personas or operator roles this entry applies to."),
            "evidence": string_array_output_schema("Source ids, source fields, URLs, or notes supporting this entry."),
            "avoid": string_array_output_schema("Phrases, claims, audiences, or conditions this entry should avoid."),
            "exact_paragraphs": {
                "type": "integer",
                "minimum": 1,
                "description": "Optional exact paragraph count for output-rules entries."
            },
            "constraints": constraints_output_schema(),
            "metadata": {
                "type": "object",
                "description": "Optional advisory extension data preserved for agents but not enforced by the CLI.",
                "additionalProperties": true
            },
            "confidence": {
                "enum": ["high", "medium", "low", "unknown"]
            },
            "provenance": string_array_output_schema("Specific source references that explain where this candidate came from."),
            "status": {
                "enum": ["candidate", "needs-review", "gap", "rejected"]
            },
            "notes": string_array_output_schema("Reviewer notes, caveats, or unresolved questions.")
        }
    })
}

fn rejected_claims_output_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["claim", "reason"],
            "properties": {
                "claim": {"type": "string"},
                "reason": {"type": "string"},
                "source": {
                    "type": "string",
                    "description": "Source field or reference for the rejected claim, or N/A when absent."
                }
            }
        }
    })
}

fn string_array_output_schema(description: &str) -> Value {
    json!({
        "type": "array",
        "description": description,
        "items": {"type": "string"}
    })
}

fn constraints_output_schema() -> Value {
    json!({
        "type": "object",
        "description": "Optional deterministic output constraints for generated drafts. Use only when the source or pack author explicitly defines the rule.",
        "properties": {
            "word_count": count_constraint_output_schema("Body word count limits."),
            "subject_words": count_constraint_output_schema("Subject line word count limits."),
            "subject_avoid": string_array_output_schema("Case-insensitive subject literals to avoid, such as Re: or Fwd:."),
            "max_questions": {
                "type": "integer",
                "minimum": 0
            },
            "forbid_links": {"type": "boolean"},
            "forbid_attachments": {"type": "boolean"},
            "forbid_images": {"type": "boolean"},
            "forbid_html": {"type": "boolean"},
            "forbid_tracking": {"type": "boolean"}
        }
    })
}

fn count_constraint_output_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "description": description,
        "properties": {
            "min": {"type": "integer", "minimum": 0},
            "max": {"type": "integer", "minimum": 0},
            "target_min": {"type": "integer", "minimum": 0},
            "target_max": {"type": "integer", "minimum": 0}
        }
    })
}

pub(crate) fn starter_prompts(include_output_schemas: bool) -> Vec<(&'static str, Value)> {
    let mut prompts = generated_starter_prompts(include_output_schemas);
    prompts[0].1 = legacy_prospect_normalization_prompt_contract(include_output_schemas);
    prompts
}

fn prompt_entry(
    id: &str,
    title: &str,
    body: &str,
    applies_to: &[&str],
    evidence: &[&str],
    avoid: &[&str],
    confidence: &str,
    provenance: &[&str],
    status: &str,
) -> Value {
    json!({
        "id": id,
        "title": title,
        "body": body,
        "applies_to": applies_to,
        "evidence": evidence,
        "avoid": avoid,
        "confidence": confidence,
        "provenance": provenance,
        "status": status,
        "notes": []
    })
}
