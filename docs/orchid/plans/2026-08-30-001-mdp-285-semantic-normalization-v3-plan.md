# MDP-285 — Semantic Normalization v3 implementation plan

Status: `READY_TO_PIN`

## 1. Context and current behavior

The approved contract is recorded in
`docs/orchid/decisions/2026-08-30-semantic-normalization-v3-contract.md`.
Repository inspection was performed on `origin/main` at
`ecb9ad017942162ff62b1ca9fc1e7bd3416ab86c`; this plan is grounded in that
source rather than prior Linear completion claims.

Confirmed implementation seams:

- `cli/src/models.rs::DecisionInputContract` and
  `DecisionInputAttribute` model DICs but have no observed/classified
  processing distinction or taxonomy reference.
- `cli/src/commands/schemas.rs::decision_input_contracts_schema` requires the
  existing DIC shape and selects normalized v1/v2 according to signal
  projections.
- `cli/src/commands/requirements.rs::compile_contract`,
  `source_attempt_request_schema`, `collected_attempt_results_schema`, and
  `normalized_envelope_schema` compile the exact job-specific source and
  normalization contracts.
- `validate_normalized_decision_input_with_projection` currently enforces
  direct observed-value equality into `normalized_prospect`; it is the central
  place where v3 must validate derived lineage before deterministic ingress.
- `cli/src/starter.rs::starter_decision_input_normalization_prompt` produces
  v2, forbids inference, and asks the model to echo lineage and control fields.
- `cli/src/model_steps.rs::resolve_model_steps` already resolves one exact
  normalization authority and compiles its prompt/output contract.
- `cli/src/run_runtime.rs::project_output_schema_for_openai`,
  `provider_schema_source_for_contract`, and
  `host_wrap_governed_output` already implement provider schema projection and
  trusted host wrapping for generation.
- `cli/src/decision_input.rs::DecisionInput` already provides the target
  private neutral `fields`/`signals`/`attributes` representation, while
  `from_proposal_output` retains v0 alias compatibility.
- `cli/src/profile_conformance.rs` is the existing shared conformance gate and
  should be extended, not duplicated.

## 2. Objective, scope, out of scope, and assumptions

### Objective

Ship one additive semantic-normalization v3 path in which job-scoped pack
taxonomies convert bounded observed evidence into closed model classifications
with enforceable lineage, the host constructs the trusted neutral envelope,
and the existing deterministic engine remains the sole decision authority.

### In scope

- `mdp.normalized-decision-input.v3` and `normalized_input`.
- Explicit `observed` versus `model-classified` DIC processing.
- Canonical pack-level classification taxonomies and job-scoped compilation.
- Semantic-only provider output plus host wrapping for normalization.
- Deterministic validation/projection of classification lineage.
- GTM and proposal template/prompt migrations.
- Explicit v0-v2 compatibility and actionable migration diagnostics.
- Neutral, profile, adversarial, runtime, receipt, and installed-path proof.
- MDP-for-MDP downstream adoption after the released artifact exists.

### Out of scope

- Provider-specific evidence adapters or source retrieval.
- Cloud implementation or raw-evidence upload.
- A third production profile.
- A new runner unless downstream dogfood proves repeated orchestration friction.
- Automatic taxonomy invention or automatic semantic pack migration.
- Numeric model self-confidence as decision authority.
- Replacing deterministic fit/routing, governed generation, or receipts.

### Explicit assumptions

- Existing v1/v2 valid artifacts remain immutable and readable.
- New v3 packs explicitly opt into v3; omitted DIC processing remains
  `observed` only for legacy reads.
- The current native OpenAI structured-output driver is the pilot provider.
- Model-classified attributes cite observed contributor attempts; classifications
  may not depend on another model-classified attribute in v3.
- The main repository delivery is cohesive and versioned. The MDP-for-MDP
  adoption is a separate downstream repository change after release.

## 3. Acceptance mapping

