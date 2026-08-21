---
title: "feat: Concise and filterable route-budget diagnostics"
type: bug
date: 2026-08-21
topic: route-budget-summary-filters
execution: code
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-235
linear_issues:
  - MDP-235
  - MDP-239
  - MDP-233
  - MDP-65
  - MDP-224
  - MDP-228
repository: orchidautomation/message-decision-packs
base_branch: main
base_commit: 2cba9919483b5a7ba46efed53e3b5502b2abf477
source_branch: codex/mdp-235-plan
---

# MDP-235: concise and filterable route-budget diagnostics

## Goal capsule

| Field | Decision |
|---|---|
| Objective | Make `mdp --json route-budget --summary` a bounded operator/agent rollup and add deterministic exact `--job <id>` / `--persona <id>` projections without weakening the full machine-readable preflight authority. |
| Authority | `cli/src/routing.rs::route_budget_preflight` remains the only route-budget evaluator. Summary and filter code project that result; they must not re-route cards, recalculate minimality, truncate entries, or infer a different validity result. |
| Summary contract | Additive `mdp.route-budget-summary.v1` summary metadata reports validity, route status counts, tightest entry/byte headroom, top blockers/contributors, and one safe next action. It contains no full route array or entry body. |
| Full contract compatibility | Preserve the existing `mdp.route-budget.v0` full payload and unfiltered `validate --strict` merge behavior. Add canonical `job_id` to route records while retaining `job` as a deprecated equal-value alias for existing v0 consumers; the plan documents the alias and tests its equality. A future removal is a separate versioned change. |
| Filter semantics | `--job` matches one exact manifest `jobs[].id`; `--persona` matches one exact declared `manifest.personas` value case-insensitively and returns the authored canonical label. When both are supplied, the projection is their deterministic intersection. No substring, title, prose, or fuzzy matching is introduced. |
| Safety | Overflow guidance identifies the smallest deterministic entry/byte reduction and a bounded applicability/card target where one exists. It never recommends truncation, silently dropping guardrails, changing a budget to hide overflow, or opening a full card. Required-only overflow becomes an explicit review/budget decision rather than an unsafe narrowing instruction. |
| MCP boundary | The current repository has clean-run MCP servers but no route-budget MCP evaluator. No second evaluator is added. Any existing MCP introspection/resource wrapper that documents or proxies this command must consume the CLI's `mdp.route-budget-summary.v1`/`mdp.route-budget.v0` output and expose the same filters and limits; transport remains non-authoritative. |
| Sequencing | MDP-233 remains the existing Linear blocker and must be implemented/validated before this work consumes final route counts. MDP-228 remains a related required-first allocation design; this issue reports the resulting budgets and does not redesign allocation. MDP-239 remains the parent execution index. |
| Stop condition | A 12-route synthetic pack produces a bounded summary, exact job/persona projections, stable `job_id` identity, percentages, safe overflow guidance, schema/help/skill/MCP parity, and installed CLI proof while existing full output, `validate --strict`, route-card-cap blocking, legacy unbudgeted jobs, and MDP-233 blocker metadata remain intact. |

## Repository routing and handoff

- Repository: `orchidautomation/message-decision-packs`.
- Base branch: `main` at `2cba9919483b5a7ba46efed53e3b5502b2abf477` (`origin/main`, v0.1.73).
- Planning branch: `codex/mdp-235-plan`.
- Isolated planning checkout: `/private/tmp/mdp-235-plan-work`.
- This document is plan-only. It does not implement runtime behavior, change issue status/labels/delegation/relations, add `delegate:blocks`, add Blocks branding, open/merge a PR, or alter MDP-233.
- The canonical checkout contains unrelated dirty files and is out of scope; it must remain untouched.

## Problem frame and current evidence

The current release already computes the right route-budget authority, but the
default summary is almost as large as the full result:

- `cli/src/output.rs::summarize("route-budget", ...)` copies the top-level
  counters and then maps every route into the summary, including diagnostics,
  reason distributions, largest contributors, and the full route-card receipt.
  On the MDP-235 Sanity reproduction this is 36,485 bytes versus 37,605 bytes
  for the full JSON result, so `--summary` does not provide a bounded diagnostic.
