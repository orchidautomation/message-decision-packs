---
title: Cold-Model Conformance and MDP-for-MDP Reference Proof - Plan
type: feat
date: 2026-08-13
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: legacy-requirements
execution: code
origin: docs/orchid/requirements/2026-08-08-mdp-195-self-standing-pack-sufficiency-contract.md
---

# Cold-Model Conformance and MDP-for-MDP Reference Proof

## Goal Capsule

Implement the MDP-201 qualification layer that proves whether each released job in an exact pack release is self-standing for a cold model under a recorded host envelope. The proof must combine deterministic MDP assertions with separately recorded behavioral trials without making MDP a model provider or agent runtime.

The product repository is authoritative for conformance contracts, validation, trace projection, offline fixtures, CLI discovery, skills, and release behavior. The `mdp-for-mdp` repository is authoritative for the canonical three-job reference-pack proof. Harvey remains historical comparison material only.

Stop implementation if a proposed change makes MDP call a model, weakens existing deterministic validation, exposes private model-visible content in public reports, or treats an unverified host claim as cold-context assurance.

Execution profile: two sequential repository changes and two PRs. Ship and install the product change first. Then qualify the reference pack against that installed release. Live provider calls require separate action-time approval and are not required for the deterministic PR or installer smoke test.

---

## Product Contract

### Summary

MDP-195 defines what self-standing means. MDP-196 through MDP-200 provide pack-owned product foundations, prompt contracts, source lineage, and minimal routed context. MDP-201 adds the missing qualification record: a closed, replayable answer to whether a fresh model could complete one declared job using only the exact released pack, declared inputs, and recorded host envelope.

The result is job-specific. A pack can qualify one job and fail another. Deterministic sufficiency and behavioral qualification remain separate dimensions so a structurally correct pack cannot hide weak model behavior, and a good-looking model output cannot bypass a broken pack contract.

### Actors

- **Pack author:** declares jobs, product authority, inputs, prompts, output contracts, and fixtures.
- **Qualification operator:** freezes the release and evaluator, runs or imports fresh host trials, and publishes the bounded result.
- **Customer-chosen host:** performs model calls outside MDP and records the invocation envelope and outputs.
- **MDP CLI:** validates contracts, computes deterministic assertions, validates recorded behavioral evidence, and renders projections.
- **Reviewer:** inspects job-level status, failures, sanitized evidence, and the full person journey without reading implementation code.

### Requirements

- **R1 — Job-specific qualification:** Qualify every declared released job independently against an exact pack identity, portable digest, CLI version, evaluator version, and host envelope. Covers D1-D12, Q1-Q4, and B1-B9 from the origin contract.
- **R2 — Separate proof planes:** Keep deterministic conformance assertions independent from model-sensitive behavioral assertions and report both before computing a qualification verdict.
- **R3 — Host-neutral invocation evidence:** Define a closed, versioned model-invocation record for normalization, generation, and review that records requested and resolved model identity, exact prompt and input hashes, model-visible context hash, isolation state, optional provider metadata, terminal state, and output references without storing secrets or public message bodies.
- **R4 — Cold-context assurance:** Treat only enforced or independently verified isolation dimensions as satisfying cold-context preflight. Attestation, a new chat, or a claimed fresh session cannot elevate assurance.
- **R5 — Versioned evaluator:** Bind every trial and report to a profile-owned challenge inventory and evaluator version that a pack author cannot silently weaken.
- **R6 — Deterministic assertion compilation:** Compute D1-D12 from existing pack, requirements, routing, prompt, output, run, verification, and trace authorities instead of duplicating their rules in the new evaluator.
- **R7 — Behavioral trial validation:** Validate recorded Q1-Q4 and B1-B9 evidence with closed statuses, exact hashes, required challenge coverage, and independent fresh trial identities.
- **R8 — Sampling and verdicts:** Require three fresh trials for each model-sensitive fixture. Every hard boundary must pass 3/3. Useful job completion must pass at least 2/3. Expected bounded non-success counts as confirmed behavior, not a suite failure.
- **R9 — No-draft failure:** A failed release, lineage, fit, routing, invocation, governed-output, claim, isolation, or receipt gate must prevent a usable qualified output and preserve a bounded reason code.
- **R10 — Composite person journey:** Create a hash-linked conformance authority that connects source lineage, normalization invocation, normalized validation, deterministic fit, routed context, generation or review invocation, output validation, claims validation, deterministic run receipt, and verification for one synthetic subject.
- **R11 — Trace remains a projection:** Extend trace rendering to inspect the composite conformance authority without making the trace a decision, model-output, or storage authority.
- **R12 — Privacy projection:** Keep the private conformance record exact and hash-complete while limiting public reports to synthetic or approved-public metadata, opaque private evidence IDs, statuses, hashes allowed by policy, and sanitized limitations.
- **R13 — Offline repeatability:** Ship a key-free conformance suite with recorded synthetic evidence and mutation cases. Ordinary `make validate`, releases, and installer smoke tests must never require a provider key or billable model call.
- **R14 — Agent discoverability:** Expose schemas, command capabilities, job status, evaluator identity, evidence requirements, and inspection commands through CLI JSON and canonical plugin skills so a new host does not need explanatory chat.
- **R15 — Canonical reference proof:** Produce independent deterministic sufficiency results and approved fresh-host behavioral qualification for the three released `mdp-for-mdp` jobs: `prospect-fit-or-brief`, `outbound-copy-brief`, and `outbound-copy-review`. The intermediate offline proof reports behavioral status as `unassessed`; MDP-201 cannot close until fresh trials satisfy R8.
- **R16 — Immutable reference identity:** Bind the reference report to an immutable `mdp-for-mdp` commit or release plus exact pack, requirements, prompt, routed-context, invocation, output, and receipt hashes.
- **R17 — Product boundary:** MDP validates imported evidence and deterministic authorities. It does not call models, select providers, price tokens, enrich records, send outreach, or retain live prospect data.
- **R18 — Private evidence policy:** Every private record binds an access class, policy owner or reference, retention deadline, deletion disposition, and host capability status. Missing or unsupported policy enforcement blocks qualification.
- **R19 — Public hash eligibility:** A public projection may expose an artifact digest only for synthetic bytes or bytes covered by an exact-hash named-human `sanitized-public` approval receipt. Other private artifacts use opaque report-local IDs.
- **R20 — Hostile artifact safety:** Treat every imported candidate, invocation, evaluator, output, and journey artifact as untrusted. Enforce staged-root containment, regular-file rules, bounded resources, provenance-backed freshness, and protected-challenge bindings before reading, hashing, traversing, or qualifying it.

