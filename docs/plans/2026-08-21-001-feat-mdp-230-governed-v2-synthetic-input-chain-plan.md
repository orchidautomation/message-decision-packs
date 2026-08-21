---
title: MDP-230 Governed v2 Synthetic Input Chain - Plan
type: feat
date: 2026-08-21
execution: code
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-230
linear_issue: MDP-230
parent_linear_issue: MDP-239
related_linear_issues:
  - MDP-225
  - MDP-149
  - MDP-167
  - MDP-198
repository: orchidautomation/message-decision-packs
base_ref: origin/main
base_commit: 2cba9919483b5a7ba46efed53e3b5502b2abf477
---

# MDP-230 Governed v2 Synthetic Input Chain - Plan

## Goal Capsule

| Field | Decision |
| --- | --- |
| Objective | Add one deterministic, offline CLI path that creates or safely rebinds a complete synthetic v2 source-binding, source-attempt-request, collected-attempt-results, and normalized-decision-input chain for one exact pack root and canonical job ID. |
| Product shape | Add `rebind-synthetic-chain` with explicit dry-run, apply, force, output-directory, and deterministic timestamp/seed controls. It emits four public-safe JSON artifacts plus a machine-readable write/validation report. |
| Authority boundary | The MDP pack and compiled `requirements --job` result own contract identity, job scope, schemas, applicability, and exact pack/requirements digests. MDP owns synthetic construction, lineage hashes, validation, and safe local writes; it does not collect evidence, call providers/models, or rebind real/customer evidence. |
| Compatibility | Additive to the v2 validators, `sample-leads`, v1 jobs, and the existing manually authored integration-fixture flow. Existing commands and existing files retain their current semantics. |
| Public safety | Fresh artifacts use only `synthetic_fixture`, opaque non-URL locators, stable example values, explicit `synthetic: true`, and `do_not_contact: true` where the normalized prospect contract supports it. Rebinding refuses real, private, customer, provider, or ambiguous provenance. |
| Execution state | This is an implementation-ready plan artifact only. MDP-230 remains Backlog/`phase:planned` under the MDP-239 queue; implementation is gated by the parent execution index and is not authorized by this plan commit. |
| Dependency preservation | MDP-225 is the shipped downstream signal-consumer fix. MDP-230 remains a declared prerequisite/blocker for MDP-237, while MDP-226 and MDP-231 remain separate runtime-boundary work. No issue status, dependency relation, delegation, or automation branding changes are part of this handoff. |

## Product Contract

### Problem frame

The v2 chain is valid when its four files are authored consistently, but a
pack or compiled-requirements change invalidates the pack and requirements
receipts, the source-binding hash, the request hash, the collected-results
hash, the normalized-output hashes, and every nested signal-observation
receipt. Today a host must repair those values by hand. The existing
`sample-leads` command creates safe prospect rows but does not create a
job-bound, signal-aware lineage chain that the existing validators can consume.

MDP-230 supplies deterministic fixture scaffolding and a safe rebind path. It
must make exact-byte lineage easier to produce without turning synthetic
values into evidence or moving collection/model execution into MDP.

### CLI contract

Add a command following the existing clap and checked-output conventions:

```text
mdp --json rebind-synthetic-chain \
  --dir PACK_ROOT \
  --job CANONICAL_JOB_ID \
  --out-dir OUTPUT_DIR \
  [--input-dir EXISTING_SYNTHETIC_CHAIN_DIR] \
  [--as-of 2026-01-01T00:00:00Z] \
  [--seed 0] \
  [--dry-run] \
  [--apply] \
  [--force]
```

- `--dir`, `--job`, and `--out-dir` are required. The job must resolve to a
  signal-aware `mdp.requirements.v2` contract; a scalar/v1 job fails with a
  stable `synthetic_chain_v2_required` diagnostic.
- Without `--input-dir`, the command builds a fresh chain from the selected
  job's compiled requirements. With `--input-dir`, it reads exactly these
  conventional files as one candidate chain:

  ```text
  source-binding.json
  source-attempt-request.json
  collected-attempt-results.json
  normalized-input.json
  ```

- The default is a report-only dry run. Only `--apply` may create or replace
  destination files. `--apply` without `--force` refuses a changed existing
  file. `--force` is valid only with `--apply` and must create a recoverable
  digest-keyed backup before replacement.
- An existing file whose candidate bytes are identical is reported
  `unchanged` and is never rewritten. Dry-run must not create the output
  directory, destination files, or backups.
- `OUTPUT_DIR` must resolve outside the active pack's `.mdp` tree. The command
  must not change the pack digest or mutate manifests, prompts, cards,
  requirements, source ledgers, or input files in place.

### Result and artifact contracts

Introduce `mdp.synthetic-v2-chain.v1` for the command result. It contains:

