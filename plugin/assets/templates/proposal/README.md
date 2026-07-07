# Proposal Reference Profile Sample

This is a synthetic proposal-review sample pack for MDP.

It demonstrates how a proposal profile can use the existing `mdp.v0` card model while keeping proposal-specific meaning in card IDs, entries, tags, sources, and route evals.

The sample is fictional. It does not represent a real customer, RFP, agency, opportunity, price, certification, or past performance claim.

The manifest includes a proposal `primitive_map`, an `opportunity` input contract, `prompts/normalize-opportunity.yaml`, profile jobs, and categorized `profile_eval` metadata. Run `mdp --json validate --dir plugin/assets/templates/proposal` to inspect `data.profile.activation_ready`; `profile.id: proposal` and `profile.agent_surface` route skills, but they are not the full activation gate.

## Demo Scenario

The fictional opportunity is a public services modernization RFP.
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
cargo run --manifest-path cli/Cargo.toml -- --json validate-prompt-output --dir plugin/assets/templates/proposal --prompt-id normalize-opportunity --file <prompt-output.json>
cargo run --manifest-path cli/Cargo.toml -- --json verify-output --dir plugin/assets/templates/proposal --file plugin/assets/templates/proposal/examples/proof-output/valid-binding.json
cargo run --manifest-path cli/Cargo.toml -- verify-output --readable --dir plugin/assets/templates/proposal --file plugin/assets/templates/proposal/examples/proof-output/valid-binding.json
cargo run --manifest-path cli/Cargo.toml -- --json route --entries --dir plugin/assets/templates/proposal --persona "Proposal Lead" --job "bid no bid review"
cargo run --manifest-path cli/Cargo.toml -- --json gaps --dir plugin/assets/templates/proposal
cargo run --manifest-path cli/Cargo.toml -- --json check-claims --dir plugin/assets/templates/proposal --persona "Proposal Lead" --job "compliance review" --text "The sample team is CMMC compliant."
```

The eval fixtures cover:

- prompt-output validation for `normalize-opportunity`, including insufficient context and invalid enum values
- route behavior for bid/no-bid, compliance, proof, and red-team review jobs
- durable gap surfacing for missing RFP text, missing proof, and public-safety gaps
- unsupported compliance/security claims and invented proof guardrails
- proof-output verification for valid bindings, fake IDs, missing bindings, malformed text coverage, safe gaps, connective text, and unsupported full-text claims
- insufficient-context and policy-bypass fit outcomes

`prompts/normalize-opportunity.yaml` also includes a neutral `output_contract.example` fixture. Treat that as a JSON contract example, not as the active demo scenario; when retargeting the template, update eval IDs, titles, jobs, and scenario examples together or explicitly mark examples as contract-only fixtures.

The files under `examples/proof-output/` are synthetic `mdp.proof-output.v0` artifacts. A source ID written by a model is not proof by itself; run `mdp --json verify-output --dir <pack> --file <proof-output.json>` and only treat the generated text as proof-bound when the verifier returns `valid: true`.

For human review, run `mdp verify-output --readable --dir <pack> --file <proof-output.json>`. The readable artifact is Markdown with top-of-file YAML frontmatter and proposal review sections for requirement status, proof receipts, unsupported claims, gaps, verification status, and next actions. The Markdown is a review layer only; the proof-output JSON and verifier result remain the machine source of truth.

## Safety Boundary

Do not replace the synthetic content with raw customer proposal material in this public repo.
Real proposal packs should live in private customer-controlled workspaces unless the source material is explicitly approved and sanitized for public use.