### Flows

#### F1 — Deterministic preflight

1. The operator freezes the candidate pack release and evaluator version.
2. The CLI validates the pack and compiles each job's skills route, requirements, foundation, prompts, source contract, and context budget.
3. The CLI computes D1-D12 from existing authorities.
4. Any failed hard deterministic assertion blocks behavioral qualification for that job.

#### F2 — Normalization qualification

1. A customer-chosen host receives the exact normalization prompt and declared inputs for `prospect-fit-or-brief`.
2. The host records a fresh invocation envelope and returns a structured result.
3. MDP validates the source binding, attempts, collected results, normalized artifact, invocation hashes, and isolation evidence.
4. A ready artifact enters deterministic fit; a bounded non-success result remains no-draft evidence.

#### F3 — Governed generation qualification

1. The operator builds a distinct v2 lineage chain for `outbound-copy-brief` rather than reusing a job-mismatched fit fixture.
2. Deterministic fit and minimal routing produce canonical `mdp.routed-context.v1` bytes.
3. The host receives only the exact job prompt, declared inputs, and canonical routed model-visible context produced in step 2.
4. MDP validates the invocation, governed output, routed-context attachment, selected authority, and claims.
5. The conformance record links the behavioral evidence to the deterministic run and verification receipts.

#### F4 — Governed review qualification

1. The operator compiles `outbound-copy-review` with a synthetic supplied draft.
2. The host records a fresh review invocation and returns the governed review artifact.
3. MDP validates review status, unsupported claims, boundary findings, and no-draft behavior.
4. Accepted and rejected review cases remain separate recorded trials.

#### F5 — Report and trace

1. The CLI validates every artifact and hash link in the composite conformance authority.
2. The evaluator computes job-level deterministic, preflight, behavioral, and overall statuses.
3. The CLI emits a private exact projection and a sanitized public projection from the composite conformance authority.
4. `mdp trace` renders JSON or Mermaid from the validated authority and cannot add facts.

### Acceptance Examples

- **AE1 — Complete job proof (covers R1-R8, R15):** Given an exact released job, complete deterministic assertions, three fresh qualifying trials per required behavioral fixture, and a verified cold host envelope, the report is `qualified-for-job-under-envelope` and names the exact release, job, evaluator, fixture-set identity, model/runtime envelope, evaluation date, sampling results, and known limitations.
- **AE2 — Structurally ready but behaviorally weak (covers R2, R8):** Given D1-D12 pass but useful completion succeeds only once in three trials, deterministic status remains passed while the overall job is not qualified.
- **AE3 — Correct refusal (covers R8-R9):** Given missing required evidence, all three trials return the declared bounded non-success with no usable draft, and the fixture passes B7.
- **AE4 — Unknown isolation (covers R4, R7):** Given a host says the session is fresh but cannot enforce or verify memory, tools, or neighboring context, Q1 fails and the job is not qualified.
- **AE5 — Mismatched job lineage (covers R1, R10):** Given a `prospect-fit-or-brief` source chain attached to `outbound-copy-brief`, validation fails before model output can qualify.
- **AE6 — Tampered invocation (covers R3, R9):** Given a changed prompt, model-visible input, output, or receipt byte, the invocation hash chain fails closed.
- **AE7 — Trace mutation (covers R10-R11):** Given a trace projection edited independently of its authorities, re-rendering ignores the edit and reproduces the authoritative journey.
- **AE8 — Safe publication (covers R12):** Given a private report with exact local artifact references, the public projection omits private paths, raw prompts, provider payloads, and message bodies while retaining sufficient statuses and hashes for the approved public proof.
- **AE9 — Offline release proof (covers R13):** Given no provider credentials, the full recorded fixture suite, mutation tests, release validation, and installed CLI smoke test pass.
- **AE10 — No hidden explanation (covers R14):** Given a new shell-capable host with only the installed CLI and plugin, it discovers all three jobs, their prompts, trial requirements, validators, statuses, and trace commands without repository archaeology.
- **AE11 — Private evidence lifecycle (covers R18-R19):** Given a host that cannot honor the declared access, retention, deletion, or publication policy, qualification returns `no-draft:policy-blocked`; no private hash or artifact reference enters the public projection.
- **AE12 — Hostile artifact containment (covers R20):** Given an absolute path, parent traversal, link, device, oversized artifact, deep JSON, cyclic chain, forged freshness label, replayed trial, or pre-exposed challenge, validation fails before unsafe bytes are read or a passing assurance state is computed.

