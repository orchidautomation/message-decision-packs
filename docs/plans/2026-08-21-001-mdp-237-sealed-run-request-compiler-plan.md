---
title: MDP-237 Sealed Native Run-Request Compiler and Preflight - Plan
type: feature
date: 2026-08-21
topic: sealed-run-request-compiler
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-237
execution: code
linear_issue: MDP-237
parent_linear_issue: MDP-239
---

# MDP-237 Sealed Native Run-Request Compiler and Preflight - Plan

## Goal Capsule

- **Objective:** Add one offline, file-oriented compiler/preflight that derives
  a schema-valid `mdp.run-request.v1` generative request from a pack, one
  declared model step, and declared local input paths. The caller chooses the
  pack/job/step, input files, requested model, and retention policy; MDP derives
  execution identity, pack release identity, prompt authority, artifact
  metadata, execution policy, driver identity, executable identity, model
  parameter identity, and all required hashes.
- **Current failure:** 0.1.73 makes callers hand-author
  `execution_id`, timestamp, idempotency identity, `pack_release_id`, prompt
  and input metadata, policy fields, bundled-driver hashes, Node executable
  hash, configuration hash, and model-parameter hash. The universal parity
  harness has to construct these fields itself. That creates a second
  authority next to the pack/model-step resolver and permits a validly shaped
  request to carry self-attested or stale runtime identities.
- **Authority:** The staged pack, `Manifest`, `resolve_selected_model_step`,
  exact prompt bytes, exact declared input bytes, `pack_content_snapshot`,
  the native driver/runtime identity helper delivered by MDP-231, and the
  compiler's canonical model-parameter/policy objects. CLI and MCP are
  transports over the same Rust compiler kernel; neither chat text, an MCP
  envelope, a provider, or a caller-authored hash is authority.
- **Stop conditions:** Refuse before writing a request when the pack/job/step
  is missing or ambiguous, a required declared input is absent or unsafe, a
  prompt/schema/pack contract is invalid, MDP-226 or MDP-230's required
  artifact contract is not satisfied, the MDP-231 identity observation cannot
  be established, limits exceed the native boundary, or canonicalization
  changes any derived bytes. Errors are bounded machine-safe codes with the
  exact contract/next safe command, never source bodies, credentials, or
  arbitrary local paths.
- **Execution profile:** One additive CLI command, one reusable Rust compiler
  kernel, a read-only MCP preparation tool, focused schema/compiler tests,
  CLI/MCP byte-parity tests, no-network proof, installed skill smoke, and
  synthetic GTM/proposal fixtures. The implementation must not invoke a
  provider, read a provider key, or change `mdp run`'s explicit execution
  boundary.
- **Tail ownership:** MDP-237 owns compilation/preflight, request assembly,
  CLI/MCP parity, and the operator surface. MDP-226 owns canonical routed
  context readiness; MDP-230 owns governed v2 synthetic lineage; MDP-231 owns
  runtime-observed/recomputed driver and model identity. MDP-237 consumes
  those released contracts and must not duplicate or weaken them.

## Product Contract

### Requirements

- **R1 — One sealed compiler path.** `mdp prepare-run` (the selected public
  name) compiles a complete native generative `mdp.run-request.v1` from
  `--dir`, `--job`, an exact model step selection when required, repeatable
  `--input logical_name=PATH`, `--model`, and optional retention/output
  planning flags. It rejects caller-supplied `execution_id`, `idempotency_key`,
  `pack_release_id`, prompt path, artifact hashes, driver hashes, policy hash,
  or model-parameter hash flags.
- **R2 — Derive every runtime-observable identity.** The compiler derives a
  stable pack release ID from the manifest identity/version and portable pack
  digest; the selected job/step and exact prompt authority from the model-step
  resolver; input schema/media/provenance and bytes from the step declaration
  plus opened regular files; a domain-separated idempotency key and execution
  ID from the canonical compilation inputs; native policy from fixed product
  limits and explicit non-secret retention choice; and driver, executable,
  configuration, and model-parameter hashes from shared MDP helpers. No
  arbitrary SHA-shaped value can enter the generated request.
- **R3 — Preflight is useful before execution.** The default output is concise
  JSON containing status, request contract, execution ID, job/operation,
  pack/input/prompt hashes, driver/model identities, endpoint, effective byte
  and time limits, data boundary, authorization requirement, anticipated
  assurance, and the exact next safe `mdp run` command. `--manifest-out` (or
  an explicit full-output option) writes the full compiler manifest and
  `--out` writes the exact request bytes. Neither output calls a provider.
- **R4 — Preserve the v1 execution wire contract.** The generated authority
  remains `mdp.run-request.v1`; existing `mdp run --request ... --out-dir ...`
  continues to parse and execute compatible hand-authored fixtures. A new
  `mdp.run-request-compile.v1` result identifies compiler provenance and
  preflight diagnostics without adding fields to the closed run-request wire
  contract.
- **R5 — Respect declared input authority.** Input names must exactly match
  the selected step's declared inputs, required inputs must be present, extra
  names and duplicate names fail, files must be regular non-symlink files
  within bounded size, and schema/media type come from the pack declaration.
  MDP-226's canonical `mdp.routed-context.v1` and MDP-230's governed v2
  artifacts are consumed as exact declared bytes; the compiler does not make
  a placeholder `{status: "ready"}` or synthesize lineage in their place.
