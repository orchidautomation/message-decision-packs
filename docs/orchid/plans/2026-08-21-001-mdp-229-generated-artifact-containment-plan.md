---
title: MDP-229 Generated Clean-Run Artifact Containment - Implementation Plan
type: bug
date: 2026-08-21
topic: generated-artifact-containment
execution: code
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-229
linear_issues:
  - MDP-229
  - MDP-239
  - MDP-182
  - MDP-130
  - MDP-140
source_note: Public-safe plan using temporary synthetic fixtures only; no generated run output, private source data, or credentials belongs in the repository.
---

# MDP-229 Generated Clean-Run Artifact Containment - Implementation Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Keep clean-run control-plane artifacts outside the active pack and make target-contamination validation scan only authored, prospect-facing surfaces. |
| Primary authority | The Rust CLI remains authoritative for path policy, preflight/no-draft state, pack validation, and diagnostics. The stdio MCP adapter performs an early duplicate path check and returns the CLI result unchanged when it invokes the CLI. |
| Boundary decision | Reject every `output_dir` that resolves to the active pack root or any descendant. This ticket introduces no in-pack exception. The existing `.mdp/briefs` and `.mdp/traces` digest/staging exclusions remain compatibility behavior, not a supported clean-run output location. |
| Validation decision | Keep authored cards, prompts, eval fixtures, manifest/source fields, and prospect-facing examples in the contamination scan. Exclude known/generated evidence trees from content scanning and report one concise placement diagnostic per detected generated run root. |
| Compatibility | `mdp.run-request.v1`, receipt schemas, portable pack digest semantics, and external scratch output remain unchanged. Existing in-pack artifacts are never deleted; validation reports a remediation diagnostic until an operator moves them. |
| Product boundary | MDP owns deterministic contracts, path safety, validation, and receipts. It does not clean up user files, collect source data, invoke a provider as part of validation, or turn generated evidence into authored pack authority. |
| Stop condition | CLI and MCP reject unsafe output roots before writing/spawning; authored-surface target checks remain strict; legacy generated roots produce one actionable diagnostic; focused and full validation pass. |

## Context and Problem

MDP already treats `.mdp/briefs` and `.mdp/traces` as generated directories when calculating the portable pack digest and staging a run (`cli/src/artifact_hash.rs::collect_regular_files`, `cli/src/run_runtime.rs::validate_pack_directory_bounds` and `copy_pack_directory`). The target validator does not share that boundary: `cli/src/commands/health.rs::validate_target_identity` calls `target_scan_files`, which recursively walks all supported files under `.mdp/` and `examples/`.

The clean-run CLI currently accepts an output path anywhere as long as the final directory is new. The MCP adapter checks that the output leaf is new and that its immediate parent is a non-symlink directory, but it does not compare the output root with `pack_dir`. A run written under a pack can therefore leave `run-bundle.json`, `run-receipt.json`, audit, trace, or other generated evidence in the very tree later treated as prospect-facing authoring context. The validator then emits many ordinary target-contamination findings for control-plane terms instead of identifying the unsafe output-root relationship.

The implementation must make the relationship explicit, preserve real authored-example coverage, and keep cleanup out of the validator. Synthetic temporary directories are sufficient for all regression coverage; no generated run fixture should be committed under a pack.

## Product Contract

### Requirements

- **R1. Canonical output containment:** Before creating a parent directory, claim file, transaction directory, or final run directory, the CLI must resolve the active pack and requested output path and reject an output root equal to or beneath the pack. Lexical `..` paths and symlink/canonical aliases must not bypass the check.
- **R2. MCP parity:** `mdp_run` must reject the same unsafe relationship before spawning a child. It may fail as an MCP invalid-parameter response for an unsafe request, while a direct CLI invocation returns a sanitized `no-draft:preflight-refused` result with a stable reason code.
- **R3. Authored-surface scan:** Pack validation must enumerate declared/known authored surfaces rather than recursively treating every generated evidence tree as copy. It must continue to inspect manifest/source positioning fields, manifest-declared cards, top-level pack prompts/evals, and prospect-facing example files.
- **R4. Generated-root diagnostic:** A pre-existing generated clean-run root beneath a pack receives one stable, concise remediation diagnostic at the root path; descendants do not produce repeated target-contamination findings. The diagnostic must say that the output must be moved outside the pack and that validation performs no deletion.
- **R5. Target enforcement remains strict:** Excluded prior-target terms and internal control-plane vocabulary still fail on actual prospect-facing cards, prompt positioning fields, eval fixtures, and examples. The change is a scope correction, not a global contamination ignore.
- **R6. Safety matrix:** Tests cover direct descendants, nested descendants, `..`/canonical aliases, symlink ancestors/aliases, sibling directories, and a safe external scratch directory. They also cover generated-root suppression versus authored-example detection.
- **R7. No destructive migration:** Validation and preflight never remove existing user artifacts. Runtime recovery may remove only its own transaction/claim state under the existing exact-ownership rules.