- `cli/src/cli.rs::Commands::RouteBudget` currently accepts only `--dir` and
  `--strict`. `cli/src/app.rs` passes those values to
  `commands::routing::route_budget_preflight_command` without a projection.
- `cli/src/commands/routing.rs::route_budget_preflight_command` applies strict
  warnings after the full route matrix is built. The same command is called by
  `cli/src/app.rs::merge_route_budget_preflight` during `validate`; filtered
  options must never leak into that validation merge.
- `cli/src/routing.rs::route_budget_preflight` iterates every canonical job and
  manifest persona, computes minimality counts and bytes through
  `entry_route_scoped`, records `context_entry_budget_exceeded`,
  `context_byte_budget_exceeded`, `near_context_budget`, and
  `route_card_cap_excluded_applicable`, and emits each route's `job` field.
  `context_minimality` already owns the exact counts, digest, exclusions, and
  body-free `largest_contributing_cards` receipt.
- `cli/src/app.rs::merge_route_budget_preflight` reads `route["job"]` when it
  turns preflight diagnostics into `validate` issues. That compatibility read
  must accept the canonical `job_id` while retaining the old v0 alias.
- `cli/src/commands/schemas.rs` exposes schemas for the surrounding manifest,
  context, and routed-context contracts but no explicit route-budget schema
  target. The output contract is therefore under-specified for consumers that
  need to know whether route arrays are allowed in a summary.
- `cli/src/commands/capabilities.rs` advertises `route-budget` as a read-only
  `mdp.route-budget.v0` command with only `--dir`/`--strict`, so help,
  capabilities, and a future summary schema would otherwise drift.
- `Makefile`, `scripts/build-route-budget-fixtures.mjs`, and
  `examples/route-budget/{overflow,ready}` prove overflow/ready behavior but
  do not assert summary boundedness, selector intersections, or 12-route
  aggregation.
- Installed skill guidance in `plugin/skills/mdp-pack-builder/SKILL.md`,
  `plugin/skills/mdp-pack-review/SKILL.md`, and the operator references tells
  agents to run the unfiltered strict preflight but does not provide a compact
  triage query or define safe narrowing output.
- `scripts/mdp-run-mcp-server.mjs` and `scripts/mdp-proposal-mcp-server.mjs`
  expose clean-run/proposal tools, not route-budget. This is an important
  boundary: adding a route-budget implementation to either MCP server would
  create a second authority. Parity work is limited to any introspection or
  resource description that names the CLI contract.

## Scope and non-goals

### In scope

- A typed/internal route-budget query containing optional exact job and persona
  filters, applied after the canonical route matrix is evaluated and before
  public projection.
- A body-free summary projection with status counts, utilization/headroom,
  blockers, contributors, and safe next-action guidance.
- Additive `job_id` identity and an explicit one-release compatibility alias
  for the existing v0 `job` key.
- Filter-aware full output, summary output, human rendering, JSON wrapper,
  CLI help/capabilities, explicit schema targets, local MCP introspection
  parity where such a surface exists, docs, skills, fixtures, and installed
  smoke proof.
- Deterministic 12-route synthetic coverage and output-size assertions.

### Out of scope

- Changing card selection, `applies_to` semantics, context minimality,
  route-card cap policy, budget values, required-first allocation, or the
  route evaluator's validity semantics.
- Fixing MDP-233's empty-selector routing behavior. MDP-233 remains the blocker
  and its issue state, relation, labels, and plan are read-only context here.
- Truncating route arrays, dropping required/guardrail entries, ranking away
  authority, or silently raising budgets to make a summary look healthy.
- A new hosted/MCP route evaluator, remote resource, provider call, CRM action,
  sender, enrichment, customer data, private Sanity evidence, or raw output
  upload.
- Removing `job` from `mdp.route-budget.v0`; that requires a separately
  versioned migration after consumers have adopted `job_id`.

## Public route-budget contract

### Query and projection behavior

The command keeps the current default/full behavior and adds selectors:

