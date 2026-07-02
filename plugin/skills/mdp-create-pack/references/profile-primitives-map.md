# Profile Primitive Mapping

Use this when deciding where domain vocabulary belongs in an MDP pack.

## Rule

Do not create new core system objects for every domain noun. Map domain nouns to universal primitives, then use profile-owned card IDs, jobs, input contracts, and eval categories to make the domain feel native.

## Universal Primitive Matrix

| Primitive | Purpose | GTM profile examples | Proposal profile examples |
| --- | --- | --- | --- |
| `actors` | Who matters | personas, buyers, users | proposal roles, evaluators, reviewers |
| `decision-criteria` | When to proceed, pause, decline, route | fit rules, disqualifiers, segment rules | bid/no-bid rules, evaluation criteria |
| `source-signals` | Observable facts from inputs | account signals, triggers, row fields | opportunity context, requirement signals |
| `needs-requirements` | Problems, requirements, stakes | pains, jobs to be done | requirements matrix |
| `evidence-proof` | Claims that need proof | claims, proof points | proof library, differentiators |
| `boundaries` | Things not to say or do | avoid rules, no-message cases | compliance boundaries, no-invention rules |
| `output-contracts` | What the downstream model may produce | output rules, copy patterns, CTAs | review outputs, proposal output rules |
| `routing-jobs` | Which task loads which context | motions, channel policies | review gates |
| `gaps` | Unknowns that must stay visible | missing ICP proof | missing RFP/proof/compliance context |
| `evals` | Deterministic checks | route, fit, prompt-output fixtures | review route and safety fixtures |

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