### Acceptance Examples

1. A request whose `output_root` is `<pack>/generated-mcp-fixture` is refused before any output parent/claim is written; the MCP adapter refuses it before spawning the CLI.
2. `<pack>/nested/runs/new-run`, `<pack>/../pack/nested/run`, and a path reached through a symlinked pack/output ancestor are refused after canonical comparison.
3. `<pack>-scratch/new-run` and `/tmp/mdp-clean-run-*/new-run` remain valid external output roots.
4. A target pack containing a generated root with many `MDP` terms reports one `generated_artifact_inside_pack`-class diagnostic at that root, not dozens of `target_contamination_*` findings for its descendants.
5. Internal vocabulary injected into a positioning card, prompt instruction/example, eval fixture, or `examples/prospect-facing.json` still reports the existing stable contamination code and field location.
6. Existing `.mdp/briefs`/`.mdp/traces` behavior for portable digest and staging remains unchanged, while their presence is not permission to place a new clean run inside the pack.

## Planning Contract

### Key Technical Decisions

- **KTD1. One canonical path policy in Rust.** Add a helper adjacent to `execute_run_inner_with_driver` that canonicalizes the existing pack root and the requested new output path (canonicalizing the nearest existing ancestor for a not-yet-created leaf). Compare path components, not string prefixes. Run the check before `create_dir_all(parent)` and repeat it immediately before the claim/commit boundary if needed to close a symlink-swap race. Use a stable `output-directory-inside-pack` reason code mapped to the existing sanitized preflight result.
- **KTD2. Duplicate the safety check at the MCP edge.** Extend `freezeRequestFile` to retain the parsed `pack_dir` assertion without treating it as authority. After `canonicalNewOutputDir` and request freezing, compare the canonical pack `.mdp` root with the canonical output parent/leaf and reject before `invokeCli`. The Rust check remains mandatory because direct CLI callers and mutated request bytes cannot rely on the adapter.
- **KTD3. No implicit in-pack exception.** The existing generated-directory list is a digest/staging compatibility mechanism. It is not an allowlist for clean-run output. A future supported validator-excluded root would need one explicit shared contract and paired CLI/MCP tests; it is outside MDP-229.
- **KTD4. Separate placement diagnostics from contamination diagnostics.** Add an unconditional generated-artifact boundary pass in `validate_pack` before target scanning. Recognize the existing `.mdp/briefs` and `.mdp/traces` generated roots and a clean-run output root by its complete run marker pair (`run-bundle.json` plus `run-receipt.json`, with the normal run artifact layout). Collapse nested matches to the topmost root and emit one error with a stable code, remediation, and no-delete statement. Do not parse or walk descendants for target vocabulary after classification.
- **KTD5. Enumerate authored surfaces deliberately.** Refactor `target_scan_files` to accept the loaded `Manifest` or an authored-surface inventory. Include `.mdp/manifest.yaml`, `.mdp/sources.yaml`, manifest-resolved card paths, the same top-level `.mdp/prompts/*.yaml|yml` and `.mdp/evals/*.yaml|yml` files used by their validators, and `examples/**` files except classified generated roots. Do not recurse into unknown control-plane/evidence directories merely because they contain a supported extension. Do not follow directory symlinks during inventory.
- **KTD6. Preserve existing field semantics.** Keep `walk_strings`, `is_external_surface`, raw-text line handling, active-target redaction, negative guardrail exemptions, and stable `target_contamination_excluded_term` / `target_contamination_internal_vocabulary` codes. Only the file inventory and generated-root short-circuit change.
- **KTD7. Keep the public contract aligned.** Expose the new output-root requirement and stable code in capabilities/help where the run contract is described, and update operator/MCP guidance to use an external customer-controlled run directory. Do not add a new skill, provider, cleanup command, or public generated fixture.

