---
title: MDP-127 Verified Runner Support Matrix and Receipt Requirements - Implementation Plan
type: docs
date: 2026-07-24
topic: mdp-runner-support-matrix
execution: code
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-127
linear_issues:
  - MDP-127
  - MDP-144
source_note: Public-safe plan. It records only synthetic fixtures, public runner contracts, and locally observed tool availability; it does not contain credentials or customer proposal material.
---

# MDP-127 Verified Runner Support Matrix and Receipt Requirements - Implementation Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Make MDP's proposal-runner claims match machine-observed evidence by separating verified, recipe-only, unsupported, and fixture/mock-only modes. |
| Current truth | The repo implements and offline-tests the native OpenAI request boundary, but no committed real-provider receipt proves an audit-grade run. Codex is installed on the current VPS and exposes the required isolation flags, but no MDP wrapper or recorded end-to-end verification exists. Claude Code, Cursor, and OpenCode are not installed on this VPS. |
| Client-video decision | Until a real native/headless run produces an accepted runner audit and `mdp.run-receipt.v0`, the public video must use the existing synthetic mock flow and label it non-audit-grade. |
| First verification target | Native OpenAI Responses API is the preferred first verified path because MDP already owns the bounded request builder and runner-audit emitter. A real run remains operator-selected BYOK work and must never cause credentials or raw provider responses to enter the repo. |
| Enforcement | Documentation state alone never upgrades a runner. Audit-grade status requires machine-observed runner evidence, matching artifact hashes, and `run-receipt --require-runner-audit`. |
| Product boundary | MDP remains local/offline decision context and review support. It does not become a hosted runner, credential manager, proposal writer, compliance certifier, or model-provider abstraction. |
| Stop condition | The branch is ready when the public support matrix is explicit, CLI/schema tests keep unsupported and fixture paths blocked, proposal-facing skills use the same vocabulary, and the real native verification follow-up is linked without requiring a secret for normal validation. |

## Product Contract

### Problem Frame

The current documentation explains several native and headless recipes, while the CLI accepts six runner identifiers and can return `headless-verified` or `stateless-api-verified` from a structurally valid runner audit. That is a contract validator, not proof that every documented host has been exercised end to end.

Without a separate support-state vocabulary, a reader can incorrectly interpret “supported runner audit shape” as “verified production runner.” MDP-127 must make the distinction visible and fail closed in the client-facing workflow.

### Requirements

- R1. Publish one canonical runner support matrix using exactly four states: `verified`, `recipe-only`, `unsupported`, and `fixture/mock-only`.
- R2. A mode is `verified` only when an end-to-end invocation produced prompt output, runner audit, validation output, and an audit-grade run receipt with matching hashes.
- R3. Schema acceptance or a documented command recipe alone is `recipe-only`; it is never enough to claim a verified MDP integration.
- R4. Mock, demo, fixture, or synthetic model evidence is `fixture/mock-only` and must remain blocked from audit-grade receipts.
- R5. A runner without a maintained command/wrapper and observed proof is `unsupported`, even when `custom-headless` can represent a future host-owned adapter.
- R6. The support matrix must record required isolation properties, current repo evidence, missing proof, and the exact upgrade condition for each candidate runner.
- R7. Public client-video language must make a binary choice: show a real verified run or explicitly label the existing mock walkthrough non-audit-grade.
- R8. Normal repo validation, dry-runs, mocks, and installation must not require an API key. Real native verification is an explicit operator action using a secure local key destination.
- R9. `run-receipt` tests must prove that allowed runner identifiers still fail when runner-specific evidence is missing, inconsistent, synthetic, or mock-only.
- R10. Proposal-facing authored skills must instruct agents to consult the support state and must not equate MCP transport, recipe availability, or schema validity with verified isolation.
- R11. Existing GTM and Proposal strict validation/evals must remain green; this issue does not add GTM runner functionality.
- R12. The implementation must link a follow-up issue for the first real native verification run if that run cannot be safely completed as part of this branch.

### Current Evidence Matrix

This is the plan-time baseline to encode and then review. No row may be upgraded without new machine-observed evidence.