```text
mdp --json route-budget --dir PACK_ROOT
mdp --json --summary route-budget --dir PACK_ROOT
mdp --json route-budget --dir PACK_ROOT --job outbound-copy-brief
mdp --json route-budget --summary --dir PACK_ROOT --persona PMM
mdp --json route-budget --summary --dir PACK_ROOT --job outbound-copy-brief --persona PMM
```

The exact query rules are:

1. No selector plus no `--summary` returns the complete existing machine
   authority (`mdp.route-budget.v0`) with all route records. It remains the
   explicit/full diagnostic mode; no route is silently dropped.
2. `--job` selects the exact `jobs[].id`. It does not match job titles,
   descriptions, tokens, or model-step prose. An unknown ID returns one
   sanitized deterministic `route_budget_filter_not_found` error with the
   available route-independent field name, not a dump of the manifest.
3. `--persona` matches one declared manifest persona with the same
   case-insensitive trimmed comparison used by existing persona resolution and
   emits the authored manifest spelling. An unknown persona returns the same
   bounded error class; it does not infer a persona from cards or prose.
4. Supplying both selectors applies AND semantics. The query receipt records
   `{job_id, persona}` with null for an omitted selector and a deterministic
   `matched_route_count`.
5. A selector without `--summary` returns full route records for only the
   selected routes. A selector with `--summary` returns one rollup over only
   those routes; it still does not include a `routes` array.
6. `--summary` never emits the full route array, full minimality exclusions,
   route-card receipts, or entry bodies. It may include bounded contributor and
   blocker arrays with fixed maximum lengths (proposed five each), and every
   item is metadata-only.
7. `validate --strict` continues to call an unfiltered internal preflight and
   receives every route needed to merge blocking issues. CLI filters are only
   for the explicit `route-budget` command path.

### Summary shape

The planned summary is an additive projection, not a lossy replacement for the
full authority:

```json
{
  "contract": "mdp.route-budget-summary.v1",
  "source_contract": "mdp.route-budget.v0",
  "valid": false,
  "strict": {"enabled": true, "warnings_fail": true, "warning_count": 0},
  "pack_id": "synthetic-route-budget-pack",
  "query": {
    "job_id": "outbound-copy-brief",
    "persona": "Buyer",
    "matched_route_count": 1
  },
  "route_count": 1,
  "route_status_counts": {"ready": 0, "blocked": 1, "unassessed": 0},
  "tightest_headroom": {
    "job_id": "outbound-copy-brief",
    "persona": "Buyer",
    "dimension": "bytes",
    "used": 89000,
    "limit": 65536,
    "remaining": -23464,
    "utilization_percent": 135.65
  },
  "top_blockers": [
    {"code": "context_byte_budget_exceeded", "route_count": 1}
  ],
  "top_contributors": [
    {"card_id": "buyer-case-studies", "card_kind": "claims", "route_count": 1, "entry_count": 99, "canonical_bytes": 82000}
  ],
  "next_safe_action": {
    "kind": "narrow_applicability",
    "job_id": "outbound-copy-brief",
    "persona": "Buyer",
    "dimension": "bytes",
    "minimum_reduction": {"entries": 35, "bytes": 23464},
    "target_card": {"card_id": "buyer-case-studies", "card_kind": "claims"},
    "preserve_guardrails": true,
    "do_not": ["truncate", "drop_guardrails", "open_full_card"]
  }
}
```

The concrete numbers above are illustrative; the implementation must derive
them from the existing route receipts. The contract rules are normative:

- `route_status_counts` includes all statuses represented by the selected
  matrix and is stable even when a count is zero.
- `tightest_headroom` considers both entry and byte dimensions for budgeted
  routes. It uses integer arithmetic rounded to two decimal places, sorts by
  highest utilization, then `job_id`, persona, and dimension (`bytes` before
  `entries` on a complete tie), and reports negative `remaining` on overflow.
  Unassessed routes are not assigned fake utilization.
- `top_blockers` aggregates diagnostic codes across selected routes, sorts by
  descending route count then code, and keeps only the fixed bounded prefix.
- `top_contributors` aggregates the existing body-free card contribution
  receipts by `card_id`/kind, sums route/entry/byte counts, sorts by bytes then
  IDs, and keeps only the fixed bounded prefix. It must not add bodies,
  snippets, paths, or evidence.