### Success Criteria

- All three `mdp-for-mdp` jobs have independent D/Q/B matrices. Offline proof produces exact deterministic verdicts and an `unassessed` behavioral status until fresh trials are separately approved and completed.
- The full synthetic person journey is hash-linked and traceable from source attempt through governed output and deterministic verification.
- Every committed conformance fixture validates offline with no API keys.
- A mutation to any frozen authority produces a stable failure at the correct boundary.
- The public projection passes privacy validation and contains no private or live prospect content.
- The installed released CLI reproduces the committed reference proof.

### Scope Boundaries

In scope:

- Closed conformance, invocation, report, and composite-journey contracts.
- Deterministic CLI compilation, validation, reporting, and trace projection.
- Synthetic offline fixtures and recorded behavioral evidence.
- Canonical plugin skill and capability parity.
- Exact three-job `mdp-for-mdp` qualification.

Deferred to an action-time approval gate but required for MDP-201 closeout:

- Live billable provider qualification runs.
- A hosted qualification service or Cloudflare deployment.
- Broader API/MCP workflow wrappers beyond the CLI contract needed for this proof.

Out of scope:

- MDP-199 model economics, token pricing, Braintrust integration, or observability vendor selection.
- Provider selection, model routing, enrichment, outreach, CRM, sequencing, or identity storage.
- Harvey as the release candidate.
- Claims that Codex, Claude, or another interactive host is cold solely because a new task was opened.

### Dependencies

- MDP-192 and MDP-195 through MDP-200 are shipped and form the product contract.
- MDP-186 supplies the deterministic host/run conformance baseline.
- The product CLI release must land and install before the reference pack proof is frozen.
- Live qualification evidence depends on separate approval for provider calls and public projection approval.

### Sources

- `docs/orchid/requirements/2026-08-08-mdp-195-self-standing-pack-sufficiency-contract.md`
- `docs/host-conformance.md`
- `docs/run-receipts.md`
- `docs/decision-input-contracts.md`
- `docs/job-prompt-contracts.md`
- `docs/minimal-context-routing.md`
- `docs/decision-traces.md`
- `docs/orchid/qa/2026-08-03-mdp-184-clean-run-proof.md`
- `cli/src/run_contracts.rs`
- `cli/src/run_runtime.rs`
- `cli/src/commands/prompt_output.rs`
- `cli/src/commands/decision_trace.rs`
- `scripts/test-run-conformance.mjs`

---

## Planning Contract

### Key Technical Decisions

- **KTD1 — Compose existing authorities.** The conformance evaluator reads existing validation outputs and hashes. It does not reimplement pack, routing, prompt, claim, or run semantics. Governs R2, R6, R9.
- **KTD2 — Keep model execution external.** The CLI accepts and validates host-produced invocation evidence. It never sends a request to a model provider. Governs R3, R13, R17.
- **KTD3 — Add only missing behavioral invocation evidence.** Normalization, generation, and review share one closed trial envelope with a phase discriminator. It references existing prompt-invocation, source-lineage, run-bundle, runner-audit, and run-receipt hashes wherever those authorities already own a field. New fields are limited to the external behavioral call: phase, trial identity, requested and resolved model identity, fresh-session timestamps, model-visible context hash, isolation observations, optional provider request metadata, and the raw output hash/private reference. Governs R3-R5.
- **KTD4 — Separate exact and public projections.** The composite conformance record is the sole hash-complete cross-phase authority. The private exact report and public sanitized report are validated projections; the public projection contains no raw content or local paths. Governs R10-R12.
- **KTD5 — Make the composite record authoritative, not the trace.** A new hash-linked journey/conformance record owns cross-phase references. Trace remains a deterministic JSON/Mermaid projection. Governs R10-R11.
- **KTD6 — Use the CLI as the normative host API for this increment.** Shell-capable hosts invoke the installed CLI or wrap it as a subprocess. Capability JSON and skills expose the workflow. Do not expand the existing run MCP into a general orchestration service during MDP-201. Governs R14, R17.
- **KTD7 — Keep behavioral calls out of ordinary validation.** Committed recorded synthetic trials are verified during `make validate`. Creating new live trials is a separate operator command and approval path. Governs R7-R8, R13.
- **KTD8 — Qualify jobs independently.** Each job gets its own source/input lineage, prompt, routed context, behavioral matrix, and verdict. Shared subject metadata may be referenced, but job IDs and hashes cannot be reused across incompatible contracts. Governs R1, R15-R16.
- **KTD9 — Use `mdp-for-mdp` as the canonical reference.** The proof freezes an immutable reference-pack identity after the product CLI release is available. Harvey remains non-authoritative comparison material. Governs R15-R16.
- **KTD10 — Preserve existing replay semantics.** Deterministic replay stays byte-exact. Behavioral outputs need not be byte-identical; qualification depends on recorded hashes, classifications, thresholds, and fresh independent trial IDs. Governs R7-R8, R10.
- **KTD11 — Compile from one closed candidate manifest.** `mdp.conformance-candidate.v1` binds one exact release, job, evaluator-owned fixture or challenge ID, evaluator-inventory digest, and the paths plus hashes of every existing authority required to evaluate it. Expected results resolve only from the evaluator inventory; a pack-authored expectation is classified separately and cannot satisfy Q3. Release-level assertions run once per job; fixture-level assertions run once per candidate. Governs R1-R2, R5-R7, R10.
- **KTD12 — External scorers produce qualitative evidence.** MDP never derives product accuracy, usefulness, or policy judgment from an output hash. A host or human produces a closed evaluator result bound to the inspected output hash, evaluator identity/version, scorer type, per-assertion score and rationale, disagreement state, and any required named-human resolution. MDP validates and aggregates those records. Governs R2, R5, R7-R8.
- **KTD13 — Reuse staged-root and resource-bound validation.** Every file reference resolves beneath one declared artifact root through the existing run-contract containment pattern. Reject absolute paths, traversal, symlinks, hard links, non-regular files, missing members, post-validation substitution, and contract-specific size or cardinality overflow before unsafe reads or recursive traversal. Governs R12, R18-R20.
- **KTD14 — Integrity cannot elevate provenance.** Hashes prove captured bytes, not fresh execution or truthful host claims. Trial freshness and each cold-context dimension bind the shipped provenance and assurance vocabulary, evidence reference, verifier identity and configuration hash, candidate digest, model envelope, and creation evidence. Self-attestation remains below passing assurance. Governs R3-R5, R7, R20.
- **KTD15 — Protect challenge independence.** The evaluator inventory records challenge selection or generation method/version, creation time, frozen candidate digest, seed or selection receipt, evaluator version, and prior-exposure status. Q3 fails when protected challenge provenance is absent, pre-freeze, exposed, reused contrary to rotation policy, or substituted by the pack author. Governs R5, R7, R20.

