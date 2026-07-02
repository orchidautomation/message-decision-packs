# Conceptual Decision Flow

Message Decision Packs (MDP) are a local decision/context layer for GTM messaging. An MDP pack stores the decisions an agent needs before writing or evaluating a message: fit rules, personas, pains, hooks, approved claims, CTA and channel policy, avoid-rules, output-rules, evidence, and gaps.

MDP does not send messages, enrich leads, update CRM, scrape the web, run sequences, or act as an AI SDR. It decides what context is allowed into the drafting task and what should block the task.

## Mental Model

A provider-neutral prospect/source row supplies row-level inputs. The row can come from a user note, CSV, CRM export, Clay, Deepline, spreadsheet, or research workflow after it has been normalized into MDP prospect JSON. The pack supplies both modular decision entries and prompt contracts. The prompt contracts help upstream agents normalize messy source data; the CLI applies the reviewed pack decisions without asking a model to make the final fit or route decision.

```text
messy source row
  |
  v
.mdp/prompts/normalize-prospect.yaml
  |
  v
prospect JSON
  |
  +-- title/persona
  +-- company_domain
  +-- segment
  +-- trigger
  +-- signals
  +-- attributes
  +-- background
  +-- source/provenance
  |
  v
fit gate
  |
  +-- disqualified ----------> stop, no draft
  |
  +-- insufficient-context --> stop, ask for missing context
  |
  v
persona
  |
  v
pains and signals
  |
  v
hooks
  |
  v
claims and proof
  |
  v
CTA and channel policy
  |
  v
avoid-rules
  |
  v
output-rules
  |
  v
bounded context for drafting
```

The `trigger` field is an input from the prospect JSON. It means why this message is timely now. It is not itself a pack card. The relevant card entries explain how to interpret the trigger, which pains or hooks it can support, which claims are allowed, and which claims remain out of bounds.

## Row Inputs

The prospect schema still admits legacy rows with `name`, `title`, and `company`, but new lead workflows should also provide `company_domain`. `company` stays useful for human-readable drafts; `company_domain` is the stronger account routing key when the pack requires it.

`mdp fit` canonicalizes supplied domains and URLs locally. For example, `https://www.apple.com/` becomes `apple.com`. It does not browse, DNS-check, enrich, or infer a domain from a company name.

Good routing needs more than the admission fields:

```json
{
  "name": "Alex Rivera",
  "title": "GTM Engineering Lead",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "persona": "GTM Engineering",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents and systems",
  "background": "building repeatable agent-assisted GTM workflows across Clay, Codex, and Claude Code",
  "source_kind": "synthetic-example",
  "synthetic": true,
  "attributes": {
    "fiscal_year": "FY2027"
  },
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
}
```

The row should preserve source and confidence when available. Weak signals should stay hypotheses. Use `attributes` only for bounded reviewed metadata such as fiscal year, segment tier, or another pack-specific routing value. Put evidence in `signals` with `source`, not in `attributes`.

Packs can declare readiness requirements in `manifest.yaml`:

```yaml
lead_input_requirements:
  required_fields:
    - name
    - title
    - company_domain
    - trigger
    - persona
    - segment
    - signals
  required_signal_fields:
    - source
  required_attributes:
    - fiscal_year
  value_contracts:
    segment:
      type: string
      enum:
        - agent-assisted GTM
    source_kind:
      type: string
      enum:
        - user-provided-row
        - csv-row
        - crm-export-row
        - clay-row
        - deepline-row
        - private-scratch-row
        - sanitized-example
        - synthetic-example
  attribute_definitions:
    fiscal_year:
      type: string
      description: Optional reviewed account metadata.
```

A row can parse successfully and still return `insufficient-context` if it does not satisfy the pack's declared requirements or emits a value outside the pack's declared enum/type/date contracts.

If the input is account-only and does not include a person name and title, do not invent a contact just to satisfy the prospect schema. Ask for a person row or return an insufficient-context decision until MDP has a provider-neutral account input contract.

## Runtime Normalization Prompt

New packs include `.mdp/prompts/normalize-prospect.yaml`. This prompt contract is meant for the upstream AI or workflow that sees messy source rows before the CLI runs. Its job is to return strict JSON with:

