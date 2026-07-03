use crate::constants::{
    FORMAT_VERSION, PROMPT_CARD_PATCH_SCHEMA_REF, PROMPT_FORMAT_VERSION, PROMPT_OUTPUT_CONTRACT,
    PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF,
};
use crate::models::{
    AgentSurface, BlockedSkill, Card, CardKind, CardRef, CountConstraint, Entry, EntryConstraints,
    InputContract, JobSkillRoute, LeadInputRequirements, Manifest, PersonaMapping, Policy,
    PrimitiveMapping, Profile, ProfileActivation, ProfileEval, ProfileJob, Provenance,
    ValueContract,
};
use crate::runtime_context::runtime_context_schema;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(crate) fn starter_manifest(name: &str, slug: &str, _template: &str) -> Manifest {
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
    let attribute_definitions = BTreeMap::from([(
        "fiscal_year".to_string(),
        ValueContract {
            value_type: Some("string".to_string()),
            description: Some(
                "Optional reviewed account metadata. Keep proof in signals, not attributes."
                    .to_string(),
            ),
            ..ValueContract::default()
        },
    )]);
    Manifest {
        format: FORMAT_VERSION.to_string(),
        id: slug.to_string(),
        name: name.to_string(),
        version: "0.1.0".to_string(),
        description: Some("A modular message decision pack for agent-readable ICP, pains, triggers, proof, CTA policy, avoid-rules, output rules, and copy guidance.".to_string()),
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
            required_fields: vec!["name".to_string(), "title".to_string(), "company_domain".to_string(), "trigger".to_string(), "persona".to_string(), "segment".to_string(), "signals".to_string()],
            required_signal_fields: vec!["source".to_string()],
            required_attributes: Vec::new(),
            value_contracts,
            attribute_definitions,
            allow_undeclared_attributes: true,
        },
        required_primitives: gtm_required_primitives(),
        primitive_map: gtm_primitive_map(),
        input_contracts: vec![InputContract {
            id: "prospect".to_string(),
            description: Some(
                "Reviewed prospect/source-row input contract for person, account, and relationship context."
                    .to_string(),
            ),
            schema_ref: Some("mdp.input.prospect.v0".to_string()),
            prompt: Some("prompts/normalize-prospect.yaml".to_string()),
            normalizes: strings(&["account", "person", "relationship"]),
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
            card_ref("ctas", "cards/ctas.yaml", CardKind::Ctas, "CTA rules, reply paths, and ask boundaries for outbound copy.", &["PMM", "GTM Engineering"], &["cta", "ask", "reply", "copy", "outbound", "message"]),
            card_ref("avoid-rules", "cards/avoid-rules.yaml", CardKind::AvoidRules, "Claims and categories the agent must avoid.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "avoid"]),
            card_ref("output-rules", "cards/output-rules.yaml", CardKind::OutputRules, "Global style, formatting, and output-structure rules for generated text.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "style", "format"]),
            card_ref("copy-patterns", "cards/copy-patterns.yaml", CardKind::CopyPatterns, "Copy structures and brief patterns for GTM outputs.", &["PMM"], &["copy", "brief", "outbound", "message"]),
            card_ref("objections", "cards/objections.yaml", CardKind::Objections, "Expected objections, category confusion, and approved response logic.", &["PMM", "GTM Engineering"], &["objection", "alternative", "response"]),
            card_ref("gaps", "cards/gaps.yaml", CardKind::Gaps, "Known gaps and open questions agents must surface instead of filling in.", &["GTM Engineering", "PMM", "PM"], &["gap", "unknown", "open-question"]),
        ],
        policy: Policy { progressive_disclosure: true, load_manifest_first: true, max_cards_per_route: 13, json_contract: "mdp.cli.v0".to_string(), no_auth_required: true },
        provenance: Provenance { owner: "local".to_string(), created_by: "mdp init".to_string(), notes: vec!["This pack is guidance and evidence context, not an execution system.".to_string(), "Agents should load only routed cards unless the user asks for a full audit.".to_string()] },
    }
}

fn gtm_profile() -> Profile {
    Profile {
        id: "gtm".to_string(),
        label: Some("GTM Messaging".to_string()),
        version: Some("mdp.profile.v0".to_string()),
        agent_surface: AgentSurface {
            recommended_skills: vec![
                "mdp".to_string(),
                "mdp-create-pack".to_string(),
                "mdp-icp-builder".to_string(),
                "mdp-prospect-brief".to_string(),
                "mdp-pack-eval".to_string(),
            ],
            allowed_skills: vec![
                "mdp".to_string(),
                "mdp-lfg".to_string(),
                "mdp-create-pack".to_string(),
                "mdp-icp-builder".to_string(),
                "mdp-source-extract".to_string(),
                "mdp-message-angles".to_string(),
                "mdp-cta-builder".to_string(),
                "mdp-avoid-rules".to_string(),
                "mdp-output-rules".to_string(),
                "mdp-prospect-brief".to_string(),
                "mdp-copy-brief".to_string(),
                "mdp-copy-eval".to_string(),
                "mdp-pack-review".to_string(),
                "mdp-pack-eval".to_string(),
            ],
            blocked_skills: vec![
                BlockedSkill {
                    name: "mdp-proposal-pack-builder".to_string(),
                    reason: "Proposal/RFP review pack builder; use only with profile.id: proposal."
                        .to_string(),
                },
                BlockedSkill {
                    name: "mdp-proposal-bid-no-bid-review".to_string(),
                    reason: "Proposal review job; not a GTM prospect or outbound-copy workflow."
                        .to_string(),
                },
                BlockedSkill {
                    name: "mdp-proposal-compliance-review".to_string(),
                    reason: "Proposal compliance review job; not a GTM messaging workflow."
                        .to_string(),
                },
                BlockedSkill {
                    name: "mdp-proposal-win-theme-proof-review".to_string(),
                    reason: "Proposal proof review job; use GTM claims/copy review skills instead."
                        .to_string(),
                },
                BlockedSkill {
                    name: "mdp-proposal-red-team-gap-review".to_string(),
                    reason: "Proposal red-team review job; use mdp-pack-review or mdp-pack-eval for GTM packs."
                        .to_string(),
                },
            ],
            job_skills: vec![
                JobSkillRoute {
                    job: "create or improve GTM messaging pack".to_string(),
                    skills: vec!["mdp-create-pack".to_string(), "mdp-icp-builder".to_string()],
                },
                JobSkillRoute {
                    job: "prospect row to fit decision or brief".to_string(),
                    skills: vec!["mdp-prospect-brief".to_string()],
                },
                JobSkillRoute {
                    job: "copy brief or copy evaluation".to_string(),
                    skills: vec!["mdp-copy-brief".to_string(), "mdp-copy-eval".to_string()],
                },
                JobSkillRoute {
                    job: "pack validation and eval coverage".to_string(),
                    skills: vec!["mdp-pack-review".to_string(), "mdp-pack-eval".to_string()],
                },
            ],
        },
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
                    "prompt-output-validation",
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
                    "create-or-improve-gtm-pack",
                    "prospect-fit-or-brief",
                    "outbound-copy-brief",
                    "pack-validation",
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
                    "prompt-output-validation",
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
                    "claim-check-output-rule",
                    "linkedin-copy-route",
                    "email-initial-route",
                    "call-prep-route",
                    "account-context-present",
                    "account-context-missing",
                    "account-only-no-draft",
                    "prompt-output-validation",
                ],
            ),
        ),
    ])
}

