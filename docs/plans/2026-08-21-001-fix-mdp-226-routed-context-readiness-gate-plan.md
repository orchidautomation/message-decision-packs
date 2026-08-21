---
title: MDP-226 Canonical Routed-Context Readiness Gate - Plan
type: bug
date: 2026-08-21
topic: routed-context-readiness
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-226
execution: code
linear_issue: MDP-226
parent_linear_issue: MDP-239
---

# MDP-226 Canonical Routed-Context Readiness Gate - Plan

## Goal Capsule

- **Objective:** Make the generative clean-run preflight accept the exact
  `mdp.routed-context.v1` bytes emitted by a ready `brief --context` or
  `emit-brief --routed-context-out`, while rejecting context that is malformed,
  non-canonical, blocked, stale, from another job, or compiled from another
  pack.
- **Current failure:** `cli/src/run_runtime.rs::validate_generative_input_gates`
  currently requires `value["status"] == "ready"` and optionally accepts a
  `draft_status` field. The canonical producer writes
  `context.model_context`, whose closed schema has `contract`, `job`,
  `persona`, `scope`, product-foundation fields, `entries`, `gaps`, and
  `policy`; it intentionally has neither `status` nor `draft_status`.
- **Authority:** The staged MDP pack, compiled job/model step, routed-context
  schema, and exact canonical bytes are authoritative. The host, MCP wrapper,
  provider response, and skill prose are not readiness authority.
- **Stop conditions:** Stop before driver invocation if schema validation,
  canonical-byte validation, job/persona/scope binding, or recompilation from
  the staged pack fails. Never make a schema-invalid artifact pass by adding a
  readiness field to the v1 envelope.
- **Execution profile:** One Rust runtime fix with focused unit tests, real
  brief-to-run synthetic coverage, CLI/stdio-MCP authority parity, and an
  installed plugin/CLI smoke. No real provider call, key, hosted service, or
  cross-repository change is required.
- **Tail ownership:** MDP-226 owns the generative input gate and its proof. It
  does not own MDP-227 field-level diagnostics, MDP-237 sealed request
  compilation, model/provider behavior, or a new routed-context schema
  version.

## Product Contract

### Requirements

- **R1 — Accept the producer artifact.** The exact bytes written by
  `brief --context --routed-context-out` and `emit-brief --routed-context-out`
  pass generative preflight without mutation or hand-edited fields.
- **R2 — Validate the closed contract.** The gate parses the staged file as
  JSON, validates it with the existing `routed_context_schema()` for
  `mdp.routed-context.v1`, requires `application/json` and the canonical input
  schema identity, and rejects unknown fields, missing fields, wrong contract
  values, malformed entries, and invalid JSON.
- **R3 — Bind current execution authority.** The gate requires the context
  `job` to equal the selected model step's `job_identity.job_id`, validates the
  serialized scope, and recompiles the model-visible projection from the
  immutable staged pack. The supplied context must equal that projection
  byte-for-byte after canonical JSON serialization; this catches stale
  context, wrong-job context, wrong-pack context, changed selected entries,
  and changed scope/persona.
- **R4 — Preserve fail-closed readiness.** A blocked or unavailable compiled
  context never reaches the driver. Keep the existing sanitized
  `draft-readiness-blocked` behavior for a readiness failure and use the
  existing invalid-context failure family for structural or binding failures.
- **R5 — Cover both runtime preflights.** The same validator is used from
  `validate_native_request_size_before_bundle` and `execute_generative_step`,
  so request-size preflight and just-before-driver execution cannot disagree.
- **R6 — Preserve non-context steps.** Normalization steps that do not declare
  `routed_context`, deterministic runs, legacy prompt inputs, and the existing
  `routed-context` spelling alias retain their current behavior. Required
  input presence and duplicate logical-name checks remain owned by the current
  request/step validators.
- **R7 — Preserve transport parity.** Direct CLI and stdio `mdp_run` invoke the
  same runtime and expose the same terminal state, authority, and sanitized
  reason for one request. MCP adds no readiness or assurance semantics.
- **R8 — Prove the released surface.** Synthetic parity covers GTM generation,
  GTM review, and proposal review with real compiled routed-context artifacts;
  installed release smoke runs the same proof against the installed CLI and
  installed plugin assets. No test calls a provider.

### Acceptance Examples

- **AE1 — Ready GTM context:** Given a ready basic-pack
  `outbound-copy-brief` context emitted by `emit-brief`, a generative run
  passes context preflight and reaches the normal default-deny native-driver
  boundary when no provider permission is supplied; it does not fail with
  `draft-readiness-blocked`.
