---
name: mdp
description: Use when the user wants to create, brainstorm, validate, inspect, route, or use a Message Decision Pack for GTM messaging, ICP, pains, hooks, avoid-rules, CTA rules, copy patterns, Clay or Deepline enriched rows, LinkedIn/email outbound, or agent-readable message briefs. Prefer the installed `mdp` CLI for validation and routing before manually reading `.mdp/` YAML files.
---

# MDP

For fuzzy or multi-step MDP work, use `$mdp-lfg` first, then route to the narrower skill or CLI command.

Use Message Decision Packs as the source of messaging decisions, not as the execution system. The pack stores ICP, fit rules, personas, pains, signals, positioning, claims, hooks, channel policies, avoid rules, CTA rules, objections, gaps, and copy patterns. The `mdp` CLI validates, routes, checks fit, checks claims, lists gaps, and runs eval fixtures. The agent drafts only after the CLI returns the relevant cards.

## First Check

From the workspace that contains or should contain a pack:

```bash
command -v mdp
mdp --json doctor --dir .
```

If `mdp` is missing, say the CLI is not installed and ask whether to install or locate it. Do not fake validation by reading YAML manually.

## Create Or Improve A Pack

For a new generic pack:

```bash
mdp --json init --name "Message Pack" --dir .
```

For a neutral demo:

```bash
mdp --json init --template gtm --name "Example Message Pack" --dir .
```

When brainstorming the pack, help fill these files:

- `.mdp/manifest.yaml`
- `.mdp/cards/personas.yaml`
- `.mdp/cards/positioning.yaml`
- `.mdp/cards/fit-rules.yaml`
- `.mdp/cards/signals.yaml`
- `.mdp/cards/pains.yaml`
- `.mdp/cards/claims.yaml`
- `.mdp/cards/motions.yaml`
- `.mdp/cards/channel-policies.yaml`
- `.mdp/cards/hooks.yaml`
- `.mdp/cards/avoid-rules.yaml`
- `.mdp/cards/copy-patterns.yaml`
- `.mdp/cards/ctas.yaml`
- `.mdp/cards/objections.yaml`
- `.mdp/cards/gaps.yaml`
- `.mdp/evals/*.yaml`

After edits:

```bash
mdp --json validate --dir .
```

## Use A Prospect Row

Convert Clay, Deepline, CSV, LinkedIn research, or enrichment output into a small JSON file. Check the expected shape:

```bash
mdp --json schema prospect
```

Minimum fields: `name`, `title`, `company`. Prefer adding `linkedin_url`, `company_url`, `background`, `trigger`, `persona`, `segment`, and structured `signals`.

Then create a brief:

```bash
mdp --json brief --dir . --prospect examples/clay-row.json --channel linkedin
```

Read only `data.required_load_order`. Then draft using the brief plus those loaded card files.

## Route Without A Prospect

```bash
mdp --json route --entries --dir . --persona "VP Finance" --job "linkedin outbound copy"
mdp --json emit-brief --dir . --persona "VP Finance" --job "linkedin outbound copy"
```

Use `load_order` or `required_load_order` as the progressive-disclosure contract.


Before drafting from a prospect row, check fit:

```bash
mdp --json fit --dir . --prospect examples/clay-row.json
```

Before approving generated copy, check claims and guardrails:

```bash
mdp --json check-claims --dir . --text "<draft copy>"
```

For pack QA:

```bash
mdp --json gaps --dir .
mdp --json eval --dir .
```

## Demo Copy

For local demos only:

```bash
mdp --json copy --dir . --prospect examples/clay-row.json --channel linkedin
```

For production-quality output, use `brief` and draft from the returned contract and routed cards.

## Boundaries

- Do not send LinkedIn messages or emails.
- Do not update CRM, Clay, Deepline, sequencers, or enrichment systems unless the user explicitly asks and a separate tool is available.
- Do not call MDP an AI SDR, CRM, sequencer, enrichment provider, BI tool, or generic automation system.
- Do not invent missing claims. Surface gaps in the brief.
- Keep `--json` on when another agent, script, or tool will parse the output.
