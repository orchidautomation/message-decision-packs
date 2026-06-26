---
name: mdp
description: Use when the user explicitly wants to create, validate, inspect, route, or use a Message Decision Pack, `.mdp/` pack, MDP CLI, MDP skills, or MDP brief. Prefer `mdp` CLI before reading `.mdp/` YAML.
---

# MDP

For fuzzy or multi-step MDP work, use `$mdp-lfg` first, then route to the narrower skill or CLI command.

Use Message Decision Packs as the source of messaging decisions, not as the execution system. The pack stores ICP, fit rules, personas, pains, signals, positioning, claims, hooks, channel policies, avoid rules, output rules, CTA rules, objections, gaps, copy patterns, and prompt contracts. Prompt contracts normalize messy upstream rows or propose reviewed card entries; the `mdp` CLI validates, routes, checks fit, checks claims and output guardrails, lists gaps, and runs eval fixtures. Draft only after the CLI returns the relevant cards and fit is acceptable.

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
- `.mdp/cards/output-rules.yaml`
- `.mdp/cards/copy-patterns.yaml`
- `.mdp/cards/ctas.yaml`
- `.mdp/cards/objections.yaml`
- `.mdp/cards/gaps.yaml`
- `.mdp/evals/*.yaml`
- `.mdp/prompts/normalize-prospect.yaml`

After edits:

```bash
mdp --json validate --dir .
```

## Use A Prospect Or Source Row

Convert an existing prospect/source row, CSV row, CRM export row, research note, Clay/Deepline row, spreadsheet row, or user-provided source row into a small JSON file under a repo-ignored agent artifacts directory or another ignored scratch path unless the user explicitly wants to commit a sanitized example. Prefer the pack-owned `.mdp/prompts/normalize-prospect.yaml` contract for messy rows; save its `normalized_prospect` output as the file that feeds `mdp fit`. Do not commit private prospect data. Check the expected shape:

```bash
mdp --json schema prospect
```

Minimum fields: `name`, `title`, `company`. Prefer adding `linkedin_url`, `company_url`, `background`, `trigger`, `persona`, `segment`, structured `signals`, `source_kind`, and `synthetic` when relevant.

Use provider-neutral `source_kind` values unless the source itself matters: `user-provided-row`, `csv-row`, `crm-export-row`, `clay-row`, `deepline-row`, `private-scratch-row`, `sanitized-example`, or `synthetic-example`. Clay is one possible source, not the default MDP mental model.

Normalization prompts may map messy titles into pack-owned personas and signals, but they must preserve raw evidence, uncertainty, missing fields, and disqualifying execution asks. When using any `.mdp/prompts/*.yaml` prompt contract, treat `output_contract.schema_ref` as the response contract; if the prompt includes `output_contract.schema`, give that literal schema to the model or host. `output_contract.example` is only a reference. The CLI still owns final fit and route decisions.

If the input is account-only and lacks a person name and title, do not invent a contact. Ask for the missing person fields or return the fit gate's insufficient-context decision.

If `persona` is missing, the CLI can resolve it from pack-owned `.mdp/manifest.yaml` `persona_mappings.title_keywords`. Treat `persona_resolution.source: builtin.title_keywords` or `fallback` as review-needed; those weak fallbacks do not make a prospect fit-ready by themselves.

Generated starter rows include `source_kind: synthetic-example` and `synthetic: true`. Treat those as demo fixtures, not real prospects. For production work, use a real row in ignored scratch or an intentionally sanitized example.

Run fit first and stop on `disqualified` or `insufficient-context` unless the user explicitly overrides. If the user only asked whether a row should be messaged, return the `mdp fit` decision, matched rules, disqualifiers, and gaps instead of drafting or creating a parallel evaluation.

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

## Agent Framework Wrappers

Frameworks such as Flue or Vercel Eve may wrap MDP for webhook admission, durable runs, filesystem staging, model drafting, and artifact collection. Keep that layer as an adapter around the CLI:

1. Verify and normalize the inbound event in trusted application code.
2. Use `.mdp/prompts/normalize-prospect.yaml` when an upstream AI normalizer is needed; preserve its `normalization_trace`.
3. Write the raw payload and normalized prospect JSON to ignored scratch.
4. Run `mdp --json fit` before drafting.
5. Run `mdp --json brief --context` and draft only from the returned brief/context.
6. Run `mdp --json check-claims` before treating draft text as ready.

Do not move fit logic, route selection, claim checks, or card interpretation into the framework layer. Do not let the framework wrapper send, schedule, enrich, scrape, update a CRM, or write to a sequencer unless the user explicitly asks for that separate system action outside MDP.

## Boundaries

- Do not send LinkedIn messages or emails.
- MDP stops at pack, route, fit, claims, gaps, evals, and brief contracts.
- Sending, scheduling, enriching, CRM updates, Clay/Deepline writes, or sequencer work requires a separate exact-action handoff/tool outside MDP and explicit user approval.
- Do not call MDP an AI SDR, CRM, sequencer, enrichment provider, BI tool, or generic automation system.
- Do not invent missing claims. Surface gaps in the brief.
- Keep `--json` on when another agent, script, or tool will parse the output.
