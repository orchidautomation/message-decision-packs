---
title: "fix: Report and enforce one effective deadline across MCP, runtime, and provider phases"
type: fix
date: 2026-08-21
execution: code
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-238
linear_issues:
  - MDP-238
  - MDP-239
  - MDP-227
---

# One Effective Deadline Across the Clean-Run Boundary

## Lifecycle Route

| Field | Decision |
|---|---|
| Repository | `orchidautomation/message-decision-packs` |
| Base | `main` at the clean `origin/main` source revision `2cba9919483b5a7ba46efed53e3b5502b2abf477` |
| Source branch | `codex/mdp-238-plan` |
| Host route | Direct Codex implementation handoff; this artifact is plan-only and does not authorize an implementation PR |
| Sync | `sync:pr-link-only` |
| Parent | MDP-239 remains the execution index and stays `Backlog`/planned |
| Dependency | MDP-227 remains the direct blocker and stays `Backlog`/planned; its typed sanitized diagnostic carrier must be available before implementation consumes it |
| Delegation | Preserve existing `delegate:codex`; do not restore native delegation or change labels in this planning task |

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Make one Rust-owned monotonic deadline plan observable and enforce it across transport, staging, driver, provider, validation, finalization, and cancellation. |
| Recommended default | `60_000` ms for the canonical v1 clean-run path, including `mdp_run`, because the generative execution policy currently caps the native path at 60 seconds. Operators see this default, not an unexplained menu. |
| Authority | The Rust run kernel computes the plan, starts the monotonic clock, classifies the timed-out phase, and emits the sanitized authority evidence. MCP and compatibility runners transport the plan and copy the CLI result; they do not classify provider failures or recalculate receipt authority. |
| Effective limit | `min(runtime execution_policy.timeout_ms, transport timeout after its bounded handoff reserve)`; provider/driver receive the remaining budget after the single finalization reserve. An outer value never silently extends a tighter inner policy. |
| Public evidence | Add an optional closed timeout observation to the existing v1 runner audit and canonical authority block. New timeout/cancellation receipts always include phase, elapsed time, configured limits, effective limit, outcome, and terminal state. Legacy v1 artifacts without the optional field remain readable with timeout evidence marked unavailable. |
| Failure boundary | Timeout and cancellation remain `no-draft:runner-failed` after the run bundle boundary, or sanitized preflight refusal before an immutable bundle can exist. No provider body, response text, credential, path, or partial generated output is returned or published. |
| Stop condition | Preflight exposes every configured deadline and the computed effective value; all phases use the same clock; CLI/MCP/provider tests agree on the phase and limit; recovery removes abandoned transactions; `make validate` and installed MCP/CLI smoke pass. |

## Problem Frame

MDP 0.1.73 has several independently configured timers:

1. `scripts/mdp-run-mcp-server.mjs` uses `DEFAULT_TIMEOUT_MS = 120_000` and
   `MAX_TIMEOUT_MS = 300_000` for the CLI child. It reports only `cli-timeout`
   after the process group is terminated. It does not read the request's inner
   `execution_policy.timeout_ms`, pass the outer limit to Rust, or report which
   inner phase was tighter.
2. `cli/src/run_runtime.rs::RunDeadline` starts after request parsing and uses
   the request policy as one runtime budget. It checks only at selected phase
   boundaries and returns generic `execution-timeout`. Generative execution
   subtracts `MAX_FINALIZATION_RESERVE_MS` from the remaining budget for the
   driver, but the reserve, phase, configured policy, and elapsed time are not
   part of the receipt.
3. `execute_generative_step` puts the remaining driver budget in
   `DriverProviderPolicyV2.timeout_ms`, and `invoke_native_driver` uses it as
   the subprocess supervisor limit. The bundled Node driver then starts a new
   `AbortController` timer for the same scalar and independently caps requests
   at 60 seconds. A provider timeout's stable `diagnostic_code` is not carried
   into `RunnerAuditV1` or the public failure result.
4. `scripts/lib/process-supervisor.mjs` has a separate timer and a 250 ms
   termination grace period. It correctly kills a Unix process group and
   performs exact recovery-claim cleanup, but a forced outer kill normally has
   no CLI-created receipt and exposes no canonical phase/effective-limit data.