- exact pack ID/version/content digest and compiled requirements contract/digest;
- canonical job ID and deterministic inputs (`as_of`, `seed`, fresh versus
  rebind mode);
- the four output paths, byte counts, exact emitted-byte SHA-256 digests, and
  per-file action (`create`, `unchanged`, `blocked`, or
  `overwrite-with-backup`);
- planned backup paths where applicable;
- staged `validate-source-binding` and bound `validate-prompt-output` results;
- a stable refusal or write-conflict code when the operation cannot proceed.

Expose the result schema through `mdp --json schema synthetic-v2-chain` (or an
equivalent explicit schema target) and register the command, flags, side
effects, boundaries, and stable diagnostics in `mdp --json capabilities`.

The four files retain the existing v2 contracts. Serialize each candidate
exactly once using the repository's pretty-JSON-plus-trailing-newline format;
hash those final bytes with `artifact_hash::sha256_hex`; then feed the exact
hash into the next dependent artifact. Continue using
`canonical_json_sha256` for the existing requirements digest contract only;
never substitute a canonical value hash for a required emitted-file hash.

### Synthetic provenance contract

Fresh generation must use `synthetic_fixture` at every source-binding,
attempt, result, and signal-observation boundary. Every locator and upstream
reference is an opaque, non-URL identifier derived from the job, qualified
attribute/projection ID, and seed. Generated normalized values must satisfy
the selected value contract and use example-safe names/domains/labels.

Rebinding is allowed only when all source classes and provenance entries are
explicitly synthetic, all locators remain opaque and non-URL, the normalized
envelope is explicitly synthetic, and no private path, customer identifier,
provider record, credential, raw source value, or ambiguous source class is
present. The command fails closed rather than downgrading or rewriting an
ambiguous source to `synthetic_fixture`. Accepted rebinds preserve semantic
values/statuses and repair only the current pack/requirements pins and the
dependent exact-byte hash graph.

### Scope boundaries

**Included**

- v2-only job compilation and deterministic values for declared scalar
  attributes and signal projections;
- one source binding for every compiled signal projection;
- one attempt for each compiled attribute, with additional deterministic
  contributor attempts where a projection requires them;
- exact-byte hash propagation through source binding, request, results,
  normalized output, and nested signal receipts;
- staged validation through the existing source-binding and prompt-output
  validators;
- dry-run diff/hash/write planning, apply/force/backup behavior, and no-change
  replay;
- CLI capabilities, schema discovery, focused tests, public-safe example proof,
  and operator documentation.

**Out of scope**

- v1 chain generation or changes to v1 behavior;
- source collection, provider calls, credentials, enrichment, scraping, model
  calls, normalization execution, CRM writes, or hosted APIs;
- rebinding real, customer, private, reviewed-internal, public-web, or unknown
  evidence;
- changing routed-context acceptance (MDP-226), observed driver configuration
  binding (MDP-231), or sealed request compilation (MDP-237);
- changing the semantic authority of signal observations or treating synthetic
  lineage as source truth;
- automatically editing pack files, manifests, prompts, or source ledgers.

## Acceptance Examples

- **AC1 — Exact-job complete chain.** Given the canonical public-safe Clay
  pack and `prospect-fit-or-brief`, the command emits all four files. The
  source binding covers every compiled projection; request, results, normalized
  output, and observations agree on job, contract, and lineage hashes. A v1
  job is rejected with a stable v2-required diagnostic.
- **AC2 — Exact emitted-byte hashes.** Every downstream hash equals SHA-256 of
  the final bytes written for its upstream file. Reordering, indentation, or a
  newline mutation changes the hash and causes validation/replay failure until
  the chain is rebuilt. All nested observation receipts use the same exact
  source/request/results tuple.
- **AC3 — Existing validators pass before write.** Before an apply result is
  reported successful, staged candidate bytes pass the actual
  `validate-source-binding` and bound strict `validate-prompt-output` paths.
  If either fails, no destination write occurs and the result identifies the
  bounded failure.
- **AC4 — Real or ambiguous provenance is refused.** Any non-synthetic source
  class, URL locator, missing synthetic marker, private-looking path,
  customer/provider record, or inconsistent provenance blocks before any
  destination write.
- **AC5 — Dry run is non-mutating.** Default dry-run reports all create,
  unchanged, blocked, and force-required actions plus old/new byte counts,
  hashes, and planned backup locations. It creates no output directory, file,
  or backup and leaves the pack digest unchanged.
- **AC6 — Force is explicit and recoverable.** Apply creates missing files.
  Apply without force refuses changed existing files using the stable
  write-conflict contract. Apply plus force backs up each prior byte sequence
  before atomic replacement; an interrupted operation leaves the previous
  destination chain intact.
