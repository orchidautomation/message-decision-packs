---
title: MDP-230 Governed v2 Synthetic Input Chain - Plan
type: feat
date: 2026-08-21
execution: code
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-230
linear_issues:
  - MDP-230
  - MDP-239
  - MDP-225
---

# MDP-230 Governed v2 Synthetic Input Chain - Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Add one deterministic, offline CLI path that generates a complete synthetic v2 source-binding, source-attempt-request, collected-attempt-results, and normalized-decision-input chain for one exact pack root and canonical job ID, or safely rebinds an existing synthetic chain after pack/requirements drift. |
| Product shape | Add the rebind-synthetic-chain command with explicit dry-run, apply, force, output-directory, and deterministic timestamp/seed controls. The command emits four public-safe JSON artifacts and a machine-readable write/validation report. |
| Authority boundary | MDP owns the compiled requirements contract, synthetic fixture construction, exact-byte digests, lineage receipts, local validation, and safe file-write policy. It does not collect evidence, call providers or models, or rebind real/customer evidence. |
| Compatibility | Additive to the existing v2 validators, sample-leads, v1 jobs, and manual integration-owned fixture flow. Existing commands and existing files keep their current semantics. |
| Public safety | Fresh artifacts use only synthetic_fixture, opaque non-URL locators, stable example values, and explicit synthetic markers. Rebinding refuses real, private, customer, provider, or ambiguous provenance. |
| Execution state | This is a hosted-execution-ready plan handoff. The Linear issue remains Backlog and phase planned until the MDP-239 batch gate and dependency ordering permit implementation; this plan does not authorize an implementation PR. |

## Product Contract

### Problem

The current v2 chain is valid when its four files are authored consistently, but changing a pack or compiled requirements digest requires a host to manually update the source binding, request hash, collected-results hash, normalized envelope hash fields, and every nested signal-observation receipt. The existing sample-leads command creates safe prospect rows but not a job-bound v2 lineage chain. That makes synthetic validation fixtures useful but fragile to author.

MDP-225 already proves that signal observations can be consumed by the downstream prospect-fit-or-brief route. MDP-230 should supply the deterministic fixture and rebind path that lets the existing validators consume that lineage without moving evidence collection or model execution into MDP.

### CLI contract

Add a subcommand with this shape, following the current clap and checked-output conventions:

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

The exact job ID and pack root are required inputs to the command's execution contract. When input-dir is absent, the command builds a fresh deterministic chain from the selected job's compiled v2 requirements. When input-dir is present, the command reads the four conventional files below as one synthetic chain and updates only the governed bindings needed to match the current exact pack and requirements:

    source-binding.json
    source-attempt-request.json
    collected-attempt-results.json
    normalized-input.json

The output directory is explicit and outside the pack's .mdp tree by default. The command must reject output paths that would alter the source tree digest or write generated artifacts into the authored pack. Any future in-pack output exception requires a separate issue and contract.

The command defaults to dry-run. Apply is the only mode permitted to create files. A changed existing file requires both apply and force. Force must create a recoverable backup before replacement. An existing file with identical emitted bytes is reported unchanged and is never rewritten.

### Generated artifact contract

Introduce a versioned command result contract, proposed as mdp.synthetic-v2-chain.v1, containing:

- contract, job_id, pack and requirements identity receipts;
- the four output paths and their exact emitted-byte SHA-256 digests;
- dry_run, applied, and deterministic inputs;
- per-file action: create, unchanged, blocked, or overwrite-with-backup;
- planned backup paths where applicable;
- validation status for source binding and bound prompt output;
- a stable refusal or write-conflict issue code when the operation cannot proceed.

The four artifact hashes in lineage fields must be computed from the exact bytes that will be or were written. The implementation must serialize each JSON artifact once using the repository's established pretty-JSON-plus-trailing-newline format, hash those bytes with artifact_hash::sha256_hex, and feed that exact hash into the next dependent artifact. canonical_json_sha256 is appropriate for the compiled requirements receipt where the existing contract requires it, but must not be substituted for an emitted file hash.

