# MDP-345 — Temporal Provenance And Pack-Health Plan

**Date:** 2026-09-02  
**Issue:** MDP-345  
**Repository:** `orchidautomation/message-decision-packs`  
**Base branch:** `codex/mdp-owner-governance-stack`  
**Planning baseline:** `f54b1f09578f3a355b65a8fb9708e4e6df1c8018`  
**Implementation branch:** `codex/mdp-345-temporal-health`  
**Risk:** Elevated — new durable governance metadata and a public deterministic health contract

## 1. Current Repository Behavior

- `cli/src/models.rs::Manifest` has no decision-group registry. Its required
  `Provenance` contains only `owner`, `created_by`, and `notes`.
- Card entries are typed authority. `Entry.metadata` is an open map, but the
  accepted MDP-344 contract requires governance to be typed rather than hidden
  in arbitrary metadata.
- `.mdp/sources.yaml` uses `mdp.sources.v0` and is currently loaded as
  `serde_json::Value` by README, health, and prompt-output paths. Entries expose
  descriptive `freshness` strings but no portable observation/publication,
  byte-identity, revocation, or supersession contract.
- `cli/src/commands/requirements.rs` already validates UTC timestamps and derives
  input age from explicit provenance plus a trusted `as_of`. That implementation
  is job-input-specific and must not become a second incompatible clock parser.
- `cli/src/runtime_context.rs` owns local UTC runtime metadata. Filesystem mtime
  is not semantic pack input. `artifact_hash.rs` already binds semantic `.mdp`
  authority while excluding generated briefs and traces.
- `mdp status`, `mdp doctor`, and `mdp check` have distinct established roles.
  MDP-345 must not silently tighten them for legacy packs. MDP-351 will later
  compose temporal results into broader maintenance guidance.

## 2. Objective

Add one additive, portable governance contract and one read-only
`mdp temporal-health` projection that distinguish source age from decision-review
state. The result must be deterministic from declared governance data and an
explicit or locally trusted UTC evaluation time, must never read filesystem
mtime, and must preserve existing-pack validity and readiness.

This slice also introduces the minimal typed decision-group registry required by
the accepted owner contract. It indexes exact existing entries and affected
canonical jobs but does not yet compute job completeness; MDP-346 owns that
projection.

## 3. Contract Decisions

### D1. Put decision groups in the manifest

Add optional `Manifest.decision_groups`. Each group has a stable ID and label,
exact `{card_id, entry_id}` references, canonical job IDs, optional owner, and an
optional review policy. It contains no decision prose. This keeps membership and
governance in the semantic pack hash while leaving referenced entries as the
message authority.

Minimum serialized shape:

```yaml
decision_groups:
  - id: local-first-category-boundary
    label: Local-first category boundary
    entries:
      - card_id: positioning
        entry_id: decision-layer
    jobs: [outbound-copy-brief]
    owner: product-marketing
    review_policy:
      cadence: P90D
    temporal:
      lifecycle: current
      changed_at: 2026-08-01T12:00:00Z
      reviewed_at: 2026-08-28T12:00:00Z
      source_revisions:
        - source_id: mdp-reference-contract
          sha256: <64 lowercase hex characters>
```

The decision temporal block also permits `revoked` or `superseded` lifecycle,
with matching `revoked_at` or `superseded_at` and an optional replacement group
reference. The exact Rust names may follow repository conventions, but
serialized field names and semantics must remain explicit. `changed_at` and
`reviewed_at` are optional until MDP-348/MDP-354 can bind them to
approval/history receipts. Absent evidence remains unknown; a declaration
cannot claim a receipt exists.

### D2. Extend source entries additively

Keep `mdp.sources.v0` valid. Allow an optional typed `temporal` object on each
source entry with:

- `observed_at`, `published_at`, and `imported_at` as independent optional UTC
  instants;
- optional exact `sha256` for imported/pack-local bytes;
- lifecycle `current`, `revoked`, or `superseded`;
- `revoked_at` or `superseded_at` only with the matching lifecycle;
- optional `superseded_by` source ID;
- optional owner and review policy, including a portable ISO-8601 day cadence
  (`P<n>D`) and optional source aging/stale thresholds.

