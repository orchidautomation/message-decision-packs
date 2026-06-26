# Conceptual Decision Flow

Message Decision Packs (MDP) are a local decision/context layer for GTM messaging. An MDP pack stores the decisions an agent needs before writing or evaluating a message: fit rules, personas, pains, hooks, approved claims, CTA and channel policy, avoid-rules, output-rules, evidence, and gaps.

MDP does not send messages, enrich leads, update CRM, scrape the web, run sequences, or act as an AI SDR. It decides what context is allowed into the drafting task and what should block the task.

## Mental Model

A provider-neutral prospect/source row supplies row-level inputs. The row can come from a user note, CSV, CRM export, Clay, Deepline, spreadsheet, or research workflow after it has been normalized into MDP prospect JSON. The pack supplies modular decision entries. The CLI and skills route the row through a decision tree and return only the context needed for the job.

```text
prospect JSON
  |
  +-- title/persona
  +-- segment
  +-- trigger
  +-- signals
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

The prospect schema requires only `name`, `title`, and `company`, but good routing needs more context:

```json
{
  "name": "Alex Rivera",
  "title": "GTM Engineering Lead",
  "company": "ExampleCo",
  "persona": "GTM Engineering",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context across agents and systems",
  "background": "building repeatable agent-assisted GTM workflows across Clay, Codex, and Claude Code",
  "source_kind": "synthetic-example",
  "synthetic": true,
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

The row should preserve source and confidence when available. Weak signals should stay hypotheses. A LinkedIn URL, CSV row, CRM export, Clay row, Deepline row, spreadsheet row, or user-provided note can inform routing, but it should not be treated as proof of a product need by itself.

If the input is account-only and does not include a person name and title, do not invent a contact just to satisfy the prospect schema. Ask for a person row or return an insufficient-context decision until MDP has a provider-neutral account input contract.

## Pack Entries

The pack is modular. Each card holds entries with ids, bodies, applicability rules, evidence, and avoid terms.
Output-rule entries can also set `exact_paragraphs` when a fixed paragraph count should be checked deterministically.

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
  +-- trigger present?
  +-- persona present in the row?
  +-- segment present?
  +-- signals present?
  +-- source present?
  +-- disqualifier terms present?
  |
  v
fit | insufficient-context | disqualified
```

`mdp fit` is the fit evaluator. Skills should not create a parallel row-evaluation path; they should normalize row-like input, call `mdp fit`, and report that decision.

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
missing: trigger, persona, segment, signals, source
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
ctas:no-false-urgency
ctas:reply-path
channel-policies:linkedin-initial-touch
avoid-rules:not-execution
avoid-rules:no-unsourced-claims
output-rules:no-em-dashes
```

That selected set is the bounded context for the drafting step. Current CLI contracts expose this as route output, `entry_route`, and brief `required_load_order`. A future bounded-context command or flag can package the same concept more tightly without changing the model: the drafting agent should receive selected context, not the whole pack.

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