- **AC7 — Exact replay is a no-change operation.** Repeating the same pack,
  job, `as_of`, seed, and accepted synthetic input produces identical bytes,
  reports `unchanged`, performs no writes, and creates no backups.
- **AC8 — Public-safe operator path.** The public example documents and proves
  deterministic fit → brief → routed-context → clean-run preparation. It
  contains no credentials, private paths, customer data, raw provider records,
  or real contact values, and labels synthetic lineage as fixture scaffolding
  rather than source truth.

## Planning Contract

### Key technical decisions

1. **Add a dedicated command module.** Do not overload `sample-leads`; keep
   `mdp.sample-leads.v0` output and tests stable. Extract a helper only when it
   avoids duplicate safe-value recipes without changing the existing contract.
2. **Build in dependency order.** Serialize source binding first, then request,
   results, and normalized output. Do not create four independent JSON values
   and patch hashes afterward. Each downstream object receives hashes from the
   final bytes of its upstream object.
3. **Reuse existing requirements and validators.** Expose only the smallest
   crate-private compiled-job/schema seams from `requirements.rs`. Reuse
   `validate_source_binding_file`/`validate_source_binding_v2` and
   `validate_prompt_output_file_with_lineage_inputs`; do not create a second
   source-binding, signal-observation, or prompt-output validator.
4. **Derive values from pack-owned contracts.** Select only declared enum/type
   values, satisfy conditional applicability deterministically, and preserve
   status semantics. If a format or dependency cannot be synthesized safely,
   return a stable refusal instead of inventing a value.
5. **Stage before write.** Build and validate all four final bytes in an
   OS-temporary staging root before planning destination writes. Dry-run must
   stop at the plan. Apply uses exact ownership, digest-keyed backups, and
   atomic replacement; it never performs an implicit cleanup migration.
6. **Keep generated artifacts outside `.mdp`.** This preserves
   `pack_content_sha256` and the existing integration-owned artifact boundary.
   Any future in-pack exception requires a separate contract and issue.
7. **Keep readiness separate from delegation.** This plan is implementation
   ready, but MDP-230 remains planned under MDP-239. It does not restore
   `phase:ready-for-agent`, native delegation, `delegate:blocks`, or any
   automation/autofix branding.

### High-level technical design

```text
exact pack root + canonical job
              |
              v
requirements(root, job)
              |
      require signal-aware v2
              |
       SyntheticRecipe
       /       |       \
      v        v        v
source binding  request/results  normalized envelope
all projections  all attempts    attrs + observations
       |             |             |
       +------ exact emitted-byte SHA-256 ------+
              |
              v
  stage -> source-binding validation -> prompt-output validation
              |
       dry-run report / apply transaction
```

Fresh generation derives every value from compiled contracts and a fixed
`as_of`/seed. Rebinding starts from an accepted synthetic chain, checks
synthetic-only provenance, updates current pack/requirements pins, and rebuilds
the dependent hashes in the same order. Both paths share one staging and
validation gate.

### Exact files, symbols, and responsibilities

