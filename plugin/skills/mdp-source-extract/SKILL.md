---
name: mdp-source-extract
description: Use when the user wants to turn source material such as websites, positioning docs, sales notes, call summaries, customer research, product docs, or competitive notes into Message Decision Pack card entries with evidence and gaps.
---

# MDP Source Extract

Extract pack-ready card entries from source material. Preserve evidence and do not smooth over gaps.

## Source Rules

- Prefer primary sources and user-provided source files.
- If using URLs, fetch current source content when facts may have changed.
- Keep source URLs, document names, or note identifiers in `evidence`.
- Separate direct source claims from interpretation.
- Mark missing evidence as a gap instead of inventing proof.

## Extraction Targets

Map source material into:

- positioning: category, boundaries, and what the product/pack is not
- personas: who cares and why
- fit-rules: qualification, disqualification, and no-message cases
- signals: source fields, triggers, confidence, and freshness
- pains: problems, triggers, stakes, current alternatives
- claims: approved claims with proof/evidence
- motions: approved workflows and where the message is used
- channel-policies: LinkedIn/email/call-prep rules
- hooks: reusable claims or angles with source evidence
- avoid-rules: unsupported claims, risks, category boundaries
- objections: alternatives and response logic
- copy-patterns: reusable message structures, not final sends
- ctas: asks, reply paths, and meeting boundaries
- gaps: missing proof, unclear fit, and open questions

## Workflow

1. Read or fetch the source material.
2. Extract candidate entries by card kind.
3. Deduplicate overlapping claims.
4. Add evidence per entry.
5. Edit only relevant `.mdp/cards/*.yaml` files.
6. Run validation.

```bash
mdp --json validate --dir .
mdp --json gaps --dir .
```

## Output

Report:

- sources used
- entries added or changed
- evidence gaps
- claims that should become avoid-rules
