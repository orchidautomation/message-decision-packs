---
title: MDP-243 Transactional mdp init - Implementation Plan
type: hardening
date: 2026-08-26
topic: transactional-init
execution: orchid
artifact_contract: orchid-plan/v1
artifact_readiness: implementation-ready
linear_issues:
  - MDP-243
---

# MDP-243 Transactional `mdp init` - Implementation Plan

## Context and current behavior

At planning base `5aaaf850b24b57622aca118da84cf02649380ab7`, `cli/src/app.rs::run` dispatches `Commands::Init` to `cli/src/commands/init.rs::init_pack_targeted` or `init_pack_targeted_dry_run`.

`init_gtm_pack` checks only a subset of destinations before creating directories and sequentially writing manifest, sources, cards, evals, prompts, README, and examples. The collision check for `examples/clay-row.json` or `examples/prospect-row.json` happens after most pack files are already written. `init_proposal_pack` similarly checks README, creates directories, and writes `PROPOSAL_TEMPLATE_FILES` sequentially. With `--force`, files are overwritten one by one. There is no common artifact inventory, staging validation, publication transaction, or rollback.

Canonical GTM content is produced by `cli/src/starter.rs` and `cli/src/target_starter.rs`; proposal content comes from `PROPOSAL_TEMPLATE_FILES` plus `proposal_template_contents` and `proposal_readme`. Existing tests `generated_basic_starter_matches_plugin_template` and `generated_proposal_starter_matches_plugin_template_pack_files` prove exact file-set and byte parity. `cli/src/commands/health.rs::validate_pack` is the existing staged-tree validation authority.

## Objective, scope, and assumptions

Make initialization publish one complete validated starter tree or leave the destination unchanged after any handled failure.

In scope:

- One authoritative generated-artifact inventory shared by dry-run and real initialization.
- Exhaustive collision and path-type preflight before destination mutation.
- Private same-filesystem staging, validation, publication, cleanup, and deterministic fault testing.
- Atomic directory rename when the destination root is absent.
- Rollback-protected generated-file merge when the destination root already exists and unrelated files must be preserved.
- Explicit success/failure publication state in JSON and human-facing messages.

Out of scope: starter-content changes, overwriting unrelated files, weakening target/collision checks, claiming crash atomicity for an existing-root merge, or deleting user artifacts.

Confirmed assumption: the existing-root case cannot be made an atomic directory replacement without discarding unrelated user files. It therefore receives explicit rollback-protected handled-failure semantics rather than a false atomicity claim.

## Acceptance mapping

| Acceptance criterion | Implementation | Validation |
|---|---|---|
| Any collision leaves the destination byte-for-byte unchanged | Build the complete inventory and preflight every file/directory with `symlink_metadata` before staging/publication. | Early manifest/README, late prospect-row, decision-scenario, proposal-late, dangling-symlink, and non-regular collision snapshots. |
| Success matches the canonical starter | Keep current generators and embedded proposal bytes; stage those exact bytes. | Existing basic/proposal golden tests plus staged validation. |
| `--force` cannot leave a mixed tree after handled failure | Snapshot eligible generated destinations, move them to transaction-owned backups, publish staged files, and rollback in reverse on error. | Inject a failure after at least one replacement; compare the complete pre/post snapshot and prove staging/backups are removed. |
| Early/late collision, staged validation, cleanup, and success are covered | Add deterministic fault seams under tests and a process-level integration matrix. | `commands::init::tests` plus `cli/tests/init_transactional.rs`. |
| JSON and human output state whether publication occurred | Return `publication.status`, `mode`, `atomic`, and bounded fallback semantics on success/dry-run; prefix handled errors with an exact `init not published` or `publication indeterminate` statement. Existing JSON/human error paths carry that message without requiring presentation-owned file changes. | Process tests assert JSON envelope fields/messages, human text, stderr/stdout, and nonzero exit. |

## Affected files and symbols

- `cli/src/commands/init.rs`
  - Refactor `init_gtm_pack`, `init_proposal_pack`, `init_gtm_pack_dry_run`, and `init_proposal_pack_dry_run` around one generated-tree inventory.
  - Preserve `resolve_target_identity`, `validate_target_destination`, canonical generators, and proposal name rewriting.
  - Add private staging/transaction types with exact ownership and drop cleanup.
  - Extend `init_payload` with publication evidence.
