---
name: mdp-lfg
description: "Use as the master orchestrator for fuzzy or multi-step Message Decision Pack work: create, use, review, route, brief, evaluate, or improve a pack."
---

# MDP LFG

## Progressive References

Load these files only when the task needs them:

- Read `references/mdp-mental-model.md` when the user is confused about what MDP is, what landed, or how deterministic MDP flows work.
- Read `references/prompt-output-validation.md` when the user asks how messy source blobs, prompt outputs, allowed enum values, and CLI validation connect.
- Read `references/template-qa-recipes.md` when testing a released install, default template, GTM pack, proposal pack, or fresh QA workspace.
- Use `assets/prompt-output-fixtures/` for public-safe prompt-output skeletons. Treat them as examples to adapt to the current pack, not universal accepted values.
- Use `evals/trigger-queries.json` and `evals/output-evals.json` when improving this skill's trigger or output quality.

## Profile Gate

Before using this skill against an existing pack, run:

```bash
mdp --json agent-surface --dir .
```

Use this skill only when the surface is legacy/generic or this skill is listed in `recommended_skills` or `allowed_skills` and is not listed in `blocked_skills`. If the surface blocks this skill, stop and reroute to an allowed or recommended skill named by the surface before editing or reviewing pack content.

Run the Message Decision Pack workflow end to end. Use this as the entry point for fuzzy or multi-step requests, then route to narrower MDP skills when the task becomes specific.

## First Move

If the operator is confused about normalization, fit, route, brief, attributes, or profiles, answer with the canonical flow first:

```text
messy row -> normalize -> validate prompt output -> fit/readiness -> route/brief -> draft/check-claims
```

Then apply the right narrow skill. Normalization does not mutate packs and runtime normalization keeps `card_patches` empty. A brief is not final copy; it is stdout-only unless `--out` is used, and it is draftable only when `draft_status` is `ready`.

1. Check the installed CLI and local pack state:

```bash
command -v mdp
mdp --json doctor --dir .
```

2. Before initializing, state the exact destination directory. If the user did not specify one, prefer the current repo/workspace root or an ignored scratch path; do not silently create a pack in `$HOME` or an unrelated code folder.

3. If `.mdp/manifest.yaml` is missing and the user wants a pack, initialize with exactly one closest template.

For generic GTM packs:

```bash
mdp --json init --template gtm --name "<pack name>" --dir .
```

For proposal, RFP, capture, or bid/no-bid review packs:

```bash
mdp --json init --template proposal --dir .
```

4. If a pack exists, validate before changing it:

```bash
mdp --json validate --dir .
mdp --json explain --dir .
```

## Route The Work

Choose the narrow path and use that skill's workflow:

- New or messy GTM pack: `$mdp-create-pack`
- Proposal, RFP, capture, bid/no-bid, compliance, proof, red-team, or executive-review pack: `$mdp-proposal-pack-builder`
- Bid/no-bid review for a supplied proposal pursuit: `$mdp-proposal-bid-no-bid-review`
- Compliance review for supplied proposal requirements, matrices, or answer drafts: `$mdp-proposal-compliance-review`
- Win-theme or proof review for supplied proposal themes, differentiators, or claim-bearing draft text: `$mdp-proposal-win-theme-proof-review`
- Red-team or gap review for supplied proposal sections, matrices, or answer drafts: `$mdp-proposal-red-team-gap-review`
- ICP, segments, personas, fit/disqualifiers, signals, and no-message logic: `$mdp-icp-builder`
- Website, docs, notes, sales calls into card entries: `$mdp-source-extract`
- Source/search/intake strategy before extraction, GTM scouting, or proposal corpus review: `$mdp-source-strategy`
- Hooks, objections, message angles, copy structures: `$mdp-message-angles`
- CTA, ask style, reply path, meeting boundary: `$mdp-cta-builder`
- Forbidden claims, category boundaries, unsupported promises: `$mdp-avoid-rules`
- Global style, punctuation, formatting, and structure constraints: `$mdp-output-rules`
- Existing provider-neutral prospect/source row to local MDP prospect JSON, fit decision, and brief: `$mdp-prospect-brief`

If a GTM pack mentions a proposal-governance wedge, treat it as GTM positioning or outbound-fit context unless the user supplies actual proposal/RFP/review material. Do not route GTM positioning work into proposal compliance, bid/no-bid, or customer proposal handling skills.
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

8. If a prospect/source row is involved, normalize it through `$mdp-prospect-brief`, using `.mdp/prompts/normalize-prospect.yaml` when the pack provides it. Treat the prompt's `output_contract.schema_ref` as the response contract; if it includes `output_contract.schema`, give that literal schema to the model or host. Use the example as a reference, not the contract. Then run `mdp fit`, and produce the brief only when fit allows. Use `--out` when the user expects a durable artifact; otherwise say the brief was emitted to stdout only:

```bash
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel <channel> --out .mdp/briefs/<brief-name>.json
```

For human review, add a readable Markdown artifact from the same prospect/context path while keeping JSON as the machine source of truth:

```bash
mdp brief --context --readable --dir . --prospect <prospect.json> --channel <channel> --out .mdp/briefs/<brief-name>.md
```

This readable command is the GTM prospect review artifact. Do not reuse prospect/outreach labels for proposal packs. For proposal profile work, preserve the same human-review-layer principle but use opportunity/review frontmatter and proposal-owned sections such as bid/no-bid read, compliance gaps, requirement status, proof or win-theme receipts, unsupported claims, red-team gaps, and `verify-output` status. If that artifact is not implemented in the current CLI slice, file or use a proposal-profile follow-up instead of expanding GTM prospect brief work into proposal generation or proposal management.

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
- Use `signals` for evidence, prospect `attributes` for bounded reviewed row metadata, row fields such as `source_kind` for source markers, and entry `metadata` for card annotations. Do not put proof into attributes or prospect facts into entry metadata.
- Treat `lead_input_requirements` as the manifest wire key for input readiness policy and `qualification_gates` as the manifest wire key for source-backed fit gates enforced by `mdp fit`. Treat `activation_ready` as structural profile/template readiness only, not commercial readiness.
- Treat GTM, proposal, and future domains as profiles/templates over shared primitives. Keep profile nouns in card IDs, prompts, jobs, attributes, signals, traces, gaps, and evals unless the CLI core contract changes.
- Keep `--json` on for CLI output that another tool, script, or agent will parse.
- Use `--summary` for status checks instead of piping JSON into ad hoc scripts.
- Starter `examples/clay-row.json` rows are synthetic fixtures kept for compatibility unless the prospect says otherwise. Do not present them as real prospects, and do not treat Clay as the required or default source system.
- Do not add a parallel row-evaluation or qualification skill for fit. Use `$mdp-prospect-brief` and the CLI-owned `mdp fit` decision, including `context.qualification_gate`, instead.

## Closeout

End with:

- files changed or reviewed
- validation result
- route or brief command tested, including whether output was saved with `--out` or only printed to stdout
- `mdp --json gaps --dir .` result
- `mdp --json check-claims --dir . --text "<sample draft or risky claim>"` result when copy or claims changed
- remaining evidence gaps
- the next best MDP command