| MDP-285 acceptance criterion | Implementation units | Validation |
| --- | --- | --- |
| Decisions are explicit and consistent | Decision document; U1 | Schema examples plus design review against all child briefs |
| Model-owned and host-owned fields are enumerated | U3 | Provider schema excludes every host field; injection fixtures fail |
| Decisions cannot be confused with normalization | U3, U4, U6 | v3 omits model outcome/draft fields; deterministic route tests |
| v0-v2 reads and v3 writes are specified | U1, U3, U5 | Legacy fixtures pass; mixed/alias v3 fixtures fail |
| Taxonomy authoring, compilation, hashing, selection are specified | U1, U2 | Pack/schema/compiler/hash tests |
| Evidence privacy and receipts are specified | U3, U6, U8 | Receipt fixture excludes evidence prose; tamper tests |
| Provider-schema preflight is exact | U3 | Unsupported projection fails before driver invocation |
| GTM, proposal, neutral third domain share one mechanism | U5, U6 | Three profile-shaped conformance suites with neutral core assertions |
| MDP-26 and MDP-59 are reconciled | U1, U5 | Decision text and migration diagnostics; no second opportunity core |
| MDP-154/187 receive bounded disposition | U1, U8 | Closeout links local-first privacy handoff; no cloud code |
| Complete plan pin passes readiness | This document | Lint, clean diff, commit, push, exact ref/commit/path readback |

## 4. Affected files and symbols

Exact ownership may narrow during implementation, but widening beyond these
surfaces requires a plan-conflict update.

### U1/U2 — authored and compiled contract

- `cli/src/constants.rs`
  - Add the v3 normalized contract identifier and taxonomy contract/hash
    identifiers if a separately named schema is required.
- `cli/src/models.rs`
  - Add `DecisionInputProcessing` and the model-classification reference on
    `DecisionInputAttribute`.
  - Add typed `ClassificationTaxonomy`, value definition, minimum-evidence,
    and closed-policy models on `Manifest`.
  - Generalize `PromptHostEnvelope` validation by output kind while retaining
    existing governed-artifact fixed-field behavior.
- `cli/src/commands/schemas.rs`
  - Extend manifest/DIC schemas and publish the generic v3 schema.
  - Preserve existing v1/v2 and legacy prompt-output schemas.
- `cli/src/commands/health.rs`
  - Validate taxonomy uniqueness, exact enum equality, contributor ownership,
    no cycles, source classes, limits, prompt bindings, and v3 explicitness.
- `cli/src/commands/requirements.rs`
  - Extend `compile_contract` with processing and selected taxonomy data.
  - Exclude model-classified attributes from source-attempt schemas.
  - Compile a job-scoped collection specification and classification
    specification plus canonical taxonomy-set hash.
  - Add the v3 envelope schema and validation/projection path.
- `cli/src/artifact_hash.rs`
  - Reuse canonical hashing for selected taxonomy and requirements identity;
    add no competing hash algorithm.

### U3 — runtime and authority boundary

- `cli/src/model_steps.rs`
  - Resolve v3 normalization prompts and carry the selected semantic/host
    envelope metadata without adding another model-step registry.
- `cli/src/run_runtime.rs`
  - Generalize provider-schema source selection and host wrapping for
    `decision-input-normalization` v3.
  - Keep OpenAI schema projection before driver request sealing.
  - Build the sealed v3 envelope from trusted staged inputs, compiled
    requirements/taxonomy identity, observed attempts, validated semantic
    classifications, and deterministic projections.
- `cli/src/commands/prompt_output.rs`
  - Validate sealed v3 output and route v1/v2 to their unchanged validators.
  - Reject direct semantic fragments, host-owned injection, unknown evidence
    references, and mixed v3/legacy payloads.
- `cli/src/decision_input.rs`
  - Add v3 ingress from `normalized_input.fields/signals/attributes` to the
    existing private neutral value; preserve legacy GTM/proposal adapters.
- Run preparation, bundle, receipt, and verification modules identified by
  call-site inspection
  - Bind requirements, taxonomy, prompt, evidence, normalized-output, and
    downstream decision identities without copying raw evidence text.

### U4/U5 — profiles, templates, skills, and docs

- `cli/src/starter.rs`
  - Produce the v3 GTM prompt, DIC processing metadata, taxonomies, semantic
    example, and host envelope.
- `plugin/assets/templates/basic/.mdp/**`
  - Mirror generated GTM v3 manifest/prompt/evals/examples.
- `plugin/assets/templates/proposal/.mdp/**`
  - Migrate proposal to v3 and deterministic pursuit; remove new producer
    dependence on `normalized_prospect` and manual `existing_pack_context`.
- `cli/src/commands/init.rs` and template parity tests
  - Preserve registry/publication behavior and assert new current language.
