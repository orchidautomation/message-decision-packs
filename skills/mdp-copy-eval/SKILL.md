---
name: mdp-copy-eval
description: Use to evaluate copy against an MDP pack, including fit, routed card fidelity, claims, avoid-rules, output-rules, CTA fit, channel constraints, and evidence gaps.
---

# MDP Copy Eval

Evaluate copy against the pack, not against generic taste.

## Workflow

1. Run or inspect the relevant brief:

```bash
mdp --json brief --context --dir . --prospect <prospect.json> --channel <channel>
```

or:

```bash
mdp --json emit-brief --dir . --persona "<persona>" --job "<channel> outbound copy"
```

2. Run the deterministic claim/guardrail check against the draft:

```bash
mdp --json check-claims --dir . --text "<draft copy>"
```

3. Use `check-claims` for `valid`, `matched_claims`, `claim_gaps`, `guardrail_hits`, and `unsupported_claims`. `guardrail_hits` can come from avoid-rules or output-rules.
4. Read `context.entries` first for prospect briefs. Open card files only from `context.full_card_required`, a brief `required_load_order`, or route `load_order` when the bounded context is missing or insufficient.
5. Compare the copy to:

- persona fit
- pain/trigger relevance
- approved positioning and claims
- approved hooks
- CTA style and reply path
- channel policy
- avoid-rules
- output-rules
- evidence requirements
- gaps the copy should surface rather than hide
- channel length and ask style
- global style, punctuation, and structure rules
- unsupported or invented claims
- loaded card ids, missing card ids, or unrouted card references

## Scoring

Use a compact scorecard:

- Fit: pass/fail plus note
- Claim safety: pass/fail plus note
- Specificity: 1-5
- CTA fit: 1-5
- Channel fit: 1-5
- Output rules: pass/fail plus note
- Revision needed: yes/no

If revising, make the smallest change that fixes the issue. Do not add new claims.