- `cli/src/pack_io.rs`
  - Reuse or add narrow byte-render/write helpers only when needed by the generated-tree representation; do not change unrelated pack-write behavior.
- `cli/tests/init_transactional.rs` (new)
  - Black-box success, collision, output, and cleanup coverage using `CARGO_BIN_EXE_mdp` and test-owned temporary roots.

Forbidden without replanning: `cli/src/cli.rs`, `cli/src/app.rs`, `cli/src/main.rs`, `cli/src/output.rs`, `cli/src/commands/capabilities.rs`, `cli/src/starter.rs`, `cli/src/target_starter.rs`, and `plugin/assets/templates/**`.

## Ordered implementation steps

1. Introduce a generated-file model containing repository-relative destination, rendered bytes, content kind, and overwrite eligibility. Make both GTM and proposal builders return the full model before any destination write.
2. Derive dry-run `write_plan` from that model so dry-run and apply cannot drift. Preserve `validate_target_destination` as the first target-authority gate.
3. Preflight every generated path and required directory with `symlink_metadata`. Without `--force`, any occupied generated file blocks. With `--force`, accept only explicit eligible regular generated files; reject symlinks and non-regular nodes. Capture an identity/snapshot used for a pre-publication recheck.
4. Allocate a nonce-named MDP staging directory beneath the destination parent so rename stays on one filesystem. Render the entire starter there without touching the destination.
5. Run `validate_pack` against the staged root and require its existing validity gate. A failed stage is removed and reported as not published.
6. If the destination root is absent, rename the staged root into place and report `atomic-directory-rename`, `atomic: true`, `status: published`.
7. If the root exists, preserve unrelated files and publish only generated paths through a transaction-owned backup journal. Recheck preflight, move existing eligible generated files to backups, move staged files into place, then finalize. On handled failure, remove only transaction-created paths and restore backups in reverse. Report `rollback-protected-merge`, `atomic: false`; if rollback itself fails, report `indeterminate` and exact recovery locations without broad cleanup.
8. Ensure cleanup targets only the transaction's nonce-named staging/backup paths. Add test-only fault points for staged-validation failure and mid-publication failure.
9. Preserve existing error classification strings such as `already exists; pass --force` while adding bounded publication-state language.

## Tests and validation

Focused commands:

```bash
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo test --manifest-path cli/Cargo.toml commands::init::tests
cargo test --manifest-path cli/Cargo.toml --test init_transactional
cargo test --manifest-path cli/Cargo.toml generated_basic_starter_matches_plugin_template
cargo test --manifest-path cli/Cargo.toml generated_proposal_starter_matches_plugin_template_pack_files
```

Exact-head regression gate:

```bash
cargo test --manifest-path cli/Cargo.toml
```

Manual synthetic proof: initialize into an absent temporary root and an existing temporary root containing an unrelated sentinel, parse JSON publication evidence, validate the result with `mdp --json validate`, and confirm the sentinel remains unchanged.

## Compatibility, migration, rollout, and rollback

- Starter schemas, file names, and bytes remain unchanged.
- Existing-root success remains supported, but its non-atomic fallback is now explicit.
- `--force` continues to authorize replacement only for generated destinations; it never authorizes cross-target retargeting, symlink traversal, or unrelated-file removal.
- No migration or version bump is required by the plan. Delivery is one cumulative foundation PR; release remains separate and unauthorized.
- Code rollback is a PR revert. Runtime handled-failure rollback is the transaction journal described above.

## Risks and safety boundaries

- Rename is atomic only on one filesystem and only for the absent-root publication path.
- A process crash during existing-root merge cannot be claimed atomic; keep recoverable backups and report the limitation.
- TOCTOU with concurrent writers requires the pre-publication snapshot recheck; any mismatch fails before replacement.
- Dangling symlinks count as collisions.
- Never use recursive deletion on a broad root or a path not created by the current transaction.

## Blockers and readiness verdict

No issue dependency or unresolved product decision blocks implementation. Repository authority, generator sources, validator, tests, output boundary, and rollback semantics are identified.

**Verdict: `READY_TO_PIN`.**
