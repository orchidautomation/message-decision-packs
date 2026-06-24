---
name: mdp-pack-review
description: Use when the user asks to review, audit, harden, QA, or improve a Message Decision Pack. Checks structure, routing, ICP clarity, evidence strength, CTA policy, avoid-rules, duplication, claim risk, and missing cards.
---

# MDP Pack Review

Review an MDP like a GTM control surface: structure first, then decision quality.

## Workflow

1. Run deterministic checks:

```bash
mdp --json doctor --dir .
mdp --json validate --dir .
mdp --json pack --dir .
```

2. Review the manifest:

- format and version
- personas
- card index
- progressive disclosure policy
- max cards per route

3. Review cards:

- unclear personas
- broad ICP
- weak or unsourced claims
- missing disqualifiers
- missing CTA rules or hard-sell asks
- missing avoid-rules
- duplicated hooks or pains
- copy patterns that imply unsupported claims

4. Test routing with representative jobs:

```bash
mdp --json route --dir . --persona "<persona>" --job "linkedin outbound copy"
mdp --json route --dir . --persona "<persona>" --job "email follow-up"
```

## Findings Format

Lead with issues, ordered by severity:

- High: likely to cause wrong messaging or unsupported claims
- Medium: likely to reduce usefulness or routing quality
- Low: cleanup, wording, duplication, metadata polish

Include exact file paths and card ids when possible.
