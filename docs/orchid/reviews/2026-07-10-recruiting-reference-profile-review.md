# Recruiting Reference Profile Independent Review

## Scope and method

This is the required CE code/doc self-review for the MDP Recruiting reference-profile vertical slice on `codex/mdp-recruiting-reference-profile`.
The review used the CE code-review, doc-review, shipping, and findings-followup checklists inline because this work is explicitly single-owner and may not be delegated.

Review base: `origin/main` at `ab8b3f50f3ab1577dac3339880208565d5c192be`.

Review lenses:

- correctness and regression behavior;
- employment-domain safety and public-data handling;
- API/CLI and template compatibility;
- testing and adversarial failure modes;
- maintainability and stale-fixture risk;
- agent-native skill/routing coverage;
- documentation, product-boundary, and domain-leakage consistency;
- canonical/bundled and root/plugin parity.

## Findings and fixes

### Fixed during review

1. **P2 — Proposal post-init commands changed unintentionally.** The shared profile init payload initially gave Proposal the new Recruiting-oriented strict/agent-surface command set. Proposal now retains its established five commands; Recruiting receives its own six strict/safety commands. Golden tests assert both command contracts.
2. **P2 — CLI init module became too large.** Embedded Proposal and Recruiting file registries made `init.rs` exceed the CE maintainability threshold. They now live in `cli/src/commands/init_templates.rs`; generation behavior remains covered by exact golden-tree tests.
3. **P2 — Protected-trait examples were incomplete.** The prompt, boundary card, and interview skill named sex but did not explicitly name sexual orientation and gender identity. Both are now explicit blocked traits, and a manual claim check proves those terms plus ranking are rejected.
4. **P3 — Create-pack job claimed all ten primitives but listed nine.** The job now includes `evals`; strict validation reports no missing primitive coverage.
5. **P2 — Recruiting normalization leaked GTM prospect/fit semantics.** Added the backward-compatible `context-normalization` family with `normalized_context`, `ready_for_review`, and Recruiting-owned value contracts; existing GTM and Proposal paths remain unchanged.
6. **P2 — Source completeness, identity minimization, and reviewer handoff were implicit.** Added exact present/empty/missing expected-source coverage, opaque identity defaults, and a required human-review handoff with owner and safe next action.
7. **P2 — Handoff text could encode an autonomous candidate outcome.** Prompt-output validation now rejects candidate hire/reject/rank/advance/shortlist/recommend/score actions. Token-aware matching avoids falsely rejecting safe `candidate scorecard` review language; focused unit tests cover both sides.

### Actionable residuals

None. No unresolved P1 or P2 findings remain.

## Requirements and acceptance trace

| Contract | Evidence |
|---|---|
| R1-R5 | Recruiting-owned manifest/cards cover all ten primitives; operator personas and candidate subject are distinct; one bounded input contract and six jobs exist; readiness/fit language is explicitly limited to artifact-context sufficiency. |
| R6-R12 | Public fixtures are synthetic; boundary cards, prompt instructions, source classifications, skills, and negative evals prohibit protected/proxy use, invention, restricted-source misuse, autonomous outcomes, and legal/compliance claims. |
| R13-R18 | Role, evidence, interview, and scorecard skills/cards define criterion-level outputs, bounded evidence labels, proof bindings, gaps, and accountable human checkpoints. |
| R19-R20 | Five Recruiting skills gate on agent-surface behavior; the manifest recommends them and blocks conflicting GTM, Proposal, sourcing, and execution skills. |
| R21 | Strict validation/evals cover structure, six routes, gaps, refusal, unsafe outputs, prompt outputs, and proof outputs. |
| R22 | Final strict GTM and Proposal validation/evals pass with zero warnings. |
| R23 | Canonical/bundled assets and root/plugin skills match; CLI init golden tests, help, docs, metadata, and validation entry points agree. |
| AE1-AE6 | Degree/source gaps, proxy and ranking refusal, culture-fit ambiguity, criterion-level labels, fake proof IDs, and restricted/public-source handling all have executable fixtures or explicit skill gates. |
| R24-R27 | CLI schema/capabilities expose the additive neutral family; Recruiting fixtures enforce exact source coverage, opaque identity, and accountable reviewer handoff while GTM remains backward compatible. |
| AE7-AE10 | GTM/Recruiting contract separation, incomplete coverage, opaque identity leak, missing owner, and autonomous handoff all have executable regression fixtures. |

