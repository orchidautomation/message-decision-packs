---
title: "fix: Return field-level diagnostics for sanitized clean-run policy blocks"
type: fix
date: 2026-08-21
execution: code
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-227
linear_issues:
  - MDP-227
  - MDP-239
---

# Field-Level Diagnostics for Sanitized Clean-Run Policy Blocks

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Make a sanitized `mdp.run-execution.v1` policy block explain which bounded stage, gate, logical input, and contract-safe field caused the refusal. |
| Authority | The Rust CLI remains the sole source-authority owner. Diagnostics are an additive explanation carried inside the CLI-owned canonical authority block; MCP returns that block unchanged and adds no authority of its own. |
| Compatibility | Keep `mdp.run-execution.v1`, terminal states, `authority`, and existing `reason_codes` stable. Add an optional, closed `authority_block.diagnostics` carrier and require at least one bounded diagnostic on `no-draft:policy-blocked`; successful runs and unrelated legacy failure classes need not emit it. |
| Dependency gate | MDP-226 must provide the canonical routed-context schema/readiness decision first. MDP-227 may be planned in parallel, but its implementation must not restore the top-level `status`/`draft_status` assumption or duplicate MDP-226's readiness authority. |
| Product boundary | No provider call, source-content disclosure, private-path disclosure, CRM action, MCP-side classification, or new orchestration surface enters this work. |
| Stop condition | The CLI emits actionable but bounded policy diagnostics, canonical routed-context mismatches remain fail-closed, direct CLI and MCP payloads are identical, redaction tests pass, and the repository validation gate passes. |

## Problem Frame

The current generative input gate in `cli/src/run_runtime.rs` parses a staged
routed-context file and requires top-level `status == "ready"` (and rejects a
non-ready `draft_status`). The canonical `mdp.routed-context.v1` schema is a
closed model-visible projection with no such top-level fields. The result is a
false policy block for a correctly emitted canonical artifact, while a malformed
or stale artifact is reduced to a generic reason code.

The public failure boundary in `cli/src/commands/run.rs` currently downcasts a
`RunFailure` to only its kind and reason code. That preserves no-draft safety but
loses the deterministic distinction between malformed JSON, a wrong contract,
a missing or disallowed field, a readiness veto, a stale binding, and an
internal contract mismatch. The implementation must preserve the MDP-210
authority rule: a diagnostic may explain a source-owned block, but it cannot
replace the source reason, upgrade disposition, or make surrounding commentary
authoritative.

## Product Contract

### Public diagnostic shape

Add `authority_block.diagnostics` to the existing canonical authority block as
an optional closed array. A policy-blocked result must contain at least one
entry. The shape is intentionally structured and does not contain a free-text
message:

```json
{
  "stage": "generative-preflight",
  "gate": "routed-context-schema",
  "code": "wrong-contract",
  "input": "routed_context",
  "field": "/contract",
  "expected": {"kind": "contract", "value": "mdp.routed-context.v1"},
  "observed": {"kind": "contract", "value": "missing"}
}
```

The implementation must enforce these invariants in both the builder and the
schema tests:

- `stage` and `gate` are stable allowlisted identifiers. At minimum, cover
  `run-preflight`/`policy`, `generative-preflight`/`routed-context-schema`,
  `generative-preflight`/`routed-context-readiness`,
  `generative-preflight`/`declared-inputs`, and
  `source-integrity`/`declared-input-immutability`.
- `code` is an allowlisted category, including exactly
  `malformed-json`, `wrong-contract`, `missing-required-field`,
  `disallowed-field`, `readiness-failure`, `stale-binding`, and
  `internal-contract-mismatch`. Existing `reason_codes` remain the stable
  authority reason and are not replaced by these categories.
- `input` is either `null` for a run-level gate or a sanitized logical input
  name such as `routed_context`; it is never a filesystem path, source locator,
  request body, credential name/value, or model content.
- `field` is either `null` or a bounded contract-field pointer such as
  `/contract`, `/job`, or `/entries`; it is not a source path. Unknown or
  suspicious keys use an allowlisted unknown-field marker rather than echoing
  attacker-controlled text.