| Candidate | Plan-time state | Current evidence | Missing evidence / upgrade condition |
|---|---|---|---|
| `native-api` | `recipe-only` | Implemented OpenAI Responses runner; rejects prior messages, free-form instructions, and tools; emits runner audit; offline dry-run/mock tests pass. | One real stateless request using synthetic source data, followed by prompt-output validation and an audit-grade receipt. The committed proof must exclude credentials and private/raw provider data. |
| `codex-exec` | `recipe-only` | Current VPS has Codex CLI 0.145.0; `codex exec` exposes `--ephemeral`, `--ignore-user-config`, `--ignore-rules`, sterile `--cd`, read-only sandbox, JSON events, output schema, and no-resume operation. CLI validates the corresponding runner-audit fields. | A maintained wrapper that creates an isolated home/workdir, audits model-visible input, parses events, proves zero tool calls, and completes the full MDP receipt chain. |
| `claude-print` | `recipe-only` | Public recipe and CLI receipt validator exist; official CLI contract supports `--bare`, print mode, no session persistence, JSON Schema output, and tool restrictions. | Runner is not installed on this VPS and no maintained MDP wrapper or end-to-end receipt is recorded. |
| `cursor-print` | `recipe-only` | Public recipe and CLI validator exist; official docs expose print/stream JSON and explicitly state print mode has tool access. | A wrapper/external sandbox must deny tools, prevent resume and instruction discovery, parse events, and produce a full receipt. No local runner is installed. |
| `opencode-run` | `recipe-only` | Public recipe and CLI validator exist; official docs expose non-interactive runs, configurable permissions, and config/plugin discovery surfaces. | A pinned no-tool configuration/wrapper must prove config/plugin/instruction isolation and complete the receipt chain. No local runner is installed. |
| `custom-headless` | `unsupported` | Generic runner-audit schema branch validates common no-session/no-tools fields. | A named maintained adapter, documented ownership, machine-observed event evidence, and a full audit-grade receipt. |
| Native dry-run/mock and video fixtures | `fixture/mock-only` | Offline test and demo paths intentionally emit `mock_response`/fixture signals; CLI blocks them. | Never upgrade these artifacts. Run a separate real invocation instead. |

## Key Technical Decisions

### KTD-1: Separate support state from receipt assurance

`verified` describes maintained, machine-observed integration evidence. `headless-verified` and `stateless-api-verified` remain per-receipt assurance values. A structurally valid one-off runner audit can produce receipt assurance without automatically changing the public support matrix.

### KTD-2: Keep the support matrix documentation-owned in this slice

Do not add a new runtime registry or mutate `mdp.runner-audit.v0` just to publish support state. The CLI continues to validate evidence artifacts. A future machine-readable capability registry is justified only if installed hosts need programmatic discovery and can keep it release-synchronized.

### KTD-3: Native API is the first real verification path

The repo already controls request construction and output/audit emission for `native-api`, making it the smallest credible path to one verified row. The branch must not silently perform a paid or credentialed request. If no explicit secure-key run is authorized, create/link the bounded follow-up and keep the row recipe-only.

### KTD-4: Preserve fail-closed fixtures

The existing mock video is valuable precisely because it demonstrates blocked evidence. Do not rewrite mock artifacts to look production-like. Tests must continue asserting `decision: blocked` and `runner.assurance: invalid` for mock/demo evidence.

## Implementation Units

### U1. Canonical support matrix and public claim boundary

**Files**

- `docs/headless-normalization-runners.md`
- `docs/native-api-normalization-runner.md`
- `docs/run-receipts.md`
- `docs/proposal-runner.md`
- `README.md`

**Work**

Add the canonical four-state matrix, define the upgrade rule, and make the client-video decision explicit. Cross-link rather than duplicating divergent tables. State that current repo evidence is recipe/offline-fixture evidence until a real audit-grade receipt is recorded.

**Test scenarios**

1. A reader can identify the current state of all six runner identifiers without inferring verification from the presence of a recipe.
2. Native mock mode is visibly fixture-only and blocked.
3. Client-video instructions distinguish a real verified path from the default synthetic walkthrough.
4. No public copy claims compliance, CUI readiness, semantic truth, or hosted isolation.

**Covers** R1-R8, R12.

### U2. Receipt and schema regression contract

**Files**

- `cli/src/commands/run_receipt.rs`
- `cli/src/commands/schemas.rs`
- `scripts/test-native-runner.sh`
- `scripts/test-proposal-runner.sh`

**Work**

Add or tighten characterization tests around each accepted runner identifier. Prove that identifiers do not bypass runner-specific evidence, missing hashes, mock flags, synthetic model detection, isolation fields, or zero-tool requirements. Avoid changing the receipt contract unless a concrete false-positive is demonstrated.

**Test scenarios**

1. Valid `native-api` audit requires stateless request, no prior messages, no tools, matching output hash, and non-synthetic evidence.
2. Codex/Claude/Cursor/OpenCode audits fail when any runner-specific isolation field is absent.
3. `custom-headless` with only common fields cannot be advertised as maintained/verified in docs; fixture flags still block its receipt.
4. Mock native output remains blocked end to end through the proposal runner.
5. Runner-audit schema continues to expose the six identifiers and fixture markers without promising public support state.

