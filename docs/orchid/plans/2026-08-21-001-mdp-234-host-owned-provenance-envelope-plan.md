---
title: "feat: Host-wrap deterministic provenance fields instead of requiring model echo"
type: feat
date: 2026-08-21
execution: code
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-234
linear_issues:
  - MDP-234
  - MDP-239
repository: orchidautomation/message-decision-packs
base_branch: main
base_commit: 2cba9919483b5a7ba46efed53e3b5502b2abf477
source_branch: codex/mdp-234-plan
---

# MDP-234: host-owned provenance envelope for governed model output

## Goal capsule

| Field | Decision |
| --- | --- |
| Objective | Let the model return only semantic governed output while MDP constructs the immutable prompt/job/input provenance envelope from the observed native run. |
| Authority | Rust owns prompt resolution, staged input authority, invocation-receipt bytes, routed-context bytes, host wrapping, final validation, and receipt publication. The model can select semantic authority and draft fields but cannot author control-plane identity. |
| Envelope contract | Add an opt-in, closed `mdp.governed-host-envelope.v1` declaration to the pack-owned `PromptOutputContract`. Keep the final artifact contract `mdp.prompt-output.v0` and its receipt/validation authority stable. |
| Host-owned fields | For the migrated governed prompts, MDP supplies `contract`, `prompt_id`, `job_id`, `prompt_version`, `prompt_sha256`, `context_sha256`, `invocation_receipt_sha256`, and `source_summary.inputs_used` from exact observed run authority. A prompt may opt in only when every required source is available; the host-owned set is a fixed MDP allowlist, not arbitrary pack-authored authority. |
| Semantic fields | The model supplies `selected_authority`, `artifact`, `gaps`, and `rejected_claims`. The semantic schema has no host-owned properties and remains closed to unknown fields. |
| Failure rule | A model response containing an owned field, malformed semantic JSON, an unknown semantic field, or a semantic reference outside declared authority fails closed before final artifact publication. MDP never silently accepts a model-supplied identity. |
| Compatibility | Existing prompts without the envelope declaration continue using the v0 echo-and-validate path. Migrated prompt versions use the new schema projection and host wrapper; the final governed output and downstream receipt contracts remain v0-compatible after wrapping. |
| Sequencing | MDP-231 is the direct blocker: observed driver/model identity work must land first so the host envelope is attached to the strongest available run authority. MDP-239 remains the parent Phase 2 execution gate. |
| Stop condition | A valid semantic response is wrapped with exact prompt/context/receipt/input identities, all existing authority/reference/claim checks still run on the final object, attempted envelope injection and altered receipts are rejected, mechanical output burden decreases, and no secret/raw input/provider body enters model output authority or diagnostics. |

## Repository routing and handoff

- Repository: `orchidautomation/message-decision-packs`
- Base branch: `main` at `2cba9919483b5a7ba46efed53e3b5502b2abf477` (`origin/main`, v0.1.73).
- Planning branch: `codex/mdp-234-plan`.
- Isolated planning checkout: `/private/tmp/mdp-234-plan-work`.
- The canonical checkout has unrelated dirty files; it is out of scope and must remain untouched.
- This document is a plan-only artifact. It does not implement runtime behavior, change issue state, edit labels/delegation/relations, open a PR, or add automation branding.

## Problem frame and current evidence

The current native path already has the exact values needed for the final envelope,
but asks the model to reproduce them:

- `cli/src/run_runtime.rs::execute_generative_step` writes a private
  `mdp.prompt-invocation.v1` file and passes its bytes and detached SHA-256 through
  `native_visible_input`; the model is instructed to echo the receipt hash.
- `cli/src/run_runtime.rs::provider_schema_source` projects the provider schema
  from `PromptOutputContract::required_top_level`, so all deterministic envelope
  fields are currently required at the provider boundary.
- `cli/src/commands/prompt_output.rs::validate_governed_invocation_receipt`
  checks `invocation_receipt_sha256` against the exact receipt bytes, while
  `validate_governed_artifact_authority` checks `prompt_sha256`,
  `context_sha256`, selected authority, input inventory, and routed-context
  binding after the model response has arrived.
- `cli/src/run_runtime.rs::validate_native_request_size_before_bundle` duplicates
  the provider-schema/request construction used by `execute_generative_step`.
  Both paths must agree on the semantic-only provider schema after this change.