- `plugin/skills/mdp/SKILL.md`, `plugin/skills/mdp-gtm-brief/**`,
  `plugin/skills/mdp-pack-builder/**`, `plugin/skills/mdp-pack-review/**`, and
  `plugin/skills/mdp-proposal-review/**`
  - Teach collection-spec discovery, semantic normalization, deterministic
    handoff, privacy, and compatibility boundaries.
- `README.md`, `CONCEPTS.md`, `cli/USAGE.md`,
  `docs/decision-input-contracts.md`, `docs/prompt-extraction-contract.md`,
  `docs/conceptual-decision-flow.md`, `docs/getting-started.md`,
  `docs/headless-normalization-runners.md`, `docs/extension-boundary.md`, and
  proposal runtime docs
  - Replace current-producer prospect-shaped guidance while retaining clearly
    labeled legacy sections.

### U6 — conformance

- `cli/src/profile_conformance.rs`
  - Extend the shared gate for v3 taxonomy/classification/neutral-wire
    behavior and a test-only support-shaped fixture.
- Focused tests in the affected modules plus sanitized template fixtures
  - Cover positive, negative, ambiguous, missing, stale, conflicting,
    malformed, injection, compatibility, and tampering cases.

### U7/U8 — release and downstream proof

- `docs/distribution.md`, version files, generated bundles, and release notes
  - Only when the feature delivery is approved for release.
- `orchidautomation/mdp-for-mdp` under MDP-292
  - Adopt the released artifact through its own branch/PR. Private evidence
    stays ignored; only synthetic/sanitized fixtures may be committed.

## 5. Ordered implementation units

### U1. Freeze typed v3 and taxonomy schemas

**Owns:** MDP-286 foundation.  
**Dependencies:** MDP-285 only.  
**Can run with:** U3 scaffolding after shared field sets are frozen.

1. Add red serialization/schema tests for processing modes, taxonomy values,
   contributor constraints, policy enums, bounds, and legacy omission.
2. Add typed models and schemas with closed unknown-field behavior.
3. Require exact taxonomy value-domain equality with the referenced
   attribute's enum.
4. Reject missing/duplicate taxonomies, wrong output attributes, classified
   contributors, cycles, disallowed source classes, and invalid bounds.
5. Keep legacy omission readable as observed; require explicit processing for
   any v3 producer.

### U2. Compile collection and classification specifications

**Owns:** MDP-286 remainder.  
**Dependencies:** U1.

1. Split selected DIC attributes into observed collection requirements and
   model-classification requirements.
2. Keep existing source request/results contracts for observed attributes;
   exclude classified attributes rather than fabricating attempts.
3. Compile only job-selected taxonomies and contributor evidence policies.
4. Emit exact allowed values, definitions, indicators, exclusions, minimum
   evidence, source classes, ambiguity/no-match/conflict policies, and basis
   bounds.
5. Compute canonical selected-taxonomy and requirements hashes.
6. Preserve provider neutrality and add tests asserting that tool/vendor names
   cannot enter authored collection instructions as authority.

### U3. Add semantic-only provider output and host-owned v3 sealing

**Owns:** MDP-287.  
**Dependencies:** U1 shared schema; integrates U2 compiled output.  
**Can run with:** U2 until integration.

1. Add a v3 normalization host-envelope profile with fixed semantic fields
   `classifications`, `gaps`, and `rejected_claims` and fixed host-owned fields
   from the decision document.
2. Project the semantic provider schema and run the existing OpenAI schema
   projection before request sealing.
3. Reject host-owned-field injection and malformed or extra semantic fields.
4. Validate classification status/value/taxonomy/basis and every
   `derived_from` attempt against the trusted compiled/evidence inputs.
5. Enforce contributor, source-class, freshness, minimum-evidence, and conflict
   policy before accepting a classification.
6. Deterministically project observed and classified values into
   `normalized_input`, build signal observations, inject all hashes/identities,
   and validate the sealed v3 envelope.
7. Add v3 neutral ingress and retain unchanged v1/v2 adapters.
8. Bind new hashes through run preparation, bundle, receipt, and verifier
   surfaces without copying private prose.

### U4. Migrate the GTM vertical

**Owns:** MDP-288.  
**Dependencies:** U2 and U3.  
**Can run with:** U5 and U6.

