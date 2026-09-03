# MDP-346 — Job-Scoped Completeness And Coverage Plan

**Date:** 2026-09-03  
**Issue:** MDP-346  
**Repository:** `orchidautomation/message-decision-packs`  
**Implementation base:** `codex/mdp-owner-governance-delivery` at `e41117501c0bc0997b034fa4d33191bb024da972`  
**Implementation branch:** `codex/mdp-346-job-coverage`  
**Risk:** Elevated — a new owner-facing projection composes several existing authority and readiness contracts

## 1. Context And Current Behavior

- `cli/src/commands/readiness.rs::readiness` is the sole aggregate readiness
  projection. It composes structural validation, profile activation,
  `mdp.requirements.v1`, route-budget status, and optional input validation into
  `mdp.readiness.v1`. It deliberately owns no new validation or drafting
  authority.
- `cli/src/commands/requirements.rs::requirements` compiles one exact
  `Manifest.jobs[].id`. It already returns selected input contracts, decision
  input contracts, product-foundation resolution, profile activation, model
  steps, diagnostics, and the current draft boundary.
- `cli/src/product_foundation.rs::resolve_product_foundation` selects required
  and triggered conditional facets, preserves optional/excluded/untriggered
  facet IDs, resolves exact card-entry references, and reports explicit gaps and
  conflicts. Its `ready | blocked | unassessed` status is existing authority;
  MDP-346 must not reinterpret it.
- MDP-345 added `Manifest.decision_groups` and
  `cli/src/commands/temporal_health.rs::temporal_health`. Decision groups index
  exact existing entries and affected canonical jobs. They do not duplicate
  decision prose. Temporal health independently reports decision review state
  and source age without changing readiness.
- `ProfileJob.product_foundation` already distinguishes required, conditional,
  optional, and excluded foundation facets. `DecisionGroup.jobs` means affected
  jobs, not a separate permission or readiness declaration.
- `ProfileEval.required_categories` and activation diagnostics describe eval
  expectations. Decision-input contracts describe runtime collection needs.
  Neither is missing pack-decision authority and neither may be converted into
  a coverage gap.
- There is no exact-job owner view that brings these results together. Owners
  must currently infer completeness by reading several JSON documents, and a
  universal percentage would conceal that different jobs select different
  authority.

## 2. Objective, Scope, And Non-Goals

Add a read-only, offline `mdp job-coverage --dir PACK_ROOT --job JOB_ID
[--as-of UTC]` command with contract `mdp.job-coverage.v1`. For one exact
canonical job, it must explain:

- current readiness, copied from the existing readiness projection;
- required decision areas and whether each is established, gapped, or
  conflicted;
- decision-review state independently from decision resolution, including
  due/overdue/never-reviewed authority;
- optional, excluded, untriggered, irrelevant, and unnecessary areas without
  penalizing completeness;
- selected authority, input-contract boundaries, route-budget/readiness
  contributors, and eval expectations;
- a deterministic finite list of next owner questions for unresolved required
  decision areas.

The command is a projection, not a new decision or readiness engine. It must not
write files, call a provider, calculate an organization score, invent missing
decision groups, infer authority from README prose, or change `mdp check`,
`mdp requirements`, product-foundation resolution, routing, or temporal health.

Explicit assumptions and decisions:

1. A **decision area** is rooted in an existing product-foundation facet. A
   decision group may supply its human label, owner, exact authority references,
   affected-job relationship, and review state when its entries overlap that
   facet. This preserves the existing foundation contract as the source of
   required/optional/excluded selection.
2. Selected required and triggered conditional facets count toward
   completeness. Optional, explicitly excluded, untriggered conditional, and
   unrelated groups never reduce completeness.
3. Resolution and review are orthogonal fields. A stale/review-overdue decision
   may remain established authority; it is never relabeled absent. This follows
   the accepted MDP-344 vocabulary and MDP-345 semantics.
4. If a canonical job has no product-foundation binding, coverage is
   `unassessed`; it is not zero percent and does not block otherwise unchanged
   legacy readiness.
5. Unknown or free-text job IDs return a structured `unassessed` projection
   with an exact-job diagnostic and no fabricated requirements.

## 3. Output Contract And Data Flow