| Area | File | Existing symbol or planned symbol | Responsibility |
| --- | --- | --- | --- |
| CLI model | `cli/src/cli.rs` | `Commands`; `SchemaTarget` | Add `RebindSyntheticChain` flags and the `synthetic-v2-chain` schema target with clap conflicts/requires for `--apply`, `--dry-run`, and `--force`. |
| Dispatch | `cli/src/app.rs` | `run`; `attach_dry_run_artifact`; `print_checked` | Invoke the builder, preserve checked exit semantics, attach output/write-plan metadata, and avoid writes unless apply is explicit. |
| Module wiring | `cli/src/commands/mod.rs` | command declarations/re-exports | Register `synthetic_chain` and its public(crate) command entry point. |
| Builder | `cli/src/commands/synthetic_chain.rs` | new `SyntheticChainPlan`, `SyntheticChainArtifacts`, `build_chain`, `rebind_chain`, `stage_and_validate`, `plan_writes`, `apply_writes` | Own v2 guard, deterministic recipe, provenance policy, dependency-ordered serialization, validation, diff planning, backup, and transaction reporting. |
| Requirements | `cli/src/commands/requirements.rs` | `requirements`, `resolve_job_decision_inputs`, `source_attempt_request_schema_v2`, `collected_attempt_results_schema`, `normalized_envelope_schema`, `validate_normalized_decision_input_with_projection` | Expose the selected compiled job/contracts and exact schemas to the builder without changing the existing requirements result or validator behavior. |
| Models | `cli/src/models.rs` | `DecisionInputContract`, `DecisionInputAttribute`, `DecisionInputSignalProjection`, `DecisionInputAttemptStatus`, `DecisionInputSourceClass` | Supply typed requirement, applicability, status, source-class, value, and signal metadata for recipe derivation. |
| Source binding | `cli/src/commands/source_binding.rs` | `validate_source_binding_file`, `validate_source_binding_v2`, `source_binding_schema_v2`, `source_lineage_version_matrix` | Reuse exact v2 projection coverage and provenance constraints; expose no weaker synthetic-only path. |
| Prompt output | `cli/src/commands/prompt_output.rs` | `validate_prompt_output_file_with_lineage_inputs` | Validate staged normalized output against the canonical prompt, source binding, request, results, job, schema, and hashes before any apply. |
| Schemas | `cli/src/commands/schemas.rs` | `schema`; `signal_observation_v2_schema` | Reuse observation schema and add the closed result schema if the new `SchemaTarget` is introduced. |
| Hashing | `cli/src/artifact_hash.rs` | `sha256_hex`; `canonical_json_sha256`; `canonical_json_bytes` | Hash final emitted bytes for lineage and use canonical requirements hash only where the existing requirements contract requires it. |
| File safety | `cli/src/pack_io.rs` | `write_json_file`; `planned_json_write_after_dirs`; planned byte/backup helpers | Add byte-oriented staged/atomic replacement and recoverable backup helpers without changing existing command write semantics. |
| Capabilities | `cli/src/commands/capabilities.rs` | `capabilities`; `command`; stable error-code list | Register output contract, command flags, offline/no-model boundary, writes, dry-run behavior, and refusal/conflict codes. |
| Summary | `cli/src/output.rs` | `summarize`; write-plan/error classification | Project concise action, validation, digest, and backup information without raw source bodies or private values. |
| Safe fixture reuse | `cli/src/commands/sample_leads.rs` | `sample_leads`; `fixture_lead` | Reuse or extract only deterministic public-safe value helpers; preserve `synthetic-example` and v0 output unchanged. |
| Public proof | `examples/clay-audiences-self-serve-enterprise-expansion/README.md`; `fixtures/*.json` | current v2 fixture workflow | Document generated-chain usage and keep any checked-in fixture synthetic, opaque, and validator-ready. |
| Operator docs | `cli/USAGE.md`; `docs/decision-input-contracts.md`; `docs/getting-started.md` | current v2/manual lineage sections | Document the command, exact-byte hash rule, synthetic-only rebind boundary, dry-run/apply/force, and fit → brief → routed-context handoff. |
| Integration proof | `scripts/test-run-conformance.mjs`; `scripts/release-install-smoke.sh` | v2 run fixture setup and installed CLI/plugin smoke | Exercise the generated chain through the existing validator/run paths without provider calls; update v2 paths only where the new public command is part of installed proof. |

### Ordered implementation steps

1. Compile `requirements(root, job)` and confirm the exact job is available,
   signal-aware, v2, and not blocked by profile/foundation/model-step issues.
   Enumerate contracts, attributes, output paths, applicability predicates,
   source classes, freshness/confidence/provenance policies, projections, and
   all versioned schemas before planning writes.
2. Define the result contract, conventional filenames, deterministic defaults,
   and output containment policy. Resolve/canonicalize pack and output paths;
   reject output inside `.mdp`, pack root, input chain, or any authored source
   location before creating parents.
3. Build a `SyntheticRecipe` from declared contracts. Select safe enum/type/
   format values, use `not_applicable` only when declared predicates prove it,
   preserve required/hard-gate status behavior, and refuse unsupported formats,
   cycles, or value contracts with a stable issue code.
4. Generate `mdp.source-binding.v2` with exact pack ID/version/digest,
   requirements contract/digest, contract pins, binding/normalization release,
   synthetic adapter/transformation IDs, and one projection binding for every
   compiled projection. Use only opaque non-URL upstream references.
5. Serialize source binding to final bytes and compute its raw SHA-256. Build
   `mdp.source-attempt-request.v2` with the exact binding hash, qualified
   contract/attribute identities, trusted `as_of`, one deterministic attempt
   per attribute, and additional attempts for declared projection contributors.
   Serialize once and compute the request raw hash.
6. Build `mdp.collected-attempt-results.v2` with the exact request and binding
   hashes, immutable result metadata/value/status/provenance/confidence/
   freshness, and one result per attempt. Serialize once and compute the
   results raw hash.
7. Build `mdp.normalized-decision-input.v2` with prompt identity, exact
   request/results/binding hashes, values projected to declared output paths,
   and one valid signal observation per generated projection as allowed by
   cardinality. Populate every observation receipt from the same raw hash
   tuple. Serialize once; do not put the normalized-output hash inside the
   output being hashed.
8. For `--input-dir`, parse exactly the four files, enforce synthetic-only
   provenance and job/contract identity, preserve accepted semantic values and
   statuses, update current pack/requirements pins, and rebuild dependent
   hashes. Refuse malformed, ambiguous, mixed-version, real, private, or URL
   provenance before candidate write planning.
