# Proposal Reference Profile Sample


## Authority

This README is orientation only. The manifest, referenced card entries, source ledger, contracts, and explicit gaps remain the machine authority. README prose cannot satisfy readiness or override structured authority.

## Thesis

A synthetic proposal-review decision pack for bid/no-bid, compliance, proof, red-team, and executive review workflows.
This public sample is synthetic review support. It does not certify compliance, approve regulated-data handling, replace legal or procurement review, or authorize proposal submission.

## Actors

- Proposal Lead
- Solution Owner
- Executive Reviewer

## ICP and Fit Authority

- `cards/bid-no-bid-rules.yaml`: Bid/no-bid rules
- `cards/evaluation-criteria.yaml`: Evaluation criteria

## Supported Jobs

- `bid-no-bid-review`: Bid/no-bid review
- `compliance-review`: Compliance review
- `proof-review`: Proof or win theme review
- `red-team-review`: Red-team gap review

## Decision Flow

- Select one exact canonical job.
- Inspect its resolved product foundation and diagnostics.
- Load only the referenced cards, entries, contracts, sources, and gaps.
- Use detached prospect input only when the selected job has no direct or transitive Decision Input Contract; governed jobs require the exact normalized envelope and lineage artifacts.
- Treat raw prompt output as untrusted. Only a successful validation receipt bound to the exact pack, prompt, job when applicable, validator inputs, and output bytes may provide prompt-output decision-trace authority.
- Stop on blocked authority; never fill a gap from this README.
- Apply the job output and review boundaries before using the result.

## Boundaries

- `cards/compliance-boundaries.yaml`: Compliance boundaries
- `cards/proposal-boundaries.yaml`: Proposal public boundaries
- `cards/proposal-output-rules.yaml`: Proposal output rules

## Sources

- `synthetic-rfp-summary`: .mdp/cards/opportunity-context.yaml
- `synthetic-proof-inventory`: .mdp/cards/proof-library.yaml
- `proposal-safety-boundary`: .mdp/cards/proposal-boundaries.yaml

## Prompts

- `normalize-opportunity`
- `review-bid-no-bid-v1`
- `review-proposal-compliance-v1`
- `review-proposal-proof-v1`
- `review-proposal-red-team-v1`

## Commands

- `mdp --json validate --dir .`
- `mdp --json skills --job bid-no-bid-review --dir .`
- `mdp --json requirements --job bid-no-bid-review --dir .`
- `mdp --json skills --job compliance-review --dir .`
- `mdp --json requirements --job compliance-review --dir .`
- `mdp --json skills --job proof-review --dir .`
- `mdp --json requirements --job proof-review --dir .`
- `mdp --json skills --job red-team-review --dir .`
- `mdp --json requirements --job red-team-review --dir .`
- `mdp --json schema prompt-output-validation-v1`

## Gaps

- Missing RFP text: If the actual RFP text or requirement source is missing, do not infer mandatory requirements. Ask for approved source material or keep the requirement as a gap.
- Missing proof: If proof, certification, reference, or past performance is not approved, mark a proof gap and avoid claim-bearing language.
- Public safety gap: If a source appears private, customer-specific, access-controlled, or regulated, keep it out of the public template and move the work to private scratch or a customer-controlled pack.
