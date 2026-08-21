---
title: "fix: Bind driver configuration and model parameter hashes to observed runtime values"
type: bug
date: 2026-08-21
execution: code
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-231
linear_issues:
  - MDP-231
  - MDP-239
repository: orchidautomation/message-decision-packs
base_branch: main
base_commit: 2cba9919483b5a7ba46efed53e3b5502b2abf477
source_branch: codex/mdp-231-plan
---

# MDP-231: bind driver configuration and model-parameter hashes to observed runtime values

## Goal capsule

| Field | Decision |
| --- | --- |
| Objective | Replace caller-supplied SHA-shaped `driver.configuration_sha256` and `model.parameters_sha256` claims with hashes MDP recomputes from closed, canonical, runtime-observed projections. |
| Authority | Rust owns the native-run identity projections, the staged run bundle, the runner audit, and independent receipt verification. The JavaScript driver reports the exact provider-request-body hash but does not choose MDP assurance. |
| Evidence vocabulary | The run request carries a declaration; the sealed bundle/audit carries the bounded runtime observation; `verify-run` independently recomputes and reports verification. These are separate fields and stages, never one caller-selected assurance label. |
| Compatibility | Keep `mdp.run-request.v1`, `mdp.run-bundle.v1`, `mdp.driver-request.v2`, `mdp.runner-audit.v1`, and `mdp.run-receipt.v1` stable. Add optional closed identity-observation data for generative runs and require it for new strongest-tier verification; deterministic and external v1 paths remain unchanged. |
| Product boundary | No provider call, credential handling, source collection, CRM/outbound action, raw provider body, prompt/input content, or secret enters a canonical identity projection or ordinary diagnostic. |
| Stop condition | A native run cannot publish a strongest-tier identity from a guessed hash; every underlying provider-affecting value changes the model-parameter projection; the provider-body hash remains preserved with an explicit relationship to the projections; focused, receipt-verifier, parity, `make validate`, and installed no-provider smoke all pass. |
| Execution state | This document is the implementation-ready plan only. MDP-231 remains `Backlog`/`phase:planned` under the MDP-239 parent gate; this plan does not authorize implementation or change issue relations/delegation. |

## Repository routing and handoff

- Repository: `orchidautomation/message-decision-packs`
- Base: `main` at `2cba9919483b5a7ba46efed53e3b5502b2abf477` (`origin/main`, v0.1.73)
- Planning source ref: `refs/heads/codex/mdp-231-plan`
- Tracked plan path: `docs/orchid/plans/2026-08-21-001-mdp-231-observed-driver-model-identity-plan.md`
- The isolated planning checkout is `/private/tmp/mdp-231-plan-work`; the canonical checkout's dirty files are out of scope and must remain untouched.
- The implementation branch must be a new task branch from current `origin/main`. Keep the local production mirror `main` clean.

## Problem frame

The current generative request accepts hashes that only satisfy a 64-character
lowercase-hex check:

- `cli/src/run_contracts.rs::DriverIdentity` exposes
  `configuration_sha256`, and `ModelIdentity` exposes `parameters_sha256`.
- `cli/src/run_runtime.rs::validate_request` checks only `is_sha256` for those
  fields. It does not derive either value from the runtime it is about to use.
- `cli/src/run_runtime.rs::execute_transaction` copies the caller's driver and
  model identities into `RunBundleV1` before the native step executes.
- `cli/src/run_runtime.rs::invoke_native_driver` does observe the bundled
  driver script bytes and the resolved Node executable bytes, and enforces the
  fixed endpoint/allowlisted environment, but that evidence is not bound to
  `configuration_sha256`.
- `cli/src/run_runtime.rs::invoke_native_driver` sends a closed native request
  containing the provider, model, visible input, projected output schema,
  schema hash/name, timeout, and output-token bound. The JavaScript driver
  constructs the exact OpenAI Responses body with `store: false` and
  `tool_choice: "none"`, then returns `provider_request_body_sha256`.
- `cli/src/commands/run_verification.rs::verify_runner_audit` currently checks
  hash shape and provider evidence presence but does not independently
  recompute configuration or parameter identities.

