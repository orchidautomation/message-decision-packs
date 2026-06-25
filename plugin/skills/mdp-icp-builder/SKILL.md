---
name: mdp-icp-builder
description: "Use to codify MDP ICP and fit cards: segments, personas, disqualifiers, pains, triggers, buying committees, signals, and no-message logic."
---

# MDP ICP Builder

Turn fuzzy ICP thinking into explicit pack content that agents can route against.

## Inputs To Seek

Use whatever the user already provided. Do not block unless a missing detail would make the pack misleading.

- product/category
- current best customers or target accounts
- excluded customers or bad-fit examples
- buyer/user personas
- operational triggers
- pains and stakes
- competitive alternatives
- source evidence or confidence level

## Build The ICP

Create concise entries for:

- segments: company types that should be considered
- personas: roles, jobs, pains, and objections
- fit rules: when to proceed, ask for more context, or stop
- signals: observable events or source fields that make outreach timely
- disqualifiers: who should not be targeted
- objections: expected confusion and approved response logic
- gaps: unknowns agents must surface instead of guessing
- routing hints: which card tags should match which jobs

Write results into:

- `.mdp/cards/personas.yaml`
- `.mdp/cards/fit-rules.yaml`
- `.mdp/cards/signals.yaml`
- `.mdp/cards/pains.yaml`
- `.mdp/cards/motions.yaml`
- `.mdp/cards/objections.yaml`
- `.mdp/cards/gaps.yaml`
- `.mdp/cards/avoid-rules.yaml` when fit boundaries need enforcement

## Quality Bar

- Each persona should have a clear reason to care.
- Each pain should imply a specific message angle.
- Each trigger should be observable from user-provided sources, public research, or structured prospect rows.
- Disqualifiers should be concrete enough for an agent to apply.
- No-message cases belong in `fit-rules.yaml`, not only in persona notes or examples.
- Do not make the ICP so broad that every company qualifies.

## Validate

```bash
mdp --json validate --dir .
mdp --json fit --dir . --prospect <prospect.json>
mdp --json route --entries --dir . --persona "<persona>" --job "linkedin outbound copy"
```

Report any routing mismatch and adjust card metadata when needed.
