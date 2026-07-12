---
name: mdp-pack-review
description: Use when the user asks to review, audit, harden, QA, or improve a Message Decision Pack. Checks structure, routing, ICP clarity, evidence strength, CTA policy, avoid-rules, output-rules, duplication, claim risk, and missing cards.
---

# MDP Pack Review

## Profile Gate

Before using this skill against an existing pack, run:

```bash
mdp --json agent-surface --dir .
```

Use this skill only when the surface is compatibility/generic or this skill is listed in `recommended_skills` or `allowed_skills` and is not listed in `blocked_skills`. If the surface blocks this skill, stop and reroute to an allowed or recommended skill named by the surface before editing or reviewing pack content.

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

Use `mdp --json capabilities` when reviewing agent-facing CLI support, and use `--strict` on `validate` or `eval` when warnings should fail the review gate. Use `mdp --json pack --dir . --out <path> --dry-run` before saving a portable pack artifact.

2. Review the manifest:

- format and version
- profile routing metadata (`profile.id`, `profile.agent_surface`) separately from activation readiness
- `required_primitives`, `primitive_map`, `input_contracts`, profile `jobs`, and `profile_eval.required_categories` when profile metadata is present
- `data.profile.activation_ready`, missing primitive coverage, and missing eval categories from `mdp validate`
- personas, target personas, and operator roles
- portfolio `context_dimensions`, canonical lowercase kebab-case values, and any `context_dimension_dependencies`
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
- prospect `attributes` used as an evidence dump instead of bounded reviewed row metadata
- arbitrary unsupported YAML fields that should move under entry `metadata` or become first-class card content
- entry `metadata` that agents can inspect but should not treat as an enforceable CLI rule
- product/capability/solution applicability hidden in `metadata` or `applies_to` instead of enforced entry `scope`
- scoped capability/solution entries missing their dependency dimensions, which can create impossible or blended portfolio routes
- portfolio-sensitive routes that expose full shared-card load paths as draftable context instead of bounded entries

4. Test routing with representative jobs:

```bash
mdp --json route --entries --dir . --persona "<persona>" --job "linkedin outbound copy"
mdp --json route --entries --dir . --persona "<persona>" --job "email follow-up"
```

For a portfolio pack, repeat the same cases with product/capability selectors and with scope intentionally omitted. Confirm product A excludes product B, global entries remain available, broader entries can match either selected product, missing/unknown/conflicting scope blocks drafting, and the pack includes selected-scope plus missing-scope eval fixtures. Treat `proof_output_scope_unsupported` as the explicit V1 proof-artifact boundary, not a warning to bypass.

## Findings Format

Lead with issues, ordered by severity:

- High: likely to cause wrong messaging or unsupported claims
- Medium: likely to reduce usefulness or routing quality
- Low: cleanup, wording, duplication, metadata polish

Include exact file paths and card ids when possible.