- **R6 — Keep authorization separate.** Preparation is offline and has no
  provider/network capability. The request records the fixed authorized
  endpoint and environment boundary, while the preflight explicitly reports
  `provider_authorization: required-at-execution`. Only a later explicit
  `mdp run` with the existing permission/key boundary may cross the provider
  boundary.
- **R7 — CLI/MCP byte parity.** The Rust compiler kernel accepts one typed
  option object. The CLI wrapper and a new read-only stdio `mdp_prepare_run`
  tool map path-only arguments into that object and return the same request
  bytes, request SHA, compiler manifest, diagnostics, and next command. MCP
  adds no input/body/assurance authority and does not accept inline prompt,
  input, credential, or run-request JSON.
- **R8 — Errors are actionable and safe.** Stable compiler error codes name
  the failed contract and point to the next safe local command, for example
  `pack-invalid -> mdp validate --dir <pack>`,
  `model-step-ambiguous -> mdp requirements --dir <pack> --job <job>`,
  `declared-input-missing -> provide --input <name>=<path>`, and
  `identity-observation-unavailable -> resolve MDP-231 runtime identity
  contract`. Messages never print input contents, environment values,
  provider request bodies, credentials, or unbounded paths.
- **R9 — Teach the released operator path.** The retained `mdp` skill and
  operator reference use `prepare-run` for the happy path, describe
  preparation as offline, and reserve hand-authored run requests for
  compatibility/negative tests. Documentation keeps the explicit execution
  and verification steps separate and does not teach manual hash entry.
- **R10 — Prove real synthetic cases.** Fixtures cover GTM normalization,
  GTM generation/review, proposal review, a governed v2 chain, ready and
  blocked routed context, altered input, missing input, ambiguous step,
  unsupported model, oversized input/request, non-canonical artifact,
  changed runtime identity, and no provider permission. The harness proves
  that an accepted request reaches the existing default-deny boundary without
  a network call and that no request is emitted for refused input.

### Acceptance Examples

- **AE1 — Generated request:** Given a valid pack, one job/step, and exact
  declared local input files, `mdp --json prepare-run --dir ... --job ...
  --operation ... --input name=path --model gpt-test --out request.json`
  writes a closed-schema `mdp.run-request.v1`; the caller never supplies a
  hash, prompt path, execution ID, or idempotency key.
- **AE2 — Exact authority:** The generated request's prompt, input, pack
  release, driver/executable, policy, and model parameter identities are
  recomputed from the same bytes and helper observations. Altering any input,
  prompt, pack file, Node executable, driver source, policy field, or model
  parameter changes the relevant authority and is visible in the full
  manifest.
- **AE3 — Read-only preflight:** With no provider key or permission, compile
  succeeds offline, reports the official endpoint, data boundary, effective
  limits, anticipated assurance, and authorization requirement, and never
  creates a run directory or opens a network connection.
- **AE4 — Explicit execution:** Passing the generated request to the existing
  `mdp run --request request.json --out-dir run/` preserves current request
  validation, staging, driver permission gate, bundle/receipt authority, and
  sanitized default-deny result. Preparation cannot silently execute.
- **AE5 — Routed context:** A ready exact `mdp.routed-context.v1` emitted by
  MDP-226's producer compiles; a placeholder, blocked, stale, wrong-job,
  wrong-pack, edited, or non-canonical context fails before request output.
  MDP-237 does not add a readiness field or bypass the MDP-226 gate.
- **AE6 — Governed v2 lineage:** Exact artifacts released by MDP-230 are
  accepted only when their source-binding, attempt, collected-results, and
  normalized-input references satisfy the declared job chain. The compiler
  neither regenerates nor rebinds them.
- **AE7 — Runtime identity:** If MDP-231's shared helper cannot observe the
  bundled driver, Node executable, effective configuration, or parameter set,
  preparation fails closed with a bounded dependency/identity error; it never
  inserts a fake or caller-supplied SHA.
- **AE8 — CLI/MCP parity:** Running CLI and stdio `mdp_prepare_run` with the
  same pack, files, model, operation, retention policy, and deterministic test
  clock yields byte-identical request files and equal request/manifest hashes.
  MCP's result is a transport envelope around the same compiler payload.
- **AE9 — Installed surface:** The installed binary plus installed `mdp`
  skill can compile and then explicitly run a synthetic request, and the
  universal parity harness no longer constructs production-shaped identities
  or routed-context placeholders by hand.

### Scope Boundaries

**Included**

- A reusable Rust compiler/preflight kernel and `prepare-run` CLI wrapper.
- A new compile-result contract/schema and capability declaration; the v1
  execution request schema remains closed and compatible.
- Canonical pack/job/step/prompt/input/policy/identity derivation, bounded
  file reads, deterministic IDs, full/concise output, and safe diagnostics.
- A read-only MCP preparation tool that delegates to the CLI/kernel, direct
  CLI/MCP byte-parity coverage, universal native parity updates, synthetic
  fixtures, skill/docs refresh, and release/install smoke.

**Deferred or owned elsewhere**

- MDP-226's routed-context schema/readiness/binding implementation.
- MDP-230's governed v2 synthetic source-binding/request/results/normalized
  chain implementation.