5. `scripts/mdp-proposal-mcp-server.mjs` also defaults to 120 seconds and
   allows 300 seconds, while `mdp-proposal-runner.mjs` creates a deterministic
   clean-run request with `timeout_ms: 30_000` and runs several child phases
   with their own default process timeout. This compatibility path can make a
   120-second transport appear healthy while the inner operation stopped much
   earlier.
6. `mdp.runner-audit.v1`, `mdp.run-receipt.v1`, and
   `mdp.canonical-authority-block.v1` have no deadline observation. Existing
   limitations state only that `timeout_ms` is checked at bounded phase
   boundaries, so an operator cannot distinguish staging, provider, validation,
   finalization, transport, and cancellation outcomes.

The implementation must close the observability and enforcement gap without
turning MCP into a second runtime, weakening the run-request hash boundary, or
changing existing terminal/authority semantics.

## Product Contract

### One plan, one clock, bounded phase views

Create an internal Rust `DeadlinePlan`/`DeadlineObservation` value and a
closed public projection. The plan is created before staging from:

- the request's `execution_policy.timeout_ms` (`runtime_configured_ms`);
- an optional host-provided `transport_timeout_ms` passed by MCP or a
  compatibility adapter (`transport_configured_ms`);
- the fixed finalization handoff reserve (`transport_handoff_reserve_ms`);
- the provider/driver maximum declared by the selected execution profile
  (`provider_configured_ms`, currently the same 60,000 ms native cap).

The plan starts one `Instant` at the Rust run boundary. `remaining_ms()` and
`check(phase)` derive every later phase limit from that instant. Staging,
driver launch, provider I/O, output validation, source reread, artifact
publication, receipt verification, and cleanup do not start independent full
timers. The provider gets only the remaining runtime budget after the one
finalization reserve; the Node abort timer and Rust child supervisor receive
that same provider subdeadline, so neither can outlive the other.

The plan must distinguish configured values from effective values. For a
transport-aware call:

```text
transport_guard_ms = transport_configured_ms
runtime_effective_ms = min(
  runtime_configured_ms,
  max(1, transport_guard_ms - transport_handoff_reserve_ms),
)
provider_effective_ms = max(
  1,
  remaining_at_driver_start_ms - finalization_reserve_ms,
)
```

When no outer transport value is supplied, the runtime policy is the overall
effective limit and `transport_configured_ms` is `null`. The plan records a
warning when an outer value is larger than the tighter runtime/provider bound
(`outer-timeout-cannot-extend-inner`) and when it is the tighter bound
(`outer-timeout-truncates-runtime`). A caller may still use a deliberately
shorter outer guard, but the result must say exactly which limit won. Values
below the reserve are rejected as an invalid deadline plan rather than
silently turning the provider budget into an unexplained one millisecond run.

The exact field names and byte limits belong to the implementation's closed
schema, but the public projection must carry at least:

```json
{
  "contract": "mdp.deadline-observation.v1",
  "outcome": "timed-out",
  "phase": "provider",
  "elapsed_ms": 59842,
  "configured_limit_ms": 60000,
  "effective_limit_ms": 59750,
  "transport_configured_ms": 60000,
  "runtime_configured_ms": 60000,
  "provider_configured_ms": 60000,
  "finalization_reserve_ms": 250,
  "terminal_state": "no-draft:runner-failed",
  "warnings": ["outer-timeout-cannot-extend-inner"]
}
```

The example is synthetic and illustrative only. `phase`, `outcome`, warning
codes, and terminal states are allowlisted; elapsed and limit values are
bounded non-negative integers. The object never contains a path, provider
response, request body, stderr, environment value, secret, arbitrary error
text, or model output.

### Preflight and transport handoff

Extend the existing `mdp run` command with a read-only deadline preflight mode
that parses the exact request and returns a versioned plan without staging,
launching a driver, reading provider credentials, or creating an output
directory. The normal run computes the same object before it stages anything
and includes it in the canonical output. MCP invokes this CLI-owned preflight
with its transport timeout and then invokes the normal run with the same
transport value; MCP may display the plan but cannot rewrite it.