- `normalized_prospect`: the exact MDP prospect shape accepted by `mdp --json schema prospect`.
- `normalization_trace`: how persona, trigger, segment, signals, source, and missing fields were handled.
- `gaps`: missing or weak data that should not be invented.
- empty `card_patches`: runtime normalization does not edit pack cards.

That makes the boundary explicit:

```text
AI handles ambiguity:
  messy row -> normalized_prospect + trace

MDP CLI handles consistency:
  normalized_prospect -> fit -> route -> brief -> claim checks
```

Do not use normalization prompts to smooth over disqualifying language. If a row says "scrape contacts" or "auto-send a sequence", preserve that wording in the normalized context or trace so the fit gate can apply avoid-rules and disqualifiers.

## Pack Entries

The pack is modular. Each card holds entries with ids, bodies, applicability rules, evidence, avoid terms, and optional structured `constraints`.
Output-rule entries can also set `exact_paragraphs` when a fixed paragraph count should be checked deterministically. Entry `constraints` cover deterministic output limits such as body word count, subject word count, subject avoid literals, max questions, and forbidden links, attachments, images, HTML, or tracking. Min/max violations fail `check-claims`; target ranges are advisory warnings. Actual send metadata such as file attachments or tracking pixels cannot be proven from a single draft body, so those checks surface caveats in `unchecked_constraints`.

Do not confuse prospect `attributes` with entry `metadata`. Prospect `attributes` live on the input row and are bounded reviewed lead/account metadata that `mdp fit` can require through `manifest.yaml` `lead_input_requirements.required_attributes`. Entry `metadata` lives on card entries and describes the pack content itself for agent or human inspection.

Entries may include `metadata` for advisory custom annotations:

```yaml
entries:
  - id: partner-intro
    title: Partner intro
    body: Use for partner-referred introductions.
    applies_to: [PMM]
    evidence: [partner-notes.md]
    metadata:
      owner: partnerships
      review_status: draft
```

The CLI preserves entry `metadata` in route and brief context so agents can inspect it, but metadata keys are not enforceable constraints. Unknown arbitrary fields outside the schema are unsupported; `mdp validate` warns that those fields are ignored. Use prospect `attributes` for reviewed row metadata, entry `metadata` for advisory card annotations, and first-class fields or cards for rules the CLI should route or check.

Custom channels are declared in `manifest.yaml` `supported_channels`. Channel-policy routing uses those strings as tokenized channel names, so a pack can add `partner-intro`, `webinar-followup`, or another local channel without changing the CLI enum set.

```text
.mdp/manifest.yaml
  |
  +-- cards/fit-rules.yaml
  +-- cards/personas.yaml
  +-- cards/signals.yaml
  +-- cards/pains.yaml
  +-- cards/hooks.yaml
  +-- cards/claims.yaml
  +-- cards/ctas.yaml
  +-- cards/channel-policies.yaml
  +-- cards/avoid-rules.yaml
  +-- cards/output-rules.yaml
  +-- cards/gaps.yaml
```

The manifest is the index. Agents should load it first, then load only routed cards or routed entries. They should not read every card by default.

## Fit Gate

The fit gate runs before drafting. It answers whether the row has enough context and whether any disqualifier applies.

```text
row fields
  |
  +-- required prospect fields present?
  +-- supplied company_domain valid?
  +-- required signal fields present?
  +-- required attributes present?
  +-- disqualifier terms present?
  |
  v
fit | insufficient-context | disqualified
```

`mdp fit` is the fit evaluator. Skills should not create a parallel row-evaluation path; they should normalize row-like input, call `mdp fit`, and report that decision. A normalization prompt can prepare the row, but it should not replace the deterministic fit gate.

For the starter example row, `mdp fit` returns `fit` because the row has an explicit persona, segment, trigger, sourced signals, and a matching fit entry:

```text
status: fit
match: good-fit-agent-gtm
decision: Proceed to route/brief with stated assumptions.
```

If the row is thin, fit should block drafting:

```json
{
  "name": "Taylor Lee",
  "title": "GTM Engineering Lead",
  "company": "ExampleCo"
}
```

Expected decision:

```text
status: insufficient-context
missing: company_domain, trigger, persona, segment, signals, signals.source
decision: Ask for more context before drafting.
```