1. Characterize current basic-template bytes and behavior.
2. Define canonical persona and segment taxonomies from the existing pack
   vocabulary, persona cards/mappings, and fit rules; do not invent new market
   claims.
3. Mark title, responsibilities, company, fit, why-now, and contact-policy
   facts as observed; mark only genuine enum derivations as classified.
4. Rewrite the normalization prompt for semantic-only v3 output.
5. Keep fit and why-now evidence independently referenceable.
6. Preserve deterministic fit/routing/generation authority and update relevant
   skills/docs with no provider retrieval instructions.
7. Add positive seller-negative, ambiguous RevOps, missing, stale, and conflict
   fixtures.

### U5. Migrate the proposal vertical and legacy language

**Owns:** MDP-289.  
**Dependencies:** U2 and U3.  
**Can run with:** U4 and U6.

1. Characterize proposal v0 prompt, alias, source-audit, runner, and receipt
   behavior.
2. Define proposal taxonomies only for existing genuine enum classifications.
3. Emit v3 `normalized_input` without `normalized_prospect`.
4. Move pursuit decision to deterministic policy output.
5. Replace manual `existing_pack_context` with compiled job context.
6. Preserve legacy proposal readers and exact alias equality; reject v3/legacy
   mixtures.
7. Update current CLI/schema/help/skill/docs terminology and reconcile MDP-26.

### U6. Extend shared conformance and adversarial proof

**Owns:** MDP-290.  
**Dependencies:** U2 and U3.  
**Can run with:** U4 and U5.

1. Extend the existing conformance gate instead of adding a parallel suite.
2. Test observed passthrough, derived classification, taxonomy/hash identity,
   ambiguity/no-match/conflict, invalid enums, forged evidence refs,
   host-field injection, malformed output, mixed contracts, and tampering.
3. Prove GTM and proposal parity.
4. Add a neutral support-shaped test-only fixture that is absent from the
   profile/template/skill/package registries.
5. Prove CLI/MCP runtime parity and label fixture/mock assurance honestly.

### U7. Integrate, release, and prove the installed path

**Owns:** MDP-291.  
**Dependencies:** U4, U5, U6.

1. Integrate the main-repository delivery, resolve cross-lane conflicts, and
   run focused then exact-head full validation.
2. Include the version bump in the feature delivery when release intent is
   approved; do not create a redundant release-only PR by default.
3. Let release CI package and test the public installer.
4. Verify the installed CLI/MCP/plugin contains v3 schemas, templates, skills,
   and current proposal/GTM language.
5. Record exact commit, release, installer, and synthetic smoke evidence.

### U8. Run MDP-for-MDP dogfood and close out evidence

**Owns:** MDP-292 and MDP-293 in sequence.  
**Dependencies:** U7 published installed artifact.

1. Upgrade MDP-for-MDP through its own branch/PR.
2. Remove manual persona/segment assignment.
3. Use any host research tools to produce the compiled evidence shape without
   creating MDP provider adapters.
4. Run 10-20 real local cases through native normalization, deterministic
   routing, governed generation when allowed, receipts, and verification.
5. Keep raw/private evidence ignored and commit only synthetic or sanitized
   fixtures.
6. Separate product, taxonomy, evidence-quality, and orchestration failures.
7. Create a thin-helper follow-up only if repeated measured friction justifies
   it.
8. Reconcile the proven privacy/transport boundary into MDP-154/187 without
   implementing cloud.

## 6. Tests and validation

### Focused development gates

Run the exact module filters identified during implementation, including at
minimum:

1. `cargo fmt --manifest-path cli/Cargo.toml --check`
2. `cargo test --manifest-path cli/Cargo.toml commands::schemas::tests`
3. `cargo test --manifest-path cli/Cargo.toml commands::health::tests`
4. `cargo test --manifest-path cli/Cargo.toml commands::requirements::tests`
5. `cargo test --manifest-path cli/Cargo.toml commands::prompt_output::tests`
6. `cargo test --manifest-path cli/Cargo.toml model_steps::tests`
7. `cargo test --manifest-path cli/Cargo.toml run_runtime::tests`
8. `cargo test --manifest-path cli/Cargo.toml decision_input`
9. `cargo test --manifest-path cli/Cargo.toml profile_conformance`
10. GTM and proposal init/template parity tests
11. Receipt, run-bundle, and verifier tests touched by new hashes