- MDP-231's observed/recomputed driver, executable, configuration, and model
  parameter identity implementation. MDP-237 integrates its stable helper and
  provenance states; it does not invent a parallel hash algorithm.
- Provider calls, credentials, retries, model quality, external storage,
  CRM/email actions, host-side secret management, and changes in other repos.
- Deterministic run-request compilation, a remote job scheduler, a pack
  registry, or automatic `mdp run` after preparation.
- Changes to MDP-226/230/231/239 Linear status, labels, relations, or
  delegation metadata. Their current blockers remain visible until their
  owners complete them.

## Planning Contract

### Key Technical Decisions

- **KTD1 — Add `prepare-run`, do not overload `run`.** `prepare-run` is a
  read-only compiler with optional explicit request/manifest output paths.
  `run` stays the only command that stages a run directory or reaches the
  native driver. This keeps preparation safe for agent planning and keeps the
  existing v1 execution action stable.
- **KTD2 — Keep the wire request unchanged.** Add
  `RUN_REQUEST_COMPILE_V1 = "mdp.run-request-compile.v1"` for the result and
  preflight manifest. Continue producing the existing closed
  `RunRequestV1`; do not add compiler-only fields to it or make MCP fields
  execution authority.
- **KTD3 — Use one typed compiler kernel.** Put derivation in a reusable
  `run_request_compiler` module with `PrepareRunOptions`,
  `CompiledRunRequest`, and bounded diagnostic types. The CLI and MCP wrappers
  only validate transport arguments, invoke the kernel, and serialize its
  result. No JavaScript or shell implementation may recompute hashes.
- **KTD4 — Resolve model steps before accepting inputs.** Use
  `resolve_selected_model_step` and `CompiledModelStepV1` as the sole source
  of operation, phase, prompt ID/version/path/hash, declared input names,
  output contract, and job binding. If a job has multiple model steps, require
  exact `--operation model:<job>/<phase>`; if it has one, the compiler may
  select it deterministically and reports that selection.
- **KTD5 — Derive stable IDs by domain-separated canonical hashing.** Build a
  canonical compilation identity from contract version, portable pack digest,
  manifest ID/version, profile/job/operation, ordered input names and SHA-256s,
  requested model, canonical effective parameter object, execution policy,
  and retention choice. Derive `idempotency_key` and a portable bounded
  `execution_id` from separate domain labels such as
  `mdp.run-idempotency.v1` and `mdp.run-execution.v1`. Derive
  `pack_release_id` from a separate `mdp.pack-release.v1` tuple. Never use
  filesystem paths or wall-clock time in these hashes.
- **KTD6 — Treat time as metadata, not identity.** Normal operator runs get a
  UTC RFC3339 `created_at` from the compiler clock. A bounded `--created-at`
  value is accepted only as an explicit reproducibility/test input, is
  validated as RFC3339, is excluded from all identity hashes, and is surfaced
  as caller-supplied metadata in the compile manifest. MCP accepts the same
  field so parity fixtures can freeze time without hand-authoring an execution
  ID.
- **KTD7 — Derive effective policy and parameters from typed allowlists.** The
  compiler fixes `private-staging`, `none` tools, the official OpenAI
  responses endpoint, the existing native input/output/timeout limits, and
  the permitted environment names. It accepts a requested model and only
  non-secret, schema-validated native parameter overrides; it canonicalizes
  the resulting parameter object and computes its hash. Unknown parameters,
  alternate endpoints, ambient environment names, or provider options outside
  the allowlist fail closed.
- **KTD8 — Integrate MDP-231 identity observations.** A shared identity helper
  returns bundled driver source hash, resolved regular Node executable/hash,
  effective non-secret configuration hash, dependency identity, model
  parameter hash, and provenance/state. The compiler refuses if required
  observations are unavailable or contradictory. Runtime execution consumes
  the same helper/fields, so the compiler cannot pass a request that the
  driver later interprets under a different identity.
- **KTD9 — Preserve exact bytes and path safety.** Reuse
  `pack_content_snapshot`, `sha256_hex`, `canonical_json_sha256`, bounded
  regular-file reads, and existing pack path resolution. The compiler stores
  source paths only in the local request as required by v1, but manifests and
  diagnostics use safe logical names; no symlink, hard-link race, directory,
  oversized, or changing file is accepted.
- **KTD10 — Make preflight evidence explicit.** The result separates
  `derived`, `observed`, `anticipated`, `required-at-execution`, and
  `verified` states. Preflight may say an endpoint is authorized by policy,
  but must say provider authorization is not observed. It cannot claim a
  provider response, runtime execution, or post-run verification.

### High-Level Design

```text
pack root + job + operation/phase + declared input paths + requested model
                               |
                   mdp prepare-run (offline)
                               |
  read/validate manifest -> portable pack digest -> resolve model step/prompt
       -> open exact declared files -> validate schemas/lineage/budgets
       -> derive pack/job/ID/policy/parameter/driver/runtime identities
       -> build closed mdp.run-request.v1 -> schema + canonical-byte check
              | fail: bounded compiler code, no request/manifest write
              | pass: concise result + optional exact request/full manifest
                               |
       explicit mdp run --request ... --out-dir ...
       -> existing staging, v2 sealing, permission gate, provider/native driver
       -> bundle/audit/receipt (unchanged execution authority)

 CLI prepare-run ─┐
                  ├─ same Rust compiler kernel ── byte-identical request/result
 MCP mdp_prepare_run┘
```