Consequently, `"b".repeat(64)` and `"c".repeat(64)`-style values can appear in
the bundle as provenance-bearing metadata without identifying the observed
driver configuration or native provider parameters. The provider request-body
hash is useful stronger evidence, but it is not a substitute for either
identity: it covers the full serialized body, including model-visible input
and schema bytes, while configuration and parameter projections must remain
bounded and secret-free.

## Product contract

### Declared, observed, and verified values

1. The incoming `RunRequestV1.driver.configuration_sha256` and
   `RunRequestV1.model.parameters_sha256` are caller declarations. They remain
   accepted as input fields for compatibility, but a SHA-shaped declaration is
   never evidence by itself.
2. Before the generative bundle is sealed, Rust builds the two projections
   below from the staged pack, selected model step, effective closed native
   policy, bundled driver source, and resolved Node executable. It computes the
   observed hashes with the existing domain-separated canonical JSON helper.
3. A native request whose declaration does not equal the observed hash fails
   closed before a final run directory, bundle, receipt, or provider call is
   published. This issue chooses the fail-closed branch for the supported
   MDP-owned native driver; a future explicitly attested driver may use a
   declared/unverified state but is not widened by this work.
4. The sealed bundle uses the observed identity values. The runner audit also
   records a bounded identity-observation object containing the declaration,
   observation, projection contract, and the provider-body relationship. This
   keeps declaration and observation structurally distinct without retaining
   raw configuration, prompt, input, credential, or provider-body content.
5. `verify-run` reconstructs the projections from the sealed artifacts and
   runtime contract, compares its recomputed values with the observed audit
   values and bundle values, and emits a verification issue on any mismatch or
   missing observation. A verifier result is the only `verified` stage; it is
   not copied into a caller-controlled hash field.

### Canonical projection: `mdp.driver-configuration.v1`

The driver configuration projection is a closed JSON object containing only
values MDP can observe or enforce for the bundled native launch. It includes:

- driver ID, implementation ID, and runtime version;
- the observed SHA-256 of the bundled `scripts/mdp-native-model-openai.mjs`
  source and the observed SHA-256 of the resolved Node executable;
- the fixed native subprocess request/result contract IDs;
- `clear_env: true`, the exact allowlisted environment variable *names*
  (`MDP_ALLOW_NATIVE_MODEL_CALLS` and `OPENAI_API_KEY`), and the fact that
  secret values are out of band and excluded;
- private-staging working-directory mode, bounded stdin/stdout, request and
  response limits, timeout enforcement, fixed official endpoint, redirect
  rejection, proxy exclusion, and `store: false`/no-tools transport policy;
- only observed or fixed policy values, never caller-selected paths, process
  IDs, timestamps, API keys, environment values, raw prompt/input text, or
  provider response bodies.

Caller-provided optional build/image attestations remain separate declaration
fields. They are not silently folded into an MDP-observed configuration hash
unless the runtime can independently observe them.

### Canonical projection: `mdp.model-parameters.v1`

The model-parameter projection is generated from the exact prepared native
request and its closed provider policy. It contains all provider-affecting
values without copying model-visible content:

- provider, requested model, fixed authorized endpoint, and declared run
  timeout policy;
- `max_output_tokens` as derived by the native runtime from the bounded
  output policy;
- structured-output mode, schema name, provider-output-schema SHA-256, and
  the exact input framing (`one fresh user message`, `declared_inputs_only`);
- `store: false`, `tool_choice: "none"`, and the absence of continuation,
  conversation, tools, proxy, or caller-selected endpoint fields;
- a SHA-256 of the fully assembled model-visible input envelope, rather than
  that envelope's content, so prompt/input changes are represented without
  leaking them.

The projection deliberately excludes the dynamically decreasing remaining
deadline from the caller's parameter declaration. The declared timeout is a
stable model/transport policy input; the effective per-call timeout remains
bound by `mdp.driver-request.v2` and its exact request hash. Any future native
parameter (for example reasoning or metadata) must be added to this closed
projection and to the JS/Rust parity tests before it can affect the provider
body.

### Provider request-body relationship

