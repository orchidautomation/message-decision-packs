---
name: mdp-message-angles
description: Use to codify MDP hooks, message-angle, copy-pattern, objection, channel-policy, and CTA card entries from grounded GTM context.
---

# MDP Message Angles

## Profile Gate

Before using this skill against an existing pack, run:

```bash
mdp --json agent-surface --dir .
```

Use this skill only when the surface is legacy/generic or this skill is listed in `recommended_skills` or `allowed_skills` and is not listed in `blocked_skills`. If the surface blocks this skill, stop and reroute to an allowed or recommended skill named by the surface before editing or reviewing pack content.

Create reusable message angles for an MDP. Store strategy in cards; do not turn this into a final sending workflow.

## Workflow

1. Identify the active persona, channel, and job.
   - For outbound jobs, identify lifecycle too: initial touch or follow-up.
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
- lifecycle: initial touch, follow-up, or lifecycle-neutral
- CTA or reply path
- output-rule constraints
- persona fit

## Copy Pattern Structure

Use patterns like:

- trigger -> pain -> specific angle -> low-pressure ask
- trigger or hypothesis -> proof gap -> approved angle -> one soft CTA
- source observation -> hypothesis -> relevance -> question
- current workflow -> cost of status quo -> safer next step

Avoid generic "AI transformation" language unless the source material supports it.
Put channel/lifecycle rules in channel-policies, ask boundaries in ctas, generated-text constraints in output-rules, and only reusable narrative structures in copy-patterns.

## Validate

```bash
mdp --json validate --dir .
mdp --json route --entries --dir . --persona "<persona>" --job "<channel> outbound copy"
mdp --json gaps --dir .
```

Confirm that hooks, copy patterns, and CTA rules route for the intended persona and job.
For channel-policies, confirm first-touch and follow-up entries do not both appear unless that is intentional.