Concrete flow:

```text
exact JOB_ID
  -> existing manifest/job lookup
  -> existing product-foundation resolution + requirements + readiness
  -> MDP-345 temporal-health evaluation at one explicit/trusted UTC instant
  -> deterministic decision-area join by exact card_id/entry_id
  -> mdp.job-coverage.v1 JSON + owner-readable rendering
```

The closed output contains:

- `contract`, `status: complete | incomplete | unassessed`, `read_only`,
  `offline`, and one `evaluation.as_of`;
- `job` with requested ID and canonical match metadata;
- `readiness` with the existing `mdp.readiness.v1` status,
  `safe_to_draft_or_act`, first blocker, and next action;
- `decision_areas`, each with stable area ID, label, source facet, exact entry
  references, optional matching decision groups, selection
  (`required | conditional | optional | excluded | untriggered | irrelevant`),
  resolution (`established | gapped | conflicted | unassessed`), and independent
  review state (`review-current | review-due | review-overdue |
  never-reviewed | revoked | superseded | unassessed`);
- deterministic bucket indexes for `established`, `missing`, `conflicted`,
  `stale`, `optional`, `excluded`, and `unnecessary`; bucket values are stable
  area/group IDs, not duplicate prose;
- `selected_context`, `input_contracts`, and `eval_expectations` as named
  contributors, never as decision gaps;
- `next_questions`, one deduplicated item per unresolved required/conditional
  area, ordered by manifest/foundation authority and containing the area ID,
  authoritative label, reason code, exact references, and smallest safe command;
- bounded deterministic diagnostics.

No percentage field is permitted. `complete` means every applicable selected
required/conditional foundation area is established and non-conflicted. It does
not mean ready to draft; readiness remains whatever `mdp.readiness.v1` reports.
Review-due/overdue is listed independently and does not change completeness or
readiness in this slice.

## 4. Acceptance Mapping

| MDP-346 acceptance criterion | Implementation proof |
|---|---|
| Exact jobs report established and unresolved decision areas | Fixed fixtures assert selected required/conditional facets and exact entry/group references appear in established, missing, and conflicted buckets. |
| Coverage never upgrades readiness or treats a boundary as a gap | Projection copies readiness state and safe gate from `readiness`; tests hold readiness blocked while coverage is complete, and hold legacy readiness unchanged while coverage is unassessed. Input/eval/provider boundaries appear only under contributors. |
| Optional or irrelevant decisions do not reduce completeness | Fixtures add optional, excluded, untriggered, other-job, and unmapped affected groups and assert the same completeness status and required counts. |
| Conflicted and stale authority remain distinct from absent authority | One fixture produces an established but review-overdue area, one an explicit conflict, and one a gap; schema and human snapshots prove separate resolution/review fields and buckets. |
| Unknown/free-text jobs remain unassessed | Unknown ID test returns `status: unassessed`, no required areas, and an exact canonical-job diagnostic without calling it missing or blocked. |
| Human output gives a finite next-question list suitable for guided creation | Snapshot asserts one deterministic question per unresolved required area, no duplicate prose, stable order, exact references, and a bounded safe next command. |
| Validation matrix covers ready, gapped, conflicted, stale, optional-only, and unknown jobs | Table-driven module fixtures cover all six states plus ready-but-readiness-blocked and legacy-no-governance cases. |
| Cross-profile GTM/proposal parity | Generated basic/GTM and proposal templates both produce valid closed-schema results; absent governance is unassessed and preserves each template's existing readiness. |
| Coverage is a projection rather than a new decision engine | Regression tests compare contributor hashes/states with direct requirements/readiness/temporal-health results and verify the command performs no writes. |

## 5. Affected Files And Symbols

