# Proposal Reference Profile Sample

This is a synthetic proposal-review sample pack for MDP.

It demonstrates how a proposal profile can use the existing `mdp.v0` card model while keeping proposal-specific meaning in card IDs, entries, tags, sources, and route evals.

The sample is fictional. It does not represent a real customer, RFP, agency, opportunity, price, certification, or past performance claim.

## Demo Scenario

The fictional opportunity is a municipal permit modernization RFP.
The pack can support review jobs such as:

- bid/no-bid review
- compliance review
- win-theme proof review
- red-team gap review
- executive brief

Use the route and eval commands to inspect the sample:

```bash
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/proposal
cargo run --manifest-path cli/Cargo.toml -- --json eval --dir plugin/assets/templates/proposal
cargo run --manifest-path cli/Cargo.toml -- --json route --entries --dir plugin/assets/templates/proposal --persona "Proposal Lead" --job "bid no bid review"
cargo run --manifest-path cli/Cargo.toml -- --json gaps --dir plugin/assets/templates/proposal
cargo run --manifest-path cli/Cargo.toml -- --json check-claims --dir plugin/assets/templates/proposal --persona "Proposal Lead" --job "compliance review" --text "The sample team is CMMC compliant."
```

The eval fixtures cover:

- route behavior for bid/no-bid, compliance, proof, and red-team review jobs
- durable gap surfacing for missing RFP text, missing proof, and public-safety gaps
- unsupported compliance/security claims and invented proof guardrails
- insufficient-context and policy-bypass fit outcomes

## Safety Boundary

Do not replace the synthetic content with raw customer proposal material in this public repo.
Real proposal packs should live in private customer-controlled workspaces unless the source material is explicitly approved and sanitized for public use.
