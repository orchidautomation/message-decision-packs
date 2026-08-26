---
title: MDP-241 Productization Wave 2 - Commit-to-Build Plan
type: execution-index
date: 2026-08-26
topic: mdp-productization-wave-2
execution: orchid
artifact_contract: orchid-plan/v1
artifact_readiness: implementation-ready
linear_issues:
  - MDP-241
  - MDP-242
  - MDP-246
  - MDP-247
---

# MDP-241 Productization Wave 2 - Commit-to-Build Plan

## Context and current behavior

At planning base `5aaaf850b24b57622aca118da84cf02649380ab7`, the first foundation wave (MDP-243 and MDP-245) is already delegated to Orchid. Two independent foundation lanes remain immediately executable:

- MDP-246: both local MCP adapters accept caller-selected filesystem paths, and provider credentials are enabled at process scope rather than by consent bound to the exact frozen request bytes.
- MDP-242: the canonical CLI already compiles, executes, receipts, and verifies explicit run artifacts, but skill instructions still expose intermediate path choreography instead of treating it as private workflow state and returning one durable run-directory pointer.

MDP-247 consumes the presentation contract owned by the running MDP-245 and therefore remains blocked. This planning snapshot records that dependency but does not assign MDP-247 an implementation branch, child plan, or ready transition.

## Objective, scope, and out of scope

Prepare two independently executable Orchid lanes:

1. Put all MCP pack, input, approval, and output access behind startup-configured approved roots and require request-bound provider consent before any provider-capable child process starts.
2. Make skill workflows privately manage intermediate artifacts and hand off one explicit, verified durable run directory with decision, gaps, retention result, and next permitted action.

Out of scope: implementing MDP-247 before MDP-245 completes; hosted MDP; a global artifact registry; ambient latest selection; changing CLI decision authority; release, merge, deployment, or PR autofix.

## Execution topology and ownership

| Issue | Implementation branch | Owned paths | Forbidden paths | Dependency |
|---|---|---|---|---|
| MDP-246 | `codex/mdp-246-mcp-roots-consent` | `scripts/lib/mcp-path-policy.mjs`, `scripts/lib/mcp-provider-consent.mjs`, `scripts/mdp-run-mcp-server.mjs`, `scripts/mdp-proposal-mcp-server.mjs`, `scripts/mdp-proposal-runner.mjs`, MCP tests, MCP security docs | `plugin/skills/**`; Rust CLI authority/receipt code | none |
| MDP-242 | `codex/mdp-242-workflow-bundle-handoff` | authored MDP skill instructions/references, skill contract/eval fixtures and harness assertions, operator documentation | MCP server/runner code owned by MDP-246; run-bundle and receipt schemas | none |
| MDP-247 | none | none | all implementation paths | blocked by MDP-245 |

MDP-242 and MDP-246 may run in parallel because their writable paths do not overlap. If either lane discovers a required edit in the other lane's ownership, stop and reconcile ownership before writing. Each executable issue should produce its own PR because the changes are independently shippable and have different review profiles; MDP-246 requires security-focused human review.

## Acceptance and validation map

| Issue | Acceptance authority | Focused proof | Broad proof |
|---|---|---|---|
| MDP-246 | No path outside the approved roots is opened or created, and no provider-capable child starts without tamper-evident consent for the exact frozen request/source digest. | adversarial traversal/symlink/hard-link/rename/TOCTOU matrix plus process-spawn spy | canonical and proposal MCP suites, syntax checks, security review |
| MDP-242 | Normal skill use exposes only pack/job/approved input and returns one verified explicit run-directory pointer; scratch remains private and is removed on every terminal path. | skill contract/eval cases for success, blocked result, timeout, cancellation, resume, and concurrent runs | full skill packaging/contract/eval suite plus one synthetic end-to-end workflow |
| MDP-247 | dependency only | MDP-245 must merge and its exact contract must be inspected before a new plan pin | no Wave 2 implementation |

## Integration, rollout, observability, and rollback

- Start both executable branches from this exact pushed planning commit.
- Preserve CLI authority and pass through canonical decision/receipt results without reinterpretation.
- MDP-246 changes startup configuration and request rejection only; default state remains provider-disabled. Roll back by reverting its PR and removing any new startup configuration.
- MDP-242 changes authored workflow behavior and tests only; roll back by reverting its PR. No durable run artifact or schema migration is introduced.
- Do not merge, release, deploy, or enable `ai:autofix-enabled` automatically.

## Risks and safety boundaries

- Approved-root checks must be component-aware and descriptor/identity-based at the use boundary; string-prefix checks are insufficient.
- A process-level environment flag or credential is capability, not per-call consent.
- Consent material supplied only as an ordinary tool argument is untrusted and cannot authorize execution.
- Skill scratch state is never authority. The explicit run directory and independently verified receipt are the durable handoff.
- Tests and examples must be synthetic and must not expose absolute local paths, source bodies, credentials, or customer data.

## Blockers and readiness verdict

MDP-242 and MDP-246 have no live issue blocker and have exact ownership, acceptance, tests, compatibility, and rollback boundaries. MDP-247 remains blocked by MDP-245 and is intentionally excluded from the ready queue.

**Verdict: MDP-242 and MDP-246 are `READY_TO_PIN`; MDP-247 is `BLOCKED`.**
