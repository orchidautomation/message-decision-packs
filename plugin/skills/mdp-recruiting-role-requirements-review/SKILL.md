---
name: mdp-recruiting-role-requirements-review
description: Use when the user wants to review supplied role requirements against a Recruiting MDP pack for job-related rationale, source support, ambiguity, proxy risk, evidence expectations, and human decisions. Produces a role-requirements matrix without evaluating candidates or providing legal/compliance approval.
---

# MDP Recruiting Role Requirements Review

## Profile Gate

Run `mdp --json agent-surface --dir .`. Continue only when this skill is allowed and not blocked; otherwise reroute to the named Recruiting skill.

## Inputs

Require the pack directory, supplied role source, human review owner, requested review scope, source classification, and criteria. If the source or job-related rationale is missing, return gaps. Do not invent requirements or treat preference, pedigree, personality, culture fit, location, schedule, or language background as job-related without supplied rationale.

## Workflow

1. Run `mdp --json validate --strict --dir .` and `mdp --json gaps --dir .`.
2. Normalize messy role context with `normalize-recruiting-context`; validate the output before use.
3. Route with:

```bash
mdp --json --summary route --entries --dir . --persona "Hiring Manager" --job "role requirements review"
```

4. Review each criterion against `role-context`, `role-requirements`, `review-criteria`, `recruiting-boundaries`, `review-outputs`, and `gaps`.
5. Flag protected characteristics, proxy-like criteria, vague culture/personality language, unsupported credential requirements, and criteria without job-related rationale.
6. Use `mdp check-claims` for risky wording and keep failed checks out of accepted criteria.

## Output

Return rows with criterion, supplied source, job-related rationale, evidence expectation, ambiguity or proxy risk, bounded status, owner question, and human decision needed.

Allowed statuses are `source-backed`, `partial`, `gap`, `remove-or-rewrite`, and `needs-human-review`.

Do not evaluate a candidate, recommend legal sufficiency, or certify a selection procedure. End with the accountable human reviewer and unresolved gaps.