## Implementation Units

### U1. Shared generated-root vocabulary and Rust output boundary

**Likely files and symbols**

- `cli/src/constants.rs`: move/share the generated pack directory names currently duplicated as private `GENERATED_PACK_DIRECTORIES` constants (`briefs`, `traces`) if the implementation needs one source of truth.
- `cli/src/artifact_hash.rs::collect_regular_files`: preserve the existing digest exclusion while consuming the shared constant; add/retain a regression proving generated directories do not change `pack_content_snapshot`.
- `cli/src/run_runtime.rs::execute_run_inner_with_driver`, `validate_output_leaf`, and the run transaction/claim setup around `final_dir`, `parent`, `claim_path`, and final `rename`: add canonical pack/output containment preflight before any output-side write, with a final component-aware recheck where required by the race model.
- `cli/src/run_runtime.rs` test module: add direct/nested/`..`, canonical alias, symlink, sibling, and safe external scratch cases. Assert the stable reason/terminal state and absence of output, claim, and transaction artifacts for refusal.

**Behavioral notes**

- A missing output leaf is resolved through its nearest existing, canonical ancestor; the helper must not create the ancestor merely to inspect it.
- A pack root or output ancestor that is a symlink/canonical alias is compared by its resolved path. A path equal to the pack root is unsafe just like a descendant.
- The check must not inspect or delete unrelated existing files. The existing exact run-transaction cleanup remains the only recovery cleanup.

**Covers:** R1, R6, R7; acceptance examples 1-3 and 6.

### U2. MCP preflight parity and black-box coverage

**Likely files and symbols**

- `scripts/mdp-run-mcp-server.mjs::freezeRequestFile`, `canonicalNewOutputDir`, and `callRun`: retain the parsed `pack_dir`, add a component-aware `assertOutputOutsidePack`/equivalent, and perform it before `invokeCli` while retaining the existing CLI invocation and authority pass-through.
- `scripts/test-run-mcp-server.mjs`: extend the synthetic request fixture to include a pack root where needed. Add a table-driven test for direct/nested/canonical/symlink unsafe output, a sibling/external safe output, and the assertion that unsafe cases do not create the fake child's invocation marker.
- `scripts/test-run-conformance.mjs`: add a real-CLI case near the existing output-directory reuse/preflight cases proving an in-pack output returns `no-draft:preflight-refused` with `output-directory-inside-pack` and leaves no final output/claim.

**Boundary notes**

- MCP error text must not echo private request bodies or credentials. The path relation can be reported as a bounded parameter error; the CLI remains the source of terminal authority.
- The adapter must still freeze one bounded request read and must not trust a parsed `pack_dir` to weaken the Rust check.

**Covers:** R1, R2, R6, R7; acceptance examples 1-3.

### U3. Authored-surface inventory and generated-artifact diagnostics

**Likely files and symbols**

- `cli/src/commands/health.rs::validate_pack`: call a generated-root boundary validator before `validate_target_identity`, so the placement diagnostic also applies to packs without a target identity.
- `cli/src/commands/health.rs::validate_target_identity`, `target_scan_files`, `collect_scan_files`, `display_target_scan_path`, `parse_scan_value`, and `is_external_surface`: change inventory plumbing to the explicit authored-surface set and short-circuit classified generated roots.
- `cli/src/commands/health.rs` test module around `targeted_pack`, `validate_reports_excluded_target_term_with_field_location`, `target_name_only_exempts_its_own_internal_vocabulary_occurrence`, `validate_rejects_internal_positioning_in_prompt_instructions_and_briefs`, and `generated_samples_and_readable_brief_preserve_target_isolation`: update generated-brief/trace expectations to the single placement diagnostic and move any line-level raw-copy assertions that remain necessary to a declared prospect-facing example fixture.

**Classification and diagnostic rules**