- `next_safe_action` is generated by a deterministic decision table. For an
  entry/byte overflow it reports the exact excess and the highest-contributing
  eligible card when the card is safely narrowable. For route-card-cap or
  required-only overflow it reports `review_required_authority` and names the
  route/diagnostic without telling the operator to remove a guardrail. For an
  unassessed generative job it reports `declare_context_budget`. For a green
  result it reports `none` or `review_tightest_route` with no remediation
  pretending that a change is required.

### Identity compatibility

The existing route object key is `job`, while `requirements`, run contracts,
and surrounding public contracts use `job_id`. The implementation should make
`job_id` canonical everywhere new code reads or writes route identity:

- Add `job_id` to every full route record and use it in summary/query,
  contributor, blocker, human, schema, docs, and tests.
- Retain `job` in `mdp.route-budget.v0` for one compatibility window and set it
  to exactly the same string as `job_id`. Mark it deprecated in the schema and
  docs; do not allow the two fields to diverge.
- Update `merge_route_budget_preflight` and any internal readers to prefer
  `job_id` and fall back to `job` only when reading a legacy fixture.
- Do not rename unrelated route/context fields in this ticket. The full
  contract remains machine-readable and `source_contract` makes the summary's
  provenance explicit.
- Add `schema route-budget` (v0 full) and `schema route-budget-summary-v1`
  targets only if the existing schema registry can expose them without
  changing unrelated schema names. If the CLI cannot retain a v0 schema target
  without an undocumented alias, record the exact compatibility mapping in the
  schema description and preserve the old output fixture.

## Planned implementation surfaces

