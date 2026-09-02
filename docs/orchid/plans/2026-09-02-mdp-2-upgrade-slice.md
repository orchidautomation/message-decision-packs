# MDP-2 — MDP-4 Upgrade Slice Execution Plan

**Date:** 2026-09-02  
**Parent:** MDP-2  
**Executable child:** MDP-4  
**Repository:** `orchidautomation/message-decision-packs`  
**Base branch:** `main`  
**Planning baseline:** `e7006907ca39379f72b3676812857d71ecbb1edb`

## Context

MDP-2 groups public CLI operator-hardening work. MDP-5, MDP-6, and MDP-202
have shipped. Brandon explicitly selected MDP-4 on 2026-09-02. MDP-3 remains
separate richer-version work and MDP-7 remains separate hosted-docs work.

The current executable slice is therefore one issue only: MDP-4 adds an
installed `mdp upgrade` front door over the existing canonical aligned
installer. The implementation must not reopen already-shipped general CLI
polish or expand into version metadata, hosted docs, or installer semantics.

## Objective And Scope

Deliver MDP-4 as one plan-pinned implementation lane and one feature PR:

- add interactive, non-interactive, and read-only check modes;
- preserve the fixed public installer as the only aligned update authority;
- keep JSON execution fail-closed while supporting JSON check output;
- align help, capabilities, doctor, public docs, and authored operator guidance;
- include the next available release version bump because the new public
  command is intended to ship from the implementation PR.

The issue-specific implementation contract is:

`docs/orchid/plans/2026-09-02-mdp-4-aligned-upgrade.md`

## Execution Graph

| Issue | Disposition | Dependency | Delivery |
|---|---|---|---|
| MDP-4 | executable | none | `codex/mdp-4-aligned-upgrade`, one PR to `main` |

MDP-3 and MDP-7 are not children of this execution graph. They receive no
branch, plan, code, lifecycle, or delivery mutation from this request.

## Acceptance And Validation

MDP-4 owns the complete acceptance mapping. Parent-level completion for this
slice requires:

1. `mdp upgrade`, `mdp upgrade -y`, and `mdp upgrade --check` satisfy the
   issue-specific safety and output contracts;
2. isolated tests prove no real network or active-home mutation;
3. the Rust suite, installer regressions, version sync, skill packaging, and
   repository validation pass at the final implementation commit;
4. one validated public-safe PR links MDP-4 and stops before merge/release.

## Compatibility, Risk, And Rollback

This is an additive CLI surface. Existing pack formats, commands, installer
targets, and raw bootstrap commands remain compatible. Supply-chain and local
mutation risk is bounded by the fixed visible HTTPS origin, confirmation before
network access, an owned temporary file, delegation to the existing installer,
and deterministic fake-process tests.

Rollback is a normal revert of the additive command, tests, documentation, and
version metadata before release. No data or pack migration is involved.

No merge, release, deployment, active-home installation, or host restart is
authorized. Brandon remains the only merge authority.

## Blockers And Readiness

MDP-4 has no active blocked-by relation. Required shipped behavior is present
on current `main`. The MDP-306 Linear record is stale relative to its merged
repository implementation, but this does not block MDP-4's code dependency.

**Readiness verdict: `READY_TO_PIN`**
