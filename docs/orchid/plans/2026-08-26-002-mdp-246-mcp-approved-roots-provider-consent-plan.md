---
title: MDP-246 MCP Approved Roots and Per-Call Provider Consent - Implementation Plan
type: security-hardening
date: 2026-08-26
topic: mcp-approved-roots-provider-consent
execution: orchid
artifact_contract: orchid-plan/v1
artifact_readiness: implementation-ready
linear_issues:
  - MDP-246
---

# MDP-246 MCP Approved Roots and Per-Call Provider Consent - Implementation Plan

## Context and current behavior

At planning base `5aaaf850b24b57622aca118da84cf02649380ab7`, `scripts/mdp-run-mcp-server.mjs` freezes `request_path` by descriptor and defends its public path against symlink, hard-link, mutation, replacement, and size attacks. It also requires a new `output_dir` and keeps native credentials out of deterministic runs. However, `canonicalExistingFile`, `canonicalExistingDir`, `canonicalOutputFile`, and `canonicalNewOutputDir` accept any caller-selected path that passes local shape checks. There is no startup-approved pack/input/output root policy.

`scripts/mdp-proposal-mcp-server.mjs::callProposalRun` similarly canonicalizes caller-selected pack, workdir, source, source-intake, source-audit, and mock-response paths without checking startup-configured roots. Its `runNode` includes `OPENAI_API_KEY` for a real run based on mode, and `scripts/mdp-run-mcp-server.mjs::childEnvironment` includes native credentials when the frozen request is generative. `MDP_ALLOW_NATIVE_MODEL_CALLS=1` and a credential are therefore process-wide capabilities, not consent bound to one frozen request and source set.

The existing runner does bind operator-approved source-intake entries to staged source hashes, but a ledger supplied beside untrusted inputs is not by itself an out-of-band authorization for provider execution.

## Objective, scope, out of scope, and decisions

Prevent every MCP file access outside explicit server-startup roots and prevent every provider-capable child spawn unless a tamper-evident, single-request consent record matches the exact frozen request and source digests.

Pinned decisions:

- Startup environment/config names four allowlists: pack roots, input/approval roots, work roots, and output roots. Missing configuration fails closed for the affected tool.
- Root containment is evaluated with real paths and path-component boundaries; files are opened with no-follow semantics and identity is rechecked immediately before use.
- New output paths require an approved existing parent and exclusive creation. Existing customer files are never replacement targets.
- Provider consent is an operator-created file in an approved consent root, not inline MCP data. It binds contract/version, provider, purpose, request SHA-256, ordered source/input SHA-256 values, output root, expiry, and a nonce. The adapter freezes and validates it like the request, consumes it once per server process, and refuses replay.
- The process-wide native flag and credential remain necessary capabilities but are never sufficient authorization.

Out of scope: hosted identity, a network consent service, new decision authority, broad runner refactors, or proving operator identity cryptographically beyond the local tamper-evident file boundary.

## Acceptance mapping

| Acceptance criterion | Implementation | Validation |
|---|---|---|
| Traversal, symlink, hard-link, rename, and TOCTOU escape attempts fail before provider invocation | Add a shared approved-root path policy that opens/freeze-checks files and validates output parents at the final use boundary. | Adversarial matrix for each input class, including path replacement between validation and spawn. |
| A credential or enable flag cannot authorize a call | Require a matching consumed consent record before adding provider env keys or calling the process supervisor. | Spawn spy with flag/key present and absent/invalid/expired/replayed consent; zero spawns on denial. |
| Approval binds exact frozen bytes and is not an ordinary tool argument | Hash frozen request/source bytes, load consent from an approved startup root, and compare the complete closed binding. | Tamper each field and byte source; inline consent arguments are rejected by closed schemas. |
| Output roots cannot overwrite customer data | Resolve an approved parent and use exclusive new-directory/file publication. | Existing file/dir, symlink, hard-link, rename, and concurrent creator cases remain unchanged. |
| Denials are bounded, private, and actionable | Return stable codes and root aliases/classes, never raw source data, secrets, or unnecessary absolute paths. | Assert bounded output and absence of canaries, absolute test roots, and source bodies. |
| No provider process/request begins on denial | Put consent/root gates before `superviseProcess`/`spawn` and expose a test-only spawn seam. | Process-spawn spy and no-network fixture prove count zero. |

## Affected files and symbols

