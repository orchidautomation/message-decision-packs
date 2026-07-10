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

## Safety and boundary review

- Domain leakage: Recruiting assets contain no GTM or Proposal vocabulary except the documented `normalized_prospect` compatibility bridge and negative skill-routing fixtures.
- Candidate authority: no output contract includes a total, rank, comparison, shortlist, recommendation, advance, reject, hire, or final-outcome field.
- Protected traits and proxies: prompts/cards/skills block protected traits and named proxies; claim checks reject age, school prestige, commute, culture fit, sexual orientation, gender identity, facial analysis, and voice analysis cases.
- Evidence integrity: invented credentials and fake IDs fail; missing, weak, conflicting, restricted, or unverified evidence remains a gap or refusal.
- Privacy: public assets use `Candidate A`, synthetic IDs, and example domains only. No real candidate, customer, transcript, resume, contact detail, restricted source, token, or local auth material is present.
- Product boundary: docs and skills consistently describe local decision context and exclude ATS, sourcing, enrichment, scraping, background checks, scheduling, ranking/rejection/hiring, external writes, legal review, and compliance certification.
- Human checkpoint: every real employment decision and candidate-facing action remains outside MDP and requires accountable human review.

## Validation receipts

- `cargo fmt --manifest-path cli/Cargo.toml -- --check`: pass.
- `cargo test --manifest-path cli/Cargo.toml`: 183 passed, 0 failed.
- Recruiting strict validate: valid, activation ready, 0 warnings/errors, all ten primitives.
- Recruiting strict eval: 22 fixtures passed.
- Generated Recruiting init: exact golden-tree parity; strict validate/eval smoke passed.
- GTM strict validate/eval: activation ready, 0 warnings, 31 fixtures passed.
- Proposal strict validate/eval: activation ready, 0 warnings, 25 fixtures passed.
- Recruiting agent surface: six recommended skills, eleven allowed skills, thirteen blocked conflicts, six job mappings.
- Protected-trait manual check: invalid/`needs-revision`; hits `sexual orientation`, `gender identity`, and `rank candidates`.
- Root/plugin asset and skill diffs: clean.
- Skill validation: all root and plugin skills passed; Recruiting builder trigger/output evals passed in the four-skill harness.
- Plugin validation, Pluxx hook fixtures, shell syntax, installer fixtures, llms checks, and `make validate`: pass.

## Residual risks and human merge checkpoint

- The core prompt-output schema still uses `normalized_prospect` and `ready_for_mdp_fit`; this slice treats them as a compatibility bridge and binds them to review-context sufficiency, not candidate fit. A future first-class candidate schema would require a separate product decision.
- Deterministic fixtures prove declared unsafe strings and schema failures, not legal sufficiency, absence of adverse impact, or safe operation in every jurisdiction or real hiring workflow.
- Real candidate data remains user-controlled and outside public artifacts; source permission and employment decisions remain human responsibilities.

Before merge, Brandon or another accountable human reviewer must confirm the employment-domain boundary, synthetic-only public artifacts, fail-closed fixtures, and absence of candidate outcome authority. MDP-101 release/install closeout remains blocked until the PR is merged.
