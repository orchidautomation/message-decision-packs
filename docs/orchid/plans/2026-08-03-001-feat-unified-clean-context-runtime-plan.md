---
title: Unified Clean-Context Runtime - Plan
type: feat
date: 2026-08-03
topic: unified-clean-context-runtime
artifact_contract: ce-unified-plan/v1
artifact_readiness: requirements-only
product_contract_source: ce-brainstorm
execution: code
---

# Unified Clean-Context Runtime - Plan

## Goal Capsule

- **Objective:** Let an operator build an MDP pack in a context-rich agent conversation, then run proposal or GTM decisions through one separate, declared-input execution boundary whose artifacts and limitations are independently reviewable.
- **Product authority:** MDP owns pack authority, deterministic evaluation, run contracts, validation, and receipt verification. The host or customer owns source collection, credentials, production execution, retention, and downstream actions.
- **Open blockers:** No product decision blocks planning. Planning must choose the canonical implementation boundary, version the run-bundle and receipt contracts, and define migration from `mdp.run-receipt.v0`.

---

## Product Contract

### Summary

MDP will provide one profile-neutral clean-run contract and local runner surface for proposal and GTM. The authoring conversation remains the control plane; immutable run artifacts remain the decision authority.

### Problem Frame

MDP users often aggregate sources, discuss positioning, and build packs inside Codex, Claude Code, Clay, DeepLine, Freckle, or another context-rich host. That is useful authoring context, but it cannot prove that a later qualification, brief, or draft used only the released pack and declared run inputs.

A prompt to ignore earlier messages is still part of the polluted conversation. A new agent task improves hygiene, but the host may still inject system instructions, user configuration, repository files, tools, retrieval, environment state, or prior-session behavior. MCP transports a call but does not itself constrain what the called process or model can access.

The repository already has complementary foundations. Proposal has a local runner, native API path, stdio MCP wrapper, source/runner audits, and `mdp.run-receipt.v0`. GTM has Decision Input contracts, source-attempt ledgers, normalization validation, and deterministic `fit`, `brief --context`, and `check-claims` stages. Without one shared runtime, the two profiles will drift into duplicate isolation, hashing, receipt, and failure semantics.

### Key Decisions

- **One runner kernel, not proposal and GTM implementations.** (session-settled: user-approved — chosen over separate profile runners: duplication would let evidence and failure semantics drift.) Governs R6, R14, R19, R20, R33-R36.
- **Local and customer-controlled execution ships before a generalized hosted API.** (session-settled: user-approved — chosen over making the current synthetic Cloud gateway the production authority: the hosted surface has not earned that boundary.) Governs R21, R30, R31.
- **Same-conversation output is advisory.** (session-settled: user-approved — chosen over prompt-only context exclusion: the conversation cannot prove what the host supplied.) Governs R1, R2, R17, R39.
- **MDP returns immutable authority to the authoring task without returning decision control.** (session-settled: user-approved — chosen over allowing the original task to improve the result: ambient additions would invalidate the receipt.) Governs R15, R16, R18.
- **Customer and host systems retain operational ownership.** (session-settled: user-approved — chosen over expanding MDP into collection and orchestration infrastructure: MDP is a decision-context standard.) Governs R22, R23, R32, R37, R38.
- **Unqualified `audit-grade` language is not a v1 assurance claim.** (session-settled: user-approved — chosen over preserving a binary label: assurance must expose what was declared, observed, enforced, verified, and unknown.) Governs R3, R4, R12, R35.

### Actors

- A1. **Operator** — selects the released pack, approved inputs, runner, provider boundary, and any externally billable call.
- A2. **Authoring agent** — helps build and validate the pack, compiles requirements, launches the clean run, and explains returned artifacts without becoming decision authority.
- A3. **Shared runner** — stages declared artifacts, enforces the selected boundary, invokes a driver, records observed audit events, validates outputs, and emits terminal artifacts.
- A4. **MDP CLI evaluator** — computes portable pack identity, validates contracts, performs deterministic fit/routing/checks, and verifies receipts.
- A5. **Runner driver** — performs a native model API request, constrained subprocess/container invocation, customer-hosted call, or future hosted call.
- A6. **Customer or integration host** — owns source collection, batching, retries, credentials, network policy, retention, and downstream operational actions.
- A7. **MDP Cloud** — may later implement the same contract as a hosted adapter after the local/customer contract passes its adoption gate.