The existing descriptive `freshness` string remains compatible display metadata
and never becomes a clock. Remote/public locators are not fetched. A declared
hash proves identity only; it does not prove truth.

### D3. Keep pack publication separate

Extend manifest `Provenance` with optional typed temporal publication metadata:
`published_at` plus an optional publication receipt reference/hash. A publication
time without a binding receipt is reported as declared/unverified, not upgraded
to verified publication authority. MDP-354 will later add the durable receipt
history.

### D4. One canonical UTC/cadence implementation

Create a shared crate module for strict `YYYY-MM-DDTHH:MM:SSZ` parsing,
UTC-second conversion, checked day arithmetic, and `P<n>D` parsing. Refactor the
private equivalent in `commands/requirements.rs` to call it without changing
existing decision-input diagnostics or JSON. Do not add a network time source or
infer local timezone.

### D5. Projection, not a readiness engine

`mdp temporal-health --dir PACK_ROOT [--as-of UTC]` emits
`mdp.temporal-health.v1`. If `--as-of` is absent, use
`runtime_context::current_runtime_context()` and include the exact evaluation
instant. The output independently reports:

- sources: `current | aging | stale | unknown | revoked | superseded`;
- decision review: `review-current | review-due | review-overdue |
  never-reviewed | revoked | superseded`;
- pack publication: known/unknown plus declared/receipt-bound authority;
- explicit diagnostics and the smallest deterministic review recommendation.

It never changes `mdp check` readiness, resolves a conflict, supersedes a
decision, or treats a newer source as approval. Legacy packs return an available
projection whose governance states are `unknown`/`unassessed`, with no validation
error merely for omitted fields.

## 4. Validation Rules

Fail closed for governance metadata that is present but impossible or
authority-upgrading:

1. Every declared timestamp must be strict UTC and no later than `as_of` beyond
   the documented clock-skew allowance (default zero for persisted authority).
2. `reviewed_at` cannot precede `changed_at`.
3. `revoked_at` and `superseded_at` must match lifecycle and cannot precede the
   relevant observed/imported/changed time.
4. `superseded_by` must reference a distinct existing source; supersession does
   not transfer an approval or review timestamp.
5. A decision source binding references an existing source and exact lowercase
   SHA-256. A changed source SHA makes the decision `review-due` or
   `review-overdue` according to policy; it never rewrites the decision.
6. Entry references resolve to exact manifest card/entry IDs. Job references
   resolve to canonical manifest jobs. Unknown references are validation errors.
7. Cadences and thresholds use checked positive day counts; overflow, zero,
   malformed, or contradictory thresholds fail validation.
8. Missing dates stay missing. Do not substitute filesystem mtime, Git commit
   time, the current time, or pack version.
9. Optional Git evidence, if shown later, is supporting evidence only and is not
   part of this slice's health calculation.

Structural validation of present governance fields belongs in `mdp doctor` so
malformed claimed authority cannot be ignored. Absence remains compatible and
non-blocking.

## 5. Affected Files And Symbols