### Synthetic provenance contract

Fresh generation must use source class synthetic_fixture throughout source bindings, attempts, results, and signal observations. Every source locator and upstream reference is an opaque non-URL identifier derived from the job, qualified attribute/projection ID, and seed. The normalized prospect must carry source_kind synthetic and synthetic true, with example.com-style or otherwise target-safe values only.

Rebinding an existing chain is allowed only when all source classes, provenance entries, and signal observations are explicitly synthetic_fixture, all locators remain opaque and non-URL, the normalized output is explicitly synthetic, and no credential, private path, customer identifier, raw provider record, or private source value is present. The command must fail closed rather than downgrade or rewrite an ambiguous or real source class. It must preserve the semantic values and statuses of an accepted synthetic chain while repairing the exact binding/hash graph.

### Scope boundaries

In scope:

- v2-only job compilation and deterministic synthetic values for declared scalar contracts and signal projections;
- source-binding generation for every compiled projection;
- one attempt for every compiled attribute, with additional deterministic contributor attempts when a projection requires them;
- exact-byte hash propagation through source binding, request, results, normalized output, and nested signal receipts;
- staging validation through the existing source-binding and prompt-output validators;
- dry-run diff/hash/write planning, apply/force/backup behavior, and no-change replay;
- CLI capabilities, output summary, focused tests, public-safe example proof, and operator documentation.

Out of scope:

- v1 chain generation or changes to v1 behavior;
- source collection, provider calls, credentials, enrichment, scraping, model calls, normalization execution, CRM writes, or hosted APIs;
- rebinding real, customer, private, reviewed-internal, public-web, or unknown evidence;
- changing MDP-226 routed-context acceptance, MDP-231 observed runtime binding, or MDP-237 sealed request compilation;
- changing the semantic authority of signal observations or treating synthetic lineage as source truth;
- automatically editing existing pack files, manifests, prompts, requirements, or source ledgers.

## Product Contract and Acceptance Examples

### AC1 — One command emits a complete exact-job v2 chain

Given the canonical pack root and a job that compiles to the existing signal-aware v2 runtime, the command creates all four conventional artifacts in the requested output directory. The source binding contains every compiled projection and pins the exact pack and requirements receipts. The request, results, normalized envelope, and nested observations are mutually bound.

The canonical public example under examples/clay-audiences-self-serve-enterprise-expansion, using prospect-fit-or-brief, is the required end-to-end proof. A scalar/v1 job is rejected with a stable v2-required diagnostic rather than being silently converted.

### AC2 — Every digest uses exact emitted bytes

For each file, the hash stored in downstream lineage fields equals sha256_hex of the final bytes on disk. Re-serialization order, indentation, or newline changes must therefore produce a different hash and a validation failure until the chain is rebuilt. The implementation must test that changing one byte in source-binding causes all dependent receipts to be regenerated.

### AC3 — Existing validators pass immediately

Before a dry-run or apply result is reported successful, the candidate bytes are staged and validated with the actual validate-source-binding and validate-prompt-output paths, including the exact prompt, source binding, request, collected results, job, and strict mode required by the current v2 contract. The report must include both validator results and no write may occur if either fails.

### AC4 — Real or ambiguous provenance is refused

An input chain with any non-synthetic source class, URL locator, missing synthetic marker, private-looking path, customer/provider record, or inconsistent provenance is rejected before any destination write. Fresh generation cannot opt out of the synthetic marker and rebind cannot be used to change a source class into synthetic_fixture.

### AC5 — Dry-run reports all changes without writing

Dry-run reports create, unchanged, blocked, and overwrite-with-backup actions, old and new byte counts/digests, and the planned backup location. It must not create the output directory, modify an existing artifact, change the pack digest, or leave a backup.

### AC6 — No accidental overwrite; force is recoverable

Apply creates absent files. Apply without force refuses a changed existing file with the repository's stable write-conflict code. Apply plus force copies each replaced file to a non-colliding backup path keyed by its prior digest before atomically replacing it. A failure during staging, backup, or replacement leaves the original destination chain intact.

