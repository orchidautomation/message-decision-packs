# MDP-276 — Neutral decision-input core and compatibility adapters

Status: `READY_TO_PIN`

## 1. Context and current behavior

The approved architecture decision in
`docs/orchid/decisions/2026-08-28-primitive-core-profile-template-contract.md`
keeps every current v0 GTM and proposal field readable while moving internal
authority to a profile-neutral decision-input representation. A product-profile
adapter may translate a stable public contract into that representation; core
code may not infer or switch on a product profile.

Repository inspection on cumulative head
`70d8390e4010f3ba88e0c8fadecfcba69ab11060` confirms:

- `cli/src/models.rs::Manifest` exposes the public
  `lead_input_requirements: LeadInputRequirements` field. Its GTM-shaped name
  and serialized shape are stable compatibility surfaces.
- `cli/src/models.rs::Prospect` requires `name`, `title`, and `company` and is
  used by detached GTM fit, eval, brief, and governed qualification paths.
- `cli/src/commands/requirements.rs` already compiles a neutral, versioned
  `mdp.normalized-decision-input.v1/v2` envelope, but its payload projection,
  diagnostics, and schema keys still name `normalized_prospect` directly.
- `cli/src/commands/prompt_output.rs::validate_normalized_prospect` and
  `validate_normalized_opportunity_alias` manually validate the legacy
  `prospect-normalization` envelope. Proposal may omit
  `normalized_opportunity`; when present it must be an object and exactly equal
  `normalized_prospect`. The alias is rejected outside the proposal profile.
- `cli/src/commands/routing.rs::fit_prospect_with_signal_authority` combines
  generic readiness checks with GTM-only persona resolution, person-resolution
  gates, fit-card matching, and prospect output vocabulary.
- `cli/src/value_contracts.rs` reads `Manifest.lead_input_requirements`
  directly and reports prospect-scoped diagnostics for both typed and JSON
  inputs.
- `cli/src/run_runtime.rs` has separate proposal validation and GTM
  qualification operations. Only the GTM operation deserializes the governed
  projection as `Prospect` and invokes deterministic qualification.
- Shipped GTM uses profile `gtm`, input contract `prospect`, and
  `mdp.input.prospect.v0`. Shipped proposal uses profile `proposal`, input
  contract `opportunity`, and `mdp.input.opportunity.v0`; its normalization
  prompt deliberately retains `normalized_prospect` as the v0 compatibility
  bridge and optionally emits the exact opportunity alias.

The existing prompt-output, requirements, source-binding, compiler/runtime,
schema, brief, eval, and template-parity tests are compatibility authority. The
implementation must not change template bytes, public JSON keys, golden hashes,
receipt binding, or current diagnostics merely to make the internals appear
neutral.

## 2. Objective, scope, out of scope, and decisions

### Objective

Introduce one private neutral decision-input/subject representation plus
explicit GTM-prospect and proposal-opportunity adapters. Shared validation and
ingress will operate on the neutral value; GTM qualification and proposal
source-audit semantics will remain owned by their adapters. Current v0 wire
contracts remain unchanged and conflicting or mixed representations fail
closed.

### In scope

- A private neutral subject/value model with optional scalar fields, signals,
  and attributes that does not require person, title, company, prospect, or
  opportunity vocabulary.
- A closed adapter selection boundary for the two currently authorized
  profile/input-contract pairs: `gtm` + `prospect`, and `proposal` +
  `opportunity`.
- Adapter-backed parsing of typed `Prospect`, governed
  `normalized_prospect`, and legacy proposal prompt-output aliases.
- A neutral internal view of `LeadInputRequirements`; the serialized manifest
  field and public response keys remain untouched.
- Migration of shared value/readiness validation to the neutral representation,
  with compatibility wrappers preserving existing callers and diagnostics.
- Explicit GTM qualification and proposal normalization/source-audit adapter
  entry points.
- Producer/reader, old-artifact, mixed-representation, unknown-field,
  ambiguous-owner, and alias-conflict tests across the affected command seams.

### Out of scope

- Removing, renaming, or changing `Prospect`, `LeadInputRequirements`,
  `lead_input_requirements`, `normalized_prospect`, or
  `normalized_opportunity` on the wire.
- Making `normalized_opportunity` a second core object or changing its exact
  equality/optional behavior.
- Changing initialized GTM/proposal template bytes, prompt instructions,
  schemas, current CLI JSON keys, receipt formats, or golden hashes.
- Generalizing GTM fit cards, person resolution, outbound briefs, or
  qualification gates into proposal.
- Activating a third profile, implementing the declarative job/skill registry
  (MDP-277), or changing template publication (MDP-278).
- Merge, release, deployment, installation, or production mutation.

### Decisions and assumptions

