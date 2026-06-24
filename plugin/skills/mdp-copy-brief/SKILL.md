---
name: mdp-copy-brief
description: Use when the user wants a model-ready copywriting brief from an MDP, routed cards, persona, prospect row, channel, motion, or job. Produces a controlled writing contract before drafting LinkedIn/email copy.
---

# MDP Copy Brief

Create a writing brief from MDP routing. The brief should constrain a copywriter or model before copy is drafted.

## Workflow

1. If a prospect row exists, run:

```bash
mdp --json brief --dir . --prospect <prospect.json> --channel <channel>
```

2. If no prospect row exists, run:

```bash
mdp --json emit-brief --dir . --persona "<persona>" --job "<channel> outbound copy"
```

3. Read only the returned `required_load_order` card files.
4. Build a copy brief with:

- audience/persona
- channel and motion
- prospect context or assumptions
- loaded card ids
- approved hooks
- pains and triggers
- CTA style and reply path
- avoid-rules
- evidence gaps
- output requirements

## Drafting Rules

If the user asks for copy after the brief, draft only from loaded cards and the prospect row, including the routed CTA card when present. Keep unsupported claims out. State assumptions when source context is weak.

Do not send or schedule the copy.