### AC7 — Exact replay is a no-change operation

Running the same command with the same pack, job, seed, as-of value, and accepted synthetic input produces identical bytes. Re-running against those files reports unchanged, performs no writes, creates no backups, and returns the same artifact digests.

### AC8 — Public-safe docs and fixtures show the operator path

The docs show deterministic fit → brief → routed-context → clean-run preparation using the generated chain, and explicitly say that synthetic lineage is fixture scaffolding rather than source truth. The committed proof contains no credentials, private paths, customer data, raw provider records, or real contact values.

## Planning Contract

### Key technical decisions

1. Add a dedicated command module rather than overloading sample-leads. Keep sample-leads' mdp.sample-leads.v0 output and tests stable; share only safe value/identity helpers where extraction avoids duplication.
2. Make the chain builder a single dependency-ordered pipeline. It must not build four independent JSON objects and patch hashes afterward. Build and serialize source binding first, then request, then results, then normalized output; update all nested receipts from the exact hashes produced by those serialized bytes.
3. Reuse the requirements compiler and existing schemas. Promote the private schema helpers in requirements.rs only as needed, or expose a small crate-private compiled-job view, rather than copying source-attempt, collected-results, normalized-envelope, value-contract, or signal-observation rules into the new module.
4. Refuse unsupported inputs and impossible synthetic values deterministically. The command must not emit a superficially complete but validator-invalid chain. Diagnostics should identify the exact job, contract, projection, or attribute path and use a stable capability error code.
5. Stage before write. Candidate files are written only to a unique OS temporary staging directory for validation and byte hashing, then moved/copied under the output write transaction. Dry-run never touches the requested output directory.
6. Keep the generated chain outside .mdp. This avoids changing pack_content_sha256 and keeps generated fixtures integration-owned, consistent with the existing source-binding ownership boundary.
7. Keep readiness and delegation separate. The plan is implementation-ready as an artifact, while MDP-230 remains phase planned/backlog until MDP-239's batch gate permits execution. Do not restore phase:ready-for-agent or native delegation as part of implementation.

### High-level design

    exact pack root + job
                 |
                 v
    requirements(root, job)
                 |
       require signal-aware v2
                 |
       +---------+----------+------------------+
       |                    |                  |
       v                    v                  v
    source binding     request/results      normalized envelope
    (all projections)  (all attributes)     (attrs + observations)
       |                    |                  |
       +---------- exact emitted-byte SHA-256 -+
                            |
                            v
      stage -> validate-source-binding -> validate-prompt-output
                            |
              dry-run report / apply transaction

Fresh generation derives all values from compiled contracts and a fixed as-of/seed. Rebinding starts from an accepted synthetic chain, updates pack/requirements pins, then rebuilds the dependent hashes in the same order. Both paths share the same final staging and validation gate.

### Exact files and symbols likely touched by the implementation