- `expected` and `observed` use a small closed value vocabulary for contract
  IDs, safe field IDs, JSON types, readiness states, binding states, and
  bounded non-negative counts. They do not carry raw JSON, parser messages,
  arbitrary strings, source hashes that are not already public authority, or
  filesystem details.
- Bound diagnostic count, field/name length, value length, and serialized
  diagnostic bytes. Emit one deterministic primary diagnostic plus only
  additional bounded diagnostics that are independently useful; never dump a
  validator error list.

The policy-blocked authority block still has `decision: null`, null artifact
hashes, no partial output, and the same `no-draft:policy-blocked` terminal
state. The MCP wrapper must continue to return the CLI object without adding,
rewriting, or summarizing `diagnostics`.

### Classification contract

Classify only facts deterministically known at the boundary. The first matching
classification is stable and must not depend on `jsonschema`'s free-text error
format or map ordering:

| Category | Deterministic source | Sanitized evidence |
|---|---|---|
| `malformed-json` | Staged routed-context bytes cannot be parsed | `input: routed_context`, `field: null`, observed state `malformed` |
| `wrong-contract` | Missing or non-`mdp.routed-context.v1` contract, or a typed request/schema contract mismatch | Safe contract value or `missing` |
| `missing-required-field` | One of the canonical routed-context required fields is absent | The allowlisted field ID and observed state `missing` |
| `disallowed-field` | Closed-object validation finds an undeclared field | Safe known field ID or the unknown-field marker; never the raw key when unsafe |
| `readiness-failure` | MDP-226's canonical readiness result, or a known job/selection policy veto | Bounded readiness/binding state; never a copied gap body or private card text |
| `stale-binding` | Canonical job/pack/receipt binding or staged-source immutability is known to differ | Logical input plus `binding: mismatch`/`changed`, never a path or source bytes |
| `internal-contract-mismatch` | The runtime cannot safely establish the expected internal contract or classify a failure | Generic bounded `unavailable` state only |

For non-routed policy failures, centralize a reason-code-to-stage/gate/category
mapping in the runtime failure constructor. Unmapped policy failures receive a
safe `internal-contract-mismatch` diagnostic rather than exposing an `anyhow`
message. Preflight and runner-failed results may retain their current shape;
they must never gain less-safe error text as a side effect.

## Scope and Non-Goals

### In scope

- Threading a bounded diagnostic value from typed runtime policy failures to
  `failure_result`.
- Replacing the routed-context readiness check with schema/binding/readiness
  checks that consume MDP-226's canonical result and do not inspect invented
  top-level readiness fields.
- Updating the canonical authority-block JSON Schema and focused Rust/Node
  conformance fixtures.
- Proving direct CLI/MCP parity and privacy-safe omission of body, path,
  credentials, and partial output.
- Updating the clean-run receipt, host-conformance, CLI operator, and authority
  conformance documentation to describe the new carrier and limits.

### Out of scope

- Changing `mdp.routed-context.v1` to add `status` or `draft_status`.
- Reimplementing MDP-226's canonical routed-context producer or readiness
  authority.
- Adding a new provider/driver contract, changing provider behavior, or
  publishing a receipt for a policy-blocked run.
- Returning raw staged content, parser/schema error strings, source paths,
  environment values, credentials, or private card text.
- Making MCP a second validator or changing legacy proposal-runner authority.
- Changing MDP-239's dependency graph, reopening the batch, or marking this
  issue ready-for-agent while the parent keeps Phase 0 blockers open.

## Implementation Units

### U1. Define and sanitize the runtime diagnostic value

- **Files/symbols:** `cli/src/run_runtime.rs` — `RunFailure`, `run_failure`,
  `RunFailureKind`, `validate_generative_input_gates`,
  `validate_step_inputs`, `verify_sources_unchanged`, and the existing policy
  failure call sites.
- **Steps:**
  1. Add an internal typed diagnostic/category representation with explicit
     stage, gate, logical input, field, expected, and observed values. Keep it
     internal until `commands/run.rs` constructs the public JSON block.
  2. Extend `RunFailure` to carry a bounded diagnostic list while preserving
     `kind()` and `code()` and all existing reason-code strings. Provide a
     structured constructor for known routed-context/input failures and a
     central safe fallback for existing policy call sites.
  3. Ensure `Display` and any logs remain code-only. Do not serialize an
     `anyhow` chain or a `serde_json::Value` from the staged input.
  4. Normalize logical names and contract fields through allowlists before
     they can enter the public result; cap count and byte budgets before
     serialization.
