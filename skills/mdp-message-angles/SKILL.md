---
name: mdp-message-angles
description: Use to codify MDP hooks, message-angle, copy-pattern, objection, channel-policy, and CTA card entries from grounded GTM context.
---

# MDP Message Angles

Create reusable message angles for an MDP. Store strategy in cards; do not turn this into a final sending workflow.

## Workflow

1. Identify the active persona, channel, and job.
2. Validate and route before reading cards:

```bash
mdp --json validate --dir .
mdp --json route --entries --dir . --persona "<persona>" --job "<channel> outbound copy"
```

3. Read only routed cards plus any card you are explicitly updating.
4. Generate candidate card entries for hooks, proof-backed claims, channel policies, objections, copy patterns, output rules, and CTA options.
5. Keep only angles grounded in source context or explicitly marked assumptions.
6. Update:

- `.mdp/cards/hooks.yaml`
- `.mdp/cards/claims.yaml`
- `.mdp/cards/channel-policies.yaml`
- `.mdp/cards/copy-patterns.yaml`
- `.mdp/cards/ctas.yaml`
- `.mdp/cards/objections.yaml`
- `.mdp/cards/pains.yaml` when the angle depends on a missing pain
- `.mdp/cards/gaps.yaml` when evidence is missing
- `.mdp/cards/avoid-rules.yaml` when an angle risks overclaiming
- `.mdp/cards/output-rules.yaml` when an angle creates global style or structure constraints

## Angle Structure

Each useful angle should include:

- observable trigger
- buyer pain
- message hook
- proof or source requirement
- avoid-rule risk
- best channel
- CTA or reply path
- output-rule constraints
- persona fit

## Copy Pattern Structure

Use patterns like:

- trigger -> pain -> specific angle -> low-pressure ask
- source observation -> hypothesis -> relevance -> question
- current workflow -> cost of status quo -> safer next step

Avoid generic "AI transformation" language unless the source material supports it.

## Validate

```bash
mdp --json validate --dir .
mdp --json route --entries --dir . --persona "<persona>" --job "<channel> outbound copy"
mdp --json gaps --dir .
```

Confirm that hooks, copy patterns, and CTA rules route for the intended persona and job.