- Treat `.mdp/briefs` and `.mdp/traces` as generated roots when present with content; they are legacy generated locations, not new-run permission.
- Treat a descendant as a generated clean-run root only when the expected run marker pair/layout is present. Collapse nested marker matches to one topmost root so a run containing many files yields one issue.
- Use one stable error code (proposed: `generated_artifact_inside_pack`) with path equal to the generated root and a message such as: “generated clean-run artifacts must live outside the active pack; move this directory to an external scratch/output root; validation does not delete existing files.”
- Keep authored examples and all existing contamination field-pointer behavior covered. Do not globally ignore `examples/`, prompt outputs, negative eval fixtures, or authored traces that are not classified as generated evidence.

**Covers:** R3-R5, R7; acceptance examples 4-6.

### U4. Capabilities, operator guidance, and contract alignment

**Likely files and symbols**

- `cli/src/commands/capabilities.rs`: describe `run` as writing a new external run directory and include the stable boundary error in the machine-readable contract/error inventory; update its unit assertions.
- `plugin/skills/mdp/SKILL.md` and `plugin/skills/mdp/references/cli-operator.md`: state that `--out-dir`/`output_dir` must be a new directory outside the active pack and that generated evidence is not authored pack content.
- `docs/getting-started.md`, `docs/host-conformance.md`, and `docs/proposal-runner.md`: use explicit external scratch/customer-controlled examples and preserve the no-draft/no-cleanup boundary.

**Covers:** R1-R3, R7 and operator discoverability; acceptance examples 1, 3, and 6.

### U5. Regression and review handoff

- Keep all generated run fixtures in `std::env::temp_dir()`/`mkdtemp` and clean only test-owned temporary trees.
- Run focused Rust and Node suites, then the repository `make validate` gate. Review the diff for accidental broad scan exclusions, path leaks, cleanup of user files, and any change to receipt/hash authority.
- No version bump or migration is expected. A future in-pack supported output root, if desired, requires a separate contract decision and issue rather than an implicit exception here.

## Verification Contract

| Gate | Command or proof | Coverage |
|---|---|---|
| Rust formatting and focused boundary tests | `cargo fmt --manifest-path cli/Cargo.toml --check`; `cargo test --manifest-path cli/Cargo.toml output_directory` (plus the generated-artifact/target tests by their final names) | U1, U3 |
| Existing target validator regressions | `cargo test --manifest-path cli/Cargo.toml target_identity` and the full `health` test subset | U3, preserves R5 |
| MCP syntax and adapter suite | `node --check scripts/mdp-run-mcp-server.mjs`; `node --test scripts/test-run-mcp-server.mjs` | U2, U4 |
| Real CLI black-box containment | `cargo build --manifest-path cli/Cargo.toml`; `node scripts/test-run-conformance.mjs` | U1-U2 |
| Pack/template and installed-contract validation | `make validate` (including `validate-cli`, `validate-run-mcp`, `validate-template`, packaging, public-artifact, and installer checks) | U1-U5 |
| Static safety review | `git diff --check`; inspect that no production path calls cleanup on an existing user artifact and no diagnostics contain request bodies/secrets | U1-U5 |

If a full gate is unavailable because a local tool/dependency is missing, report the exact command and reason; do not replace it with an unverified claim.

## Dependencies, Risks, and Blocker Awareness

### Dependencies and sequencing

- MDP-182 (one clean-run entry point) is the runtime/MCP surface this plan hardens; changes landing there before implementation should be rebased into the boundary tests rather than duplicated.
- MDP-130 (managed artifact safety) and MDP-140 (workdir ownership/locks) are related safety contracts. Reuse their exact-ownership and no-destructive-cleanup semantics; neither is a reason to broaden this issue into artifact deletion or lock redesign.
- MDP-226 (canonical routed-context readiness) is a neighboring Phase 0 queue item, not a data dependency for path containment or target scan scope. Do not block MDP-229 on routed-context implementation, and do not alter MDP-226 artifacts here.
- MDP-239 remains the execution index. This plan supplies the missing implementation-ready artifact; it does not delegate the whole index or change unrelated child sequencing.
- No hard blocker is currently declared after the clean `origin/main` baseline. If the exact run contract or Linear handoff fields drift before implementation, stop and refresh the plan/branch rather than silently implementing against stale authority.

