---
title: MDP-236 Semantic Authority and Distribution Identity - Plan
type: decision
date: 2026-08-21
execution: code
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-236 / linear-mdp-239 / linear-mdp-154 / linear-mdp-179 / linear-mdp-217
linear_issues:
  - MDP-236
  - MDP-239
  - MDP-154
  - MDP-179
  - MDP-217
---

# MDP-236: separate semantic authority identity from distribution identity

## Goal capsule

| Field | Decision |
| --- | --- |
| Objective | Add one normative, fail-closed two-identity model: a semantic-authority digest for files that can change MDP decisions/readiness/prompts/schemas/validation, and a distribution digest for every shipped pack file. |
| Current failure | `artifact_hash::pack_content_snapshot` hashes every regular file under `.mdp/` except generated `briefs/` and `traces/`; README orientation and integration-owned distribution files therefore share the digest used by requirements, source bindings, prompt-output authority, clean-run bundles, traces, and conformance. |
| Product shape | Add `mdp.pack-identity.v2` plus `mdp.pack-file-classification.v1`, keep the legacy `mdp.portable-pack-snapshot.v1` readable, expose both identities in pack/requirements output, and carry semantic identity plus optional exact distribution identity through bindings and receipts. |
| Authority boundary | The Rust CLI remains the only implementation authority for classification, hashing, compatibility, and verification. README prose never becomes decision authority. Integrations may carry the identity fields but do not calculate or reinterpret them. |
| Compatibility | `portable_digest` and legacy `pack.sha256` remain the old distribution/portable value for old artifacts. New identity-aware artifacts carry explicit semantic and distribution fields. A legacy artifact is readable and verifiable at its old assurance level, never silently promoted to a semantic-authority claim. |
| Public safety | Unknown regular files, unsafe paths, symlinks, case collisions, and non-regular entries fail closed. Distribution-only files are named by an explicit contract; no arbitrary exclusion is added to make a fixture pass. |
| Execution state | This is an implementation-ready plan handoff only. MDP-236 and MDP-239 remain in their current Backlog/research/planned states until the parent execution gate permits implementation. This branch contains no runtime, template, release, label, status, or branding mutation. |

## Problem and non-negotiable boundary

The current `cli/src/artifact_hash.rs::pack_content_snapshot` walks `.mdp/`,
sorts portable relative paths, hashes every regular file, and excludes only the
generated `briefs/` and `traces/` directories. The resulting
`mdp.portable-pack-snapshot.v1` hash is correct as a distribution/integrity
snapshot, but it is also reused as if it were the semantic identity of the
pack:

- `cli/src/commands/requirements.rs::requirements` puts it in
  `data.pack.sha256`.
- `cli/src/commands/source_binding.rs` checks `pack.sha256` against the
  compiled requirements pack.
- `cli/src/commands/prompt_output.rs` writes it into validation authority and
  `cli/src/commands/decision_trace.rs::current_pack_prompt_binding_matches`
  recomputes it for trace binding.
- `cli/src/run_runtime.rs::execute_transaction` puts the snapshot hash and
  file inventory in `run-bundle.v1::PackAuthority`.
- `cli/src/conformance.rs::PackReleaseIdentity` and
  `cli/src/commands/conformance.rs::validate_inventory_candidate_context`
  use it as release identity.

`cli/src/pack_readme.rs` and the MDP-217 implementation correctly keep README
prose out of product-foundation and readiness authority, but the identity
model still treats README bytes as a semantic change. Editing only
`.mdp/README.md` should therefore change the distribution identity while
leaving semantic authority, requirements, source-binding eligibility, and
decision meaning unchanged. A mutation to `manifest.yaml`, a card, a prompt,
an eval, `sources.yaml`, or another authority file must change both identities
and must invalidate the semantic binding.

The implementation must not solve this by dropping files from the existing
portable digest, by treating README prose as structured authority, or by
accepting unknown files as “orientation.” The two identities must be explicit,
versioned, independently recomputable, and visible to operators.

## Normative identity contract

### Terms

| Term | Meaning | Failure/assurance rule |
| --- | --- | --- |
| Semantic authority | The exact set of structured files that can affect decisions, readiness, prompt behavior, schemas, or validation. | Any byte change changes `semantic_authority_sha256`; a mismatch blocks an authority-bound operation. |
| Orientation | Human-facing README prose and generated inventory projection. It may explain loaded authority but cannot satisfy or override it. | Orientation changes do not change the semantic digest. They do change distribution identity. |
| Distribution-only | A shipped file needed by an integration or package consumer but not read by the MDP decision/readiness compiler. | It participates in distribution identity only. Its change is a distribution drift, not a semantic decision change. |
| Generated local artifact | A local `.mdp/briefs/` or `.mdp/traces/` output that is not part of a shipped pack release. | It is excluded from both identities, preserving the existing non-staling behavior. A future shipped generated artifact needs a new explicit classification. |
| Unknown | Any regular file or path not covered by the classification contract. | New identity calculation and strict validation fail closed with a stable path-only diagnostic. No body is returned. |
| Legacy portable identity | `mdp.portable-pack-snapshot.v1` and fields named `portable_digest` or legacy `pack.sha256`. | Read/verify remains available for existing artifacts, but the verifier records that semantic authority was not bound. It never infers semantic equivalence from the old hash. |

### File-classification contract (`mdp.pack-file-classification.v1`)

The classifier operates on `.mdp/` logical paths after the existing
descriptor/symlink/path safety checks. The first matching rule wins; overlapping
rules are a validation error rather than an order-dependent choice.