- `cli/src/commands/schemas.rs` exports the governed prompt/output schema and tests
  that the starter contract requires the current identity fields. Those tests are
  the public contract seam, not implementation detail.
- Shipped governed prompt assets in
  `plugin/assets/templates/basic/.mdp/prompts/{generate-outbound-copy,review-outbound-copy}.yaml`,
  `plugin/assets/templates/proposal/.mdp/prompts/{review-bid-no-bid,review-proposal-compliance,review-proposal-proof,review-proposal-red-team}.yaml`,
  and the route-budget copies explicitly instruct the model to echo the fields.

The current validator is fail-closed when a model echoes a wrong value, so this
issue is not permission to weaken validation. It moves deterministic construction
to the host, removes mechanical fields from the model-required schema, rejects
model attempts to provide those fields, and runs the same final validator over the
host-completed object.

## Host-envelope contract

### Pack declaration and migration shape

Extend `PromptOutputContract` with an optional closed declaration, represented by
the exact Rust/YAML shape chosen by implementation (the following is the planned
wire shape):

```yaml
output_contract:
  contract: mdp.prompt-output.v0
  output_kind: governed-artifact
  host_envelope:
    contract: mdp.governed-host-envelope.v1
    owned_top_level:
      - contract
      - prompt_id
      - job_id
      - prompt_version
      - prompt_sha256
      - context_sha256
      - invocation_receipt_sha256
      - source_summary
    semantic_required_top_level:
      - selected_authority
      - artifact
      - gaps
      - rejected_claims
```

`required_top_level` remains the final envelope requirement and remains closed.
The new declaration is valid only for `governed-artifact`, uses the fixed MDP
allowlist above, requires the semantic list to be disjoint from the owned list,
and rejects missing/duplicate/unknown fields. It is not a general mechanism for a
pack to mark `artifact` or authority references as host-owned. `source_summary`
is host-owned as a whole in this v1 so the model cannot smuggle a second inventory
or metadata field beside `inputs_used`.

The migrated prompt version must declare the envelope and update its canonical
prompt version/hash together. The final artifact still serializes as
`mdp.prompt-output.v0`; the host-envelope contract identifies how its fields were
constructed. Unmigrated prompts have no `host_envelope` block and retain the
existing model-echo path until a coordinated prompt/version migration.

### Exact host sources

| Final field | Host source and invariant |
| --- | --- |
| `contract` | MDP constant `PROMPT_OUTPUT_CONTRACT`; never read from model output. |
| `prompt_id`, `prompt_version`, `prompt_sha256` | Selected `CompiledModelStepV1` and the exact canonical prompt under `.mdp/prompts`; the hash is the same canonical prompt hash checked by existing validation. |
| `job_id` | The single canonical manifest job bound to the selected prompt/model step; ambiguous or missing binding blocks wrapping. |
| `context_sha256` | The exact staged `routed_context` authority SHA-256, after the existing canonical routed-context and job binding checks. A host-envelope prompt that owns this field must declare the routed-context input. |
| `invocation_receipt_sha256` | SHA-256 of the exact private `mdp.prompt-invocation.v1` bytes written by the runtime, not a model-calculated or receipt-self-reported value. |
| `source_summary.inputs_used` | The invocation receipt's declared input names plus `prompt_receipt` and `invocation_receipt_sha256`, in one deterministic order. Names are logical declared names only; no source paths, snippets, bodies, or secrets are copied. |

The wrapper must use the staged `StagedInput`/`ArtifactAuthority` values and
private invocation authority already held by the runtime. It must not reopen a
caller path, hash model text as context, or infer input names from arbitrary model
metadata. It may construct the final JSON only after the semantic payload is
parsed and checked against the projected semantic schema.

### Model boundary and final validation

For an opted-in prompt, the provider-facing schema is derived from the canonical
full schema by removing `owned_top_level` and making only
`semantic_required_top_level` required. `additionalProperties: false` remains
true at every projected object boundary. The model therefore receives no schema
property for the deterministic envelope.

After the driver returns, Rust must:

1. parse bounded UTF-8 JSON and require an object;
2. explicitly detect any owned top-level field (including `source_summary`) and
   return a stable injection diagnostic before mutation;
