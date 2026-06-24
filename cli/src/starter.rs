use crate::constants::FORMAT_VERSION;
use crate::models::{Card, CardKind, CardRef, Entry, Manifest, Policy, Provenance};
use serde_json::{Value, json};

pub(crate) fn starter_manifest(name: &str, slug: &str, _template: &str) -> Manifest {
    let personas = vec![
        "GTM Engineering".to_string(),
        "PMM".to_string(),
        "PM".to_string(),
    ];
    Manifest {
        format: FORMAT_VERSION.to_string(),
        id: slug.to_string(),
        name: name.to_string(),
        version: "0.1.0".to_string(),
        description: Some("A modular message decision pack for agent-readable ICP, pains, triggers, proof, CTA policy, avoid-rules, and copy guidance.".to_string()),
        personas: personas.clone(),
        target_personas: personas,
        operator_roles: vec!["GTM Engineering".to_string(), "PMM".to_string()],
        supported_channels: vec!["linkedin".to_string(), "email".to_string(), "call-prep".to_string(), "agent-brief".to_string()],
        cards: vec![
            card_ref("personas", "cards/personas.yaml", CardKind::Personas, "Who the decision pack serves and what each persona needs.", &["GTM Engineering", "PMM", "PM"], &["persona"]),
            card_ref("positioning", "cards/positioning.yaml", CardKind::Positioning, "Category, product boundaries, value pillars, and what this pack is not.", &["GTM Engineering", "PMM", "PM"], &["positioning", "category", "boundary"]),
            card_ref("fit-rules", "cards/fit-rules.yaml", CardKind::FitRules, "ICP, fit, disqualification, and no-message rules.", &["GTM Engineering", "PMM", "PM"], &["fit", "icp", "disqualifier", "no-message"]),
            card_ref("signals", "cards/signals.yaml", CardKind::Signals, "Structured buying signals, triggers, and source interpretation rules.", &["GTM Engineering", "PMM", "PM"], &["signal", "trigger", "source", "clay", "deepline", "linkedin"]),
            card_ref("pains", "cards/pains.yaml", CardKind::Pains, "Buyer pains, triggers, and evidence requirements.", &["PMM", "PM"], &["pain", "trigger"]),
            card_ref("claims", "cards/claims.yaml", CardKind::Claims, "Approved claims and proof requirements an agent may use.", &["PMM", "GTM Engineering"], &["claim", "proof", "evidence"]),
            card_ref("motions", "cards/motions.yaml", CardKind::Motions, "Approved GTM motions and motion boundaries.", &["GTM Engineering", "PMM"], &["motion", "workflow"]),
            card_ref("channel-policies", "cards/channel-policies.yaml", CardKind::ChannelPolicies, "Channel-specific policy for LinkedIn, email, call prep, and agent briefs.", &["GTM Engineering", "PMM"], &["channel", "linkedin", "email", "brief"]),
            card_ref("hooks", "cards/hooks.yaml", CardKind::Hooks, "Messaging hooks that can be reused after evidence checks.", &["PMM"], &["hook", "copy", "message"]),
            card_ref("ctas", "cards/ctas.yaml", CardKind::Ctas, "CTA rules, reply paths, and ask boundaries for outbound copy.", &["PMM", "GTM Engineering"], &["cta", "ask", "reply", "copy", "outbound", "message"]),
            card_ref("avoid-rules", "cards/avoid-rules.yaml", CardKind::AvoidRules, "Claims and categories the agent must avoid.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "avoid"]),
            card_ref("copy-patterns", "cards/copy-patterns.yaml", CardKind::CopyPatterns, "Copy structures and brief patterns for GTM outputs.", &["PMM"], &["copy", "brief", "outbound", "message"]),
            card_ref("objections", "cards/objections.yaml", CardKind::Objections, "Expected objections, category confusion, and approved response logic.", &["PMM", "GTM Engineering"], &["objection", "alternative", "response"]),
            card_ref("gaps", "cards/gaps.yaml", CardKind::Gaps, "Known gaps and open questions agents must surface instead of filling in.", &["GTM Engineering", "PMM", "PM"], &["gap", "unknown", "open-question"]),
        ],
        policy: Policy { progressive_disclosure: true, load_manifest_first: true, max_cards_per_route: 12, json_contract: "mdp.cli.v0".to_string(), no_auth_required: true },
        provenance: Provenance { owner: "local".to_string(), created_by: "mdp init".to_string(), notes: vec!["This pack is guidance and evidence context, not an execution system.".to_string(), "Agents should load only routed cards unless the user asks for a full audit.".to_string()] },
    }
}

pub(crate) fn starter_cards(_template: &str) -> Vec<(&'static str, Card)> {
    vec![
        ("personas.yaml", card("personas", CardKind::Personas, "Core personas", "The users who author, maintain, and consume the decision pack.", &["GTM Engineering", "PMM", "PM"], &["persona"], vec![
            entry("gtm-engineering", "GTM Engineering", "Needs precise contracts, data boundaries, approved workflows, and machine-readable routing.", &["GTM Engineering"]),
            entry("pmm", "PMM", "Needs pains, triggers, hooks, proof points, CTA policy, and copy constraints without losing source fidelity.", &["PMM"]),
            entry("pm", "PM", "Needs product boundaries, roadmap-relevant pain evidence, and clear decisions about what the product is not.", &["PM"]),
        ])),
        ("positioning.yaml", card("positioning", CardKind::Positioning, "Positioning and boundaries", "Category and product truth that every routed brief should preserve.", &["GTM Engineering", "PMM", "PM"], &["positioning", "category", "boundary"], vec![
            entry_with_evidence("decision-layer", "Decision/context layer", "Describe MDP as a local, agent-readable decision and context layer for GTM messaging. It stores what an agent should load, believe, avoid, and surface as a gap.", &["GTM Engineering", "PMM", "PM"], &["README.md"]),
            entry_with_evidence("not-execution-system", "Not execution", "Do not describe MDP as a sender, CRM, sequencer, enrichment provider, scraper, AI SDR, BI tool, or generic automation system.", &["GTM Engineering", "PMM", "PM"], &["README.md"]),
            entry_with_evidence("progressive-disclosure", "Progressive disclosure", "The pack is a small manifest plus modular cards. Agents should load only the cards returned by route or brief commands.", &["GTM Engineering", "PMM"], &["README.md"]),
        ])),
        ("fit-rules.yaml", card("fit-rules", CardKind::FitRules, "Fit rules", "ICP, qualification, disqualification, and no-message rules.", &["GTM Engineering", "PMM", "PM"], &["fit", "icp", "disqualifier", "no-message"], vec![
            entry("good-fit-agent-gtm", "Good fit: agent-assisted GTM", "Use when the account is building GTM workflows with agents, Clay/Deepline-style enrichment rows, Codex/Claude Code/OpenCode, or multiple systems that need consistent message context.", &["GTM Engineering", "PMM"]),
            Entry { id: "no-context-no-copy".to_string(), title: "No message without context".to_string(), body: "If the row has no persona, trigger, source, or useful account context, return insufficient-context instead of drafting polished copy.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string()], evidence: vec![], avoid: vec!["no source".to_string(), "unknown persona".to_string(), "no trigger".to_string()] },
            Entry { id: "bad-fit-sending-only".to_string(), title: "Bad fit: sending-only ask".to_string(), body: "If the request is only to blast, sequence, or auto-send messages without decision context, treat it as out of scope for MDP.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string()], evidence: vec![], avoid: vec!["blast".to_string(), "auto-send".to_string(), "sequence everyone".to_string()] },
        ])),
        ("signals.yaml", card("signals", CardKind::Signals, "Signals and triggers", "How to interpret enriched rows, LinkedIn context, source material, and account signals.", &["GTM Engineering", "PMM", "PM"], &["signal", "trigger", "source", "clay", "deepline", "linkedin"], vec![
            entry("enriched-row-signal", "Enriched row signal", "Treat Clay, Deepline, CSV, or user-provided enrichment as input evidence. Preserve source and confidence when present, and state weak signals as hypotheses.", &["GTM Engineering", "PMM"]),
            entry("linkedin-profile-signal", "LinkedIn profile signal", "Use LinkedIn URLs or profile summaries as context for role, background, and likely priorities. Do not pretend the profile proves a product need by itself.", &["PMM"]),
            entry("company-context-signal", "Company context signal", "Company website, hiring, funding, product, and stack clues can shape the pain hypothesis when the pack states how to interpret them.", &["PMM", "PM"]),
        ])),
        ("pains.yaml", card("pains", CardKind::Pains, "Pains and triggers", "Reusable buyer pains with evidence expectations.", &["PMM", "PM"], &["pain", "trigger"], vec![
            entry("agent-context-drift", "Agent context drift", "Agents working on GTM tasks lose product truth when source context, contracts, and approved claims are scattered.", &["PMM", "PM"]),
            entry("handoff-friction", "Handoff friction", "Teams need a way to give agents enough context to draft or decide without dumping a giant doc into every prompt.", &["GTM Engineering", "PMM"]),
            entry("claim-inconsistency", "Claim inconsistency", "Different agents or workflows reuse outdated claims, unsupported proof points, or mismatched CTAs when there is no shared pack.", &["PMM"]),
        ])),
        ("claims.yaml", card("claims", CardKind::Claims, "Approved claims", "Claims an agent may use only when the route and source context support them.", &["PMM", "GTM Engineering"], &["claim", "proof", "evidence"], vec![
            entry_with_evidence("modular-pack-routing", "Modular pack routing", "MDP lets teams store messaging decisions in a manifest plus modular cards so agents load relevant context instead of a giant prompt.", &["PMM", "GTM Engineering"], &["README.md"]),
            entry_with_evidence("local-offline", "Local offline CLI", "The MVP CLI runs locally/offline without auth and returns stable JSON for agent and script usage.", &["GTM Engineering"], &["README.md", "cli/src/main.rs"]),
            entry_with_evidence("versionable-context", "Versionable message context", "A pack can live in a repo so teams can review, diff, test, and update messaging decisions over time.", &["GTM Engineering", "PMM"], &["README.md"]),
        ])),
        ("motions.yaml", card("motions", CardKind::Motions, "Approved motions", "GTM workflows this pack can support as context.", &["GTM Engineering", "PMM"], &["motion", "workflow"], vec![
            entry("copy-brief", "Copy brief", "Route persona, pain, hook, avoid-rules, CTA policy, and copy-pattern cards to produce a grounded brief, not final unsupervised sending.", &["PMM"]),
            entry("agent-preflight", "Agent preflight", "Let an agent inspect the pack before doing GTM work and report missing evidence or unsupported claims.", &["GTM Engineering"]),
            entry("clay-row-to-brief", "Enriched row to brief", "Convert a Clay/Deepline-style row into a message brief before drafting. Keep source fields as inputs, not as proof of claims.", &["GTM Engineering", "PMM"]),
        ])),
        ("channel-policies.yaml", card("channel-policies", CardKind::ChannelPolicies, "Channel policies", "Channel-specific rules for how to use the routed message decisions.", &["GTM Engineering", "PMM"], &["channel", "linkedin", "email", "brief"], vec![
            entry("linkedin-opener", "LinkedIn opener", "Keep the opener short, use one sourced or explicitly hypothetical trigger, one relevant angle, and one low-friction ask.", &["PMM"]),
            entry("email-follow-up", "Email follow-up", "Use a clearer subject, one source-backed reason for relevance, one approved claim, and a reply path that does not force a meeting too early.", &["PMM"]),
            entry("call-prep", "Call prep", "Return likely persona, pains, allowed claims, avoid-rules, open questions, and the exact cards loaded. Do not pretend this is CRM history.", &["GTM Engineering", "PMM"]),
        ])),
        ("hooks.yaml", card("hooks", CardKind::Hooks, "Hooks", "Starter hook patterns that require local evidence before use.", &["PMM"], &["hook", "copy", "message"], vec![
            entry("manifest-not-monolith", "Manifest, not monolith", "Position the pack as a small manifest plus task-specific cards so agents load the minimum needed context.", &["PMM"]),
            entry("evidence-before-action", "Evidence before action", "Emphasize that GTM execution should start with source context, contracts, and approval boundaries.", &["PMM"]),
            entry("one-context-many-agents", "One context, many agents", "Use when the account has Claude Code, Codex, OpenCode, Clay, or other systems that need the same source of messaging truth.", &["PMM", "GTM Engineering"]),
        ])),
        ("ctas.yaml", card("ctas", CardKind::Ctas, "CTA rules", "Calls to action, reply paths, and ask boundaries for outbound copy.", &["PMM", "GTM Engineering"], &["cta", "ask", "reply", "copy", "outbound", "message"], vec![
            entry("soft-ask", "Soft ask", "Default to a low-friction ask such as comparing notes, sanity-checking the hypothesis, or asking who owns the problem.", &["PMM", "GTM Engineering"]),
            entry("no-false-urgency", "No false urgency", "Do not manufacture urgency or imply the prospect has asked for help unless the source row says so.", &["PMM"]),
            entry("reply-path", "Reply path", "When the best next step is not a meeting, ask a routing question that helps identify the owner, priority, or current workflow.", &["PMM", "GTM Engineering"]),
        ])),
        ("avoid-rules.yaml", card("avoid-rules", CardKind::AvoidRules, "Avoid rules", "Category and claim boundaries agents must keep intact.", &["GTM Engineering", "PMM", "PM"], &["guardrail", "avoid"], vec![
            Entry { id: "not-execution".to_string(), title: "Do not claim execution".to_string(), body: "Do not describe the decision pack as an AI SDR, sequencer, CRM, enrichment provider, scraper, BI tool, or generic RevOps automation system.".to_string(), applies_to: vec!["GTM Engineering".to_string(), "PMM".to_string(), "PM".to_string()], evidence: vec!["README.md".to_string()], avoid: vec!["AI SDR".to_string(), "sequencer".to_string(), "CRM replacement".to_string(), "generic automation".to_string(), "scraper".to_string()] },
            Entry { id: "no-unsourced-claims".to_string(), title: "No unsourced claims".to_string(), body: "Do not add quantified outcomes, integrations, customer names, compliance claims, or product capabilities unless they are present in the claims card or supplied source material.".to_string(), applies_to: vec!["PMM".to_string(), "GTM Engineering".to_string()], evidence: vec![], avoid: vec!["guaranteed".to_string(), "proven ROI".to_string(), "fully automated".to_string()] },
        ])),
        ("copy-patterns.yaml", card("copy-patterns", CardKind::CopyPatterns, "Copy patterns", "Reusable structures for brief and copy outputs.", &["PMM"], &["copy", "brief", "outbound", "message"], vec![
            entry("brief-contract", "Brief contract", "Return audience, job, loaded cards, decision trace, approved claims, avoid rules, open questions, and draft direction.", &["PMM"]),
            entry("claim-gap", "Claim gap", "When evidence is missing, write the gap explicitly instead of smoothing over it with generic GTM language.", &["PMM", "PM"]),
            entry("trigger-pain-angle-ask", "Trigger -> pain -> angle -> ask", "Structure outbound copy as observed trigger, likely pain, approved angle, and low-friction ask. Mark weak inputs as hypotheses.", &["PMM"]),
        ])),
        ("objections.yaml", card("objections", CardKind::Objections, "Objections and alternatives", "Category confusion and response logic for agents to preserve.", &["PMM", "GTM Engineering"], &["objection", "alternative", "response"], vec![
            entry("why-not-prompt", "Why not one giant prompt?", "Explain that MDP favors versioned, testable, progressively loaded cards so agents can fetch only the context needed for the current job.", &["PMM", "GTM Engineering"]),
            entry("why-not-sequencer", "Why not a sequencer?", "Clarify that MDP stores message decisions and evidence. Sequencers or CRMs may consume outputs, but they are separate execution systems.", &["PMM", "GTM Engineering"]),
        ])),
        ("gaps.yaml", card("gaps", CardKind::Gaps, "Known gaps", "Durable gaps and open questions agents should surface instead of inventing answers.", &["GTM Engineering", "PMM", "PM"], &["gap", "unknown", "open-question"], vec![
            entry("missing-company-proof", "Missing company-specific proof", "If a prospect/account row lacks concrete source context, ask for source material or state the personalization gap before drafting.", &["PMM", "GTM Engineering"]),
            entry("unclear-fit", "Unclear fit", "If role, segment, or trigger does not map to a fit rule, return insufficient-context instead of forcing a message.", &["GTM Engineering", "PMM"]),
            entry("hosted-api-not-included", "Hosted API not included", "The MVP is local/offline. Do not imply a hosted API exists unless the user has deployed one separately.", &["GTM Engineering", "PMM"]),
        ])),
    ]
}

pub(crate) fn starter_eval() -> Value {
    json!({
        "id": "linkedin-copy-route",
        "persona": "PMM",
        "job": "linkedin outbound copy",
        "expect_load_order_contains": [
            ".mdp/cards/personas.yaml",
            ".mdp/cards/avoid-rules.yaml",
            ".mdp/cards/positioning.yaml",
            ".mdp/cards/claims.yaml",
            ".mdp/cards/ctas.yaml"
        ]
    })
}

pub(crate) fn starter_prospect(_template: &str) -> Value {
    json!({
        "name": "Alex Rivera",
        "title": "GTM Engineering Lead",
        "company": "ExampleCo",
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
    }
}