### Requirements

**Assurance semantics**

- R1. A fresh context means the inference request contains no prior conversation messages beyond the declared run envelope; it does not claim filesystem, tool, network, or provider isolation.
- R2. Stateless inference means the application does not intentionally continue a provider conversation or session; it does not promise deterministic prose, immutable provider internals, or absence of provider policy.
- R3. Declared-input isolation requires an enforced boundary that limits the runner to the hash-bound pack, prompt, inputs, allowed tools, allowed network, and explicit runtime metadata.
- R4. Audit evidence must distinguish caller declarations, runner observations, enforced controls, verifier results, unknown properties, and redacted properties.
- R5. Deterministic replay applies to canonical MDP evaluation and validation over identical artifacts and versions, not to byte-identical model generation.

**Immutable authority and receipts**

- R6. One canonical runtime implementation must own staging, canonicalization, hashing, assurance calculation, failure mapping, validation orchestration, and receipt creation for every profile and driver.
- R7. Each run must bind an immutable release identifier and full portable pack digest rather than only selected manifest files.
- R33. Preflight and invocation must consume one immutable content-addressed snapshot: the runner verifies it immediately before invocation, invokes only bytes from that snapshot, and binds the observed model-visible snapshot identity into the audit and receipt. Mutation or substitution stops no-draft.
- R8. Each run must bind a canonical declared-input manifest containing logical names, schema versions, media types, byte counts, hashes, and upstream source/normalization audit references where required.
- R34. Safe staging must reject traversal, absolute paths, links, special files, path collisions, unsupported media, and configured byte or file-count excesses; every artifact must resolve beneath a private run root without following links, and enforced limits and rejections must be audited.
- R9. Each run must bind the canonical declared prompt and every visible instruction or tool-schema component while representing hidden or unobservable instructions as unknown.
- R10. Each run must bind runner implementation/version/build and sanitized execution policy. Generative operations must additionally bind the exact driver executable or container-image digest when observable, driver configuration hash, dependency lock or build identity, authorized provider endpoint, provider, requested model, resolved model when available, inference parameters, and observable cache/session behavior; unverifiable execution identities must remain host-attested or unknown. Deterministic-only operations must record inference fields as not applicable and must not claim fresh inference.
- R11. Each successful run must bind normalized output, deterministic decision and reason codes, compiled context, validation results, and their hashes and schema versions.
- R12. Assurance must be computed from verifiable evidence and include machine-readable limitations; a caller cannot elevate assurance by assertion alone.
- R13. Receipt verification must fail after mutation, substitution, incompatible versioning, missing audit, false assurance elevation, or cross-profile artifact reuse.
- R35. Each receipt must bind a unique execution ID, creation time, profile, operation, and caller-supplied job or idempotency identity when present. Host-mode replay verification must additionally receive the expected job or idempotency identity, explicit freshness policy, and durable consumption ledger to distinguish exact deterministic replay, duplicate delivery, stale reuse, and cross-job substitution. Without that external state, standalone verification is integrity-only; a signature is not freshness proof.

**Operator and integration behavior**

- R14. The local operator entry point must be one command, working name `mdp run`, with thin CLI, stdio MCP, and plugin adapters over the same runtime.
- R15. A clean run must return terminal status, assurance and limitations, validation, and immutable output, decision, compiled-context, and receipt references with hashes.
- R16. Any mutation of a hash-bound authoritative pack, prompt, input, output, decision, compiled context, or validation artifact creates a new run identity and receipt. Explanation or presentation-only formatting by the authoring task remains outside the receipt and does not mutate the original run.
- R17. A spawned Codex, Claude Code, Cursor, or other agent task receives only the assurance supported by enforceable and observed controls; task creation alone cannot produce a verified isolation claim.
- R18. The original authoring task may explain or format returned artifacts but cannot add evidence, recompute qualification, alter the authoritative decision, or treat its commentary as covered by the receipt.