3. validate the remaining object against the semantic schema, preserving
   malformed/unknown/missing-field diagnostics without exposing raw model text;
4. inject the host-owned fields from the table above using one deterministic
   constructor; and
5. pass the completed object through the existing
   `validate_prompt_output_file_with_lineage_inputs` path, including
   `validate_governed_invocation_receipt`, routed-context compilation, selected
   authority, evidence/reference kind, ready/gap, and substantive generation
   checks.

The final output bytes written to `artifacts/output.json` are the host-wrapped
object using the repository's existing stable JSON serialization convention.
The validation artifact and run receipt hash these final bytes. The model's raw
semantic bytes may be retained only in the private transaction long enough for
validation and are never published as governed output or ordinary diagnostics.

## Planned implementation surfaces

| File | Existing symbols / responsibility | Planned change |
| --- | --- | --- |
| `cli/src/models.rs` | `PromptOutputContract`, `PromptFile` | Add an optional, serde-defaulted `PromptHostEnvelope`/equivalent closed value with contract, owned fields, and semantic required fields. Preserve serialization of legacy prompts when the block is absent; reject malformed envelope declarations during prompt/schema validation. |
| `cli/src/constants.rs` | Prompt/validation contract constants | Add the single `GOVERNED_HOST_ENVELOPE_CONTRACT` constant (`mdp.governed-host-envelope.v1`) and reuse it in runtime/schema/tests rather than duplicating string literals. |
| `cli/src/model_steps.rs` | `CompiledModelStepV1`, `compile_step`, `compiled_model_step_schema` | Carry the parsed host-envelope declaration through the compiled step/output contract hash. Add resolution tests proving the prompt/version/hash changes when migration metadata changes and that invalid ownership cannot compile. |
| `cli/src/run_runtime.rs` | `provider_schema_source`, `required_output_example`, `project_schema_node`, `validate_native_request_size_before_bundle`, `execute_generative_step`, `native_visible_input` | Add one semantic-schema projection helper and one host-wrap helper. Use the same projected schema in pre-bundle size checks and actual driver request construction. Parse/check model output, reject owned-field injection, derive envelope values from staged authority/invocation receipt/routed context, serialize final output, then invoke the existing final validator. Do not duplicate a second validator or allow model values to seed the envelope. |
| `cli/src/commands/prompt_output.rs` | `validate_prompt_output_parsed`, governed-artifact branch, `validate_governed_invocation_receipt`, `validate_governed_artifact_authority`, governed-output tests | Add semantic-payload validation and host-envelope declaration checks; retain all final-output checks. Add stable issue codes for missing envelope metadata, malformed semantic payload, unknown semantic field, host-owned field present, host source mismatch, and altered receipt/context. Ensure final `inputs_used` is checked against the exact invocation receipt inventory. |
| `cli/src/commands/schemas.rs` | Prompt-output schema around `prompt_output_schema`, `governed_artifact_example_schema`, schema tests around the governed required-field enum | Export the host-envelope declaration, semantic projection, and final governed schema invariants. Keep `required_top_level` as the final contract; prove host-owned fields are not required by the projected model schema while still required in the final envelope. Reject unknown ownership, overlap, duplicate, or missing semantic fields. |
| `cli/src/run_contracts.rs` | `DriverRequestV2`, `DriverResultV2`, `RunReceiptV1` | No new run receipt contract is required. If the implementation needs an internal typed wrapper authority, keep it private/closed and bind it through the existing output/validation authority; do not create a parallel receipt or assurance vocabulary. Add only additive serde fields if a receipt must identify the host-envelope contract. |
| `plugin/assets/templates/basic/.mdp/prompts/generate-outbound-copy.yaml` and `review-outbound-copy.yaml` | Current governed GTM prompt instructions/schema/example | Migrate to the host-envelope declaration, remove identity/source-inventory fields from model instructions and semantic required schema, retain final example shape through the wrapper, and bump prompt version/hash. |
| `plugin/assets/templates/proposal/.mdp/prompts/review-bid-no-bid.yaml`, `review-proposal-compliance.yaml`, `review-proposal-proof.yaml`, `review-proposal-red-team.yaml` | Current governed proposal prompts | Apply the same migration and remove echo language while preserving proposal-specific semantic decisions, selected claims/evidence, gaps, and refusal boundaries. |
| `examples/route-budget/ready/.mdp/prompts/{generate-outbound-copy,review-outbound-copy}.yaml` and `examples/route-budget/overflow/.mdp/prompts/{generate-outbound-copy,review-outbound-copy}.yaml` | Public synthetic copies of governed GTM prompts | Keep examples hash/version-aligned with the canonical templates and add one old-v2/legacy fixture to prove migration compatibility. |
| `scripts/test-universal-native-parity.mjs` | `providerSchemaForStep`, synthetic provider example, native run matrix and receipt verification | Generate semantic model payloads without host fields, assert the provider schema excludes them, exercise host wrapping through the real CLI path, independently recompute injected values, and keep CLI/driver/receipt parity. Replace placeholder `b`/`c` provenance values with host-derived expectations where MDP-231 provides them. |
| `scripts/test-cold-model-conformance.mjs` | Synthetic governed output/receipt fixtures around the cold-model candidate chain | Update the synthetic generator to distinguish semantic model payload from final host envelope; ensure conformance consumes only final validated output and altered host fields/receipts fail. Keep all fixtures synthetic and key-free. |
| `scripts/test-native-model-driver.mjs` | Provider schema/body projection and output-schema mutation tests | Add a semantic-schema case showing that host-owned properties are absent from provider schema while final validation still requires them. Preserve provider body/hash and no-key behavior. |
| `scripts/test-run-mcp-server.mjs` and `scripts/mdp-run-mcp-server.mjs` | CLI result transport/parity checks | No second wrapping implementation. Add a byte/parity assertion that MCP returns the CLI's final host-wrapped authority unchanged and never exposes the private semantic payload. |
| `scripts/release-install-smoke.sh`, `scripts/test-release-install-smoke.sh` | Installed CLI/plugin synthetic run checks | Add an installed no-provider/fake-driver semantic-output case and injection/altered-receipt assertions without credentials or network; verify the installed binary's final envelope and sanitized no-draft result. |
| `docs/job-prompt-contracts.md` | Public prompt/model boundary and governed-output guidance | Explain semantic payload versus host envelope, exact host sources, migration/version behavior, and that model output cannot author provenance. Update the current “model echoes” wording. |
| `docs/minimal-context-routing.md` and `docs/run-receipts.md` | Context digest and receipt authority guidance | State that MDP injects and verifies context/receipt hashes from exact bytes; preserve the distinction between integrity and semantic/reference validation. |
| `plugin/skills/mdp/references/mental-model.md`, `plugin/skills/mdp/references/cli-operator.md`, `plugin/skills/mdp-gtm-brief/references/{outbound-copy-brief,outbound-copy-review}.md`, and `plugin/skills/mdp-pack-review/references/structural-audit.md` | Agent-facing governed generation/review workflows | Replace instructions to echo deterministic identities with “return semantic fields; host wraps and validates” guidance. Keep the CLI validation command and no-draft boundary explicit. |