1. The neutral core stores a bounded closed field map (or equivalent typed
   optional fields), signals, and attributes. It must not contain a profile ID
   or label the subject a prospect/opportunity.
2. Adapter selection happens once, outside neutral validation. Selection uses
   the manifest profile plus the selected prompt/job input-contract ownership;
   missing, unknown, multiple, or mixed ownership returns a structured error.
3. The GTM adapter is the only owner of `Prospect` deserialization, persona
   projection, person resolution, qualification gates, fit cards, and the
   legacy `mdp.fit.v0` prospect response.
4. The proposal adapter treats `normalized_opportunity` as the readable
   presentation field when present and `normalized_prospect` as its required
   v0 compatibility peer. An old artifact containing only
   `normalized_prospect` remains readable. If both are present, byte-independent
   JSON value equality must be exact before either enters the neutral core.
5. Current v0 proposal output still requires the legacy
   `normalized_prospect` surface. This issue changes internal authority, not
   the public prompt contract. Opportunity-only neutral values are proved at
   the private adapter/core seam, not introduced as a new unversioned wire
   shape.
6. Neutral requirements validation checks only fields declared by the selected
   profile contract. Actor/persona resolution is adapter-owned and ambiguity
   fails rather than selecting a guessed actor.
7. Existing public diagnostics retain their codes, paths, scope strings, and
   messages for accepted v0 surfaces. New adapter-selection diagnostics are
   added only for previously ambiguous, mixed, or unsupported representations.

## 3. Acceptance mapping

| Acceptance criterion | Implementation | Validation |
| --- | --- | --- |
| Shared core validates/routes a decision input without requiring person/title/company/prospect vocabulary unless selected profile requires it | Add the neutral model and requirements view; have core readiness operate on declared fields only, then route through an already selected adapter. | Unit tests construct a neutral proposal subject with no person/title fields and validate it; GTM adapter tests still reject missing required prospect fields. |
| GTM keeps existing prospect/lead fields and interpretation | Preserve `Prospect`, public schemas/keys, existing compatibility wrappers, and route all fit/brief/eval/governed qualification through the GTM adapter. | Existing prospect validation, routing, brief, eval, requirements, v1/v2 lineage, and golden JSON tests pass unchanged. |
| Proposal accepts opportunity semantics without an internally authoritative prospect alias | Resolve proposal output into one neutral subject through the proposal adapter; the exact alias check is performed at ingress and the core receives neither wire name. | Proposal adapter tests prove alias-present, legacy alias-absent, and private opportunity subject cases; source-audit/proposal runtime tests remain green. |
| Old GTM/proposal artifacts remain readable and alias conflicts fail safely | Keep all v0 fields and wrapper paths; reject non-object, non-proposal, mismatched, or mixed aliases before conversion. | Existing fixtures plus new old-artifact and adversarial alias tests assert exact current diagnostics and no successful conversion on conflict. |
| Unknown fields, ambiguous profile ownership, and mixed representations fail closed | Use closed field sets and explicit profile/input-contract selection; never infer an adapter from payload names alone. | Tests cover unknown profile, missing profile, wrong profile/contract pair, multiple competing input contracts, opportunity field in GTM, and unequal dual fields. |

## 4. Affected files and symbols

### Worker-owned implementation surface

- `cli/src/decision_input.rs` (new)
  - Neutral `DecisionInput`/subject, requirements view, closed field access,
    adapter kind/selection, compatibility conversion, and focused unit tests.
- `cli/src/main.rs`
  - Register the private module only.
- `cli/src/models.rs`
  - Add only minimal derives/accessors needed to adapt existing public types;
    do not change serialized fields or defaults.
- `cli/src/value_contracts.rs`
  - Split neutral value/attribute validation from GTM persona projection;
    preserve `prospect_contract_violations` and
    `normalized_prospect_contract_violations` as compatibility wrappers.
- `cli/src/prospect_validation.rs`
  - Delegate the closed GTM wire check through the GTM adapter without changing
    existing error codes or guidance.
- `cli/src/commands/prompt_output.rs`
  - Resolve `prospect-normalization` payload ownership through the adapter,
    preserve exact alias checks, and keep proposal source-audit validation
    behind the proposal adapter.
- `cli/src/commands/requirements.rs`
  - Use neutral subject projection helpers internally while retaining every
    existing `normalized_prospect*` public response/schema key and diagnostic.
- `cli/src/commands/routing.rs`
  - Put current prospect/persona/qualification behavior behind the GTM adapter;
    add a neutral readiness handoff without changing `mdp.fit.v0` output.
- `cli/src/commands/source_binding.rs`
  - Use the selected neutral input ownership when binding/validating compiled
    attributes; add mixed/wrong-adapter negative tests without changing
    receipts.
