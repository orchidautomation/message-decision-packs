---
name: mdp-recruiting-candidate-evidence-review
description: Use when the user wants to map supplied candidate evidence to job-related criteria in a Recruiting MDP pack. Produces a source-backed criterion-by-criterion evidence and gap matrix for human review; never scores, ranks, compares, recommends, advances, rejects, or hires candidates.
---

# MDP Recruiting Candidate Evidence Review

## Profile Gate

Run `mdp --json agent-surface --dir .`. Continue only when this skill is allowed and not blocked.

## Inputs and Source Gate

Require the pack directory, supplied role criteria, supplied candidate evidence, explicit source references, source classification, requested scope, and human reviewer. Candidate is the evidence subject, not an operator persona.

Use only synthetic, sanitized, public-supplied, or user-approved local evidence. Restricted or unverified material is not review-ready. Do not browse, scrape, enrich, background-check, or retrieve missing facts. Never infer protected characteristics or proxies. Never invent candidate facts or source status.

## Workflow

1. Run strict validation, gaps, and the Recruiting agent surface.
2. Normalize supplied context through `normalize-recruiting-context` and validate it.
3. Route with:

```bash
mdp --json --summary route --entries --dir . --persona "Recruiter" --job "candidate evidence review"
```

4. Review each job-related criterion independently. No evidence means `Not assessed`, not a negative finding.
5. Use only `Source-backed`, `Partial evidence`, `Gap`, `Not assessed`, and `Needs human review`.
6. Run `mdp check-claims` on material review text.
7. When text cites card or source IDs, create `mdp.proof-output.v0` segments and run `mdp verify-output`. A citation is not proof until it resolves and full-text claim checks pass.

## Output

Return one row per criterion with rationale, supplied evidence, source and confidence, bounded label, conflict or gap, prohibited inference avoided, reviewer question, and `Needs human review`.

Do not include a total, weight, score, rank, comparison, fit label, recommendation, advance, reject, hire, or final-outcome field. If asked for one, refuse that portion and provide the safe criterion-level matrix or gaps.