Do not change historical plans, add a sender/CRM action, alter the provider
endpoint, or introduce a second MCP/host envelope implementation.

## Ordered implementation steps

### 1. Freeze the compatibility and ownership contract

- Add the envelope contract constant and closed model in `models.rs`.
- Define the fixed owned-field and semantic-field allowlists, disjointness,
  required routed-context condition, deterministic input ordering, and stable
  issue-code vocabulary before changing templates.
- Keep the final `mdp.prompt-output.v0` shape and existing receipt authority. A
  migrated prompt is versioned; an unmigrated prompt remains on the echo path.
- Add schema characterization tests for legacy, migrated-valid, duplicate,
  overlapping, unknown, and semantically incomplete declarations.

### 2. Split the canonical full schema from the semantic provider schema

- Refactor `provider_schema_source` and its callers so the canonical full schema
  remains the final validator schema while a host-envelope prompt produces a
  semantic schema with owned properties removed from `properties` and
  `required`.
- Apply the same split in `validate_native_request_size_before_bundle` and
  `execute_generative_step`; do not let preflight and execution construct
  different provider schemas or hashes.
- Preserve `additionalProperties: false`, nested object constraints, required
  semantic fields, and provider-schema projection safety.
- Keep legacy prompts byte-for-byte on the old projection path.

### 3. Implement one fail-closed host wrapper