**Profiles and ownership**

- R19. Proposal must migrate onto the shared runtime while preserving current public commands through thin compatibility adapters and an explicit v0 receipt migration path.
- R20. GTM must consume compiled Decision Input and source-attempt artifacts through the same runtime, run deterministic fit/routing/checks, and return bounded decision context without collecting or inventing missing evidence.
- R36. Deterministic-only operations must use the shared runtime and receipt contract without invoking a generative driver; their assurance reports input-provenance limitations and deterministic artifact integrity rather than fresh-inference properties.
- R21. The MVP must implement the shared kernel, a native stateless API/BYOK reference driver, and a bounded adapter to existing headless-runner surfaces. Customer-controlled subprocess or container execution is a host-invoked conformance boundary: container lifecycle, sandbox policy, scheduling, retries, and credentials remain host-owned, and portable container management requires a separate scoped decision.
- R22. Table and job hosts must call the contract per row or job while retaining batching, scheduling, retry, idempotent orchestration, and rate-limit ownership.
- R23. Customer and host systems retain credential custody, source access, privacy and retention policy, production auth and tenancy, downstream CRM/outbound/submission, and incident response.
- R37. Secrets must enter only through a driver-specific non-artifact channel, remain excluded from inherited environments by default, and never appear in model-visible content, logs, receipts, or retained artifacts unless an explicit declared-input policy permits the model-visible value. Private run artifacts must follow explicit access, provider-endpoint authorization, retention, redaction, deletion, and cleanup-reporting policy.
- R38. Packs and declared source artifacts are untrusted data and cannot structurally modify runner policy, instruction hierarchy, allowed tools or network, output schemas, validation rules, assurance, or receipts. Detected injection patterns and observable policy violations must be audited and fail closed; semantic model compliance with the data-versus-instruction boundary remains an explicit limitation rather than a proved isolation claim.
- R39. Clean-run assurance begins at the frozen execution boundary. It does not certify how a pack, prompt, or declared input was authored or selected, and must report upstream pack, source, and normalization provenance as a separate assurance dimension without claiming to cleanse polluted upstream authority.

**Failure and security behavior**

- R24. A run may return success only when every required artifact validates and the receipt is complete.
- R25. Pack, compatibility, declared-input, or isolation preflight failures must return `no-draft:preflight-refused`.
- R26. Provider, subprocess, container, timeout, cancellation, or host failures must return `no-draft:runner-failed`.
- R27. Schema-invalid output, invalid deterministic decisions, and incomplete audit must return `no-draft:output-invalid`, `no-draft:decision-invalid`, or `no-draft:audit-incomplete` respectively.
- R28. Privacy, retention, tool, network, or credential policy failures must return `no-draft:policy-blocked`.
- R29. Artifact publication must be transactional. A non-success run may expose only sanitized diagnostics and audit metadata; model-generated or partially normalized content remains quarantined or is deleted according to declared retention policy and is never returned through CLI, MCP, plugin, stdout, or stable artifact references.

**Phased adoption**

- R30. The existing synthetic MDP Cloud gateway may demonstrate the contract but must remain allowlisted and explicitly non-generalized until the hosted adoption gate passes.
- R31. Generalized hosted execution requires the released local contract, cross-profile proof, host conformance kit, and a separate human-approved security, reliability, tenancy, and product gate.
- R32. Public adapters and fixtures must remain generic, synthetic, or sanitized and must not turn MDP into enrichment, orchestration, outbound, proposal submission, or generic automation infrastructure.

### Key Flows

- F1. Clean run from an authoring conversation
  - **Trigger:** A1 and A2 finish building or selecting a pack and want an authoritative proposal or GTM decision.
  - **Actors:** A1, A2, A3, A4, A5.
  - **Steps:** Freeze the pack, prompt, and declared inputs; show preflight; launch A3 outside the conversation; invoke A5 only when the selected operation is generative; validate through A4; return immutable artifacts to A2.
  - **Outcome:** A2 presents the decision and limitations without adding authority.
  - **Covers:** R7-R18, R24-R29.
