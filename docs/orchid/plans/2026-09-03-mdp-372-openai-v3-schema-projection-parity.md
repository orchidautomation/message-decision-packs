# MDP-372: Align OpenAI v3 semantic schema projection with local validation

## 1. Context and current behavior

MDP-372 concerns the Rust CLI's profile-neutral semantic normalization v3
provider boundary. The issue is scoped to
`orchidautomation/message-decision-packs`, with `main` as the integration
branch. The current implementation is present at the inspected source commit
`1450a4d39421be1bb0a553aee7a746a8f3b9e8f5` on the issue branch.

The inspected code confirms three projection gaps:

1. `cli/src/commands/requirements.rs::normalized_envelope_schema` expresses
   conditional classification values with `allOf`/`if`/`then`/`else`. The
   OpenAI projection in `cli/src/run_runtime.rs::project_schema_node` flattens
   `allOf` and makes every remaining object property required. A classification
   that is locally valid without `value` for `ambiguous`, `no-match`, or
   `unsupported` therefore becomes provider-invalid, while `classified` does
   not get a provider-visible conditional requirement.
2. `cli/src/commands/v3_normalization.rs::v3_sealed_envelope_schema` leaves
   the sealed `gaps` and `rejected_claims` item schemas as empty generic
   objects. The OpenAI projector consequently sends closed empty item objects,
   losing the required `attribute`/`reason` and `claim`/`reason` contracts.
3. `host_wrap_v3_normalization_output` can return only the bounded
   `v3-semantic-output-invalid` code for a JSON Schema rejection. The runtime
   already has `V3Issue` paths, but published run/audit/authority/actionable
   diagnostics do not carry a bounded path/type projection. Raw provider
   output and raw validator error strings must remain excluded.

The generic provider projector is intentionally strict for ordinary schemas,
so the repair must make the canonical v3 semantic branches explicit rather
than weaken or bypass the shared projection. The v3 local validator remains
the semantic authority; the provider schema is a lossless provider-compatible
projection of that authority.

## 2. Objective, scope, out of scope, and assumptions

### Objective

Make every v3 semantic payload accepted by the provider-compatible schema
representable under the local v3 semantic validator, while preserving the
host-owned envelope boundary and exposing one bounded, safe first-rejection
diagnostic when validation fails.

### In scope

- Replace v3 classification conditionals with provider-compatible explicit
  `anyOf` branches: one branch for `classified` with required allowed `value`,
  and one branch for each non-classified status without a `value` property.
- Make the canonical v3 gap and rejected-claim item schemas explicit. Preserve
  optional gap metadata by representing its allowed field combinations as
  explicit provider-compatible branches.
- Update the standalone v3 provider-schema preflight helper and tests so its
  nested properties cannot regress to empty objects.
- Add bounded schema-rejection detail (`code`, JSON path, expected category,
  observed JSON type/category) to the run's additive diagnostic surfaces. The
  detail must be host-generated, length-bounded, and free of model values,
  evidence prose, provider response text, filesystem paths, and secrets.
- Preserve the detail through the generative outcome, runner audit, receipt,
  authority block, run execution envelope, command summary, and actionable
  diagnostics projection.
- Add Rust and Node parity/property fixtures for classified, all non-classified
  statuses, nonempty gaps, nonempty rejected claims, and invalid safe
  diagnostics.
- Update the single-source v3 semantic/raw-output boundary documentation.

### Out of scope

- Changes to the private MDP-for-MDP pack, prompts, source data, outreach, or
  any customer/private fixture.
- Raw model/provider output retention or raw JSON Schema error text.
- Weakening host ownership, taxonomy identity, evidence eligibility, or local
  semantic validation.
- A new envelope/primitive redesign, provider other than the existing OpenAI
  adapter, release publication, installation, deployment, or merge.
- Changes to v1/v2 schema semantics except additive optional diagnostic fields
  required to keep shared run contracts truthful.

### Assumptions and decisions

- OpenAI's strict subset accepts `anyOf` object branches, and the existing
  Rust/Node projectors already preserve `anyOf`; no provider-specific nullable
  workaround is needed.
- Job-scoped classification keys and taxonomy enum values remain fixed by
  compiled requirements. The generic standalone semantic helper may retain its
  dynamic classification map for preflight inspection, but all actual runtime
  requests continue to use the fixed job-scoped classification schema.
- New diagnostic detail is additive and optional so old receipts remain
  readable. The receipt hash naturally covers the field when present.

## 3. Acceptance mapping

