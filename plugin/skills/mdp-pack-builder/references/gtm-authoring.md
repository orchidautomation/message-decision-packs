# GTM Pack Authoring

Read this for GTM profile decisions and job bindings.

## Required Shape

Author reviewed context for:

- ICP segments, account context, personas, buying roles, pains, triggers, and disqualifiers
- source-backed signals with provenance and freshness
- approved claims/proof and explicit unsupported claims
- message angles and objection handling grounded in evidence
- CTA policy, channel policy, avoid rules, and output constraints
- gaps and representative eval fixtures

Keep account context distinct from person/persona readiness. Account-only evidence must not become a plausible invented contact or draft-ready decision.

## Closed Job Bindings

```yaml
jobs:
  - id: prospect-fit-or-brief
    skill_id: mdp-gtm-brief
  - id: outbound-copy-brief
    skill_id: mdp-gtm-brief
  - id: outbound-copy-review
    skill_id: mdp-gtm-brief
```

Each full job entry must also declare required primitives and the `prospect` input contract. Do not add authoring or pack-validation jobs; shared skills handle those intents directly.

## Deterministic Checks

Use `mdp fit` for qualification, `mdp brief --context` for bounded pre-draft context, and `mdp check-claims` for supplied text. Author pack rules so those commands can decide; do not add prose-only parallel gates.