- F2. GTM row or job
  - **Trigger:** A6 has a released pack and collected Decision Input artifacts for one prospect or account.
  - **Actors:** A3, A4, A6.
  - **Steps:** A6 supplies one canonical run envelope; A3 validates source attempts and normalization; A4 computes fit and bounded context; A6 stores the decision bundle and receipt together.
  - **Outcome:** Missing required run artifacts, unattempted required inputs, invalid normalization, or a pack-declared blocking outcome stops no-draft. Valid attempted-complete statuses preserve the pack's optional, conditional, hard-gate, and no-draft semantics; batching and downstream actions remain host-owned.
  - **Covers:** R20, R22-R29, R36.
- F3. Proposal review migration
  - **Trigger:** A1 supplies approved proposal source artifacts.
  - **Actors:** A1, A2, A3, A4, A5.
  - **Steps:** The existing proposal flow calls the shared kernel; compatibility adapters preserve current entry points; source, prompt, runner, output, validation, and decision artifacts share one receipt.
  - **Outcome:** Proposal no longer owns a parallel isolation or receipt implementation.
  - **Covers:** R6-R13, R19, R21.
- F4. Host cannot prove the requested boundary
  - **Trigger:** A driver cannot observe or enforce a required context, file, environment, tool, network, model, or audit property.
  - **Actors:** A1, A2, A3, A6.
  - **Steps:** A3 records the property as unknown or failed. If the selected execution policy requires that property, the run returns the applicable no-draft state; only optional or observational properties may remain unknown and downgrade assurance. A2 reports the limitation without substituting ambient work.
  - **Outcome:** The operator receives a reviewable limitation or no-draft state, never a falsely elevated receipt.
  - **Covers:** R1-R5, R12, R17, R24-R29.

### Acceptance Examples

- AE1. **Covers R1, R17.** Given a newly spawned coding-agent task that inherits repository rules and tools, when it runs without the shared constrained boundary, then its result is fresh-task/advisory evidence and not declared-input-isolated.
- AE2. **Covers R3, R8-R10.** Given a clean runner with one hidden environment sentinel and one undeclared file sentinel, when the run executes, then exact outbound-request capture plus filesystem and environment denial events prove the sentinels were excluded, and the verifier does not infer isolation merely because sentinel text is absent from model output.
- AE3. **Covers R5, R11.** Given identical canonical artifacts and evaluator versions but different valid model prose, when deterministic MDP evaluation replays, then the decision and reason codes match while the model-output hashes may differ.
- AE4. **Covers R12, R13.** Given a correctly signed receipt whose runner only asserted isolation, when the verifier evaluates it, then the signature verifies but assurance does not elevate beyond the observed evidence.
- AE5. **Covers R20, R24-R29.** Given a GTM record missing a required source attempt, when the host calls the clean runner, then the result is no-draft and contains no qualification or campaign draft.
- AE6. **Covers R16, R18.** Given an authoritative returned decision, when the authoring task adds evidence or rewrites the decision, then that modification is outside the original receipt and requires a new run.
- AE7. **Covers R30, R31.** Given the current synthetic Cloud gateway passes contract fixtures, when no bounded real pilot and hosted security gate exist, then it remains a synthetic evaluation surface rather than a generalized production API.
- AE8. **Covers R5, R20, R36.** Given frozen GTM Decision Input and source-attempt artifacts, when the operator requests deterministic fit and routing only, then the shared runtime invokes no model driver, emits the same deterministic decision on replay, and records inference properties as not applicable rather than fresh or unknown.
- AE9. **Covers R29, R33, R34, R37, R38.** Given a declared bundle containing a symlink escape, inherited secret sentinel, prompt-injection instruction, and provider output that later fails validation, when the runner executes, then staging or policy fails closed, no secret or partial output is returned, and only sanitized audit evidence is published.
- AE10. **Covers R22, R35.** Given a valid receipt replayed for its original idempotency identity and then substituted into another job, when verification runs in host mode, then the first event is reported as duplicate delivery or exact replay and the second is rejected as cross-job substitution.
- AE11. **Covers R4, R39.** Given a released pack whose content may have been influenced by ambient authoring context, when a clean run succeeds, then the receipt may verify execution-time declared-input isolation while separately reporting upstream authoring provenance as unknown or attested and never claims the pack itself was cleansed.