- **Checks:** Unit tests cover each category, unknown-key redaction, fallback
  mapping, bounded output, and preservation of existing `kind()`/`code()`.

### U2. Correct the canonical routed-context gate and failure propagation

- **Files/symbols:** `cli/src/run_runtime.rs` —
  `validate_generative_input_gates`, its callers in
  `validate_native_request_size_before_bundle` and `execute_generative_step`,
  plus `stage_inputs`/`verify_sources_unchanged` where a known binding mutation
  can be classified safely.
- **Dependencies:** Land or expose the MDP-226 canonical routed-context
  validation/readiness seam before implementation. Use the exact helper or
  result shape MDP-226 establishes; do not infer readiness from a field that is
  absent from `routed_context_schema()`.
- **Steps:**
  1. Parse staged bytes with a sanitized malformed-JSON path.
  2. Validate the closed `mdp.routed-context.v1` shape and classify contract,
     required-field, and disallowed-field failures deterministically before
     consulting library error text.
  3. Apply the MDP-226 canonical job/pack/readiness and stale-binding checks;
     report only the logical input and bounded contract state.
  4. Preserve fail-closed ordering: all policy diagnostics are produced before
     provider invocation, immutable bundle publication, or output authority.
  5. Map known declared-input absence/undeclared-input and source mutation
     failures to the same safe diagnostic carrier without exposing paths.
- **Checks:** A real canonical artifact emitted by `emit-brief --routed-context-out`
  passes generative preflight once MDP-226 is present. Wrong job/pack, stale,
  malformed, and blocked canonical artifacts fail before the driver and expose
  the expected category.

### U3. Project diagnostics into the canonical CLI JSON contract

- **Files/symbols:** `cli/src/commands/run.rs` — `run_request_file`,
  `preflight_refusal`, `failure_result`, and the existing policy-block test;
  `cli/src/commands/schemas.rs` — `canonical_authority_block_v1_schema`,
  `run_execution_v1_schema`, schema helpers, and schema tests.
- **Steps:**
  1. Pass the typed diagnostics from a downcast `RunFailure` into
     `failure_result`; use a safe fallback when the error is not a typed
     policy failure.
  2. Add `authority_block.diagnostics` as a closed optional array and add a
     conditional minimum of one for `no-draft:policy-blocked`. Keep all current
     terminal/authority/hash/decision invariants unchanged.
  3. Keep preflight refusal and runner-failed redaction assertions intact; no
     diagnostic may make a null output or failed authority look usable.
  4. Add schema tests for valid policy diagnostics, missing stage/gate,
     unknown properties, unbounded values, unsafe fields, and policy blocks
     without diagnostics.
- **Checks:** `mdp --json schema canonical-authority-block-v1` and
  `mdp --json schema run-execution-v1` validate the new shape; all current
  run/authority schema tests remain green.

### U4. Prove real CLI cases and MCP parity

- **Files/symbols:** `cli/src/commands/run.rs` tests,
  `cli/src/run_runtime.rs` tests, `scripts/test-run-conformance.mjs`, and
  `scripts/test-run-mcp-server.mjs`'s `fixtureCli` and canonical passthrough
  tests.
- **Steps:**
  1. Add a public-run test for a canonical routed-context artifact with a
     wrong contract or disallowed field and assert the terminal state, legacy
     reason code, stage/gate, logical input, category, and absence of paths or
     source body.
  2. Add a genuinely blocked routed-context fixture produced from the existing
     blocked-foundation/readiness path, not by adding a fake top-level status;
     assert `readiness-failure`, no driver invocation, no bundle/receipt/output,
     and no upgrade to draft authority.
  3. Add one valid canonical artifact case to prove the MDP-226 producer and
     MDP-227 consumer agree on the v1 shape.
  4. Extend the MCP fake CLI with a bounded diagnostics array and assert the
     returned `structuredContent` authority block is deep-equal to the CLI
     fixture. Keep `isError: false` for canonical no-draft data and ensure no
     `mcp_assurance`, reclassification, or raw stderr crosses the boundary.
  5. Retain existing wrong-contract, no-draft, secret, path, and partial-output
     tests as regression coverage.