`RunnerAuditV1.identity_observations.provider_request` retains:

- the exact `provider_request_body_sha256` returned by the MDP-owned native
  transport when a body was assembled;
- the provider request schema ID;
- a closed relation such as `full-body-includes-model-parameters-and-input`,
  explaining that the body hash covers the serialized provider body while the
  parameter hash covers its bounded options/projections and the configuration
  hash covers the launcher/transport authority;
- explicit `not-observed` state for a failure that occurs before the driver
  assembles a provider body.

The body hash remains independently validated for shape/presence under the
existing success rules. It is never used as, or substituted for, either the
configuration or model-parameter hash. The projection and relation contain
hashes, IDs, enums, and bounded numbers only; no API key, authorization header,
raw input, raw provider request, or provider error text is retained.

### Scope boundaries

In scope:

- Runtime-owned driver configuration and model-parameter projection helpers.
- Additive closed identity-observation data in the native runner audit and
  exported schema.
- Binding/verification of the observed hashes to the run bundle and receipt.
- Native OpenAI JS projection parity, focused negative fixtures, installed
  no-provider preflight, and contract documentation.

Out of scope:

- Adding providers, custom endpoints, new model parameters, retries, or
  changing the native provider protocol beyond the projection metadata needed
  to prove the current implementation.
- Hashing or retaining secrets, raw provider bodies, prompt text, input text,
  model output, private paths, or customer data in identity material.
- Replacing provider request-body hashing, claiming provider-to-model
  transformation visibility, or upgrading driver/host attestation to MDP
  observation without an enforcing runtime boundary.
- Changing `mdp.run-request.v1`/bundle/receipt contract IDs, deterministic run
  behavior, external driver v1 semantics, MDP-239 relations, issue labels,
  delegation, or readiness state.
- A live provider call or a real native receipt. MDP-149 remains separately
  human-gated.

## Planned implementation surfaces

The implementation should stay within the following surfaces unless a focused
test proves a narrower helper extraction is required.