| File | Intended change |
|---|---|
| `cli/src/time.rs` (new) | Strict UTC parsing, checked day arithmetic, cadence parsing, and table-driven unit tests shared by requirements and temporal health. |
| `cli/src/lib.rs` or crate module root | Register the shared time module without exposing an unstable public Rust API. |
| `cli/src/models.rs` | Add optional typed `decision_groups`; entry/job/source-ref, owner, review-policy, temporal, lifecycle, and publication structures; extend `Provenance` additively. Do not use `Entry.metadata`. |
| `cli/src/commands/temporal_health.rs` (new) | Load manifest and source ledger, validate typed optional temporal blocks, derive `mdp.temporal-health.v1`, and render bounded owner-readable text from the same result. Keep a pure evaluator accepting an explicit `as_of` for tests. |
| `cli/src/commands/requirements.rs` | Replace the private UTC conversion helper with the shared implementation while preserving behavior and diagnostics. |
| `cli/src/commands/health.rs` | Validate present decision-group/source/publication governance, exact refs, lifecycle/timestamp invariants, hashes, and cadence; absence is not an issue. Reuse temporal-health validators rather than duplicate rules. |
| `cli/src/commands/mod.rs` | Register the temporal-health module and exports. |
| `cli/src/cli.rs` | Add `Commands::TemporalHealth { dir, as_of }` and `SchemaTarget::TemporalHealthV1`; document read-only/offline behavior. |
| `cli/src/app.rs` | Dispatch with explicit `--as-of` or trusted current runtime time and preserve global JSON/summary contracts. |
| `cli/src/output.rs` | Add intentional concise human output/summary for temporal health; keep one valid JSON envelope under global `--json`. |
| `cli/src/commands/schemas.rs` | Publish a closed `mdp.temporal-health.v1` schema and schema target. |
| `cli/src/commands/capabilities.rs` | Advertise the command, schema, flags, no-network/read-only side effects, and stable diagnostics. |
| `cli/src/starter.rs` and `cli/src/target_starter.rs` | Keep generated packs compatible. Add only synthetic optional governance examples if doing so does not imply real review/publication evidence; otherwise leave starter metadata absent and test the unknown projection. |
| `cli/tests/cli_contract.rs` | Cover help, strict `--as-of`, schema target, and command argument behavior. |
| `cli/tests/json_stdout_contract.rs` | Cover JSON, summary, error, and stdout/stderr invariants. |
| `cli/tests/fixtures/` or module fixtures | Add synthetic current/stale/reviewed/superseded/legacy packs without private data. |
| `CONCEPTS.md` | Define decision-group authority, independent source/review state, and unknown semantics. |
| `cli/USAGE.md` | Add copyable `temporal-health` examples, including an explicit `--as-of`. |
| `docs/product-foundations.md` | Explain how groups reference, rather than duplicate, existing entry authority. |
| `docs/run-receipts.md` | Clarify that integrity and publication/source freshness remain separate and that future receipts bind declared times. |
| `plugin/skills/mdp/references/operator-runtime.md` | Teach agents to report source age and decision review independently and never invent dates or treat the projection as approval. |

If current module names differ, update the nearest existing canonical surface and
record the exact substitution in the implementation summary. Do not touch
generated host bundles; `plugin/skills/` is the authored source.

## 6. Ordered Implementation

### Step 1 — Centralize portable clock semantics

Add the strict UTC/cadence helpers and migrate requirements to them with
byte-for-byte-equivalent outcomes. Test leap dates, epoch boundaries, future
times, malformed zones, zero/overflow cadence, and checked date addition.

### Step 2 — Add typed additive governance models

Add decision groups and optional temporal/publication structures. Validate
stable unique IDs, exact entry/job/source references, no duplicate decision
prose field, owner/review policy shape, and legacy deserialization. Confirm the
new values participate in normal semantic pack hashing.

### Step 3 — Validate source-ledger temporal blocks

Parse only the new optional blocks into typed structures while preserving the
existing open descriptive source fields. Enforce lifecycle/timestamp/hash/ref
invariants. Never fetch a locator or consult mtime. For a safe pack-relative
regular file with a declared SHA, compare exact bytes; for remote or unavailable
locators report verification unavailable rather than pretending equality.

### Step 4 — Build the pure temporal evaluator

Given loaded authority and an explicit `as_of`, derive source age and decision
review state independently. A recently reviewed decision may remain
`review-current` while its source is `aging` or `stale`. Source-revision mismatch
starts a review need but cannot alter the referenced entry or mark a replacement
approved. Unknown inputs produce unknown/unassessed outputs.

### Step 5 — Wire CLI, schema, capabilities, and human rendering

Expose `mdp temporal-health`, closed schema discovery, global JSON behavior, and
concise owner-readable output. Human output must name evaluation time, separate
source and review state, explain unknowns, and give a finite next action without
claiming a scheduler exists.

