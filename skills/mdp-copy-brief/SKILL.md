---
name: mdp-copy-brief
description: Use to produce an MDP-routed writing contract from pack cards, fit status, persona, prospect row, channel, motion, or job before drafting.
---

# MDP Copy Brief

Create a writing brief from MDP routing. The brief should constrain a copywriter or model before copy is drafted.

## Workflow

1. If a prospect row exists, run fit first:

```bash
mdp --json fit --dir . --prospect <prospect.json>
```

Hard-stop on `disqualified` or `insufficient-context` unless the user explicitly overrides.

2. With a prospect row, build the brief:

```bash
mdp --json brief --dir . --prospect <prospect.json> --channel <channel>
```

3. Without a prospect row, build a persona/job brief:

```bash
mdp --json emit-brief --dir . --persona "<persona>" --job "<channel> outbound copy"
```

4. Read only the returned `required_load_order` card files. If `draft_status` is `no-draft`, surface the fit decision and do not draft.
5. Build a copy brief with:

- audience/persona
- fit status or insufficient-context decision
- channel and motion
- prospect context, signals, and assumptions
- loaded card ids
- approved positioning and claims
- approved hooks
- pains and triggers
- CTA style and reply path
- channel policy
- avoid-rules
- objections or alternatives
- evidence gaps
- output requirements

## Drafting Rules

If the user asks for copy after the brief, draft only from loaded cards and the prospect row, including the routed CTA card when present. Keep unsupported claims out. State assumptions when source context is weak.

Do not send or schedule the copy.