| File | Existing symbols / responsibility | Planned change |
| --- | --- | --- |
| `cli/src/run_contracts.rs` | `DriverIdentity`, `ModelIdentity`, `RunnerAuditV1`, contract constants | Add versioned projection/identity-observation constants and closed Rust structs for declared/observed identity evidence plus provider-body relation. Add the optional audit field with serde defaults so old deterministic/external receipts remain readable. Keep existing identity fields as request declarations and bundle-observed values after runtime binding. |
| `cli/src/run_runtime.rs` | `NativeSubprocessRequestV1`, `invoke_native_driver`, `execute_transaction`, `validate_native_request_size_before_bundle`, `execute_generative_step`, `validate_request`, `seal_driver_request`, `assurance_dimensions`, runtime tests | Extract one preparation path that resolves the selected step, builds the exact native request/projection, and performs routed/input/schema/size checks before bundle sealing. Add `driver_configuration_projection`, `model_parameters_projection`, `bind_native_identity`, and bounded observation/relation helpers. Observe the bundled JS source and resolved Node bytes, compare caller declarations, write observed values into the sealed bundle, carry observations into `RunnerAuditV1`, and preserve provider body hash/result handling. Map mismatches to stable sanitized policy-block codes before driver invocation. |
| `cli/src/commands/schemas.rs` | `driver_identity_v1_schema`, `model_identity_v1_schema`, `runner_audit_v1_schema`, `driver_request_v2_schema`, `v1_execution_schemas_*` tests | Export closed schemas for the two projection contracts and identity-observation/provider-relation objects. Make the new audit carrier optional for legacy receipts but closed; add generative characterization tests requiring it on newly emitted native audit fixtures and rejecting unknown fields/invalid hashes/relations. |
| `cli/src/commands/run_verification.rs` | `verify_run`, `verify_runner_audit`, `provider_request_evidence_issue`, `recompute_assurance`, verifier tests | Independently recompute both projections from the sealed bundle/audit/native request facts. Check observed hashes against bundle identities, declarations against observations, projection contract IDs, provider-body relation, and the existing provider hash/schema requirements. Add stable issues for missing, forged, mismatched, or structurally ambiguous identity evidence; do not expose raw values or paths. |
| `scripts/mdp-native-model-openai.mjs` | `buildProviderRequestBody`, `executeNativeModelRequest`, `validateNativeModelRequest`, exported SHA helpers | Add the JS-side closed model-parameter projection helper (or an equivalent test-only export) from the same provider body inputs. Keep the provider body contract, `store:false`, `tool_choice:none`, fixed endpoint, secret redaction, and result hash unchanged. Ensure any future provider-affecting field must be admitted to the projection allowlist. |
| `scripts/test-native-model-driver.mjs` | provider-body snapshots, mutation/redaction cases | Assert the projection includes each current provider-affecting option and changes when each option/schema/model/timeout/input digest changes. Assert projection/body hashes never contain credential, private endpoint, raw input, or provider-error text. Keep all calls mock/key-free. |
| `scripts/test-universal-native-parity.mjs` | generative request matrix and independent driver-request/receipt recomputation | Replace fixed `b`/`c` placeholders with independently derived declarations, inspect the sealed observed identities and provider-body relation, recompute the projection/body hashes, and add altered-declaration/altered-parameter cases that fail before the fake driver is reached. Preserve CLI receipt verification and all-template no-provider parity. |
| `scripts/release-install-smoke.sh` | installed schema loop and installed run/MCP smoke | Add one synthetic generative request against the installed GTM pack with the derived native identities and no provider permission/key. Assert the installed binary preserves observed identity evidence, the exact provider-body hash relation, and sanitized no-draft behavior; then replay with each altered hash and assert no successful run authority. Keep temporary request/output files under the isolated smoke home. |
| `scripts/test-release-install-smoke.sh` | fake installer/release fixture | Extend only if the installed smoke needs a deterministic helper to derive source/node/model projections; do not add generated artifacts or secrets to the repository. |
| `docs/run-receipts.md` | v1 contract authority table and verifier guidance | Document declaration vs observation vs verification, the two projection contracts, and the fact that provider-body SHA is exact-body evidence rather than a configuration/parameter identity. Explain legacy receipt downgrade/failure without claiming old hashes were observed. |
| `docs/host-conformance.md` | native boundary and identity guidance around the driver request/provider hash | State what MDP can observe for the bundled driver, what remains host/driver-attested, how the parameter projection relates to the full provider body, and that secrets/raw payloads never enter identity material. |
| `docs/native-api-normalization-runner.md` and `cli/USAGE.md` | native-run operator contract, if current wording mentions caller-supplied hashes | Replace any implication that callers may author truthful configuration/parameter identities. Point operators to the runtime-generated observations and sanitized verifier result. |

Do not change `scripts/mdp-run-mcp-server.mjs` merely to transport the new
fields: MCP already returns the CLI-owned run result. Do not add a provider,
second identity vocabulary, or a parallel hash implementation that can drift
from Rust's canonical authority.

## Ordered implementation steps

### 1. Characterize the current contracts and freeze the projection boundary

- Add the two projection constants and write down the exact current native
  inputs/body controls before changing runtime behavior.
- Capture the distinction between request declarations, sealed bundle
  observations, audit/provider evidence, and `verify-run` recomputation in the
  schema tests and docs.
- Confirm all current native provider-affecting values by reading both
  `NativeSubprocessRequestV1` and JS `buildProviderRequestBody`. A newly added
  JS field must fail a parity test until its projection entry and Rust
  counterpart are added.
- Preserve the fixed endpoint and environment policy. No projection may
  contain `OPENAI_API_KEY`, `MDP_ALLOW_NATIVE_MODEL_CALLS`'s value, raw input,
  raw prompt, raw provider body, or a local path.

### 2. Add closed identity-observation and projection contracts

- Define `IdentityObservationV1`/`ProviderRequestObservationV1`-style structs
  in `cli/src/run_contracts.rs` with `deny_unknown_fields`, bounded hash/enum
  values, optional legacy compatibility at the audit carrier only, and explicit
  projection contract IDs.