### Step 6 — Prove compatibility and document the contract

Run the matrix below, portable-copy tests with altered mtimes, hash identity
tests, existing template initialization, full crate tests, repository validation,
and skill packaging checks. Update docs only after serialized names are final.

## 7. Acceptance Mapping

| Acceptance criterion | Proof |
|---|---|
| Old source and recent review are distinguishable | Matrix fixture asserts source `stale` plus decision `review-current` in one projection and human output. |
| Newer/changed source never silently supersedes an approved decision | Source SHA mismatch fixture retains decision entry/disposition, reports review required, and grants no approval/supersession. |
| Changed exact source bytes start a new candidate/review cycle where applicable | Pack-local hash mismatch test emits deterministic diagnostic/review need; unchanged digest remains current. |
| Health derives only from declared fields and trusted time | Pure evaluator tests use fixed `as_of`; portable-copy test changes mtimes without changing JSON. |
| Unknown dates remain unknown | Legacy and partially populated fixtures assert unknown/unassessed with no synthesized timestamp. |
| Semantic pack identity binds authority-bearing temporal data | Artifact-hash regression changes for each manifest/source temporal field and stays invariant under mtime-only changes. |
| Existing packs remain valid and readiness-compatible | All current template fixtures pass; `mdp check` result is unchanged before/after absent governance. |

## 8. Test Matrix

Table-driven cases must include:

- no governance fields;
- old published source, recent decision review, quarterly cadence;
- recent source, never-reviewed decision;
- review exactly due and one second overdue;
- source exactly at aging/stale boundaries;
- missing observed/published/imported timestamps independently;
- strict future timestamp and explicit clock-skew boundary;
- reviewed-before-changed;
- revoked and superseded sources/decisions with valid and invalid transition time;
- supersession self-reference/missing target;
- source digest equal, changed, malformed, and unverifiable remote locator;
- duplicate group IDs and missing entry/job/source refs;
- malformed/zero/overflow cadence;
- copy to another directory with all filesystem mtimes altered;
- pack hash changes from semantic temporal data but not mtime;
- GTM and proposal template compatibility.

Focused commands (use actual test filters created by implementation):

```bash
cargo test --manifest-path cli/Cargo.toml time
cargo test --manifest-path cli/Cargo.toml temporal_health
cargo test --manifest-path cli/Cargo.toml commands::requirements
cargo test --manifest-path cli/Cargo.toml commands::health
cargo test --manifest-path cli/Cargo.toml --test cli_contract temporal_health
cargo test --manifest-path cli/Cargo.toml --test json_stdout_contract temporal_health
```

Repository gate:

```bash
cargo test --manifest-path cli/Cargo.toml
make validate
```

## 9. Compatibility, Safety, And Rollback

- All governance fields and the command are additive. Existing pack structure,
  route, brief, readiness, doctor, and check behavior remain compatible when the
  fields are absent.
- Malformed metadata that claims governance authority is an error; absence is
  not. Adoption may occur one decision/source at a time.
- The command is local/offline and read-only. It performs no source fetch,
  background scheduling, Git inference, provider call, approval, mutation,
  publication, release, or deployment.
- Do not create approval/history receipts in this slice; MDP-348 owns the
  minimal approval receipt and MDP-354 owns durable history extensions.
- Rollback is a normal revert of the additive models/module/docs. No data
  migration or destructive cleanup is required.

## 10. Worker Boundary

The Luna worker owns only the paths in Section 5 and tests/fixtures directly
needed for MDP-345. It must not implement MDP-346 coverage, MDP-347 Overview,
MDP-348 Review queue/proposals, MDP-351 combined maintenance health, or
MDP-354 history receipts. It must not change product vocabulary or weaken
existing readiness/authority checks. Any conflict between this plan and current
repository behavior is escalated to Sol rather than resolved by rescoping.

No blocker remains: the accepted product contract, serialized boundary,
compatibility policy, CLI surface, affected paths, validation matrix, and
rollback are resolved.

**Readiness verdict: `READY_TO_PIN`**
