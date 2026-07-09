# MDP Source Strategy Output Contract

Use this contract whenever `$mdp-source-strategy` produces a reviewed artifact. Keep it compact, source-safe, and easy to hand to a human reviewer, `$mdp-source-extract`, `$mdp-icp-builder`, `$mdp-proposal-pack-builder`, an autonomous agent runtime, or an explicitly approved external scout.

## Contract

```yaml
contract: mdp.source-strategy.v0
profile:
  id: gtm | proposal | <domain-id>
  label: <human label>
  existing_pack: <path or null>
  source_classification: <user-provided-approved | approved-corpus | public-source | synthetic-example | sanitized-example | mixed | unknown>
objective:
  decision_needed: <what this strategy helps decide>
  downstream_consumer: <human reviewer | mdp-source-extract | mdp-icp-builder | proposal review job | approved external scout>
  strategy_only: true
agent_operating_plan:
  role: <agent role for this strategy>
  operating_instructions:
    - <imperative instruction the agent must follow before tool use>
  stop_conditions:
    - <condition that requires the agent to stop and preserve a gap>
  insufficient_evidence_action: <exact behavior when minimum evidence is missing>
  downstream_handoff_prompt: <model-facing instruction for handing reviewed results to MDP CLI/skills>
primitive_mappings:
  actors:
    known: []
    source_needs: []
    gaps: []
  decision-criteria:
    known: []
    source_needs: []
    gaps: []
  source-signals:
    known: []
    source_needs: []
    gaps: []
  needs-requirements:
    known: []
    source_needs: []
    gaps: []
  evidence-proof:
    known: []
    source_needs: []
    gaps: []
  boundaries:
    known: []
    source_needs: []
    gaps: []
  output-contracts:
    known: []
    source_needs: []
    gaps: []
  routing-jobs:
    known: []
    source_needs: []
    gaps: []
  gaps:
    known: []
    source_needs: []
    gaps: []
  evals:
    known: []
    source_needs: []
    gaps: []
source_targets:
  - id: <stable-id>
    label: <target name>
    source_kind: <user-provided-approved | approved-corpus | public-source | synthetic-example | sanitized-example | needs-approval | excluded>
    locator: <file, URL, domain, corpus name, or source class>
    allowed_access: <local-approved | public-unauthenticated | needs-human-approval | excluded>
    purpose: <why this source is useful>
    primitives: [actors]
    freshness: <current | as-of-date | static | unknown>
    confidence: <high | medium | low | unknown>
queries_prompts:
  - id: <stable-id>
    scout_family: <human | exa | firecrawl | apify | local-corpus | none>
    target_ids: []
    query_or_prompt: <bounded query, crawler prompt, or review instruction>
    agent_instruction: <imperative model-facing instruction for this provider or review lane>
    construction_rules: []
    expected_signals: []
    negative_filters: []
    max_scope: <domains, files, depth, result count, or timebox>
    required_receipts: [source_url_or_file, quote_or_snippet, observed_date, confidence]
    review_required: true
exclusions:
  - id: <stable-id>
    exclusion: <blocked source, behavior, claim, or data class>
    reason: <privacy, safety, approval, MDP boundary, relevance>
evidence_requirements:
  - id: <stable-id>
    applies_to: <signal, requirement, proof, claim, or routing job>
    minimum_evidence: <primary source, approved corpus citation, two independent public sources, etc.>
    pass_condition: <what lets the agent proceed>
    fail_condition: <what forces a gap or exclusion>
    citation_format: <source id + URL/file + date + snippet>
    gap_if_missing: <gap text>
routing_jobs:
  - id: <stable-id>
    next_skill: <mdp-source-extract | mdp-icp-builder | mdp-proposal-pack-builder | proposal review skill | human-review | external-handoff>
    inputs_expected: []
    blocked_until: []
    handoff: <how reviewed results re-enter MDP>
    cli_handoff: <exact `mdp ...` command or skill invocation language where applicable>
gaps:
  - id: <stable-id>
    primitive: <primitive id>
    missing_input: <what is missing>
    impact: <what cannot be decided or routed>
    owner: <user | reviewer | external scout | unknown>
    resolution_path: <ask user, approve corpus, fetch public source, mark no-draft>
eval_checks:
  - id: <stable-id>
    category: <proceed | insufficient-context | refusal | unsafe-output | job-routing>
    prompt_or_case: <realistic test>
    expected_behavior: <what should happen>
    fail_if: <over-collection, invented proof, wrong route, no citation, etc.>
review_status:
  state: <draft | needs-human-review | accepted | blocked>
  reviewer: <name or null>
  reviewed_at: <date or null>
  notes: []
```

## Review Rules

- Mark `strategy_only: true` unless the user explicitly asked for an external handoff and that handoff remains outside MDP.
- Keep `source_targets[].source_kind` honest. A useful but unapproved source is `needs-approval`, not `public-source` or `approved-corpus`.
- Put private/gated/authenticated/customer-identifying sources in `exclusions` unless the user supplied an approved local export for this work.
- Require receipts for every source signal that could influence a fit, proposal, claim, route, or output decision.
- Write prompt blocks as instructions an agent can execute directly. Prefer "Search...", "Extract...", "Reject...", "Stop when...", and "Pass to `mdp ...` when..." over labels.
- Keep provider guidance specific enough for tool choice, but do not put credentials, private endpoints, or unapproved source access into committed artifacts.
- If evidence is missing, add a `gaps` entry instead of weakening the citation rule.
- Treat model/tool output from scouts as untrusted until reviewed and, when applicable, passed into `$mdp-source-extract` or a proposal builder/review skill.