- Add schema helpers beside the existing driver/model/audit schema helpers.
  Keep old v1 contract IDs; the new objects are additive and are required by
  the generative verifier for new native receipts.
- Add characterization tests for accepted complete observations, legacy
  deterministic/external audits without observations, unknown-field rejection,
  malformed hash rejection, forbidden relation values, and no raw secret/body
  fields.

### 3. Prepare the exact native request once before bundle sealing

- Refactor `validate_native_request_size_before_bundle` and the duplicated
  request assembly in `execute_generative_step` into one preparation result.
  It should own selected-step resolution, prompt/input/routed-context gates,
  invocation bytes, canonical/provider schema projections, schema hashes,
  schema name, `max_output_tokens`, and the bounded native subprocess request.
- Continue running this preparation before `write_json_create_new` writes the
  immutable bundle, and keep the second defense-in-depth validation immediately
  before driver invocation.
- Keep effective remaining timeout runtime-derived and bound by the exact
  `mdp.driver-request.v2` hash. Use the stable declared timeout in the model
  parameter projection so callers can declare it before staging.
- Ensure the preparation path consumes staged bytes and the staged pack only;
  it must not reopen caller-controlled source paths after staging.

### 4. Recompute and bind the driver configuration identity

- Add a helper that observes the bundled JS source hash and canonical Node
  executable hash using the same checks already enforced in
  `invoke_native_driver`.
- Build `mdp.driver-configuration.v1` from the observed source/runtime values
  and fixed launch policy. Use `canonical_json_sha256_for_domain`; never hash a
  caller JSON blob or path string as a substitute for observed configuration.
- Compare the incoming `DriverIdentity.configuration_sha256` declaration to
  the observed digest and fail with a stable policy-block code on mismatch.
  Preserve the declaration in the audit observation, but put the observed value
  in the sealed bundle identity. Keep optional build/image claims separate and
  attested when they cannot be independently observed.
- Reuse the resulting identity in `invoke_native_driver`; there must be one
  source/node observation, not a preflight hash and a different invocation hash.

### 5. Recompute and bind the model-parameter identity

- Build `mdp.model-parameters.v1` from the prepared native request and the
  closed provider policy. Include provider, model, endpoint, declared timeout,
  derived output-token bound, provider schema name/hash, input framing,
  visible-input digest, `store:false`, `tool_choice:none`, and all current
  no-continuation/no-tools settings.
- Compute the observed digest in Rust, compare it to the incoming
  `ModelIdentity.parameters_sha256`, and fail closed with a stable sanitized
  mismatch code before bundle publication or driver invocation.
- Keep a single canonical projection implementation per language boundary:
  Rust is the runtime authority; JS exposes only a parity helper or fixture
  projection and must not decide assurance. Add a cross-language test using
  synthetic values and exact canonical hash output.
- If a future parameter is added to `NativeSubprocessRequestV1` or
  `buildProviderRequestBody`, require the projection/parity test to fail until
  the new field is explicitly covered.

### 6. Preserve provider-body evidence and seal the audit relationship

- Populate the new audit identity-observation carrier with declared and
  observed configuration/parameter hashes, projection contract IDs, and the
  provider request hash/schema ID/relation returned by the native driver.
- Preserve provider request-body hash behavior for success and no-provider
  policy-block cases. A missing body observation must remain explicit and must
  not be backfilled from the model-parameter or configuration digest.
- Keep the existing `RunnerAuditV1`, `RunReceiptV1`, terminal-state, and
  no-draft cleanup semantics. No secret or raw provider payload may enter the
  audit, receipt, stdout, stderr, MCP response, or ordinary error text.
- Extend the generated authority/receipt tests to prove that the receipt binds
  the audit bytes and the audit explains the three distinct hash scopes.

### 7. Harden independent receipt verification

- In `verify_runner_audit`, reconstruct the driver projection from the sealed
  native identity, the current installed bundled source/runtime facts when
  available, and fixed policy;
  reconstruct the model projection from the sealed request/audit facts; and
  compare both with the recorded observations.
