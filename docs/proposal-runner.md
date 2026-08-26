# Local Proposal Runner Surface

`scripts/mdp-proposal-runner.mjs` is the host-neutral local runner surface for proposal normalization. It wraps the existing native runner and CLI gates into one customer-controlled artifact chain:

```text
local sources
  -> mdp.source-intake.v0
  -> mdp.source-audit.v0
  -> mdp.native-normalize-request.v0
  -> mdp.prompt-output.v0 + mdp.runner-audit.v0
  -> mdp validate-prompt-output --source-audit
  -> mdp run-receipt --require-runner-audit
  -> optional fit/route review-support probes
```

This runner is also wrapped by a bundled local stdio MCP server. It is not a hosted or remote MCP service.

The native runner path is currently `recipe-only`. The MCP wrapper is transport, not verification. See the [canonical runner support matrix](headless-normalization-runners.md#canonical-runner-support-matrix); do not infer a verified integration from tool availability, a runner identifier, or schema-valid audit JSON.

Inspect the versioned proposal evidence contracts directly:

```bash
mdp --json schema source-intake
mdp --json schema source-audit
mdp --json schema native-normalize-request
mdp --json schema prompt-output
mdp --json schema runner-audit
mdp --json schema run-receipt
mdp --json schema proposal-run-manifest
mdp --json schema proposal-runner-result
mdp --json schema proposal-readiness-report
mdp --json schema proposal-mcp-run-result
```

`mdp --json capabilities` lists the same targets under
`proposal_evidence_contracts`, including the required-artifact purpose and
fixture/transport caveats. Scripts and MCP clients should consume these
contracts instead of inferring shapes from examples.

The proposal runner entrypoint orchestrates two stable internal modules under
`scripts/lib/`:

- `proposal-runner-contracts.mjs` owns runner contract identifiers, the
  host-neutral tool inventory, and the native prompt-output schema.
- `proposal-runner-runtime.mjs` owns JSON/hash helpers, secret-stripped
  subprocess environments, and the shared process wrapper.

Both modules ship with installed bundles and are covered by
`scripts/test-proposal-runner-modules.mjs`. They remain internal APIs;
downstream hosts should invoke the runner or MCP surface rather than treating
the modules as a separately versioned JavaScript package.

Published-release smoke tests verify the installed CLI schemas, all three
runner modules (including readiness reports), proposal skills, local runner
tool inventory, MCP tool schema, activation guardrails, and a freshly
initialized proposal pack. Source-tree tests alone are not release proof.

```bash
node scripts/mdp-proposal-runner.mjs tools
node scripts/mdp-proposal-mcp-server.mjs
```

The runner step names are:

- `mdp_intake_sources`
- `mdp_normalize_opportunity`
- `mdp_validate_normalization`
- `mdp_run_receipt`
- `mdp_review_proposal`

When `--clean-run-v1` is selected, `mdp_clean_run_v1` replaces the legacy
`mdp_validate_normalization` and `mdp_run_receipt` steps. The Rust CLI is the
sole authority for the v1 validation artifact, hashes, terminal state, receipt,
and canonical authority block.

The stdio MCP server exposes two callable MCP tools:

- `mdp_proposal_tools` — read-only inspection of the runner boundary contract.
- `mdp_proposal_run` — file/path-only wrapper around `mdp-proposal-runner.mjs run`.

`mdp_proposal_run` intentionally accepts local source file paths and source-audit paths, not raw chat text. MCP transport is only the call boundary; audit-grade status still comes from a valid runner audit plus `mdp run-receipt --require-runner-audit`.

The MCP tool accepts `clean_run_v1: true` with a required
`pack_release_id`. It returns the CLI-owned `authority_contract`,
`terminal_state`, and `canonical_authority` fields without recalculating them.

The MCP adapter hardens that local boundary:

- it canonicalizes pack, workdir, source, intake, audit, mock-response, CLI, and native-runner paths; rejects missing/wrong-type paths and final-component symlinks; and never interprets a path as a shell command;
- it runs the proposal runner with a minimal explicit environment allowlist rather than the MCP host's full environment;
- it bounds child output and applies one parent `timeout_ms` deadline (60,000ms by default, 300,000ms maximum); every child receives only the remaining budget, timed-out runs are terminated, and exit status `124` is reported;
- it redacts credential-shaped values from returned diagnostics; and
- it returns the strict `mdp.proposal-mcp-run-result.v0` envelope with top-level `mode`, `decision`, `audit_grade_eligible`, `runner_assurance`, timeout/termination state, bounded diagnostics, and environment-policy metadata.

Host adapters should use the canonical 60,000ms recommendation. A transport
timeout is a compatibility boundary: it may truncate the request runtime after
the fixed finalization reserve, but it cannot extend that inner policy.

Clients should branch on those machine-readable fields, not parse the text
content. Setting `require_audit_grade: true` fails closed unless this invocation
returns `decision: "audit-grade"` and `audit_grade_eligible: true`; dry-run,
mock, advisory, blocked, malformed, timed-out, and failed runner results return
an MCP tool error.

## Skill Routing Decision

Proposal-facing agents use the decision tree packaged at
`plugin/skills/mdp-proposal-review/references/evidence-path.md`:

1. require explicit local source files and human approval of their exact hashes;
2. if audit-grade is requested, require a callable local runner/MCP plus a
   schema-accepted native/headless boundary;
3. never silently replace a missing evidence path with same-chat normalization;
4. call the result audit-grade only from the current receipt and runner
   assurance; otherwise return advisory or blocked; and
5. report the smallest exact source-checkout or installed-plugin command needed
   to cross the missing gate.

This preserves a usable ambient review path when the operator explicitly accepts
advisory assurance, while keeping “is this audit-grade?” a receipt-based answer.

## Source Approval Precondition

A path is not approval. Under the [proposal source import and approval contract](orchid/decisions/2026-07-24-proposal-source-import-and-approval-contract.md), chat, pasted text, email/Drive exports, PDFs, OCR, and importer output begin as unblessed input. A maintained importer may create a bounded local candidate, but only a human operator may approve its exact hash, pack source ID, privacy class, and review purpose. A `mdp.source-audit.v0` remains a citation ledger rather than an approval record.

The runner always writes `artifacts/source-intake.json`. A first dry run creates
candidate entries with the exact staged hash, source ID, source kind, privacy
class, origin, truncation state, and bound source-audit refs. A human may approve
those exact candidates outside the runner. A real native run requires that
approved ledger through `--source-intake`; the runner rechecks every binding and
the receipt hashes the ledger as a `source-intake` artifact. Agents and
importers never self-approve candidates.

## What It Does

The runner:

- stages supplied text, Markdown, CSV, JSON, or YAML source files in a local run directory;
- rejects source symlinks, unsafe source IDs, unsafe workdir ownership/modes, and stale workdirs without an exact ownership manifest;
- creates `artifacts/source-intake.json` and binds every staged source to matching source-audit snippet bytes;
- atomically writes `.mdp-proposal-run.json`, refuses concurrent/partial reuse, and read-backs terminal artifact hashes before reporting;
- preserves a supplied `mdp.source-audit.v0` or creates a bounded source-audit ledger for staged text;
- builds a single-user-message `mdp.native-normalize-request.v0` with only the prompt-declared payload fields: `raw_opportunity`, `existing_pack_context`, `source_audit`, and `source_kind`;
- calls `scripts/mdp-native-normalize-openai.mjs`;
- runs `mdp validate-prompt-output --source-audit`;
- runs `mdp run-receipt --runner-audit ... --require-runner-audit`;
- optionally runs local `fit` and `route` probes for review support.
- writes `artifacts/proposal-readiness-report.json` with deterministic blockers,
  warnings, and SHA-256 evidence anchors.

It does not parse PDFs, prove OCR quality, browse, enrich, scrape, read `.env` files, create API keys, write proposals, submit proposals, approve compliance, or prove semantic truth beyond the supplied artifacts.

The readiness report is a machine-readable review queue, not a truth score.
`confidence.level` describes how completely findings are anchored to persisted
artifacts. It does not estimate whether a proposal claim is true. The
`run-receipt` decision remains the audit-grade gate; readiness never overrides
a blocked or advisory receipt.

## Canonical v1 Compatibility Route

Use the opt-in compatibility route to finalize an already-generated proposal
normalization through the canonical Rust `mdp run` runtime:

```bash
node scripts/mdp-proposal-runner.mjs run \
  --pack <pack-root> \
  --pack-release-id <immutable-release-id> \
  --clean-run-v1 \
  --workdir <new-customer-controlled-run-dir> \
  --source <approved-text-export.txt> \
  --source-intake <operator-approved-source-intake.json> \
  --source-id <id-from-pack-.mdp-sources-yaml> \
  --source-kind private-scratch-opportunity \
  --model <openai-model-id>
```

The JavaScript runner still performs the existing proposal-specific source
staging and native model invocation. It then writes
`artifacts/run-request-v1.json` containing file-oriented declared inputs and
invokes:

```bash
mdp --json run \
  --request <workdir>/artifacts/run-request-v1.json \
  --out-dir <workdir>/artifacts/clean-run-v1
```

`<workdir>` must be external to `<pack-root>` and customer-controlled. The
clean-run output directory may not equal or descend from the active pack;
canonical CLI and MCP preflight rejects that relationship before writing.
Generated evidence belongs to the control plane, and validation does not
delete older in-pack artifacts; move them manually to the external workdir.

The request uses the deterministic `proposal` / `validate-existing-output`
operation. JavaScript does not set v1 hashes, assurance, terminal state, or
receipt values. Consumers must treat `canonical_run` and
`canonical_authority` in `mdp.proposal-runner-result.v1` as authoritative. The
legacy `decision` field is only a compatibility projection, and
`audit_grade_eligible` remains false on this route.

This boundary matters: the v1 artifacts prove deterministic validation and
hash binding of the exact declared files handed to `mdp run`. They do not prove
that the Rust runtime performed the upstream model inference, that the model
received no undeclared provider context, or that the JavaScript/native runner
was isolated from ambient host state. Until the canonical runtime supports a
generative driver, do not label this route end-to-end isolated or audit-grade.
The default invocation without `--clean-run-v1` remains the unchanged v0
proposal workflow.

## Offline Dry Run

Use dry-run to check request hygiene without an API key or model call:

```bash
node scripts/mdp-proposal-runner.mjs run \
  --pack <pack-root> \
  --workdir <customer-controlled-run-dir> \
  --source <approved-text-export.txt> \
  --source-id <id-from-pack-.mdp-sources-yaml> \
  --source-kind private-scratch-opportunity \
  --dry-run
```

Dry-run writes a request and native-runner preview, but it does not produce prompt output, runner audit, validation, receipt, or review artifacts. It is never audit-grade. It also writes candidate-only `source-intake.json` and `.mdp-proposal-workdir.json`. To reuse that directory, pass the manifest's exact `workdir_id` with `--reuse-workdir-id`; there is no generic allow-existing mode.

Every invocation also owns `.mdp-proposal-run.json` under an exclusive
`.mdp-proposal-run.lock`. The run manifest starts as `in-progress`, ends as
`completed` or `blocked`, and records the run ID, owner/workdir ID, bounded
command summary, timestamps, decision, and hashes for files under `artifacts/`
and `sources/`. Both start and terminal writes are atomic and read back before
the runner reports. Reuse requires both the ownership manifest and a matching
terminal run manifest. An in-progress manifest, stale lock, unknown manifest,
or interrupted run fails closed; the runner never deletes those states.

To approve real input, inspect the candidate preview and hashes, have a human
change each accepted entry to `state: "approved"` and
`approval_class: "operator-approved"`, and add an `approval` object whose
`artifact_sha256`, purpose, operator, decision, and timestamp bind that exact
candidate. Then pass the resulting file with `--source-intake`.

## Offline Mock Test

Use mock mode only for CI, demos, and fixture validation:

```bash
node scripts/mdp-proposal-runner.mjs run \
  --pack <pack-root> \
  --workdir <customer-controlled-run-dir> \
  --source-audit <source-audit.json> \
  --source <approved-text-export.txt> \
  --source-kind synthetic-example \
  --model gpt-test \
  --mock-response <openai-response-fixture.json>
```

Mock mode intentionally writes native-runner audit evidence with `mock_response: true`, `isolated_invocation: false`, and `stateless_request: false`. `mdp run-receipt --require-runner-audit` must block this path. Treat that blocked receipt as success for fixture safety and failure for production assurance.

The default public video uses this synthetic mock path. Label it mock/non-audit-grade. Replace that label only for a real invocation whose own required runner-audit receipt is audit-grade; do not reuse fixture artifacts as proof.

Before recording or sharing that walkthrough, use the
[Proposal Demo Go/No-Go Gate](proposal-demo-go-no-go.md). It defines the
red/yellow/green evidence states, exact safe narration for the mock path,
screen-recording redactions, and the human-owned final go/no-go decision.

Validate the local surface with:

```bash
make validate-proposal-runner
make validate-proposal-evidence-harness
make validate-proposal-mcp
```

The [deterministic proposal evidence harness](proposal-evidence-harness.md)
exercises positive contract acceptance plus negative ambient/raw-chat,
mock/hash, injection, citation-tamper, OCR-summary, unsupported-proof, and
missing-evidence cases without a provider call. Its positive receipts are
fixture-only contract proof, never production invocation evidence.

## Real Native Run

For a real normalization call, use an explicit model and provide `OPENAI_API_KEY` from the operator's secure local environment:

```bash
OPENAI_API_KEY=... \
node scripts/mdp-proposal-runner.mjs run \
  --pack <pack-root> \
  --workdir <customer-controlled-run-dir> \
  --source <approved-text-export.txt> \
  --source-intake <operator-approved-source-intake.json> \
  --source-id <id-from-pack-.mdp-sources-yaml> \
  --source-kind private-scratch-opportunity \
  --model <openai-model-id> \
  --require-audit-grade
```

Only call the result audit-grade when the final `proposal-runner-result.json` reports:

- `mode: "native"`;
- `decision: "audit-grade"`;
- `audit_grade_eligible: true`;
- `runner_assurance: "stateless-api-verified"` or another schema-accepted headless-verified mode.

If the receipt is `blocked` or `advisory`, keep the proposal review in gaps/questions and do not present it as isolated or audit-grade.

## Installed Plugin Path

Source checkouts use:

```bash
node scripts/mdp-proposal-runner.mjs ...
```

Installed Pluxx bundles package repo scripts, so hosts can use:

```bash
node "${PLUGIN_ROOT}/scripts/mdp-proposal-runner.mjs" ...
node "${PLUGIN_ROOT}/scripts/mdp-proposal-mcp-server.mjs"
```

The documented installer still installs release assets, not the current `main` branch:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

A merged runner change is shipped to installed users only after a release containing that commit is published and the installer smoke test passes.
- Host adapters should use the canonical 60,000ms recommendation. A transport timeout is a compatibility boundary: it may truncate the request’s runtime policy after the fixed finalization reserve, but it cannot extend that inner policy.
## MCP startup roots and native consent

The proposal MCP adapter is local-only and fail-closed. Configure
`MDP_MCP_PACK_ROOTS`, `MDP_MCP_INPUT_ROOTS`, `MDP_MCP_APPROVAL_ROOTS`,
`MDP_MCP_WORK_ROOTS`, `MDP_MCP_OUTPUT_ROOTS`, and `MDP_MCP_CONSENT_ROOTS` with
existing real directories before use. Pack, source, approval ledger, mock, and
work paths must remain within their corresponding roots.

Real native runs also require a one-shot consent record selected by its opaque
identifier. Consent binds provider, purpose, frozen request/source digests,
output root, expiry, and nonce; it is consumed before the native child is
created. Dry-run and mock runs still require approved roots but do not receive
provider credentials. Diagnostics remain bounded and omit secrets, source
bodies, and absolute paths.
