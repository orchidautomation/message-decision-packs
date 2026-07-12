---
title: Prevent LFG cross-target contamination with explicit target identity
date: 2026-07-12
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: ce-plan-bootstrap
execution: code
origin: Linear MDP-104
---

# Prevent LFG cross-target contamination with explicit target identity

## Goal Capsule

- Objective: ensure every LFG/create-pack run either resolves an explicit sold target or stops before authoring, and ensure validation rejects target residue with actionable locations.
- Authority: repository `AGENTS.md`, Linear MDP-104, then this plan.
- Execution profile: characterize the current target-blind scaffold, add the contract and validation first, then align generated surfaces and agent skills.
- Stop conditions: do not invent target claims, do not copy private reference-pack content, and do not publish a release before review and merge.
- Tail ownership: open a PR with `ai:autofix-enabled`; release/install closeout remains post-merge work.

---

## Product Contract

### Summary

MDP needs a first-class distinction between the external company/product/project being positioned and MDP's internal schema, CLI, prompt, and control-plane vocabulary. A named pack is not enough: today `mdp init --name` changes only manifest identity while every other generated surface remains the MDP starter demo.

### Problem Frame

The current GTM scaffold is a complete MDP product example. `starter_manifest` receives the requested name, but cards, sources, prompts, evals, and examples are generated from target-blind functions. Validation checks shape and routing contracts but has no target identity or contamination contract, so a renamed starter passes as valid.

### Requirements

- R1. Target-aware authoring must require a resolved target name and kind (`company`, `product`, or `project`); ambiguous intent must stop before files are authored.
- R2. The manifest must carry a target identity and lexicon: external names/aliases, source-backed terms, excluded prior/starter terms, and internal control-plane terms.
- R3. Existing packs without target identity remain structurally compatible; custom named initialization without target identity must no longer imply that the name retargeted the scaffold.
- R4. Target-aware initialization must generate only target-specific or intentionally neutral positioning, personas, pains, hooks, claims, prompts, examples, traces, tags, job labels, and eval fixtures.
- R5. Missing company-specific evidence must produce explicit gaps and no approved claims, not inherited starter claims.
- R6. Validation must reject excluded terms anywhere outside the lexicon declaration and reject MDP/CLI/schema vocabulary in prospect-facing fields while allowing it in implementation contracts and negative guardrail fixtures.
- R7. Every contamination issue must include a stable code and a concrete file plus JSON-pointer-like field path.
- R8. Company A then Company B regression coverage must prove no Company A, starter-demo, private reference-target, or MDP-as-product residue.
- R9. CLI help, templates, docs, `mdp-lfg`, `mdp-create-pack`, review/eval skills, and skill evals must describe the same identity gate and retarget procedure.

### Acceptance Examples

- AE1. Given a clean directory and explicit Company A target identity, initialization succeeds and the resulting pack validates; external content names Company A or remains neutral and records evidence gaps.
- AE2. Given Company B plus `Company A` as an excluded prior term, validation reports zero residue after clean generation and reports exact paths if Company A is injected into a card, prompt example, eval persona/job, or sample row.
- AE3. Given a custom `--name` without a target identity, initialization fails before writes with a clarification-oriented error.
- AE4. Given MDP control-plane text in a card positioning body or outbound eval text, validation fails; the same vocabulary remains valid in manifest schema refs, prompt contract instructions, and an explicitly negative guardrail eval.
- AE5. Given an existing valid pack with no target section, validation remains compatible.

### Scope Boundaries

- Keep MDP as local decision context and routing contracts, not execution infrastructure.
- Do not implement source research, scraping, CRM writes, or model-driven identity inference in the CLI.
- Do not add private target copy or the local reference-pack path to public fixtures.
- Do not make unsupported evidence claims merely to produce a fuller starter.

---

## Planning Contract

### Key Technical Decisions

- KTD1. Add optional manifest `target` metadata for compatibility, but make it mandatory for target-aware LFG/create-pack authoring. The CLI accepts explicit target arguments; skills own conversational identity resolution.
- KTD2. Preserve the existing `basic` template as an intentional MDP reference/demo. Add a target-aware generation branch that produces a complete neutral skeleton rather than mechanically replacing nouns in the demo.
- KTD3. Use two contamination classes. `excluded_terms` are forbidden across generated content except their own declaration. Internal control-plane terms are forbidden only on external surfaces; implementation contract fields and negative rejection fixtures are exempt.
- KTD4. Validate parsed values and emit JSON-pointer-like locations instead of plain file grep results. This makes diagnostics deterministic and avoids matching YAML keys or namespaces that are intentionally internal.
- KTD5. Source evidence is declarative. Target metadata may cite source IDs; when none exist, starter claims remain empty and gaps explain what must be sourced.

### High-Level Design