### Success Criteria

- Proposal and GTM pass through one released runner, assurance, and receipt implementation.
- The existing CLI and plugin can launch a clean run without treating the authoring conversation as decision authority.
- The verifier rejects hidden-input, stale-artifact, mutation, replay, model-identity, and incomplete-audit false positives.
- One synthetic proposal and one synthetic GTM proof pass through native/BYOK and customer-controlled driver classes with honest limitations.
- Installed release assets expose the same behavior validated in the source tree.
- Future host and Cloud adapters can conform without copying MDP policy.

### Scope Boundaries

**Included**

- Shared local runner and driver/profile boundaries, including a native/BYOK reference driver and a bounded adapter to existing headless-runner surfaces.
- Run-bundle and receipt v1 contracts and v0 migration.
- Proposal and GTM adapters.
- CLI, local stdio MCP, plugin skills, schemas, verifier, fixtures, tests, and operator documentation.
- Customer/BYOK proof, host conformance guidance, and the hosted adoption gate.

**Outside this product's identity**

- AI SDR, CRM, sequencer, enrichment provider, scraper, BI tool, proposal submission system, or generic automation platform.
- Source collection, table orchestration, outbound, CRM mutation, proposal submission, or customer credential custody.
- Portable container lifecycle or sandbox management for customer workloads.
- Deterministic model prose, provider-internal attestation, or semantic truth beyond supplied artifacts.

**Deferred until the hosted adoption gate**

- Generalized production MDP Cloud execution.
- Production auth, tenancy, billing, retention, residency, SLOs, and hosted signing infrastructure.

### Dependencies and Assumptions

- The CLI's portable pack digest remains the release identity primitive.
- Existing proposal source-intake, runner-audit, workdir, and receipt behavior remains migration input rather than discarded work.
- GTM Decision Input and source-binding contracts remain the upstream evidence boundary; MDP does not absorb collection.
- A host can truthfully report only controls it observes or enforces. Unknown host/provider properties remain unknown.
- MDP-maintained real-provider proof and evaluation calls require separate action-time approval, synthetic inputs, secure local credentials, and sanitized evidence. Customer-controlled production hosts own authorization and may execute under an explicit pre-approved run policy bound into the run envelope.

### Outstanding Questions

**Deferred to planning**

- Which existing Rust or JavaScript boundary becomes the canonical runner package while keeping every other surface thin?
- What exact schema names and compatibility rules replace or wrap the v0 receipt and proposal-specific manifests?
- Which assurance labels are derived from the assurance vector, and which dimensions remain visible independently?
- Which local containment mechanisms are portable across macOS and Linux, and which require explicit platform downgrade?
- Which receipt signing options are useful without implying trusted execution?

### Sources and Research

- `docs/run-receipts.md`
- `docs/headless-normalization-runners.md`
- `docs/native-api-normalization-runner.md`
- `docs/proposal-runner.md`
- `docs/orchid/decisions/2026-07-21-runner-receipts-and-context-isolation.md`
- `docs/orchid/decisions/2026-07-24-proposal-evidence-plane-and-local-mcp-threat-model.md`
- `docs/orchid/plans/2026-07-24-mdp-127-runner-support-matrix.md`
- `plugin/skills/mdp-gtm-brief/SKILL.md`
- `scripts/mdp-proposal-runner.mjs`
- `scripts/mdp-proposal-mcp-server.mjs`
- `scripts/lib/proposal-runner-contracts.mjs`
- `scripts/lib/proposal-runner-runtime.mjs`
- `cli/src/commands/requirements.rs`
- `cli/src/commands/run_receipt.rs`
