---
name: mdp-recruiting-scorecard-gap-review
description: Use when the user wants to review a supplied Recruiting scorecard or evidence matrix for missing, conflicting, weak, restricted, unverified, or unsupported evidence. Produces criterion-level gaps and human-review questions without totals, weights, ranks, comparisons, recommendations, or candidate outcomes.
---

# MDP Recruiting Scorecard Gap Review

## Profile Gate

Run `mdp --json agent-surface --dir .`. Continue only when this skill is allowed and not blocked.

## Inputs

Require the pack directory, supplied job-related criteria, supplied evidence rows, source references and classifications, scorecard scope, and human review owner. A scorecard here is a criterion-level review artifact, not a numerical decision model.

## Workflow

1. Run strict validation, eval, and gaps.
2. Normalize supplied context with `normalize-recruiting-context` and validate it. Require every expected scorecard/evidence source to appear exactly once as present, empty, or missing, preserve opaque identity by default, and carry unresolved gaps plus the human owner in `review_handoff`.
3. Route with:

```bash
mdp --json --summary route --entries --dir . --persona "Hiring Manager" --job "scorecard gap review"
```

4. Check every criterion for a supplied role source, job-related rationale, candidate evidence source, confidence, conflicts, prohibited proxies, and missing proof.
5. Preserve restricted or unverified material as a gap. Do not repair it by browsing, enrichment, inference, or invention.
6. Use only `Source-backed`, `Partial evidence`, `Gap`, `Not assessed`, and `Needs human review`.
7. Verify evidence-carrying text with `mdp check-claims` and `mdp verify-output` when it cites pack/source IDs.

## Output

Return criterion-level rows with source, evidence label, conflict, gap, reviewer question, and needed human action. Also return unsafe or unsupported fields removed from consideration.

Do not total, weight, score, rank, compare, recommend, advance, reject, hire, or generate an overall candidate result. Refuse those requests and retain the safe gap review.