### Implementation Units

#### U1. Define the compile result and compiler kernel

- **Goal:** Introduce the typed, reusable authority that constructs a
  generative request without changing execution semantics.
- **Primary files and symbols:**
  - `cli/src/main.rs`: register `mod run_request_compiler;`.
  - `cli/src/run_contracts.rs`: add
    `RUN_REQUEST_COMPILE_V1`, compile result/manifest/diagnostic types, and
    serde-safe provenance/assurance fields. Keep `RunRequestV1`,
    `ExecutionPolicy`, `DriverIdentity`, `ModelIdentity`, and
    `LocalArtifactInput` wire-compatible.
  - `cli/src/run_request_compiler.rs`: add
    `PrepareRunOptions`, `CompileClock`, `CompiledRunRequest`,
    `RunRequestCompileManifest`, `CompilerDiagnostic`,
    `compile_native_run_request`, `write_compiled_request`, and bounded file
    identity helpers. Keep source-path strings out of diagnostics except for
    explicit safe output destinations.
  - `cli/src/artifact_hash.rs`: reuse existing canonical JSON/domain hashing,
    `pack_content_snapshot`, `pack_content_sha256`, and bounded file hashing;
    add domain labels only if they belong in the shared hash helper.
  - `cli/src/pack_io.rs`: reuse `read_manifest` and `resolve_pack_path`; add a
    regular-file/no-symlink helper only if current staging helpers cannot be
    safely shared.
- **Ordered steps:**
  1. Validate `PrepareRunOptions` (pack root, job, optional operation,
     repeatable logical-name/path inputs, model, retention, output paths, and
     optional test clock) without reading provider environment or making
     network calls.
  2. Load and validate the manifest/profile, compute the portable pack
     snapshot, derive the stable release ID, and retain only safe logical
     references in the manifest.
  3. Resolve the model step with `resolve_selected_model_step`; require exact
     operation when selection is ambiguous; load the selected prompt from the
     staged pack and verify its declared ID/version/hash and output contract.
  4. Match supplied `logical_name=PATH` values to the step's declared inputs,
     reject missing/extra/duplicate names, open each regular file with the
     existing bounded/stable-read pattern, and compute byte count/SHA-256.
     Derive schema ID/media type/provenance from the selected prompt contract,
     not from a caller field.
  5. Run the released MDP-226 routed-context validator and MDP-230 governed
     v2 lineage validator where the selected step declares those inputs. The
     compiler must accept their exact artifacts or fail; it must not generate
     substitutes.
  6. Resolve canonical native parameters/policy and call the MDP-231 identity
     observation helper. Derive all request hashes and explicit
     `mdp-observed`/`verifier-recomputed` provenance from those results.
  7. Derive idempotency/execution/release IDs from canonical, ordered inputs;
     construct `RunRequestV1`; serialize with the repository's canonical JSON
     rules; validate the generated value through the existing run-request
     schema and `validate_request`-compatible checks.
  8. Build a full manifest with files, hashes, endpoint, boundaries, limits,
     identity evidence, authorization requirement, and anticipated assurance;
     project concise JSON by default; write `--out` and `--manifest-out`
     atomically only after every check passes.
  9. Return the exact next safe execution command without executing it. Ensure
     failed compilation leaves neither a partial request nor a provider/run
     directory.
- **Proof:** Rust unit tests for deterministic ID derivation, canonical
  serialization, option rejection, schema-valid generated requests, stable
  file reads, safe diagnostics, no-network/no-environment access, and
  atomic-write refusal. Property/table tests cover changed input/pack/driver/
  parameter identities.

#### U2. Expose `mdp prepare-run` and capability/schema contracts

- **Goal:** Give operators a discoverable command with concise/full output and
  stable flags/errors while keeping the compiler kernel transport-neutral.
- **Primary files and symbols:**
  - `cli/src/cli.rs`: add `Commands::PrepareRun` with `--dir`, `--job`,
    `--operation` (or exact model-step selector), repeatable `--input`,
    `--model`, `--retention-policy`, optional `--created-at`, `--out`,
    `--manifest-out`, and optional safe output planning fields. Do not expose
    hashes, prompt paths, execution IDs, provider keys, or inline JSON.
  - `cli/src/app.rs`: dispatch to a thin prepare wrapper, preserve global
    `--json`/`--summary`, and ensure no execution or provider environment is
    touched.
  - `cli/src/commands/mod.rs`: register/re-export the wrapper module.
  - `cli/src/commands/prepare_run.rs` (new): map Clap values to
    `PrepareRunOptions`, call the kernel, perform atomic output writes, and
    project stable compiler errors without leaking paths or content.
  - `cli/src/commands/schemas.rs`: add `run_request_compile_v1_schema()` and
    `SchemaTarget::RunRequestCompileV1`; assert closed diagnostic/manifest
    properties and the generated nested `mdp.run-request.v1` authority.
  - `cli/src/commands/capabilities.rs`: advertise `prepare-run` as
    read-only-unless-explicit-out, its result contract, flags, offline/no
    provider behavior, and safe error codes. Keep `run` capability unchanged.
  - `cli/src/commands/requirements.rs` and `cli/src/model_steps.rs`: reuse
    existing model-step/requirements projections for exact operation and
    declared-input diagnostics; do not create a second resolver.
