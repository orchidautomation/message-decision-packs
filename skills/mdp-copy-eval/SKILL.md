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

When the draft has a subject line or route-specific channel constraints, include them:

```bash
mdp --json check-claims --dir . --text "<draft copy>" --subject "<subject>" --persona "<persona>" --job "<channel> outbound copy"
```

3. Use `check-claims` for `valid`, `matched_claims`, `claim_gaps`, `guardrail_hits`, `constraint_warnings`, `unchecked_constraints`, and `unsupported_claims`. `guardrail_hits` can come from avoid-rules, output-rules, or routed entry `constraints`.
   For profile-owned packs, `check-claims` can use cards by `kind` when canonical IDs such as `claims`, `avoid-rules`, or `output-rules` are absent.
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
- structured constraints such as word count, subject length, max questions, and forbidden links/html/tracking
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
- Structured constraints: pass/fail plus target warnings or unchecked metadata notes
- Revision needed: yes/no

If revising, make the smallest change that fixes the issue. Do not add new claims.