The transport timeout is an adapter control, not a replacement request field.
It must be passed through an explicit CLI option (for example
`--transport-timeout-ms`) rather than an ambient environment variable or a
mutated request file. The run bundle continues to hash the caller-declared
execution policy; the audit and timeout observation bind the transport value
that was actually supplied. Direct CLI callers omit the option and receive a
`transport_configured_ms: null` plan.

`mdp_run_tools` and CLI help expose `60_000` ms as the recommended canonical
default. The canonical `mdp_run` tool uses that value when omitted, validates
the same minimum/maximum as the CLI, and passes it unchanged to the child.
The legacy proposal wrapper either uses the same shared default for its
`clean_run_v1` path or explicitly labels its larger source-intake timeout as a
compatibility outer guard; it must never imply that the v1 runtime received
the larger deadline.

### Timeout, cancellation, and publication

- A timeout at staging or before the immutable run bundle is written remains
  a sanitized `no-draft:preflight-refused` result with no receipt. The
  preflight plan may be returned, but no partial authority is created.
- A timeout or cancellation after the bundle boundary transitions to
  `no-draft:runner-failed`, clears output/decision/compiled-context authority,
  writes the sanitized timeout observation into the runner audit and receipt
  when the finalization reserve is still available, and publishes only the
  verified no-draft receipt.
- A timeout while finalizing or verifying cannot publish a misleading partial
  receipt. The transaction guard and transport recovery claim remove the
  private transaction and output claim; the outer adapter returns a bounded
  transport timeout/cancellation result with no artifact authority.
- Provider aborts, Rust child termination, MCP process-group cancellation, and
  explicit host cancellation map to stable `timeout` or `cancelled` outcomes
  and the canonical no-draft terminal state. Provider/driver fault codes stay
  bounded and are not promoted to raw diagnostics.
- No success artifact, partial model output, provider response, credential, or
  private staging path is returned through CLI stdout, MCP structured content,
  receipts, or ordinary logs.

## Scope and Non-Goals

### In scope

- The shared Rust deadline plan, monotonic phase checks, driver/provider
  remaining-budget propagation, timeout/cancellation classification, and
  sanitized audit/receipt projection.
- The CLI `run` preflight and explicit transport-timeout handoff.
- The canonical run MCP adapter and process supervisor, including outer
  warning/error evidence and exact recovery cleanup.
- Proposal MCP/runner compatibility wiring where it invokes canonical v1 or
  otherwise advertises a timeout; legacy v0 semantics remain unchanged except
  for an explicit documented timeout compatibility boundary.
- Closed schemas, verifier binding, decision-trace/operator output where the
  new optional timeout object is read, and direct CLI/MCP/provider/conformance
  tests.
- Help, docs, and the canonical `mdp` skill guidance for one recommended
  default and phase-specific timeout interpretation.

### Out of scope

- New provider endpoints, retry/backoff policy, model pricing, batching,
  scheduling, remote/hosted execution, or provider-side retention claims.
- A second MCP/runtime authority or any MCP-side reclassification of CLI
  terminal state, assurance, hashes, or decision data.
- Making blocking kernel filesystem calls preemptible. Such calls remain a
  documented host/runtime limitation; the outer guard still cleans abandoned
  transactions when it can prove the exact recovery claim.
- Changing `mdp.run-request.v1`, `mdp.run-bundle.v1`, or existing terminal,
  reason-code, decision, assurance, and hash semantics except for the explicit
  transport control and additive timeout observation.
- Reading or returning provider/body content to improve timeout messages.
- Any MDP-227 implementation, MDP-226 producer change, MDP-239 status/label
  transition, native delegation, merge, release, or Blocks branding.

## Implementation Units

### U1. Define the deadline plan and failure carrier

- **Files/symbols:** `cli/src/run_runtime.rs` — `RunDeadline`,
  `RunFailure`, `run_failure`, `execute_run_inner_with_driver`,
  `classify_execution_error`; `cli/src/run_contracts.rs` — new bounded
  deadline plan/observation types and optional `RunnerAuditV1` field.
- **Dependencies:** MDP-227's typed, sanitized `RunFailure` diagnostic
  carrier must be available. Do not duplicate its redaction or expose an
  `anyhow` chain.