**Covers** R2-R5, R9.

### U3. Agent-facing proposal guidance parity

**Files**

- `plugin/skills/mdp/SKILL.md`
- `plugin/skills/mdp/references/cli-operator.md`
- `plugin/skills/mdp-pack-builder/SKILL.md`
- `plugin/skills/mdp-pack-review/SKILL.md`
- `plugin/skills/mdp-proposal-review/SKILL.md`
- `plugin/assets/templates/proposal/README.md`

**Work**

Update canonical authored skills and proposal starter guidance to use the support-state vocabulary. Instruct agents to require an actual receipt for the current invocation and to avoid describing recipe-only runners as verified integrations. Preserve plugin packaging fidelity through the existing validation pipeline.

**Test scenarios**

1. Proposal review refuses to infer audit grade from MCP availability, runner name, or recipe documentation.
2. Pack builder/reviewer distinguish offline test evidence from real runner evidence.
3. Installed template guidance points to the canonical matrix and retains public-safe/local-first boundaries.
4. Skill packaging validation proves only `plugin/skills/` is authored and bundled copies remain byte-aligned.

**Covers** R7-R10.

### U4. Verification evidence and follow-up boundary

**Files**

- `.agent-artifacts/` for ignored local scratch only
- Linear MDP-127 comment/evidence links
- Follow-up Linear issue when a credentialed real run is deferred

**Work**

Run all offline/source checks in this branch. A real native verification may run only after explicit operator selection of a secure key flow and must use synthetic proposal data. Commit only sanitized hashes, versions, commands, and pass/fail summaries if useful; never commit the key, raw provider response, customer material, or local auth state.

**Test scenarios**

1. Offline validation passes with no key configured.
2. If authorized, a real native run returns `decision: audit-grade` and `runner.assurance: stateless-api-verified` with matching artifact hashes.
3. If not authorized, the support matrix remains recipe-only and a follow-up issue states the exact secure verification gate.
4. GTM and Proposal strict evals remain green after documentation/skill changes.

**Covers** R8, R11, R12.

## Sequencing

1. U1 establishes the reviewed vocabulary and baseline truth.
2. U2 checks whether the current CLI can falsely accept incomplete evidence and adds characterization coverage before any behavior change.
3. U3 aligns agent-facing surfaces with the accepted support matrix.
4. U4 runs validation and either records sanitized real proof or creates the bounded verification follow-up.

MDP-125 threat modeling may proceed alongside review of this plan, but implementation must incorporate any accepted threat-model requirement that changes the runner evidence boundary before PR closeout.

## Verification Contract

Run narrow checks first:

```bash
cargo test --manifest-path cli/Cargo.toml run_receipt
bash scripts/test-native-runner.sh
bash scripts/test-proposal-runner.sh
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/proposal
cargo run --manifest-path cli/Cargo.toml -- --json eval --strict --dir plugin/assets/templates/proposal
```

Then run:

```bash
cargo test --manifest-path cli/Cargo.toml
make validate
```

Manual review:

- Compare every matrix state to repository evidence and locally observed runner availability.
- Confirm the default proposal video remains explicitly mock/non-audit-grade.
- Confirm no credentials, customer sources, raw provider responses, local auth paths, or private transcripts are staged.
- Confirm GTM receives regression coverage only; this issue does not claim a GTM evidence-runner feature.

## Risks and Controls

| Risk | Control |
|---|---|
| Documentation says “verified” when only schema validation exists | Four-state matrix plus machine-observed upgrade rule. |
| A fixture becomes client-facing proof | Preserve fixture markers and blocked receipt tests. |
| Credentialed verification leaks a key or provider response | Explicit operator gate, secure local key flow, synthetic input, ignored scratch, sanitized evidence only. |
| Host CLI flags drift | Verify against current official documentation and installed `--help`; keep non-installed hosts recipe-only. |
| Matrix and skills diverge | Canonical matrix plus same-change updates to `plugin/skills/` and packaging validation. |
| Scope expands into every runner | Verify one native path first; create separate vertical-slice issues for maintained wrappers. |

## Definition of Done

- The support matrix is present and uses the four canonical states.
- Public docs and proposal skills no longer imply every accepted runner identifier is a verified integration.
- Receipt/schema tests cover the fail-closed runner-specific evidence paths.
- The client-video path is explicitly real-and-verified or mock-and-non-audit-grade.
- A real native verification receipt is recorded safely, or a linked follow-up keeps `native-api` recipe-only.
- Proposal and GTM regression validation passes.
- `make validate` passes.
- The branch is committed, pushed, and opened as a PR containing MDP-127; Linear has validation and residual-risk evidence.