| File | Current responsibility and intended change |
|---|---|
| `cli/src/commands/job_coverage.rs` (new) | Own `mdp.job-coverage.v1`, pure deterministic join/classification helpers, exact-job/unassessed handling, next-question generation, and table-driven fixtures. Do not own validation or readiness decisions. |
| `cli/src/commands/temporal_health.rs` | Expose a crate-private evaluator that accepts one already-parsed `as_of`, so temporal health and job coverage share exactly one clock and state implementation. Preserve existing command JSON byte semantics aside from refactoring. |
| `cli/src/product_foundation.rs` | Reuse public crate-private resolution types and exact refs. Add only narrowly scoped helper visibility if required; do not change selection or blocking semantics. |
| `cli/src/commands/readiness.rs` | Reuse `readiness` as-is. Only add narrow accessors if the projection cannot safely consume its closed JSON result. |
| `cli/src/commands/requirements.rs` | Reuse `requirements` as-is for selected context/input/eval contributor evidence. Do not alter its status or schemas for coverage. |
| `cli/src/commands/mod.rs` | Register/export the job-coverage command function and contract. |
| `cli/src/cli.rs` | Add `Commands::JobCoverage { dir, job, as_of }` and `SchemaTarget::JobCoverageV1`; describe exact canonical job and read-only behavior. |
| `cli/src/app.rs` | Dispatch the new command through normal successful output so unassessed is a valid projection, not a process failure. |
| `cli/src/output.rs` | Render the same result as concise human and summary output: job/status, current readiness, required decisions, separate review warnings, selected authority counts, and finite next questions. |
| `cli/src/commands/schemas.rs` | Add a closed `mdp.job-coverage.v1` schema with bounded enums/arrays and no percentage. |
| `cli/src/commands/capabilities.rs` | Advertise command, schema, exact flags, read-only/offline side effects, and contributor authority. |
| `cli/tests/cli_contract.rs` | Cover help, exact arguments, `--as-of`, and schema target. |
| `cli/tests/json_stdout_contract.rs` | Cover JSON/summary/stdout/stderr invariants for complete, incomplete, and unassessed projections. |
| `cli/src/starter.rs` and generated template tests | Exercise both canonical templates without adding synthetic decision authority to shipped starter packs. Add test-only governed fixtures for populated decision groups. |
| `CONCEPTS.md` | Define job completeness as a projection over selected decision authority and distinguish it from readiness and percentages. |
| `cli/USAGE.md` | Add copyable exact-job commands and one practical owner-readable example. |
| `docs/product-foundations.md` | Explain required/conditional/optional/excluded mapping into decision coverage. |
| `plugin/skills/mdp/references/operator-runtime.md` | Teach skills to show the practical Jobs block, retain readiness authority, and ask only the emitted finite next questions. |

Do not edit generated host bundles. `plugin/skills/` is the only authored skill
source. Do not add queue/proposal persistence; MDP-348 owns that surface.

## 6. Ordered Implementation Steps

### Step 1 — Freeze fixtures and the projection vocabulary

Add test helpers that construct exact jobs with required, conditional, optional,
excluded, and untriggered product-foundation facets; decision groups that
overlap exact entry refs; and fixed MDP-345 review states. Write failing tests
for the six required coverage scenarios before wiring the command. This prevents
the implementation from smuggling in a percentage or a second readiness rule.

### Step 2 — Share the temporal evaluator without changing MDP-345

Refactor `temporal_health` so command parsing and pack loading delegate to one
crate-private fixed-time evaluator. Run all MDP-345 focused tests before and
after the refactor. Job coverage must use the same `as_of` and exact decision
review rows; it must not parse review cadence or source transitions itself.

### Step 3 — Build the pure decision-area join

Resolve one canonical job and its existing product foundation. For every
selected required/conditional facet, create one area rooted in its stable facet
ID and exact refs. Join decision groups only by exact `{card_id, entry_id}`
intersection; never fuzzy-match labels or bodies. Derive resolution only from
existing facet entries, gap refs, dangling-ref diagnostics, and explicit
selected conflicts. Attach review state only from the shared MDP-345 result.

Classify optional/excluded/untriggered facets separately. A decision group that
affects the selected job but maps to no selected required/conditional facet is
optional for completeness; a group that does not affect the job is irrelevant
and appears only in `unnecessary`. Neither changes the denominator or status.

### Step 4 — Compose, but do not reinterpret, current contributors

Call existing requirements and readiness projections for the same exact job.
Copy their authoritative status/gate fields and expose selected context, input
contracts, route-budget contribution, profile activation, and eval expectations
under named contributor objects. If a contributor is unavailable or invalid,
preserve its diagnostic/status; do not manufacture a missing decision area.

