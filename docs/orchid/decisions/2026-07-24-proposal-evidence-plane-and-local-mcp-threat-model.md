# Proposal Evidence Plane And Local MCP Threat Model

Date: 2026-07-24
Issue: MDP-125
Status: proposed; security-lens and human acceptance required
Scope: local proposal normalization runner, local stdio MCP adapter, evidence artifacts, and public/client-demo claims

## Overview

Message Decision Packs (MDP) stores and validates local decision context. It is not a model host, proposal submission system, compliance certifier, credential manager, or hosted MCP service.

The proposal workflow uses a conversational host as a control plane and local files plus deterministic receipts as an evidence plane:

```text
operator-approved local sources
  -> bounded source staging and mdp.source-audit.v0
  -> fresh native/headless normalization request
  -> mdp.prompt-output.v0
  -> validate-prompt-output --source-audit
  -> mdp.runner-audit.v0
  -> run-receipt --require-runner-audit
  -> deterministic fit / route / proof / readable review
```

An accepted receipt can show that one invocation used the declared artifact chain and satisfied machine-checkable isolation assertions. It does not establish the truth of supplied documents, prove semantic entailment for every statement, certify compliance, approve a proposal, or prove that the surrounding host and operating system were uncompromised.

This model reviews:

- `scripts/mdp-proposal-runner.mjs`
- `scripts/mdp-proposal-mcp-server.mjs`
- `scripts/mdp-native-normalize-openai.mjs`
- `cli/src/commands/prompt_output.rs`
- `cli/src/commands/run_receipt.rs`
- `docs/orchid/decisions/2026-07-21-runner-receipts-and-context-isolation.md`

The baseline reviewed was `origin/main` at `5351a40`. The parallel runner-support decision is MDP-127 / public PR #126.

## Assets And Security Objectives

| Asset | Security objective |
| --- | --- |
| Supplied proposal/RFP material | Remain in operator-controlled local storage except for the approved model request; never enter public fixtures, logs, or commits. |
| Source identity and approval | Distinguish approved files from ambient chat, generated scratch, unreviewed imports, and stale artifacts. |
| Prompt-visible input | Contain only the declared prompt package and approved source payload for the current invocation. |
| Provider credentials and environment | Never appear in model input, runner output, MCP responses, error messages, committed artifacts, or public demos. |
| Prompt output and review findings | Remain bound to the exact prompt, source audit, validation result, and runner audit. |
| Runner and receipt assurance | Fail closed when isolation, tool denial, hashes, source audit, or fixture status is missing or inconsistent. |
| Workdir integrity | Prevent path escape, symlink following, stale reuse, partial writes, and cross-run substitution. |
| Public language | Describe mocks as mocks, receipts as per-run evidence, and MDP as review support. |

## Actors And Input Classes

### Trusted or conditionally trusted actors

- **Operator:** selects sources, pack, workdir, model, and mock versus real mode. Operator mistakes remain in scope.
- **MDP CLI:** trusted to parse artifacts, compute hashes, validate contracts, and return deterministic decisions.
- **Maintained runner and MCP bundle:** trusted only when installed files are reviewed versions and executable overrides are not attacker-controlled.
- **Model provider:** trusted to protect transport and process the request, but not to make source text truthful or resist every prompt injection.

### Untrusted or attacker-controlled inputs

- Supplied text, CSV, Markdown, JSON, and YAML contents.
- File names, paths, symlinks, prebuilt source-audit JSON, model output, provider error bodies, and MCP JSON-RPC arguments.
- Prompt injection inside legitimate proposal documents.
- Mock/provider response files and artifacts copied from another run.
- Pack content imported or edited by an untrusted party.

### Developer- or environment-controlled inputs

- Installed scripts and the `mdp` binary.
- Provider credentials, endpoint overrides, executable overrides, `PATH`, process environment, and filesystem permissions.
- Public docs, skills, templates, examples, release assets, and CI fixtures.

Environment-controlled inputs are not automatically safe. A compromised shell, malicious wrapper, or unsafe executable override invalidates the local runner trust assumption.

## Trust Boundaries And Assumptions

### B1. Conversation host to local MCP

The host may contain ambient chat, memory, tools, or hidden instructions. The MCP accepts paths rather than raw source/chat text, reducing accidental copying. This is not provenance proof: a host can write ambient text to a file and pass the path.

**Invariant:** audit-grade intake consumes an operator-approved source record, not merely a path supplied by the current agent.

### B2. Local MCP to proposal runner

The MCP converts JSON-RPC arguments into runner arguments, inherits the process environment, and currently accepts executable overrides.

**Invariant:** an untrusted MCP caller cannot choose executable code, redirect credentials, expand the environment exposed to children, or receive sensitive stderr.

### B3. Source filesystem to workdir