### High-Level Technical Design

```text
customer-chosen model host
  -> mdp.model-invocation.v1 records (normalization / generation / review)
  -> existing phase validators
       source lineage / fit / routed context / governed output / claims
  -> existing deterministic mdp.run + verify-run receipts
  -> mdp.job-conformance.v1 composite authority
       exact release + evaluator + trials + D/Q/B outcomes + hashes
  -> private exact report
  -> validated public projection
  -> mdp trace JSON or Mermaid projection
```

The new evaluator has three layers:

1. **Contract layer:** closed schemas and canonical hashing for invocation, evaluator inventory, trial result, composite journey, and public projection.
2. **Evaluation layer:** adapters that translate existing command outputs into D assertions, validate Q/B evidence, enforce sampling, and compute terminal status.
3. **Presentation layer:** CLI JSON, summary, public projection, trace rendering, docs, skills, and offline fixtures.

The implementation must complete a field-reuse audit before adding the trial schema. Existing authority remains the owner as follows:

| Field class | Existing owner | New trial record behavior |
|---|---|---|
| Pack, job, prompt, declared inputs | prompt invocation, source lineage, or run bundle | Reference exact authority hash; do not copy mutable values. |
| Deterministic policy, assurance, decision, context, output authority | run bundle, runner audit, run receipt, and phase validators | Reference exact artifact and validation hashes. |
| Behavioral call identity and observed isolation | use run bundle/audit/receipt when the call has that shipped authority; otherwise no current authority | Reference shipped hashes when present. For an external call, record provenance-bound observations in the trial envelope and cap assurance below passing unless an accepted verifier receipt exists. |
| Behavioral raw output | host-private artifact | Record hash and opaque private reference only. |
| Evaluator outcome | conformance evaluator | Record assertion result, rationale, evaluator role, and resolution state. |

### Versioned Status Model

The exact enum names may be refined during implementation if existing MDP vocabulary already owns an equivalent term. The semantic states are fixed:

- Deterministic assertion: `passed`, `failed`, `not-applicable`.
- Preflight assertion: `passed`, `failed`, `not-applicable`.
- Behavioral trial: `passed`, `bounded-non-success-confirmed`, `failed`, `malformed`.
- Job sufficiency: `sufficient-for-job`, `not-sufficient-for-job`, `unassessed`.
- Behavioral qualification: `qualified-for-job-under-envelope`, `not-qualified-for-job-under-envelope`, `unassessed`.
- Overall no-draft: any hard failure leaves no usable qualified result.

Terminal-state ownership is fixed before implementation:

