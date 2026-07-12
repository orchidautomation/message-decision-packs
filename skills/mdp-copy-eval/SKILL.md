---
name: mdp-copy-eval
description: Use to evaluate copy against an MDP pack, including fit, routed card fidelity, claims, avoid-rules, output-rules, CTA fit, channel constraints, and evidence gaps.
---

# MDP Copy Eval

## Profile Gate

Before using this skill against an existing pack, run:

```bash
mdp --json agent-surface --dir .
```

Use this skill only when the surface is compatibility/generic or this skill is listed in `recommended_skills` or `allowed_skills` and is not listed in `blocked_skills`. If the surface blocks this skill, stop and reroute to an allowed or recommended skill named by the surface before editing or reviewing pack content.

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

When the route is portfolio-sensitive, pass the same reviewed selectors to every command in the evaluation path:

```bash
mdp --json emit-brief --dir . --persona "<persona>" --job "<channel> outbound copy" --scope product=<product-id>
mdp --json check-claims --dir . --text "<draft copy>" --persona "<persona>" --job "<channel> outbound copy" --scope product=<product-id>
```

Require `draft_status: ready` and evaluate only the bounded `context.entries` or `entry_route.matches`. Do not open or draft from full shared cards when `portfolio_sensitive: true`; their paths are audit metadata and may contain entries for other products. Stop on missing/invalid scope rather than choosing a product from the persona, company description, or draft text.

2. Run the deterministic claim/guardrail check against the draft:

```bash
mdp --json check-claims --dir . --text "<draft copy>"
```

`check-claims` is not send approval. A passing result only says the supplied draft text did not hit known claim or output guardrails. It does not verify subject-line behavior, attachments, images, HTML rendering, tracking, sender setup, inbox behavior, CRM writes, enrichment, or any external execution boundary.

When the draft has a subject line or route-specific channel constraints, include them:

```bash
mdp --json check-claims --dir . --text "<draft copy>" --subject "<subject>" --persona "<persona>" --job "<channel> outbound copy"
```

3. Use `check-claims` for `valid`, `matched_claims`, `claim_gaps`, `guardrail_hits`, `constraint_warnings`, `unchecked_constraints`, and `unsupported_claims`. `guardrail_hits` can come from avoid-rules, output-rules, routed `exact_paragraphs`, or routed draft-text entry `constraints`.
   For profile-owned packs, `check-claims` can use cards by `kind` when canonical IDs such as `claims`, `avoid-rules`, or `output-rules` are absent.
   Avoid/claim matching is deterministic and phrase-boundary aware. Obvious immediate negations such as `not auto-send`, `do not auto-send`, or `not an AI SDR` should pass unless another active unsafe claim is present; positive execution or unsupported-claim phrases should still fail. Safe boundary disclaimers such as "does not send emails", "does not connect to CRM", or "does not replace proposal management software" should pass when they clearly negate execution, integration, or platform-replacement claims. If a safe negated boundary fails, treat it as a guardrail false-positive and add a synthetic must-pass eval before broadening avoid literals.
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
- structured draft-text constraints such as word count, subject length, max questions, and forbidden links/html/tracking
- evidence requirements
- attributes used as metadata only, not proof
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