- `cli/src/run_request_compiler.rs`
  - Validate adapter ownership before accepting normalized input; preserve
    request contracts and diagnostic codes for existing failures.
- `cli/src/run_runtime.rs`
  - Select the adapter before proposal validation or GTM qualification and
    remove internal cross-profile payload assumptions. Preserve operation names,
    terminal states, artifacts, receipts, and hashes.
- `cli/src/commands/schemas.rs` and `cli/src/commands/health.rs`
  - Add compatibility/adversarial assertions and route semantic example checks
    through the adapter where needed. Do not alter current generated schema or
    template validation output unless the change is strictly an internal
    refactor with byte-identical JSON.
- `cli/src/commands/evals.rs` and `cli/src/commands/briefs.rs`
  - Minimal adapter call-site changes/tests only; preserve GTM public output and
    markdown.

The worker may leave any listed file unchanged when the seam is already covered
through a lower-level adapter. It must not widen beyond these paths without a
plan-conflict escalation.

### Sol-owned integration surface

- `docs/orchid/plans/2026-08-28-004-mdp-276-neutral-decision-input-plan.md`
  is immutable during worker execution.
- `docs/orchid/qa/2026-08-28-mdp-276-execution-receipt.json` may be added by
  Sol after exact-head verification.
- The cumulative branch, PR #236 body, and Linear lifecycle evidence are Sol
  responsibilities.

### Forbidden worker surfaces

Do not edit template manifests/assets, prompt YAML, example fixtures, plugin
skills, Cargo files, public docs, architecture decisions, other plans, QA
receipts, CI/release files, or any file outside the worker-owned paths. Do not
create or edit a PR. Escalate instead of widening scope or accepting output
drift.

## 5. Ordered implementation steps

1. **Define the neutral core.**
   - Add a private neutral value whose core fields are optional/bounded and
     whose `signals` and `attributes` retain their existing values.
   - Add neutral field lookup/presence helpers and a requirements view derived
     from `LeadInputRequirements` without serializing a second manifest field.
   - Reject unknown top-level/signal fields before conversion. Do not retain a
     profile identifier in the neutral value.

2. **Select an explicit adapter.**
   - Resolve exactly one allowed manifest-profile/input-contract pairing.
   - Distinguish GTM prospect wire, governed GTM normalized input, and proposal
     opportunity prompt output without guessing from card kinds, payload field
     presence, prompt tags, or template paths.
   - Return deterministic structured diagnostics for unknown, absent,
     competing, or mixed ownership.

3. **Implement GTM compatibility conversion.**
   - Convert `Prospect` and normalized GTM JSON into the neutral value after
     the existing closed-wire validation.
   - Keep persona/title mapping, company-domain normalization, person
     resolution, qualification signal roles, gates, fit cards, scope, and
     `mdp.fit.v0` rendering inside a clearly named GTM adapter path.
   - Preserve existing detached/governed ingress behavior and hashes.

4. **Implement proposal compatibility conversion.**
   - Validate profile ownership before accepting `normalized_opportunity`.
   - When both public fields exist, require exact equality before selecting one
     neutral value. When only the required legacy `normalized_prospect` exists,
     adapt it as an old proposal artifact.
   - Validate proposal requirements, attributes, signals, source-audit refs,
     normalization trace, and no-fake-person behavior through the proposal
     adapter. Do not invoke GTM fit/person-resolution semantics.

5. **Move shared requirements/value validation onto the neutral view.**
   - Check declared required fields, signal fields, required attributes, value
     contracts, attribute definitions, and undeclared-attribute policy without
     prospect vocabulary in the core function.
   - Apply persona/actor mapping only through the selected adapter. Duplicate,
     conflicting, or unrecognized mappings must fail rather than select a
     candidate silently.
   - Wrap results back into existing prospect-scoped codes/paths/messages for
     current GTM and proposal v0 callers.

6. **Wire prompt output, requirements, source binding, and runtime ingress.**
   - Use the adapter at each external-input boundary, then pass only the
     neutral value into shared validation.
   - Keep `normalized_prospect`, `normalized_prospect_schema`,
     `normalized_prospect_unbound_policy`, projected-prospect hashes and paths,
     receipt fields, and current stdout intact as compatibility renderers.
   - Ensure compiler/runtime reject profile/input/operation mismatches before
     publishing success artifacts.

7. **Add compatibility and adversarial tests.**
   - Neutral subject without person/title/company succeeds when its supplied
     requirements do not declare those fields.
   - The same missing fields fail through the GTM adapter where the manifest
     requires them.
   - Old GTM prospect, GTM normalized v1/v2, old proposal prospect-only, and
     proposal exact-alias artifacts remain accepted.
   - Non-object alias, unequal aliases, opportunity alias on GTM, mixed
     prospect/opportunity ownership, wrong profile/input contract, unknown
     fields, and ambiguous actor mappings fail closed.
   - Existing producer/reader schemas, source-binding hashes, run receipts,
     brief/eval output, and template parity remain exact.