- If the verifier cannot independently observe the bundled source or runtime
  needed for the configuration projection (for example, a receipt moved to a
  different installation), retain the recorded declaration/observation as
  integrity metadata but return an explicit declared/unknown identity result;
  never promote it to strongest-tier verification from a stored hash alone.
- Require the declaration/observation relationship for new native receipts:
  equal values for the fail-closed MDP-owned route, valid projection contract
  IDs, valid provenance, and no extra fields. Missing identity observations
  yield a stable `generative-identity-evidence-missing` issue and cannot
  receive strongest-tier assurance.
- Add tamper cases for bundle driver hash, bundle model hash, declared hashes,
  observed hashes, projection contract, provider-body relation, and provider
  request hash. The verifier must catch each mutation independently without
  printing its raw value.
- Keep old deterministic and external v1 verification behavior unchanged;
  legacy generative receipts are not silently upgraded and remain declared or
  unknown according to the explicit compatibility rule.

### 8. Add the negative fixture matrix and parity proofs

Use table-driven synthetic cases and the existing fake/counting driver seam.
At minimum cover:

- correct declarations and a successful observed identity path;
- altered `configuration_sha256` and altered `parameters_sha256` declarations;
- each driver projection input independently: source hash, Node hash, driver
  implementation/version, launch contract, environment-name allowlist, fixed
  endpoint, request/response limits, redirect/proxy policy, and native protocol;
- each model projection input independently: provider, requested model,
  endpoint, declared timeout, output-token bound, schema name, schema hash,
  input framing/digest, `store`, `tool_choice`, and continuation/tool fields;
- stale/wrong provider-body hash, missing schema ID, and relation substitution;
- no provider key or permission, where body-hash evidence may still be present,
  and a true driver-start failure where it is absent;
- an API key sentinel, private endpoint, raw prompt/input sentinel, and provider
  error sentinel that must never occur in projections, audits, receipts,
  diagnostics, stdout, stderr, or MCP output.

Every negative identity case must fail before the fake driver is called and
before a committed run directory exists. Existing native output/schema,
deterministic run, and v1 conformance tests remain green.

### 9. Prove the installed release and document the contract

- Extend `scripts/release-install-smoke.sh` using its isolated home and the
  existing synthetic GTM pack. Build an installed generative request with the
  independently derived identity declarations, run with no native-call
  permission/key, and assert the observed audit fields and sanitized outcome.
- Mutate each declaration in the installed request and assert the installed
  binary rejects it before any provider path or final run authority. Keep
  generated requests, outputs, and logs under the temporary smoke directory.
- Update `docs/run-receipts.md`, `docs/host-conformance.md`, and only the
  native operator docs whose current text is inaccurate. Explain projection
  ownership, exact provider-body relationship, legacy downgrade behavior, and
  the secret/raw-content exclusion.

### 10. Finish the implementation validation gate

- Run focused Rust and JS tests first, then the complete `make validate` gate
  and installed release smoke. Inspect generated output for hash scope and
  secret redaction rather than relying only on exit status.
- Run the repository's code review/security review before an implementation PR
  is opened. This planning branch contains no runtime implementation and must
  not claim those implementation tests passed.

## Validation contract

The implementation PR must run these repository commands from its task branch;
temporary files stay outside the checkout and no command uses a real provider
or credential:

| Command | Proof |
| --- | --- |
| `cargo fmt --manifest-path cli/Cargo.toml -- --check` | Rust formatting remains clean. |
| `cargo test --manifest-path cli/Cargo.toml run_runtime` | Native preparation, runtime binding, mismatch refusal, no-driver/no-run-directory, and identity projection tests. |
| `cargo test --manifest-path cli/Cargo.toml run_verification` | Independent audit/bundle/receipt recomputation and tamper detection. |
| `cargo test --manifest-path cli/Cargo.toml schemas` | Closed projection/audit schemas and legacy compatibility characterization. |
| `node --check scripts/mdp-native-model-openai.mjs` | Native driver syntax. |
| `node scripts/test-native-model-driver.mjs` | Provider-body projection, parameter mutation, fixed policy, and redaction matrix. |
| `node scripts/test-universal-native-parity.mjs` | Cross-template CLI/native request, observed identity, provider-body relationship, and receipt verification parity. |
| `scripts/test-release-install-smoke.sh` | Installed CLI/plugin native-run no-provider preflight and altered-hash refusal. |
| `make validate` | Full repository, template, authority, native, MCP, plugin, installer, and public-artifact validation. |
| `git diff --check` | Plan/code diff whitespace hygiene. |