- Add a helper near the governed output code (or a narrowly scoped runtime
  adapter) that receives the selected compiled step, staged input authorities,
  private invocation authority, routed context, and bounded driver bytes.
- Parse strict JSON; detect owned fields before any overwrite; validate semantic
  shape; construct the envelope from exact host values; serialize with the
  existing stable writer.
- Reject missing/ambiguous job binding, missing required routed context, stale
  receipt/prompt linkage, malformed semantic JSON, and host-field injection with
  sanitized stable issue codes. Never include raw model text, paths, keys, or
  provider error text in those codes.
- Run final `validate_prompt_output_file_with_lineage_inputs` on the wrapped
  bytes. Existing authority/reference/evidence/claim checks remain the last
  word.

### 4. Migrate shipped prompt assets and guidance

- Add the envelope declaration and increment versions for the two basic prompts,
  four proposal review prompts, and route-budget copies.
- Remove echo instructions/checklist entries for host-owned fields. Keep semantic
  selection, gap/refusal, evidence, and claim rules unchanged.
- Update the prompt examples and public docs/skills together so agents are not
  told to manufacture hashes or inventories.
- Keep a v2 legacy fixture with the old schema to prove the explicit fallback.

### 5. Add the adversarial and efficiency fixture matrix

At minimum, cover:

- valid semantic payload → exact valid host-wrapped output;
- malformed JSON and markdown-wrapped semantic output;
- missing semantic field, unknown semantic field, wrong artifact type, and
  semantic identifier outside selected authority;
- model supplies every owned top-level field, including a forged
  `source_summary`, and the wrapper fails before final publication;
- altered prompt bytes/hash, routed-context bytes/hash, invocation receipt bytes,
  and invocation input hash; final authority fails closed;
- a valid wrapped output whose model payload is byte-identical across runs while
  host receipt/context identity changes, proving identities come from the host;
- legacy prompt/output still validates through the existing echo path;
- provider schema required-field/property count and synthetic JSON byte counts
  decrease for the model payload while final wrapped validation and claim checks
  remain unchanged;
- output, validation, receipt, CLI stdout, and MCP response contain no key,
  private path, raw input/body, or provider-error sentinel.

Every injection/altered-authority case must have no successful governed output;
where the native run has already staged a bundle, its terminal state remains the
existing no-draft invalid/policy outcome and the receipt verifier cannot upgrade
it.

### 6. Prove CLI/MCP/installed parity and finish validation

- Run focused Rust schema/runtime/prompt-output tests, then native-driver and
  universal parity scripts, then installed smoke.
- Run `make validate` from a clean task branch and inspect the generated final
  envelope, validation artifact, receipt, and MCP result for exact hashes and
  redaction.
- Run code review/security review before any implementation PR. This plan branch
  must not claim the unimplemented wrapper has passed those gates.

## Validation contract

The implementation PR must run these commands without a provider call or
credential; generated files stay in temporary directories outside the checkout:

| Command | Proof |
| --- | --- |
| `cargo fmt --manifest-path cli/Cargo.toml -- --check` | Rust formatting. |
| `cargo test --manifest-path cli/Cargo.toml prompt_output` | Semantic-vs-final schema, host-source, injection, receipt/context mutation, legacy fallback, and authority-reference checks. |
| `cargo test --manifest-path cli/Cargo.toml run_runtime` | One semantic provider schema in preflight/execution, valid wrapping, malformed/injection refusal, no final output on failure, and output-size behavior. |
| `cargo test --manifest-path cli/Cargo.toml schemas` | Closed envelope declaration/projection/final schema and migration characterization. |
| `node --check scripts/mdp-native-model-openai.mjs` | Native driver syntax remains valid. |
| `node scripts/test-native-model-driver.mjs` | Provider schema/body/hash and host-field exclusion behavior. |
| `node scripts/test-universal-native-parity.mjs` | GTM/proposal semantic payload, host wrapping, run receipt, verifier, and CLI parity across all model steps. |
| `node scripts/test-cold-model-conformance.mjs` | Final host-wrapped governed output remains the only conformance authority. |
| `scripts/test-release-install-smoke.sh` | Installed prompt/schema, no-provider, injection, and altered-receipt smoke. |
| `make validate` | Full Rust, schema, authority, MCP, template, skill, packaging, installer, and public-artifact gates. |
| `git diff --check` | Plan/code whitespace hygiene. |

