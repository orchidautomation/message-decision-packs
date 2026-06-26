---
name: mdp-pack-review
description: Use when the user asks to review, audit, harden, QA, or improve a Message Decision Pack. Checks structure, routing, ICP clarity, evidence strength, CTA policy, avoid-rules, output-rules, duplication, claim risk, and missing cards.
---

# MDP Pack Review

Review an MDP like a GTM control surface: structure first, then decision quality.

## Workflow

1. Run deterministic checks:

```bash
mdp --json doctor --dir .
mdp --json validate --dir .
mdp --json pack --dir .
mdp --json gaps --dir .
mdp --json eval --dir .
```

2. Review the manifest:

- format and version
- personas, target personas, and operator roles
- supported channels, including any custom channel names used by channel-policies
- card index
- progressive disclosure policy
- max cards per route

3. Review cards:

- unclear personas
- broad ICP
- missing fit-rules or no-message logic
- weak or unsourced claims
- missing signals or source interpretation rules
- missing disqualifiers
- missing channel policies
- missing CTA rules or hard-sell asks
- missing avoid-rules
- missing output-rules for global style or structure constraints
- missing objections or alternatives
- missing durable gaps
- duplicated hooks or pains
- copy patterns that imply unsupported claims or bury global style rules outside output-rules
- arbitrary unsupported YAML fields that should move under entry `metadata` or become first-class card content
- metadata that agents can inspect but should not treat as an enforceable CLI rule

4. Test routing with representative jobs:

```bash
mdp --json route --entries --dir . --persona "<persona>" --job "linkedin outbound copy"
mdp --json route --entries --dir . --persona "<persona>" --job "email follow-up"
```

## Findings Format

Lead with issues, ordered by severity:

- High: likely to cause wrong messaging or unsupported claims
- Medium: likely to reduce usefulness or routing quality
- Low: cleanup, wording, duplication, metadata polish

Include exact file paths and card ids when possible.