- `scripts/lib/mcp-path-policy.mjs` (new): parse startup root allowlists; canonicalize roots; freeze approved files by descriptor; enforce component containment, file identity, link count, size, and approved output-parent creation.
- `scripts/lib/mcp-provider-consent.mjs` (new): strict consent schema, canonical digest construction, expiry/nonce/replay checks, and bounded denial codes.
- `scripts/mdp-run-mcp-server.mjs`: replace `canonicalExistingFile`, `canonicalExistingDir`, `canonicalOutputFile`, and `canonicalNewOutputDir` at MCP boundaries; extend the run tool with a consent reference identifier; gate `childEnvironment(true)` and both preflight/run spawn paths on the same validated frozen binding.
- `scripts/mdp-proposal-mcp-server.mjs`: route pack/workdir/source/ledger/mock paths through approved-root roles; gate `runNode(..., includeProviderCredential=true)` on consent validated against the frozen source/input set.
- `scripts/mdp-proposal-runner.mjs`: accept only the already-resolved consent binding metadata needed for the child audit record; do not perform MCP authorization or trust a sibling approval file.
- `scripts/test-run-mcp-server.mjs`: extend the existing request mutation, hard-link, output, credential, cancellation, and privacy cases with approved-root and consent matrices plus the spawn spy.
- `scripts/test-proposal-mcp-server.sh`: add pack/source/ledger/work/output escape, consent mismatch/replay/expiry, and provider-spawn-denial cases.
- `docs/orchid/decisions/2026-07-24-proposal-evidence-plane-and-local-mcp-threat-model.md`, `docs/run-receipts.md`, and `docs/proposal-runner.md`: document startup configuration, consent binding, stable denials, limitations, and migration.

Forbidden without replanning: `plugin/skills/**`, Rust decision/receipt schemas, hosted services, and provider endpoint expansion.

## Ordered implementation steps

1. Add closed startup root parsing with explicit role aliases and fail-closed diagnostics. Canonicalize each root once at startup and reject missing, symlinked, non-directory, duplicate, or overly broad roots.
2. Implement one descriptor-based approved-file primitive. It must check component containment, `O_NOFOLLOW`, regular-file type, link count where immutability is required, identity/size/timestamps before and after the bounded read, and root membership of the opened object.
3. Implement approved output creation from an already-opened/canonical approved parent. Require a new leaf, exclusive publication, and a final parent identity check; never follow or replace an existing leaf.
4. Define and validate the strict consent contract. Compute a canonical binding from provider, purpose, frozen request bytes, ordered frozen inputs/sources, output root, expiry, and nonce. Record only hashes and root aliases in child/audit metadata.
5. Gate provider-capable execution before the first preflight/runner spawn. Only after a matching unused consent is accepted may the adapter include native enablement and credentials. Mark the nonce consumed before spawn so retries require fresh consent.
6. Apply the same primitives to proposal pack, source, source-intake, source-audit, mock, workdir, and output paths. Preserve the source-intake authority checks as a separate inner gate.
7. Add stable denial codes and privacy-preserving messages. Keep transport errors subordinate to canonical CLI decision results once a run actually begins.
8. Add adversarial fixtures and a spawn-count seam. Exercise each attack before any real child or network-capable code and keep all fixtures synthetic/key-free.
9. Update security/operator documentation with configuration examples, the one-shot consent lifecycle, compatibility behavior, and explicit limitations.

## Tests and validation

Focused:

```bash
node --check scripts/lib/mcp-path-policy.mjs
node --check scripts/lib/mcp-provider-consent.mjs
node --check scripts/mdp-run-mcp-server.mjs
node --check scripts/mdp-proposal-mcp-server.mjs
node --test scripts/test-run-mcp-server.mjs
bash scripts/test-proposal-mcp-server.sh
```

Regression:

```bash
node --test scripts/test-run-conformance.mjs
node --test scripts/test-universal-native-parity.mjs
make validate
```

Manual synthetic proof: start each MCP adapter with temporary approved roots and key/flag canaries; run one valid consent-bound fixture; then repeat with path escape, replaced source, expired/replayed consent, and occupied output. Only the valid case may increment the spawn spy.

## Compatibility, migration, rollout, observability, and rollback

- This is intentionally fail-closed: existing MCP launches must add startup root configuration, and provider-capable calls must add one out-of-band consent record per request.
- Offline/deterministic calls still require roots but never provider consent or credentials.
- CLI invocation outside MCP is unchanged. MCP remains transport-only and adds no authority or assurance.
- Emit bounded denial codes and consent/root aliases for operator diagnosis; never emit absolute source paths, source bodies, or secrets.
- Roll back by reverting the PR and removing the new startup variables. No persisted customer schema migration is required.

## Risks and safety boundaries

- A root path prefix is not containment; use relative component checks against canonical opened objects.
- Consent validates exact bytes and purpose, not the truth of source content or the identity of the human who wrote the record.
- Consume-before-spawn prevents same-process replay; cross-process replay prevention is limited to nonce state available to the configured consent store and must be documented honestly.
- Root/consent denial must happen before any provider-capable process exists, including preflight helpers that could inherit credentials.
- Never log credentials, raw consent files, source bodies, or unredacted local paths.

## Blockers and readiness verdict

The shipped MDP-126 source-intake ledger and current descriptor-freezing tests provide the substrate. No live dependency or unresolved product decision blocks implementation. Exact boundaries, symbols, tests, migration, and rollback are defined.

**Verdict: `READY_TO_PIN`.**