| MDP-372 acceptance criterion | Implementation and proof |
| --- | --- |
| Provider `gaps.items` requires `attribute` and `reason`. | Explicit canonical gap schema, provider preflight assertions, Rust/Node projection parity fixtures. |
| Provider `rejected_claims.items` requires `claim` and `reason`. | Explicit canonical rejected-claim schema, preflight assertions, parity fixtures. |
| Classified status supports an allowed value; ambiguous/no-match/unsupported do not require or accept fabricated value. | Job-scoped `anyOf` classification branches plus local semantic validation and status matrix tests. |
| Provider-valid semantic payloads cannot fail local structural validation from projection loss. | Cross-product/property fixtures for statuses, gap metadata combinations, rejected claims, and projected-schema validation. |
| Synthetic nonempty gap seals with deterministic non-ready/invalid behavior. | Native/runtime fixture and v3 envelope validation tests. |
| Synthetic rejected claim passes structural validation and remains in host-sealed artifact. | Sealing/runtime fixture asserts exact bounded claim/reason fields. |
| Invalid semantic payload exposes stable safe code plus bounded path/expected type; raw output absent. | Host wrapper, receipt/audit/authority/actionable propagation tests with a sentinel payload value. |
| Existing MDP-297/298/361 and native driver/run/universal parity remain green. | Targeted and full validation commands listed in section 6. |
| Provider schema has no closed empty gap/rejected item and its hash changes. | Schema inspection/hash assertions; no private canary mutation or release install. |
| Documentation states one semantic schema and raw-output boundary. | `docs/prompt-extraction-contract.md` and related native-run documentation update. |

## 4. Affected files and symbols

- `cli/src/commands/v3_normalization.rs`
  - `v3_semantic_provider_schema`, classification/gap/rejected item schema
    constructors, `v3_sealed_envelope_schema`,
    `project_v3_semantic_provider_schema_for_openai`, and v3 schema tests.
  - Add a bounded conversion helper for first JSON Schema errors and safe
    `V3Issue` diagnostic categories without exposing raw values.
- `cli/src/commands/requirements.rs`
  - `normalized_envelope_schema` and its v3 schema/validation tests; compile
    fixed classification keys, taxonomy constants, and status branches.
- `cli/src/run_runtime.rs`
  - `RunExecution`, `RunFailure`, `GenerativeOutcome`, host v3 wrapping,
    `validate_v3_classification_evidence`, schema validation, transaction
    propagation, receipt/audit/authority construction, and projector tests.
- `cli/src/run_contracts.rs`
  - Add the shared optional bounded diagnostic-detail contract to runner audit
    and run receipt while preserving `deny_unknown_fields` and legacy defaults.
- `cli/src/commands/schemas.rs`
  - Add the same optional diagnostic-detail property to runner-audit,
    run-receipt, run-execution, and canonical-authority schemas.
- `cli/src/diagnostics.rs` and `cli/src/output.rs`
  - Carry the first safe detail into actionable diagnostics and the `run`
    summary without changing legacy low-level fields.
- `scripts/mdp-native-model-openai.mjs` and
  `scripts/test-native-model-driver.mjs`
  - Keep the generic projector algorithm unchanged; add explicit nested v3
    anyOf/required-field parity assertions and native request fixtures.
- `scripts/test-universal-native-parity.mjs` and relevant Rust JSON contract
  tests
  - Assert Rust/Node provider projection identity and additive diagnostics.
- `docs/prompt-extraction-contract.md` and, if needed,
  `docs/native-api-normalization-runner.md`
  - Document the canonical v3 semantic language, provider projection rule,
    and no-raw-output diagnostic boundary.

No generated host bundle, private pack, or release artifact is owned by this
change.

## 5. Ordered implementation steps

1. **Make the canonical semantic schema lossless under strict projection.**
   Build reusable common classification properties and explicit `anyOf`
   branches. A classified branch will require `status`, `value`, taxonomy
   identity, nonempty `derived_from`, and bounded `basis`; each non-classified
   branch will require the common fields and omit `value`. Keep the local
   typed payload and validator as the authority.

2. **Close v3 gap/rejected item schemas.** Replace empty sealed-envelope item
   schemas with the same explicit semantic item contracts. Encode allowed
   optional gap metadata combinations as branches so strict projection does
   not silently turn optional properties into an incompatible empty object or
   invent private values.

3. **Repair the standalone preflight projection.** Make
   `project_v3_semantic_provider_schema_for_openai` derive/reflect the explicit
   nested schemas and assert that no item schema is `{}` or an object with an
   empty property set. Keep the fixed three top-level semantic fields required.

4. **Repair job-scoped classification compilation.** In
   `normalized_envelope_schema`, use the compiled attribute's exact enum and
   taxonomy constants in the same explicit status branches. Ensure each fixed
   classification key remains required, while the branch selected by status
   determines whether `value` exists.

5. **Add safe first-rejection extraction.** Use the JSON Schema validator's
   first error instance path and keyword, but map its instance to a bounded
   JSON type/category and map all schema details to allowlisted stable labels.
   Cap path and label lengths, escape/sanitize JSON Pointer segments, and never
   serialize `ValidationError::to_string`, `V3Issue.observed`, model values,
   evidence prose, or provider response bodies.

6. **Propagate diagnostic detail through the run boundary.** Add an optional
   shared detail object to the in-memory failure/outcome and additive wire
   surfaces. Host v3 schema, semantic, and sealed-envelope failures will attach
   the first safe detail; transport/driver paths continue to expose only their
   existing stable scalar code. Ensure timeout/audit-incomplete overrides do
   not retain stale validation detail.