- **AE2 — Ready proposal context:** Given any proposal review job's exact
  `mdp.routed-context.v1` artifact, the same gate accepts it and does not
  require a top-level `status` or `draft_status`.
- **AE3 — Blocked or malformed:** Given invalid JSON, an unknown field, a
  missing required field, a wrong contract, or a non-canonical JSON encoding,
  preflight blocks before any driver invocation and publishes no usable
  output.
- **AE4 — Stale/wrong identity:** Given a context edited after emission, a
  context emitted for another job, a context compiled from another pack, a
  changed persona, or a changed scope, preflight blocks before the provider
  boundary with a sanitized invalid-context/readiness reason.
- **AE5 — CLI/MCP parity:** Given one canonical request and context, direct
  `mdp run` and stdio `mdp_run` return equivalent canonical authority and
  terminal data; the MCP envelope does not reinterpret a policy block.
- **AE6 — Regression fixture:** Given the existing universal native parity
  harness, replacing its `{ "status": "ready" }` placeholder with the real
  `emit-brief` artifact makes all declared generation/review bindings pass
  context preflight while preserving normalization parity.
- **AE7 — Installed proof:** Given the staged release CLI and installed plugin
  tree, the installed native-parity harness passes with synthetic inputs and
  the installed CLI/plugin pair returns the same canonical schema and runtime
  behavior as source validation.

### Scope Boundaries

**Included**

- The shared canonical routed-context validation path used by the Rust
  generative runtime.
- Reconciliation with the existing schema, routing compiler, prompt-output
  validation, synthetic parity harness, stdio MCP parity, documentation, and
  installed release smoke.
- Focused tests for acceptance, fail-closed behavior, source/pack identity,
  and no-driver invocation.

**Deferred or owned elsewhere**

- MDP-227's richer field-level diagnostics; this change returns bounded
  machine-safe run failure codes only.
- MDP-237's sealed request compiler and MDP-231's observed driver/model hash
  binding.
- Any new `mdp.routed-context.v2` pack identity or release-digest field. v1
  identity is proven by exact recompilation from the staged pack; unrelated
  pack changes that cannot alter the selected projection are outside the v1
  artifact's available identity data.
- Provider calls, credentials, retries, model quality, CRM/email actions,
  hosted storage, or external repository changes.

## Planning Contract

### Key Technical Decisions

- **KTD1 — Keep v1 closed and producer-owned.** Do not add `status`,
  `draft_status`, or an ad hoc readiness field to
  `mdp.routed-context.v1`. The producer's `context.minimality.status` is the
  gate before export; the exported model-visible envelope is ready only when
  the compiler returns it as `model_context`.
- **KTD2 — Reuse one canonical compiler.** Add a reusable routed-context
  identity/compilation helper beside `routing::entry_context_scoped` rather
  than inventing a second route evaluator in the run runtime. It must use the
  staged `Manifest`, `ContextScope`, `resolve_runtime_scope`, and existing
  product-foundation/routing rules.
- **KTD3 — Verify exact bytes at the runtime boundary.** Compare the staged
  input's SHA-256 with canonical JSON bytes for the parsed value, then compare
  the parsed value with the staged pack's recomputed `model_context`. This
  prevents whitespace/ordering mutations and semantic drift from being
  treated as equivalent context.
- **KTD4 — Separate structural and readiness failures.** Keep the current
  `routed-context-invalid` family for parse/schema/canonical/identity failures
  and `draft-readiness-blocked` for a valid context that cannot prove a ready
  current compilation. The exact public terminal remains
  `no-draft:policy-blocked`; do not expose paths or source bodies.
- **KTD5 — Validate twice, with one implementation.** Invoke the helper at
  both current generative call sites because the first invocation is the
  request-size boundary and the second is the driver boundary. Avoid divergent
  checks; the helper must be pure with respect to the staged pack and file
  bytes.
- **KTD6 — Keep the output validator aligned.** Reuse the helper, or make the
  existing checks in `commands::prompt_output` call the same identity routine,
  so run preflight and governed-output validation cannot disagree about job,
  scope, pack, or canonical compilation. Preserve existing detailed output
  issue codes and legacy non-governed behavior.
- **KTD7 — Make parity fixtures real.** Generate routed context through the
  installed/source CLI (`emit-brief --routed-context-out`) for the correct
  GTM or proposal persona and pass those exact bytes into the run request.
  Placeholder readiness objects remain only negative fixtures.

### High-Level Design