- **Ordered steps:**
  1. Add the command and result contract to the registry and schema target.
  2. Implement concise/full projection and explicit output semantics; default
     stdout must not dump prompt/input bodies.
  3. Add command-level tests for missing/ambiguous job/step, malformed input
     mapping, unsupported model, output collisions, summary/JSON parity, and
     stable next-command formatting.
  4. Update capability snapshots and CLI help/golden tests.
- **Proof:** `cargo test` for CLI parsing, schema target, capability contract,
  output projection, and compiler integration; malformed/unknown flags prove
  caller cannot smuggle manual authorities.

#### U3. Share runtime identity and execution validation with the compiler

- **Goal:** Ensure compiled identities are the same identities observed and
  enforced at execution; remove the current fake-hash fixture dependency
  without weakening the v1 validator.
- **Primary files and symbols:**
  - `cli/src/run_runtime.rs`: extract/reuse
    `BUNDLED_NATIVE_DRIVER_SOURCE`, `resolve_node_executable`, native limits,
    provider boundary constants, model-parameter canonicalization, and
    `validate_request`; make execution consume the shared identity result.
    Preserve `execute_run`, `execute_generative_step`,
    `validate_native_request_size_before_bundle`, staging, v2 sealing, and
    default-deny authorization ordering.
  - `cli/src/run_request_compiler.rs`: call the shared helper rather than
    hashing bundled source/Node/config/model values independently.
  - MDP-231's delivered identity module/API (exact file/symbol to be pinned
    when MDP-231 lands): expose observed/recomputed state and evidence refs;
    reject unavailable/contradictory observations.
  - `cli/src/run_contracts.rs`: preserve existing enum/string values; add only
    compiler provenance/observation fields outside `RunRequestV1` if needed.
- **Ordered steps:**
  1. Identify the MDP-231 helper's stable public-in-crate boundary and add a
     compile-time integration test so drift cannot silently recreate a second
     hash path.
  2. Move identity derivation behind that helper while retaining runtime
     checks that compare request identity to the actual driver/runtime bytes.
  3. Replace fake `b`/`c`/`d` identity values in generative fixtures with
     compiler-produced values; keep one deliberately tampered-request test to
     prove execution still rejects altered authority.
  4. Ensure compiler-only tests cannot require `OPENAI_API_KEY`,
     `MDP_ALLOW_NATIVE_MODEL_CALLS`, or network access; only the explicit run
     path may inspect those execution permissions.
- **Proof:** runtime tests for identity mismatch/unavailable observations,
  compiler-to-runtime round-trip, altered request rejection, and no-provider
  execution. Existing v2 driver request/result/bundle/receipt tests remain the
  execution regression suite.

#### U4. Integrate MDP-226 and MDP-230 as released input contracts

- **Goal:** Make preparation honor the upstream gates without taking their
  ownership or duplicating their compilers.
- **Primary files and symbols:**
  - `cli/src/run_request_compiler.rs`: invoke the released routed-context
    identity/readiness helper and governed v2 artifact validator at the same
    input boundary used by runtime.
  - `cli/src/run_runtime.rs`: keep existing execution call sites aligned with
    the compiler's generated input authority; no placeholder readiness checks.
  - `cli/src/routing.rs`, `cli/src/commands/prompt_output.rs`, and the MDP-226
    changed files: consume their canonical context result only.
  - MDP-230's source-binding/request/collected-results/normalized-input
    validators and exact contract modules: consume their result only.
- **Ordered steps:**
  1. Add dependency contract tests that compile a ready MDP-226 artifact and
     reject malformed/stale/wrong-job/wrong-pack context.
  2. Add dependency contract tests for MDP-230's synthetic lineage and reject
     altered hashes, missing predecessors, cross-job artifacts, and unsupported
     v2 states.
  3. Preserve the current `validate_step_inputs` ownership for names and
     requiredness; make the upstream validators own semantic readiness and
     lineage.
  4. Fail closed with a bounded next-safe-command diagnostic when the required
     upstream helper/contract is unavailable; never silently fall back to the
     0.1.73 placeholder behavior.
- **Proof:** synthetic GTM/proposal fixtures, focused cross-contract tests,
  and full native parity with no real provider call. The test fixture must
  assert no request file is written on either upstream refusal.

#### U5. Make CLI/MCP the same preparation authority

- **Goal:** Add a read-only MCP preparation tool without creating a second
  request compiler or accepting unsafe inline authority.
- **Primary files and symbols:**
  - `scripts/mdp-run-mcp-server.mjs`: add `mdp_prepare_run` to the tool list,
    schema, argument allowlist, and handler. Accept canonical pack/job,
    operation, input path mappings, model, retention, deterministic test clock,
    and explicit output paths only. Spawn `mdp --json prepare-run`; do not
    inspect source bodies, provider environment, or recompute hashes. Freeze
    bounded output paths using existing safe-file helpers.
  - `scripts/test-run-mcp-server.mjs`: update tool inventory and add valid,
    parity, malformed-argument, symlink/race/oversized-path, timeout, no-key,
    and no-inline-body tests. Compare CLI and MCP request bytes/hashes.
  - `cli/src/commands/capabilities.rs` and `scripts/mdp-run-mcp-server.mjs`:
    state CLI authority for compile/result/identities/assurance and empty MCP
    authority; preserve existing `mdp_run`/`mdp_verify_run` semantics.
