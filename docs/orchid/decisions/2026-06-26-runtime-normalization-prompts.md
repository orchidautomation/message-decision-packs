# Runtime Normalization Prompts For MDP

Date: 2026-06-26

## Decision

Treat runtime prospect normalization prompts as first-class MDP prompt contracts.

An MDP should not only store cards for downstream drafting and review. It should also carry version-controlled prompt contracts that upstream agents can use to turn messy source rows into provider-neutral prospect JSON before the CLI runs.

## Operating Model

The split is:

```text
AI handles ambiguity:
  messy row -> normalized_prospect + normalization_trace

MDP CLI handles consistency:
  normalized_prospect -> fit -> route -> brief -> claim checks
```

The prompt contract lives under:

```text
.mdp/prompts/normalize-prospect.yaml
```

It outputs `normalized_prospect` in the same shape accepted by `mdp --json schema prospect`, plus `normalization_trace`, `gaps`, empty `card_patches`, and `rejected_claims`.

## Boundary

The normalization prompt may:

- map messy titles into pack-owned personas;
- preserve source fields, signal confidence, and hypotheses;
- expose missing context and fit-readiness;
- preserve disqualifying source language such as scraping, auto-send, sequence, enrichment, or CRM update asks.

The normalization prompt must not:

- browse, scrape, enrich, send, sequence, or update external systems;
- silently decide final fit;
- smooth away disqualifiers;
- invent contacts for account-only input;
- rewrite pack cards at runtime.

The deterministic CLI remains the source of truth for fit, route, brief, and claim-check outputs.

## Consequence

This makes MDP a stronger contract between messy GTM systems and reliable agent workflows:

```text
Prompt contracts in.
Decision contracts out.
```

Companies can version the prompts that feed their systems and the decision rules that route the resulting data. Agents can use the prompt contracts for normalization while still relying on the CLI for auditable, repeatable decisions.