Plan-only validation is limited to Markdown/front matter, diff hygiene, and the
unchanged baseline. It must not claim host wrapping, token reduction, or
implementation tests passed before implementation exists.

### Plan-branch validation record

- `git diff --check` passed.
- `make validate` was run from this clean plan checkout. All gates through
  `validate-public-artifacts` passed, including the 623 Rust unit tests and
  authority, conformance, template, skill, asset, version, native, parity,
  proposal, MCP, and public-artifact checks.
- The run stopped at `validate-pluxx-hooks`: the repository requires Pluxx
  `0.1.40`, the checkout only has `0.1.32`, and the `npx` fallback could not
  resolve `registry.npmjs.org` (`ENOTFOUND`). A local `0.1.32` retry reached
  the hook fixture but failed its expected newer plugin-root behavior. This is
  an environment/dependency blocker, not a plan-artifact failure.
- No host wrapping, token reduction, or implementation behavior is claimed as
  tested by this plan-only commit.

## Dependencies and sequencing

| Dependency | Relationship | Execution rule |
| --- | --- | --- |
| MDP-231 | Direct blocker. Observed driver/model identities are part of the strongest native run authority that this envelope must preserve. | Implement MDP-234 only against the landed observed-identity seams; do not duplicate its projection or verifier work. |
| MDP-179 | Shipped run-bundle/receipt assurance vocabulary. | Preserve declared/observed/verified distinctions and existing v1 receipt hashes; host wrapping is not a signer or isolation claim. |
| MDP-197 | Shipped job-owned prompt/output contracts. | Extend the existing `PromptOutputContract`; do not create a parallel prompt engine or host protocol. |
| MDP-226 / MDP-230 | Neighboring routed-context and governed-lineage work in the MDP-239 Phase 0 queue. | Consume their canonical artifacts once available; do not reintroduce top-level readiness fields or rebind real evidence here. |
| MDP-237 | Downstream request compiler blocked by MDP-231/226/230. | Ensure the new semantic/full schema split is discoverable by the future compiler, but do not implement compiler behavior in this issue. |
| MDP-239 | Parent execution index and Phase 2 gate. | Keep MDP-234 `Backlog`/`phase:planned`; do not restore ready-for-agent or delegate the parent batch. |

The implementation order is: envelope schema and migration contract → semantic
schema projection → runtime wrapping/final validation → prompt/assets/docs →
adversarial/efficiency fixtures → CLI/MCP/installed parity → full validation and
review.

## Risks and mitigations

| Risk | Mitigation and failure contract |
| --- | --- |
| Semantic schema and final schema drift. | Keep one canonical prompt schema; derive only the provider projection, assert projected required fields are a strict subset, and validate the wrapped final object with the unchanged full schema. |
| The wrapper silently overwrites a model-forged identity. | Detect every owned property before injection and return a stable injection failure. Never “repair” a supplied hash by replacement. |
| A pack marks semantic authority as host-owned. | Accept only the fixed MDP v1 ownership allowlist; reject unknown/overlapping ownership and keep selected authority/artifact semantic. |
| Context or receipt changes between model call and wrapping. | Use immutable staged/private bytes, verify source stability at existing post-checks, inject observed staged hashes, and let final receipt/context validation fail closed on any mismatch. |
| Prompt version/hash migration breaks old hosts. | Keep old prompts on the legacy path; migrate prompt versions atomically with the new binary/templates; document that new host-envelope metadata requires the coordinated runtime. |
| Input inventory ordering differs across hosts. | Define one deterministic order from the prompt declaration/invocation receipt and assert exact replay; never use an unordered model list as authority. |
| Token savings are claimed without measuring the real provider request. | Compare projected schema and serialized semantic payload/body on the same synthetic cases; report byte/token proxy and mechanical failure counts, not provider billing claims. |
| Existing model/reference validation is accidentally skipped. | Wrap first, then call the existing governed validator and receipt verifier; adversarial selected-authority/evidence/claim fixtures must remain failing. |
| Raw model/provider data leaks through diagnostics or published artifacts. | Keep semantic bytes private, allowlist all diagnostic values, scan final outputs/stdout/stderr/MCP for synthetic secrets/paths/bodies, and never serialize provider errors. |
| Scope expands into MDP-237/compiler or provider changes. | Keep the implementation surface table and dependency graph as review boundaries; expose only the metadata/schema seam needed by the future compiler. |

