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
```

## Safety Boundary

Do not replace the synthetic content with raw customer proposal material in this public repo.
Real proposal packs should live in private customer-controlled workspaces unless the source material is explicitly approved and sanitized for public use.