If the row or request is only about blasting or sequencing without decision context, fit should also block drafting:

```json
{
  "name": "Sam Patel",
  "title": "Revenue Operations",
  "company": "ExampleCo",
  "persona": "PMM",
  "segment": "sending-only workflow",
  "trigger": "sequence everyone this week",
  "signals": [
    {
      "id": "sending-request",
      "title": "Wants to blast a list",
      "source": "user-provided row"
    }
  ]
}
```

Expected decision:

```text
status: disqualified
disqualifier: sequence everyone
decision: Do not draft outbound copy unless the user overrides the disqualifier.
```

## Routing After Fit

When fit passes, the route narrows the pack to the entries needed for the task.

For `persona = PMM` and `job = linkedin outbound copy`, the starter pack routes these cards:

```text
.mdp/cards/personas.yaml
.mdp/cards/avoid-rules.yaml
.mdp/cards/output-rules.yaml
.mdp/cards/fit-rules.yaml
.mdp/cards/positioning.yaml
.mdp/cards/pains.yaml
.mdp/cards/signals.yaml
.mdp/cards/hooks.yaml
.mdp/cards/claims.yaml
.mdp/cards/copy-patterns.yaml
.mdp/cards/ctas.yaml
.mdp/cards/channel-policies.yaml
.mdp/cards/objections.yaml
```

With `mdp route --entries`, the entry route further narrows those cards. Example matched entries include:

```text
personas:pmm
fit-rules:good-fit-agent-gtm
fit-rules:no-context-no-copy
pains:agent-context-drift
pains:handoff-friction
pains:claim-inconsistency
hooks:manifest-not-monolith
hooks:evidence-before-action
hooks:one-context-many-agents
claims:modular-pack-routing
claims:versionable-context
ctas:soft-ask
ctas:calendar-second
ctas:no-false-urgency
ctas:reply-path
channel-policies:linkedin-initial-touch
avoid-rules:not-execution
avoid-rules:no-unsourced-claims
output-rules:no-em-dashes
output-rules:plain-text-by-default
output-rules:no-fake-personalization
```

That selected set is the bounded context for the drafting step. Current CLI contracts expose this as route output, `entry_route`, and brief `required_load_order`. A future bounded-context command or flag can package the same concept more tightly without changing the model: the drafting agent should receive selected context, not the whole pack.

Channel rules should stay split by responsibility. `channel-policies.yaml` owns channel and lifecycle policy, including `email-initial-touch`, `email-follow-up`, `linkedin-initial-touch`, `linkedin-follow-up`, `call-prep`, and `agent-brief`. `output-rules.yaml` owns formatting and generated-text constraints, including plain text by default, no links/HTML/tracking by default, initial email 90-125 word guidance, subject guidance, no fake personalization, and no meta commentary. `ctas.yaml` owns ask boundaries and reply paths, including soft asks and calendar-second policy. `copy-patterns.yaml` owns reusable structures such as trigger or hypothesis -> proof gap -> approved angle -> one soft CTA.

## Drafting Boundary

A brief is not a sender. It is a contract for the next drafting step.

```text
fit: fit
draft_status: ready
required_load_order:
  - personas
  - avoid-rules
  - output-rules
  - fit-rules
  - positioning
  - signals
  - hooks
  - claims
  - ctas
  - channel-policies

agent instruction:
  Draft only from the prospect row and routed context.
  Use approved claims only.
  Use the routed CTA/channel policy.
  Preserve weak signals as assumptions or hypotheses.
  Surface gaps instead of inventing proof.
```

If `draft_status` is `no-draft`, the agent should stop. It can summarize the fit decision and missing context, but it should not produce polished outbound copy unless the user explicitly overrides the gate.

## Why This Matters

Without MDP, an agent tends to turn all available context into one large prompt:

```text
all product notes + all prospect notes + all prior copy + generic instructions
  |
  v
draft
```

MDP makes the decision path explicit:

```text
source row + versioned pack
  |
  v
fit gate
  |
  v
selected persona, pains, hooks, proof, CTA, channel policy, avoid-rules, output-rules
  |
  v
bounded drafting context
```

The result is smaller context, clearer provenance, fewer unsupported claims, and a durable place to review how GTM messaging decisions are made.