- **Ordered steps:**
  1. Define a narrow MCP input schema mirroring only the compiler's typed
     options; reject `request`, prompt body, input content, hash fields,
     credential values, and unknown arguments before spawn.
  2. Forward only bounded safe paths and non-secret scalar options to the
     installed/source CLI; preserve sanitized child error behavior.
  3. Return `mdp.run-request-compile.v1` unchanged in `structuredContent` and
     keep transport metadata separate from compiler authority.
  4. Test direct CLI and MCP with the same frozen `created_at`; compare exact
     request bytes, request SHA, full manifest SHA, diagnostics, and next
     command. Test that MCP never invokes `mdp run` as a side effect.
- **Proof:** `node --test scripts/test-run-mcp-server.mjs`, CLI/MCP fixture
  parity, and a no-network assertion around the compiler child.

#### U6. Replace manual production-shaped fixtures and update the operator surface

- **Goal:** Ensure the released happy path uses the compiler while preserving
  low-level compatibility tests and avoiding hand-authored identity guidance.
- **Primary files and symbols:**
  - `scripts/test-universal-native-parity.mjs`: replace the manual
    `runRequest` object, fake hashes, and routed-context `{status: "ready"}`
    placeholder with a `prepare-run` invocation using exact synthetic
    declared artifacts and frozen clock. Assert request/manifest hashes and
    preserve bundle/audit/receipt checks. Keep a separate altered-request
    negative fixture for execution validation.
  - `scripts/test-run-conformance.mjs`, native/proposal runner fixtures, and
    relevant release/install smoke scripts: add generated-request coverage and
    leave hand-authored requests only where the test explicitly verifies wire
    compatibility or rejection.
  - `plugin/skills/mdp/SKILL.md`: teach `prepare-run`, offline preflight,
    explicit `run`, and `verify-run`; remove happy-path manual hash/request
    construction.
  - `plugin/skills/mdp/references/cli-operator.md`: document compiler inputs,
    concise/full manifest, safe diagnostics, and compatibility-only manual
    request use.
  - `cli/USAGE.md`, `docs/getting-started.md`, `docs/host-conformance.md`,
    `docs/native-api-normalization-runner.md`, and README command inventory:
    replace manual request recipes and state MCP/CLI shared authority.
- **Ordered steps:**
  1. Add public synthetic fixture builders for exact prompt/input/routed/
     governed artifacts without private data or provider bodies.
  2. Convert parity and install smoke to compile first, assert no network,
     then execute explicitly with native permission denied.
  3. Update skill/docs examples to use logical input mappings and generated
     manifest output; retain a warning that `mdp run` is a separate action.
  4. Run installed skill packaging/contract checks and verify no docs teach
     caller-authored execution or hash fields.
- **Proof:** universal native parity, proposal/native runner smoke, skill
  contract/eval tests, installed CLI/plugin smoke, and documentation grep for
  forbidden manual hash guidance.

#### U7. Release validation and handoff evidence

- **Goal:** Prove the implementation against the repository's release gates
  and hand off an auditable implementation PR.
- **Primary files/commands:** `Makefile`, `make validate`, `make
  validate-skills`, `make validate-instructions`, CLI/MCP/conformance scripts,
  and the implementation PR linked to MDP-237. No release artifact is changed
  by this plan-only branch.
- **Ordered steps:**
  1. Run focused Rust compiler/schema/runtime tests and all new Node tests.
  2. Run `cargo fmt --check`, `make validate`, and `eve`/plugin checks required
     by the repository; record any external registry limitation precisely.
  3. Run the installed CLI plus installed plugin skill smoke and one complete
     synthetic preflight with no provider key/network.
  4. Review the diff for private paths, raw bodies, secrets, hash literals,
     accidental status/label changes, and scope creep into MDP-226/230/231.
  5. Open the implementation PR from its task branch only after code review;
     do not merge, enable auto-merge, or push to `main`.
- **Proof:** full validation output, focused test names, CLI/MCP request SHA
  equality, no-network evidence, installed smoke result, and a PR description
  mapping every acceptance example to evidence.

## Dependencies, Risks, and Mitigations