| Logical path/pattern | Class | Semantic digest | Distribution digest | Rationale |
| --- | --- | ---:| ---:| --- |
| `manifest.yaml` | semantic-authority | yes | yes | Pack identity, profile/jobs, policies, prompt bindings, schemas, and requirements. |
| Every path referenced by `manifest.cards[].path` | semantic-authority | yes | yes | Canonical card content is decision authority even when a pack uses a custom subdirectory. |
| `cards/**/*.yaml` and `cards/**/*.yml` | semantic-authority | yes | yes | Standard card tree; an unreferenced card still cannot be silently treated as orientation. Health validation must report dangling/unmapped content separately. |
| `prompts/**/*.yaml` and `prompts/**/*.yml` | semantic-authority | yes | yes | Prompt instructions and output contracts can change model-visible behavior and validation. |
| `evals/**/*.yaml` and `evals/**/*.yml` | semantic-authority | yes | yes | Profile activation and validation coverage consume these fixtures. |
| `sources.yaml` | semantic-authority | yes | yes | Source IDs, purposes, gaps, and source validation references are loaded authority. |
| `README.md` | orientation | no | yes | Human orientation and MDP-217 owned inventory projection only. |
| `source-strategy.json` | distribution-only | no | yes | The shipped example integration strategy is consumed by an external adapter, not by MDP's decision compiler. It must still be integrity-bound as a distributed file. |
| `briefs/**`, `traces/**` at the `.mdp/` root | generated-local | no | no | Preserve the existing generated-artifact exclusion; these paths are not shipped pack authority. |
| Any other regular file, unsupported extension under a known directory, or unknown directory containing files | unknown | no | no | Fail closed with `pack_file_classification_unknown`; add a reviewed classifier row before shipping a new file class. |

The implementation should keep the exact current generated-directory boundary
and should not recurse into generated directories merely to discover an
unclassified local output. Symlinks, path traversal, non-regular entries,
non-UTF-8 logical names, and case-folded path collisions continue to fail as
they do today, with the new identity contract preserving the path-only,
public-safe diagnostic behavior.

### Identity payload and hashing

Add a closed, schema-advertised `mdp.pack-identity.v2` payload. The exact
field names are part of the implementation contract:

```json
{
  "contract": "mdp.pack-identity.v2",
  "classification_contract": "mdp.pack-file-classification.v1",
  "semantic_authority_sha256": "<64 lowercase hex chars>",
  "distribution_sha256": "<64 lowercase hex chars>",
  "semantic_files": [
    {"logical_path": "manifest.yaml", "byte_count": 123, "sha256": "<...>"}
  ],
  "distribution_files": [
    {"logical_path": "README.md", "byte_count": 456, "sha256": "<...>"}
  ]
}
```

The implementation must make these invariants testable rather than relying on
field names alone:

1. `distribution_sha256` uses the existing portable relative-path/file-hash
   framing and the existing generated-directory exclusion. For a pack with no
   unknown files, it is byte-for-byte equal to the old
   `PortablePackSnapshot.sha256`/`portable_digest`. This preserves release and
   artifact integrity while giving the value an explicit name.
2. `semantic_authority_sha256` uses the same sorted logical-path/file-hash
   record framing over only semantic-authority files, prefixed by the
   domain/version `mdp.pack-semantic-authority.v2\0`. The domain prefix keeps
   the semantic value distinct from raw artifact hashes and the legacy
   distribution value.
3. File records are built from the bytes read for hashing, not from a separate
   path read. The returned inventory and both digests come from one coherent
   snapshot. Independent recomputation from `semantic_files` and
   `distribution_files` must reproduce the published values.
4. The semantic list is non-empty and includes `manifest.yaml`. Both lists are
   sorted by ASCII logical path, have no case-folded collisions, and contain no
   generated or unknown path.
5. The classifier contract version is bound in the identity payload. A future
   classification change cannot silently reinterpret an old digest.

The existing `mdp.portable-pack-snapshot.v1` struct and
`pack_content_snapshot`/`pack_content_sha256` functions remain as a legacy
read/verify path. New callers use `pack_identity_snapshot` and explicit
`semantic_authority_sha256`/`distribution_sha256` accessors. Do not rename the
old function in a way that makes old `portable_digest` fields appear to be
semantic.

### Identity drift semantics

Identity-aware artifacts must carry `identity_contract` plus
`semantic_authority_sha256`. They may carry `distribution_sha256` when the
host has the exact shipped bundle. The old required `sha256`/`portable_digest`
field remains a distribution alias solely so existing outer contracts and
readers can parse the new artifact.

| Observed change | Semantic digest | Distribution digest | Authority result |
| --- | --- | --- | --- |
| README prose or generated inventory block only | unchanged | changed | New identity-aware source binding/decision validation stays semantic-ready; it reports non-blocking distribution drift when an old optional distribution pin is present. It must not claim the decision authority changed. |
| `source-strategy.json` only | unchanged | changed | Same semantic-ready/distribution-drift behavior. Exact bundle consumers may require the new distribution pin. |
| Manifest, cards, prompts, evals, `sources.yaml`, or a custom manifest-referenced authority file | changed | changed | Block stale semantic bindings and receipts. |
| `.mdp/briefs/**` or `.mdp/traces/**` only | unchanged | unchanged | Existing behavior remains: local output does not stale a pack identity. |
| Unknown/unclassified file added or changed | not published | not published | Fail closed before an identity-aware decision or release is produced. |