1. Extend `Manifest` and the manifest schema with `TargetIdentity`.
2. Extend `mdp init` with target name/kind, aliases, supported external terms, and repeatable excluded terms. Reject partial/ambiguous combinations before directory creation.
3. Route explicit targets to target-aware starter builders for manifest, cards, sources, prompts, evals, and sample rows. Keep the existing basic builders for the uncustomized reference template.
4. Add contamination validation after cards/prompts/evals/examples are parseable. Walk parsed values with surface-specific policies and stable field paths.
5. Add regression tests around initialization, validation diagnostics, and Company A to Company B isolation.
6. Update checked-in starter/template artifacts and skill/docs contracts together.

### Assumptions

- Target identity is authoring metadata, not a new routing primitive.
- Existing generic packs can migrate incrementally because `target` is optional at the wire level.
- A target-aware starter may be structurally complete while commercially unready; its gaps and empty claim ledger make that distinction explicit.

---

## Implementation Units

### U1. Target identity wire and CLI gate

- **Goal:** represent target identity and prevent ambiguous named initialization.
- **Files:** `cli/src/models.rs`, `cli/src/cli.rs`, `cli/src/app.rs`, `cli/src/commands/init.rs`, `cli/src/commands/schemas.rs`.
- **Patterns:** existing serde-default compatibility, repeatable Clap arguments, and pre-write validation in `init_pack`.
- **Test scenarios:** explicit complete target; partial target flags; custom name without target; unchanged default reference init; manifest schema contract.
- **Covers:** R1, R2, R3, AE3.

### U2. Target-aware complete scaffold

- **Goal:** generate all public surfaces from one target identity without inherited product claims.
- **Files:** `cli/src/starter.rs`, `cli/src/commands/init.rs`, `plugin/assets/templates/basic/.mdp/**`, relevant CLI init tests.
- **Patterns:** existing modular starter builders and template-drift checks.
- **Test scenarios:** Company A generated content, evidence gaps, neutral synthetic example, target-aware prompt examples, required profile/eval coverage.
- **Covers:** R4, R5, AE1.

### U3. Deterministic contamination validation

- **Goal:** reject prior-target and control-plane leakage with actionable paths.
- **Files:** `cli/src/commands/health.rs`, `cli/src/models.rs`, focused validation tests.
- **Patterns:** existing `issue(code, severity, path, message)` diagnostics and parsed YAML/JSON validation.
- **Test scenarios:** old noun in every surface class; internal term in positioning/copy; internal term allowed in schema refs and negative guardrail fixture; existing target-less pack compatibility.
- **Covers:** R6, R7, R8, AE2, AE4, AE5.

### U4. Agent and public contract alignment

- **Goal:** make LFG/create-pack/review/eval behavior consume the target gate and retarget lexicon.
- **Files:** `plugin/skills/mdp-lfg/**`, `plugin/skills/mdp-create-pack/**`, `plugin/skills/mdp-pack-review/**`, `plugin/skills/mdp-pack-eval/**`, `docs/**`, `README.md` where applicable.
- **Patterns:** progressive references and public-safe skill eval fixtures.
- **Test scenarios:** trigger/output evals mention identity resolution, ambiguity stop, excluded prior terms, and control-plane separation.
- **Covers:** R9.

### U5. End-to-end regression and closeout

- **Goal:** prove Company A to Company B isolation, review the diff, and package the PR.
- **Files:** CLI tests plus `docs/orchid/qa/` only if durable evidence adds value.
- **Test scenarios:** targeted tests, full Cargo suite, template validation, `make validate`, clean temp-directory smoke runs.
- **Covers:** R8 and all acceptance examples.

---

## Verification Contract

| Gate | Command or proof | Units |
|---|---|---|
| Focused CLI tests | `cargo test --manifest-path cli/Cargo.toml target_identity` and contamination-specific tests | U1-U3 |
| Full CLI suite | `cargo test --manifest-path cli/Cargo.toml` | U1-U3, U5 |
| Basic template | `cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic` | U2-U3 |
| Full repository | `make validate` | U1-U5 |
| Company isolation | Clean Company A and Company B generation with injected-residue negative cases | U2-U3, U5 |
| Review | CE code review with no unresolved P1/P2 findings | U1-U5 |

---

## Definition of Done

- Explicit target identity is the only target-aware creation path and ambiguity stops before writes.
- Target-aware packs contain no inherited MDP product positioning, old target names, demo personas, demo scope labels, or unsupported product claims.
- Validation emits stable actionable contamination diagnostics without rejecting legitimate internal namespaces or unchanged target-less packs.
- CLI schema, starter/template, docs, skills, and evals describe the same behavior.
- All verification gates pass or any unavailable gate is reported with the exact reason.
- MDP-104 is linked from a pushed PR labeled `ai:autofix-enabled`; no release is published before merge.