- **Checks:** The direct CLI and MCP tests must make the same reason code and
  diagnostic object observable for the same request bytes.

### U5. Align operator and authority documentation

- **Files:** `docs/run-receipts.md`, `docs/host-conformance.md`,
  `docs/authority-conformance.md`, `cli/USAGE.md`, and
  `plugin/skills/mdp/references/cli-operator.md`.
- **Steps:**
  1. Document `authority_block.diagnostics` as bounded explanation, not a new
     authority source, and show the schema inspection command.
  2. Document the category/stage/gate vocabulary, logical-input-only rule,
     absent body/path/credential rule, and the unchanged no-draft semantics.
  3. State that MCP copies the CLI authority block and that host commentary or
     stderr is not authoritative.
  4. Remove any operator guidance that assumes routed-context has a top-level
     `status` or `draft_status`; point operators to the canonical v1 schema and
     MDP-226 readiness result.
- **Checks:** Markdown formatting/lint and the repository's docs/public-artifact
  validation pass without introducing private examples or stale commands.

## Exact File and Symbol Matrix

| Area | Expected implementation surface | Expected test/proof surface |
|---|---|---|
| Failure transport | `cli/src/run_runtime.rs`: `RunFailure`, `run_failure`, routed-context/input gates, source-integrity classification | Rust runtime tests for taxonomy, redaction, and fail-before-driver ordering |
| Public JSON | `cli/src/commands/run.rs`: `run_request_file`, `failure_result`; `cli/src/commands/schemas.rs`: canonical/run schemas | Rust command/schema tests and schema CLI output |
| Direct conformance | Existing run fixture helpers in `scripts/test-run-conformance.mjs` plus real `emit-brief` routed artifact command | Wrong-contract, blocked-readiness, valid-canonical, no-output, no-leak cases |
| MCP parity | `scripts/test-run-mcp-server.mjs`: `fixtureCli` and passthrough assertions; no new MCP authority logic | Deep equality of CLI authority block and MCP structured content |
| Guidance | Receipt, host-conformance, authority-conformance, CLI usage, and MDP operator reference docs | Markdown/public-artifact checks |

No change is expected in `scripts/mdp-run-mcp-server.mjs`: its existing
`callRun` contract check and CLI-data passthrough are the parity mechanism. If
the implementation changes that file, it must be limited to recognizing the
new optional field without copying or rewriting its contents, with a test that
proves byte-for-byte semantic equality.

## Ordered Execution and Dependency Handoff

1. Confirm MDP-226's landed canonical routed-context producer/schema/readiness
   contract and record its exact helper/result seam in the implementation PR.
2. Add the internal diagnostic taxonomy and sanitized `RunFailure` carrier;
   keep all existing reason codes and failure classes working.
3. Replace the incorrect routed-context top-level readiness check with the
   MDP-226 seam, including schema classification, job/pack binding, and source
   immutability checks.
4. Project the carrier into `authority_block.diagnostics`, update the closed
   schema, and preserve all authority monotonicity invariants.
5. Add Rust unit/command tests, direct conformance fixtures, and MCP passthrough
   tests before documentation edits.
6. Run focused checks, then the full repository validation. Review the final
   JSON fixtures for body/path/credential leaks and verify that only the plan's
   implementation branch would change runtime files.
7. Release the implementation only after MDP-239's Phase 0 blockers and native
   host handoff are repaired; MDP-227's plan can be handed off now, but its
   issue remains planned/backlog until that dependency state changes.

## Validation Contract

Run from the repository root after implementation. These are existing
repository commands; do not introduce a `ce-plan` or removed plan validator:

```bash
git diff --check
cd cli && cargo fmt --check
cd cli && cargo test run_runtime::tests -- --nocapture
cd cli && cargo test commands::run::tests -- --nocapture
cd cli && cargo test commands::schemas::tests -- --nocapture
cd .. && make validate-run-v1-golden
cd .. && make validate-run-conformance
cd .. && make validate-run-mcp
cd .. && make validate-authority-conformance
cd .. && make validate
```

The focused tests must assert all of the following:

- canonical routed-context v1 passes the generative preflight gate after
  MDP-226's producer/readiness contract is available;