```text
brief/emit-brief
  -> entry_context_scoped / context_minimality
  -> exact context.model_context bytes (mdp.routed-context.v1)
  -> host declares routed_context input
  -> stage pack + input into private run tree
  -> resolve selected model step and job gates
  -> validate canonical schema, bytes, job, persona, scope, and recompilation
       | fail -> no-draft:policy-blocked; no driver, no output authority
       | pass -> build sealed driver request
  -> native driver or default-deny synthetic boundary
  -> prompt-output validation and receipt (unchanged)
  -> direct CLI / stdio MCP projections of the same authority
```

### Implementation Units

#### U1. Centralize canonical routed-context identity validation

- **Goal:** Give runtime and governed-output validation one authority check for
  the closed v1 envelope.
- **Primary files and symbols:**
  - `cli/src/routing.rs`: add a `pub(crate)` helper adjacent to
    `entry_context_scoped` (for example,
    `validate_routed_context_for_job`) and a small result/error shape carrying
    canonical SHA or bounded reason classification.
  - `cli/src/scope.rs`: reuse `ContextScope` and
    `resolve_runtime_scope`; do not introduce new scope vocabulary.
  - `cli/src/commands/schemas.rs`: call existing
    `routed_context_schema()`; do not change its required/properties set.
- **Ordered steps:**
  1. Read the already-staged bytes with the existing input-size boundary and
     parse JSON; map parse failures to the invalid-context family.
  2. Validate `mdp.routed-context.v1` with the existing Draft 2020-12 schema,
     require `contract` and the JSON media/schema identity, and reject unknown
     fields.
  3. Require non-empty `job` and compare it to the selected model job.
  4. Deserialize `scope.requested`, resolve it against the staged manifest,
     and require the serialized context scope to equal the resolved scope.
  5. Read `persona`, run `entry_context_scoped(staged_pack, manifest,
     persona, job, true, &scope)`, require a ready compilation, and require its
     `model_context` to equal the parsed input.
  6. Canonicalize the parsed value and require its SHA-256 to equal the staged
     input authority SHA-256. Return no raw value or path in errors.
- **Proof:** unit cases cover valid basic/proposal artifacts, schema-invalid
  values, non-canonical bytes, wrong job, wrong persona/scope, changed
  selected card content, and a different pack.

#### U2. Replace the undeclared readiness-field gate

- **Goal:** Make both generative runtime preflights consume U1 rather than
  inspecting absent `status`/`draft_status` fields.
- **Primary file and symbols:** `cli/src/run_runtime.rs`:
  `validate_generative_input_gates`, its calls in
  `validate_native_request_size_before_bundle` and
  `execute_generative_step`, plus the `run_runtime` unit-test module near
  `generative_request_fixture`.
- **Ordered steps:**
  1. Pass the staged pack, loaded manifest, and selected job identity into the
     gate; retain both `routed_context` and `routed-context` logical-name
     handling.
  2. For a declared routed-context input, require the canonical JSON media and
     schema identity and call U1. Leave all non-routed inputs unchanged.
  3. Preserve `validate_step_inputs` as the owner of required/undeclared input
     membership and preserve the no-driver ordering: U1 runs before
     `DriverRequestV2` sealing and before the driver closure.
  4. Add explicit tests that a valid emitted artifact gets past context
     preflight, while malformed/blocked/stale/wrong-job/wrong-pack fixtures
     fail before the closure increments a call counter.
  5. Keep the existing normalization fixture and deterministic run tests green
     to prove no regression for steps without routed context.
- **Proof:** focused `cargo test` for the runtime module and a full Rust test
  run; assert no private transaction/output is published on preflight refusal.

#### U3. Align governed-output validation and contract tests

- **Goal:** Prevent the later `validate-prompt-output` path from accepting a
  different identity interpretation than native preflight.
- **Primary files and symbols:**
  - `cli/src/commands/prompt_output.rs`: the routed-context branch in
    `validate_prompt_output_value_with_inputs` around the existing
    `governed_artifact_routed_context_*` checks and its current routed-context
    fixtures/tests.
  - `cli/src/commands/schemas.rs`: retain and exercise
    `routed_context_schema_is_closed_and_versioned` and
    `routed_context_schema_validates_live_entries_and_rejects_malformed_entries`;
    only touch this file if a helper import/test seam requires it, never to add
    undeclared readiness fields.
- **Ordered steps:**
  1. Route schema, contract, job, scope, canonical-byte, and recompilation
     checks through U1 where practical; translate helper failures to the
     existing detailed `governed_artifact_routed_context_*` issue codes.
  2. Preserve context digest and invocation-receipt checks, selected-authority
     kind checks, and legacy prompts that do not declare routed context.
  3. Add regression tests for exact producer bytes and confirm a context that
     passes runtime identity checks also passes governed-output identity checks.
