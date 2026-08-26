---
title: MDP-241 Productization Wave 1 - Commit-to-Build Plan
type: execution-index
date: 2026-08-26
topic: mdp-productization-wave-1
execution: orchid
artifact_contract: orchid-plan/v1
artifact_readiness: implementation-ready
linear_issues:
  - MDP-241
  - MDP-243
  - MDP-245
source_note: Public-safe planning evidence; no customer data, credentials, transcripts, or generated execution state belongs in this branch.
---

# MDP-241 Productization Wave 1 - Commit-to-Build Plan

## Context and current behavior

The productization project has two unblocked foundation defects that can be implemented independently from the refreshed `origin/main` baseline `5aaaf850b24b57622aca118da84cf02649380ab7`:

- MDP-243: `mdp init` writes a starter tree sequentially, so a late collision or handled failure can leave a partial pack.
- MDP-245: global `--json` can be bypassed by human presentation modes, most visibly `verify-output --readable`, and capabilities do not describe the conflict contract.

Both issues affect first-contact trust and agent automation. They are the first executable wave because neither has an unresolved upstream product dependency. MDP-245 remains the contract owner for presentation-mode behavior consumed later by MDP-247.

## Objective and scope

Deliver two independently reviewable implementation lanes from one pinned planning snapshot:

1. Make initialization all-or-no-change for handled failures while preserving canonical starter bytes and unrelated user files.
2. Make every `--json` invocation produce exactly one parseable JSON value, with explicit conflict behavior and capability metadata.

Out of scope: changing starter content, redesigning human output generally, implementing MDP-247, releasing the CLI, enabling PR autofix, merging, or deploying.

## Execution topology and ownership

| Issue | Implementation branch | Owned paths | Forbidden paths | Dependency |
|---|---|---|---|---|
| MDP-243 | `codex/mdp-243-transactional-init` | `cli/src/commands/init.rs`, `cli/src/pack_io.rs`, `cli/tests/init_transactional.rs` | presentation-mode files owned by MDP-245; starter/template content | none |
| MDP-245 | `codex/mdp-245-json-output-invariant` | `cli/src/cli.rs`, `cli/src/app.rs`, `cli/src/main.rs`, `cli/src/output.rs`, `cli/src/commands/capabilities.rs`, `cli/tests/json_stdout_contract.rs` | initializer implementation and starter/template content | none |

The lanes may execute in parallel because ownership does not overlap. Integration produces one cumulative PR for this cohesive same-repository foundation wave. If implementation proves a cross-lane edit is unavoidable, stop and reassign ownership before writing rather than creating competing changes.

## Acceptance and validation map

| Issue | Acceptance authority | Focused proof | Broad proof |
|---|---|---|---|
| MDP-243 | The destination is unchanged on every handled failure; successful bytes match canonical starters; `--force` cannot leave a mixed tree; output reports publication state. | initializer collision/fault/rollback integration matrix and existing basic/proposal golden tests | full Rust suite at exact integrated head |
| MDP-245 | Every `--json` invocation emits exactly one JSON value; non-JSON presentation conflicts are stable; capabilities and tests share one exact matrix. | process-level JSON stdout matrix plus CLI/output/capabilities unit tests | full Rust suite and representative `jq -e` smoke |

## Integration, rollout, and rollback

- Rebase both implementation lanes from this exact planning commit.
- Integrate only after each focused suite passes and ownership remains clean.
- Run the full Rust suite once on the exact cumulative head.
- Open one PR; do not merge, release, or deploy automatically.
- Rollback is a normal PR revert because this wave has no data migration or persistent schema migration. Runtime filesystem rollback behavior for MDP-243 is specified in its child plan.

## Risks and safety boundaries

- Transactional filesystem claims must distinguish atomic same-filesystem rename from rollback-protected merge and must never delete broad or unrelated paths.
- JSON-mode changes must not leak diagnostics to stderr or silently ignore requested presentation formats.
- Public planning and tests must remain synthetic. Do not commit generated run state or local-only paths.
- `ai:autofix-enabled` is not authorized for the eventual PR.

## Blockers and readiness verdict

No repository, product, or issue blocker prevents implementation. Each child has exact ownership, acceptance, tests, compatibility behavior, and rollback boundaries.

**Verdict: `READY_TO_PIN`.**
