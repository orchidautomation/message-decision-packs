---
name: mdp-create-pack
description: Use when the user wants to create a new Message Decision Pack from messy GTM or proposal context, product notes, ICP notes, positioning docs, sales/proposal context, or a blank workspace. Creates or improves `.mdp/` cards and validates with `mdp`.
---

# MDP Create Pack

## Profile Gate

Before using this skill against an existing pack, run:

```bash
mdp --json agent-surface --dir .
```

Use this skill only when the surface is legacy/generic or this skill is listed in `recommended_skills` or `allowed_skills` and is not listed in `blocked_skills`. If the surface blocks this skill, stop and reroute to an allowed or recommended skill named by the surface before editing or reviewing pack content.

Create a usable `.mdp/` pack from the user's GTM context. The goal is a small manifest plus modular cards, not one giant prompt file.

## Workflow

1. Check the CLI:

```bash
command -v mdp
mdp --json doctor --dir .
```

2. State the destination directory before writing. If Brandon did not specify one, use the current workspace root for durable work or an ignored scratch path for disposable demos. Do not silently create a pack in `$HOME`.

3. If no pack exists, initialize with the closest template. Use `gtm` for generic messaging packs and `proposal` only when the user is explicitly building a proposal/RFP/capture review pack:

```bash
mdp --json init --template gtm --name "<pack name>" --dir .
# or
mdp --json init --template proposal --dir .
```

4. Build the source ledger before writing cards. Add public URLs, user-provided docs, or note identifiers to `.mdp/sources.yaml`; separate direct source claims from interpretation; preserve missing proof in `gaps.yaml`.

5. Gather or infer the first version:

- product/category in one sentence
- positioning and product boundaries
- target segments
- buying personas and user personas
- fit rules, disqualifiers, and no-message cases
- structured signals from prospect rows, public research, website, or source material
- pains and triggers
- approved claims and proof requirements
- allowed message motions
- channel policies
- hooks and proof points
- CTA style, reply paths, and ask boundaries
- avoid-rules and claim boundaries
- output rules for global style, formatting, deterministic constraints, and structure constraints
- objections and alternatives
- copy patterns by channel
- open gaps that need source evidence

6. Edit only the pack files:

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

For proposal packs, keep the template's profile-owned card IDs and review jobs unless the user explicitly asks for a new proposal profile. Proposal packs may not have GTM prompt files by default; do not invent private proposal content or overwrite customer material.

Work in slices instead of rewriting the whole pack at once:

- First: positioning, fit-rules, claims, gaps, source ledger, and runtime prospect normalization prompt.
- Second: personas, signals, pains, and motions.
- Third: channel-policies, hooks, ctas, output-rules, copy-patterns, objections, and evals.

7. Validate after each meaningful slice:

```bash
mdp --json validate --dir .
mdp --json explain --dir .
mdp --json gaps --dir .
mdp --json eval --dir .
```

Use route-derived eval scaffolds before hand-writing assertions:

```bash
mdp --json --summary route --entries --eval-fixture --dir . --persona "<persona>" --job "<channel> outbound copy"
```

## Authoring Rules

- Keep each card small and task-specific.
- Put evidence URLs or source names on entries when available.
- Put source inventory and interpretation notes in `.mdp/sources.yaml` before compressing research into cards.
- Mark guesses as assumptions in the card body.
- Prefer concrete disqualifiers over vague ICP language.
- Put global style and structure rules, such as banned punctuation or fixed paragraph counts, in `output-rules.yaml` instead of burying them in examples.
- Keep channel rule responsibilities separate: `channel-policies.yaml` owns channel/lifecycle entries such as email initial, email follow-up, LinkedIn initial, LinkedIn follow-up, call prep, and agent brief; `output-rules.yaml` owns generated-text and formatting constraints; `ctas.yaml` owns ask boundaries and reply paths; `copy-patterns.yaml` owns reusable structures.
- Use entry `constraints` for deterministic output limits such as body word counts, subject word counts, subject avoid literals, max questions, and forbidden links, attachments, images, HTML, or tracking. Keep sequence-wide policies such as max follow-up count in prose or evals unless the supplied draft includes enough context to check them.
- Do not describe the pack as a sender, CRM, sequencer, enrichment provider, AI SDR, or execution agent.
- Do not invent customer names, pricing, integrations, or proof points.
- Mark generated example prospects as synthetic fixtures and do not confuse them with real target accounts.
- Use prospect `attributes` for bounded reviewed row metadata that `mdp fit` may require through `lead_input_requirements.required_attributes`; put evidence and provenance in `signals[].source`.
- Define pack-owned prompt value domains in `.mdp/manifest.yaml` `lead_input_requirements.value_contracts` and `attribute_definitions` when prompts should emit blessed personas, segments, source kinds, date/date-time values, enums, or typed attributes. Do not leave important routing labels as free-form prompt prose.
- Put custom advisory annotations about card content under entry `metadata`; do not add arbitrary unsupported fields as if the CLI routes or enforces them.
- Add custom channel names to `.mdp/manifest.yaml` `supported_channels` before relying on channel-policy routing for those channels.

## Response

End with files changed, validation result, strongest gaps, whether any brief was saved with `--out`, and the next command to produce a prospect brief.