## Compatibility and rollback

- `mdp.prompt-output.v0`, `mdp.prompt-output-validation.v1`,
  `mdp.run-request.v1`, `mdp.run-bundle.v1`, `mdp.driver-request.v2`,
  `mdp.run-receipt.v1`, and existing terminal states remain unchanged.
- Prompts without `host_envelope` continue to require the full model echo and
  pass through current validation. There is no silent reinterpretation of a
  legacy prompt.
- Migrated prompts increment their prompt version and canonical hash. A new
  runtime recognizes `mdp.governed-host-envelope.v1`; an older runtime may read
  the final v0 shape but is not promised to validate the migrated prompt hash or
  execute its semantic-only provider schema. Operators must upgrade the runtime
  and pack together, as documented.
- Final wrapped output contains the same deterministic fields and semantic
  fields as the old governed artifact, so downstream validation/trace/receipt
  consumers do not need a second output contract. The provenance is stronger
  because the values are observed and injected by MDP, not trusted from model
  text.
- Rollback is a single implementation PR/commit revert plus reverting the
  coordinated migrated prompt assets to their previous versions. No pack data,
  customer record, provider call, deployment, or destructive cleanup is part of
  this change. Existing legacy prompts remain usable during rollback.
- If a partially migrated pack is encountered, readiness fails closed on the
  unsupported envelope/version rather than falling back to an unvalidated
  semantic payload. Do not strip fields or edit generated receipts in place.

## Acceptance-criteria mapping

| MDP-234 acceptance criterion | Planned proof |
| --- | --- |
| Host-owned fields are absent from the model-required output schema. | `host_envelope` splits the canonical final schema from the provider semantic projection; schema tests and universal parity assert no owned properties or required entries reach the model. |
| MDP creates the final governed envelope from observed run authority. | `host_wrap_governed_output` derives prompt/job/context/receipt/input inventory from compiled step, staged authorities, and private invocation bytes, then final validation/receipt hashes the wrapped bytes. |
| Final artifact preserves or strengthens MDP-179 assurance and receipt verification. | Final output remains `mdp.prompt-output.v0`; existing validation and run receipt bindings execute after wrapping, while MDP-231 observed run identities remain untouched and independent. |
| The model cannot override injected identities. | Owned-field presence/injection fixtures fail before mutation/publication; altered receipt/context/prompt fixtures fail final validation and never gain governed authority. |
| Semantic references to authority IDs remain validated against declared context. | Existing `selected_authority`, kind, evidence, gap-reference, ready/gap, and substantive generation checks run unchanged against the wrapped final object; negative semantic fixtures remain blocked. |
| Existing prompt-output contracts have an explicit migration/version path. | Optional closed `mdp.governed-host-envelope.v1`, prompt version/hash bump for shipped migrations, and legacy no-envelope echo fixtures document and test both paths. |
| Token/output comparisons show reduced mechanical failure without weaker claim validation. | Same synthetic semantic cases compare provider schema/serialized payload size and mechanical failure counts before/after; authority/reference/claim validation and no-draft outcomes are asserted equal or stricter. |
| Fixtures cover malformed semantic output, attempted envelope injection, altered receipts, and valid host wrapping. | Focused Rust plus universal parity/cold-model/installed smoke matrix covers all four cases, with no final output authority for malformed/injected/altered inputs. |
| Skills and references explain the semantic-payload/host-envelope boundary concisely. | Prompt YAML, `docs/job-prompt-contracts.md`, routing/receipt docs, and affected MDP skills remove echo instructions and teach the host-owned boundary. |

## Definition of done

- The only new public prompt metadata is the closed, versioned host-envelope
  declaration; no parallel model/receipt authority exists.
- Migrated native model calls receive a semantic-only schema, and MDP publishes
  only a host-completed final governed artifact after all existing checks pass.
- Every forged/mutated deterministic field fails closed without exposing raw
  model/provider/input data, while legacy prompts remain readable and validated.
- Focused Rust/Node tests, universal CLI/MCP parity, installed smoke, `make
  validate`, code review, and security review pass on the implementation PR.
- MDP-231 and MDP-239 blocker/state/relations remain intact; this plan does not
  change status, labels, delegation, or add automation branding.