Plan-only validation is limited to front matter, Markdown/diff hygiene, and
the unchanged repository baseline. It must not claim the unimplemented runtime
hash binding has passed.

## Dependencies and sequencing

| Dependency | Relationship | Execution rule |
| --- | --- | --- |
| MDP-179 | Defines run-bundle/receipt assurance tiers and compatibility vocabulary. | Reuse its declared/observed/verified boundary; do not introduce an `audit-grade` shortcut or signer claim. |
| MDP-188 / current native runtime | Supplies `DriverIdentity`, `ModelIdentity`, `DriverRequestV2`, native subprocess, exact provider-body hash, and verifier seams. | Refactor in place; preserve deterministic and external-driver behavior. |
| MDP-149 | Human-gated real native receipt work. | No live provider invocation or real receipt is part of this implementation. |
| MDP-239 | Parent execution index for the v0.1.73 sanity batch. | MDP-231 remains Backlog/phase planned until the parent handoff gate permits execution. Do not change parent status, labels, delegation, or relations. |
| MDP-234 and MDP-237 | MDP-231 currently has relations to both downstream provenance/request-boundary issues. | Preserve the existing relations; do not implement or relabel downstream work here. |
| MDP-226 and MDP-230 | Adjacent MDP-239 children required by MDP-237, not by this identity fix. | Do not duplicate their routed-context or synthetic-chain changes. Consume only current shipped contracts. |

The execution order is therefore: contract/projection schemas → pre-bundle
native preparation → runtime observations/bundle/audit → independent verifier →
negative/parity fixtures → installed smoke/docs → full validation/review. The
implementation must not be marked ready or delegated solely because this plan
is implementation-ready.

## Risks and mitigations

| Risk | Mitigation and failure contract |
| --- | --- |
| Rust and JS canonical projections drift. | Rust remains authoritative; expose a bounded JS parity helper, use exact synthetic vectors, and fail tests when a provider-affecting field is added without projection coverage. |
| Dynamic remaining timeout makes declarations impossible to precompute. | Hash the stable declared timeout in `mdp.model-parameters.v1`; bind effective timeout in the exact driver-request hash and audit relation. |
| Caller hash mismatch is silently overwritten. | Compare declarations before bundle publication; return stable sanitized policy-block codes and count zero driver calls. Preserve the declaration only in bounded audit evidence for valid runs. |
| Provider request-body SHA is mistaken for parameter/config identity. | Add closed relation metadata, separate field names/projection IDs, and verifier tests that substitute each hash independently. |
| A path, environment value, API key, or raw input enters a projection. | Build projections from allowlisted values/hashes only; reject unknown fields; scan serialized projections/audits/diagnostics with secret and private-sentinel fixtures. |
| New native driver options bypass the projection. | Keep the JS request/body allowlist closed and make parameter projection mutation tests table-driven over every current field. |
| Adding audit fields breaks old deterministic/external receipts. | Make the identity carrier optional at the v1 audit envelope, require it only for newly emitted native strongest-tier receipts, and preserve explicit legacy declared/unknown verification behavior. |
| Recomputing Node/source hashes at different phases observes different bytes. | Centralize one bounded observation helper and reuse its values for preflight, invocation authorization, bundle, and audit. |
| Existing parity fixtures depend on fixed fake hashes. | Regenerate declarations with the independent projection helper; keep old v1 fixtures unchanged and assert stale values fail closed. |
| Installed smoke accidentally needs credentials/network. | Use the existing no-permission path, assert `native_model_calls_not_allowed`, and scan output for key/private sentinels. |
| Scope expands into providers, host attestation, or release changes. | Keep the implementation surface table and MDP-239 dependency graph as review boundaries; defer new providers/attested routes to separate issues. |

## Compatibility and rollback