- **Proof:** `cargo test` for `commands::prompt_output` and `commands::schemas`,
  including existing tamper and selected-authority cases.

#### U4. Repair source-tree and installed parity fixtures

- **Goal:** Exercise the real producer-to-runtime path rather than a schema-
  invalid placeholder.
- **Primary files and symbols:**
  - `scripts/test-universal-native-parity.mjs`: run-input construction in the
    native binding loop around the current `input.name === 'routed_context'`
    placeholder.
  - `scripts/release-install-smoke.sh`: installed plugin/CLI validation block
    after the existing installed schema and skill checks.
- **Ordered steps:**
  1. For each job/step that declares `routed_context`, call the selected
     `mdp` binary with `--json emit-brief --dir PACK --persona PERSONA
     --job JOB --routed-context-out PATH` (use the shipped GTM persona and
     proposal persona), assert minimality is ready, and reuse the exact file
     bytes as the run input.
  2. Set the routed input's declared `schema_id` to
     `mdp.routed-context.v1` and `media_type` to `application/json`; retain
     synthetic empty JSON only for other required inputs where the test is
     intentionally exercising the driver default-deny boundary.
  3. Assert the emitted file's contract, canonical SHA, and job before building
     the run request. Keep no API key or provider response in fixtures.
  4. Add one direct-CLI/stdio-MCP comparison using the same canonical request;
     compare terminal, `authority`, and `authority_block` rather than MCP
     transport status.
  5. Invoke the installed plugin copy of this parity harness from
     `release-install-smoke.sh` with `MDP_BIN` set to the isolated installed
     CLI. This proves installed scripts/assets and the installed binary work
     together.
- **Proof:** `node --check scripts/test-universal-native-parity.mjs`,
  `node scripts/test-universal-native-parity.mjs`, and the installed release
  smoke. The run should reach the expected synthetic default-deny driver
  boundary, not the old readiness-field failure.

#### U5. Document the exact handoff and run review gates

- **Goal:** Keep operators and agents from recreating the invalid placeholder
  contract.
- **Primary files:** `docs/minimal-context-routing.md`,
  `docs/native-api-normalization-runner.md`, and the canonical
  `plugin/skills/mdp/SKILL.md`; mirror only if a user-facing contract changes.
- **Ordered steps:**
  1. State that `routed_context` is the exact saved
     `mdp.routed-context.v1` model-context object and has no top-level
     `status`/`draft_status` readiness fields.
  2. State that the runtime revalidates schema, canonical bytes, selected job,
     scope/persona, and current staged-pack compilation before model execution.
  3. Keep the existing no-draft, local-first, provider-owned execution and
     MCP-transport boundaries.
  4. Run skill-contract, public-artifact, and asset-sync checks if the skill or
     any mirrored asset changes.
- **Proof:** documentation commands match `mdp --json schema` and the live
  `emit-brief`/`run` behavior; no agent instruction tells a host to hand-author
  readiness fields.

### Dependencies and Risks

| Dependency/risk | Mitigation or decision |
| --- | --- |
| MDP-200 owns minimal-context compilation and MDP-188 owns the native driver. | Reuse their shipped compiler/run contracts; change only the consumer gate and proof. |
| `prompt_output.rs` already has similar but richer context checks. | Centralize identity checks in U1 and preserve its issue-code projection; add parity tests before refactoring. |
| v1 context has no pack digest. | Detect stale/wrong-pack semantic drift by exact recompilation from the staged pack; do not silently add a v1 field. Escalate any requirement for unrelated-pack-change detection to a new contract/version decision. |
| Runtime may compile context twice. | Keep the helper bounded and deterministic; optimize only after correctness proof, never by skipping the second boundary check. |
| Existing generated or user-authored placeholder contexts. | Intentionally fail closed; no migration or compatibility exception for schema-invalid readiness fields. |
| Proposal and GTM personas differ. | Resolve persona from each shipped pack in the parity harness and assert the emitted context's job; do not hard-code GTM context into proposal runs. |
| Installed smoke can accidentally use source paths. | Run the harness from the installed plugin tree with `MDP_BIN` pointing at the isolated installed binary and compare release-tree assets. |
| Provider credential leakage during tests. | Use default-deny native execution and synthetic mock/subprocess boundaries; never set or print `OPENAI_API_KEY`. |

### Sequencing