- **Steps:**
  1. Replace the scalar-only `RunDeadline` with a plan that records configured
     transport/runtime/provider limits, reserve, warnings, one monotonic start,
     and the current allowlisted phase.
  2. Add `remaining_ms`, `provider_budget`, `check(phase)`, and a bounded
     timeout/cancellation constructor that attaches phase/effective values to
     the MDP-227 failure carrier while preserving `kind()`, existing reason
     codes, and no-draft mapping.
  3. Use a sealed transport option supplied by the CLI adapter; validate
     integer range, reserve, overflow, and unsupported combinations before
     creating the transaction directory.
  4. Define `DeadlineObservationV1` as an additive, closed object. Keep the
     field optional for old v1 audits/receipts during deserialization, but make
     every new timed-out/cancelled receipt emit it.
- **Checks:** Rust unit tests for min-bound selection, outer tighter/looser
  warnings, absent transport, reserve underflow, monotonic elapsed bounds,
  phase allowlists, and secret/path/body redaction.

### U2. Thread one deadline through staging, provider, validation, and publish

- **Files/symbols:** `cli/src/run_runtime.rs` — `execute_transaction`,
  `validate_native_request_size_before_bundle`, `execute_generative_step`,
  `invoke_native_driver`, `supervise_child`, `validate_request`,
  `assurance_dimensions`, and receipt/audit construction.
- **Steps:**
  1. Check the same plan at pack snapshot, input/prompt staging, source
     reread, bundle sealing, and the pre-driver boundary. A pre-bundle
     timeout returns a sanitized refusal with no output directory.
  2. Pass the plan into both native preflight and `execute_generative_step`;
     set `DriverProviderPolicyV2.timeout_ms` to the bounded provider remaining
     budget, never the original outer or request value when less time remains.
  3. Make `invoke_native_driver` and `supervise_child` accept the same
     provider subdeadline/absolute deadline. Keep the process supervisor and
     provider `AbortController` bounded by one value and distinguish
     `driver`, `provider`, and `cancellation` failures.
  4. Add phase checks around output parsing/validation, source mutation
     reread, `before_post_check`, receipt construction, `verify_run_files`,
     private cleanup, and final rename. If the bundle exists and reserve
     remains, convert timeout/cancellation into a sanitized no-draft receipt;
     otherwise let recovery remove the incomplete transaction.
  5. Emit `DeadlineObservationV1` in `RunnerAuditV1`, bind the audit artifact
     to receipt/authority, and retain the existing null output/decision rules.
  6. Preserve the current limitation that a blocking filesystem syscall is
     not preempted; record `phase: staging` or `phase: finalization` only at a
     bounded checkpoint, never claim an exact interrupt point that Rust did
     not observe.
- **Checks:** Focused Rust tests force timeouts in staging, provider/driver,
  output validation, post-bundle finalization, cancellation, and cleanup;
  assert no provider invocation where preflight fails, no partial output, and
  exact receipt/audit fields when publication succeeds.

### U3. Extend CLI preflight, schemas, verification, and authority output

- **Files/symbols:** `cli/src/cli.rs` — `Commands::Run` options and parsing;
  `cli/src/app.rs` — run/preflight dispatch; `cli/src/commands/run.rs` —
  `run_request_file`, `preflight_refusal`, `failure_result`;
  `cli/src/commands/schemas.rs` — run-request, driver provider, driver result,
  runner audit, run receipt, canonical authority, run execution, and a
  `run-preflight-v1` schema target; `cli/src/commands/run_verification.rs` —
  `verify_runner_audit` and receipt verification.
- **Steps:**
  1. Add the read-only preflight result with the computed plan, warnings,
     request mode, and the recommended default. It must not read credential
     values, invoke a provider/driver, create an output directory, or expose
     source paths/body text.
  2. Add optional closed `deadline`/`timeout` projections to the authority
     block and runner audit. Keep `mdp.run-execution.v1` and receipt hashes
     domain-stable; new producers hash the additive fields as part of the
     existing receipt/audit bytes.
  3. Add schema constraints for allowlisted phases/outcomes, bounded integer
     values, warning codes, terminal state consistency, null output/decision
     on no-draft, and optional absence for legacy v1 artifacts.
  4. Verify observation/plan consistency, audit/receipt terminal state, and
     no-draft authority monotonicity. Legacy audits without a timeout field
     stay readable and report timeout evidence as unknown rather than being
     upgraded.
  5. Keep MDP-227 diagnostics and reason codes intact; timeout evidence is
     explanation/observability, not a new decision source.
