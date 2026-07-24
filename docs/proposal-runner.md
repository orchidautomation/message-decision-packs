# Local Proposal Runner Surface

`scripts/mdp-proposal-runner.mjs` is the host-neutral local runner surface for proposal normalization. It wraps the existing native runner and CLI gates into one customer-controlled artifact chain:

```text
local sources
  -> mdp.source-audit.v0
  -> mdp.native-normalize-request.v0
  -> mdp.prompt-output.v0 + mdp.runner-audit.v0
  -> mdp validate-prompt-output --source-audit
  -> mdp run-receipt --require-runner-audit
  -> optional fit/route review-support probes
```

This runner is also wrapped by a bundled local stdio MCP server. It is not a hosted or remote MCP service.

The native runner path is currently `recipe-only`. The MCP wrapper is transport, not verification. See the [canonical runner support matrix](headless-normalization-runners.md#canonical-runner-support-matrix); do not infer a verified integration from tool availability, a runner identifier, or schema-valid audit JSON.

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

The stdio MCP server exposes two callable MCP tools:

- `mdp_proposal_tools` — read-only inspection of the runner boundary contract.
- `mdp_proposal_run` — file/path-only wrapper around `mdp-proposal-runner.mjs run`.

`mdp_proposal_run` intentionally accepts local source file paths and source-audit paths, not raw chat text. MCP transport is only the call boundary; audit-grade status still comes from a valid runner audit plus `mdp run-receipt --require-runner-audit`.

## What It Does

The runner:

- stages supplied text, Markdown, CSV, JSON, or YAML source files in a local run directory;
- preserves a supplied `mdp.source-audit.v0` or creates a bounded source-audit ledger for staged text;
- builds a single-user-message `mdp.native-normalize-request.v0` with only the prompt-declared payload fields: `raw_opportunity`, `existing_pack_context`, `source_audit`, and `source_kind`;
- calls `scripts/mdp-native-normalize-openai.mjs`;
- runs `mdp validate-prompt-output --source-audit`;
- runs `mdp run-receipt --runner-audit ... --require-runner-audit`;
- optionally runs local `fit` and `route` probes for review support.

It does not parse PDFs, prove OCR quality, browse, enrich, scrape, read `.env` files, create API keys, write proposals, submit proposals, approve compliance, or prove semantic truth beyond the supplied artifacts.

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

Dry-run writes a request and native-runner preview, but it does not produce prompt output, runner audit, validation, receipt, or review artifacts. It is never audit-grade.

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

Validate the local surface with:

```bash
make validate-proposal-runner
make validate-proposal-mcp
```

## Real Native Run

For a real normalization call, use an explicit model and provide `OPENAI_API_KEY` from the operator's secure local environment:

```bash
OPENAI_API_KEY=... \
node scripts/mdp-proposal-runner.mjs run \
  --pack <pack-root> \
  --workdir <customer-controlled-run-dir> \
  --source <approved-text-export.txt> \
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
