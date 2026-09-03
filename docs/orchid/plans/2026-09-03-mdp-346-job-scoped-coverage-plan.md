# MDP-346 — Job-Scoped Completeness And Coverage Plan

**Date:** 2026-09-03  
**Issue:** MDP-346  
**Repository:** `orchidautomation/message-decision-packs`  
**Implementation base:** `codex/mdp-owner-governance-delivery` at `fd3d3f1278c451ca7176cc4cb5a8d139e66db5d3`
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
- required decision areas and whether each is established, gapped, conflicted,
  blocked, or unassessed;
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

1. A **decision area** is rooted in a product-foundation facet declared by the
   exact job binding. When the selected registry or facet cannot resolve, the
   projection retains the declared facet ID and kind as unresolved selection
   identity; it does not invent a label, entries, or authority. A decision group
   attaches only when the group both names the exact canonical job **and** has
   at least one exact entry reference intersecting the facet. This preserves the
   existing foundation contract as the source of required/optional/excluded
   selection and `DecisionGroup.jobs` as affected-job authority.
2. Selected required and triggered conditional facets count toward
   completeness. Optional, explicitly excluded, untriggered conditional, and
   unrelated groups never reduce completeness.
3. Resolution, decision review, and source health are three orthogonal fields.
   A review-overdue decision may remain established authority; it is never
   relabeled absent. A stale source with a recently reviewed decision reports
   `source_state: stale`, `review_state: review-current`, and
   `resolution: established`. Source age does not silently change resolution or
   readiness unless an existing explicit policy already requires freshness.
4. If a canonical job has no product-foundation binding, coverage is
   `unassessed`; it is not zero percent and does not block otherwise unchanged
   legacy readiness.
5. Unknown or free-text job IDs return a structured `unassessed` projection
   with an exact-job diagnostic and no fabricated requirements.
6. Optional means only an explicit `ProfileJob.product_foundation.optional`
   facet. A group that names the job but maps to no required, triggered,
   optional, excluded, or untriggered facet is `unnecessary`; a group for a
   different job is `irrelevant`. Neither is silently reclassified as optional.
7. The accepted contract does not settle whether `review-due` or
   `review-overdue` changes the top-level completeness status. That rule is an
   explicit product-decision blocker below; implementation must not infer it.

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
- `decision_areas`, each with stable area ID, nullable authoritative label plus
  a display fallback derived only from declared facet ID/kind, source facet,
  exact entry references, selection (`required | conditional | optional |
  excluded | untriggered`), and resolution (`established | gapped | conflicted
  | blocked | unassessed`);
