---
name: mdp-create-pack
description: Use when the user wants to create a new Message Decision Pack from messy GTM context, product notes, ICP notes, positioning docs, website research, sales context, or a blank workspace. Creates or improves `.mdp/manifest.yaml` and starter card files, then validates with the `mdp` CLI.
---

# MDP Create Pack

Create a usable `.mdp/` pack from the user's GTM context. The goal is a small manifest plus modular cards, not one giant prompt file.

## Workflow

1. Check the CLI:

```bash
command -v mdp
mdp --json doctor --dir .
```

2. If no pack exists, initialize:

```bash
mdp --json init --template gtm --name "<pack name>" --dir .
```

3. Gather or infer the first version:

- product/category in one sentence
- target segments
- buying personas and user personas
- pains and triggers
- allowed message motions
- hooks and proof points
- CTA style, reply paths, and ask boundaries
- avoid-rules and claim boundaries
- copy patterns by channel
- open gaps that need source evidence

4. Edit only the pack files:

- `.mdp/manifest.yaml`
- `.mdp/cards/personas.yaml`
- `.mdp/cards/pains.yaml`
- `.mdp/cards/motions.yaml`
- `.mdp/cards/hooks.yaml`
- `.mdp/cards/avoid-rules.yaml`
- `.mdp/cards/copy-patterns.yaml`
- `.mdp/cards/ctas.yaml`

5. Validate:

```bash
mdp --json validate --dir .
mdp --json explain --dir .
```

## Authoring Rules

- Keep each card small and task-specific.
- Put evidence URLs or source names on entries when available.
- Mark guesses as assumptions in the card body.
- Prefer concrete disqualifiers over vague ICP language.
- Do not describe the pack as a sender, CRM, sequencer, enrichment provider, AI SDR, or execution agent.
- Do not invent customer names, pricing, integrations, or proof points.

## Response

End with files changed, validation result, strongest gaps, and the next command to produce a prospect brief.
