---
name: mdp-source-extract
description: Use to turn user-provided or public source material into MDP card entries with evidence, confidence, freshness, and explicit gaps.
---

# MDP Source Extract

Extract pack-ready card entries from source material. Preserve evidence and do not smooth over gaps.

## Source Rules

- Prefer primary sources and user-provided source files.
- If using URLs, fetch current source content when facts may have changed.
- Do not scrape private, gated, or authenticated sources. Use user-provided files, notes, or public/current URLs.
- Keep source URLs, document names, or note identifiers in `evidence`.
- Add each source to `.mdp/sources.yaml` before using it to write many card entries.
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
- output-rules: global style, punctuation, formatting, and structure constraints
- objections: alternatives and response logic
- copy-patterns: reusable message structures, not final sends
- ctas: asks, reply paths, and meeting boundaries
- gaps: missing proof, unclear fit, and open questions

## Workflow

1. Read the provided source material or fetch public/current URLs when appropriate.
2. Update `.mdp/sources.yaml` with source id, kind, locator, freshness, confidence, direct claims, interpretations, and gaps.
3. Extract candidate entries by card kind.
4. Save the model output as a review artifact before editing cards.
5. Validate the review artifact against the prompt contract.
6. Deduplicate overlapping claims.
7. Add evidence per entry using source ids, URLs, or document names from the ledger.
8. Copy only reviewed entry fields into relevant `.mdp/cards/*.yaml` files.
9. Run validation.

```bash
mdp --json validate --dir .
mdp --json validate-prompt-output --dir . --prompt-id <prompt-id> --file <output.json>
mdp --json gaps --dir .
```

## Output

Report:

- sources used
- entries added or changed
- confidence and freshness for extracted entries when known
- evidence gaps
- claims that should become avoid-rules
- style or structure preferences that should become output-rules