| Area | File | Existing symbol or planned symbol | Planned responsibility |
|---|---|---|---|
| Command model | cli/src/cli.rs | Commands; new RebindSyntheticChain variant | Parse dir, job, out-dir, input-dir, as-of, seed, dry-run, apply, and force with clap conflicts/requires. |
| Dispatch | cli/src/app.rs | run dispatch match; attach_dry_run_artifact; print_checked/print_output | Invoke the builder, preserve checked exit semantics, and emit the new result contract. |
| Command module | cli/src/commands/mod.rs | command module declarations | Register synthetic_chain. |
| Builder | cli/src/commands/synthetic_chain.rs | new SyntheticChainPlan, SyntheticChainArtifacts, build/rebind/stage/write functions | Own provenance guard, deterministic value recipe, dependency-ordered serialization, validation, and write transaction. |
| Requirements | cli/src/commands/requirements.rs | requirements; source_attempt_request_schema_v2; collected_attempt_results_schema; normalized_envelope_schema | Expose only the crate-private compiled schemas/metadata needed by the builder; preserve existing output and hash behavior. |
| Source binding | cli/src/commands/source_binding.rs | validate_source_binding_file; validate_source_binding_v2 | Reuse exact file validation and, if needed, expose a value-level synthetic provenance guard without weakening current validation. |
| Prompt output | cli/src/commands/prompt_output.rs | validate_prompt_output_file_with_lineage_inputs; read_json_file_with_hash | Reuse strict lineage validation against staged exact bytes; do not create a parallel validator. |
| Hashing | cli/src/artifact_hash.rs | sha256_hex; canonical_json_sha256 | Use raw-byte SHA-256 for emitted artifacts and existing canonical requirements SHA only where its contract requires it. |
| File safety | cli/src/pack_io.rs | write_json_file; planned_json_write_after_dirs; planned_file_write_after_dirs | Add byte-oriented serialization/planning and recoverable backup/atomic replacement helpers without changing existing command semantics. |
| Contracts | cli/src/constants.rs | contract constants | Add the versioned synthetic-chain result contract and stable refusal/write-conflict codes only if the existing registries do not already cover them. |
| Capabilities | cli/src/commands/capabilities.rs | command registry; stable_error_codes; boundaries | Register command, flags, output contract, offline/read/write behavior, and stable diagnostics. |
| Summary | cli/src/output.rs | summarize | Add concise report fields for dry-run, applied, validation, actions, and digest receipts. |
| Schemas | cli/src/commands/schemas.rs | SchemaTarget; signal_observation_v2_schema | Export/reuse current v2 schema pieces and add a result schema only if capabilities/tests require it; do not duplicate the observation schema. |
| Safe fixture reuse | cli/src/commands/sample_leads.rs | sample_leads and deterministic fake-value helpers | Extract or reuse safe identity/value construction without changing mdp.sample-leads.v0. |
| Tests | cli/src/commands/requirements.rs or synthetic_chain.rs test module | existing signal_projection_fixture_from_root and signal_observation helpers | Move reusable fixture construction into production/test-shared code only if it avoids canonical-vs-raw hash mistakes; update tests to hash final bytes. |
| Public docs | cli/USAGE.md; docs/decision-input-contracts.md; examples/clay-audiences-self-serve-enterprise-expansion/README.md; README.md if the command inventory requires it | current v2/manual workflow sections | Document generation, rebind, dry-run/apply/force, exact-byte receipts, and fit → brief → routed-context → clean-run preparation. |

The implementation PR should not modify MDP-239's dependency graph, MDP-226/231/237 behavior, or unrelated dirty work from other branches.

### Ordered implementation steps

