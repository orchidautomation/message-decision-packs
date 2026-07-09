---
name: mdp-source-strategy
description: Use when the user needs a reviewed, domain-agnostic MDP source strategy before source extraction, pack building, or external scout handoff, including GTM/BDR scouting plans from ICP/account/persona context, proposal/RFP source-intake planning from approved opportunity or corpus context, and mapping any domain's actors, decision criteria, source signals, requirements, evidence, boundaries, output contracts, routing jobs, gaps, and evals into source targets, queries, exclusions, evidence rules, and review checks.
---

# MDP Source Strategy

Create a reviewed source strategy artifact for MDP work. The artifact tells a human, autonomous agent runtime, or approved external scout what to look for, where to look, what not to touch, which evidence is required, and how results should route back into MDP primitives.

MDP remains the decision/context layer. This skill may create a strategy or handoff artifact; it must not run scraping, outreach, enrichment, CRM writes, proposal submission, private/gated extraction, or tool-side execution.

## Use Another Skill Instead

- Use `$mdp-source-extract` when the source material is already supplied or fetched and the user wants pack-ready card entries, `.mdp/sources.yaml`, evidence, confidence, and gaps.
- Use `$mdp-icp-builder` when the user wants to codify GTM ICP, account/persona fit, triggers, disqualifiers, pains, or no-message logic into pack cards.
- Use `$mdp-proposal-pack-builder` when the user wants to build or improve a proposal/RFP pack from approved source material.
- Use proposal review skills (`$mdp-proposal-bid-no-bid-review`, `$mdp-proposal-compliance-review`, `$mdp-proposal-win-theme-proof-review`, `$mdp-proposal-red-team-gap-review`) when reviewing supplied proposal context or drafts against an existing proposal pack.
- Use `$mdp-pack-review` or `$mdp-pack-eval` when the pack already exists and the task is structural review, route QA, or eval coverage.

If the user asks to run Exa, Firecrawl, Apify, browser scraping, enrichment, or a CRM/sequencer action, do not execute it here. Produce a bounded handoff only when the requested source class is public/approved and the user has explicitly authorized an external execution step outside MDP.

## Progressive References

- Read `references/output-contract.md` before producing a source strategy artifact.
- Read `references/profile-patterns.md` when the task is GTM/BDR scouting, proposal source-intake planning, or needs concrete profile examples.

## Profile And Privacy Gate

When working inside an existing pack and editing pack files, run:

```bash
mdp --json agent-surface --dir .
```

Honor `blocked_skills`, `recommended_skills`, and `allowed_skills` for pack edits. For strategy-only artifacts, use any available profile metadata as context but do not force pack changes.

Classify every source target before recommending it:

- `user-provided-approved`: supplied or approved by the user for this work.
- `approved-corpus`: local/internal corpus explicitly approved for review.
- `public-source`: public, unauthenticated, sourceable material.
- `synthetic-example` or `sanitized-example`: safe examples or fixtures.
- `needs-approval`: potentially useful but not yet approved.
- `excluded`: private, gated, authenticated, regulated, personal, unsafe, or outside scope.

## Workflow

1. **Classify the objective.** Name the profile (`gtm`, `proposal`, or domain-specific), the decision being supported, the downstream consumer, and whether this is strategy-only or a handoff to a human/tool scout.
2. **Map universal primitives.** For `actors`, `decision-criteria`, `source-signals`, `needs-requirements`, `evidence-proof`, `boundaries`, `output-contracts`, `routing-jobs`, `gaps`, and `evals`, list what is known, what source evidence is needed, and what must stay a gap.
3. **Design source targets.** Prefer user-provided, approved-corpus, and public primary sources. Include why each target matters, allowed access, freshness needs, and primitive coverage.
4. **Draft queries and scout prompts.** Include search terms, target URLs/domains/files, negative filters, expected source signals, citation requirements, and direct agent instructions. For Exa/Firecrawl/Apify-style scouts, write the contract as a public/approved-source handoff with max scope, not as an executed crawl. The prompt block should tell the agent how to construct queries, how to choose the provider, how to extract evidence, and when to stop.
5. **Define exclusions and boundaries.** Explicitly reject private/gated extraction, personal contact enrichment, CRM writes, outreach, proposal submission, invented proof, and unapproved confidential corpus use.
6. **Define evidence requirements.** State what counts as proof, minimum citations, freshness, confidence, pass/fail conditions, review owner, and what should become a gap or avoid-rule.
7. **Add routing jobs.** Name the next MDP skill, CLI command, or review job for accepted results, such as `mdp-source-extract`, `mdp fit`, `mdp brief --context`, `mdp check-claims`, ledger append, `mdp-icp-builder`, `bid-no-bid-review`, `compliance-review`, or a domain-specific review route.
8. **Add eval checks.** Include proceed, insufficient-context, refusal/exclusion, unsafe-output, and job-routing checks that prove the strategy would not over-collect or over-claim.
9. **Set review status.** Mark the artifact `draft`, `needs-human-review`, `accepted`, or `blocked`; do not call it accepted without reviewer approval.

