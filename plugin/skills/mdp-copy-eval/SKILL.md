---
name: mdp-copy-eval
description: Use when the user wants to evaluate, QA, score, or revise generated LinkedIn/email copy against a Message Decision Pack, including ICP fit, loaded card fidelity, claims, avoid-rules, tone, CTA fit, channel constraints, and evidence gaps.
---

# MDP Copy Eval

Evaluate copy against the pack, not against generic taste.

## Workflow

1. Run or inspect the relevant brief:

```bash
mdp --json brief --dir . --prospect <prospect.json> --channel <channel>
```

or:

```bash
mdp --json emit-brief --dir . --persona "<persona>" --job "<channel> outbound copy"
```

2. Read only the returned card paths.
3. Compare the copy to:

- persona fit
- pain/trigger relevance
- approved hooks
- CTA style and reply path
- avoid-rules
- evidence requirements
- channel length and ask style
- unsupported or invented claims

## Scoring

Use a compact scorecard:

- Fit: pass/fail plus note
- Claim safety: pass/fail plus note
- Specificity: 1-5
- CTA fit: 1-5
- Channel fit: 1-5
- Revision needed: yes/no

If revising, make the smallest change that fixes the issue. Do not add new claims.
