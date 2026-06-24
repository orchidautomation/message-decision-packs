---
name: mdp-avoid-rules
description: Use when the user needs claim boundaries, forbidden claims, tone rules, legal/compliance guardrails, category boundaries, disallowed personas, no-send criteria, or other avoid rules for a Message Decision Pack.
---

# MDP Avoid Rules

Create guardrails that prevent agents from overclaiming or drifting into the wrong product category.

## Workflow

1. Review current positioning, claims, ICP, and planned copy patterns.
2. Identify the ways an agent might overreach.
3. Add explicit entries to `.mdp/cards/avoid-rules.yaml`, and use `.mdp/cards/fit-rules.yaml` for no-message or disqualification rules.
4. If needed, add supporting constraints to positioning, claims, personas, pains, hooks, channel-policies, ctas, or copy patterns.
5. Validate the pack.

## Avoid Rule Categories

Cover the categories that apply:

- unsupported product claims
- exaggerated ROI or time savings
- unverified customer, integration, or pricing claims
- regulated or compliance-sensitive claims
- category confusion
- tone and style boundaries
- disallowed channels or no-send criteria
- bad-fit segments and personas

## Entry Requirements

Each avoid rule should include:

- what not to say or do
- why it matters
- examples of blocked language in `avoid`
- affected personas in `applies_to`
- evidence when the boundary comes from source material

## Validate

```bash
mdp --json validate --dir .
mdp --json brief --dir . --prospect examples/clay-row.json --channel linkedin
mdp --json check-claims --dir . --text "<draft copy>"
```

Check that avoid-rules appear in `required_load_order` for copy jobs.