The runner resolves source/workdir paths, copies files, and writes fixed artifact names. Current path resolution does not establish ownership, reject symlinks, or make an existing workdir transactionally exclusive.

**Invariant:** every real run owns a fresh directory inside an approved root; paths cannot escape through traversal, symlinks, races, or stale content.

### B4. Workdir to model request

The runner builds one declared payload and rejects prior-response/conversation fields and tools. Supplied source text and source-audit snippets remain untrusted model input.

**Invariant:** the request contains only approved bytes plus maintained prompt/pack context; source content is data, never authority to change tools, scope, output contract, or evidence rules.

### B5. Provider response to validation

Structured output constrains shape. `validate-prompt-output` checks prompt/value contracts, declared references, audited source IDs, and snippets. It does not prove semantic entailment.

**Invariant:** unsupported or ambiguous material becomes a gap, rejected claim, or human-review item.

### B6. Validation artifacts to receipt

`run-receipt` hashes and cross-checks prompt output, validation, source audit, and runner audit. It blocks missing, malformed, mismatched, resumed, tool-enabled, mock, demo, fixture, or synthetic evidence.

**Invariant:** a receipt is valid only for its exact immutable artifact set and current run.

### B7. Local artifacts to public/client communication

Artifacts may contain private excerpts, absolute paths, provider identifiers, errors, or sensitive metadata.

**Invariant:** public demos use synthetic fixtures only. Client material and raw operational artifacts are not committed, pasted into issues, or used in public media.

## Security Invariants

1. Ambient conversation is never supplied evidence without separate operator approval.
2. A real run uses a fresh, owned workdir and fails on existing, symlinked, escaped, or concurrently owned paths.
3. Staged bytes, source ledger, model-visible bytes, validation, and receipt bind to the same run.
4. MCP callers cannot select arbitrary executables or inherit unnecessary secrets.
5. Provider credentials never appear in request artifacts, errors, MCP results, logs, or public output.
6. Source instructions cannot enable tools, resume context, change the prompt contract, or authorize unsupported claims.
7. Mock, fixture, demo, dry-run, synthetic-model, or fabricated audits never produce audit-grade status.
8. Per-run assurance does not promote an integration to verified support.
9. Missing proof, source approval, freshness, or hash agreement blocks rather than degrades silently.
10. Human review separates supplied facts, validated bindings, hypotheses, gaps, and unsupported claims.

## Attack Surface, Controls, And Gaps

