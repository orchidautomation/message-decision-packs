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

2. If `.mdp/manifest.yaml` is missing and the user wants a pack, initialize:

```bash
mdp --json init --template gtm --name "<pack name>" --dir .
```

3. If a pack exists, validate before changing it:

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
- Existing prospect row to local MDP prospect JSON and brief: `$mdp-prospect-brief`
- Model-ready writing contract: `$mdp-copy-brief`
- Generated copy QA, claim checks, or revision: `$mdp-copy-eval`
- Full pack audit, routing QA, completeness check: `$mdp-pack-review`
- Scenario/eval testing of a pack: `$mdp-pack-eval`

Stay in the current thread unless the user explicitly asks to split work.

## Operating Loop

For most requests, run this loop:

1. Establish the current objective in one sentence.
2. Run `mdp --json doctor --dir .` and `mdp --json validate --dir .` when a pack exists.
3. Identify which card files matter; do not load the entire pack unless reviewing the whole pack.
4. Make the smallest useful pack edits.
5. Validate again.
6. Test one representative route:

```bash
mdp --json route --entries --dir . --persona "<persona>" --job "<channel> outbound copy"
```

7. If a prospect row is involved, produce the brief:

```bash
mdp --json brief --dir . --prospect <prospect.json> --channel <channel>
```

## Required Card Coverage

A usable GTM messaging pack should usually have:

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
- `.mdp/cards/ctas.yaml`
- `.mdp/cards/avoid-rules.yaml`
- `.mdp/cards/copy-patterns.yaml`
- `.mdp/cards/objections.yaml`
- `.mdp/cards/gaps.yaml`
- `.mdp/evals/*.yaml`

Treat `ctas.yaml` as the policy for the ask or reply path. Treat `claims.yaml` as the approved proof ledger, `fit-rules.yaml` as the no-message gate, and `gaps.yaml` as the place to preserve known unknowns. Do not bury these rules only inside copy examples.

## Boundaries

- Do not send messages, update Clay, update CRM, enroll sequences, enrich leads, or scrape private data. MDP can produce an explicit handoff; execution happens outside MDP.
- Do not call MDP a sender, CRM, sequencer, enrichment provider, AI SDR, BI tool, or generic automation system.
- Do not invent unsupported claims. Put gaps in the brief or card entries.
- Keep `--json` on for CLI output that another tool, script, or agent will parse.

## Closeout

End with:

- files changed or reviewed
- validation result
- route or brief command tested
- remaining evidence gaps
- the next best MDP command