| File | Existing symbols / responsibility | Planned change |
|---|---|---|
| `cli/src/cli.rs` | `Commands::RouteBudget`, `SchemaTarget`, parser/help tests | Add optional `--job` and `--persona` selectors with explicit exact-match help. Keep `--strict`; reject contradictory future projection flags if one is added. Add explicit route-budget schema target(s) and parser coverage. |
| `cli/src/app.rs` | `Commands::RouteBudget` dispatch; `merge_route_budget_preflight` | Pass a typed filter/query only for the direct command. Keep validate's internal call unfiltered. Read `job_id` first with a legacy `job` fallback when merging issues; preserve manifest paths, diagnostics, and cap-blocking behavior. |
| `cli/src/commands/routing.rs` | `route_budget_preflight_command` and strict warning construction | Accept query filters, validate exact IDs before projection, attach query metadata, apply strict warnings before summary projection, and expose one sanitized filter-not-found error. Keep the full unfiltered result for `validate`. |
| `cli/src/routing.rs` | `route_budget_preflight`, `context_minimality`, `largest_contributing_cards`, route-budget tests | Keep evaluation authoritative and add `job_id` to route records. Extract a bounded projection helper that filters already-evaluated records and computes status/headroom/blocker/contributor aggregates. Extend contributor metadata only with body-free fields needed to decide whether narrowing is safe. Preserve route-card-cap, unassessed, overflow, and legacy semantics. |
| `cli/src/output.rs` | `summarize("route-budget")`, `print_summary`, `print_human` | Replace the current route-by-route summary mapping with the shared summary projection. Ensure JSON and human summary render the same fields and never serialize the full route array under `--summary`; preserve full default output and summary tests for other commands. |
| `cli/src/commands/schemas.rs` | `schema`, context/minimality schemas, schema tests | Add closed schemas for full route-budget compatibility and `mdp.route-budget-summary.v1`, including query, percentages, status counts, bounded arrays, safe-action variants, and deprecated `job` alias rules. Ensure no summary schema allows route arrays/bodies. |
| `cli/src/commands/capabilities.rs` | `route-budget` command descriptor and stable error codes | Advertise `--job`/`--persona`, summary/full behavior, canonical `job_id`, summary contract, and the bounded `route_budget_filter_not_found` diagnostic. Keep read-only side effects and existing `mdp.route-budget.v0` full contract visible. |
| `cli/src/models.rs` or a focused route-budget module | Existing `JobContextBudget` and route value construction | Prefer a small typed query/summary model or private structs over stringly duplicated aggregation. Do not change manifest budget schema or job identity authority. |
| `scripts/build-route-budget-fixtures.mjs` | Public synthetic overflow/ready pack generator | Add deterministic personas/jobs to produce at least 12 route combinations and retain separate overflow/ready cases. Keep every value synthetic and no customer/provider/private content. |
| `Makefile` | `validate-route-budget` recipe | Add full/summary/job/persona/intersection invocations, assert no `routes` in summary, bounded byte length, percentages, safe-action shape, and `job_id` alias equality. Keep existing overflow/ready/route/brief checks. |
| `examples/route-budget/README.md` | Public fixture explanation | Document compact summary, exact selectors, full-mode opt-in/default, `job_id` compatibility, and safe narrowing/no-truncation behavior. |
| `docs/minimal-context-routing.md` | Current route-budget operator contract | Explain the two projections, status/headroom metrics, exact selectors, overflow next-action decision table, and legacy `job` alias. Preserve the rule that guardrails are never dropped. |
| `README.md`, `llms.txt`, `llms-full.txt` | CLI inventory and agent-readable command examples | Add concise route-budget examples and state when to use full output versus summary/filter. Keep generated inventory syntax synchronized with `readme-check`. |
| `plugin/skills/mdp-pack-builder/SKILL.md` | Pack authoring strict route-budget gate | Teach agents to run an unfiltered strict gate for release readiness, then use `--summary`, `--job`, or `--persona` for bounded triage. Require narrowing structured applicability, not truncation or budget inflation. |
| `plugin/skills/mdp-pack-review/SKILL.md`, `references/routing-evals.md`, `references/installed-template-qa.md` | Pack review and installed proof workflows | Add exact selector examples, expected summary fields, body-free diagnostics, 12-route boundedness proof, and installed CLI parity checks. |
| `plugin/skills/mdp/references/cli-operator.md`, `mental-model.md` | Shared operator model | State that route-budget summary is diagnostic-only, full output remains authority, `job_id` is canonical, and filters are exact projections. |
| `plugin/skills/mdp-gtm-brief/references/outbound-copy-review.md` | Job-specific route review | Replace any assumption that agents must consume the full matrix with the bounded summary/filter workflow and stop on blocked/near-budget status. |
| `scripts/release-install-smoke.sh`, `scripts/test-release-install-smoke.sh` | Installed CLI/plugin proof | Run the installed binary against the synthetic 12-route pack, compare summary/full/filter contracts to source expectations, and prove installed skill examples use the same flags. Keep output files scratch-only. |
| `scripts/mdp-run-mcp-server.mjs`, `scripts/mdp-proposal-mcp-server.mjs`, relevant MCP tests | Existing MCP introspection/transport boundary | Do not add route-budget evaluation. If their `*_tools` result, instructions, or resources mention route-budget, update the description/schema to point to the CLI contracts and exact selectors; assert returned data is copied from CLI authority. If no such resource exists, record the absence in tests/docs rather than inventing a duplicate tool. |

## Ordered implementation steps

### 1. Freeze contracts and characterize current output

- Capture current full, summary, strict, overflow, ready, and installed outputs
  from synthetic packs without committing generated JSON or private paths.
- Add failing parser/schema/summary tests for a 12-route matrix, exact filter
  combinations, route-array absence in summary, fixed status counts, percentages,
  and output-size ceiling.
- Define the private query type, canonical sort keys, summary max lengths, safe
  action variants, and `job_id`/`job` alias invariant before touching the
  evaluator.
- Confirm `validate --strict` continues to receive the complete unfiltered
  route set and that MDP-233's blocked relation is not part of this code diff.

### 2. Add exact query plumbing without changing evaluation

- Add `--job` and `--persona` to `Commands::RouteBudget` and pass them through
  `app.rs` to `route_budget_preflight_command`.
- Resolve filters against manifest-owned IDs/labels before evaluating or
  project after a single full evaluation; choose the implementation that keeps
  error behavior deterministic and avoids a second route loop. Do not allow
  filter values to change the default scope, budgets, route-card cap, or
  applicability semantics.
- Keep the internal `route_budget_preflight(root, manifest)` call available for
  validation and tests. Apply query selection only at the command/projection
  boundary, never inside `entry_route_scoped`.
