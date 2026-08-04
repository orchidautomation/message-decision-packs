---
title: MDP-177 Unified Runtime Code Review
date: 2026-08-03
status: ready-for-pr
plan: docs/orchid/plans/2026-08-03-001-feat-unified-clean-context-runtime-plan.md
linear: MDP-177
---

# MDP-177 Unified Runtime Code Review

## Scope and intent

The review covered the full branch diff from `origin/main` through the working
tree. The intent was to ship one bounded local clean-context kernel shared by
proposal validation and GTM qualification. The Rust CLI remains the only
authority for snapshots, assurance, terminal states, verification, and replay;
JavaScript and MCP surfaces remain compatibility or transport layers.

## Review coverage

The full reviewer roster covered correctness, project standards, testing,
maintainability, agent-native parity, security, API contracts, reliability, and
adversarial failure construction. An external cross-model review was attempted
but blocked by the host's code-egress policy before any repository content was
sent. The adversarial lens therefore ran in-process and a separate post-fix
adversarial pass rechecked every blocker.

## Applied blocker groups

| # | Group | Applied resolution |
| --- | --- | --- |
| 1 | Verifier authority | Recompute supported assurance from the sealed bundle and terminal state; reject forged assurance; bind audit snapshot, limitations, and receipt |
| 2 | Public contracts | Add closed execution and authority-block schemas; align capabilities, deterministic request schema, proposal v1 result, and Rust required fields |
| 3 | Filesystem isolation | Refuse top-level and nested links; bound reads before allocation; require exact input roles; use RAII cleanup and one exact recovery claim per transaction |
| 4 | Process failure | Supervise process groups through SIGTERM and SIGKILL; validate the exact owned recovery claim; remove only the named private transaction after forced termination |
| 5 | Decision integrity | Cover GTM qualified, disqualified, insufficient, missing-evidence, contradictory-source, direct-CLI, and MCP paths while keeping drafting authority not granted |
| 6 | Proof fidelity | Execute installed proposal and GTM runs, `verify-run`, and real installed MCP calls; add concurrent replay, mutation, schema, timeout, overflow, descendant, and cleanup tests |

The review rejected two preference-only blockers: splitting the large runtime
module and extracting shared MCP protocol plumbing. Those changes were not
required for correctness and would have expanded the release surface after the
security repairs.

## Agent-native result

The unified MCP exposes run inspection, `mdp_run`, and read-only
`mdp_verify_run`. It accepts only explicit local paths and returns canonical CLI
authority without recomputing or promoting assurance. Replay consumption and
production freshness remain host-owned, so the originating authoring
conversation cannot silently acquire decision authority.

## Validation

- Rust: 344 of 344 tests passed.
- Canonical JSON: 8 accepted and 14 rejected golden vectors passed.
- Cross-profile conformance: 20 of 20 passed.
- MCP plus proposal runtime: 21 of 21 focused behavior tests passed.
- Real forced-termination proof removed the exact claim and private staging
  transaction without deleting an unrelated sibling.
- `make validate` passed, including templates, skill contracts and evals,
  plugin packaging, public-artifact lint, Pluxx generation, installer fixtures,
  and installed-artifact proposal/GTM/MCP smoke tests.

## Residual boundaries

- Receipts are unsigned local integrity evidence, not issuer identity or
  non-repudiation.
- Portable path checks and recovery cleanup cannot eliminate every same-user
  replacement race without lower-level descriptor-relative primitives.
- Phase deadlines do not preempt a blocking kernel filesystem call.
- The local replay ledger cannot detect rollback, cloning, or snapshot restore.
- Windows proves direct-child termination; the descendant process-group proof
  is Unix-only.
- Real provider inference and customer sandbox claims remain MDP-184 and require
  explicit action-time approval after installation.

## Verdict

Ready for PR. No review blocker remains in the local deterministic release
scope. MDP Cloud remains `do-not-generalize-yet`; production auth, tenancy,
credential custody, retention, durable replay, and incident response remain
host or customer responsibilities.