- **Checks:** Schema-target tests, preflight CLI tests, receipt hash/mutation
  tests, legacy-artifact compatibility fixtures, sanitized failure tests, and
  direct CLI versus MCP deep-equality checks.

### U4. Align canonical MCP and process-supervisor semantics

- **Files/symbols:** `scripts/mdp-run-mcp-server.mjs` — timeout constants,
  tool schemas, `callRun`, `callVerifyRun`, `invokeCli`, and guardrails;
  `scripts/lib/process-supervisor.mjs` — `superviseProcess`, escalation,
  recovery and bounded termination metadata; `scripts/test-run-mcp-server.mjs`
  and `scripts/test-run-conformance.mjs`.
- **Steps:**
  1. Move default/min/max/reserve/plan helper values into a small shared
     `scripts/lib/deadline-policy.mjs` module so canonical MCP and compatible
     adapters cannot drift. Use 60,000 ms as the canonical recommended
     default; keep any larger compatibility cap explicit and warn when it
     cannot extend the Rust runtime.
  2. After the request file is frozen, read only the bounded structured
     timeout/mode fields needed for the preflight handoff. Invoke the Rust
     preflight and normal run with the same `--transport-timeout-ms`; never
     rewrite the request or pass provider/body content into diagnostics.
  3. Keep the outer process-group timer as a last-resort guard. Return stable
     `cli-timeout`/`cli-cancelled` data with bounded elapsed/phase/limit fields
     when no receipt can be published, and retain exact recovery-claim cleanup.
  4. Preserve the rule that canonical CLI data, including `deadline`, is
     returned unchanged for a well-formed run. MCP may add transport metadata
     only in its own error envelope and must not change CLI authority.
  5. Ensure `mdp_verify_run` uses the same bounded transport policy without
     confusing verification timeout with a run timeout.
- **Checks:** Outer transport shorter/longer/equal to runtime; default and
  schema help; process-group descendant cleanup; output overflow; malformed
  CLI output; cancellation; recovery claim ownership; CLI/MCP semantic parity;
  no path/body/key leak.

### U5. Reconcile proposal compatibility without a second deadline authority

- **Files/symbols:** `scripts/mdp-proposal-mcp-server.mjs` — timeout constants,
  `runNode`, `callProposalRun`, output schema and guardrails;
  `scripts/mdp-proposal-runner.mjs` — `buildCleanRunV1Request`, canonical v1
  invocation, and phase child calls; `scripts/lib/proposal-runner-runtime.mjs`
  — `runProcess`; `scripts/test-proposal-mcp-server.sh`,
  `scripts/test-proposal-runner-modules.mjs`, and `docs/proposal-runner.md`.
- **Steps:**
  1. Use the shared deadline policy for `clean_run_v1`, changing the generated
     v1 request's unexplained 30-second value to the canonical recommended
     default or recording an explicit compatibility override in its plan.
  2. Pass the outer value to canonical `mdp run` rather than letting the
     proposal wrapper's 120/300-second child timer imply a longer runtime
     authority. Keep legacy v0 proposal output fields (`timed_out`,
     `termination_signal`, `timeout_ms`) readable and add bounded deadline
     metadata only where the versioned envelope permits it.
  3. Let each proposal child call consume the parent remaining budget; do not
     restart a full 120-second timer for pack validation, native normalization,
     clean-run validation, and review. Preserve source intake/workdir behavior,
     redaction, mock/non-audit-grade boundaries, and v0 receipt semantics.
  4. Document the legacy outer guard as a compatibility boundary if it cannot
     be changed without a contract migration; the canonical v1 effective
     deadline remains the only execution authority.
- **Checks:** Proposal MCP default/max/help, native timeout, clean-run timeout,
  source-intake timeout, cancellation, mock/no-draft, legacy v0 fixture
  parsing, and no change to the v1 CLI authority object.

### U6. Update operator guidance and installed surfaces