- Emit `job_id` plus the equal legacy `job` in full route records and update
  readers to prefer `job_id`.

### 3. Implement one bounded summary projection

- Build summary from the already-evaluated selected routes, not from card files
  or route bodies. Use stable sorting and integer arithmetic for percentages,
  negative overflow headroom, zero-count statuses, and ties.
- Aggregate diagnostics and body-free contributor metadata with fixed limits.
  Preserve only IDs, kinds, counts, bytes, route identities, and reason codes.
- Implement the safe-action decision table. A target card is eligible only when
  the route receipt proves it is not a universal guardrail or required-only
  authority. Required-only and cap displacement cases return review guidance,
  never removal/truncation guidance.
- Return `mdp.route-budget-summary.v1` from summary mode while retaining the
  full v0 result under default/full mode and the internal validation path.

### 4. Align human/JSON/schema/capability surfaces

- Make `output.rs` use the shared summary projection for both JSON and human
  output. Human text may be formatted for scanning, but the same values and
  bounded arrays must be present; no human-only inference is allowed.
- Add the schema targets and live schema tests. Ensure summary schemas reject
  route arrays, entry bodies, unknown diagnostic/action fields, and mismatched
  `job`/`job_id` values.
- Update `capabilities`, CLI `--help`, `README`, `llms*`, and minimal-context
  docs with exact command examples and compatibility language.
- Check MCP wrapper descriptions/resources. Keep the CLI as authority and add
  a parity test only where an existing MCP surface can legitimately expose the
  command; do not create a new remote or hosted resource.

### 5. Prove synthetic, installed, and skill parity

- Expand the public fixture generator to a deterministic 12-route matrix with
  at least one ready route, one near-budget route, one entry/byte overflow, one
  unassessed legacy job, and one route-card-cap blocker.
- Update focused Rust/Node/Make tests for all projection combinations and
  assert summary byte size remains below a fixed bound with no route arrays,
  bodies, paths, or raw synthetic card prose.
- Run skill validators and the installed release smoke. Compare source and
  installed `--help`, capabilities, schema, summary, full output, and filtered
  projections. Ensure package mirrors are updated only when they are generated
  or repository-owned sources of truth.

### 6. Review and hand off implementation

- Run `git diff --check`, focused route/schema/output tests, and full repository
  validation. Review the diff for hidden route truncation, accidental budget or
  selector changes, output leakage, and changes to MDP-233 relation/state.
- Record any unavailable network-dependent gate exactly. This plan does not
  open a PR, mutate Linear labels/status/delegation, add Blocks branding, or
  merge runtime changes.

## Verification contract

| Gate | Command/proof | Coverage |
|---|---|---|
| Parser/help | `cargo test --manifest-path cli/Cargo.toml route_budget` plus `mdp route-budget --help` and `mdp capabilities` snapshots | Exact `--job`/`--persona`, strict/full/summary descriptions, read-only boundary |
| Rust route projection | `cargo test --manifest-path cli/Cargo.toml route_budget_preflight` and focused `output`/`schema` tests | 12-route filtering, status counts, headroom percentages, deterministic ordering, alias equality, safe actions |
| Schema compatibility | `mdp schema route-budget` and `mdp schema route-budget-summary-v1` live-output validation | v0 full authority, v1 summary closed shape, no summary arrays/bodies, job identity mapping |
| Synthetic CLI matrix | `make validate-route-budget` with extended assertions | Overflow/ready/near/unassessed/cap routes, full vs summary vs job/persona/intersection, byte bound |
| Validate merge parity | Existing `cargo test` for `merge_route_budget_preflight`/strict validation and `mdp --json validate --strict` synthetic fixture | Unfiltered route matrix remains complete; MDP-224 cap errors and MDP-233 dependency are untouched |
| Node/MCP syntax/parity | `node --check` affected wrappers; existing MCP tests plus any new route-budget metadata test | No duplicate evaluator, consistent contract descriptions, no raw route/body leakage |
| Skills/docs/package | `python3 scripts/validate-skill-contracts.py`, skill packaging/eval gates, `make validate-llms`, `make validate-public-artifacts` | Installed examples and generated agent-readable docs match flags/contracts |
| Installed proof | `bash scripts/test-release-install-smoke.sh` or repository-supported local equivalent | Installed CLI and skill bundle produce the same bounded summary and exact filtered projections |
| Full repository gate | `make validate` | All Rust, Node, template, skill, packaging, public-artifact, installer, and route-budget gates |
| Static safety review | `git diff --check`; inspect generated outputs and diff | No private/customer data, no truncation, no unsafe guardrail advice, no MDP-233/Blocks metadata mutation |