For a new identity-aware binding, semantic mismatch is an error. A supplied
distribution pin is an exact historical observation: a mismatch caused only by
orientation/distribution-only bytes is a warning/compatibility state, not a
semantic success claim about the new bundle. Run-bundle verification and an
explicit exact-distribution release check may still require the distribution
value to match.

## Precise implementation surfaces

The implementation branch should confirm these seams against the rebased
target and avoid unrelated route/runtime changes.

| Area | File | Current symbol(s) | Planned responsibility |
| --- | --- | --- | --- |
| Hash authority | `cli/src/artifact_hash.rs` | `PortableFileRecord`, `PortablePackSnapshot`, `GENERATED_PACK_DIRECTORIES`, `pack_content_snapshot`, `pack_content_sha256`, `collect_regular_files` | Add `PackFileClass`, `PackIdentitySnapshot`, the classifier, `pack_identity_snapshot`, and explicit semantic/distribution accessors. Keep the v1 portable reader and byte/path safety behavior. Return stable unknown-classification errors without exposing file bodies. |
| Pack I/O | `cli/src/pack_io.rs` | `read_manifest`, `resolve_pack_path` | Reuse manifest/card path resolution for custom manifest-referenced semantic files; add no second path traversal or hashing implementation. |
| Pack command | `cli/src/commands/pack.rs` | `pack` | Emit the v0 card/prompt inventory plus an additive `identity` projection: contract versions, both digests, class counts, and path/hash inventory. Keep existing card/prompt fields and output artifact behavior. |
| Health/validation | `cli/src/commands/health.rs` | `validate_pack`, `doctor`, `validate_manifest_shapes`, `collect_eval_inventory` | Invoke the classifier in strict validation, surface `pack_file_classification_unknown` as an error, and keep generated README drift diagnostic-only. Ensure identity calculation does not turn README prose into readiness authority. |
| Requirements | `cli/src/commands/requirements.rs` | `requirements`, `pack_summary`, `finalize_requirements` | Add identity contract and semantic/distribution fields to `data.pack`; preserve legacy `sha256` as the distribution alias; retain the legacy full-object `requirements_sha256`, and add a domain-separated `semantic_requirements_sha256` computed from a projection that removes distribution-only identity fields. |
| Signal authority | `cli/src/commands/routing.rs` | `fit_normalized` | Bind `identity_contract`, `semantic_authority_sha256`, and `semantic_requirements_sha256` into new signal authority; keep legacy `pack_sha256`/`requirements_sha256` evidence for old artifacts and exact distribution checks, but do not let README/distribution drift change semantic routing authority. |
| Source binding | `cli/src/commands/source_binding.rs` | `PackPin`, `SourceBinding`, `validate_source_binding_value`, `validate_source_binding_v2`, `source_binding_schema_v1`, `source_binding_schema_v2`, `validation_result` | Add optional identity fields to both existing lineage versions. Legacy artifacts use exact old distribution matching and are reported as legacy. Identity-aware artifacts require semantic equality, optionally report distribution drift as a warning, and reject semantic mismatch. Do not require a v1 binding to pretend it was created with v2 identity. |
| Prompt authority | `cli/src/commands/prompt_output.rs` | `validate_prompt_output_file_with_lineage_inputs`, `attach_prompt_output_validation_authority`, `enforce_unchanged_validation_pack`, `validate_governed_artifact_authority` | Bind both identities in new validation authority; compare semantic identity for authority and retain exact distribution evidence for optional bundle checks. Preserve prompt/output/input hashes and no-draft behavior. |
| Trace authority | `cli/src/commands/decision_trace.rs` and `cli/src/commands/decision_trace/tests.rs` | `current_pack_prompt_binding_matches`, trace projection tests | Recompute the identity snapshot once, require semantic equality for identity-aware validation, preserve legacy trace compatibility, and project safe identity mode/drift diagnostics without raw pack bytes. |
| Run bundle | `cli/src/run_contracts.rs` | `PackAuthority`, `RunBundleV1`, `RunReceiptV1`, contract constants | Add serde-defaulted identity fields (`identity_contract`, `semantic_authority_sha256`, `distribution_sha256`) to `PackAuthority`. Keep `portable_digest` and v1 contract strings readable; new runtime receipts populate both identities and retain the distribution file inventory. |
| Run runtime | `cli/src/run_runtime.rs` | `execute_transaction`, `validate_pack_snapshot_bounds`, `copy_pack`, post-run snapshot comparison, `success_artifacts`, `gtm_lineage_schema_ids` | Stage and compare one identity snapshot before/after execution; bind semantic and distribution fields into `PackAuthority`; keep exact source/staged mutation checks and audit-incomplete behavior. README mutations during a run remain an immutable-snapshot failure even though they do not change semantic authority. |
| Run verification | `cli/src/commands/run_verification.rs` | `verify_run`, `verify_runner_audit`, v1 fixture builders | Validate identity-field shape and legacy/new mode. Recompute both digests when the staged pack/file inventory is available; otherwise report semantic recomputation as unavailable rather than inferring it from `portable_digest`. Preserve old v1 receipt validity and lower-assurance legacy limitations. |
| Conformance | `cli/src/conformance.rs` and `cli/src/commands/conformance.rs` | `PackReleaseIdentity`, `ConformanceCandidateV1`, `validate_inventory_candidate_context`, `compile_candidate`, `evaluate_*` | Carry semantic identity plus optional distribution identity through candidate/job/report contracts. Legacy candidates remain readable but cannot claim the new semantic identity assurance. New candidates compare semantic authority and exact distribution where requested. |
| Schemas | `cli/src/commands/schemas.rs` and `cli/src/cli.rs` | `portable_file_v1_schema`, `pack_authority_v1_schema`, `run_bundle_v1_schema`, `SchemaTarget`, `conformance_schema` | Add the closed `pack-identity-v2` schema target, optional identity fields to v1 pack/run/conformance schemas, and conditional legacy-vs-identity tests. Keep existing v0/v1 schema targets and unknown-field rejection. |
| Constants/capabilities | `cli/src/constants.rs`, `cli/src/commands/capabilities.rs` | contract constants, command/contract registry, stable errors | Register `PACK_IDENTITY_CONTRACT_V2`, `PACK_CLASSIFICATION_CONTRACT_V1`, capability metadata, identity modes, and stable diagnostics (`pack_file_classification_unknown`, `pack_semantic_authority_mismatch`, `pack_distribution_drift`, `pack_identity_legacy`). |
| Human/summary output | `cli/src/output.rs` | `summary_for`, `context_summary`, pack/requirements summaries | Explain in plain language that semantic authority controls decisions while distribution identity covers shipped bytes. Show both hashes and drift mode without printing file bodies or private content. |
| Starter/template parity | `cli/src/commands/init.rs`, `cli/src/starter.rs`, `cli/src/target_starter.rs`, `assets/templates/**`, `plugin/assets/templates/**` | init writers and identity/readme regression tests | Ensure both starter templates and `mdp init` pass the classifier. Update only assertions/docs unless a template has an unclassified file; preserve plugin/assets parity and README ownership. |
| Public fixtures/scripts | `cli/tests/fixtures/pack-identity-v2/`, `scripts/test-cold-model-conformance.mjs`, `scripts/test_skill_contracts.py`, `scripts/test-run-conformance.mjs` | fixture builders and portable digest expectations | Add synthetic mutation/migration vectors and update consumers to use semantic/distribution fields without weakening legacy checks. No real contacts, provider payloads, secrets, or private paths. |
| User contracts | `README.md`, `cli/USAGE.md`, `docs/product-foundations.md`, `docs/decision-input-contracts.md`, `docs/minimal-context-routing.md`, `docs/decision-traces.md`, `docs/run-receipts.md`, `docs/getting-started.md` | current portable-digest/README identity statements | Replace claims that README changes semantic authority. Document the classifier, two identities, drift semantics, migration, and exact CLI output. |
| Canonical skills and durable architecture docs | `plugin/skills/mdp/SKILL.md`, `plugin/skills/mdp/references/mental-model.md`, `plugin/skills/mdp/references/cli-operator.md`, `plugin/skills/mdp-pack-builder/SKILL.md`, `plugin/skills/mdp-pack-review/references/structural-audit.md`, `docs/orchid/decisions/2026-08-03-unified-clean-context-runtime.md`, `docs/orchid/requirements/2026-08-08-mdp-195-self-standing-pack-sufficiency-contract.md` | portable-digest guidance and run/release identity assertions | Align operator guidance, builder/reviewer checklists, MDP-154 release semantics, and MDP-179 assurance language. Keep source/readability boundaries and public-artifact rules intact. |