- **Files:** `cli/USAGE.md`, `docs/run-receipts.md`, `docs/host-conformance.md`,
  `docs/native-api-normalization-runner.md`, `docs/proposal-runner.md`,
  `plugin/skills/mdp/SKILL.md`, `plugin/skills/mdp/references/cli-operator.md`,
  and any generated plugin asset mirrors required by the repository workflow.
- **Steps:**
  1. Show one canonical 60-second recommendation, the preflight command, and
     how to read configured versus effective limits and phase outcomes.
  2. Explain that MCP transport is an outer guard, cannot extend a tighter
     request/provider policy, and must not be treated as a second assurance or
     receipt authority.
  3. Document timeout/cancellation no-draft behavior, finalization reserve,
     recovery cleanup, legacy artifact compatibility, and the blocking-syscall
     limitation without promising hard preemption.
  4. Remove menus of unexplained 30/60/120/300-second values. Retain a larger
     legacy proposal value only where the docs explicitly identify why it is
     an outer compatibility cap and how the canonical v1 plan wins.
- **Checks:** Markdown/public-artifact/skill contract checks, generated asset
  parity, installed help output, and source-tree versus installed MCP smoke.

## Exact File and Symbol Matrix

| Surface | Current seam | Planned proof |
|---|---|---|
| Deadline kernel | `cli/src/run_runtime.rs`: `RunDeadline`, `execute_transaction`, `execute_generative_step`, `invoke_native_driver`, `supervise_child` | Rust phase matrix, monotonic remaining-budget assertions, timeout/cancel redaction, no-output publication |
| Failure/contract carrier | `cli/src/run_contracts.rs`: `RunnerAuditV1`, `RunReceiptV1`, `DriverProviderPolicyV2`, `DriverResultV2`; MDP-227 `RunFailure` diagnostic seam | Closed additive schemas, hash binding, terminal/phase consistency, legacy read compatibility |
| CLI preflight/authority | `cli/src/cli.rs`, `cli/src/app.rs`, `cli/src/commands/run.rs`, `cli/src/commands/schemas.rs` | `run-preflight-v1`, normal run plan parity, no credential/provider/staging side effects, sanitized failure result |
| Verification/trace | `cli/src/commands/run_verification.rs`, `cli/src/commands/decision_trace.rs` | Receipt/audit timeout binding; trace may explain the phase but cannot alter authority |
| Canonical MCP | `scripts/mdp-run-mcp-server.mjs`, `scripts/lib/process-supervisor.mjs` | Outer tighter/looser/equal, process group/recovery cleanup, transport error metadata, exact CLI passthrough |
| Proposal compatibility | `scripts/mdp-proposal-mcp-server.mjs`, `scripts/mdp-proposal-runner.mjs`, `scripts/lib/proposal-runner-runtime.mjs` | Parent remaining budget, clean-run v1 handoff, legacy v0 envelope compatibility |
| Provider | `scripts/mdp-native-model-openai.mjs` | Abort uses the Rust-provided remaining budget; provider timeout is stable and body/key-free |
| Tests | `cli/src/run_runtime.rs`, `cli/src/commands/run.rs`, `cli/src/commands/run_verification.rs`, `cli/src/commands/schemas.rs`, `scripts/test-native-model-driver.mjs`, `scripts/test-run-mcp-server.mjs`, `scripts/test-run-conformance.mjs`, proposal tests | Staging/provider/validation/transport/cancellation/overflow/descendant/cleanup matrix |
| Guidance | Docs and `plugin/skills/mdp*` listed in U6 | One recommended default, phase interpretation, no-draft and compatibility boundaries |

## Ordered Execution and Dependency Handoff

1. Confirm MDP-227's landed diagnostic carrier and exact allowlisted phase/code
   seam. Do not implement against an invented diagnostic shape.
2. Add the internal deadline plan and additive observation type; first make the
   preflight calculation deterministic with unit tests.
3. Add the CLI transport handoff and preflight output, then thread the same
   plan through staging, driver/provider, validation, finalization, receipt,
   and cleanup.
4. Update schemas and verifier binding together. Preserve the existing v1
   contracts, terminal states, reason codes, null output/decision invariants,
   and legacy artifact readability.
5. Update the bundled native driver and Rust supervisor to consume one
   remaining provider budget. Prove provider abort and driver kill separately.