9. Stage all four final bytes in a unique temporary root. Run the real
   `validate-source-binding` and strict bound `validate-prompt-output` paths
   against those exact staged files and the selected canonical prompt. Abort
   before destination planning if either validator is invalid.
10. Produce per-file dry-run actions by comparing candidate bytes with
    destination bytes. In apply mode create only missing parents/files; for
    changed files require force, create a non-colliding digest-keyed backup,
    then atomically replace. On any staging/backup/replacement failure retain
    the previous destination chain and report the bounded failure.
11. Add focused unit/integration tests and the canonical public example proof:
    exact replay, one-byte drift, pack/requirements drift, nested receipt
    drift, missing/duplicate projection, non-synthetic/URL/private refusal,
    dry-run no-write, conflict without force, force backup, interrupted
    transaction, v1 rejection, and pack-digest stability.
12. Update capabilities, schema discovery, CLI usage, decision-input docs, and
    the public example. Run focused checks, then repository validation and
    installed-artifact smoke as applicable. Inspect the final diff for plan
    scope, private-data leaks, in-pack writes, and accidental changes to v1 or
    MDP-239 metadata.

## Implementation Units

### U1. Add the explicit command and result contract

- **Goal:** Make the operation discoverable, bounded, and impossible to invoke
  with ambiguous write-mode flags.
- **Primary files/symbols:** `cli/src/cli.rs::Commands` and
  `SchemaTarget`; `cli/src/app.rs::run`; `cli/src/commands/mod.rs`;
  `cli/src/commands/capabilities.rs::capabilities`; `cli/src/output.rs`.
- **Steps:** Add command arguments and clap constraints; add the result
  contract/schema target; dispatch to a crate-private builder; preserve checked
  JSON/summary output and stable nonzero behavior; register side effects,
  dry-run, output flags, and refusal/conflict codes.
- **Tests:** parser accepts exact command and rejects `--force` without
  `--apply`, mutually conflicting dry-run/apply, missing pack/job/output, and
  invalid paths; capabilities and `schema synthetic-v2-chain` expose the same
  contract/flags; result summary omits bodies and private paths where required.
- **Dependencies:** None beyond current v2 command/schema contracts.

### U2. Compile deterministic synthetic v2 values and source binding

- **Goal:** Derive a complete synthetic projection map from one exact compiled
  job without duplicating policy authority.
- **Primary files/symbols:** new `cli/src/commands/synthetic_chain.rs`;
  `requirements.rs::requirements`, `resolve_job_decision_inputs`, and v2
  schema helpers; `models.rs` decision-input types;
  `source_binding.rs::validate_source_binding_v2` and schema helpers;
  `sample_leads.rs` safe value helpers only if extraction is justified.
- **Steps:** Require available `mdp.requirements.v2`; construct a deterministic
  recipe keyed by qualified contract/attribute/projection ID; select only
  declared values; use synthetic source classes/locators; emit one binding per
  projection; and stage the candidate before validation.
- **Tests:** valid Clay-like signal-aware job; scalar/v1 rejection; all
  projections present exactly once; missing/duplicate/unknown projection;
  source-class incompatibility; unsupported value format; conditional
  applicability; stable output for repeated seed/as-of; no pack mutation.
- **Dependencies:** Existing v2 source-binding/schema behavior; MDP-225's
  downstream signal projection semantics remain unchanged.

### U3. Build the dependent request, results, and normalized envelope

- **Goal:** Produce validator-ready exact-byte lineage with complete attempts
  and nested observation receipts.
- **Primary files/symbols:** new builder module; private-to-crate seams in
  `requirements.rs::source_attempt_request_schema_v2`,
  `collected_attempt_results_schema`, `normalized_envelope_schema`, and
  `validate_normalized_decision_input_with_projection`; reuse
  `schemas.rs::signal_observation_v2_schema` and
  `artifact_hash.rs::sha256_hex`.
- **Steps:** Serialize/hash in dependency order; emit one attempt for every
  declared attribute plus required contributor attempts; copy result status,
  provenance, confidence, freshness, and error fields exactly into normalized
  attributes; project typed observations according to cardinality and conflict
  policy; set `draft_allowed: false` and preserve no-draft semantics.
- **Tests:** attempted-complete coverage; every attribute represented; request
  and results bind the exact prior bytes; normalized values preserve contract
  paths; observations map to valid contributor attempts; duplicate agreement
  remains inspectable; conflict stays human-review/no-draft; stale/weak/missing
  receipts fail; one-byte and newline mutations change the expected hashes.
- **Dependencies:** U2; existing prompt-output/requirements validator behavior.

### U4. Add synthetic-only rebind and safe write transaction

- **Goal:** Make rebind useful for clearly synthetic fixtures without enabling
  provenance laundering or destructive overwrites.