If a network-dependent Pluxx or installer gate cannot run, report its exact
command and failure. Do not replace it with a fabricated pass or commit
generated `/tmp` output.

## Dependencies, risks, and blocker awareness

### Dependencies and sequencing

- **MDP-233 — hard blocker:** Existing Linear `blockedBy` relation remains
  authoritative. MDP-235 must not remove, resolve, relabel, delegate, or
  reinterpret it. Implementation should rebase on the final MDP-233 routing
  semantics before asserting route-count parity, because universal empty
  selectors can change which routes are counted.
- **MDP-239 — execution index:** Parent ordering and completion contract remain
  read-only context. This plan supplies an implementation-ready child artifact;
  it does not delegate or update the whole index.
- **MDP-65 — generated-route budget gate:** Reuse existing overflow/near-budget
  diagnostics and strict validation merge. Do not replace preflight with a
  summary-only check.
- **MDP-224 — route-card cap:** Preserve cap receipts and blocking. Summary may
  aggregate the diagnostic, but must not suggest increasing/truncating around
  an authority exclusion without review.
- **MDP-228 — required-first allocation:** If it lands before implementation,
  consume its stable allocation receipts and preserve required-first semantics;
  do not duplicate allocation or make summary guidance assume optional entries
  can always be removed.
- Existing minimal-context contract in `docs/minimal-context-routing.md` and
  `mdp.context.v0` remains the byte/entry authority; this issue only projects
  it for route-budget diagnosis.

### Risks and mitigations

- **Summary still grows with route count:** Use fixed-size blocker/contributor
  prefixes, scalar aggregates, no route array, and a regression byte ceiling on
  a 12-route and a larger generated matrix.
- **Filter changes validity:** Evaluate once, project after evaluation, and
  preserve top-level validity semantics. A filtered result reports the selected
  matrix and query; strict validation remains unfiltered.
- **Job identity drift:** Make `job_id` canonical, require `job == job_id` for
  v0 alias output, update all readers, and add schema/fixture checks before
  changing docs.
- **Unsafe remediation:** Mark guardrails/required authority in the internal
  contributor decision, and return `review_required_authority` when no safe
  narrowable target exists. Never recommend truncation or dropping required
  output/safety context.
- **MDP-233 route-count churn:** Keep blocker relationship and state intact;
  re-run the route matrix after MDP-233 rather than snapshotting counts as a
  permanent product invariant.
- **Schema consumer breakage:** Keep the v0 full payload/additive alias and
  version the summary projection. No field is silently removed in this issue.
- **MCP authority duplication:** Do not implement route-budget calculations in
  JavaScript MCP wrappers. Any resource/tool metadata must call or describe the
  Rust CLI contract and have parity tests.
- **Output/privacy leakage:** Contributors and next actions use IDs/kinds/counts
  only. Tests assert no `body`, `entries`, path, raw card text, source snippet,
  customer input, or provider data appears in summary/human/MCP projections.

## Compatibility and rollback

### Compatibility contract

- `mdp.route-budget.v0` full JSON remains available and retains existing top
  level counters, route arrays, diagnostics, cap receipts, and legacy unbudgeted
  route behavior. `job_id` is additive; `job` remains an equal deprecated alias.
- `mdp.route-budget-summary.v1` is a new bounded projection used only when
  `--summary` is requested. Consumers that need full route authority continue
  using default/full mode or an explicit filter without summary.
- `validate --strict` remains unfiltered and continues to map route-budget
  failures into the existing manifest issue paths/codes. `--job`/`--persona`
  are diagnostic selectors, not pack-authoring selectors and do not alter pack
  state.
