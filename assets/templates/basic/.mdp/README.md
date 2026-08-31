# Basic MDP Template

<!-- mdp:readme-ownership v1 begin -->

## README ownership

- Machine-owned: this ownership legend and the marker-delimited Inventory block. `mdp readme refresh` may replace only those regions.
- Human-owned: every other README byte. Refresh preserves that prose without reviewing its thesis, claims, source interpretation, or gaps.
<!-- mdp:readme-ownership v1 end -->


## Authority

This README is orientation only. The manifest, referenced card entries, source ledger, contracts, and explicit gaps remain the machine authority. README prose cannot satisfy readiness or override structured authority.
The inventory block below is a machine-generated projection of loaded structured authority; refresh it with `mdp readme refresh` and never hand-maintain its counts.

## Thesis

A modular message decision pack for agent-readable ICP, pains, triggers, proof, CTA policy, avoid-rules, output rules, and copy guidance.

## Actors

- GTM Engineering
- PMM
- PM

## ICP and Fit Authority

- `cards/fit-rules.yaml`: Fit rules

## Supported Jobs

- `prospect-fit-or-brief`: Prospect row to fit decision or brief
- `outbound-copy-brief`: Outbound copy brief
- `outbound-copy-review`: Supplied outbound copy review

## Decision Flow

- Select one exact canonical job.
- Inspect its resolved product foundation and diagnostics.
- Load only the referenced cards, entries, contracts, sources, and gaps.
- Use detached prospect input only when the selected job has no direct or transitive Decision Input Contract; governed jobs require the exact normalized envelope and lineage artifacts.
- For governed GTM jobs, the host supplies researched observations matching the compiled collection specification. It does not pre-assign persona or segment and the pack never prescribes which provider or tool collected the evidence.
- The v3 normalization model proposes only closed taxonomy classifications with `derived_from` attempt IDs and a bounded basis. The host validates and seals provenance; deterministic MDP policy alone decides fit, routing, and draft eligibility.
- Treat raw prompt output as untrusted. Only a successful validation receipt bound to the exact pack, prompt, job when applicable, validator inputs, and output bytes may provide prompt-output decision-trace authority.
- Stop on blocked authority; never fill a gap from this README.
- Apply the job output and review boundaries before using the result.

## Boundaries

- `cards/channel-policies.yaml`: Channel policies
- `cards/avoid-rules.yaml`: Avoid rules
- `cards/output-rules.yaml`: Output rules

## Sources

- `mdp-reference-contract`: .mdp/cards/positioning.yaml
- `mdp-pack-manifest`: .mdp/manifest.yaml
- `example-prospect`: examples/clay-row.json

## Prompts

- `normalize-prospect-row`
- `generate-outbound-copy-v1`
- `review-outbound-copy-v1`
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
- `mdp --json readme check --dir .`
- `mdp --json readme refresh --dir .`
- `mdp --json schema prompt-output-validation-v1`

## Gaps

- Missing company-specific proof: If a prospect/account row lacks concrete source context, ask for source material or state the personalization gap before drafting.
- Unclear fit: If observed role responsibilities, company-fit evidence, or separate why-now evidence cannot support a closed classification, return the configured gap or review state instead of forcing a message.
- Hosted API not included: The MVP is local/offline. Do not imply a hosted API exists unless the user has deployed one separately.

<!-- mdp:readme-inventory v1 begin -->

## Inventory

Machine-generated from loaded structured authority. Do not edit by hand; run `mdp readme refresh` to update. This block is a projection of the manifest, cards, sources, and prompts; it cannot satisfy a product-foundation facet, close a gap, or override structured authority.

- cards: 16
- card entries: 55
- prompts: 12
- sources: 3

### Card entries

- `avoid-rules` (avoid-rules): 2 entries
- `channel-policies` (channel-policies): 7 entries
- `claims` (claims): 3 entries
- `copy-patterns` (copy-patterns): 3 entries
- `ctas` (ctas): 4 entries
- `fit-rules` (fit-rules): 3 entries
- `gaps` (gaps): 3 entries
- `hooks` (hooks): 3 entries
- `motions` (motions): 3 entries
- `objections` (objections): 2 entries
- `output-rules` (output-rules): 6 entries
- `pains` (pains): 3 entries
- `personas` (personas): 3 entries
- `portfolio-examples` (hooks): 4 entries
- `positioning` (positioning): 3 entries
- `signals` (signals): 3 entries

### Sources

- `mdp-reference-contract`
- `mdp-pack-manifest`
- `example-prospect`

### Prompts

- `extract-avoid-rules`
- `extract-claims-proof`
- `extract-cta-channel-policy`
- `extract-fit-rules`
- `extract-gaps`
- `extract-hooks`
- `extract-icp-persona`
- `extract-output-rules`
- `extract-pains`
- `generate-outbound-copy-v1`
- `normalize-prospect-row`
- `review-outbound-copy-v1`
<!-- mdp:readme-inventory v1 end -->