8. **Stop on unexplained compatibility drift.**
   - Do not update templates, fixtures, snapshots, diagnostics, hashes, or
     expected output to normalize a failure. Any current valid artifact or
     exact-output drift is a plan conflict for Sol.

## 6. Tests and validation

Run on the worker commit and on the final integrated exact head:

1. `cargo fmt --manifest-path cli/Cargo.toml --check`
2. `cargo test --manifest-path cli/Cargo.toml decision_input`
3. `cargo test --manifest-path cli/Cargo.toml prospect_validation`
4. `cargo test --manifest-path cli/Cargo.toml value_contracts`
5. `cargo test --manifest-path cli/Cargo.toml commands::prompt_output::tests`
6. `cargo test --manifest-path cli/Cargo.toml commands::requirements::tests`
7. `cargo test --manifest-path cli/Cargo.toml commands::source_binding::tests`
8. `cargo test --manifest-path cli/Cargo.toml commands::routing::tests`
9. `cargo test --manifest-path cli/Cargo.toml run_request_compiler::tests`
10. `cargo test --manifest-path cli/Cargo.toml run_runtime::tests`
11. `cargo test --manifest-path cli/Cargo.toml commands::evals::tests`
12. `cargo test --manifest-path cli/Cargo.toml commands::briefs::tests`
13. `cargo test --manifest-path cli/Cargo.toml commands::schemas::tests`
14. `cargo test --manifest-path cli/Cargo.toml commands::health::tests`
15. `cargo test --manifest-path cli/Cargo.toml generated_basic_starter_matches_plugin_template`
16. `cargo test --manifest-path cli/Cargo.toml generated_proposal_starter_matches_plugin_template_pack_files`
17. Strict validate and eval for freshly initialized GTM and proposal packs.
18. `cargo test --manifest-path cli/Cargo.toml`
19. `python3 -m unittest scripts/test_public_artifact_lint.py`
20. `python3 scripts/lint-public-artifacts.py`
21. `git diff --check`
22. Deterministic source audit: product profile IDs occur only in adapter
    selection/compatibility tests, neutral core symbols contain no GTM,
    proposal, prospect, opportunity, person, title, or company requirement,
    shipped template bytes are unchanged, and no new public schema/receipt key
    was introduced.

The optional Clippy component is not installed in the pinned Rust toolchain and
is not required proof. Do not install it for this issue.

## 7. Compatibility and migration behavior

- No persisted-data, schema-version, template, prompt, or receipt migration.
- `Prospect`, `LeadInputRequirements`, manifest field names/defaults,
  `normalized_prospect`, proposal alias equality, and all serialized spellings
  remain unchanged.
- The private adapter is additive. Old GTM/proposal artifacts enter through the
  same public surfaces and are converted before shared validation.
- Proposal alias absence remains accepted exactly where it is accepted today;
  alias mismatch, non-object, and non-proposal use retain current failures.
- GTM continues to require its manifest-declared person/account fields and is
  the only profile that invokes deterministic fit/person-resolution behavior.
- Unknown or competing ownership fails before core execution and before a run
  publishes success artifacts.
- No release, install, deployment, migration command, or third-profile
  activation is authorized.

## 8. Risks, safety boundaries, rollout, observability, and rollback

Risk is **Elevated** because versioned inputs, qualification decisions, and
receipt-bound runtime execution cross this adapter boundary.

Safety boundaries:

- Fail closed before conversion on unknown fields, ambiguous ownership, or
  alias disagreement.
- Never expose source bodies, private material, or new raw values in errors.
- Do not infer adapter ownership from card kinds, profile-native card IDs,
  prompt prose, or payload aliases.
- Do not move GTM qualification into neutral or proposal code.
- Do not change public examples/template bytes or add a third profile.
- Do not create/edit PRs, merge, release, deploy, or mutate production.

Rollout is limited to the issue branch and the existing cumulative PR #236.
Exact-head tests, strict starter validation/evals, JSON/receipt compatibility,
and CI are the observable proof.

Rollback is a clean revert of the MDP-276 implementation commit(s). Because the
wire formats and stored artifacts remain unchanged, rollback needs no reverse
migration.

## 9. Blockers and readiness verdict

MDP-273 is approved; MDP-274 and MDP-275 are integrated into the cumulative
head; MDP-276 has no current Linear blocker; the worktree is clean and based on
the cumulative branch containing current `origin/main`. The compatibility
decision is explicit, and no product decision or cross-repository dependency
remains.

Readiness verdict: `READY_TO_PIN`.