7. **Project actionable diagnostics and summaries.** Collect detail from the
   top-level run execution and authority block, attach it to the corresponding
   actionable diagnostic, and include it in the run summary where the scalar
   code is already projected. Preserve the existing fallback and all raw-output
   suppression behavior.

8. **Add regression and parity tests.** Cover every classification status,
   nonempty gap, nonempty rejected claim, optional gap metadata combinations,
   invalid schema path/code/type, receipt/audit/authority/actionable
   propagation, sentinel suppression, and Rust/Node projected-schema hashes.
   Keep generic non-v3 conditional projector tests unchanged to prove the
   generic projector has not been weakened.

9. **Update documentation and inspect the final diff.** State that the v3
   semantic schema is canonical, the OpenAI schema is a strict projection,
   the host seals envelope fields, and diagnostics retain only bounded safe
   location/category metadata.

## 6. Tests and validation

### Focused checks during implementation

- `cargo fmt --manifest-path cli/Cargo.toml -- --check`
- `cargo test --manifest-path cli/Cargo.toml v3_semantic -- --nocapture`
- Focused requirements/schema and run-runtime tests by name as added.
- `node scripts/test-native-model-driver.mjs`
- `node scripts/test-universal-native-parity.mjs`

### Required issue validation

```text
cargo test --manifest-path cli/Cargo.toml v3_semantic -- --nocapture
cargo test --manifest-path cli/Cargo.toml run_runtime -- --nocapture
node scripts/test-native-model-driver.mjs
node scripts/test-universal-native-parity.mjs
node scripts/test-run-v1-golden.mjs
cargo test --manifest-path cli/Cargo.toml
```

Also run `cargo fmt --manifest-path cli/Cargo.toml -- --check` and any
repository-native conformance script required by the changed test surface.
Record failures caused only by an unavailable local binary/environment
separately; do not weaken tests or claim them as green.

### Manual proof

Inspect the emitted provider schema and verify:

- `gaps.items` is not an empty object and requires `attribute`/`reason`;
- `rejected_claims.items` is not an empty object and requires `claim`/`reason`;
- each classification branch has exact status/taxonomy/value behavior;
- an invalid synthetic payload's serialized receipt/audit/actionable result
  contains only the bounded diagnostic fields and no sentinel/raw content.

No real provider call, private canary rerun, release installation, or external
system mutation is authorized by this plan.

## 7. Compatibility and migration behavior

- The canonical v3 semantic contract remains `classifications`, `gaps`, and
  `rejected_claims`; only its provider-compatible JSON Schema representation is
  corrected. Host-owned fields remain host-generated and rejected from provider
  output.
- The classification schema changes from a lossy flattened conditional to
  explicit branches. Existing valid classified and non-classified payloads
  remain valid; payloads that fabricate a non-classified value or omit a
  classified value remain invalid.
- Gap/rejected item contracts become stricter and truthful. This is an
  intentional fail-closed correction, not a migration of stored private data.
- Diagnostic detail is optional and additive on v1 run surfaces. Older
  receipts without it remain readable; new receipts include it in the
  self-hash. Consumers that ignore unknown additive top-level command fields
  retain behavior.
- Provider schema/request hashes change by design. Existing requests are not
  rewritten; a new run binds its exact canonical/provider schema hashes.
- v1/v2 normalization and generic non-v3 provider projection behavior remain
  unchanged apart from shared optional run diagnostic schema declarations.

## 8. Risks, safety boundaries, rollout, observability, and rollback

### Risks and mitigations

- **OpenAI branch support or schema limits:** use only existing projector
  primitives (`anyOf`, object properties, enum/const, bounded arrays) and
  Rust/Node parity tests; fail preflight closed if projection cannot be built.
- **Over-constraining optional gap metadata:** test all four allowed metadata
  combinations and ensure local serde/validator behavior matches each branch.
- **Diagnostic leakage:** use keyword/path allowlists and JSON type/category
  observations only; test with unique private sentinels and assert they do
  not occur in any published/public projection.
- **Receipt contract drift:** validate every emitted run/audit/authority
  object against the updated schemas and run verification/hash tests.
- **Schema hash churn:** expected for this bug fix; report exact changed hash
  behavior without publishing a release.

### Safety and rollout

The change remains local/offline by default. It does not authorize provider
calls beyond existing mock seams, does not touch private source material, and
does not weaken fail-closed validation. Rollout is one feature branch and one
PR to `main`; release CI owns packaging after merge.

### Observability and rollback

The provider schema hash, request schema hash, terminal state, stable scalar
diagnostic code, and bounded detail are the allowed observations. Raw model
output, provider response bodies, evidence prose, and raw validator messages
remain absent. Rollback is a normal revert of the single PR; no data migration
or release rollback is required.

## 9. Blockers and readiness verdict

The repository, branch, issue scope, relevant implementation, and validation
commands are identified. The private dependent pack remains read-only and is
not needed for implementation or proof. No blocker prevents a bounded
single-repository implementation.

**Verdict: READY_TO_PIN.**