1. Land U1's shared identity helper and focused helper tests.
2. Land U2's runtime call-site replacement and no-driver regressions.
3. Land U3's output-validator alignment and schema/contract regressions.
4. Land U4's real producer parity, CLI/MCP parity, and installed-smoke hook.
5. Land U5 docs/skills only where the live contract is clarified.
6. Run focused tests, `make validate`, code review, PR checks, and the
   post-release installed smoke required by MDP-239.

## Verification Contract

| Gate | Command or evidence | Done signal |
| --- | --- | --- |
| Format and focused Rust | `cargo fmt --manifest-path cli/Cargo.toml --check`; `cargo test --manifest-path cli/Cargo.toml run_runtime`; `cargo test --manifest-path cli/Cargo.toml routed_context_schema`; `cargo test --manifest-path cli/Cargo.toml prompt_output` | Canonical context accepts; all malformed, stale, identity, and no-driver cases fail closed. |
| Source parity | `cargo build --manifest-path cli/Cargo.toml`; `node --check scripts/test-universal-native-parity.mjs`; `node scripts/test-universal-native-parity.mjs` | Basic GTM and proposal real emitted contexts pass preflight; no provider call. |
| CLI/MCP parity | The U4 direct-vs-stdio test plus `node --test scripts/test-run-mcp-server.mjs` | Canonical authority/terminal output matches; MCP adds no readiness meaning. |
| Existing conformance | `make validate-run-conformance validate-cold-model-conformance validate-native-parity validate-run-mcp` | Existing deterministic, receipt, cold-model, and native parity contracts remain green. |
| Packaging/docs | `make validate-skill-contracts validate-skill-packaging validate-asset-sync validate-public-artifacts` when affected | Canonical skills/assets/docs remain synchronized and public-safe. |
| Full repository gate | `make validate` | All repository validation passes on the implementation commit. |
| Installed release proof | `scripts/release-install-smoke.sh VERSION` with the staged release manifest, then installed plugin `test-universal-native-parity.mjs` | The isolated installed CLI/plugin pair accepts real routed-context artifacts and preserves the same fail-closed behavior. |

## Compatibility and Rollback

- **Wire compatibility:** `mdp.routed-context.v1` remains unchanged. Existing
  valid producer bytes continue to work; schema-invalid `{status: ready}`
  placeholders become the intentionally blocked path.
- **Runtime compatibility:** Deterministic runs, normalization steps without a
  routed-context declaration, prompt-output validation for legacy prompts, and
  both existing routed-context logical-name spellings remain supported.
- **Operational compatibility:** The native driver, provider endpoint policy,
  run bundle, receipt, MCP server, and terminal-state vocabulary do not change.
  Only preflight eligibility changes from an undeclared field check to the
  canonical artifact check.
- **Rollback:** Revert the single implementation commit and restore the prior
  parity fixture if emergency rollback is required. No migration, stored-data
  rewrite, release-tag mutation, or external-system rollback is needed. A
  rollback restores the known bug, so it must be followed by a new corrective
  implementation rather than treated as resolution.

## Acceptance Mapping

| Linear MDP-226 criterion | Plan coverage |
| --- | --- |
| Exact ready brief artifact passes without mutation | R1, AE1/AE2, U1-U2, U4 |
| Canonical contract instead of undeclared fields | R2, KTD1, U1-U2, U4 |
| Blocked/malformed/stale/wrong-job/wrong-pack fail closed | R3-R4, AE3/AE4, U1-U2 |
| No schema-invalid `status` workaround | KTD1, U1/U3/U5 |
| CLI and stdio MCP authority parity | R7, AE5, U4, verification contract |
| Real brief-emitted artifact regression | AE1/AE6, U2/U4 |
| Installed CLI/plugin smoke, synthetic only | R8, AE7, U4, installed release gate |

## Definition of Done

- The implementation has one tested canonical routed-context identity check and
  both generative preflight call sites use it before driver invocation.
- The v1 schema and producer remain closed and no readiness field is invented.
- Exact GTM and proposal producer artifacts pass; malformed, non-canonical,
  blocked, stale, wrong-job, changed-scope, changed-persona, and wrong-pack
  artifacts fail before provider execution.
- Existing governed-output, normalization, deterministic, receipt, and MCP
  behavior remains green.
- Source and installed parity use real emitted context bytes, direct CLI/MCP
  authority matches, no credential/provider call is required, and `make
  validate` plus release-install smoke pass.
- The implementation PR is linked to MDP-226 under parent MDP-239; MDP-227 and
  MDP-237 remain correctly sequenced follow-ups.
