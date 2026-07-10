---
name: mdp-recruiting-pack-builder
description: Use when the user wants to build or improve a Recruiting Message Decision Pack for local role-requirements, candidate-evidence, interview-brief, scorecard-gap, or pack-validation workflows from synthetic, sanitized, or approved local source material. Produces a validated review-context pack or an explicit missing-information list; never sources, ranks, rejects, advances, or hires candidates.
---

# MDP Recruiting Pack Builder


Build or improve a Recruiting reference-profile pack from approved source material. MDP prepares validated local decision context for human review. It is not an ATS, job board, sourcing or enrichment provider, scraper, background-check service, employee database, scheduler, ranker, rejection engine, hiring decision maker, legal reviewer, or compliance certifier.

## Profile Gate

Run `mdp --json agent-surface --dir .`. Use this skill only for a new legacy/generic pack or when `profile.id` is `recruiting` and this skill is allowed. If blocked, stop and reroute to a recommended or allowed skill.

Full activation comes from `mdp --json validate --strict --dir .`. Preserve the ten universal primitives, fixed card kinds, `recruiting-context` input contract, six jobs, and categorized eval coverage.

## Source Gate

Classify every input as `synthetic-example`, `sanitized-example`, `user-approved-local`, `public-supplied`, `restricted-local`, or `unverified`.

Commit only synthetic or explicitly sanitized content. Do not collect, browse for, scrape, enrich, background-check, or retrieve candidate data. If permitted use is unclear, return a missing-information list.

Never infer protected characteristics or non-job-related proxies. Never invent credentials, education, employment history, skills, achievements, identity, or source status.


## Workflow

1. Check `command -v mdp`, `mdp --json capabilities`, `mdp --json doctor --dir .`, and the agent surface.

2. Initialize when needed:

```bash
mdp --json init --template recruiting --dir . --dry-run
mdp --json init --template recruiting --dir .
```

3. Write `.mdp/sources.yaml` first. Record source kind, locator or local note ID, freshness, confidence, direct claims, interpretations, gaps, and approved handling.

4. Normalize supplied context through `.mdp/prompts/normalize-recruiting-context.yaml`. Pass role/review facts in `raw_recruiting_context`, explicit candidate subject fields in `person_data`, pack vocabulary in `existing_pack_context`, and the reviewed `source_kind`.

```bash
mdp --json validate-prompt-output --dir . --prompt-id normalize-recruiting-context --file <prompt-output.json>
```

`normalized_prospect` is a compatibility bridge: the candidate is the evidence subject and `persona` is the human operator. `human-review-ready` and `ready_for_mdp_fit` mean only that enough permitted context exists for the requested artifact.


5. Map reviewed material into:

- `recruiting-roles` for operators and the distinct candidate subject
- `role-context` and `candidate-evidence` for supplied facts
- `role-requirements` and `review-criteria` for job-related criteria
- `evidence-standards` for proof and no-invention rules
- `recruiting-boundaries` for protected/proxy, source, privacy, execution, and authority limits
- `recruiting-output-rules` and `review-outputs` for bounded labels and structures
- `review-gates` for the six jobs
- `gaps` for missing, conflicting, weak, restricted, unverified, or unsupported evidence
- `.mdp/evals/*.yaml` for route, refusal, unsafe-output, prompt-output, gap, and proof coverage

6. Validate:

```bash
mdp --json validate --strict --dir .
mdp --json gaps --dir .
mdp --json eval --strict --dir .
mdp --json agent-surface --dir .
```

7. Test the role requirements, candidate evidence, interview brief, and scorecard gap routes.

8. Run `mdp check-claims` on risky text. For evidence-carrying text that cites card or source IDs, require `mdp.proof-output.v0` and run `mdp verify-output`. Missing proof stays a gap.


## Safety Outcomes

Refuse requests to:

- rank, compare, score, recommend, advance, reject, or hire candidates
- use protected characteristics or non-job-related proxies
- invent candidate facts, credentials, evidence, or source status
- browse, scrape, source, enrich, background-check, schedule, contact, or write to an ATS
- publish restricted or identifying candidate material
- claim legal sufficiency, bias-free behavior, validated selection procedures, or compliance certification

Use only `Source-backed`, `Partial evidence`, `Gap`, `Not assessed`, and `Needs human review`. No evidence is not a negative candidate finding.

## Response

End with the pack path, source classifications, files changed, missing-information list or review packet, strict validation/eval receipts, routes tested, claim/proof checks, human-review items, and confirmation that no employment outcome was produced.
