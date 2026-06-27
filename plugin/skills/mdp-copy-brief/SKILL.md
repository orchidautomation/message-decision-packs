---
name: mdp-copy-brief
description: Use to produce an MDP-routed writing contract from pack cards, fit status, persona, prospect row, channel, motion, or job before drafting.
---

# MDP Copy Brief

Create a writing brief from MDP routing. The brief should constrain a copywriter or model before copy is drafted.

## Workflow

1. If a prospect/source row exists, use the normalized MDP prospect JSON and run fit first:

```bash
mdp --json fit --dir . --prospect <prospect.json>
```

Hard-stop on `disqualified` or `insufficient-context` unless the user explicitly overrides.

Do not redo row normalization or fit evaluation in this skill. `$mdp-prospect-brief` owns row normalization, and `mdp fit` owns the decision.

2. With a prospect/source row, build the brief:

```bash
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel <channel>
```

If the user expects a created file, save it explicitly:

```bash
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel <channel> --out .mdp/briefs/<brief-name>.json
```

3. Without a prospect/source row, do not draft lead-specific copy from a persona/job route alone. If the user needs outbound-copy testing, generate clearly fake fixture leads first:

```bash
mdp sample-leads --dir . --persona "<persona>" --job "<channel> outbound copy" --count 3 --format yaml
```

Then save one fixture row to ignored scratch if needed, run `mdp fit`, build `mdp brief --context`, and draft only against `safe_personalization` and `known_gaps`. Never treat fixture leads as real prospects.

If the user only needs a route-level writing contract, build a persona/job brief:

```bash
mdp --json --summary emit-brief --dir . --persona "<persona>" --job "<channel> outbound copy"
```

Route-style commands resolve pack-owned persona aliases before routing. Check `requested_persona` and `persona_resolution` when the user supplied an alias.

4. Read `data.context.entries` first. Open `data.context.full_card_required` paths only when present. If `draft_status` is `no-draft`, surface the fit decision and do not draft. If the brief says the prospect is synthetic, treat it as a demo or fixture lead.
5. Build a copy brief with:

- audience/persona
- fit status or insufficient-context decision
- channel and motion
- lifecycle when relevant: initial touch vs follow-up
- prospect/source row context, signals, and assumptions
- loaded context entry ids and card ids
- approved positioning and claims
- approved hooks
- pains and triggers
- CTA style and reply path
- channel policy
- avoid-rules
- output-rules
- objections or alternatives
- evidence gaps
- output requirements

## Drafting Rules

If the user asks for copy after the brief, draft from bounded context entries and the prospect row first, including the routed CTA and output-rule entries when present. Open fallback card files only when `full_card_required` lists them. Keep unsupported claims out. State assumptions when source context is weak.

For outbound copy, preserve lifecycle distinctions. An initial email/LinkedIn touch should not use follow-up policy, and a follow-up should not use first-touch policy unless the route returns both intentionally.

Do not send or schedule the copy.