## Ordered implementation units

### U1. Freeze the classification and identity vocabulary

1. Characterize current v1 pack snapshots on the basic and proposal templates,
   the MDP-217 README inventory path, the source-strategy example, generated
   directories, and a custom manifest card path.
2. Add `mdp.pack-file-classification.v1` and `mdp.pack-identity.v2` constants,
   schema target, capability entries, and a short normative documentation
   section before changing consumers.
3. Implement one classifier in `artifact_hash.rs`. It must load the manifest
   once, resolve all manifest card paths through `pack_io::resolve_pack_path`,
   enumerate the standard semantic directories, recognize the two explicit
   distribution classes, and return a typed unknown-classification error for
   everything else.
4. Define whether a missing optional `README.md`, `sources.yaml`, or `evals/`
   directory is simply absent (allowed for legacy packs) versus malformed. Do
   not add a new required file solely to compute an identity.
5. Add a checked-in synthetic classification corpus that names every class,
   an unknown file, an unsupported extension, a path collision, a symlink, and
   a generated-only mutation. The corpus must contain no private data.

### U2. Add dual hashing without changing legacy bytes

1. Add `pack_identity_snapshot(root)` returning one coherent
   `PackIdentitySnapshot` with semantic/distribution inventories and hashes.
   Retain `pack_content_snapshot` unchanged for old readers and migration
   comparisons.
2. Prove `distribution_sha256 == pack_content_sha256(root)` for every
   classifier-complete fixture. If a future distribution algorithm changes,
   change the identity contract explicitly rather than silently altering the
   old value.
3. Compute `semantic_authority_sha256` over sorted semantic records with the
   domain/version prefix. Reject an empty semantic set, duplicate/case-folded
   paths, unsafe logical paths, and any unknown file before returning the
   identity object.
4. Reuse the existing bounded file/path checks and hash each retained byte
   snapshot once. Do not parse README prose or use its generated inventory as
   semantic input.
5. Add unit and independent recomputation tests for path independence, byte
   mutation, record ordering, class membership, domain separation, and exact
   equality with legacy distribution hashes.

### U3. Make requirements and source lineage semantic-aware

1. Extend `requirements::pack_summary` with `identity_contract`,
   `semantic_authority_sha256`, and `distribution_sha256`; retain `sha256` as
   the documented legacy distribution alias so existing v1/v2 source-binding
   fixtures can still be read.