| Dependency or risk | Why it matters | Mitigation/owner |
| --- | --- | --- |
| MDP-226 — canonical routed-context readiness | The compiler must not emit a request containing a placeholder or stale context. | Remains a blocking dependency, currently `Backlog`/`phase:planned`; consume its released validator and exact bytes. Do not patch its status or duplicate its gate. |
| MDP-230 — governed v2 synthetic input chain | Generated requests must bind exact source/attempt/results/normalized lineage. | Remains a blocking dependency, currently `Backlog`/`phase:planned`; consume its validators and fixture contracts. Do not regenerate or rebind artifacts in MDP-237. |
| MDP-231 — observed driver/model identity | Current requests accept self-attested `b`/`c`/`d`-style hashes, defeating the compiler goal. | Remains a blocking dependency, currently `Backlog`/`phase:planned`; integrate its shared observation/recompute helper and fail closed when unavailable. |
| MDP-239 parent gate | Parent explicitly sequences MDP-237 after the three blockers and requires focused tests/release evidence. | Leave MDP-239 `Backlog`/`phase:planned`; update only MDP-237 with this plan handoff. |
| Existing v1 wire compatibility | Changing `RunRequestV1` would break hand-authored fixtures, MCP run, and installed consumers. | Add compile-result contract only; generate the unchanged v1 request and retain compatibility tests. |
| Deterministic IDs collide on replay | Same canonical inputs intentionally produce the same identity and may target an already-used output directory. | Make the collision visible in preflight; require a new caller-selected `mdp run --out-dir` path for a separate materialization, never silently mutate identity. |
| Wall-clock timestamps break byte parity | CLI/MCP processes otherwise produce different `created_at` values. | Keep time outside identity; allow validated deterministic test clock and use one kernel result for both transports. |
| Hash/provenance drift | Duplicated JS/Rust hashing or a provider-side resolution could diverge. | Rust kernel is sole authority; MCP forwards; MDP-231 helper is shared; compare exact request/manifest SHA in parity tests. |
| Input path races/symlinks | A file can change after preflight or escape the declared boundary. | Reuse stable bounded regular-file reads, no symlink/hard-link acceptance, hash exact bytes, and revalidate at execution staging. |
| Provider/network leakage | Preparation must be safe in offline agent environments. | No provider imports/HTTP in compiler; never read native permission/key; test with blocked network and absent environment. |
| Large prompt/input/request | A compiler could produce a request that passes schema but fails native limits later. | Apply existing 128 KiB declared-input, 2 MiB serialized native request, output, and timeout limits before writing authority. |
| Unsafe diagnostics | Paths, source bodies, or credentials in errors would violate repository privacy rules. | Bounded stable codes and logical names only; add redaction tests and inspect full/concise output. |
| CLI/MCP argument drift | Separate schemas could reintroduce a second authority. | Define typed kernel options once; MCP mirrors only allowed scalar/path fields and parity-tests exact bytes. |
| Skill/install drift | Users may continue hand-authoring hashes after code ships. | Update only retained `plugin/skills/mdp` files and docs; run skill contract/eval/packaging and installed smoke. |
| External validation services unavailable | `make validate` may reach registry/Pluxx hooks. | Run all local focused checks; report the exact unavailable target and do not weaken gates or claim a pass. |

## Sequencing and Dependency Gate

1. **Gate A — upstream contracts available:** MDP-226, MDP-230, and MDP-231
   each have their implementation/plan evidence and stable symbols/contracts
   available from their own task branches or merged `origin/main`. If any is
   unresolved, stop at the dependency test and do not provide a fallback
   compiler that weakens the contract.
2. **Gate B — kernel and wire result:** implement U1 and U2; generated
   requests validate as unchanged `mdp.run-request.v1`, compile result schema
   is closed, and focused tests pass without a provider.
3. **Gate C — runtime identity integration:** implement U3 and prove a
   compiler-generated request passes ordinary preflight but rejects any
   post-compile identity mutation at execution.
4. **Gate D — upstream artifact integration:** implement U4 and synthetic
   ready/blocked/altered lineage coverage.
5. **Gate E — transport and operator surface:** implement U5 and U6; assert
   CLI/MCP byte parity and remove manual production-shaped examples.
6. **Gate F — release evidence:** implement U7, run repository gates, perform
   code review, and publish an implementation PR with MDP-237 linked. This
   plan branch itself remains plan-only.

## Verification Contract

### Focused Rust tests

- `run_request_compiler` unit tests for option parsing, operation selection,
  pack release/idempotency/execution ID derivation, canonical request bytes,
  schema closure, parameter/policy derivation, safe diagnostics, stable file
  identity, atomic writes, and no-environment/no-network behavior.
- `commands::schemas` tests for `mdp.run-request-compile.v1` closed schema,
  nested v1 request validity, concise/full projection, and unknown-field
  rejection.
- `commands::capabilities` and CLI parser tests for command registration,
  exact flags, read-only classification, forbidden manual authority fields,
  and stable error codes.
- Runtime round-trip tests: compiler request -> `validate_request` -> normal
  `execute_run` with a driver spy; assert no driver invocation on compiler
  refusal or identity mismatch and preserve existing bundle/audit/receipt
  hashes.
- MDP-226/230 contract tests for ready/blocked/stale/wrong-job/wrong-pack
  routed context and exact governed v2 lineage, with no generated request on
  failure.

### CLI/Node/MCP tests

- `scripts/test-run-mcp-server.mjs`: tool inventory, narrow argument schema,
  safe path handling, no inline bodies/credentials, timeout/output bounds,
  no-provider side effect, and exact CLI/MCP request and manifest SHA parity.
- `scripts/test-universal-native-parity.mjs`: all declared GTM/proposal model
  steps compile from synthetic artifacts, then execute through existing
  default-deny native boundary; assert prompt/input/pack/driver/model/policy
  authorities and receipts.
- Conformance and native/proposal runner tests: hand-authored v1 request
  compatibility/negative fixtures remain explicit and compiler-generated
  requests become the happy path.