- Contract IDs remain `mdp.run-request.v1`, `mdp.run-bundle.v1`,
  `mdp.driver-request.v2`, `mdp.runner-audit.v1`, and `mdp.run-receipt.v1`.
  Identity observations are additive, closed fields; deterministic runs and
  external v1 driver records do not need them.
- New MDP-owned native runs require declaration/observation equality and
  publish the recomputed values. A legacy generative receipt missing the new
  carrier is never upgraded to strongest-tier identity; verification reports a
  bounded missing-evidence issue or an explicit declared/unknown downgrade.
- Existing caller requests with the old hash fields remain parseable, but
  stale/arbitrary values no longer produce a strongest-tier run. Operators
  regenerate the declarations from the released runtime projection; no pack
  source migration is required.
- The provider-body hash remains backward-compatible and retains its exact
  raw-byte meaning. It may be absent when the transport never assembled a
  body, but it cannot be synthesized from another identity hash.
- Rollback is one implementation-commit/PR revert. It removes the new
  projection enforcement and optional audit carrier while leaving authored
  packs, prompts, source bindings, and temporary smoke artifacts untouched.
  If a partially updated audit is encountered, the additive default allows
  readback and the verifier stops at the explicit legacy/missing-evidence
  boundary rather than guessing.
- No destructive cleanup, release tag, deployment, issue-state transition,
  relation edit, or external side effect is part of this plan.

## Acceptance-criteria mapping

| MDP-231 acceptance criterion | Planned proof |
| --- | --- |
| Every strongest-tier configuration/parameter hash is recomputed from canonical observed values. | Rust projection helpers use observed bundled source/Node/runtime policy and the prepared native request; runtime binds recomputed values before bundle sealing; `verify-run` independently recomputes both. |
| Declared, observed, and verified values remain structurally distinct. | Request fields are declarations; `RunnerAuditV1.identity_observations` stores declaration/observation/projection IDs; `verify-run` is the separate verification stage and never trusts a caller-selected label. |
| Arbitrary SHA-shaped substitutions fail verification or produce an explicit assurance downgrade. | Pre-bundle mismatch cases return sanitized `no-draft:policy-blocked` and never call the driver; missing legacy evidence yields an explicit bounded declared/unknown verifier result, never strongest-tier assurance. |
| Canonical model-parameter projection includes all provider-affecting values used by the native driver. | Closed `mdp.model-parameters.v1` covers provider/model/endpoint, timeout policy, token bound, schema name/hash, input framing/digest, fixed store/tool/continuation policy, and parity tests mutate every current field. |
| Driver configuration identity is derived by MDP, not hand-authored by the caller. | Rust observes the bundled JS and resolved Node bytes plus fixed launch policy; caller configuration hash is compared only as a declaration and never used as the projection source. |
| Receipts preserve the provider request-body hash and explain its relationship to parameter/config identities. | Runner audit retains exact provider-body hash/schema ID plus closed relation metadata; verifier checks presence/shape independently and never substitutes it for either projection. |
| Negative fixtures alter each declared hash and each underlying parameter independently. | Table-driven Rust, JS parity, and installed-smoke matrices mutate both declarations and every current projection input, asserting no driver call and no committed run authority. |
| No secrets are included in canonical configuration material or ordinary diagnostics. | Projection allowlists names/enums/hashes only; key/private/raw-body sentinels are scanned across projection JSON, bundle/audit/receipt, stdout, stderr, and MCP output. |

## Definition of done

- The implementation is limited to the listed runtime, schema, verifier,
  native-driver parity, smoke, and documentation surfaces.
- Native run requests cannot elevate arbitrary hash-shaped declarations; valid
  runs contain recomputable configuration/parameter observations and distinct
  provider-body evidence.
- The independent verifier rejects every altered hash/projection case and
  keeps deterministic/external legacy behavior stable.
- Focused Rust/JS tests, universal parity, installed native preflight,
  `make validate`, code review, and security review pass without a provider or
  credential.
- MDP-239 remains the parent execution gate; no issue status, labels,
  delegation, relation, or downstream implementation is changed by the plan.