## Required Output

Return or save a normalized `mdp.source-strategy.v0` artifact with:

- `profile` and `objective`
- `agent_operating_plan`
- `primitive_mappings`
- `source_targets`
- `queries_prompts`
- `exclusions`
- `evidence_requirements`
- `routing_jobs`
- `gaps`
- `eval_checks`
- `review_status`

Use concise YAML or JSON. Keep raw private source text out of committed paths; use ignored scratch for private review artifacts.

## GTM / BDR Example

For ICP/account/persona context, produce a scout strategy that turns target context into public-source search terms, source targets, trigger patterns, negative filters, evidence requirements, route hints, and eval checks.

Example source targets: account website, press/news pages, careers pages, product docs, security/compliance pages, public funding or filing pages, public job posts, and user-approved account notes.

Example handoff rows:

```yaml
profile: {id: gtm, label: GTM / BDR scouting}
objective: {decision_needed: "Find source-backed account trigger evidence before outreach brief generation"}
agent_operating_plan:
  role: "Public-source BDR scout planner"
  operating_instructions:
    - "Start from the approved ICP, persona, and account context; do not invent missing account facts."
    - "Use Exa for broad discovery, Firecrawl only for accepted public URLs, and Apify only for approved public listing pages."
    - "Stop and record a gap when a trigger cannot be supported by a public URL, observed date, and bounded snippet."
  insufficient_evidence_action: "Do not qualify the candidate; preserve the missing signal as a gap for human review."
  downstream_handoff_prompt: "After review, pass accepted evidence to `mdp fit --context <candidate.json>` and then `mdp brief --context <fit-result.json>`; run `mdp check-claims` before any ledger append."
queries_prompts:
  - id: public-trigger-scout
    scout_family: exa
    query_or_prompt: "{{account_name}} AND (hiring OR expansion OR migration OR funding OR launch) -jobs-spam"
    agent_instruction: "Search for current, public account-level trigger evidence. Return only results with a URL, date, snippet, and explicit link to the ICP signal."
    construction_rules:
      - "Combine account or market terms with one trigger family at a time."
      - "Add negative filters for personal contact data, login-required pages, and unverified directories."
    negative_filters: ["personal email", "login-required", "unverified contact database"]
    expected_signals: ["current trigger", "source URL", "date", "confidence", "ICP relevance"]
evidence_requirements:
  - id: public-trigger-minimum
    applies_to: source-signals
    minimum_evidence: "One public primary source or two independent public secondary sources."
    pass_condition: "The trigger is current, tied to a named company/team, and includes a bounded snippet."
    fail_condition: "The source is gated, contact-enrichment-only, stale without operator approval, or only implies buying intent."
routing_jobs:
  - id: extract-accepted-signals
    next_skill: mdp-source-extract
    handoff: "Only reviewed public/source-approved evidence becomes card candidates."
    cli_handoff: "Run `mdp fit --context <candidate.json>`, then `mdp brief --context <fit-result.json>`, then `mdp check-claims --context <brief.json>` before appending a reviewed ledger row."
```

## Proposal Example

For an opportunity, RFP, or approved corpus, produce a source-intake plan. Do not imply autonomous web scraping is the default for proposal work. Default to user-provided/approved corpus, then explicitly approved public sources only.

Example source targets: RFP files, amendments, Q&A, SOW/PWS sections, evaluation criteria, submission instructions, compliance clauses, approved proof library, approved past performance, security/compliance policy excerpts, and capture notes approved for this pack.

Example handoff rows:

```yaml
profile: {id: proposal, label: Proposal source intake}
objective: {decision_needed: "Inventory requirement, proof, compliance, and review sources before proposal pack extraction"}
source_targets:
  - id: approved-rfp-corpus
    source_kind: approved-corpus
    purpose: "Requirements, evaluation criteria, submission instructions, and amendments"
    primitives: [source-signals, needs-requirements, decision-criteria, gaps]
exclusions:
  - "Do not scrape procurement portals or gated repositories unless the user supplies approved exports."
queries_prompts:
  - id: approved-corpus-requirement-extract
    scout_family: local-corpus
    query_or_prompt: "Review only the approved RFP/Q&A/proof-library files. Extract requirement IDs, evaluation criteria, compliance clauses, proof claims, source file names, page/section references, and gaps."
    agent_instruction: "Do not browse for missing proposal facts. If the approved corpus lacks support, mark an unsupported-proof or compliance gap."
    construction_rules:
      - "Prefer requirement IDs, section headings, page numbers, and amendment dates over paraphrase-only notes."
      - "Keep source excerpts short and cite the approved file or corpus ID."
routing_jobs:
  - {id: compliance-intake, next_skill: mdp-proposal-pack-builder, review_job: compliance-review}
```