- Installed CLI/plugin smoke: run the installed `mdp` skill's prepare -> review
  manifest -> explicit run -> verify flow with synthetic input and no provider
  key; compare source and installed contract/capability surfaces.

### Repository validation

- `git diff --check` and Markdown/frontmatter/required-plan-section checks.
- `cargo fmt --check` and focused `cargo test` targets, followed by full
  `make validate` (including skill, instruction, installer, parity, and public
  artifact targets).
- Confirm no network/provider call during compile with a blocked/absent
  environment test and no output directory creation.
- Review generated fixtures and logs for raw input bodies, private paths,
  credentials, cookies, or unbounded error text.

## Compatibility and Rollback

### Compatibility

- `mdp run --request` remains the execution entry point and accepts existing
  valid `mdp.run-request.v1` files. No existing request field is renamed,
  removed, or made compiler-only.
- Existing `mdp_run` MCP and `mdp_verify_run` tool names, arguments, result
  contracts, and transport-only semantics remain unchanged. The new
  `mdp_prepare_run` tool is additive and read-only.
- Existing hand-authored request fixtures stay in compatibility tests, but
  public skill/docs happy paths use the compiler and do not teach manual hash
  authoring.
- Generated IDs are stable for identical canonical inputs; the compiler
  version/domain contract participates in the identity, so future derivation
  changes cannot silently replay an older authority.
- Output paths are explicit writes only. Without `--out`/`--manifest-out`,
  preparation emits concise data and does not write a request, run directory,
  or provider artifact.

### Rollback

1. If compiler validation or install smoke fails before release, revert only
   the implementation PR. The existing `mdp run` and v1 request path remain
   available because the wire contract was not changed.
2. If a released compiler emits an identity/provenance regression, stop using
   `prepare-run`, preserve the affected request/manifest as local evidence,
   and use the last known-good compiler or an existing reviewed v1 fixture
   only under the compatibility/approval path. Do not edit a generated
   request in place.
3. If MCP preparation misbehaves, remove/disable only the additive
   `mdp_prepare_run` dispatch while keeping `mdp_run` and `mdp_verify_run`
   unchanged; direct CLI preparation remains the diagnostic fallback.
4. If MDP-226/230/231 contracts change, fail closed at the dependency gate and
   update the compiler integration in a new reviewed change. Do not restore
   placeholder readiness or fake hashes to keep old fixtures green.

## Acceptance Mapping

| Linear acceptance | Planned evidence |
| --- | --- |
| 1. One command compiles a schema-valid sealed request without caller hashes | U1/U2 compiler option rejection, generated nested v1 schema test, AE1, focused CLI integration test. |
| 2. Runtime-observable driver/executable/prompt/pack/parameter identities derive from MDP | U1/U3 shared identity integration, MDP-231 dependency gate, altered-identity round-trip test, AE2/AE7. |
| 3. Preflight displays exact files/hashes, endpoint, boundary, limits, assurance | Compile manifest/projection in U1/U2, redaction tests, AE3, full-manifest fixture assertions. |
| 4. Compilation is offline/no provider | U1 no-environment/no-network tests, MCP child boundary, blocked-network parity, AE3. |
| 5. Execution is a separate explicit action with provider authorization | KTD1, U6 prepare-then-run smoke, unchanged `mdp run` permission gate, AE4. |
| 6. Errors name exact missing artifact/contract and next safe command | U1/U2 stable `CompilerDiagnostic` table and malformed/blocked dependency fixtures, R8. |
| 7. Concise JSON default plus explicit full manifest/file option | U1/U2 default projection, `--out`, `--manifest-out`, schema/capability/help tests, R3. |
| 8. CLI and MCP use one kernel and byte-equivalent authority | U5 shared typed options, direct CLI/MCP request/manifest SHA comparison, AE8. |
| 9. Skill happy path uses compiler instead of manual hashes | U6 skill/reference/docs updates, skill contract/eval/packaging checks, forbidden-guidance grep, AE9. |
| 10. Synthetic GTM/proposal ready/altered/unsupported fixtures | U4/U6 fixture matrix and universal native parity/conformance tests, AE5/AE6/AE7. |

## Definition of Done

- [ ] MDP-226, MDP-230, and MDP-231 dependency contracts are available and
  their blocker state is acknowledged in the implementation PR; no upstream
  issue is silently changed.
- [ ] `mdp prepare-run` and `mdp.run-request-compile.v1` are implemented,
  documented, capability-advertised, schema-validated, and offline.
- [ ] Generated `mdp.run-request.v1` contains no caller-authored identity or
  hash authority and passes the existing execution validator.
- [ ] CLI/MCP preparation is one-kernel, path-only, byte-equivalent, and
  transport-only; existing MCP run/verify behavior is unchanged.
- [ ] MDP-226 routed-context and MDP-230 governed v2 artifacts are consumed
  exactly; MDP-231 observations are shared with runtime; no placeholders or
  fake hashes remain in the happy-path parity harness.
- [ ] Focused tests, full `make validate`, installed skill/CLI smoke, and
  no-network/no-provider evidence pass, or any external validation limitation
  is recorded without weakening the gate.
- [ ] Code review is complete and the implementation PR is linked to MDP-237;
  this plan-only artifact is the sole change in the planning branch.