### Step 5 — Generate deterministic next questions and owner output

Emit one next-question record for each required/conditional area whose
resolution is gapped, conflicted, or unassessed, plus a separate review action
for due/overdue/never-reviewed authority. Use only declared labels and exact
references. Sort by canonical facet/group order with deterministic tie-breakers,
deduplicate exact records, and provide existing safe commands. Human output
must show readiness first, then required decisions, review warnings, selected
authority counts, and the next-action list.

### Step 6 — Wire CLI, schema, capabilities, docs, and compatibility

Register the command and schema, add JSON/summary contracts, and update only the
authored docs/skill source. Prove both shipped templates remain valid and
readiness-identical when governance metadata is absent. Confirm the command is
read-only by hashing/listing the pack before and after representative runs.

## 7. Tests And Validation

Focused automated checks:

1. `cargo test --manifest-path cli/Cargo.toml commands::job_coverage::tests`
2. `cargo test --manifest-path cli/Cargo.toml commands::temporal_health::tests`
3. Targeted schema, CLI parsing, output rendering, JSON stdout, template, and
   product-foundation tests.
4. Table matrix: complete, gapped, conflicted, established-but-stale,
   optional-only, unknown job, no foundation binding, other-job groups,
   readiness-blocked/coverage-complete, and deterministic ordering.
5. Snapshot/schema proof for both human and JSON output; validate emitted JSON
   against `mdp job-coverage-v1` schema discovery.
6. Pack-tree digest/listing equality before and after command execution.

Broader regression checks after the focused suite is green:

- `cargo fmt --manifest-path cli/Cargo.toml --all -- --check`
- `cargo check --manifest-path cli/Cargo.toml`
- `cargo test --manifest-path cli/Cargo.toml`
- affected template, skill-contract, skill-eval, skill-packaging, plugin,
  asset-sync, public-artifact, version-sync, and documentation validation gates
- exact-head GitHub CI and one exact-head Codex review (Cubic is unavailable on
  the current plan)

Manual proof uses a synthetic pack and fixed `--as-of`: show one job that is
coverage-complete but readiness-blocked by an input boundary, and one established
decision whose review is overdue. The rendered answer must explain both without
calling the input boundary a decision gap or the stale decision absent.

## 8. Compatibility, Safety, Rollout, And Rollback

- The change is additive: no existing manifest field becomes required and no
  starter receives invented governance evidence.
- Existing packs without product-foundation or decision-group governance remain
  valid. Coverage is `unassessed`; existing readiness and routing are unchanged.
- Schema and human output are new public surfaces. Unknown fields are rejected
  by the new closed schema, while absence of optional governance is accepted.
- No migration, network, provider, AI, queue, publication, release, deployment,
  or production mutation is authorized.
- Performance risk is bounded to local pack reads and deterministic joins. Reuse
  one loaded/fixed-time evaluation where practical and avoid quadratic body
  matching; all joins use exact IDs/refs and ordered maps/sets.
- Rollout is the cumulative draft branch and PR only. The public version bump is
  deferred to the final cohesive MDP-357 stack closeout.
- Rollback is a revert of the MDP-346 commits. MDP-345 temporal governance and
  all pre-existing commands remain independently usable.

Primary regression risks are accidentally upgrading readiness, conflating stale
with missing, counting optional/unrelated authority, changing MDP-345 semantics
during refactor, unstable ordering, and leaking full decision prose into a
summary. Every risk has an explicit matrix or snapshot assertion above.

## 9. Blockers And Readiness Verdict

- MDP-344 is accepted and authorizes repository-grounded planning.
- MDP-345 implementation is locally verified and pushed on the cumulative
  branch, but its exact-head CI and Codex review must complete cleanly before
  MDP-346 implementation dispatch.
- No unresolved product decision is required for the projection described here.
  The plan preserves affected-job semantics and derives completeness from
  existing product-foundation selection rather than inventing a new requirement
  registry.

**Verdict: BLOCKED** until the exact MDP-345 cumulative head has green required
checks and a completed clean Codex review. Once that gate clears, update only
the implementation baseline if the cumulative head changed, commit/push this
plan pin, create the issue-bound worktree, and dispatch one plan-pinned Luna
implementation lane.
