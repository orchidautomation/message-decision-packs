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
mdp --json eval --dir .
```

2. Choose representative cases:

- each primary persona
- LinkedIn initial touch and LinkedIn follow-up
- initial email and email follow-up
- call prep
- source extraction or pack review if relevant
- one bad-fit or unsupported persona

3. Run routes:

```bash
mdp --json --summary route --entries --eval-fixture --dir . --persona "<persona>" --job "<job>"
```

4. For prospect cases, run:

```bash
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel <channel>
```

## Evaluate

For each case, check:

- expected base cards loaded
- expected bounded context entries returned for prospect briefs
- fit-rules and positioning included when relevant
- claims included for copy jobs that use proof
- avoid-rules included for copy jobs
- ctas and channel-policies included for outbound/message/copy jobs
- initial vs follow-up channel-policy entries route separately for email and LinkedIn
- persona-specific cards included
- entry-level route is useful, not noisy
- irrelevant cards omitted
- route does not exceed policy limits
- eval fixtures pass
- decision trace is understandable
- generated eval fixture scaffolds are reviewed before committing so tests encode intended behavior, not accidental routing noise

## Fixes

Recommend metadata edits to manifest card refs first: personas, tags, descriptions, and max route size. Edit card content only when routing exposes a real content gap.