| ID | Attack or failure story | Existing control | Missing control / required action | Demo disposition |
| --- | --- | --- | --- | --- |
| T1 | Ambient chat is written to a file and passed as source. | MCP rejects raw chat/source-text arguments; native request rejects conversation resume. | Add unblessed → candidate → operator-approved states bound to staged hashes (MDP-141, MDP-126). | **Blocks real/audit-grade claim.** Mock video requires committed synthetic fixtures. |
| T2 | A document injects instructions to ignore evidence rules, browse, or invent compliance claims. | Tools disabled; single payload and strict schema; prompt prohibits invention; downstream checks reject unsupported structures. | Add adversarial fixtures and instruction/data separation tests (MDP-128, MDP-139). Retain human review because validation is not semantic entailment. | Mock walkthrough allowed. **Blocks claims that normalization proves truth or safety.** |
| T3 | A source symlink with an allowed extension points to a secret or unapproved file. | Extension allowlist and byte bound. | Verify real paths, reject symlinks/special files, require approved-root containment, bind exact bytes (MDP-126). | **Blocks real client-source runs.** |
| T4 | Workdir or artifact child symlinks cause writes outside the run directory. | Non-empty workdirs fail unless `--allow-existing`. | Add owned-root containment, `lstat`/realpath checks, safe permissions, and no-follow writes (MDP-126). | **Blocks real client-source runs.** |
| T5 | `--allow-existing` reuses stale output or a concurrent process replaces artifacts. | Receipt hashes catch many mismatches; expected artifacts are rewritten. | Add run manifest, exclusive lock, unique ID, atomic writes, and completion state (MDP-140). Constrain existing-dir mode for audit-grade runs. | **Blocks audit-grade language.** |
| T6 | Source bytes change between read, copy, ledger, and request construction. | Runner records a source hash; receipt hashes the source audit. | Stage once from approved bytes, hash staged bytes, include hashes in a first-class ledger and receipt (MDP-124, MDP-126). | **Blocks audit-grade claim without immutability proof.** |
| T7 | A malicious/stale prebuilt source audit injects snippets or references different material. | Contract checked before request; later validation checks refs, source IDs, snippets; receipt hashes ledger. | Validate the complete ledger before model invocation and require approval/hash bindings (MDP-124, MDP-126). | **Blocks real/audit-grade claim.** |
| T8 | MCP caller supplies a malicious runner or CLI executable that inherits environment values. | Child processes run without a shell, limiting argument injection. | Remove executable overrides from MCP or require maintained allowlisted digests; pass a minimal child environment (MDP-123). | **Blocks untrusted MCP calls with real credentials.** |
| T9 | Endpoint override redirects the credential and proposal payload. | Default is the official provider endpoint; override is local environment rather than source text. | Treat endpoint as trusted config, validate HTTPS/allowlist, record non-secret endpoint identity (MDP-123, MDP-135). | **Blocks real runs when endpoint provenance is unknown.** |
| T10 | Provider/wrapper/CLI errors include source text, paths, headers, or environment values; MCP returns stderr. | MCP truncates stderr; public-fixture rules prohibit secrets. | Redact errors, return stable codes, minimize environment, keep sensitive stderr local, sanitize exports (MDP-123, MDP-130). | **Blocks real customer data in recorded/public demos.** |
| T11 | Mock response or fabricated audit is labeled as real. | Mock audit marks isolation false; receipt blocks mock/demo/fixture/synthetic markers and missing fields. | Maintain negative corpus and require machine-observed support proof (MDP-127, MDP-128, MDP-149). | Mock allowed only when labeled non-audit-grade and receipt remains blocked. |
| T12 | Output, validation, source audit, or runner audit is copied from another run. | Receipt checks contracts and cross-checks prompt-output/source-audit hashes and runner output hash. | Add run ID, lifecycle state, pack/prompt/source-ledger hashes, and replay policy (MDP-124, MDP-140). | **Blocks audit-grade claim if ownership/freshness is unknown.** |
| T13 | A valid receipt is presented after pack, prompt, or review rules change. | Receipt records prompt ID and artifact hashes; validation reads current pack. | Bind pack identity, prompt hash, CLI/runtime version, and run manifest (MDP-124, MDP-133, MDP-140). | **Blocks replayability claims across changed packs.** |
| T14 | Structurally valid output invents a requirement, deadline, certification, price, or past performance. | Prompt prohibits invention; refs/snippets and proof validation surface unsupported material. | Add semantic spot checks, structured findings/confidence anchors, and mandatory human review (MDP-128, MDP-143). | **Always blocks replacement-of-review claims.** |
| T15 | Docs/narration call recipe-only, mock, or per-run assurance a verified integration or compliance guarantee. | `AGENTS.md` guardrails; docs distinguish advisory/blocked/audit-grade; MDP-127 defines states. | Add claim linter, concepts vocabulary, and video go/no-go checklist (MDP-134, MDP-138, MDP-142). | **Blocks publication until corrected.** |
| T16 | Private artifacts are committed, attached to issues, or included in release/demo assets. | Repo requires synthetic/sanitized artifacts and scratch outside commits. | Add managed-artifact/release scans and retention guidance (MDP-130, MDP-142). | **Blocks publication and release.** |
| T17 | An MCP caller sends an oversized JSON-RPC line or an unbounded list of sources, exhausting memory, CPU, child buffers, or disk. | Each source excerpt has a byte limit and child stdout/stderr buffers are capped. | Bound JSON-RPC message size, source count, total staged bytes, artifact bytes, and run duration; return a stable resource-limit error (MDP-123, MDP-126). | Does not block a bounded synthetic walkthrough; blocks treating the MCP surface as safe for untrusted bulk input. |
| T18 | An unvalidated or attacker-edited pack changes prompt instructions, source declarations, or output rules before the model call. | Runner fixes the prompt ID and later CLI validation reads the selected pack. | Validate the pack before model invocation, require approved pack identity, and bind manifest/prompt hashes into run state and receipt (MDP-124, MDP-129, MDP-140). | **Blocks real/audit-grade claim when pack provenance is unknown.** |

## Existing Controls That Reduce Risk

- MCP accepts file paths rather than raw chat text and says transport alone is not audit-grade.
- Native requests reject prior responses, conversations, extra instructions, and enabled tools.
- Provider requests use strict structured output, `store: false`, and no tools.
- Real mode requires source text instead of source-audit-only model input.
- Source excerpts are bounded and unsupported extensions are rejected.
- Prompt validation checks prompt identity, declared references, value contracts, source IDs, refs, and snippets.
- Receipts bind validation and runner audit to prompt output and bind validation to the source audit.
- Runner-specific evidence fails closed and requires zero observed tool calls.
- Demo, fixture, mock, synthetic-model, resumed-context, missing-audit, and mismatched-hash cases block.
- Public proposal guidance prohibits invented certifications, compliance status, past performance, pricing, and customer fixtures.

These controls do not close source provenance, path ownership, environment minimization, atomic run ownership, or semantic-truth gaps.

## Client-Video And Demo Gate

### Green: safe public walkthrough

A public walkthrough may proceed only when:

