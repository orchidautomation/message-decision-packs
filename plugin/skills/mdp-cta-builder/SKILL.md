---
name: mdp-cta-builder
description: Use to codify MDP CTA card rules, reply paths, ask styles, meeting boundaries, handoff prompts, or no-send CTA guidance.
---

# MDP CTA Builder

Create CTA guidance as a decision card, not as one-off copy. CTA rules should tell an agent what kind of ask is allowed, when to use it, and what not to ask for.

## Workflow

1. Validate the pack:

```bash
mdp --json validate --dir .
```

2. Run a route for the active persona/channel/job:

```bash
mdp --json route --entries --dir . --persona "<persona>" --job "<channel> CTA policy"
```

3. Read `.mdp/cards/ctas.yaml` plus only routed channel-policies, persona, pains, hooks, copy-patterns, claims, gaps, and avoid-rules cards needed for the change.
4. Add or revise CTA entries with:

- ask type
- channel fit
- persona fit
- source or evidence requirement
- reply path when a meeting ask is too strong
- avoid phrases or pressure tactics

5. Keep CTAs small and reusable. If final copy is requested, route to `$mdp-copy-brief` or `$mdp-copy-eval`; this skill only maintains CTA policy.
6. Validate routing:

```bash
mdp --json route --entries --dir . --persona "<persona>" --job "<channel> outbound copy"
```

Confirm `.mdp/cards/ctas.yaml` and `.mdp/cards/channel-policies.yaml` appear in `load_order` for outbound, message, CTA, or copy jobs.

## CTA Patterns

Prefer concrete patterns such as:

- compare notes
- sanity-check this hypothesis
- who owns this internally
- worth a quick look
- should I send the short version

Avoid hard calendar pushes, false urgency, implied prior interest, and unsupported pain certainty.