### Risks and mitigations

- **Symlink/race gap:** canonicalizing only once could allow an ancestor swap. Resolve nearest existing ancestors without creating them, recheck before claim/commit, and keep the CLI check authoritative even when MCP preflights.
- **False-positive generated classification:** authored examples may contain runner vocabulary. Require the run marker pair/layout, preserve the existing authored inventory, and add positive/negative fixtures for both classes.
- **Compatibility surprise:** packs with legacy in-pack generated roots will become visibly invalid with one error. Preserve digest/staging read behavior, never delete files, provide the exact move-outside-pack remediation, and document the behavior.
- **Over-broad ignore regression:** excluding all `.mdp` or `examples` would hide real prospect-facing contamination. Keep explicit files/roots and retain card/prompt/eval/example injection tests.
- **Adapter drift:** MCP could reject one path class while direct CLI accepts it. Keep duplicate edge checks plus black-box CLI coverage and compare canonical component paths in both implementations.

## Compatibility and Rollback

### Compatibility contract

- Existing `mdp.run-request.v1`, `mdp.run-execution.v1`, run bundle/receipt/verification schemas, pack digest, and staging contracts remain version-compatible.
- External output roots behave exactly as before. Unsafe in-pack roots now fail closed before output publication and surface a stable preflight code.
- Existing `.mdp/briefs` and `.mdp/traces` continue to be excluded from portable pack identity/staging. Their historical contents are not automatically migrated or deleted; validation reports the placement issue and operators choose when/how to move them.
- Packs without target identity retain their existing target-contamination compatibility, but the generated-root placement check is independent and can still identify an unsafe generated run root.

### Rollback

- Revert the single implementation PR/commit if a release blocker appears; no database, pack schema, or irreversible file migration is introduced.
- If an operator needs an immediate compatible path during rollout, place the run in a new external scratch/customer-controlled directory and leave the existing in-pack artifacts untouched. Do not add a temporary global ignore or cleanup command.
- Any future formal in-pack exception must be introduced as a separately reviewed contract with an explicit allowlist, validator scope, and MCP/CLI parity tests.

## Acceptance Mapping

| MDP-229 acceptance criterion | Planned implementation and proof |
|---|---|
| Clean-run CLI/MCP rejects `output_dir` inside the active pack unless a formally supported excluded location exists | U1 canonical Rust preflight rejects all in-pack roots for this ticket; U2 duplicates the check in `mdp_run`; no in-pack exception is declared. |
| Validation scans declared authored surfaces, not arbitrary generated evidence trees | U3 refactors `target_scan_files` to the manifest/source/card/prompt/eval/example inventory and short-circuits generated roots. |
| Target contamination still catches actual prospect-facing cards, prompts, evals, and examples | U3 preserves existing `is_external_surface`/`walk_strings` semantics and adds positive authored-surface fixtures for each class. |
| Unsafe output-root relationship is diagnosed before writing | U1 checks before parent/claim/transaction creation; U2 checks before child spawn; tests assert no output/claim/invocation marker. |
| Existing generated directories produce one concise remediation diagnostic | U3 detects topmost legacy/run marker roots, emits one `generated_artifact_inside_pack` error, and excludes descendants from contamination scanning. |
| Direct, nested, symlink/canonical, sibling, and safe external cases are covered | U1 Rust matrix, U2 MCP matrix, and U2 real-CLI conformance case cover each relation. |
| No cleanup command deletes existing artifacts automatically | U1/U3 tests assert existing fixtures remain; code only uses existing transaction-owned cleanup; docs state manual move/remediation. |

## Definition of Done

- Unsafe CLI and MCP output paths fail closed before writes/spawn with stable, bounded diagnostics.
- Authored target surfaces retain strict contamination enforcement and exact field paths.
- Generated evidence roots under a pack produce one actionable placement diagnostic and never become prospect-facing validation input.
- Existing hash/receipt contracts and external-run behavior remain compatible.
- Focused tests, `make validate`, and static safety review pass, or any unavailable gate is reported exactly.
- The implementation PR links MDP-229 and preserves `sync:pr-link-only`; this plan does not add automation labels, delegate the parent index, publish, merge, or delete artifacts.