- Every input and visible artifact is synthetic or explicitly sanitized.
- Narration calls the run **mock**, **fixture**, or **non-audit-grade**.
- The displayed receipt remains blocked or clearly non-audit-grade.
- No real credential, customer source, private path, provider error, or operational artifact appears.
- No claim is made about verified integration, compliance, CUI readiness, proposal approval, legal/procurement approval, or automated writing/submission.

### Yellow: real runner proof with synthetic sources

After MDP-127 is accepted and MDP-149 succeeds, a sanitized recording may show a real provider invocation using synthetic source material. It may describe the exact invocation and the maintained runner state supported by the accepted matrix, but it must not be presented as proof that real client-source intake, MCP exposure, workdir reuse, or private-artifact handling is production-ready. No credential, raw provider response, private path, or unsanitized operational artifact may appear.

### Red: block real/audit-grade client proof

Do not describe a real client-source run as audit-grade until:

1. MDP-127 is accepted and MDP-149 records a sanitized, machine-observed real runner chain.
2. MDP-141/MDP-126 provide source approval, safe path containment, symlink rejection, and source-byte hash binding.
3. MDP-123 pins executables, minimizes environment, constrains endpoints, and redacts MCP errors.
4. MDP-140 provides fresh workdir ownership, atomic run state, and stale/replay protection.
5. The exact invocation returns `decision: "audit-grade"` with valid runner assurance and matching hashes.
6. Human review confirms material findings are supported and narration stays inside the receipt claim.

MDP-138 should encode these as the final go/no-go checklist.

## Deferred And Out Of Scope

The following remain deferred while MDP is local and single-operator:

- Hosted multi-tenant MCP authentication, authorization, tenant isolation, rate limiting, and remote sessions.
- Formal CMMC/NIST certification, CUI authorization, legal advice, procurement approval, or records certification.
- Protection against a fully compromised OS, malicious administrator, compromised provider, or stolen operator account.
- Parsing/sandboxing arbitrary PDF/DOC/OCR formats in the current text-focused runner.
- Proposal submission, signatures, approval workflow, CRM writes, enrichment, scraping, sequencing, or autonomous execution.

If these boundaries change, revise this model before implementation or public claims expand.

## Severity Calibration

### Critical

Direct credential or private-material exposure outside the operator boundary with little additional action.

- MCP-controlled executable runs attacker code with provider credentials inherited.
- An attacker-controlled or silently substituted endpoint sends credentials and proposal content to an unintended service. An explicitly approved operator gateway is a separate trusted deployment choice, not automatically a vulnerability.
- Path/symlink escape reliably overwrites a high-value file and leads to code execution.

### High

Forged audit-grade evidence or private material escaping the intended run boundary.

- Symlinked source reads a secret into the model request.
- Stale/cross-run artifacts yield an accepted receipt for the wrong source or prompt.
- Customer material or credentials appear in MCP errors, releases, or public media.
- Ambient chat is treated as approved evidence without an approval/hash record.

### Medium

Misleading review or incomplete fail-closed behavior without direct secret exposure or a forged accepted chain.

- Prompt injection produces unsupported findings structural validation cannot semantically disprove.
- Absolute paths or non-secret operational details leak into a private host conversation.
- Partial writes cause denial of service or manual cleanup without a false receipt.

### Low

Diagnostics or documentation drift that does not cross a meaningful trust boundary.

- Mock narration is inconsistent but the receipt is still visibly blocked.
- A malformed local fixture produces an unclear error without leaking source content.

## Follow-Up Ownership

| Control area | Owning issue(s) |
| --- | --- |
| Runner support state and real sanitized proof | MDP-127, MDP-149 |
| Evidence schemas and receipt bindings | MDP-124, MDP-133 |
| Source approval, path safety, symlink rejection | MDP-126, MDP-141 |
| Atomic run state, workdir lock, stale/replay protection | MDP-140 |
| MCP executable pinning, minimal environment, endpoint policy, error redaction | MDP-123, MDP-135 |
| MCP/source resource limits | MDP-123, MDP-126 |
| Deterministic harness and adversarial coverage | MDP-128, MDP-139 |
| Structured findings and human confidence anchors | MDP-143 |
| Public vocabulary, claim linting, and video gate | MDP-134, MDP-138, MDP-142 |
| Installed artifact and release safety | MDP-130, MDP-132 |

## Required Review Decisions

Human acceptance should confirm:

1. The current public video stays synthetic and non-audit-grade.
2. Real client-source audit-grade proof is blocked on the red-gate controls.
3. MCP callers cannot choose arbitrary executables or receive unsanitized child stderr when real credentials are present.
4. File paths alone are not source approval; operator-approved, hash-bound intake is required.
5. Structural validation and receipts do not replace human review of material proposal claims.

Until accepted, this is a proposed security boundary, not a compliance statement or production authorization.
