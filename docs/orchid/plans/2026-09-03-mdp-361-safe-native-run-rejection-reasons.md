# MDP-361: Restore safe native-run rejection reasons after output-invalid

## Goal Capsule

When a governed native model run is rejected, the operator must be able to see
one stable, bounded rejection code and the phase where it was classified — in
the runner audit, the published receipt, and the JSON command summary — without
ever retaining raw model output or private evidence. Today the code is dropped
or collapsed at several boundaries, so a received-but-invalid model output is
indistinguishable from generic execution unavailability. This plan is the
implementation authority for
[MDP-361](https://linear.app/orchid-automation/issue/MDP-361/bug-restore-safe-native-run-rejection-reasons-after-output).

Verified against current `main` (commit `7a21420`) and published tag `v0.1.110`:
`git diff v0.1.110 main -- cli/src/run_runtime.rs scripts/mdp-native-model-openai.mjs`
is empty, so the defect reproduces identically on both. Source evidence: the
1Password v0.7.0 generation canary in `orchidautomation/mdp-for-mdp`
(`docs/orchid/qa/2026-09-03-1password-v7-generation-canary.md`, branch
`codex/mdp-v7-canary-entities`, read-only reference).

This plan extends, and must not regress, the MDP-298 behavior shipped in commit
`8945522` (safe v3 normalization rejection diagnostics).

## Current Baseline and Defect Map (inspected evidence)

All five loss points below are confirmed in source. `no-draft` terminal states
are defined in `cli/src/run_contracts.rs` (`TerminalState`, lines ~78–88) and
enumerated in the schema projection in `cli/src/commands/schemas.rs` (~1472).

### D1. Driver fault misclassification — `scripts/mdp-native-model-openai.mjs`

`class DriverFault` (line ~21) defaults every fault to
`no-draft:runner-failed`. Faults where the provider **responded** but the
received output is unusable — `model_incomplete` (~358), `model_refusal`
(~364), no-text `provider_response_invalid` in `extractOutputText` (~369),
`model_output_too_large` (~493), `model_output_invalid_json` (~498), and
`model_output_too_deep` (via `validateJsonComplexity`, ~211) — therefore
terminate as generic runner failures instead of received-but-invalid output.
True transport/unavailability faults — `provider_timeout`,
`provider_transport_error` (~429–430), `provider_http_error` (~433),
`runtime_fetch_unavailable` (~412), `provider_response_headers_too_large`
(~377), `provider_response_too_large` (~380/384/396) — correctly remain
runner-failed. This is canary observation 3, second attempt: requested model
`gpt-5.6` ended `no-draft:runner-failed` with no provider observation and no
way to tell unavailable vs rejected vs pre-transport failure.

### D2. Runner passthrough drops the driver's code — `cli/src/run_runtime.rs`

In `execute_generative_step_with_deadline` (~2715), the non-success driver
result branch (`if !result.terminal_state.is_success()`, ~2862) builds the
`GenerativeOutcome` with `diagnostic_code: None`, discarding
`result.diagnostic_code` even though the driver result contract
(`DriverResultV2.diagnostic_code`) carried a stable code. The
`validate_driver_result` failure branch (~2848) also emits `diagnostic_code:
None`. Result: runner-failed receipts/audits carry no reason. This is the
direct cause of the canary's "no phase-specific explanation".

### D3. Host-envelope sanitizer collapses its own codes — `cli/src/run_runtime.rs`

`sanitized_host_envelope_diagnostic` (~3048) preserves `v3-*` codes (MDP-298)
and an explicit allow-list, but `host_wrap_v3_normalization_output` (~3880)
emits `normalization-host-envelope-metadata-missing` and
`normalization-host-envelope-metadata-invalid`, which are **not** in the list
and collapse to the generic `host-envelope-failed`.

### D4. Validation diagnostic keeps only a code, no phase — `cli/src/run_runtime.rs`

`sanitized_prompt_validation_diagnostic` (~3073) keeps the first allow-listed
issue code (or falls back to `prompt-output-validation-failed`) and drops phase
context. The runner marks `DeadlinePhase::Driver` / `DeadlinePhase::Validation`
around the generative step, but that phase knowledge is not attached to the
diagnostic.

### D5. Receipt and JSON summary never carry the code

- `RunReceiptV1` (`cli/src/run_contracts.rs` ~643) has no `diagnostic_code`;
  the code lives only in `runner-audit.json`, which the receipt references by
  hash. Receipt-only consumers cannot see the reason.
- The completed-run `authority_block` built in `execute_transaction`
  (`cli/src/run_runtime.rs` ~1575) and the `RunExecution` envelope (~1600)
  carry only the generic authority reason (`run-output-invalid` /
  `run-runner-failed` from `SourceAuthority::from_run`, `cli/src/authority/mod.rs`
  ~257–263).
- The actionable diagnostics projection (`cli/src/diagnostics.rs`) never
  collects a scalar `diagnostic_code`; for run commands with no collected
  issues it falls back to `fallback_code("run") == "execution_unavailable"`
  (~447) and classifies `runner-failed` as `Transient` / phase `execution`
  (`diagnostic_class` ~301, `phase` ~337). A received invalid output is
  therefore mislabeled as generic execution unavailability — exactly the
  acceptance criterion MDP-361 forbids.
- `failure_result` in `cli/src/commands/run.rs` (~160) emits empty diagnostics
  for `RunnerFailed`/`Preflight` kinds (only `PolicyBlocked` carries them).

## Implementation Units

### 1. Driver fault classification (D1) — `scripts/mdp-native-model-openai.mjs`

Give received-but-invalid faults the terminal state `no-draft:output-invalid`:
`model_incomplete`, `model_refusal`, `model_output_too_large`,
`model_output_invalid_json`, `model_output_too_deep`. Split the no-text case in
`extractOutputText` (~369) into a new bounded code `model_output_missing`
(received a completed response with no usable text) classified
`no-draft:output-invalid`, keeping `provider_response_invalid` for
transport/body-read failures (~382, ~437) as `no-draft:runner-failed`.
Transport, availability, size-cap, and request faults keep their current
terminal states. Final classification table (implement as a single explicit
mapping so tests can assert it):

| Fault code | Terminal state |
|---|---|
| `model_incomplete`, `model_refusal`, `model_output_missing`, `model_output_too_large`, `model_output_invalid_json`, `model_output_too_deep` | `no-draft:output-invalid` |
| `provider_timeout`, `provider_transport_error`, `provider_http_error`, `runtime_fetch_unavailable`, `provider_response_headers_too_large`, `provider_response_too_large`, `request_too_large`, `provider_request_too_large`, `driver-start`-side faults | `no-draft:runner-failed` |
| `native_model_calls_not_allowed`, `openai_api_key_missing`, `dry_run_complete` | `no-draft:policy-blocked` (unchanged) |
| `output_schema_projection_unsupported`, `request_invalid` | `no-draft:preflight-refused` (unchanged) |

`emptyResult` continues to emit `output: null` and `provider_observation: null`
for faults; no raw content is retained.

### 2. Runner passthrough preservation (D2) — `cli/src/run_runtime.rs`

In the non-success driver result branch of
`execute_generative_step_with_deadline`, set `diagnostic_code:
result.diagnostic_code.clone()` (and keep the existing hash/observation
passthrough). In the `validate_driver_result` failure branch, emit the bounded
static code `driver-result-invalid` instead of `None`. Driver-invocation
`Err` paths already route through `failed_generative_outcome` with a code;
leave them unchanged.

### 3. Host-envelope sanitizer completion (D3) — `cli/src/run_runtime.rs`

Add `normalization-host-envelope-metadata-missing` and
`normalization-host-envelope-metadata-invalid` to the preserved allow-list in
`sanitized_host_envelope_diagnostic`. They are static, host-owned codes and
safe to preserve. Do not widen the list further; unknown codes still collapse
to `host-envelope-failed`.

### 4. Stable code plus phase in generative outcomes (D4) — `cli/src/run_runtime.rs`

Add a `diagnostic_phase: Option<String>` to `GenerativeOutcome` (~2677),
reusing the existing `DeadlinePhase` vocabulary (`driver`, `provider`,
`validation`, `finalization`, `cancel` as already stringified by the deadline
observation code). Assign:

- driver-invocation errors, `validate_driver_result` failures, and non-success
  driver results → phase `driver`;
- host-envelope wrap failures (`host_envelope_failure_outcome`) and
  prompt-output validation failures → phase `validation`;
- deadline timeouts keep using the existing deadline observation phase.

`sanitized_prompt_validation_diagnostic` keeps its current allow-list and
fallback behavior (MDP-298 contract); only the phase is added alongside it.

### 5. Receipt, audit, and JSON summary projection (D5)

- `cli/src/run_contracts.rs`: add `#[serde(default)] pub(crate)
  diagnostic_code: Option<String>` and `#[serde(default)] pub(crate)
  diagnostic_phase: Option<String>` to both `RunnerAuditV1` (~606) and
  `RunReceiptV1` (~643). `diagnostic_code` already exists on the audit; add
  only the phase there. Populate both in the audit/receipt emission sites in
  `cli/src/run_runtime.rs` (~2540–2640). Receipts remain self-hashed; the hash
  covers the new fields naturally.
- `execute_transaction` authority_block (~1575) and the `RunExecution` envelope
  (~1600): include `diagnostic_code` and `diagnostic_phase` (omit when `None`)
  so the JSON command result and `mdp.canonical-authority-block.v1` carry the
  reason inline.
- `cli/src/commands/schemas.rs`: update the `run-receipt-v1`,
  `runner-audit-v1`, and `run-execution-v1` schema projections for the new
  optional fields so `verify_run_files` and `mdp schemas` stay truthful.
- `cli/src/commands/run.rs` `failure_result` (~160): for `RunnerFailed` and
  `Preflight` kinds, include the `RunFailure` code diagnostics (bounded static
  codes) instead of an empty array; keep the existing sanitization boundary —
  never echo raw error text.

### 6. Actionable diagnostics classification — `cli/src/diagnostics.rs`

- For run-family commands, collect the scalar `diagnostic_code` /
  `diagnostic_phase` fields (top-level and inside `authority_block`) as raw
  codes so `diagnostics_for_result` uses the real code instead of the
  `execution_unavailable` fallback. The fallback remains only for runs with no
  observable code.
- Extend `diagnostic_class` / `phase` so the received-but-invalid family
  (`model-output-invalid-json`, `model-refusal`, `model-incomplete`,
  `model-output-missing`, `model-output-too-large`, `model-output-too-deep`,
  and the host/validation codes such as `v3-*`, `prompt-output-*`) projects as
  phase `validation`, not `Transient`/`execution`. Transport/timeout codes
  remain `Transient` / `execution`.

### 7. Consumer pass-through verification (no behavior change expected)

Verify, and only adjust if fields are filtered: `scripts/mdp-run-mcp-server.mjs`
(~1005–1025 no-draft handling), `scripts/mdp-proposal-runner.mjs` (~1146),
`cli/src/commands/decision_card.rs` (~486), and
`cli/src/commands/decision_trace` projections. These consumers pass CLI JSON
through; new optional fields should flow without edits.

## Acceptance Mapping

| Issue acceptance criterion | Units |
|---|---|
| A received invalid v3 output reports one stable bounded validation code and phase | 3, 4, 5 + tests T1/T3 |
| A pre-provider or transport runner failure reports a distinguishable safe reason when observed | 1, 2 + tests T2/T3 |
| JSON command may return fail-closed authority but must not mislabel received invalid output as generic execution unavailability | 1, 5, 6 + tests T3/T4 |
| Regression tests cover both canary cases | T1–T4 |
| MDP-298 behavior intact; private content not retained | T5, sanitizers unchanged in kind |

Out of scope: MDP-362 (`--model-context` suggestion), MDP-363 (evidence
rematerialization), MDP-364–370, any `mdp-for-mdp` mutation (read-only
reference), releases, deployments, and any change to raw-output retention
policy.

## Tests and Validation

- **T1 (canary case 1, Rust):** in the `run_runtime.rs` test module, use the
  existing mock-driver seam (as in `model_call_wrapper_failures_return_safe_no_draft_outcomes`
  and `post_bundle_driver_failure_publishes_a_safe_no_draft_receipt`, ~6427/+)
  with a driver result whose output fails v3 host wrap / prompt-output
  validation. Assert: receipt `terminal_state == no-draft:output-invalid`,
  receipt `diagnostic_code` is the stable code (not `host-envelope-failed` for
  the `normalization-host-envelope-*` cases), `diagnostic_phase ==
  "validation"`, audit matches, and no raw output bytes appear in any
  published artifact.
- **T2 (canary case 2, Rust):** mock driver returns a non-success result with
  `diagnostic_code: Some("provider-http-error")` (and separately a
  `no-draft:output-invalid` driver result with
  `diagnostic_code: Some("model-output-invalid-json")`). Assert the
  passthrough preserves the code and phase `driver`, and that the two cases
  remain distinguishable.
- **T3 (driver contract, Node):** update `scripts/test-native-model-driver.mjs`
  (~238–347) to the new classification table: refusal/invalid-json/too-large
  → `no-draft:output-invalid` with the code preserved; provider failure and
  timeout → `no-draft:runner-failed` with code preserved. Add a
  `model_output_missing` mock case.
- **T4 (summary projection):** extend `cli/tests/json_stdout_contract.rs` (or a
  focused diagnostics test) so a run envelope with `diagnostic_code:
  "model-output-invalid-json"` projects actionable diagnostics with phase
  `validation` and does not emit `execution_unavailable`.
- **T5 (MDP-298 regression):** the existing
  `v3_host_wrapper_diagnostics_preserve_only_static_reason_codes` and
  `prompt_validation_diagnostics_preserve_only_safe_local_issue_codes` tests
  must remain green unmodified.
- **Re-baseline checks:** `scripts/test-run-v1-golden.mjs`,
  `scripts/test-run-conformance.mjs`, `scripts/test-native-runner.sh`,
  `scripts/test-cold-model-conformance.mjs`,
  `scripts/test-universal-native-parity.mjs` — update goldens/assertions only
  where the intended mapping/projection changed; any other failure is a bug in
  this change.
- **Focused run before PR:** `cargo test --manifest-path cli/Cargo.toml`,
  `node scripts/test-native-model-driver.mjs`,
  `node scripts/test-run-v1-golden.mjs`,
  `node scripts/test-run-conformance.mjs`,
  `bash scripts/test-native-runner.sh`. Full CI is authoritative for the exact
  PR commit.

## Compatibility and Migration

- Terminal-state re-mapping of received-but-invalid driver faults is an
  intentional, observable contract correction (the issue's core ask). The
  `TerminalState` enum and its schema enumeration are unchanged; only which
  faults map to which state changes.
- New receipt/audit/authority-block fields are additive and optional
  (`serde(default)`; omitted when `None` in authority-block/RunExecution).
  New CLI builds read old receipts/audits via the defaults. Old CLI builds
  cannot parse new receipts (deny_unknown_fields); acceptable because receipts
  are produced and verified by the same CLI generation, and `mdp run-receipt`
  verification of prior receipts keeps working.
- Receipt hashes change shape (new fields are covered by `receipt_sha256`);
  each receipt is self-contained so no migration is required.
- Human output and MCP surfaces gain the code additively; no consumer breaks
  on additional JSON keys (verified in Unit 7).

## Risks, Safety Boundaries, Rollback

- **Risk: over-broad sanitizer widening.** Mitigation: only two
  `normalization-host-envelope-*` codes are added; everything unknown still
  collapses. Raw provider output and validation messages remain excluded.
- **Risk: golden/conformance churn masking a real regression.** Mitigation:
  re-baseline only the intended mapping/projection diffs; every other golden
  change is investigated, not accepted.
- **Risk: phase vocabulary drift.** Mitigation: reuse `DeadlinePhase` strings
  only; no new phase vocabulary.
- **Safety:** fail-closed validation is unchanged in kind; no provider call is
  authorized by this plan beyond existing tests' mock seams; no private
  evidence, raw model output, or provider error text is retained or echoed.
- **Rollout:** one PR to `main`; no release, install, or deployment (merge
  completes the task; release CI owns packaging). **Rollback:** revert the PR.

## Blockers and Readiness Verdict

No blockers. Repository routing is single-repo
(`orchidautomation/message-decision-packs`, changes expected;
`orchidautomation/mdp-for-mdp` read-only). Base branch `main`. Every acceptance
criterion maps to units and tests above.

**Verdict: READY_TO_PIN.**