2. Preserve `requirements_sha256` as the legacy/full canonical requirements
   digest: hash the complete identity-aware object before adding that field, so
   old readers retain their exact meaning. Then derive a deterministic semantic
   requirements projection by removing `requirements_sha256` and the
   distribution-only `pack.sha256`/`pack.distribution_sha256` fields while
   retaining the pack identity contract, pack ID/version, and
   `pack.semantic_authority_sha256`; hash that projection with the explicit
   domain `mdp.requirements-semantic.v2` and emit
   `semantic_requirements_sha256` after the legacy field. This is the digest
   that remains stable for README/source-strategy-only distribution drift.
3. Extend v1/v2 source-binding pack and requirements schemas with an identity
   discriminator and optional semantic/distribution fields. New source-binding
   fixtures include all identity fields, set legacy `sha256` to the
   distribution alias, and pin `requirements.semantic_requirements_sha256`
   alongside the legacy/full `requirements.sha256` when the identity contract
   is present.
4. In `validate_source_binding_value` and `validate_source_binding_v2`, branch
   on `identity_contract`:
   - no identity fields: preserve exact legacy distribution checking and return
     a safe `legacy-portable` mode;
   - identity fields: require current pack semantic and
     semantic-requirements hashes, compare optional distribution/full
     requirements pins as drift evidence, and never use the old
     `sha256`/full requirements digest alone to decide semantic equality;
   - malformed or partial identity objects fail closed with a stable issue.
5. Keep source-binding `valid: true` for README/source-strategy-only drift when
   the semantic digest still matches, but expose a warning/compatibility state
   that exact-distribution consumers can require. A card/prompt/manifest/eval/
   source change remains an error.
6. Update `routing::fit_normalized` to use the compiled
   `semantic_requirements_sha256` (and pack semantic hash) in new
   `mdp.signal-qualification-authority.v1` projections. Keep the existing
   full `requirements_sha256`/`pack_sha256` fields as legacy evidence with an
   explicit identity mode, so route authority does not churn on orientation
   edits while exact-distribution consumers can still audit the full bytes.
7. Update the source-lineage version matrix documentation and v1/v2 tests to
   prove no mixed contract can smuggle an old distribution hash into a new
   semantic claim, and prove semantic requirements remain stable when only
   README/source-strategy bytes change.

### U4. Bind both identities into prompt, decision, run, and conformance authority

1. Extend prompt-output validation authority with the identity contract and
   both hashes. Make `validate_governed_artifact_authority` and
   `current_pack_prompt_binding_matches` require semantic equality for new
   artifacts, while preserving legacy `pack.sha256` verification for old
   artifacts.
2. Add optional identity fields to `PackAuthority` and populate them in
   `run_runtime::execute_transaction` from the staged snapshot. Keep
   `portable_digest` equal to distribution identity and keep `files` as the
   distribution inventory for v1 compatibility.
3. Compare one identity snapshot before staging, after staging, before output,
   and after driver completion. Any mutation of either source or staged bytes
   still yields `no-draft:audit-incomplete`; semantic equivalence does not
   weaken immutable-run snapshot integrity.
4. Update `run_verification::verify_run` and `verify_runner_audit` to validate
   the new fields, recompute identity from an available pack/file root, and
   distinguish `legacy-portable`, `identity-bound`, and
   `identity-recomputation-unavailable` without elevating assurance.
5. Extend `PackReleaseIdentity`, conformance candidate/job/report schemas, and
   `compile_candidate` so a new candidate binds semantic authority. A legacy
   candidate remains inspectable but its semantic identity is unbound and its
   assurance/report must say so.
6. Preserve domain-separated run/receipt hashes. Adding identity fields to a
   serialized v1 bundle changes that bundle's canonical hash as it should; old
   bundles continue to verify under their old bytes and contract rules.

### U5. Expose the identity clearly and keep all projections consistent

1. Make `mdp --json pack` include the closed identity payload (or an exact
   safe projection) alongside current card/prompt inventory. Human output and
   `--summary` must state: “semantic authority changes decisions; distribution
   identity changes when any shipped byte changes.”
2. Include pack semantic/distribution hashes, semantic requirements hash, and
   identity mode in `requirements`, routing/source-binding validation,
   prompt-output validation, trace, run, verify-run, and conformance summaries.
   Preserve the full legacy requirements digest as clearly labeled evidence.
   Exclude bodies, private values, and arbitrary user-controlled prose from all
   summaries.
3. Add `schema pack-identity-v2` and capability metadata for the classifier,
   hash algorithms, legacy read path, and stable refusal/drift diagnostics.
4. Update the README, CLI usage, architecture/run/release docs, and canonical
   MDP skills in one documentation pass. In particular, replace every current
   statement that README changes the *portable/authority* identity with the
   precise statement that README changes distribution identity only.
5. Keep `mdp readme check/refresh` semantics unchanged: the inventory block is
   a one-way projection and its drift is not a product-foundation authority.

### U6. Add migration, rollback, and release proof

1. Implement legacy readers that accept v1 portable snapshots, old
   `requirements.v1/v2`, old source bindings, old prompt validation artifacts,
   old run bundles/receipts, and old conformance candidates. The read result
   must carry a legacy mode or limitation; it must not synthesize a historical
   semantic binding.
2. Define the migration path as re-emission by the owning producer: rerun
   `requirements`, regenerate an integration-owned source binding, and create
   new prompt/run/conformance artifacts with `mdp.pack-identity.v2`. Do not
   mutate old receipts in place and do not overwrite old source-binding files
   silently.
