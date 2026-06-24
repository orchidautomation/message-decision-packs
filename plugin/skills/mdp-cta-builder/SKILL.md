---
name: mdp-cta-builder
description: Use when the user wants to create, refine, audit, or codify calls to action, reply paths, asks, meeting asks, handoff prompts, or no-send CTA guidance for a Message Decision Pack.
---

# MDP CTA Builder

Create CTA guidance as a decision card, not as one-off copy. CTA rules should tell an agent what kind of ask is allowed, when to use it, and what not to ask for.

## Workflow

1. Validate the pack:

```bash
mdp --json validate --dir .
```

2. Read `.mdp/cards/ctas.yaml` plus routed channel-policies, persona, pains, hooks, copy-patterns, claims, gaps, and avoid-rules cards.
3. Add or revise CTA entries with:

- ask type
- channel fit
- persona fit
- source or evidence requirement
- reply path when a meeting ask is too strong
- avoid phrases or pressure tactics

4. Keep CTAs small and reusable. Do not draft final messages unless the user asks.
5. Validate routing:

```bash
mdp --json route --entries --dir . --persona "<persona>" --job "linkedin outbound copy"
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