| Condition | Deterministic status | Behavioral status | Overall result | Usable output |
|---|---|---|---|---|
| Invalid release or unknown job | `failed` | `unassessed` | `not-sufficient-for-job` | No |
| Missing evaluator or candidate authority | `failed` | `unassessed` | `not-sufficient-for-job` | No |
| Malformed invocation, hash, or evidence | existing assertion state or `failed` | `malformed` | `not-qualified-for-job-under-envelope` | No |
| Required context absent or unusable | existing `insufficient-context` assertion | `bounded-non-success-confirmed` only when the model returns the declared non-success | `not-sufficient-for-job` or `not-qualified-for-job-under-envelope` as reported by the two independent planes | No |
| Isolation preflight fails or is unknown | deterministic plane unchanged | `failed` | `not-qualified-for-job-under-envelope` | No |
| Expected negative behavior is correctly bounded | `passed` when deterministic gates agree | `bounded-non-success-confirmed` | Counts as a passing behavioral fixture, not a qualified usable output | No |
| Any hard behavioral assertion fails | deterministic plane unchanged | `failed` | `not-qualified-for-job-under-envelope` | No |
| Required sampling is incomplete | deterministic plane unchanged | `unassessed` | `unassessed` behavioral qualification | No qualified output |
| All deterministic and approved behavioral gates pass | `passed` | `passed` | `qualified-for-job-under-envelope` | Only when the authoritative job result separately grants it |

Stable reason codes must refine these rows without inventing a second terminal-state vocabulary. Existing `insufficient-context`, `no-draft`, and validation outcomes remain authoritative where they already apply.

### Implementation Constraints

- Unknown fields fail in every new public contract.
- Private records bind access, retention, deletion, and host-capability policy. Unsupported policy enforcement yields `no-draft:policy-blocked`.
- Public digests require synthetic classification or an exact-hash named-human `sanitized-public` approval receipt. Transformation invalidates the approval.
- Canonical JSON hashing uses the repository's shared helpers.
- Requested and resolved model identities are distinct fields.
- Prompt inputs and model-visible context are represented by ordered names and byte hashes; public reports do not copy their bodies.
- Assurance uses the shipped host-conformance vocabulary and cannot be raised by caller-selected labels.
- Evaluator-only expectations, protected challenges, and scoring guidance cannot appear in the model-visible context hash.
- Disputed qualitative hard-boundary scores preserve both scores, rationales, and evaluator roles. A public resolved claim requires a second named human adjudicator who is neither the sole pack author nor sole release approver.
- The adjudicator must supply a provenance-bearing approval receipt from a customer-controlled identity or review authority. The receipt binds the output hash, competing scores, decision, purpose, and reviewer role; self-declared identity leaves the dispute unresolved.
- Retries create new trial IDs. They do not overwrite failed trials.
- New trial IDs and recomputed hashes cannot prove freshness. Provenance and verifier bindings under KTD14 are required.
- Expected negative fixtures retain their failed terminal state while counting as correct bounded behavior.
- Fixtures remain synthetic or explicitly approved `sanitized-public` artifacts.
- No new production dependency on a model SDK, provider API, telemetry vendor, database, or cloud service.

### Sequencing

1. Implement and test the product contracts and offline evaluator.
2. Add composite journey validation and trace projection.
3. Add offline fixtures, skills, docs, capability discovery, and full product validation.
4. Merge, patch-release, install, and smoke test the product CLI.
5. Branch `mdp-for-mdp` from current `origin/main` and build exact three-job proof fixtures against the installed release.
6. Merge the reference proof after strict validation.
7. Request action-time approval, execute live fresh behavioral trials, and attach the sanitized projection without changing deterministic authorities. If approval is not granted, stop with MDP-201 still open and behavioral status `unassessed`.

### System-Wide Impact

- **CLI contract:** gains conformance schemas, validation/reporting commands, and capability metadata.
- **Trace contract:** gains a new accepted authoritative input while preserving projection-only semantics.
- **Run contract:** remains deterministic-only.
- **Agent behavior:** gains a discoverable cold-model qualification workflow and no-draft rules.
- **Release process:** ordinary release validation stays offline and key-free.
- **Privacy:** exact private evidence and public sanitized projection become distinct validated artifacts.

### Risks and Mitigations

- **Risk: duplicated decision logic.** Mitigation: D assertions consume existing validators and receipts under KTD1.
- **Risk: a recorded invocation is mistaken for verified cold context.** Mitigation: Q1 fails unless every hard dimension is enforced or independently verified.
- **Risk: reference fixtures reuse the wrong job lineage.** Mitigation: build separate v2 chains and add a job-mismatch mutation case.
- **Risk: public reports leak prompts or prospect content.** Mitigation: use an allowlisted projection schema and negative privacy fixtures.
- **Risk: behavioral tests become flaky CI.** Mitigation: CI validates committed records only; new provider calls remain separate.
- **Risk: MDP expands into orchestration.** Mitigation: no provider client, generic task runner, or outreach action enters this plan.
- **Risk: reference proof discovers a core defect after release.** Mitigation: use a linked product follow-up PR and patch release; do not hide core changes in the reference-pack PR.

---

## Implementation Units

### U1. Add closed conformance contracts

**Goal:** Define canonical, versioned authority for the candidate manifest, host invocation evidence, evaluator inventory and results, private-record policy, publication approvals, trial results, composite job conformance, and public projection.

**Requirements:** R1-R5, R7-R8, R12, R16.

**Files:**

- Create `cli/src/conformance.rs` or a focused equivalent module selected after nearby-pattern inspection.
- Modify `cli/src/main.rs` or the crate module registry.
- Modify `cli/src/commands/schemas.rs`.
- Modify `cli/src/commands/capabilities.rs`.
- Add focused fixtures under `examples/cold-model-conformance/`.