1. Confirm the selected job compiles to runtime v2 and enumerate its exact decision-input contracts, attributes, signal projections, normalization prompt, source-binding schema, source-attempt schema, collected-results schema, and normalized-output schema through requirements(root, job). Fail with a stable v2-required or requirements-unavailable diagnostic before planning any write.
2. Define the generated output file names, command result contract, deterministic defaults, and output containment rule. Validate that output-dir is outside the pack's authored .mdp tree and that input-dir cannot be the pack root or a path containing authored pack files.
3. Build a deterministic SyntheticRecipe from the compiled job. Resolve attributes in declared dependency order, select the first allowed enum or a type/format-specific safe value, mark conditional attributes not_applicable when their declared predicates do not apply, and preserve the same status/value in request results and normalized attributes. Reject a contract whose declared value or applicability rules cannot be safely synthesized.
4. Build source-binding.v2 with exact pack ID/version/content digest, requirements contract/digest, selected contract versions, normalization release, synthetic adapter/transformation identifiers, and one complete projection binding per compiled projection. Use only synthetic_fixture and opaque locators.
5. Serialize source-binding to final bytes in the shared byte serializer and compute the raw source-binding SHA. Build source-attempt-request.v2 with qualified contract IDs and one deterministic attempt per attribute, plus contributor attempts required by signal projections. Set its source_binding_sha256 to the source-binding raw hash, serialize once, and compute the request raw hash.
6. Build collected-attempt-results.v2 with the request's exact hash, source-binding hash, raw values/status/provenance/confidence/freshness required by each attribute, and one result per attempt. Include projection contributors as synthetic_fixture and keep all locators opaque. Serialize once and compute the results raw hash.
7. Build normalized-decision-input.v2 using the job-owned normalization receipt and normalized prospect schema. Set its request/results/binding hashes, project the generated attributes to their declared output paths, and emit one valid signal observation per generated projection when allowed by cardinality. Every observation receipt must contain the three raw hashes from steps 4–6. Serialize once.
8. For rebind input-dir, parse all four input files, enforce the synthetic-only policy and exact job/contract identity, preserve semantic values/statuses, update only current pack/requirements pins and dependent raw hashes, and run the same staged validation. Never convert non-synthetic provenance or silently repair malformed semantics.
9. Stage all four final bytes in a unique OS temporary directory. Call validate_source_binding_file with the exact staged binding and call validate_prompt_output_file_with_lineage_inputs in strict mode with the exact staged request/results/output, bound prompt, pack root, and job. Abort the command result before destination planning if either validator is invalid.
10. Produce the per-file dry-run plan by comparing final candidate bytes with destination bytes. For apply, create only missing parents/files; for changed files require force, create non-colliding digest-keyed backups first, then replace files using the repository's existing safe write patterns. Ensure partial failure reports the affected path and retains the prior file.
11. Add focused unit/integration tests and the canonical public example proof. Include exact replay, one-byte drift, pack/requirements drift, nested receipt drift, non-synthetic refusal, URL/private-locator refusal, dry-run no-write, conflict without force, recoverable force backup, and interrupted-transaction behavior.
12. Update capabilities, CLI usage, decision-input docs, and example operator instructions. Run the repository's actual validation commands, inspect the final diff for plan/runtime scope, and keep generated artifacts outside the pack and untracked unless the implementation explicitly adds a public-safe fixture.

### Dependency and sequencing contract

| Dependency | Relationship to MDP-230 | Execution rule |
|---|---|---|
| MDP-225 | Completed upstream signal-observation consumer fix; establishes that valid v2 observations are consumed by prospect-fit-or-brief. | Use the shipped behavior as the downstream proof; do not reimplement or alter it. |
| MDP-198 | Historical source-binding/v2 lineage work and manual fixture hash churn. | Reuse its schema/lineage decisions and use its repeated hash updates as the regression case. |
| MDP-167 | Source-binding validation and portable digest contract. | Treat validate-source-binding and exact pack/requirements pins as the authority; do not create a second binding contract. |
| MDP-149 | Related native signal/receipt provenance work. | Preserve its provenance boundary; no native evidence collection belongs here. |
| MDP-239 | Parent execution index for the 13-issue batch. | Keep MDP-230 as a Phase 1 planned child. Plan preparation may proceed in parallel; implementation readiness remains gated by the parent. |
| MDP-226 | Downstream/adjacent routed-context readiness work. | Do not add routed-context generation; document the generated chain as an input to the later fit → brief → routed-context path. |
| MDP-231 | Downstream runtime configuration/model-parameter binding. | Do not generate or bind runtime model parameters here. |
| MDP-237 | Downstream sealed native request compilation; blocked by MDP-230, MDP-226, and MDP-231. | Preserve the dependency graph. This plan supplies synthetic lineage fixtures only; it does not make MDP-237 ready or change its issue metadata. |

### Verification and real repository commands

The implementation PR must run the following commands from the repository root, adapting only temporary output paths and keeping them outside the pack:

1. Formatting and documentation hygiene:

       cargo fmt --manifest-path cli/Cargo.toml -- --check
       git diff --check

2. Focused synthetic-chain tests:

       cargo test --manifest-path cli/Cargo.toml synthetic_chain
       cargo test --manifest-path cli/Cargo.toml signal_aware

   If test names are split during implementation, run the exact module/test filters printed by cargo.

