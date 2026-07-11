# Profile Primitive Mapping

Use this when deciding where domain vocabulary belongs in an MDP pack.

## Rule

Do not create new core system objects for every domain noun. Map domain nouns to universal primitives, then use profile-owned card IDs, jobs, input contracts, and eval categories to make the domain feel native.

## Universal Primitive Matrix

| Primitive | Purpose | GTM examples | Proposal examples | Recruiting examples |
| --- | --- | --- | --- | --- |
| `actors` | Who matters | personas, buyers, users | proposal roles, evaluators, reviewers | Recruiter, Hiring Manager, Interviewer; candidate subject |
| `decision-criteria` | When to proceed, pause, decline, route | fit rules, disqualifiers | bid/no-bid and evaluation criteria | job-related criterion review rules |
| `source-signals` | Observable facts from inputs | account signals and row fields | opportunity and requirement signals | role context and supplied candidate evidence |
| `needs-requirements` | Problems, requirements, stakes | pains and jobs to be done | requirements matrix | role requirements and rationale |
| `evidence-proof` | Claims that need proof | claims and proof points | proof library and differentiators | evidence standards and source bindings |
| `boundaries` | Things not to say or do | avoid rules and no-message cases | compliance and no-invention rules | protected/proxy, source, privacy, and no-outcome rules |
| `output-contracts` | What the downstream model may produce | output rules, copy patterns, CTAs | proposal review outputs | evidence matrices, interview briefs, and gap reports |
| `routing-jobs` | Which task loads which context | motions and channel policies | proposal review gates | role, candidate, interview, scorecard, and validation gates |
| `gaps` | Unknowns that must stay visible | missing ICP proof | missing RFP/proof/compliance context | missing, weak, conflicting, restricted, or unverified evidence |
| `evals` | Deterministic checks | route, fit, prompt-output fixtures | review route and safety fixtures | route, refusal, prompt, gap, safety, and proof fixtures |

## GTM Account Context

Company-level ICP usually maps across:

- `source-signals`: headcount, revenue band, hiring, tool stack, operational triggers.
- `decision-criteria`: fit/disqualifier rules for account qualification.
- `actors`: person/persona readiness once a real role is supplied.
- `gaps`: missing account or person data.
- `evals`: account-context-present, account-context-missing, account-only-no-draft, prompt-output-validation.

Do not add a new core object named account context unless the CLI contract explicitly changes. A profile-specific card ID can exist as vocabulary, but it should still map to a universal primitive.

## Proposal Opportunity Context

Proposal opportunity context usually maps across:

- `source-signals`: RFP facts, requirement snippets, due dates, vehicles, incumbent clues.
- `decision-criteria`: bid/no-bid gates, evaluation criteria.
- `evidence-proof`: proof library and approved differentiators.
- `boundaries`: compliance and no-invention limits.
- `routing-jobs`: review gates.
- `gaps`: missing RFP sections, proof, owners, or approval status.

## Safety Check

Before editing, answer:

1. Which universal primitive owns this concept?
2. Which profile-owned card ID makes it readable?
3. Which prompt contract normalizes it?
4. Which CLI command validates it?
5. Which eval proves it?