**Approach:** Use serde models with closed enums and unknown-field rejection. Reuse canonical JSON and digest helpers. The candidate manifest binds one staged artifact root, evaluator-owned fixture or challenge identity, evaluator-inventory digest, and every required existing authority by relative path and digest; it cannot define the expected result. Before finalizing the trial schema, complete a field-by-field ownership table against `RunBundleV1`, `RunnerAuditV1`, `RunReceiptV1`, prompt invocation, and source lineage. Reference shipped owners wherever present. External-only observations remain lower assurance without an accepted verifier receipt. Model invocation records carry artifact hashes and opaque private references, not raw public content. Evaluator results bind the inspected output hash and preserve scorer identity, rationale, disagreement, and provenance-backed adjudication. Private records bind lifecycle policy. Public digests bind synthetic classification or an exact-hash named-human approval receipt. Contracts set explicit byte, depth, string, collection, fan-out, and chain-length limits using existing bounded run-contract patterns. Export each schema and advertise it through capabilities.

**Test scenarios:** Valid normalization, generation, and review records; complete and incomplete candidate manifests; requested/resolved model mismatch retained as evidence; unknown fields; malformed hashes; duplicate trial IDs; forbidden public paths/content; private digest without approval; approval invalidated by transformed bytes; absent or unsupported lifecycle policy; unsupported assurance elevation; forged freshness labels; evaluator/model-context contamination; exposed or pre-freeze challenges; conflicting evaluator scores; missing required independent adjudicator; every resource limit.

**Verification:** Focused Rust schema/model tests and live `mdp schema` plus `mdp capabilities` JSON validation.

**Dependencies:** None.

### U2. Compile deterministic sufficiency assertions

**Goal:** Produce D1-D12 for one exact job by composing existing validators and authorities.

**Requirements:** R1-R2, R5-R6, R9, R14.

**Files:**

- Create or modify `cli/src/commands/conformance.rs`.
- Modify `cli/src/cli.rs` and `cli/src/app.rs`.
- Reuse `cli/src/commands/health.rs`, `requirements.rs`, `skills.rs`, `routing.rs`, `prompt_output.rs`, and `run_verification.rs` through stable helpers rather than shelling out.
- Add command schema coverage in `cli/src/commands/schemas.rs`.

**Approach:** Add an inspect/compile command that accepts one closed `mdp.conformance-candidate.v1` manifest. Before reading referenced bytes, it enforces KTD13 against the staged artifact root. It then validates release, job, evaluator-owned fixture or challenge identity, evaluator-inventory digest, authority digests, evaluator and challenge provenance, and private-record policy; resolves the expected result from evaluator authority; runs existing checks; and emits assertion records with authority references and reason codes. Release-level D assertions are computed once per job. Fixture-dependent D assertions are computed from each candidate's explicit lineage, routed context, governed output, run bundle, receipt, and trace authorities. A failed hard assertion blocks behavioral qualification.

**Test scenarios:** Complete job; legacy/unassessed job; missing prompt; invalid foundation; missing DIC; over-budget route; invalid governed-output contract; failed run verification; privacy fixture failure; absolute path; traversal; symlink; hard link; non-regular file; missing member; post-validation replacement; unrelated job error does not contaminate the selected job.

**Verification:** Focused command tests and JSON schema validation for every emitted status.

**Dependencies:** U1.

### U3. Validate behavioral trials and compute verdicts

**Goal:** Validate imported fresh-trial evidence and calculate Q1-Q4, B1-B9, sampling thresholds, job sufficiency, and job qualification.

**Requirements:** R2-R9, R13.

**Files:**

- Extend `cli/src/conformance.rs`.
- Extend `cli/src/commands/conformance.rs`.
- Modify `cli/src/output.rs` for compact summaries.
- Add adversarial fixtures under `examples/cold-model-conformance/fixtures/`.

**Approach:** Validate invocation records, external evaluator results, lifecycle and publication policy, provenance-backed freshness, evaluator bindings, model-visible hashes, independent trial identity, and protected challenge coverage. MDP validates and aggregates qualitative evidence; it does not generate scores. Preserve competing scores and rationale. Require the independent human resolution defined by KTD12 for disputed qualitative hard-boundary claims. Calculate hard-boundary 3/3 and usefulness 2/3 thresholds. Preserve expected bounded non-success separately from malformed or failed execution.

**Test scenarios:** Three passing trials; 2/3 useful; 1/3 useful; one hard-boundary failure; repeated trial ID; replay under a new ID; cross-job receipt reuse; resumed session; unknown isolation; self-attested assurance; evaluator oracle leak; exposed, pre-freeze, reused, or pack-authored challenge substitution; prompt injection; stale source; conflict; refusal; unsafe claim; latent-knowledge counterfactual; retry as a new trial; disputed score without adjudication; disputed score resolved by the wrong role; valid independent resolution; every terminal-state table row.

**Verification:** Focused evaluator tests with exact terminal states and stable reason codes.

**Dependencies:** U1, U2.

### U4. Add composite person journey and trace projection

**Goal:** Hash-link all behavioral and deterministic phases for one synthetic subject and render the journey without creating a second decision authority.

**Requirements:** R9-R12, R16.

**Files:**