- Legacy jobs without `context_budget` remain `unassessed`; they do not gain
  fake percentages or become invalid solely because summary mode is used.
- Existing strict warnings, route-card cap diagnostics, minimality digests,
  selected/excluded counts, and no-body guarantees remain unchanged except for
  additive identity/summary fields.
- Human output is a rendering of the same bounded summary values. It does not
  claim that compact output is a different authority or that a safe action was
  completed.
- Installed artifacts and skill examples must be updated together at release;
  source-only docs are not completion proof for changed CLI behavior.

### Rollback

- Revert the single implementation commit/PR if summary or filter consumers
  regress. No manifest migration, persisted database, remote resource, or
  destructive file operation is introduced.
- During rollback/roll-forward, operators can use default full
  `mdp --json route-budget` output; no route authority is lost and no pack
  content needs rewriting.
- Do not solve a rollout problem by deleting the `job` alias, truncating route
  arrays, raising budgets, or adding an MCP-side fallback. A future identity
  removal or new MCP resource requires a separately reviewed contract.

## Acceptance mapping

| MDP-235 acceptance criterion | Planned implementation and proof |
|---|---|
| `--summary` emits a true rollup: validity, route counts by status, tightest headroom, top blockers/contributors, next safe action | Shared bounded projection in `routing.rs` consumed by `output.rs`; summary schema and Rust tests assert all fields and fixed ordering with no `routes` array. |
| `--job <id>` and `--persona <id>` provide deterministic projections | Exact manifest-owned filter query in `cli.rs`/`commands/routing.rs`; matrix tests cover job-only, persona-only, AND, case normalization, unknown selectors, and stable canonical labels. |
| Full route arrays require full mode or an explicit selector | Default no-summary output remains explicit/full authority; summary mode never maps all routes. Tests assert route arrays appear only in full/selected full projections. |
| Job identity naming is consistent as `job_id`, or a versioned compatibility plan explains retained `job` | `job_id` is canonical in new code, summary, schemas, filters, docs, and readers. v0 full output keeps equal `job` alias with deprecation/schema/test mapping; removal is deferred to a future version. |
| Summary includes entry and byte utilization percentages and identifies which limit fails next | Integer-derived `tightest_headroom` evaluates both dimensions, returns used/limit/remaining/percentage and deterministic dimension choice; overflow tests cover entry-only, byte-only, and both. |
| Overflow output reports the minimum deterministic adjustment/narrowing target without unsafe truncation | Safe-action decision table reports exact excess and eligible top contributor; required-only/cap cases return review guidance and explicit `do_not` constraints. Tests assert no truncation/drop-guardrail language and no body leakage. |
| Human, JSON, CLI help, MCP resources, and installed skill examples agree | `output.rs`, `capabilities.rs`, parser help, schemas, docs, skills, installed smoke, and existing MCP metadata/resource tests all point to the same v1 summary/v0 full behavior. No route-budget duplicate MCP evaluator is introduced. |
| Regression tests bound summary size for multi-persona packs | 12-route synthetic fixture plus larger stress matrix; `make validate-route-budget` and Rust/Node tests assert fixed summary byte ceiling, no route arrays, and no entry bodies. |

## Definition of done

- `--summary` is materially smaller than the full route matrix and bounded by a
  tested ceiling independent of route count within the supported summary cap.
- Exact `--job`/`--persona` projections are deterministic, body-free, and
  cannot affect route evaluation or `validate --strict`.
- `job_id` is canonical and the retained v0 `job` alias is explicit, equal, and
  covered by compatibility tests.
- Headroom percentages, blocker/contributor aggregates, and safe next actions
  are generated from existing route authority and never recommend unsafe
  truncation or guardrail removal.
- Full output, schemas, human output, CLI capabilities/help, docs, skills,
  MCP metadata/resources (where present), synthetic fixtures, and installed
  artifacts agree.
- Focused tests and `make validate` pass, or unavailable gates are reported
  exactly.
- MDP-233 remains the existing blocker/state/relation; MDP-239 remains the
  parent index; no Blocks branding, `delegate:blocks`, status, label, or
  delegation mutation is made by this plan.
