---
name: mdp-pack-eval
description: Use when the user wants to test whether a Message Decision Pack routes correctly across sample personas, jobs, channels, prospect rows, or copy tasks. Produces route checks, failure cases, and recommended card metadata fixes.
---

# MDP Pack Eval

Test whether the pack routes the right cards for realistic GTM tasks.

## Workflow

1. Validate structure:

```bash
mdp --json validate --dir .
```

2. Choose representative cases:

- each primary persona
- LinkedIn opener
- email follow-up
- call prep
- source extraction or pack review if relevant
- one bad-fit or unsupported persona

3. Run routes:

```bash
mdp --json route --dir . --persona "<persona>" --job "<job>"
```

4. For prospect cases, run:

```bash
mdp --json brief --dir . --prospect <prospect.json> --channel <channel>
```

## Evaluate

For each case, check:

- expected base cards loaded
- avoid-rules included for copy jobs
- ctas included for outbound/message/copy jobs
- persona-specific cards included
- irrelevant cards omitted
- route does not exceed policy limits
- decision trace is understandable

## Fixes

Recommend metadata edits to manifest card refs first: personas, tags, descriptions, and max route size. Edit card content only when routing exposes a real content gap.