- **Primary files/symbols:** new builder module; `cli/src/pack_io.rs::write_json_file`,
  `planned_json_write_after_dirs`, and new byte/backup/atomic helpers;
  `artifact_hash.rs::sha256_hex`; existing path containment patterns such as
  `source_binding.rs::binding_is_inside_pack`.
- **Steps:** Validate input directory identity and all four files; scan source
  class/locator/synthetic markers and reject unsafe values; preserve semantic
  data while repairing only current pins/hashes; stage and validate before
  destination writes; report old/new bytes/digests; require apply/force;
  backup changed files by prior digest before replacement; leave prior files
  untouched on partial failure.
- **Tests:** non-synthetic source class, URL locator, private-looking path,
  missing marker, mixed versions, changed output without force, force backup,
  backup collision, interrupted replacement, dry-run no-write, same-byte
  replay, output-inside-pack refusal, and pack digest unchanged.
- **Dependencies:** Existing integration-owned outside-pack boundary; no
  cleanup or migration command.

### U5. Wire validator/capability/installed proof

- **Goal:** Prove the generated chain is consumed by the real v2 validators and
  installed CLI/plugin surface without provider execution.
- **Primary files/symbols:** `cli/src/commands/source_binding.rs::validate_source_binding_file`;
  `cli/src/commands/prompt_output.rs::validate_prompt_output_file_with_lineage_inputs`;
  `cli/src/commands/capabilities.rs`; `scripts/test-run-conformance.mjs`;
  `scripts/release-install-smoke.sh` where installed v2 coverage is added.
- **Steps:** Invoke the same validators used by documented CLI commands on
  staged bytes; expose result validation receipts; add one source-tree
  conformance case and one installed CLI/plugin case; compare canonical
  contract, status, lineage hashes, and no-draft behavior rather than process
  success alone.
- **Tests:** `node --check`/test for changed scripts; real generated chain
  through `validate-source-binding`, strict `validate-prompt-output`, `fit`,
  and the documented brief/routed-context preparation; installed smoke when
  release assets are available. No provider/key/model call.
- **Dependencies:** U1-U4; MDP-226/231/237 remain separate and are not altered.

### U6. Update docs, public fixture proof, and review handoff

- **Goal:** Make ownership, provenance, and the deterministic operator path
  understandable and durable.
- **Primary files/symbols:** `examples/clay-audiences-self-serve-enterprise-expansion/README.md`;
  `cli/USAGE.md`; `docs/decision-input-contracts.md`; `docs/getting-started.md`;
  `README.md` only if command inventory requires it; relevant synthetic eval
  or fixture files.
- **Steps:** Document fresh versus rebind modes, exact-byte hashes, dry-run/
  apply/force/backup behavior, external output roots, synthetic-not-source-
  truth warning, and fit → brief → routed-context → clean-run sequencing.
  Keep checked-in examples synthetic and generated output outside `.mdp`.
- **Tests:** public-artifact lint, fixture validator commands, README/docs
  command smoke, `git diff --check`, and full `make validate` on the
  implementation PR.
- **Dependencies:** U1-U5; MDP-239 queue/status/dependency metadata is
  read-only from this plan branch.

## Verification Contract

| Gate | Command/proof | Coverage |
| --- | --- | --- |
| Plan diff hygiene | `git diff --check`; inspect front matter and `git status --short --branch` | Plan-only artifact is tracked, scoped, and free of malformed whitespace. |
| CLI parser/schema/capability | `cargo test --manifest-path cli/Cargo.toml cli`; focused `synthetic_chain`/`capabilities`/`schemas` tests | U1 flags, result contract, stable output and diagnostics. |
| Synthetic-chain focused Rust tests | `cargo test --manifest-path cli/Cargo.toml synthetic_chain`; `cargo test --manifest-path cli/Cargo.toml requirements`; `cargo test --manifest-path cli/Cargo.toml source_binding` | U2-U4 generation, lineage, provenance, write safety, and existing v2 regression behavior. |
| Formatting | `cargo fmt --manifest-path cli/Cargo.toml -- --check` | Implementation PR only; no Rust implementation is present in this plan branch. |
| Public example command proof | Build the CLI, run `rebind-synthetic-chain --dry-run/--apply`, then strict `validate-source-binding`, strict `validate-prompt-output`, and `fit` against the staged output outside the pack | AC1-AC3 and AC8; all values synthetic and offline. |
| Script/installed parity | `node --check` and relevant `node`/shell tests; run installed release smoke when v2 fixture coverage changes | U5; source/installed contract parity without provider execution. |
| Full repository gate | `make validate` from the implementation PR checkout | U1-U6; record actual output and do not claim this unimplemented command passed on the plan branch. |