3. Dry-run and apply proof on the public example:

       cargo run --quiet --manifest-path cli/Cargo.toml -- --json rebind-synthetic-chain \
         --dir examples/clay-audiences-self-serve-enterprise-expansion \
         --job prospect-fit-or-brief \
         --out-dir /tmp/mdp-230-chain \
         --dry-run

       cargo run --quiet --manifest-path cli/Cargo.toml -- --json rebind-synthetic-chain \
         --dir examples/clay-audiences-self-serve-enterprise-expansion \
         --job prospect-fit-or-brief \
         --out-dir /tmp/mdp-230-chain \
         --apply

       cargo run --quiet --manifest-path cli/Cargo.toml -- --json validate-source-binding \
         --dir examples/clay-audiences-self-serve-enterprise-expansion \
         --job prospect-fit-or-brief \
         --file /tmp/mdp-230-chain/source-binding.json

       cargo run --quiet --manifest-path cli/Cargo.toml -- --json validate-prompt-output --strict \
         --dir examples/clay-audiences-self-serve-enterprise-expansion \
         --prompt .mdp/prompts/normalize-prospect.yaml \
         --source-binding /tmp/mdp-230-chain/source-binding.json \
         --source-attempt-request /tmp/mdp-230-chain/source-attempt-request.json \
         --collected-attempt-results /tmp/mdp-230-chain/collected-attempt-results.json \
         --file /tmp/mdp-230-chain/normalized-input.json

4. Downstream preparation proof:

       cargo run --quiet --manifest-path cli/Cargo.toml -- --json fit \
         --dir examples/clay-audiences-self-serve-enterprise-expansion \
         --normalized-input /tmp/mdp-230-chain/normalized-input.json \
         --prompt .mdp/prompts/normalize-prospect.yaml \
         --source-binding /tmp/mdp-230-chain/source-binding.json \
         --source-attempt-request /tmp/mdp-230-chain/source-attempt-request.json \
         --collected-attempt-results /tmp/mdp-230-chain/collected-attempt-results.json \
         --job prospect-fit-or-brief

   Follow the repository's existing brief and routed-context commands documented in cli/USAGE.md for the same validated output; do not add provider/model execution to this command.

5. Full repository validation:

       make validate

   Also run the existing public-artifact/template checks when they are part of make validate in the target revision. Record their actual output in the implementation PR. No validation step may write product artifacts into .mdp or use real/private data.

Plan-only validation for this branch is limited to the new document's diff hygiene and the unchanged repository baseline. It must not claim the unimplemented command has passed.

### Risks and mitigations

| Risk | Mitigation and failure contract |
|---|---|
| Hashes are calculated from in-memory canonical JSON while validators hash pretty JSON bytes. | Serialize once to final bytes at every dependency edge; use sha256_hex on those bytes; add a byte mutation/replay regression. |
| A generated value satisfies JSON Schema but violates readiness/applicability semantics. | Derive values from compiled contracts and existing status guards, run strict prompt-output validation before write, and refuse unsupported recipes. |
| A projection is omitted, duplicated, or bound to the wrong contract. | Enumerate projections from requirements, key them by qualified contract/projection ID, validate source binding, and test missing/duplicate/unknown cases. |
| Nested signal receipts drift from top-level hashes. | Generate observations only after results bytes are final; populate every receipt from the same raw digest tuple; test each receipt field independently. |
| Rebinding accidentally legitimizes real evidence. | Require synthetic_fixture at every source/provenance boundary, explicit synthetic normalized output, opaque non-URL locators, and a pre-write refusal scan. |
| Force or partial writes destroy a useful fixture. | Never write in dry-run; compare bytes first; require force for changed files; make digest-keyed backups before replacement; stage and validate all files before any destination write. |
| Output changes pack content hashes or authored files. | Reject destinations inside the pack's .mdp tree and default output outside the pack. Test that requirements pack digest is unchanged before/after generation. |
| v1 and sample-leads behavior regresses through shared helpers. | Keep the new command additive, preserve existing contracts, run focused legacy/sample-leads tests and make validate. |
| Generic packs have unsupported value formats or conditional cycles. | Use declared schema/value contracts only; return a stable unsupported-synthetic-recipe issue with the exact path rather than fabricating data. |
| Docs imply synthetic lineage is evidence or a ready-for-agent signal. | Label artifacts as public-safe fixture scaffolding, retain the MDP-239 planned/backlog gate, and document the no-provider boundary. |

