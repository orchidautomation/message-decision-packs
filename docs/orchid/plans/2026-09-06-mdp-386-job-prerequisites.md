# MDP-386: Derive Readiness From Every Selected-Job Prerequisite

## Objective

Compile and evaluate every prerequisite of the selected final job before the
earliest authoritative readiness decision. Keep normalization success distinct
from permission to generate, and remain neutral to GTM, support, recruiting,
proposal, and future domain vocabulary.

## Source

- Repository: `orchidautomation/message-decision-packs`
- Source ref: `origin/main`
- Source commit: `6c9d8c7cd4e256cab8f54c5b0670530028b96159`
- Linear issue: `MDP-386`

## Owned implementation surface

- A neutral selected-job prerequisite compiler/evaluator, preferably isolated
  in a new module.
- Requirements projection of compiled prerequisite identities and states.
- Job-scoped fit, brief, check, readiness, and run-preflight consumption.
- Stable missing/unknown/not-applicable states and smallest safe next actions.
- Sanitized GTM, support, recruiting, and proposal-shaped fixtures.

Primary ownership is the new module plus
`cli/src/commands/requirements.rs`, `routing.rs`, `briefs.rs`, and
`readiness.rs`. Small integration edits to shared runtime or prompt-output files
must be isolated and reported to the root before final integration.

## Forbidden and deferred surface

- Do not alter model-selected reference vocabularies (`MDP-387`).
- Do not add artifact text-field guardrail semantics (`MDP-390`).
- Do not implement final post-generation validator composition (`MDP-388`).
- Do not hard-code `relationship_lane` or any GTM-specific prerequisite.
- Do not overload normalization `outcome` with generation readiness.
- Do not modify outreach, CRM, HeyReach, cloud, or hosted-product behavior.

## Acceptance

1. A deterministic regression proves a structurally valid normalized input can
   currently project ready while an active selected-job no-draft prerequisite
   is absent.
2. Decision-input requirements, active no-draft/readiness effects, required
   model-step inputs, routed context, and host-owned execution inputs compile
   into one stable prerequisite projection.
3. Missing active prerequisites block fit/brief/check/readiness/run preflight at
   the earliest honest boundary with the same stable identifier.
4. Optional nonblocking prerequisites remain optional; false conditions become
   not-applicable; unresolved conditions remain unknown and fail closed when
   required for authority.
5. Adding the missing prerequisite makes the same synthetic case ready without
   changing unrelated authority.
6. GTM, support, recruiting, and proposal-shaped fixtures prove neutrality.
7. Existing compatibility tests and the full Rust test suite remain green.

## Validation

- Focused red/green tests for requirements, routing, brief, readiness, and run
  preflight agreement.
- `cargo fmt --check`
- `cargo test`
- `make validate-version-sync`
- `git diff --check`

## Integration

This lane runs concurrently with `MDP-387` but does not own the shared output
reference contract. The root orchestrator integrates any small shared-file
touches after both lane candidates are stable. `MDP-388` remains blocked on both
contracts. Brandon alone merges.