- per-area `governance` with every matching decision-group row, exact
  `covered_refs` and `uncovered_refs`, membership coverage (`complete | partial
  | unassessed`), and each group's independent review state (`review-current |
  review-due | review-overdue | never-reviewed | revoked | superseded |
  unassessed`). If a derived area review summary is emitted, it must use a
  documented worst-state precedence and can never hide partial membership;
- per-area `source_support` rows resolved from each matching group's exact
  `source_revisions`, preserving MDP-345 source state (`current | aging | stale
  | unknown | revoked | superseded`) independently from decision review;
- deterministic bucket indexes for `established`, `missing`, `conflicted`,
  `blocked`, `review_due`, `review_overdue`, `stale_sources`, `optional`,
  `excluded`, `irrelevant`, and `unnecessary`. Every bucket value is a typed
  reference such as `{ "kind": "area", "id": "proof" }` or
  `{ "kind": "decision-group", "id": "proof" }`, so facet/group ID
  collisions are unambiguous;
- `foundation_authority` containing job-level product-foundation selection and
  load-order evidence, `input_contracts`, route-budget/readiness contributors,
  and `eval_expectations` explicitly labeled `scope: profile`. The command does
  not claim persona- or scope-sensitive routed context because its signature
  has no persona/scope inputs;
- `next_questions`, one deduplicated item per resolution-unresolved
  required/conditional area, ordered by manifest/foundation authority and
  containing the area ID, nullable authoritative label, display fallback,
  reason code, exact references, and an existing read-only inspection command;
- `review_actions`, separately, one deterministic action per applicable
  due/overdue/never-reviewed/unassessed/revoked/superseded governance row. These
  actions may point to existing `mdp temporal-health`, `mdp requirements`, or
  pack-review guidance, but cannot fabricate the MDP-348 Review queue or the
  MDP-352 guided-creation mutation surface;
- bounded deterministic diagnostics.

No percentage field is permitted. The projection exposes separate
`resolution_status` and `governance_status`; readiness remains whatever
`mdp.readiness.v1` reports. A bound job with no decision groups, or with only
partial group membership, cannot report fully assessed governance even when all
foundation facets resolve. The top-level rule for due/overdue review remains
blocked on the explicit product decision in section 9.

## 4. Acceptance Mapping

| MDP-346 acceptance criterion | Implementation proof |
|---|---|
| Exact jobs report established and unresolved decision areas | Fixed fixtures assert selected required/conditional facets and exact entry/group references appear in established, missing, conflicted, blocked, and unassessed buckets, including missing registry/facet identity retained from the job binding. |
| Coverage never upgrades readiness or treats a boundary as a gap | Projection copies readiness state and safe gate from `readiness`; tests hold readiness blocked while coverage is complete, and hold legacy readiness unchanged while coverage is unassessed. Input/eval/provider boundaries appear only under contributors. |
| Optional or irrelevant decisions do not reduce completeness | Fixtures add explicit optional, excluded, untriggered, other-job, and unmapped affected-job groups; only the explicit optional facet is optional, the other-job group is irrelevant, and the unmapped affected-job group is unnecessary. |
| Conflicted and stale authority remain distinct from absent authority | Fixtures independently combine old-source/current-review, current-source/overdue-review, explicit conflict, gap, revoked, and superseded states; schema and human snapshots prove separate resolution, per-group review, source-support, and typed buckets. |
| Unknown/free-text jobs remain unassessed | Unknown ID test returns `status: unassessed`, no required areas, and an exact canonical-job diagnostic without calling it missing or blocked. |
| Human output gives a finite next-question list suitable for guided creation | Snapshot asserts one deterministic question per resolution-unresolved required area and separate review actions, with no duplicate prose, stable order, exact references, nullable labels/fallbacks, and only existing read-only commands. |
| Validation matrix covers ready, gapped, conflicted, stale, optional-only, and unknown jobs | Table-driven module fixtures cover all six states plus bound/ready foundation without decision groups, partial governance membership, ready-but-readiness-blocked, and legacy-no-governance cases. |
| Cross-profile GTM/proposal parity | Generated basic/GTM and proposal templates both produce valid closed-schema results; absent governance is unassessed and preserves each template's existing readiness. |
| Coverage is a projection rather than a new decision engine | Regression tests compare contributor hashes/states with direct requirements/readiness/temporal-health results and verify the command performs no writes. |

## 5. Affected Files And Symbols

| File | Current responsibility and intended change |
|---|---|
| `cli/src/commands/job_coverage.rs` (new) | Own `mdp.job-coverage.v1`, pure deterministic join/classification helpers, exact-job/unassessed handling, next-question generation, and table-driven fixtures. Do not own validation or readiness decisions. |
| `cli/src/commands/temporal_health.rs` | Expose a crate-private evaluator that accepts one already-parsed `as_of`, so temporal health and job coverage share exactly one clock and state implementation. Preserve existing command JSON byte semantics aside from refactoring. |
| `cli/src/product_foundation.rs` | Reuse public crate-private resolution types and exact refs. Add only narrowly scoped helper visibility if required; do not change selection or blocking semantics. |
| `cli/src/commands/readiness.rs` | Reuse `readiness` as-is. Only add narrow accessors if the projection cannot safely consume its closed JSON result. |
| `cli/src/commands/requirements.rs` | Reuse `requirements` as-is for foundation selection, input contracts, and profile-level eval contributor evidence. Do not describe this as routed selected context and do not alter its status or schemas for coverage. |
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
excluded, and untriggered product-foundation facets; decision groups that vary
exact job membership independently from exact ref overlap; multiple groups per
facet; one group spanning facets; partial ref membership; exact ID collisions;
and fixed MDP-345 source/review states. Write failing tests for the required
coverage scenarios before wiring the command. This prevents the implementation
from smuggling in a percentage, a second readiness rule, or a lossy group join.

### Step 2 — Share the temporal evaluator without changing MDP-345

Refactor `temporal_health` so command parsing and pack loading delegate to one
crate-private fixed-time evaluator. Run all MDP-345 focused tests before and
after the refactor. Job coverage must use the same `as_of` and exact decision
review rows; it must not parse review cadence or source transitions itself.

### Step 3 — Build the pure decision-area join

Resolve one canonical job and its declared product-foundation binding. For every
selected required/conditional facet, create one area rooted in its stable facet
ID and exact refs. If registry/facet resolution fails, retain the declared
selection identity and resolver diagnostic while leaving label/refs absent.
Join a decision group only when `group.jobs` contains the exact job **and** its
exact `{card_id, entry_id}` set intersects the area; never fuzzy-match labels or
bodies. Derive resolution only from existing facet entries, gap refs,
dangling-ref diagnostics, explicit selected conflicts, and existing blocked or
unassessed resolver status. Preserve every matching group row, group/ref
cardinality, covered refs, and uncovered refs; attach review and source-support
states only from the shared MDP-345 result.

Classify optional/excluded/untriggered facets separately. Only an exact mapping
to an explicitly optional facet is optional. A group that affects the selected
job but maps to no declared facet class is unnecessary; a group that does not
affect the job is irrelevant. Neither changes required resolution status.

### Step 4 — Compose, but do not reinterpret, current contributors

Call existing requirements and readiness projections for the same exact job.
Copy their authoritative status/gate fields and expose job-level foundation
authority, input contracts, route-budget contribution, profile activation, and
profile-scoped eval expectations under named contributor objects. If a
contributor is unavailable or invalid, preserve its diagnostic/status; do not
manufacture a missing decision area or claim persona-sensitive routed context.

### Step 5 — Generate deterministic next questions and owner output

Emit one next-question record for each required/conditional area whose
resolution is gapped, conflicted, blocked, or unassessed, plus a separate typed
review action for due/overdue/never-reviewed/unassessed/revoked/superseded
governance. Use only authoritative labels where present; otherwise show the
declared facet ID/kind fallback without presenting it as an authoritative label.
Sort by canonical facet/group order with deterministic tie-breakers, deduplicate
exact records, and provide only existing read-only inspection commands. Human
output must show readiness first, then required decisions, membership coverage,
review warnings, source health, selected authority counts, and the two bounded
action lists.

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
4. Table matrix: complete, gapped, conflicted, blocked, optional-only, unknown
   job, no foundation binding, bound foundation with no decision groups,
   partially grouped facet refs, shared ref/different job, listed job/no ref
   intersection, two groups with different review states on one facet, one group
   spanning two facets, duplicate membership, old-source/current-review,
   current-source/overdue-review, revoked/superseded group, source-revision
   mismatch, missing registry, missing selected facet, dangling ref, empty facet,
   unknown conditional, mixed gap+conflict precedence, group/facet ID collision,
   affected-job unmapped group, other-job group, contributor failure, explicit
   profile eval scope, readiness-blocked/resolution-complete, and deterministic
   ordering.
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

Manual proof uses a synthetic pack and fixed `--as-of`: show one job whose
required areas resolve while readiness remains blocked by an input boundary;
one stale source supporting a recently reviewed established decision; and one
current source supporting an overdue established decision. The rendered answer
must keep resolution, governance membership/review, source health, and readiness
separate without calling the input boundary a decision gap or stale support
absent.

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
source support with overdue review or missing authority, losing partial/multiple
group membership, dropping unresolved declared facets, counting
optional/unrelated authority, changing MDP-345 semantics during refactor,
unstable ordering, ID-type collisions, and leaking full decision prose into a
summary. Every risk has an explicit matrix or snapshot assertion above.

## 9. Blockers And Readiness Verdict

- MDP-344 is accepted and authorizes repository-grounded planning.
- MDP-345 implementation is locally verified and pushed on the cumulative
  branch, but its exact-head CI and Codex review must complete cleanly before
  MDP-346 implementation dispatch.
- **Product decision required:** the accepted MDP-344 contract and MDP-346 issue
  require reviewed authority and keep review state separate from resolution, but
  do not state whether `review-due` and `review-overdue` change top-level job
  completeness. Sol recommends a two-axis contract with
  `resolution_status: complete | incomplete | unassessed` and
  `governance_status: current | action-needed | unassessed`, with top-level
  `status: complete` only when required resolution is complete, membership is
  fully assessed, and every applicable current group is `review-current`.
  `review-due`, `review-overdue`, `never-reviewed`, revoked, or unreplaced
  superseded governance would therefore make coverage `incomplete` while still
  leaving the established resolution and existing readiness unchanged. This is
  the safer interpretation of “enough reviewed authority.” The alternative is
  complete-with-review-warning for due/overdue governance. Brandon must choose
  before the output schema is pinned.

**Verdict: BLOCKED** until (1) the exact MDP-345 cumulative head has green
required checks and a completed clean Codex review and (2) Brandon resolves the
top-level due/overdue completeness rule above. Once both gates clear, update only
the implementation baseline if the cumulative head changed, change this verdict
to `READY_TO_PIN`, commit/push the immutable plan pin, create the issue-bound
worktree, and dispatch one plan-pinned Luna implementation lane.