Plan-branch validation is intentionally limited to the new Markdown artifact's
diff hygiene and unchanged baseline. The implementation PR must run the
focused and full commands above after code exists; this plan does not claim
the future command or generated chain has passed.

## Dependencies, Risks, and Blocker Awareness

### Dependencies and sequencing

| Dependency | Relationship | Execution rule |
| --- | --- | --- |
| MDP-225 | Completed upstream fix: valid v2 signal observations are consumed by `prospect-fit-or-brief`. | Use shipped behavior as downstream proof; do not reimplement or weaken signal readiness. |
| MDP-167 | Source-binding validation and portable digest contract. | Reuse exact requirements/source-binding identity and outside-pack boundary; do not create a second binding contract. |
| MDP-198 | First-class sourced-signal schema/lineage foundation. | Preserve v2 projection, contributor, and receipt semantics; test hash churn as the regression. |
| MDP-149 | Related native signal/receipt provenance proof. | Keep provenance boundaries and synthetic evidence assumptions; no evidence collection belongs here. |
| MDP-239 | Parent execution index owning ordering and closeout. | Keep MDP-230 as a Phase 1 planned child; implementation readiness remains gated by the parent. |
| MDP-226 | Canonical routed-context readiness work. | Do not generate or change routed-context acceptance; document the generated chain as upstream input to the later fit → brief → routed-context path. |
| MDP-231 | Observed driver configuration/model identity work. | Do not bind runtime model parameters or provider observations here. |
| MDP-237 | Sealed native request compiler that MDP-230 blocks. | Preserve the existing blocker relation; do not mark MDP-237 ready or edit its metadata from this plan. |

### Risks and mitigations

| Risk | Mitigation and failure contract |
| --- | --- |
| Hashes computed from in-memory/canonical JSON instead of final bytes | Serialize once at each dependency edge; hash the final pretty JSON plus newline; test byte/newline drift and exact replay. |
| Generated value is schema-valid but violates applicability/readiness | Derive from compiled value/status policies, use declared predicates, validate the full staged chain, and return a stable unsupported-recipe issue rather than fabricate. |
| Projection or attempt omitted/duplicated/wrongly qualified | Enumerate from `requirements`; key by qualified contract/projection and `(contract, attribute)`; run missing/duplicate/unknown tests and actual validators. |
| Nested signal receipts drift from top-level hashes | Generate observations only after results bytes are final and populate each receipt from one immutable hash tuple; mutate each receipt field independently in tests. |
| Rebinding legitimizes real/private evidence | Require `synthetic_fixture`, explicit normalized synthetic marker, opaque non-URL locators, safe-value scan, and pre-write refusal for every input boundary. |
| Force/partial writes destroy useful fixtures | Validate all candidates before writes; require force; create digest-keyed backups before atomic replacement; preserve prior files on failure. |
| Output changes pack content digest | Reject all destinations inside pack `.mdp`/authored roots; test digest before and after fresh/rebind operations. |
| Shared helper regresses v1/sample-leads | Keep command additive, preserve v1 and `mdp.sample-leads.v0` fixtures, run legacy/source-binding/prompt-output suites and full validation. |
| Generic packs have unsupported formats or conditional cycles | Use only declared contracts and return bounded unsupported/cycle diagnostics; never infer values from prose. |
| Documentation implies synthetic lineage is evidence or readiness | Label outputs as fixture scaffolding, keep `draft_allowed: false`, preserve MDP-239 planned state, and avoid delegation/automation labels. |

## Compatibility and Rollback

### Compatibility contract

- Existing `mdp.requirements.v1`, v2 lineage validators, v1 jobs,
  `sample-leads`, manually authored fixtures, and current command outputs stay
  compatible. The new command is additive and v2-only.
- `mdp.source-binding.v2`, `mdp.source-attempt-request.v2`,
  `mdp.collected-attempt-results.v2`, and `mdp.normalized-decision-input.v2`
  retain their current contracts. The new result contract does not alter their
  authority or source-truth meaning.
- Generated outputs remain outside the pack, so the portable pack digest and
  authored source inventory are unchanged. Existing files are never overwritten
  unless apply and force are both explicit.
- Dry-run is the safe default; identical replay is a no-op; changed-file
  replacement is recoverable through digest-keyed backups.
- Synthetic lineage remains non-authoritative fixture scaffolding. No command
  invocation, validator success, or hash proves source authenticity,
  authorization, provider execution, or truth.

### Rollback

- Revert the single implementation PR/commit. No database, pack schema
  migration, release tag rewrite, or irreversible source mutation is required.
- If a generated fixture must be discarded, remove only the explicit external
  output directory or restore its digest-keyed backup; do not delete pack files
  or add a temporary global ignore.
- A failed apply must leave the prior destination chain intact. Mixed partial
  chains are invalid and must be reported for manual recovery from backups.