6. Align canonical MCP and then reconcile proposal compatibility. Run focused
   tests after each adapter change; never let MCP recalculate CLI authority.
7. Update docs/skills and generated assets only after contract tests establish
   the final field names and one recommended default.
8. Run all focused and full validation, inspect generated schemas/help, and
   verify no fixture/log includes provider bodies, credentials, paths, or raw
   stderr. Implementation remains blocked until MDP-227 and MDP-239's Phase 0
   handoff policy are satisfied.

## Validation Contract

Run from the repository root after implementation; these are existing gates,
with focused additions colocated in the named modules:

```bash
git diff --check
cd cli && cargo fmt --check
cd cli && cargo test run_runtime::tests -- --nocapture
cd cli && cargo test commands::run::tests -- --nocapture
cd cli && cargo test commands::run_verification::tests -- --nocapture
cd cli && cargo test commands::schemas::tests -- --nocapture
cd .. && node scripts/test-native-model-driver.mjs
cd .. && node scripts/test-run-conformance.mjs
cd .. && node scripts/test-run-mcp-server.mjs
cd .. && bash scripts/test-proposal-mcp-server.sh
cd .. && node scripts/test-proposal-runner-modules.mjs
cd .. && make validate-run-v1-golden
cd .. && make validate-run-conformance
cd .. && make validate-run-mcp
cd .. && make validate
```

The focused matrix must prove:

- preflight reports configured transport/runtime/provider limits, reserve,
  warnings, and computed effective limit without staging or provider access;
- equal limits use one effective deadline; an outer larger limit cannot extend
  a tighter runtime/provider limit; an outer smaller limit is observable as the
  winner; reserve underflow is rejected;
- staging, provider/driver, validation, source reread, finalization, and outer
  transport timeout cases identify the correct bounded phase and elapsed/limit
  values;
- provider abort and process-group termination use the same remaining budget,
  cancellation has a stable bounded outcome, and descendants/recovery claims
  are cleaned up;
- post-bundle timeout/cancellation yields a verified no-draft receipt when the
  finalization reserve allows it, while pre-bundle/finalization interruption
  never publishes partial output or a misleading receipt;
- CLI, canonical MCP, and clean-run proposal compatibility expose identical
  CLI authority/timeout observations; MCP-only errors contain no raw path,
  body, key, provider response, or stderr;
- schema mutation, hash mismatch, legacy v1 audit without the optional field,
  deterministic runs, mock/dry-run, overflow, and provider failure remain
  compatible with existing no-draft and assurance rules.

## Risks and Mitigations

| Risk | Mitigation |
|---|---|
| MDP-227 changes the failure carrier shape or MDP-226/237 changes runtime seams | Make MDP-227 the first implementation dependency; record the landed helper/result seam before editing and keep this plan's phase projection additive. |
| Two timers race near the boundary and produce different phases | Start one Rust monotonic clock; pass one absolute/remaining provider budget; reserve transport handoff time; test equal, shorter, and longer outer values with generous CI margins. |
| Adding fields to closed v1 artifacts breaks old consumers | Keep fields optional for deserialization and legacy schema fixtures; enforce new fields on newly emitted timeout/cancelled observations; ship producer/schema/verifier changes together. |
| A caller treats a timeout observation as decision authority | Keep terminal state, source authority, reason codes, decision nullability, and assurance recomputation unchanged; document the object as bounded explanation only. |
| Forced outer termination happens before Rust writes a receipt | Preserve process-group SIGTERM/SIGKILL escalation and exact recovery-claim cleanup; report a transport-only no-draft error with no artifact authority when no receipt can exist. |
| Blocking filesystem calls exceed the deadline | Keep the explicit limitation; check at every safe boundary and never claim kernel-call preemption. Outer recovery remains the last guard. |
| Provider or driver diagnostics leak secret/body/path content | Allowlist phase/outcome/codes and numeric values; discard raw stderr/provider envelopes; retain existing environment filtering and assert redaction at every public surface. |
| Proposal compatibility creates a second effective deadline | Pass parent remaining budget to all child calls and make canonical `mdp run` authoritative; retain larger legacy values only as explicitly labeled outer caps. |
| Small timeout fixtures become flaky after reserve enforcement | Raise test values above the reserve, use deterministic fake clocks/hooks where possible, and assert ranges/ordering rather than wall-clock equality. |