- Extend `cli/src/conformance.rs`.
- Modify `cli/src/commands/decision_trace.rs`.
- Modify `cli/src/commands/decision_trace/schema.rs`.
- Modify `cli/src/commands/decision_trace/render.rs`.
- Modify `cli/src/commands/decision_trace/tests.rs`.
- Update `docs/decision-traces.md`.

**Approach:** The composite authority references each source artifact by contract, contained relative path or opaque ID, and exact digest. KTD13 applies before traversal. Validation follows the bounded chain without importing bodies into the report. Trace accepts the validated composite authority and renders selected/excluded authority, model phase, deterministic decisions, validations, gaps, and limitations.

**Test scenarios:** Complete journey; missing link; mismatched job; mismatched pack release; tampered prompt/output/context/run receipt; private reference in public projection; deterministic re-render; JSON/Mermaid parity; candidate authority near and over the existing 1 MiB input limit; projection near and over the 256-node/512-edge trace limits.

**Verification:** Trace schema tests, canonical output snapshots, and mutation tests.

**Dependencies:** U1-U3.

### U5. Ship the offline cold-model conformance suite

**Goal:** Prove the new contracts and evaluator without API keys or live model calls.

**Requirements:** R7-R9, R12-R14.

**Files:**

- Create `scripts/test-cold-model-conformance.mjs`.
- Create `examples/cold-model-conformance/README.md`.
- Add synthetic pack, invocation, output, report, and mutation fixtures under `examples/cold-model-conformance/`.
- Modify `Makefile`.
- Modify release/install smoke scripts if needed for installed-artifact coverage.

**Approach:** Mirror the existing `scripts/test-run-conformance.mjs` pattern. Validate recorded evidence and deterministic replay only. Include the required positive, non-success, mutation, isolation, injection, and privacy cases.

**Test scenarios:** At minimum the categories in the origin contract for normalization, generation, and review; all work offline; any credential lookup or network access is a test failure. Add lifecycle-policy, public-hash membership leakage, forged freshness, challenge-provenance, containment, oversized, deeply nested, cyclic, and amplification cases.

**Verification:** `node scripts/test-cold-model-conformance.mjs` and the new `make validate` target.

**Dependencies:** U1-U4.

### U6. Align CLI discovery, docs, and agent skills

**Goal:** Make the qualification flow executable by a new shell-capable host with no explanatory chat.

**Requirements:** R3-R5, R13-R14, R17.

**Files:**

- Modify `README.md`, `CONCEPTS.md`, and `cli/USAGE.md`.
- Create `docs/cold-model-conformance.md`.
- Update `docs/host-conformance.md`, `docs/job-prompt-contracts.md`, and `docs/decision-traces.md`.
- Modify `plugin/skills/mdp/SKILL.md` and `plugin/skills/mdp/references/cli-operator.md`.
- Modify `plugin/skills/mdp-gtm-brief/SKILL.md` and `plugin/skills/mdp-gtm-brief/references/outbound-copy-brief.md`.
- Modify `plugin/skills/mdp-pack-review/SKILL.md` and relevant references.
- Modify skill contract and behavioral tests.

**Approach:** Teach agents to compile deterministic sufficiency first, import only host-generated invocation evidence, stop no-draft on any failed gate, validate before tracing, and distinguish recorded evidence from verified isolation. State that CLI subprocess access is the normative API surface for this increment.

**Test scenarios:** Skill package parity; missing/non-ready conformance evidence stops drafting; no skill implies MDP calls a model; capabilities expose every required command and schema; public docs contain no private paths.

**Verification:** Skill validators, packaging checks, CLI help/capability tests, and doc-link checks available in the repo.

**Dependencies:** U1-U5.

### U7. Merge, release, install, and smoke test the product CLI

**Goal:** Make the conformance contracts available as an immutable installed product release before reference-pack evidence is frozen.

**Requirements:** R13-R14, R17.

**Files:** Release metadata and installer smoke coverage required by the repository workflow.

**Approach:** Merge the product PR, cut the next patch release from current `main`, run the documented installer, and smoke test the installed CLI's schemas, capabilities, and offline conformance fixture.

**Test scenarios:** Released tag contains the merged commit; installed version matches the release; installed schema and offline fixture validation pass without provider keys.

**Verification:** Product `make validate`, GitHub release checks, documented installer, and installed binary smoke test.

**Dependencies:** U1-U6.

### U8. Build the canonical `mdp-for-mdp` three-job proof

**Goal:** Apply the installed released CLI to the canonical reference pack and produce independent deterministic proof for all three declared jobs. Behavioral status remains `unassessed` until U9 runs approved fresh trials.

**Requirements:** R1, R8-R16.

**Target repository:** `orchidautomation/mdp-for-mdp`.

**Files in that repository:**

- Extend `.mdp/` only where the released contracts expose a real pack gap.
- Create `tests/fixtures/cold-model-conformance/` with per-job source, invocation, output, receipt, and report fixtures.
- Create `scripts/validate-cold-model-conformance.sh`.
- Create or update a focused QA proof under `docs/orchid/qa/`.
- Update `README.md` or the revenue hub with a link to the proof, without duplicating product contracts.

**Approach:** Branch from current `origin/main`. Freeze the exact pack commit/release, pack digest, evaluator, and installed CLI version after all `.mdp/` changes have settled. Build a new v2 lineage chain for `outbound-copy-brief`. Add recorded synthetic generation and review evidence that proves the offline evaluator while leaving behavioral qualification `unassessed`. Keep all people, companies, inputs, and outputs synthetic. Do not reuse Harvey or live Ledger data.