3. Add a migration/read-verify command or documented `pack` identity path only
   if the existing CLI cannot expose the identity payload and legacy mode
   clearly. The minimum shipped path is `mdp --json pack` plus schema output,
   while all validators must read both modes. A new migration helper must be
   dry-run-first, exact-byte based, and integration-artifact only; it must never
   edit `.mdp` authority or claim a v1 receipt reached v2 assurance.
4. Add installed CLI/plugin smoke proof for a README-only change, an authority
   change, an unknown-file refusal, and an old artifact read. Confirm plugin
   assets and root assets remain identical.
5. Run the full repository validation and record any unavailable external gate
   separately from the plan artifact; do not claim installed/release proof from
   a source-only test.

## Versioning and migration contract

### Old artifacts

- `mdp.portable-pack-snapshot.v1` remains readable with its original `sha256`
  algorithm and generated-directory exclusion.
- `pack.sha256` in old requirements/source bindings and `portable_digest` in
  old run/conformance artifacts continue to mean the old portable/distribution
  value. They are not renamed in place and are not compared as semantic
  authority.
- Old artifacts may remain `valid` under their existing exact-byte checks. The
  result must expose `identity_mode: legacy-portable` (or the equivalent
  documented projection) and must say semantic authority was not bound.
- A verifier may compute the current semantic hash for diagnostics, but it
  must not assert that the old artifact committed to that value.

### New artifacts

- New pack/requirements output emits `mdp.pack-identity.v2` with the
  classification contract and both pack hashes. Requirements additionally
  emits `semantic_requirements_sha256`, a domain-separated hash of the
  semantic requirements projection; the existing `requirements_sha256`
  remains the full/legacy canonical requirements digest.
- New source bindings, prompt-output validation authority, run bundles, and
  conformance identities include `identity_contract` and
  `semantic_authority_sha256`; identity-aware source bindings also include
  `requirements.semantic_requirements_sha256`. `distribution_sha256` is
  included whenever the exact shipped bundle is available; the legacy hash
  fields remain aliases/evidence for wire compatibility.
- The semantic field is the only field that proves the decision authority
  stayed the same. `semantic_requirements_sha256` is the corresponding
  requirements/route authority proof. Distribution drift and the full
  `requirements_sha256` are separate observations and may be required by a
  release/exact-bundle check.
- Outer `mdp.requirements.v1/v2`, `mdp.source-binding.v1/v2`, and
  `mdp.run-bundle.v1` readers remain compatible through additive optional
  identity fields plus explicit discriminator validation. Do not silently
  repurpose a required legacy field. If implementation review determines that
  a closed contract cannot safely carry the additive shape, introduce a new
  outer contract (for example `requirements.v3`/`source-binding.v3`) and add
  an explicit lineage matrix; never loosen `additionalProperties: false`.

### Migration test matrix

| Fixture | Expected legacy read | Expected identity-aware read |
| --- | --- | --- |
| Unchanged v0.1.73 pack and old source binding | Exact old distribution match; legacy mode; no semantic claim. | New identity computes distribution equal to old `portable_digest`, emits semantic hash, and validates newly emitted binding. |
| README-only mutation | Old binding reports the existing distribution mismatch and needs re-emission; its legacy `requirements_sha256` may also differ because it covers the full compiled object. | New binding keeps `semantic_authority_sha256` and `semantic_requirements_sha256` stable, reports `pack_distribution_drift`/full-requirements drift if it retained old optional pins, and never reports semantic authority changed. |
| Card/manifest/prompt/eval/source mutation | Old binding fails old exact hash as before. | Semantic mismatch is a blocking diagnostic even when a distribution pin is absent or updated. |
| Generated `briefs`/`traces` mutation | Old and new identities are unchanged. | Existing exclusion and no-stale behavior remain green. |
| Unknown file | Legacy snapshot can be read only for historical integrity; no new identity is produced. | New identity and strict validate fail closed with the logical path and stable code. |
| Reordered directory traversal or copied pack root | Same old distribution and new semantic values. | Independent recomputation matches both inventories and hashes. |
| Old run receipt/conformance candidate | Read/verify under old contract; no semantic assurance. | Re-emission binds both values; tampering either field fails. |

## Golden fixtures and validation scenarios

The implementation should add a small, public-safe
`cli/tests/fixtures/pack-identity-v2/` corpus or an equivalent established
fixture location. Keep fixtures synthetic and deterministic. At minimum:

| Scenario | Required assertion |
| --- | --- |
| Basic and proposal template roots | Classification is complete; both templates have non-empty semantic authority and distribution inventories. |
| README prose change | `semantic_authority_sha256` and `semantic_requirements_sha256` stay equal; `distribution_sha256` and legacy/full `requirements_sha256` change; resolver/readiness/semantic route outputs remain equal. |
| README generated inventory change | Same as README prose; `readme_inventory_drift` remains a separate diagnostic and the full requirements digest is not mistaken for semantic authority. |
| `source-strategy.json` change | Semantic unchanged; distribution changed; source strategy is not loaded as MDP decision authority. |
| Manifest/card/prompt/eval/source mutation | Both hashes change; source binding, requirements, prompt-output, run/conformance authority becomes stale. |
| Unknown note or unsupported extension | New identity returns stable `pack_file_classification_unknown`; no partial digest is published. |
| Symlink, hard link/non-regular entry, traversal/case collision | Existing fail-closed path diagnostics remain; no class exclusion bypasses them. |
| Generated local output | Neither hash changes; old generated-directory tests remain green. |
| Independent recomputation | A shell/Node/Rust fixture helper recomputes semantic and distribution records from final bytes and matches the CLI payload. |
| Legacy portable snapshot | Old digest exactly equals new `distribution_sha256`; old artifacts remain readable and are marked legacy. |
| Identity field tampering | Any semantic or classification field change is rejected by schema/verification; a distribution-only mismatch is distinguishable from semantic mismatch. |
| Cross-surface parity | `pack`, `requirements`, source-binding validation, routing authority, prompt-output validation, trace, run/verify-run, and conformance expose the same pack semantic/distribution values and the same semantic requirements value for one root. |

