# Proposal Pack Authoring

Read this for proposal, RFP, and capture packs.

## Approved Inputs

Use supplied or approved RFP files, amendments, Q&A, instructions, evaluation criteria, requirements, capture notes, proof libraries, and sanitized examples. Do not default to procurement-portal scraping or unapproved customer material.

Map opportunity facts and requirement snippets to `source-signals`, must-answer requirements to `needs-requirements`, bid/no-bid and evaluation gates to `decision-criteria`, approved proof to `evidence-proof`, and missing proof or context to `gaps`.

`normalize-opportunity` still emits `normalized_prospect` because the CLI compatibility contract ingests prospect-shaped normalization output. Proposal packs may also include `normalized_opportunity` as an exact alias for readability, but opportunity-specific fields belong in signals, attributes, trace, gaps, requirements, and proof—not in a new core object.

When supplied PDFs/docs are extracted before `normalize-opportunity`, preserve a bounded `mdp.source-audit.v0` ledger rather than raw source documents in the pack. The ledger's refs should cite `.mdp/sources.yaml` source IDs and short snippets; run `validate-prompt-output --source-audit` so nonexistent raw refs or snippet mismatches block review.

## Closed Job Bindings

```yaml
jobs:
  - id: bid-no-bid-review
    skill_id: mdp-proposal-review
  - id: compliance-review
    skill_id: mdp-proposal-review
  - id: proof-review
    skill_id: mdp-proposal-review
  - id: red-team-review
    skill_id: mdp-proposal-review
```

Each job must declare the relevant universal primitives and `opportunity` input contract.

## Safety

Never invent or imply certifications, compliance status, security posture, references, past performance, pricing, evaluator criteria, deadlines, procurement eligibility, or approval status. Write “source material does not establish,” record the gap, and require the responsible human reviewer.

MDP supports local, customer-controlled review. It does not certify compliance, approve procurement language, submit proposals, or replace legal, security, procurement, or proposal review.
