---
name: mdp-message-angles
description: Use when the user wants to brainstorm, refine, or codify GTM messaging angles, hooks, proof patterns, objections, channel-specific copy structures, LinkedIn/email openers, CTA styles, reply paths, or message themes for a Message Decision Pack.
---

# MDP Message Angles

Create reusable message angles for an MDP. Store strategy in cards; do not turn this into a final sending workflow.

## Workflow

1. Read the manifest and relevant cards.
2. Identify the active persona, channel, and job.
3. Generate candidate hooks, proof-backed claims, channel policies, objections, copy patterns, and CTA options.
4. Keep only angles grounded in source context or explicitly marked assumptions.
5. Update:

- `.mdp/cards/hooks.yaml`
- `.mdp/cards/claims.yaml`
- `.mdp/cards/channel-policies.yaml`
- `.mdp/cards/copy-patterns.yaml`
- `.mdp/cards/ctas.yaml`
- `.mdp/cards/objections.yaml`
- `.mdp/cards/pains.yaml` when the angle depends on a missing pain
- `.mdp/cards/gaps.yaml` when evidence is missing
- `.mdp/cards/avoid-rules.yaml` when an angle risks overclaiming

## Angle Structure

Each useful angle should include:

- observable trigger
- buyer pain
- message hook
- proof or source requirement
- avoid-rule risk
- best channel
- CTA or reply path
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