### Integrated repository gates

1. `cargo test --manifest-path cli/Cargo.toml`
2. `make validate-profile-conformance`
3. `make validate-skills validate-skill-contracts validate-skill-evals validate-skill-packaging validate-asset-sync`
4. `make validate-public-artifacts`
5. Strict validation/eval of freshly initialized GTM and proposal packs
6. `make validate`
7. `git diff --check`

Run the full gate once on the exact integrated release-candidate commit unless
the tree changes or a concrete failure requires diagnosis.

### Required manual/synthetic proofs

- Compile one GTM and one proposal job's collection/classification package and
  inspect that only job-relevant taxonomies appear.
- Run one synthetic native v3 normalization per profile and verify that the
  provider output contains semantic fields only.
- Attempt host-field injection and an unknown evidence reference; both must
  fail without publishing a successful sealed artifact.
- Verify one ambiguous case produces a valid normalization artifact but no
  ready deterministic route.
- Verify old v1/v2 GTM and proposal artifacts remain readable.
- Verify the installed release through CLI and MCP before downstream dogfood.

## 7. Compatibility and migration

- v3 is additive; no v1/v2 artifact is rewritten.
- New producers write v3 only. Legacy aliases are forbidden in v3.
- Legacy DIC omission maps to observed for reads, while v3 authoring requires
  explicit processing.
- Compatibility adapters remain outside the neutral core.
- Divergent proposal aliases retain current failure behavior.
- Automatic taxonomy migration is unsafe and out of scope. Exact health
  diagnostics identify author work; MDP-59 remains parked pending evidence.
- The proposal profile owns opportunity vocabulary; the core owns no second
  opportunity object. MDP-26 is resolved after proposal v3 conformance ships.
- Pack releases are immutable. Taxonomy changes require a new pack version and
  produce different hashes.

## 8. Risks, safety, rollout, observability, and rollback

Risk is **Elevated** because the change crosses public schemas, model
invocation, deterministic routing inputs, templates, receipts, and two
repositories.

### Safety boundaries

- Fail before provider invocation on invalid pack/taxonomy/schema projection.
- Fail before deterministic evaluation on invalid model semantics or lineage.
- Never log secrets, complete private evidence, provider request bodies, or
  raw prospect data into public fixtures/receipts/Linear.
- Never allow model confidence, basis, or prose to grant decision authority.
- Never infer missing taxonomy values or silently choose among conflicts.
- Do not modify cloud, provider integrations, or external systems.
- Do not hide compatibility drift by updating expected snapshots without
  explaining the contract change.

### Rollout

1. Contract/plan pin under MDP-285.
2. Parallel MDP-286/287 implementation after readiness.
3. Parallel MDP-288/289/290 after kernel integration.
4. One cohesive main-repository release and installed proof under MDP-291.
5. Downstream MDP-for-MDP adoption under MDP-292.
6. Evidence/cloud-boundary closeout under MDP-293.

### Observability

Use stable policy-blocked diagnostics for schema projection, classification,
lineage, injection, compatibility, and deterministic readiness failures. Run
bundles and receipts bind artifact identities; they do not attest external
source truth. Pilot results distinguish product failure, taxonomy failure,
evidence failure, and host orchestration friction.

### Rollback

- Before merge: revert/close the feature branch.
- After merge but before release: revert the main-repository PR.
- After release: stop v3 production, retain v1/v2 readers, and roll packs back
  to the prior immutable release. No legacy data migration is required.
- Downstream MDP-for-MDP rollback pins the prior released dependency and pack
  version through its own PR.

## 9. Blockers and readiness verdict

### Resolved

- Product decisions were explicitly approved by Brandon before planning.
- Primary and supporting repositories are named.
- The current implementation and tests were inspected on the default branch.
- v1/v2 compatibility, proposal disposition, cloud boundary, provider
  boundary, and runner non-goal are explicit.
- Work is split into independently executable dependency lanes in Linear.

### Remaining execution gates, not planning blockers

- Each child implementation issue must receive the exact pinned source ref,
  source commit, and this plan path before Orchid delegation.
- No child may start while its Linear blockers remain open.
- Release/version choice occurs only when the integrated delivery is ready.

**Readiness verdict: `READY_TO_PIN`.**

