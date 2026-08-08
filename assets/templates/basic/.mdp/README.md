# Basic MDP Template


## Authority

This README is orientation only. The manifest, referenced card entries, source ledger, contracts, and explicit gaps remain the machine authority. README prose cannot satisfy readiness or override structured authority.

## Thesis

A modular message decision pack for agent-readable ICP, pains, triggers, proof, CTA policy, avoid-rules, output rules, and copy guidance.

## Actors and ICP

- GTM Engineering
- PMM
- PM

## Supported Jobs

- `prospect-fit-or-brief`: Prospect row to fit decision or brief
- `outbound-copy-brief`: Outbound copy brief
- `outbound-copy-review`: Supplied outbound copy review

## Decision Flow

- Select one exact canonical job.
- Inspect its resolved product foundation and diagnostics.
- Load only the referenced cards, entries, contracts, sources, and gaps.
- Stop on blocked authority; never fill a gap from this README.
- Apply the job output and review boundaries before using the result.

## Boundaries

- `cards/channel-policies.yaml`: Channel policies
- `cards/avoid-rules.yaml`: Avoid rules
- `cards/output-rules.yaml`: Output rules

## Sources

- `mdp-reference-contract`: .mdp/cards/positioning.yaml
- `example-prospect`: examples/clay-row.json

## Prompts

- `normalize-prospect-row`
- `extract-icp-persona`
- `extract-pains`
- `extract-hooks`
- `extract-claims-proof`
- `extract-fit-rules`
- `extract-avoid-rules`
- `extract-output-rules`
- `extract-cta-channel-policy`
- `extract-gaps`

## Commands

- `mdp --json validate --dir .`
- `mdp --json skills --job prospect-fit-or-brief --dir .`
- `mdp --json requirements --job prospect-fit-or-brief --dir .`
- `mdp --json skills --job outbound-copy-brief --dir .`
- `mdp --json requirements --job outbound-copy-brief --dir .`
- `mdp --json skills --job outbound-copy-review --dir .`
- `mdp --json requirements --job outbound-copy-review --dir .`

## Gaps

- Missing company-specific proof: If a prospect/account row lacks concrete source context, ask for source material or state the personalization gap before drafting.
- Unclear fit: If role, segment, or trigger does not map to a fit rule, return insufficient-context instead of forcing a message.
- Hosted API not included: The MVP is local/offline. Do not imply a hosted API exists unless the user has deployed one separately.