- malformed JSON, wrong contract, missing required field, disallowed field,
  readiness failure, stale binding, and internal mismatch map to stable
  categories when deterministically knowable;
- every input diagnostic names only its logical input and safe contract field;
- expected/observed values stay in the allowlisted bounded vocabulary;
- `no-draft:policy-blocked` has no provider invocation, bundle, receipt,
  partial output authority, source content, secret, or filesystem path;
- MCP preserves the exact CLI reason code and diagnostics without adding
  transport authority.

## Risks and Mitigations

| Risk | Mitigation |
|---|---|
| MDP-226 changes the same runtime/schema functions while this plan is being prepared | Implement only after reading the landed seam; keep this plan parallel but make MDP-226 the first implementation dependency. |
| JSON-schema library error text leaks paths or becomes unstable | Inspect known fields and contract values before schema validation; never serialize validator errors; assert redaction and stable category values. |
| A diagnostic is mistaken for authority or upgrades a block | Keep `authority`, `reason_codes`, terminal state, null decision, and hash invariants unchanged; document diagnostics as explanation only. |
| Closed v1 consumers reject the additive field | Keep the field optional outside policy-blocked results, publish the updated canonical schema with the release, and retain old fields/terminal semantics. Do not silently create a second authority contract. |
| Unknown request/logical names or JSON keys contain secrets | Normalize against character/length limits and use the unknown-field marker for unsafe values; never echo paths, bodies, environment variables, or credential names. |
| One failure reaches the public boundary without typed context | Use the central safe fallback category; test downcast loss and ensure it remains no-draft rather than exposing an error chain. |
| MCP or legacy proposal surfaces mutate the new carrier | Test semantic deep equality and leave MCP/proposal surfaces as passthrough/compatibility consumers. |

## Rollback and Compatibility Notes

- This plan document has no product/runtime effect. Dropping or reverting the
  future implementation commit restores the existing reason-code-only payload
  and no-draft behavior; it must not be used to undo MDP-226's separate
  canonical producer fix.
- The implementation is additive at the authority-block field level. Existing
  consumers may continue reading `reason_codes`, terminal state, and null
  hashes; updated consumers can opt into `diagnostics`. A policy block without
  a valid diagnostic remains invalid under the updated schema, so producers
  must ship the builder and schema together.
- No receipt format, provider request, routed-context producer, or authority
  disposition changes. MDP-238 remains downstream of MDP-227, and MDP-239's
  dependency graph is preserved.
- Release/installation smoke must use the repository's existing `make validate`
  and installer targets; no private or customer data may be added to fixtures.

## Acceptance Criteria Mapping

| MDP-227 acceptance criterion | Plan implementation and proof |
|---|---|
| Policy-blocked results include a stable stage/gate identifier | U1/U3 add bounded `stage` and `gate`; runtime and schema tests assert policy blocks cannot omit diagnostics. |
| Input-specific failures name the logical input, never a raw body or secret-bearing path | U1's sanitized `input`/`field` allowlists plus U4 path/body/secret assertions. |
| Diagnostics distinguish malformed JSON, wrong contract, missing required field, disallowed field, readiness failure, stale binding, and known internal mismatch | U1/U2's deterministic classifier and U4 one-case-per-category coverage. |
| CLI and MCP expose the same reason codes and structured diagnostics | U3 preserves reason codes; U4 compares direct CLI output with MCP `structuredContent` without MCP mutation. |
| Sanitized failures omit partial output, provider credentials, private content, and unapproved filesystem details | U3 keeps null output/artifact authority; U4 retains and extends existing no-leak fixtures; docs state the boundary. |
| Tests cover canonical routed-context mismatch and at least one genuinely blocked routed context | U2/U4 use a real `emit-brief --routed-context-out` canonical artifact, a wrong-contract/field mutation, and a real blocked-readiness fixture without invented top-level status fields. |

## Readiness and Handoff

The implementation plan is ready for a hosted Codex execution after MDP-226's
Phase 0 contract is available. MDP-227 itself must remain `Backlog`/planned
while MDP-239 keeps its Phase 0 blockers open; this plan does not authorize an
implementation PR or change the parent graph. Preserve the existing
delegation metadata during the handoff.
