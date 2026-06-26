---
name: mdp-avoid-rules
description: Use to codify MDP avoid-rule cards for claim boundaries, forbidden claims, compliance-sensitive language, category boundaries, bad-fit personas, and no-send criteria.
---

# MDP Avoid Rules

Create guardrails that prevent agents from overclaiming or drifting into the wrong product category.

## Workflow

1. Validate the current pack.
2. Review current positioning, claims, ICP, and planned copy patterns.
3. Identify the ways an agent might overreach.
4. Add explicit entries to `.mdp/cards/avoid-rules.yaml`, and use `.mdp/cards/fit-rules.yaml` for no-message or disqualification rules.
5. If needed, add supporting constraints to positioning, claims, personas, pains, hooks, channel-policies, ctas, output-rules, or copy patterns.
6. Validate the pack again.

## Avoid Rule Categories

Cover the categories that apply:

- unsupported product claims
- exaggerated ROI or time savings
- unverified customer, integration, or pricing claims
- regulated or compliance-sensitive claims
- category confusion
- wording guardrails tied to unsafe or unsupported claims
- disallowed channels or no-send criteria
- bad-fit segments and personas

Use `.mdp/cards/output-rules.yaml` instead for global style and structure rules such as no em dashes, fixed paragraph counts, formatting constraints, or no meta commentary.

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
mdp --json brief --context --dir . --prospect <prospect.json> --channel <channel>
mdp --json check-claims --dir . --text "<draft copy>"
```

Check that avoid-rules appear in `required_load_order` and guardrail entries appear in `context.entries` for copy jobs. If the change is a style or structure constraint, check that output-rules appear too.