fn gtm_profile_jobs() -> Vec<ProfileJob> {
    vec![
        ProfileJob {
            id: "create-or-improve-gtm-pack".to_string(),
            label: Some("Create or improve GTM messaging pack".to_string()),
            description: Some(
                "Author or revise reviewed GTM decision context, not execution infrastructure."
                    .to_string(),
            ),
            required_primitives: strings(&[
                "actors",
                "decision-criteria",
                "source-signals",
                "needs-requirements",
                "evidence-proof",
                "boundaries",
                "output-contracts",
                "gaps",
                "evals",
            ]),
            input_contracts: Vec::new(),
        },
        ProfileJob {
            id: "prospect-fit-or-brief".to_string(),
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
        },
        ProfileJob {
            id: "outbound-copy-brief".to_string(),
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
        },
        ProfileJob {
            id: "pack-validation".to_string(),
            label: Some("Pack validation and eval coverage".to_string()),
            description: Some(
                "Check structural validity, profile primitive coverage, and local eval categories."
                    .to_string(),
            ),
            required_primitives: strings(&["boundaries", "gaps", "evals"]),
            input_contracts: Vec::new(),
        },
    ]
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

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

pub(crate) fn starter_cards(_template: &str) -> Vec<(&'static str, Card)> {
    vec![
        ("personas.yaml", card("personas", CardKind::Personas, "Core personas", "The users who author, maintain, and consume the decision pack.", &["GTM Engineering", "PMM", "PM"], &["persona"], vec![
            entry_with_evidence("gtm-engineering", "GTM Engineering", "Needs precise contracts, data boundaries, approved workflows, and machine-readable routing.", &["GTM Engineering"], &["README.md"]),
            entry_with_evidence("pmm", "PMM", "Needs pains, triggers, hooks, proof points, CTA policy, and copy constraints without losing source fidelity.", &["PMM"], &["README.md"]),
            entry_with_evidence("pm", "PM", "Needs product boundaries, roadmap-relevant pain evidence, and clear decisions about what the product is not.", &["PM"], &["README.md"]),
        ])),
        ("positioning.yaml", card("positioning", CardKind::Positioning, "Positioning and boundaries", "Category and product truth that every routed brief should preserve.", &["GTM Engineering", "PMM", "PM"], &["positioning", "category", "boundary"], vec![
            entry_with_evidence("decision-layer", "Decision/context layer", "Describe MDP as a local, agent-readable decision and context layer for GTM messaging. It stores what an agent should load, believe, avoid, and surface as a gap.", &["GTM Engineering", "PMM", "PM"], &["README.md"]),
            entry_with_evidence("not-execution-system", "Not execution", "Do not describe MDP as a sender, CRM, sequencer, enrichment provider, scraper, AI SDR, BI tool, or generic automation system.", &["GTM Engineering", "PMM", "PM"], &["README.md"]),
            entry_with_evidence("progressive-disclosure", "Progressive disclosure", "The pack is a small manifest plus modular cards. Agents should load only the cards returned by route or brief commands.", &["GTM Engineering", "PMM"], &["README.md"]),
        ])),
        ("fit-rules.yaml", card("fit-rules", CardKind::FitRules, "Fit rules", "ICP, qualification, disqualification, and no-message rules.", &["GTM Engineering", "PMM", "PM"], &["fit", "icp", "disqualifier", "no-message"], vec![
            entry_with_evidence("good-fit-agent-gtm", "Good fit: agent-assisted GTM", "Use when the account is building GTM workflows with agents, provider-neutral source rows, Codex/Claude Code/OpenCode, or multiple systems that need consistent message context.", &["GTM Engineering", "PMM"], &["README.md", "examples/clay-row.json"]),
            Entry { id: "no-context-no-copy".to_string(), title: "No message without context".to_string(), body: "If the row has no persona, trigger, source, or useful account context, return insufficient-context instead of drafting polished copy.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string()], evidence: vec!["README.md".to_string()], avoid: vec!["no source".to_string(), "unknown persona".to_string(), "no trigger".to_string()], exact_paragraphs: None, constraints: EntryConstraints::default(), metadata: BTreeMap::new() },
            Entry { id: "bad-fit-sending-only".to_string(), title: "Bad fit: sending-only ask".to_string(), body: "If the request is only to blast, sequence, or auto-send messages without decision context, treat it as out of scope for MDP.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string()], evidence: vec!["README.md".to_string()], avoid: vec!["blast".to_string(), "auto-send".to_string(), "sequence everyone".to_string()], exact_paragraphs: None, constraints: EntryConstraints::default(), metadata: BTreeMap::new() },
        ])),
        ("signals.yaml", card("signals", CardKind::Signals, "Signals and triggers", "How to interpret source rows, LinkedIn context, source material, and account signals.", &["GTM Engineering", "PMM", "PM"], &["signal", "trigger", "source", "source-row", "csv", "crm", "linkedin"], vec![
            entry_with_evidence("source-row-signal", "Source row signal", "Treat user-provided rows, CSVs, CRM exports, Clay, Deepline, or other supplied row-like inputs as evidence inputs. Preserve source and confidence when present, and state weak signals as hypotheses.", &["GTM Engineering", "PMM"], &["examples/clay-row.json"]),
            entry_with_evidence("linkedin-profile-signal", "LinkedIn profile signal", "Use LinkedIn URLs or profile summaries as context for role, background, and likely priorities. Do not pretend the profile proves a product need by itself.", &["PMM"], &["examples/clay-row.json"]),
            entry_with_evidence("company-context-signal", "Company context signal", "Company website, hiring, funding, product, and stack clues can shape the pain hypothesis when the pack states how to interpret them.", &["PMM", "PM"], &["examples/clay-row.json"]),
        ])),
        ("pains.yaml", card("pains", CardKind::Pains, "Pains and triggers", "Reusable buyer pains with evidence expectations.", &["PMM", "PM"], &["pain", "trigger"], vec![
            entry_with_evidence("agent-context-drift", "Agent context drift", "Agents working on GTM tasks lose product truth when source context, contracts, and approved claims are scattered.", &["PMM", "PM"], &["README.md"]),
            entry_with_evidence("handoff-friction", "Handoff friction", "Teams need a way to give agents enough context to draft or decide without dumping a giant doc into every prompt.", &["GTM Engineering", "PMM"], &["README.md"]),
            entry_with_evidence("claim-inconsistency", "Claim inconsistency", "Different agents or workflows reuse outdated claims, unsupported proof points, or mismatched CTAs when there is no shared pack.", &["PMM"], &["README.md"]),
        ])),
        ("claims.yaml", card("claims", CardKind::Claims, "Approved claims", "Claims an agent may use only when the route and source context support them.", &["PMM", "GTM Engineering"], &["claim", "proof", "evidence"], vec![
            entry_with_evidence("modular-pack-routing", "Modular pack routing", "MDP lets teams store messaging decisions in a manifest plus modular cards so agents load relevant context instead of a giant prompt.", &["PMM", "GTM Engineering"], &["README.md"]),
            entry_with_evidence("local-offline", "Local offline CLI", "The MVP CLI runs locally/offline without auth and returns stable JSON for agent and script usage.", &["GTM Engineering"], &["README.md", "cli/src/main.rs"]),
            entry_with_evidence("versionable-context", "Versionable message context", "A pack can live in a repo so teams can review, diff, test, and update messaging decisions over time.", &["GTM Engineering", "PMM"], &["README.md"]),
        ])),
        ("motions.yaml", card("motions", CardKind::Motions, "Approved motions", "GTM workflows this pack can support as context.", &["GTM Engineering", "PMM"], &["motion", "workflow"], vec![
            entry_with_evidence("copy-brief", "Copy brief", "Route persona, pain, hook, avoid-rules, CTA policy, and copy-pattern cards to produce a grounded brief, not final unsupervised sending.", &["PMM"], &["README.md"]),
            entry_with_evidence("agent-preflight", "Agent preflight", "Let an agent inspect the pack before doing GTM work and report missing evidence or unsupported claims.", &["GTM Engineering"], &["README.md"]),
            entry_with_evidence("source-row-to-brief", "Source row to brief", "Convert a provider-neutral prospect/source row into a message brief before drafting. Keep source fields as inputs, not as proof of claims.", &["GTM Engineering", "PMM"], &["README.md", "examples/clay-row.json"]),
        ])),
        ("channel-policies.yaml", card("channel-policies", CardKind::ChannelPolicies, "Channel policies", "Channel and lifecycle rules for how routed message decisions should be used.", &["GTM Engineering", "PMM"], &["channel", "linkedin", "email", "initial", "follow-up", "call", "prep", "agent", "brief"], vec![
            entry_with_evidence("linkedin-initial-touch", "LinkedIn initial touch", "For a first LinkedIn touch, use one sourced observation or explicitly labeled hypothesis, one relevant angle, and one low-friction ask. Keep it brief and do not make the first note feel like a full pitch.", &["PMM"], &["README.md"]),
            entry_with_evidence("linkedin-follow-up", "LinkedIn follow-up", "For a later LinkedIn note, reference the earlier outreach lightly, add one new relevance angle or question, and keep the ask low-friction. Do not use guilt, breakup framing, or a bare bump.", &["PMM"], &["README.md"]),
            Entry { id: "email-initial-touch".to_string(), title: "Email initial touch".to_string(), body: "For a first cold email, use the email output rules, one source-backed reason or explicit hypothesis, one approved angle, and one reply path. Keep one soft CTA and one question only. Do not lead with a calendar ask unless fit is strong and the source context supports it. Default to no links, attachments, images, HTML polish, or tracking unless the user explicitly overrides.".to_string(), applies_to: vec!["PMM".to_string()], evidence: vec!["README.md".to_string()], avoid: vec![], exact_paragraphs: None, constraints: initial_email_constraints(), metadata: BTreeMap::new() },
            entry_with_evidence("email-follow-up", "Email follow-up", "For follow-up email copy, assume a maximum of three follow-up notes after the initial email. Refer back without assuming interest, add one concrete reason, question, angle, or proof gap, and keep the reply path to owner validation or relevance. Do not use bump language, bare bumps, guilt breakup framing, or imply a longer follow-up sequence than the user supplied.", &["PMM"], &["README.md"]),
            entry_with_evidence("call-prep", "Call prep", "Return likely persona, pains, allowed claims, avoid-rules, open questions, and the exact cards loaded. Do not pretend this is CRM history.", &["GTM Engineering", "PMM"], &["README.md"]),
            entry_with_evidence("agent-brief", "Agent brief", "Return fit status, loaded cards, approved claims, avoid-rules, source hypotheses, open gaps, and exact handoff boundaries. Do not send, enrich, or update external systems.", &["GTM Engineering", "PMM"], &["README.md"]),
        ])),
        ("hooks.yaml", card("hooks", CardKind::Hooks, "Hooks", "Starter hook patterns that require local evidence before use.", &["PMM"], &["hook", "copy", "message"], vec![
            entry_with_evidence("manifest-not-monolith", "Manifest, not monolith", "Position the pack as a small manifest plus task-specific cards so agents load the minimum needed context.", &["PMM"], &["README.md"]),
            entry_with_evidence("evidence-before-action", "Evidence before action", "Emphasize that GTM execution should start with source context, contracts, and approval boundaries.", &["PMM"], &["README.md"]),
            entry_with_evidence("one-context-many-agents", "One context, many agents", "Use when the account has Claude Code, Codex, OpenCode, Clay, or other systems that need the same source of messaging truth.", &["PMM", "GTM Engineering"], &["README.md", "examples/clay-row.json"]),
        ])),
        ("ctas.yaml", card("ctas", CardKind::Ctas, "CTA rules", "Calls to action, reply paths, and ask boundaries for outbound copy.", &["PMM", "GTM Engineering"], &["cta", "ask", "reply", "copy", "outbound", "message"], vec![
            entry_with_evidence("soft-ask", "Soft ask", "Default to a low-friction ask that optimizes for a human reply: compare notes, sanity-check the hypothesis, ask who owns the problem, or ask whether the angle is worth a quick look.", &["PMM", "GTM Engineering"], &["README.md"]),
            entry_with_evidence("calendar-second", "Calendar second", "Do not make the first CTA a calendar booking unless fit is strong, the reason for urgency is sourced, and the channel policy allows it. Use a reply-path question first when fit or ownership is uncertain.", &["PMM", "GTM Engineering"], &["README.md"]),
            entry_with_evidence("no-false-urgency", "No false urgency", "Do not manufacture urgency or imply the prospect has asked for help unless the source row says so.", &["PMM"], &["README.md"]),
            entry_with_evidence("reply-path", "Reply path", "When the best next step is not a meeting, ask a routing question that helps identify the owner, priority, or current workflow.", &["PMM", "GTM Engineering"], &["README.md"]),
        ])),
        ("avoid-rules.yaml", card("avoid-rules", CardKind::AvoidRules, "Avoid rules", "Category and claim boundaries agents must keep intact.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "avoid"], vec![
            Entry { id: "not-execution".to_string(), title: "Do not claim execution".to_string(), body: "Do not describe the decision pack as an AI SDR, sequencer, CRM, enrichment provider, scraper, BI tool, or generic RevOps automation system.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string(), "PM".to_string()], evidence: vec!["README.md".to_string()], avoid: vec!["AI SDR".to_string(), "sequencer".to_string(), "CRM replacement".to_string(), "generic automation".to_string(), "scraper".to_string()], exact_paragraphs: None, constraints: EntryConstraints::default(), metadata: BTreeMap::new() },
            Entry { id: "no-unsourced-claims".to_string(), title: "No unsourced claims".to_string(), body: "Do not add quantified outcomes, integrations, customer names, compliance claims, or product capabilities unless they are present in the claims card or supplied source material.".to_string(), applies_to: vec!["PMM".to_string(), "GTM Engineering".to_string()], evidence: vec![], avoid: vec!["guaranteed".to_string(), "proven ROI".to_string(), "fully automated".to_string()], exact_paragraphs: None, constraints: EntryConstraints::default(), metadata: BTreeMap::new() },
        ])),
        ("output-rules.yaml", card("output-rules", CardKind::OutputRules, "Output rules", "Global style, formatting, and output-structure rules generated text must follow.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "style", "format"], vec![
            Entry { id: "no-em-dashes".to_string(), title: "No em dashes".to_string(), body: "Do not use em dashes in generated copy. Use commas, periods, colons, or shorter sentences instead.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string(), "PM".to_string()], evidence: vec![], avoid: vec!["—".to_string()], exact_paragraphs: None, constraints: EntryConstraints::default(), metadata: BTreeMap::new() },
            Entry { id: "plain-text-by-default".to_string(), title: "Plain text by default".to_string(), body: "For outbound email or LinkedIn copy, default to plain text. Do not include links, attachments, images, HTML, tracking parameters, or decorative formatting unless the user explicitly asks and the pack supports it.".to_string(), applies_to: vec!["PMM".to_string(), "GTM Engineering".to_string()], evidence: vec![], avoid: vec!["http://".to_string(), "https://".to_string(), "<html".to_string(), "<img".to_string(), "utm_".to_string()], exact_paragraphs: None, constraints: EntryConstraints::default(), metadata: BTreeMap::new() },
            entry("initial-email-shape", "Initial email shape", "When drafting an initial cold email, aim for roughly 90-125 words, use a short non-clickbait subject, and avoid fake Re: or Fwd: framing. Put detailed narrative structure in copy-patterns, not here.", &["PMM"]),
            entry("no-fake-personalization", "No fake personalization", "Do not imply the sender read, watched, met, noticed, or personally researched something unless that source context is present. Use hypotheses when the source signal is weak.", &["PMM", "GTM Engineering"]),
            entry("honor-paragraph-count", "Honor paragraph count", "If the user or pack states a paragraph count, match it exactly. Do not add setup, recap, or explanation paragraphs outside the requested structure.", &["PMM", "GTM Engineering", "PM"]),
            entry("no-meta-commentary", "No meta commentary", "Do not explain why the copy works, describe the structure, or include drafting notes unless the user asks for critique or rationale.", &["PMM", "GTM Engineering", "PM"]),
        ])),
        ("copy-patterns.yaml", card("copy-patterns", CardKind::CopyPatterns, "Copy patterns", "Reusable structures for brief and copy outputs.", &["PMM"], &["copy", "brief", "outbound", "message"], vec![
            entry_with_evidence("brief-contract", "Brief contract", "Return audience, job, loaded cards, decision trace, approved claims, avoid rules, open questions, and draft direction.", &["PMM"], &["README.md"]),
            entry_with_evidence("claim-gap", "Claim gap", "When evidence is missing, write the gap explicitly instead of smoothing over it with generic GTM language.", &["PMM", "PM"], &["README.md"]),
            entry_with_evidence("trigger-hypothesis-proof-gap-angle-cta", "Trigger/hypothesis -> proof gap -> angle -> CTA", "Structure outbound copy as observed trigger or explicit hypothesis, proof gap or missing context, approved MDP angle, and one soft CTA. Mark weak inputs as hypotheses instead of fake personalization.", &["PMM"], &["README.md", "examples/clay-row.json"]),
        ])),
        ("objections.yaml", card("objections", CardKind::Objections, "Objections and alternatives", "Category confusion and response logic for agents to preserve.", &["PMM", "GTM Engineering"], &["objection", "alternative", "response"], vec![
            entry_with_evidence("why-not-prompt", "Why not one giant prompt?", "Explain that MDP favors versioned, testable, progressively loaded cards so agents can fetch only the context needed for the current job.", &["PMM", "GTM Engineering"], &["README.md"]),
            entry_with_evidence("why-not-sequencer", "Why not a sequencer?", "Clarify that MDP stores message decisions and evidence. Sequencers or CRMs may consume outputs, but they are separate execution systems.", &["PMM", "GTM Engineering"], &["README.md"]),
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
                "id": "mdp-readme",
                "kind": "repo-doc",
                "locator": "README.md",
                "freshness": "repo-current",
                "confidence": "high",
                "direct_claims": [
                    "MDP is a local/offline standard, CLI, and plugin for modular GTM messaging context.",
                    "MDP stores decision context and routing contracts; it is not execution infrastructure."
                ],
                "interpretations": [
                    "Use this source for category boundaries, not for third-party customer proof."
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

pub(crate) fn starter_evals() -> Vec<(&'static str, Value)> {
    vec![
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
                "expect_load_order_contains": [
                    ".mdp/cards/channel-policies.yaml",
                    ".mdp/cards/copy-patterns.yaml"
                ],
                "expect_entry_titles_contains": ["LinkedIn follow-up"],
                "expect_entry_titles_excludes": ["LinkedIn initial touch", "Email initial touch", "Email follow-up", "Call prep"]
            }),
        ),
        (
            "gtm-engineering-route.yaml",
            json!({
                "id": "gtm-engineering-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "job-routing",
                    &["actors", "source-signals", "routing-jobs"],
                    &["prospect-fit-or-brief"]
                ),
                "persona": "GTM Engineering",
                "job": "agent brief for enriched row",
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
            "pm-route.yaml",
            json!({
                "id": "pm-route",
                "command": "route",
                "profile_eval": eval_profile(
                    "job-routing",
                    &["actors", "boundaries", "output-contracts"],
                    &["create-or-improve-gtm-pack"]
                ),
                "persona": "PM",
                "job": "product boundary review",
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
                "profile_eval": eval_profile(
                    "account-context-present",
                    &["actors", "decision-criteria", "source-signals"],
                    &["prospect-fit-or-brief"]
                ),
                "expect_status": "fit",
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
                "job": "linkedin outbound copy",
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
            "prompt-output-validation.yaml",
            json!({
                "id": "prompt-output-validation",
                "command": "validate-prompt-output",
                "profile_eval": eval_profile(
                    "prompt-output-validation",
                    &["actors", "source-signals", "gaps"],
                    &["prospect-fit-or-brief"]
                ),
                "prompt_id": "normalize-prospect-row",
                "expect_valid": false,
                "prompt_output": {
                    "contract": "mdp.prompt-output.v0",
                    "prompt_id": "normalize-prospect-row",
                    "source_summary": {
                        "account_name": "Northstar Cloud",
                        "company_domain": "northstarcloud.com",
                        "company_name": "Northstar Cloud",
                        "confidence": "medium",
                        "inputs_used": ["company_domain"],
                        "person_name": "N/A",
                        "person_title": "N/A"
                    },
                    "normalized_prospect": {
                        "name": "Alex Rivera",
                        "title": "Revenue Operations Lead",
                        "company": "Northstar Cloud",
                        "company_domain": "northstarcloud.com",
                        "source_kind": "synthetic-example",
                        "synthetic": true,
                        "persona": "GTM Engineering",
                        "segment": "agent-assisted GTM",
                        "trigger": "standardizing prospect qualification data",
                        "signals": [
                            {
                                "id": "qualification-data-standardization",
                                "title": "Standardizing prospect qualification data",
                                "source": "company_domain"
                            }
                        ]
                    },
                    "normalization_trace": {
                        "persona": {
                            "source": "invented",
                            "confidence": "low",
                            "needs_review": true
                        },
                        "fit_readiness": {
                            "ready_for_mdp_fit": true
                        },
                        "missing_required": [],
                        "preserved_raw_fields": ["company_domain"]
                    },
                    "card_patches": [],
                    "gaps": [],
                    "rejected_claims": []
                }
            }),
        ),
        (
            "account-only-normalization-output.yaml",
            json!({
                "id": "account-only-normalization-output",
                "command": "validate-prompt-output",
                "profile_eval": eval_profile(
                    "prompt-output-validation",
                    &["actors", "source-signals", "gaps"],
                    &["prospect-fit-or-brief"]
                ),
                "prompt_id": "normalize-prospect-row",
                "expect_valid": true,
                "expect_normalization_ready": false,
                "prompt_output": {
                    "contract": "mdp.prompt-output.v0",
                    "prompt_id": "normalize-prospect-row",
                    "source_summary": {
                        "account_name": "Northstar Cloud",
                        "company_domain": "northstarcloud.com",
                        "company_name": "Northstar Cloud",
                        "confidence": "medium",
                        "inputs_used": ["raw_row", "company_domain", "existing_pack_context", "source_kind"],
                        "person_name": "N/A",
                        "person_title": "N/A"
                    },
                    "normalized_prospect": {
                        "name": "N/A",
                        "title": "N/A",
                        "company": "Northstar Cloud",
                        "company_domain": "northstarcloud.com",
                        "source_kind": "synthetic-example",
                        "synthetic": true,
                        "segment": "agent-assisted GTM",
                        "trigger": "standardizing prospect qualification data before routing new campaigns",
                        "signals": [
                            {
                                "id": "qualification-data-standardization",
                                "title": "Standardizing prospect qualification data",
                                "source": "raw_row.account_note"
                            }
                        ]
                    },
                    "normalization_trace": {
                        "persona": {
                            "source": "N/A",
                            "matched_keywords": [],
                            "confidence": "unknown",
                            "needs_review": true
                        },
                        "fit_readiness": {
                            "has_company_domain": true,
                            "has_persona": false,
                            "has_segment": true,
                            "has_signal_source": true,
                            "has_signals": true,
                            "has_trigger": true,
                            "ready_for_mdp_fit": false,
                            "ready_for_brief": false,
                            "no_draft_reason": "No person name or title was present in the source row; provide a reviewed contact before drafting."
                        },
                        "missing_required": [
                            {
                                "field": "name",
                                "path": "normalized_prospect.name",
                                "reason": "not_available_in_source",
                                "source_evidence": "Raw row contained account context but no named person."
                            },
                            {
                                "field": "title",
                                "path": "normalized_prospect.title",
                                "reason": "not_available_in_source",
                                "source_evidence": "Raw row contained account context but no person title."
                            },
                            {
                                "field": "persona",
                                "reason": "not_extractable_without_person",
                                "source_evidence": "No reviewed person or role was supplied."
                            }
                        ],
                        "preserved_raw_fields": ["raw_row.company", "raw_row.account_note", "company_domain", "source_kind"]
                    },
                    "card_patches": [],
                    "gaps": [
                        "No person name or title was present in the source row; provide a reviewed contact before drafting."
                    ],
                    "rejected_claims": []
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
                    &["pack-validation"]
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
                "profile_eval": eval_profile(
                    "proceed",
                    &["actors", "decision-criteria", "source-signals"],
                    &["prospect-fit-or-brief"]
                ),
                "expect_status": "fit",
                "prospect": starter_prospect("gtm")
            }),
        ),
        (
            "fit-insufficient-context.yaml",
            json!({
                "id": "fit-insufficient-context",
                "command": "fit",
                "profile_eval": eval_profile(
                    "insufficient-context",
                    &["decision-criteria", "source-signals", "gaps"],
                    &["prospect-fit-or-brief"]
                ),
                "expect_status": "insufficient-context",
                "prospect": {
                    "name": "Taylor Lee",
                    "title": "GTM Engineering Lead",
                    "company": "ExampleCo"
                }
            }),
        ),
        (
            "fit-disqualified.yaml",
            json!({
                "id": "fit-disqualified",
                "command": "fit",
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
                "job": "linkedin outbound copy",
                "expect_draft_status": "no-draft",
                "prospect": {
                    "name": "Taylor Lee",
                    "title": "GTM Engineering Lead",
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
            "claim-check-approved.yaml",
            json!({
                "id": "claim-check-approved",
                "command": "check-claims",
                "profile_eval": eval_profile(
                    "proceed",
                    &["evidence-proof", "boundaries"],
                    &["pack-validation"]
                ),
                "text": "MDP is a local offline CLI that stores versionable message context in a manifest plus modular cards.",
                "expect_valid": true
            }),
        ),
    ]
}

pub(crate) fn starter_prompts(include_output_schemas: bool) -> Vec<(&'static str, Value)> {
    vec![
        (
            "normalize-prospect.yaml",
            prospect_normalization_prompt_contract(include_output_schemas),
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
        "title": "GTM Engineering Lead",
        "company": "ExampleCo",
        "company_domain": "example.com",
        "source_kind": "synthetic-example",
        "synthetic": true,
        "linkedin_url": "https://www.linkedin.com/in/example-mdp-demo",
        "company_url": "https://example.com",
        "background": "building repeatable agent-assisted GTM workflows across Clay, Codex, and Claude Code",
        "trigger": "standardizing outbound context across agents and systems",
        "persona": "GTM Engineering",
        "segment": "agent-assisted GTM",
        "signals": [
            {
                "id": "agent-gtm-workflow",
                "title": "Building multi-agent GTM workflow",
                "source": "example enrichment row",
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
    }
}

fn prospect_normalization_prompt_contract(include_output_schemas: bool) -> Value {
    let mut prompt = json!({
        "format": PROMPT_FORMAT_VERSION,
        "id": "normalize-prospect-row",
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
                "missing_behavior": "Return gaps and do not create normalized_prospect fields from absent source material."
            },
            {
                "name": "company_domain",
                "description": "Company domain when available.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Use N/A and do not infer company identity from absent data."
            },
            {
                "name": "existing_pack_context",
                "description": "Relevant manifest personas, persona_mappings, lead_input_requirements.value_contracts, lead_input_requirements.attribute_definitions, fit rules, signal definitions, avoid-rules, output rules, and source policy from this MDP.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Do not assume pack-owned persona mappings, value domains, fit rules, attributes, or signal names when this field is N/A."
            },
            {
                "name": "runtime_context",
                "description": "Optional MDP runtime context with now_utc, date_utc, timezone UTC, and local_time_policy. Use it only for temporal framing; fiscal year, renewal dates, event dates, and campaign windows remain pack-declared or supplied metadata.",
                "required": false,
                "default": "N/A",
                "missing_behavior": "Do not infer fiscal years, renewal windows, event timing, or local business calendar facts from missing runtime context."
            },
            {
                "name": "source_kind",
                "description": "Provider-neutral source marker such as user-provided-row, csv-row, crm-export-row, clay-row, deepline-row, private-scratch-row, sanitized-example, or synthetic-example.",
                "required": false,
                "default": "user-provided-row",
                "missing_behavior": "Use user-provided-row unless the caller supplies a more specific source kind."
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
            "When existing_pack_context includes lead_input_requirements.attribute_definitions, emit only declared attributes when allow_undeclared_attributes is false, and match declared type, enum, date, or date-time formats. Invalid or unreviewed metadata belongs in gaps or normalization_trace, not attributes.",
            "Preserve uncertainty: weak inferences belong in signal state_as as hypothesis, low confidence, gaps, or normalization_trace.needs_review. Do not smooth away disqualifying execution asks such as scrape contacts, auto-send, sequence everyone, enrich leads, or update CRM.",
            "Keep raw evidence traceable. Each signal should name the supplied source field, note, URL, or row fragment that supports it when available.",
            "If the input is account-only and lacks person name or title, do not invent a contact. Keep compatibility fields as N/A where the prospect schema requires them, add structured normalization_trace.missing_required entries with field, reason, and source_evidence, add a human-readable gap, and set normalization_trace.fit_readiness.ready_for_mdp_fit and ready_for_brief to false.",
            "Missing-field example: if the row has company but no person title, do not fabricate a title; add {\"field\":\"title\",\"reason\":\"not_available_in_source\",\"source_evidence\":\"Raw row contained no person title.\"} to normalization_trace.missing_required and set ready_for_mdp_fit false.",
            "Invalid-value example: if the row says segment enterprise but value_contracts.segment only allows agent-assisted GTM, do not output segment enterprise; add a gap asking for a reviewed pack segment or manifest update.",
            "Keep card_patches empty. This prompt normalizes runtime prospect input; it does not propose edits to MDP cards."
        ],
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
    if include_output_schemas {
        prompt["output_contract"]["schema"] = prospect_normalization_output_schema();
    }
    prompt
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
            "Keep source evidence in evidence and provenance. Do not put prospect facts, proof, citations, or raw source excerpts only in metadata.",
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
            "inputs_used": string_array_output_schema("Input fields used to create this output."),
            "confidence": {
                "enum": ["high", "medium", "low", "unknown"],
                "description": "Overall confidence in the source summary."
            }
        }
    })
}

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