- Rebinding never mutates `--input-dir` in place, so operators can compare or
  discard the result before replacing integration-owned fixtures.
- Do not roll back by weakening synthetic-only checks, allowing URLs, widening
  v2 source classes, changing v1 behavior, or editing MDP-239/MDP-237
  dependencies.

## Acceptance Mapping

| MDP-230 acceptance criterion | Planned implementation and proof |
| --- | --- |
| One command accepts exact pack root and canonical job ID and emits complete synthetic v2 chain | U1 command contract plus U2/U3 deterministic builder; canonical Clay public example and exact-job/v2-only tests. |
| Pack, requirements, top-level lineage, and nested signal receipt hashes use exact emitted bytes | U3 dependency-ordered final-byte serializer and `sha256_hex`; byte/newline mutation and nested-receipt tests. Existing canonical requirements digest remains canonical by contract. |
| Output immediately passes existing validators | U3/U5 stage all four final bytes and call `validate_source_binding_file` plus strict `validate_prompt_output_file_with_lineage_inputs` before any apply. |
| Rebinding only clearly synthetic fixtures; real/customer evidence cannot silently rebind | U4 synthetic-only provenance scan, opaque locator rule, explicit synthetic marker, and refusal matrix. |
| Dry-run reports files/digest changes before writing | U1/U4 result contract and per-file old/new action/hash/byte plan; test no directory/file/backup creation. |
| Existing files require explicit apply/force and recoverable backup | U4 safe transaction: create missing files under apply, block changed files without force, digest-keyed backup before atomic replacement, interrupted-write recovery tests. |
| Fixtures are public-safe | U2/U3 deterministic example values, `synthetic_fixture` source classes, opaque locators, no credentials/private paths/customer records, public-artifact lint. |
| Documentation shows fit → brief → routed-context → clean-run preparation | U6 updates `cli/USAGE.md`, `docs/decision-input-contracts.md`, and Clay example README; U5 runs the actual offline command chain without a provider. |

## Definition of Done

- The implementation exists only after the MDP-239 execution-index gate permits
  it; this plan branch contains no runtime implementation or generated product
  artifacts.
- `rebind-synthetic-chain` has one versioned result contract, deterministic
  defaults, exact-byte lineage propagation, strict staged validation,
  synthetic-only refusal, dry-run, apply/force, backup, and no-change replay.
- Focused tests cover the eight acceptance examples, v1 compatibility, all
  projection/attempt joins, provenance refusal, pack containment, and partial
  write recovery.
- The canonical public-safe example produces a validator-ready chain and the
  documented fit → brief → routed-context → clean-run preparation path without
  network/model calls.
- Capabilities, schema discovery, docs, and the example agree on ownership,
  command flags, output hashes, write policy, and synthetic-not-source-truth.
- The implementation PR records actual `cargo fmt`, focused tests, CLI proofs,
  `git diff --check`, `make validate`, and installed smoke results where
  applicable. No plan-only validation is presented as implementation proof.
- The Linear handoff records the exact repository, base ref/commit, branch,
  pushed commit, tracked plan path, validation status, and remaining blockers;
  MDP-239/MDP-237 relationships and planned state remain unchanged, with no
  automation/autofix or `delegate:blocks` branding added.

## Sources and Repository Evidence

- Linear MDP-230: Generate and rebind complete governed v2 synthetic input chains.
- Linear MDP-239: Execution Index — MDP 0.1.73 Sanity dogfood and Agent Skill friction.
- Linear MDP-225: v2 signal observations are validated but ignored by `prospect-fit-or-brief` (completed upstream context).
- `cli/src/cli.rs`, `cli/src/app.rs`, `cli/src/commands/mod.rs`: command parsing and dispatch seams.
- `cli/src/commands/requirements.rs`: `requirements`, v2 source-attempt/results/normalized schemas, and normalized-lineage validation.
- `cli/src/commands/source_binding.rs`: v2 binding schema, projection coverage, and semantic validation.
- `cli/src/commands/prompt_output.rs`: file/value validation and raw-byte lineage reads.
- `cli/src/commands/schemas.rs`: `signal_observation_v2_schema` and schema registry.
- `cli/src/models.rs`: typed Decision Input attributes, source classes, statuses, projections, and policies.
- `cli/src/artifact_hash.rs`, `cli/src/pack_io.rs`: canonical/byte hashing, pack digest, and current write-plan helpers.
- `cli/src/commands/sample_leads.rs`: bounded synthetic fixture values and public-safety wording.
- `examples/clay-audiences-self-serve-enterprise-expansion/README.md` and `fixtures/*.json`: current public-safe v2 lineage examples.
- `docs/decision-input-contracts.md`, `cli/USAGE.md`, and `docs/getting-started.md`: current operator and contract guidance.