The current `cli/src/commands/init.rs` test
`readme_is_non_authoritative_but_part_of_portable_identity` should become a
two-identity characterization test rather than being deleted: it must assert
README distribution drift and semantic stability. The existing
`artifact_hash.rs` path/byte/generated tests remain and gain semantic-record
assertions. `scripts/test_skill_contracts.py` and any Node conformance fixture
that currently expects only `portable_digest` must assert legacy compatibility
and the new semantic field separately.

## Acceptance-criteria mapping

| MDP-236 acceptance criterion | Planned proof and owner |
| --- | --- |
| Normative file-classification contract identifies semantic-authority and orientation/distribution files | U1 adds `mdp.pack-file-classification.v1`, the explicit class table, closed schema/capability metadata, and unknown-file fail-closed tests. |
| Decision/run receipts bind semantic digest and, where available, full bundle digest | U4 extends `PackAuthority`, prompt/decision authority, conformance release identity, runtime staging, and verifier paths with semantic plus distribution fields; U5 proves cross-surface parity. |
| README-only prose changes bundle identity without falsely claiming changed decision authority | U2/U3 golden mutation fixture holds pack semantic and `semantic_requirements_sha256` plus readiness/route results stable while changing distribution and full requirements hashes; U3 validators report drift separately from semantic mismatch. |
| Manifest, cards, prompts, schemas/foundation/evals/source authority changes alter both appropriate identities | U1 classifies all current authority directories; U2 mutation matrix and U3/U4 stale-binding tests require both changes and semantic rejection. |
| Unknown/unclassified files fail closed or receive explicit documented classification | U1 names every shipped current class, recognizes only those paths, and tests unknown files, extensions, symlinks, and collisions with stable diagnostics. |
| MDP-154 release/API and MDP-179 assurance semantics are updated | U5 updates `docs/run-receipts.md`, runtime/release architecture docs, conformance schemas, and MDP-154/179 contract language: semantic authority is required for strongest claims; distribution is exact bundle integrity. |
| Existing portable digests have versioned migration/read-verify path | U2 keeps v1 portable hashing; U6 adds v2 identity output, legacy readers, explicit re-emission migration, no retroactive assurance, and the old/new fixture matrix. |
| Golden fixtures cover orientation-only, authority, generated, and unknown-file changes | U2 and the `pack-identity-v2` corpus cover each class; `make validate`, installed smoke, and independent recomputation prove the final bytes. |
| CLI explains both identities in plain language | U5 updates `commands/pack.rs`, `output.rs`, capabilities/schema output, `README.md`, `cli/USAGE.md`, and operator skills with the semantic-vs-distribution explanation. |

## Dependencies and execution ordering

- **MDP-217 (shipped prerequisite):** Reuse its owned README inventory markers
  and one-way projection. Do not parse README prose as authority; preserve
  legacy README-without-marker compatibility.
- **MDP-154 (related release/API decision):** Update pack-release and hosted
  decision-bundle language so release identity can expose both semantic and
  distribution identity. No hosted endpoint or cloud mutation is in scope for
  this implementation.
- **MDP-179 (shipped run/assurance contract):** Preserve v1 run bundle/receipt
  compatibility and explicit assurance dimensions. Semantic identity is a
  required input to the strongest pack-authority claim; old portable receipts
  remain lower-assurance legacy evidence.
- **MDP-239 (parent execution index):** This issue remains a research child in
  the parent queue. The plan does not change parent ordering, blocker/related
  relations, delegation, labels, or readiness state.
- **MDP-226/227/230/231/237:** Keep their staged context, synthetic-chain,
  runtime-observation, diagnostics, and sealed-request behavior compatible. A
  README-only identity drift must not reopen or mask those gates.
- **Release/install proof:** Any implementation that changes CLI/plugin
  contracts must run the repository's installed release smoke and asset parity
  checks after focused Rust/Node/schema tests. This plan branch itself has no
  runtime or release delta.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| A new classifier accidentally excludes a decision file | Resolve manifest card paths explicitly, classify all current semantic directories, fail unknown files closed, and assert both identities in mutation tests. |
