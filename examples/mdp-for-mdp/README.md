# MDP for MDP

This is a public, self-referential Message Decision Pack: an MDP that explains MDP.

Use it when you want to show a GTM engineer, PMM, RevOps person, founder, or agency operator what a pack stores and why it is better than pasting a giant prompt into every agent thread.

The point is not that MDP writes better copy by itself. The point is that MDP stores the decision context an agent should use before drafting:

- who the message is for
- what makes the row a fit
- when the system should refuse to draft
- which claims are approved
- which claims and categories are forbidden
- which CTA and channel rules apply
- what evidence is missing
- which cards are routed into a bounded brief
- which evals prove the behavior still works

MDP is a decision/context layer. It is not a sender, CRM, sequencer, enrichment provider, scraper, AI SDR, BI tool, or generic automation system.

## What To Inspect

```text
examples/mdp-for-mdp/
  .mdp/
    manifest.yaml
    sources.yaml
    cards/
      personas.yaml
      positioning.yaml
      fit-rules.yaml
      signals.yaml
      pains.yaml
      claims.yaml
      motions.yaml
      channel-policies.yaml
      hooks.yaml
      ctas.yaml
      avoid-rules.yaml
      output-rules.yaml
      copy-patterns.yaml
      objections.yaml
      gaps.yaml
    prompts/
      normalize-prospect.yaml
    evals/
      *.yaml
  examples/
    messy-source-row.json
    clay-row.json
    thin-source-row.json
    thin-prospect.json
    execution-ask-row.json
    execution-ask-prospect.json
```

The rows are synthetic. The raw source rows include `do_not_contact: true`; the normalized prospect rows use the public MDP prospect schema with `source_kind: synthetic-example` and `synthetic: true`.

## The Demo Story

Start with a messy supplied row:

```bash
cat examples/mdp-for-mdp/examples/messy-source-row.json
```

It contains a fictional GTM engineer whose team keeps pasting one large prompt with ICP, claims, CTAs, avoid rules, and channel instructions into different agents.

The pack-owned normalization prompt, `.mdp/prompts/normalize-prospect.yaml`, shows how an upstream agent should turn that messy row into provider-neutral prospect JSON while preserving uncertainty and source traces. The normalized fixture is committed here:

```bash
cat examples/mdp-for-mdp/examples/clay-row.json
```

That normalized prospect is what the CLI uses for fit and brief commands.

## 1. Validate The Pack

```bash
mdp --json validate --dir examples/mdp-for-mdp
```

Expected outcome: `valid: true`, with 15 card files and 10 prompt files loaded.

## 2. Route Bounded Context

```bash
mdp --json --summary route \
  --entries \
  --eval-fixture \
  --dir examples/mdp-for-mdp \
  --persona "GTM Engineer" \
  --job "linkedin outbound copy"
```

Expected outcome: the route loads the base guardrails first, then the MDP-specific cards needed for the job:

- `personas`
- `avoid-rules`
- `output-rules`
- `fit-rules`
- `positioning`
- `pains`
- `signals`
- `hooks`
- `claims`
- `copy-patterns`
- `ctas`
- `channel-policies`
- `gaps`

This is the progressive-disclosure contract. The agent does not need to paste the whole pack into a giant prompt.

## 3. Run Fit Before Drafting

Fit-ready synthetic row:

```bash
mdp --json fit \
  --dir examples/mdp-for-mdp \
  --prospect examples/mdp-for-mdp/examples/clay-row.json
```

Expected outcome: `status: fit`, with matches such as:

- `Good fit: agent-assisted GTM workflow`
- `Good fit: source row normalization`

Thin synthetic row:

```bash
mdp --json fit \
  --dir examples/mdp-for-mdp \
  --prospect examples/mdp-for-mdp/examples/thin-prospect.json
```

Expected outcome: `status: insufficient-context`. Missing fields include trigger, persona, segment, signals, and source. The correct behavior is no draft.

Execution-boundary row:

