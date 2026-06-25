---
name: mdp
description: Use when the user explicitly wants to create, validate, inspect, route, or use a Message Decision Pack, `.mdp/` pack, MDP CLI, MDP skills, or MDP brief. Prefer `mdp` CLI before reading `.mdp/` YAML.
---

# MDP

For fuzzy or multi-step MDP work, use `$mdp-lfg` first, then route to the narrower skill or CLI command.

Use Message Decision Packs as the source of messaging decisions, not as the execution system. The pack stores ICP, fit rules, personas, pains, signals, positioning, claims, hooks, channel policies, avoid rules, CTA rules, objections, gaps, and copy patterns. The `mdp` CLI validates, routes, checks fit, checks claims, lists gaps, and runs eval fixtures. Draft only after the CLI returns the relevant cards and fit is acceptable.

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
- `.mdp/sources.yaml`
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

Convert an existing prospect row, CSV row, research note, or user-provided source row into a small JSON file under a repo-ignored agent artifacts directory or another ignored scratch path unless the user explicitly wants to commit a sanitized example. Do not commit private prospect data. Check the expected shape:

```bash
mdp --json schema prospect
```

Minimum fields: `name`, `title`, `company`. Prefer adding `linkedin_url`, `company_url`, `background`, `trigger`, `persona`, `segment`, and structured `signals`.

Generated starter rows include `source_kind: synthetic-example` and `synthetic: true`. Treat those as demo fixtures, not real prospects. For production work, use a real row in ignored scratch or an intentionally sanitized example.

Run fit first and stop on `disqualified` or `insufficient-context` unless the user explicitly overrides.

Then create a brief:

```bash
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel <channel>
```

If the user expects a created artifact, save it explicitly:

```bash
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel <channel> --out .mdp/briefs/<brief-name>.json
```

Read `data.context.entries` first. Open `data.context.full_card_required` paths only when present. Draft only when `data.draft_status` is `ready`.

## Route Without A Prospect

```bash
mdp --json --summary route --entries --eval-fixture --dir . --persona "VP Finance" --job "linkedin outbound copy"
mdp --json emit-brief --dir . --persona "VP Finance" --job "linkedin outbound copy"
```

Use `load_order` or `required_load_order` as the progressive-disclosure contract.


Before drafting from a prospect row, check fit:

```bash
mdp --json fit --dir . --prospect <prospect.json>
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

Use `--summary` for compact status instead of piping JSON into one-off scripts.

## Demo Copy

For local demos only:

```bash
mdp --json copy --dir . --prospect <prospect.json> --channel <channel>
```

For production-quality output, use `brief` and draft from the returned contract and routed cards.

## Boundaries

- Do not send LinkedIn messages or emails.
- MDP stops at pack, route, fit, claims, gaps, evals, and brief contracts.
- Sending, scheduling, enriching, CRM updates, Clay/Deepline writes, or sequencer work requires a separate exact-action handoff/tool outside MDP and explicit user approval.
- Do not call MDP an AI SDR, CRM, sequencer, enrichment provider, BI tool, or generic automation system.
- Do not invent missing claims. Surface gaps in the brief.
- Keep `--json` on when another agent, script, or tool will parse the output.