| README drift is treated as a semantic mismatch | Use the identity discriminator and compare semantic first; distribution drift is a separate warning/strict exact-bundle check. Add the MDP-217 README mutation regression. |
| `portable_digest` is silently reinterpreted | Keep v1 function/field semantics and old schemas readable; document the distribution alias and require an explicit identity contract before semantic claims. |
| Distribution-only integration files are omitted | Classify `source-strategy.json` explicitly and include it in distribution inventory. Unknown future integration files fail until added to the reviewed table. |
| Outer `v1` contracts become ambiguous | Use additive optional fields with conditional schema tests and explicit `identity_contract`; if that cannot remain closed, add a new outer contract and a lineage matrix instead of weakening schemas. |
| Different consumers calculate different hashes or route authority churns on full requirements bytes | Keep hashing in `artifact_hash.rs`; pass one snapshot through requirements, routing, runtime, trace, and conformance. Derive `semantic_requirements_sha256` once from the documented projection, and add independent recomputation and cross-surface equality tests. |
| Source bindings still block on the legacy alias | In identity-aware validation, do not compare legacy `sha256` as the semantic gate; treat an optional old distribution pin as drift evidence and require only semantic equality for decision authority. |
| Legacy receipts are overstated as new assurance | Preserve legacy mode/limitations and require semantic identity for new strongest-tier output. A verifier never reconstructs historical semantic provenance. |
| Unknown files leak private names or bodies | Stable diagnostics contain only the safe logical path, class/error code, and bounded contract metadata; never include file bytes, README prose, source values, or credentials. |
| Receipt size and schema churn | Keep semantic file inventory in the pack identity projection; receipts carry the two hashes and existing distribution inventory unless an exact release contract explicitly needs more. Use serde defaults and closed schemas. |
| Runtime mutation checks are weakened by semantic equivalence | Continue comparing the full staged/source distribution snapshot before and after invocation. Semantic equality never authorizes a changed immutable snapshot. |

## Compatibility and rollback

### Compatibility rules

- Existing packs with only the current files continue to load. Their new
  distribution hash equals the old portable hash, and their new semantic hash
  is additive metadata.
- Existing old source bindings, prompt validation results, run bundles, and
  conformance candidates remain readable under their original contract. They
  retain old exact-byte behavior and are explicitly marked legacy/unbound for
  semantic authority.
- New identity-aware source bindings remain decision-valid across
  orientation/distribution-only changes when the semantic digest is unchanged,
  and `semantic_requirements_sha256` remains unchanged, while exact bundle
  consumers can require matching distribution/full-requirements pins.
- Authority changes continue to fail closed. No migration path may downgrade a
  semantic mismatch to a distribution warning.
- Generated `.mdp/briefs` and `.mdp/traces` remain excluded from both identities,
  preserving the existing trace/brief non-staling contract.

### Rollback plan

1. If implementation validation finds a compatibility defect, stop emitting
   identity-aware fields but keep the legacy readers and old `portable_digest`
   behavior intact. Do not delete or rewrite old artifacts.
2. If a released identity-aware artifact must be checked after rollback, the
   v1 reader can still verify its legacy distribution alias and report the
   semantic fields as unsupported/unknown; it must not claim the stronger
   semantic tier.
3. If the classifier table is wrong, remove only the new identity-aware
   producer path or correct the reviewed class row. Never add a catch-all
   “distribution-only” fallback and never raise an allowlist to make unknown
   files pass.
4. Re-run the unchanged-template, legacy-read, generated-output, README-only,
   authority-mutation, schema, and installed smoke suites. Compare the legacy
   distribution digest against the pre-change v1 golden value.

## Validation contract

### Plan-branch validation

This branch changes one tracked Markdown plan only. Before commit:

```sh
git diff --check
git status --short --branch
git diff --stat
```

Run the repository validation appropriate to the exact changed tree:

```sh
make validate
```

If an external dependency gate is unavailable, record the exact command and
failure in the handoff; do not replace it with a fabricated green result.

### Implementation validation required by this plan

```sh
git diff --check
(cd cli && cargo fmt --check)
(cd cli && cargo test artifact_hash)
(cd cli && cargo test commands::source_binding)
(cd cli && cargo test commands::requirements)
(cd cli && cargo test commands::prompt_output)
(cd cli && cargo test commands::decision_trace)
(cd cli && cargo test commands::run_verification)
(cd cli && cargo test conformance)
(cd cli && cargo test commands::schemas)
node scripts/test-run-v1-golden.mjs
node scripts/test-run-conformance.mjs
node scripts/test-cold-model-conformance.mjs
python3 -m unittest scripts/test_skill_contracts.py
make validate
```

Add focused filters for the identity corpus, README-only drift, unknown-file
refusal, legacy read/verify, source-binding compatibility mode, run-bundle
semantic/distribution recomputation, and independent hash recomputation. If
CLI/plugin/release assets change, also run the installed release smoke,
`make validate-asset-sync`, and the repository's public-artifact/privacy gates.

## Definition of done

- `mdp.pack-file-classification.v1` and `mdp.pack-identity.v2` are normative,
  closed, schema-advertised, capability-advertised contracts.
- One Rust classifier and one coherent snapshot implementation produce the
  semantic and distribution inventories/hashes; unknown files fail closed.
- The distribution hash remains equal to the old portable hash for complete
  current packs; the semantic hash excludes only explicitly documented
  orientation/distribution-only files and generated local outputs.
- New requirements, source bindings, prompt validation, traces, run bundles,
  receipts, routing authority, and conformance artifacts bind semantic authority
  and expose exact distribution identity when available. Requirements and
  signal authority also bind the semantic requirements projection digest.
- Old artifacts remain readable/verifiable at legacy assurance, with no
  retroactive semantic claim or silent field reinterpretation.
- README-only and source-strategy-only changes alter distribution identity but
  do not alter semantic requirements/readiness/decision results; their full
  legacy requirements digest may change but is not used as the semantic gate.
  Authority mutations alter both identities and block stale semantic bindings.
- CLI and canonical skills explain the distinction in plain language without
  exposing raw pack content, provider data, or private values.
- Golden fixtures cover orientation-only, authority, generated, unknown,
  migration, path-safety, and independent-recomputation cases.
- Focused tests, `make validate`, and installed artifact proof pass for the
  implementation commit. This plan branch itself contains only the tracked
  plan artifact and does not claim implementation/release completion.
