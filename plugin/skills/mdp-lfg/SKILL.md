---
name: mdp-lfg
description: "Use as the master orchestrator for fuzzy or multi-step Message Decision Pack work: create, use, review, route, brief, evaluate, or improve a pack."
---

# MDP LFG

Run the Message Decision Pack workflow end to end. Use this as the entry point for fuzzy or multi-step requests, then route to narrower MDP skills when the task becomes specific.

## First Move

1. Check the installed CLI and local pack state:

```bash
command -v mdp
mdp --json doctor --dir .
```

2. Before initializing, state the exact destination directory. If the user did not specify one, prefer the current repo/workspace root or an ignored scratch path; do not silently create a pack in `$HOME` or an unrelated code folder.

3. If `.mdp/manifest.yaml` is missing and the user wants a pack, initialize:

```bash
mdp --json init --template gtm --name "<pack name>" --dir .
```

4. If a pack exists, validate before changing it:

```bash
mdp --json validate --dir .
mdp --json explain --dir .
```

## Route The Work

Choose the narrow path and use that skill's workflow:

- New or messy pack: `$mdp-create-pack`
- ICP, segments, personas, fit/disqualifiers, signals, and no-message logic: `$mdp-icp-builder`
- Website, docs, notes, sales calls into card entries: `$mdp-source-extract`
- Hooks, objections, message angles, copy structures: `$mdp-message-angles`
- CTA, ask style, reply path, meeting boundary: `$mdp-cta-builder`
- Forbidden claims, category boundaries, unsupported promises: `$mdp-avoid-rules`
- Global style, punctuation, formatting, and structure constraints: `$mdp-output-rules`
- Existing provider-neutral prospect/source row to local MDP prospect JSON, fit decision, and brief: `$mdp-prospect-brief`
- Model-ready writing contract: `$mdp-copy-brief`
- Generated copy QA, claim checks, or revision: `$mdp-copy-eval`
- Full pack audit, routing QA, completeness check: `$mdp-pack-review`
- Scenario/eval testing of a pack: `$mdp-pack-eval`

Stay in the current thread unless the user explicitly asks to split work.

## Operating Loop

For most requests, run this loop:

1. Establish the current objective in one sentence.
2. Run `mdp --json doctor --dir .` and `mdp --json validate --dir .` when a pack exists.
3. Capture source facts in `.mdp/sources.yaml` before bulk card writing. Keep direct source claims separate from interpretation, and put missing proof in `gaps.yaml`.
4. Identify which card files matter; do not load the entire pack unless reviewing the whole pack.
5. Make the smallest useful pack edits. For new packs, fill cards in slices: positioning/fit/claims/gaps first, then personas/signals/pains, then motions/hooks/ctas/output-rules/copy-patterns/evals.
6. Validate again.
7. Test one representative route:

```bash
mdp --json --summary route --entries --eval-fixture --dir . --persona "<persona>" --job "<channel> outbound copy"
```

8. If a prospect/source row is involved, normalize it through `$mdp-prospect-brief`, using `.mdp/prompts/normalize-prospect.yaml` when the pack provides it. Then run `mdp fit`, and produce the brief only when fit allows. Use `--out` when the user expects a durable artifact; otherwise say the brief was emitted to stdout only:

```bash
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel <channel> --out .mdp/briefs/<brief-name>.json
```

## Required Card Coverage

A usable GTM messaging pack should usually have:

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
- `.mdp/cards/ctas.yaml`
- `.mdp/cards/avoid-rules.yaml`
- `.mdp/cards/output-rules.yaml`
- `.mdp/cards/copy-patterns.yaml`
- `.mdp/cards/objections.yaml`
- `.mdp/cards/gaps.yaml`
- `.mdp/evals/*.yaml`
- `.mdp/prompts/normalize-prospect.yaml`

Treat `ctas.yaml` as the policy for the ask or reply path. Treat `output-rules.yaml` as the global style and structure policy. Treat `claims.yaml` as the approved proof ledger, `fit-rules.yaml` as the no-message gate, and `gaps.yaml` as the place to preserve known unknowns. Do not bury these rules only inside copy examples.

## Boundaries

- Do not send messages, update Clay, update CRM, enroll sequences, enrich leads, or scrape private data. MDP can produce an explicit handoff; execution happens outside MDP.
- Do not call MDP a sender, CRM, sequencer, enrichment provider, AI SDR, BI tool, or generic automation system.
- Do not invent unsupported claims. Put gaps in the brief or card entries.
- Keep `--json` on for CLI output that another tool, script, or agent will parse.
- Use `--summary` for status checks instead of piping JSON into ad hoc scripts.
- Starter `examples/clay-row.json` rows are synthetic fixtures kept for compatibility unless the prospect says otherwise. Do not present them as real prospects, and do not treat Clay as the required or default source system.
- Do not add a parallel row-evaluation skill for fit. Use `$mdp-prospect-brief` and the CLI-owned `mdp fit` decision instead.

## Closeout

End with:

- files changed or reviewed
- validation result
- route or brief command tested, including whether output was saved with `--out` or only printed to stdout
- `mdp --json gaps --dir .` result
- `mdp --json check-claims --dir . --text "<sample draft or risky claim>"` result when copy or claims changed
- remaining evidence gaps
- the next best MDP command