## Safety and boundary review

- Domain leakage: Recruiting assets use the profile-neutral `normalized_context` contract and contain no GTM or Proposal vocabulary except negative skill-routing fixtures.
- Candidate authority: no output contract includes a dedicated total, rank, comparison, shortlist, recommendation, advance, reject, hire, or final-outcome field; the free-text next action is checked for candidate-outcome instructions.
- Protected traits and proxies: prompts/cards/skills block protected traits and named proxies; claim checks reject age, school prestige, commute, culture fit, sexual orientation, gender identity, facial analysis, and voice analysis cases.
- Evidence integrity: invented credentials and fake IDs fail; missing, weak, conflicting, restricted, or unverified evidence remains a gap or refusal.
- Privacy: public assets use `Candidate A`, synthetic IDs, and example domains only. Real/local prompt guidance defaults to opaque subject IDs and redacted job-related evidence. No real candidate, customer, transcript, resume, contact detail, restricted source, token, or local auth material is present.
- Product boundary: docs and skills consistently describe local decision context and exclude ATS, sourcing, enrichment, scraping, background checks, scheduling, ranking/rejection/hiring, external writes, legal review, and compliance certification.
- Human checkpoint: every real employment decision and candidate-facing action remains outside MDP and requires accountable human review.

## Validation receipts

- `cargo fmt --manifest-path cli/Cargo.toml -- --check`: pass.
- `cargo test --manifest-path cli/Cargo.toml`: 185 passed, 0 failed.
- `cargo clippy --manifest-path cli/Cargo.toml --all-targets -- -D warnings`: non-gating baseline check remains noisy on warnings already present on `origin/main`; the configured `make validate` gate is clean.
- Recruiting strict validate: valid, activation ready, 0 warnings/errors, all ten primitives.
- Recruiting strict eval: 27 fixtures passed, including opaque identity, source coverage, reviewer-handoff, and autonomous-handoff validation.
- Generated Recruiting init: exact golden-tree parity; strict validate/eval smoke passed.
- GTM strict validate/eval: activation ready, 0 warnings, 31 fixtures passed.
- Proposal strict validate/eval: activation ready, 0 warnings, 25 fixtures passed.
- Recruiting agent surface: six recommended skills, eleven allowed skills, thirteen blocked conflicts, six job mappings.
- Protected-trait manual check: invalid/`needs-revision`; hits `sexual orientation`, `gender identity`, and `rank candidates`.
- Root/plugin asset and skill diffs: clean.
- Skill validation: all root and plugin skills passed; Recruiting builder trigger/output evals passed in the four-skill harness.
- Plugin validation, Pluxx hook fixtures, shell syntax, installer fixtures, llms checks, and `make validate`: pass.

## Residual risks and human merge checkpoint

- GTM and Proposal compatibility paths still use the existing prospect-normalization family. Recruiting now uses the additive profile-neutral context-normalization family, avoiding candidate-as-prospect and fit-readiness vocabulary without breaking existing packs.
- Deterministic fixtures prove declared unsafe strings and schema failures, not legal sufficiency, absence of adverse impact, or safe operation in every jurisdiction or real hiring workflow.
- Real candidate data remains user-controlled and outside public artifacts; source permission and employment decisions remain human responsibilities.

Before merge, Brandon or another accountable human reviewer must confirm the employment-domain boundary, synthetic-only public artifacts, fail-closed fixtures, and absence of candidate outcome authority. MDP-101 release/install closeout remains blocked until the PR is merged.