## Compatibility and Rollback Notes

- This plan-only commit has no runtime effect. Reverting a future implementation
  restores the current scalar timeout behavior and existing no-draft semantics;
  it must not revert MDP-227's separate diagnostics or MDP-226's routed-context
  producer work.
- `mdp.run-request.v1`, `mdp.run-bundle.v1`, `mdp.driver-request.v2`,
  `mdp.driver-result.v2`, `mdp.runner-audit.v1`, and `mdp.run-receipt.v1` remain
  the canonical contracts. The timeout observation is additive and
  hash-bound; it does not create a parallel receipt or decision authority.
- Existing v1 receipts/audits without timeout evidence remain parseable and
  verifiable for their original claims. A verifier must report timeout
  evidence as unavailable/unknown rather than infer it from a terminal state.
- Existing v0 proposal receipts and MCP envelope fields remain readable. The
  proposal compatibility wrapper may retain its historical outer timeout only
  with explicit documentation and must not upgrade its v0 evidence into v1.
- The transport option is host-controlled and not written into the caller's
  request file or policy hash. The emitted observation records it so replay or
  verification can detect which bound won.
- No provider retry, output recovery, CRM/outbound action, release, merge,
  status/label transition, native delegation, or external data mutation is
  part of rollback or this issue.

## Acceptance Criteria Mapping

| MDP-238 acceptance criterion | Plan implementation and proof |
|---|---|
| Preflight reports every relevant timeout and computed effective deadline | U1/U3 define the closed plan and `run-preflight-v1`; U4/U5 pass transport configuration into the same Rust calculation; CLI/MCP preflight tests assert configured runtime/provider/transport limits, reserve, effective value, and warnings. |
| Configuration rejects or warns when an outer timeout cannot affect the tighter inner bound | U1 emits bounded `outer-timeout-cannot-extend-inner` or `outer-timeout-truncates-runtime`; U4 validates range/reserve and tests both outer-longer and outer-shorter cases. |
| Timeout receipts identify phase, elapsed time, configured limit, effective limit, and no-draft terminal state | U1/U2 add `DeadlineObservationV1`; U3 binds it through `RunnerAuditV1`/`RunReceiptV1`/authority output; Rust receipt/verifier tests assert provider, staging, validation, finalization, timeout, and cancellation observations. |
| CLI and MCP defaults are aligned or difference explicitly justified | U4 makes canonical v1 `mdp_run` use the shared 60-second recommendation and explicit transport handoff; U5 either adopts it for clean-run v1 or labels legacy proposal 120/300-second values as outer compatibility caps in output/docs/tests. |
| Cancellation/timeout remain canonical no-draft outcomes with no partial success artifact | U2/U4/U5 preserve `no-draft:runner-failed`, null output/decision/context, transactional publication, process-group kill, and recovery cleanup; tests cover provider abort, outer kill, cancellation, and finalization interruption. |
| Help and skill guidance show one recommended default | U6 updates CLI/MCP help, `mdp_run_tools`, docs, and plugin references to recommend 60,000 ms and explain only the bounded phase/winner fields. |
| Deterministic tests cover staging/provider/validation/outer transport/cancellation deadlines | U2-U5 add the phase matrix across Rust, native driver, canonical MCP, proposal compatibility, conformance, and installed smoke targets. |
| No timeout path leaks provider/body content into diagnostics | U1/U3/U4/U5 keep all observations numeric/allowlisted, discard raw provider/stderr data, preserve path redaction, and add serialized-output no-leak assertions. |

## Readiness and Handoff

This plan is ready for hosted Codex implementation only after MDP-227's
field-level diagnostic seam and MDP-239's Phase 0 handoff policy are available.
MDP-238 must remain `Backlog`/`phase:planned` while its direct MDP-227 blocker
and parent execution-index gates remain open. Preserve `delegate:codex`,
`sync:pr-link-only`, and the existing MDP-227 blocking relation. The exact
pushed source ref, full commit SHA, and tracked plan path are recorded in the
Linear handoff comment after publication; no runtime implementation or PR is
authorized by this artifact.
