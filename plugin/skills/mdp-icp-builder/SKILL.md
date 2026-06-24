---
name: mdp-icp-builder
description: Use when the user needs to define, sharpen, or codify ICP, segments, personas, disqualifiers, pains, triggers, buying committees, or fit logic for a Message Decision Pack. Produces content for MDP personas, pains, motions, and avoid-rules cards.
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
- triggers: events that make outreach timely
- disqualifiers: who should not be targeted
- routing hints: which card tags should match which jobs

Write results into:

- `.mdp/cards/personas.yaml`
- `.mdp/cards/pains.yaml`
- `.mdp/cards/motions.yaml`
- `.mdp/cards/avoid-rules.yaml` when fit boundaries need enforcement

## Quality Bar

- Each persona should have a clear reason to care.
- Each pain should imply a specific message angle.
- Each trigger should be observable from enrichment, research, or user-supplied data.
- Disqualifiers should be concrete enough for an agent to apply.
- Do not make the ICP so broad that every company qualifies.

## Validate

```bash
mdp --json validate --dir .
mdp --json route --dir . --persona "<persona>" --job "linkedin outbound copy"
```

Report any routing mismatch and adjust card metadata when needed.