```bash
mdp --json fit \
  --dir examples/mdp-for-mdp \
  --prospect examples/mdp-for-mdp/examples/execution-ask-prospect.json
```

Expected outcome: `status: disqualified`, with disqualifiers for requests such as scrape, enrich leads, sequence prospects, auto-send, and update CRM. MDP should hand off the boundary, not pretend to execute.

## 4. Build A Brief With Context

Ready brief:

```bash
mdp --json --summary brief \
  --context \
  --dir examples/mdp-for-mdp \
  --prospect examples/mdp-for-mdp/examples/clay-row.json \
  --channel linkedin
```

Expected outcome: `draft_status: ready`, `fit_status: fit`, and bounded `context.entries` from the routed cards.

No-draft brief:

```bash
mdp --json --summary brief \
  --context \
  --dir examples/mdp-for-mdp \
  --prospect examples/mdp-for-mdp/examples/thin-prospect.json \
  --channel linkedin
```

Expected outcome: `draft_status: no-draft`. The brief tells the agent to stop, surface missing context, and ask for an explicit override before creating outbound copy.

## 5. Surface Durable Gaps

```bash
mdp --json gaps --dir examples/mdp-for-mdp
```

Expected outcome: durable gaps for missing customer proof, synthetic rows, missing integration proof, and the production policy a real team should add before using a pack for production messaging.

## 6. Check Claims And Guardrails

Approved claim:

```bash
mdp --json check-claims \
  --dir examples/mdp-for-mdp \
  --text "MDP decision context is a local/offline standard, CLI, and plugin for modular GTM messaging decision context. It helps teams keep fit rules, approved claims, CTA policy, avoid rules, output rules, source gaps, and brief routing in reviewable files before a downstream agent drafts copy. That makes the handoff more testable than one giant prompt."
```

Expected outcome: `valid: true`.

Unsupported claim:

```bash
mdp --json check-claims \
  --dir examples/mdp-for-mdp \
  --persona "Founder" \
  --job "initial email outbound message" \
  --subject "Guaranteed meetings" \
  --text "MDP guarantees 40% reply rates, creates pipeline, sends emails, and updates CRM records automatically."
```

Expected outcome: `valid: false`, with unsupported quantified outcome and execution-claim hits.

Output-rule violation:

```bash
mdp --json check-claims \
  --dir examples/mdp-for-mdp \
  --persona "PMM" \
  --job "initial email outbound message" \
  --subject "Re: urgent quick question" \
  --text "Read the MDP docs at https://example.com and book a meeting?"
```

Expected outcome: `valid: false`, with link, subject, and word-count guardrail hits.

## 7. Run The Pack Evals

```bash
mdp --json eval --dir examples/mdp-for-mdp
```

Expected outcome: `valid: true` with 10 fixtures asserting the main route, fit, brief, claim-check, and output-rule outcomes:

- GTM Engineer LinkedIn route
- PMM initial email route
- fit-ready prospect
- insufficient-context prospect
- disqualified execution ask
- ready brief
- no-draft brief
- approved claim check
- unsupported claim check
- output-rule claim check

The detailed missing fields, disqualifier terms, routed context counts, and guardrail hit details are demonstrated by the individual commands above.

## Why This Beats A Giant Prompt

A giant prompt makes the agent re-read everything and decide what matters at runtime. It is hard to review, hard to test, and easy to drift across channels or agents.

This pack makes the decisions inspectable:

- `sources.yaml` separates facts, interpretations, and gaps.
- `manifest.yaml` defines personas, channels, cards, and routing policy.
- `fit-rules.yaml` owns the stop/go decision before drafting.
- `claims.yaml` owns what can be said.
- `avoid-rules.yaml` owns category and execution boundaries.
- `ctas.yaml` and `channel-policies.yaml` own the ask and channel behavior.
- `output-rules.yaml` owns deterministic format checks.
- `gaps.yaml` preserves missing proof.
- `evals/` turns the intended behavior into tests.

That is the product value: MDP keeps GTM judgment durable, local, routed, and testable before any downstream model or execution system gets involved.