### Rollback and compatibility

- Rollback is additive: revert the implementation commit/PR, leaving existing source-binding, prompt-output, sample-leads, v1, and manual fixture paths unchanged.
- Generated outputs live outside the pack. Removing a generated output directory or restoring a digest-keyed backup is recoverable and does not alter authored source.
- A failed apply must retain the previous destination bytes. If a transaction cannot complete, report the exact path and backup; do not silently continue with a mixed chain.
- Rebinding must not mutate input files in place. It writes only to the explicit output directory, so an operator can compare or discard the result before replacing an integration-owned fixture.
- The new result contract and CLI command are versioned/additive. Existing capabilities, schemas, and command output contracts remain stable; any schema-helper visibility change is crate-private.
- Do not change labels, statuses, dependency relations, or delegation on MDP-239 or downstream issues from the implementation branch. The issue stays Backlog/phase planned until the parent gate is explicitly cleared.

## Explicit Acceptance-Criteria Mapping

| MDP-230 acceptance criterion | Planned implementation proof |
|---|---|
| One command accepts exact pack root and canonical job ID and emits a complete synthetic source-binding/request/results/normalized-input chain. | RebindSyntheticChain CLI contract, deterministic SyntheticRecipe, four conventional files, v2-only exact-job guard; canonical Clay example end-to-end test. |
| Every pack, requirements, top-level lineage, and nested signal receipt hash is calculated from exact emitted bytes. | Dependency-ordered final-byte serializer using artifact_hash::sha256_hex; raw hash fields propagated after each serialization; one-byte and nested-receipt tests. |
| Output immediately passes validate-source-binding and bound validate-prompt-output. | Staged final bytes run through validate_source_binding_file and validate_prompt_output_file_with_lineage_inputs in strict mode before any apply. |
| Rebinding only clearly synthetic fixtures; real/customer evidence cannot silently rebound. | Synthetic-only provenance scan, explicit synthetic normalized marker, opaque locator rule, refusal diagnostics, and non-synthetic/URL/private-path tests. |
| Dry-run reports files/digest changes before writing. | Dry-run default, per-file action/old-new digest/backup plan result, no destination directory creation, and no-write test. |
| Existing files never overwritten without explicit apply/force and recoverable diff/backup contract. | Apply creates missing files; changed files require force; digest-keyed backups precede replacement; write conflict and recovery tests. |
| Fixtures public-safe, no credentials/private paths/customer data/raw provider records. | Deterministic example values, synthetic_fixture-only source classes, opaque locators, safe output scan, and public fixture review. |
| Docs show deterministic fit → brief → routed-context → clean-run preparation path. | Updates to cli/USAGE.md, docs/decision-input-contracts.md, and the public example README, with actual fit/brief/routed-context command verification. |

## Definition of Done

- The implementation exists only after the MDP-239 readiness gate permits it; this plan branch contains no product/runtime changes.
- The command has one versioned output contract, deterministic defaults, exact-byte hash propagation, strict staged validation, synthetic-only refusal, dry-run, apply/force, backup, and no-change behavior.
- All likely code paths listed above have focused tests, including the eight acceptance scenarios and legacy compatibility.
- The canonical public example produces a complete chain that passes the existing validators and downstream fit preparation without network or model calls.
- Documentation states the ownership boundary, safe file-write contract, deterministic operator path, and synthetic-not-source-truth warning.
- cargo fmt --check, focused cargo tests, the documented CLI proofs, git diff --check, and make validate pass in the implementation PR.
- The implementation PR is focused on MDP-230, does not change MDP-239 dependency metadata, and does not include unrelated worktree changes.