**Test scenarios:** Per-job positive; missing source; stale evidence; invalid enum; ambiguity/conflict; refusal; unsupported claim; routing isolation; receipt mutation; prompt injection; latent-knowledge counterfactual. Add a full person journey for `outbound-copy-brief` and job-specific matrices for fit and review.

**Verification:** Strict pack validation/eval, v2 lineage validation, cold-model conformance validation, deterministic run/verify, trace JSON/Mermaid, and private/public projection checks.

**Dependencies:** U7.

### U9. Merge the reference proof and complete approved live qualification

**Goal:** Merge the offline reference proof with exact release receipts, then complete the fresh behavioral trials required to close MDP-201 after separate action-time approval.

**Requirements:** R8, R13, R15-R17.

**Files:** Release metadata and QA proof files required by repository workflows; no live outputs in public fixtures unless explicitly approved and sanitized.

**Approach:** Merge the reference-pack PR after its installed-release checks pass. Ask for action-time approval before any billable provider call. After approval, record three fresh trials per model-sensitive fixture, validate them locally, preserve evaluator disagreements and required independent adjudication, and publish only the sanitized projection. Without approval, stop cleanly with MDP-201 still open and no qualification claim.

**Test scenarios:** Released commit equals installed artifact; installed CLI validates reference fixtures; no-key smoke test; live-call approval absent means clean paused status and no qualification claim; approved trials satisfy the R8 thresholds before closeout.

**Verification:** Product `make validate`, release checks, installed CLI smoke, reference validation script, and exact merged commit/release/install state summary.

**Dependencies:** U8.

---

## Verification Contract

Run narrow tests after each unit, then the full gates.

Product repository gates:

```bash
cargo fmt --manifest-path cli/Cargo.toml --all -- --check
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --strict --dir plugin/assets/templates/basic
node scripts/test-run-conformance.mjs
node scripts/test-cold-model-conformance.mjs
python3 scripts/validate-skill-contracts.py
python3 scripts/validate-skill-packaging.py
make validate
git diff --check
```

Reference repository gates use the released, installed `mdp` binary:

```bash
mdp --json validate --strict --dir .
mdp --json eval --strict --dir .
scripts/validate-v2-lineage.sh
scripts/validate-cold-model-conformance.sh
git diff --check
```

Release gate:

1. Product PR checks are green and merged.
2. The next patch tag contains the merged commit.
3. The documented installer succeeds.
4. The installed binary reports the new version and passes the offline cold-model smoke test.
5. The reference proof is regenerated or revalidated with that installed binary.

Behavioral gate, only after separate approval:

1. One fresh, non-resumed invocation per trial.
2. Three trials per model-sensitive fixture.
3. All hard boundaries pass 3/3.
4. Useful completion passes at least 2/3.
5. Requested and resolved model identities plus host envelope are recorded.
6. The public projection passes privacy validation before publication.

---

## Definition of Done

- [ ] U1: All new contracts are closed, versioned, schema-exported, canonically hashed, and mutation-tested.
- [ ] U2: D1-D12 compile independently for any declared job from existing authorities.
- [ ] U3: Q1-Q4 and B1-B9 validate from recorded trials with the approved 3/3 and 2/3 thresholds.
- [ ] U4: One composite person journey validates end to end and trace renders it without becoming authority.
- [ ] U5: The full offline cold-model suite passes with no provider keys or network access.
- [ ] U6: Capabilities, CLI help, docs, canonical skills, and packaged skills agree on the workflow and no-draft rules.
- [ ] U7: Product merge, patch release, install, and installed smoke test are recorded explicitly.
- [ ] U8: All three `mdp-for-mdp` jobs have independent exact-release deterministic reports, and the outbound generation journey is complete with behavioral status honestly `unassessed`.
- [ ] U9: The reference-pack merge is recorded, action-time approval is captured, and fresh trials, evaluator disagreements, sampling, adjudication, and public projection pass. Without approval, the work remains correctly paused and MDP-201 remains open.
- [ ] Every origin-contract assertion D1-D12, Q1-Q4, and B1-B9 maps to an implementation test and a reference proof result.
- [ ] Public artifacts contain only synthetic or approved sanitized evidence and no raw prompts, provider payloads, private paths, credentials, or live prospect data.
- [ ] Every private record has a validated access, retention, deletion, and host-capability policy; unsupported policy enforcement is `no-draft:policy-blocked`.
- [ ] Every public artifact digest is synthetic or backed by a valid exact-hash named-human approval receipt; all other private evidence uses opaque report-local IDs.
- [ ] Every imported artifact passes staged-root containment, regular-file, resource-bound, provenance/freshness, and protected-challenge validation before qualification.
- [ ] MDP performs no model, provider, enrichment, outreach, pricing, or cloud execution work.
- [ ] No live behavioral call occurs without separate action-time approval, and MDP-201 is not marked complete while behavioral qualification is absent.
- [ ] No abandoned experimental code, duplicate validators, stale fixtures, or superseded documentation remains in either diff.
- [ ] Both repositories end on reviewable, independently shippable branches with one PR per repository change.
