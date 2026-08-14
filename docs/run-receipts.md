# MDP Run Receipts

`mdp run-receipt` creates the legacy local `mdp.run-receipt.v0` artifact for workflows where an agent host or runner normalized messy source material before deterministic MDP checks ran. The unified clean-context runtime uses the v1 run contracts described below for deterministic work and for one selected declared normalization, generation, or review step. The v0 command remains available for proposal compatibility.

Use v0 only to preserve or inspect an existing proposal/document-review flow,
especially when a PDF/doc extraction step produced a `mdp.source-audit.v0`
ledger. Its historical `audit-grade` decision is not the v1 assurance vector
and must not be used as proof of the new clean-run boundary.

A receipt assurance value describes one invocation; it is not a public integration-support claim. Consult the [canonical runner support matrix](headless-normalization-runners.md#canonical-runner-support-matrix) before describing a runner as verified. All currently documented named runners are recipe-only, while `custom-headless` is unsupported and mock/demo evidence is fixture/mock-only.

## Unified Execution Contracts (v1)

The v1 family separates what an operator asked to run, the immutable bytes the runtime staged, what a driver claims happened, the receipt MDP issued, and what a later verifier could recompute. Each contract is a closed Draft 2020-12 JSON Schema: unknown fields are rejected instead of becoming unaudited side channels.

| Contract | CLI schema target | Authority |
| --- | --- | --- |
| `mdp.run-request.v1` | `run-request-v1` | Operator intent and local source paths before safe staging. Paths are not evidence that the staged bytes matched. |
| `mdp.run-bundle.v1` | `run-bundle-v1` | Immutable pack snapshot, declared input hashes, policy hash, and pinned driver/model identity used for this invocation. |
| `mdp.driver-request.v1` | `driver-request-v1` | The bounded request passed to an external driver. It contains staged artifact authority, not arbitrary workspace access. |
| `mdp.driver-result.v1` | `driver-result-v1` | Driver-returned terminal state plus hashed output and runner-audit artifacts. Driver statements remain driver-attested until independently observed or verified. |
| `mdp.runner-audit.v1` | `runner-audit-v1` | Runtime observations, explicit evidence provenance, assurance dimensions, and limitations for one invocation. |
| `mdp.run-receipt.v1` | `run-receipt-v1` | MDP's terminal result, bound artifacts, decision authority, validation, assurance vector, limitations, and receipt hash. |
| `mdp.run-verification.v1` | `run-verification-v1` | A verifier's recomputed integrity and assurance result. `integrity_only: true` means external provider or host state was unavailable and was not silently assumed. |

The bundled native path also uses closed `mdp.driver-request.v2` and
`mdp.driver-result.v2` envelopes. V2 binds the exact model-visible prompt,
prompt invocation, declared input bytes, canonical and provider-adherence
schemas, provider/model policy, and request/result hashes. These are
CLI-to-subprocess contracts, not caller-authored alternatives to
`mdp.run-request.v1`. Inspect them with `mdp --json schema driver-request-v2`
and `mdp --json schema driver-result-v2`.

One generative run selects exactly one stable model-step ID and produces one
receipt. The customer host separately sequences normalization, deterministic
fit/routing, and generation/review. A receipt does not imply automatic
multi-step orchestration.

Inspect the exact contracts with:

```bash
mdp --json schema run-request-v1
mdp --json schema run-bundle-v1
mdp --json schema driver-request-v1
mdp --json schema driver-result-v1
mdp --json schema driver-request-v2
mdp --json schema driver-result-v2
mdp --json schema runner-audit-v1
mdp --json schema run-receipt-v1
mdp --json schema run-verification-v1
```

Verify v1 artifacts without invoking a runner:

```bash
mdp --json verify-run \
  --bundle <run-bundle.json> \
  --receipt <run-receipt.json> \
  --artifact-root <published-artifact-directory>
```

Project the verified relationships into a bounded explanation with:

```bash
mdp --json trace \
  --bundle <run-bundle.json> \
  --receipt <run-receipt.json> \
  --artifact-root <published-artifact-directory>
```

The resulting `mdp.decision-trace.v1` object is explanatory only. It reuses
the same verification rules, preserves integrity limitations, and never
replaces the bundle, receipt, decision hash, or verification result.

Omitting `--artifact-root` checks the bundle, decision, receipt, terminal-state,
and assurance relationships but cannot re-read published artifact bytes. The
result therefore remains `integrity_only: true`. Supplying a v0 receipt without
`--bundle` emits an explicit legacy mapping whose isolation dimension remains
unknown; even a historical `audit-grade` label cannot become v1 verified
assurance.

Integrity verification is deliberately separate from freshness consumption.
For conformance and local pilots, an operator can atomically compare and
consume a verified receipt with:

```bash
mdp --json consume-run \
  --ledger <local-ledger.jsonl> \
  --job-id <expected-job-id> \
  --idempotency-key <expected-idempotency-key> \
  --receipt-sha256 <verified-receipt-hash> \
  --expected-prior-version <ledger-version>
```

`--permit-exact-replay` permits only the exact same job, idempotency identity,
receipt hash, and original prior version. The local ledger serializes writers
and verifies an append-only hash chain, but it cannot detect filesystem
rollback, restored snapshots, or cloned ledgers. Production hosts must replace
it with their own durable atomic transaction and monotonic trust anchor.

### Assurance Is a Vector

V1 does not use one unqualified `audit-grade` boolean. It records dimensions such as context isolation, declared-input isolation, stateless request construction, tool/filesystem/network enforcement, artifact integrity, validation, and replay protection separately. Every dimension has:

- a state: `declared`, `observed`, `enforced`, `verified`, `unknown`, `redacted`, `unsupported`, or `not-applicable`;
- provenance: `mdp-observed`, `provider-returned`, `customer-attested`, `host-attested`, `driver-attested`, `verifier-recomputed`, or `unknown`;
- evidence references and explicit limitations.

These terms are deliberately narrower:

- **Fresh context** means the model invocation did not inherit the authoring conversation or another prior session. A new process is useful evidence only when session resume, configuration discovery, instruction discovery, caches, and persisted state are also controlled.
- **Stateless inference** means the provider request did not intentionally attach prior provider-side messages or a reusable session. It does not prove the provider performed no caching, logging, retention, or hidden request transformation.
- **Declared-input isolation** means only the immutable pack release and manifest-listed run inputs were available to the execution boundary. It requires filesystem, tool, network, environment, and driver controls; prompt language alone cannot establish it.
- **Deterministic replay** means deterministic stages can be recomputed from the same canonical bytes and policy. It does not promise byte-identical generative output unless the provider supplies and honors a deterministic contract.
- **Audit evidence** means the receipt identifies exactly which claim was declared, observed, enforced, or recomputed and by whom. It is not a claim that source content was true or that a third-party model exposed its hidden context.

The terminal state is either `success` or an explicit `no-draft:*` state.
`success` means the runtime completed and sealed an authoritative decision; it
does not mean the decision authorized drafting. For example, a successful GTM
evaluation may return decision `no-draft` with reason `disqualified` or
`insufficient-context`. Hosts must inspect the sealed decision, never `valid`
or terminal state alone. Preflight refusal, runner failure, invalid output,
invalid decision, incomplete audit, or policy failure must not leave any
decision or draft authority that a host can mistake for usable output.

### V0 Compatibility and Migration

`mdp.run-receipt.v0` and `mdp.runner-audit.v0` remain readable and their existing schema targets remain `run-receipt` and `runner-audit`. They are legacy evidence, not aliases for v1:

- a v0 receipt must never be relabeled as v1 or silently upgraded to a stronger v1 assurance state;
- v0 `audit-grade` maps only to the historical v0 decision under its original assumptions;
- migration requires constructing a new v1 bundle from exact available bytes, preserving the v0 artifacts as provenance, and marking unavailable controls `unknown` or `unsupported`;
- a verifier must report when it can check only hashes and local structure (`integrity_only: true`); missing external host/provider evidence cannot be inferred from a successful historical result;
- v0 and v1 hashes are contract-domain-separated and are not interchangeable.

This compatibility rule lets existing proposal pilots remain inspectable without overstating them, while proposal and GTM workflows converge on one v1 execution and receipt authority.

For public demos, apply the
[Proposal Demo Go/No-Go Gate](proposal-demo-go-no-go.md) before recording or
presenting a receipt. A mock run is safe to show only when its blocked decision
and synthetic status are explicit.

## Legacy v0: What It Proves

The receipt records:

- whether the host runner reports a fresh/stateless model call (`--isolation isolated`);
- whether the host runner confirms only prompt-declared payload fields crossed into that model call (`--declared-inputs-only`);
- optional `mdp.runner-audit.v0` evidence from a native API, Codex headless, Claude headless, Cursor headless, OpenCode headless, or custom headless runner;
- hashes and byte counts for the pack manifest, prompt output, validation result, source audit, and any extra artifacts;
- whether `validate-prompt-output` succeeded;
- whether the proposal source audit was present and used by validation.

It does not prove the semantic truth of claims beyond the supplied artifacts, and it cannot itself create model context isolation. The host runner owns that boundary; the CLI records and gates the declared boundary.

## Legacy v0 Proposal Review Command

For the default `proposal-review` workflow, save the validation result first, then create the receipt:

```bash
mdp --json validate-prompt-output \
  --dir <pack-root> \
  --prompt-id normalize-opportunity \
  --file <normalize-opportunity-output.json> \
  --source-audit <source-audit.json> \
  > <validate-prompt-output-result.json>

mdp --json run-receipt \
  --dir <pack-root> \
  --workflow proposal-review \
  --isolation isolated \
  --declared-inputs-only \
  --prompt-id normalize-opportunity \
  --prompt-output <normalize-opportunity-output.json> \
  --validation <validate-prompt-output-result.json> \
  --source-audit <source-audit.json> \
  --runner-audit <runner-audit.json> \
  --require-runner-audit \
  --out <run-receipt.json>
```

The validation result may be either the raw `data` object from `validate-prompt-output` or the full CLI wrapper. For an audit-grade receipt, the validation result must include artifact hashes for the exact prompt output and source audit that `run-receipt` is hashing:

```json
{
  "ok": true,
  "command": "validate-prompt-output",
  "data": {
    "valid": true,
    "file": "normalize-opportunity-output.json",
    "prompt": {"id": "normalize-opportunity"},
    "source_audit": {"contract": "mdp.source-audit.v0"},
    "artifacts": {
      "prompt_output": {
        "path": "normalize-opportunity-output.json",
        "sha256": "<prompt-output-sha256>"
      },
      "source_audit": {
        "path": "source-audit.json",
        "sha256": "<source-audit-sha256>"
      }
    }
  }
}
```

## Legacy v0 Decisions

| Decision | `valid` | Meaning |
| --- | --- | --- |
| `audit-grade` | `true` | Required artifacts exist, validation passed, source audit is present when required, and the runner confirmed an isolated declared-input-only model call. |
| `advisory` | `false` | Artifacts can be checked, but the model boundary was ambient or unknown, or declared-input-only was not confirmed. Treat review output as useful but not audit-grade. |
| `blocked` | `false` | Required artifacts are missing, malformed, failed validation, validation hashes do not match the supplied artifacts, runner-audit prompt-output hashes do not match, or source-audit use cannot be proven. Do not rely on the review until fixed. |

Validation-style CLI behavior applies: a non-`audit-grade` receipt prints the JSON result and exits nonzero.

## Legacy v0 Runner Audit

`mdp.runner-audit.v0` is the host-owned artifact that makes the isolation claim reviewable. Get its schema with:

```bash
mdp --json schema runner-audit
```

The rest of the proposal evidence chain is also inspectable:

```bash
mdp --json schema source-intake
mdp --json schema source-audit
mdp --json schema native-normalize-request
mdp --json schema prompt-output
mdp --json schema proposal-runner-result
mdp --json schema proposal-readiness-report
mdp --json schema proposal-mcp-run-result
```

These schemas describe artifact shape and contract version. They do not upgrade
mock/demo evidence, prove that a provider call occurred, approve source
material, or turn MCP transport into model-isolation evidence.

The local proposal runner supplies its checked `mdp.source-intake.v0` ledger to
`run-receipt` as an extra artifact with kind `source-intake`. The receipt hashes
that exact file. This binds the ledger to the run but does not replace the
human approval recorded inside approved entries.

The surrounding workdir lifecycle is recorded separately in
`.mdp-proposal-run.json` (`mdp --json schema proposal-run-manifest`). It binds
the files produced by the invocation and blocks partial/concurrent reuse, but it
does not replace the semantic assurance decision in `mdp.run-receipt.v0`.

The runner also writes `artifacts/proposal-readiness-report.json`. Its
structured findings and hash anchors make blockers easier to review, but its
confidence field measures evidence anchoring—not claim truth, compliance, or
submission approval. It cannot upgrade the receipt decision.

For proposal pilots, prefer `--require-runner-audit`. This blocks the receipt unless the supplied runner audit proves one of the schema-accepted isolated modes and includes `prompt_id`, the exact `prompt_output_sha256`, and `tool_invocations_observed: 0`. Schema acceptance does not make a runner a maintained or verified MDP integration:

- `native-api`: a direct stateless API request with no prior messages and no tools. For new v1 runs the bundled reference is `scripts/mdp-native-model-openai.mjs`; the normalization-named script is v0 compatibility. See [Native API Model Runner](native-api-normalization-runner.md).
- `codex-exec`: `codex exec` in a sterile working directory with ephemeral output, read-only sandboxing, no resume, prompt-input audit, and zero observed tool events.
- `claude-print`: `claude --bare -p` with no session persistence, no resume/continue, structured output, disabled tools, and zero observed tool events.
- `cursor-print`: `cursor-agent -p` only when a wrapper proves no resume, no `--force`, sterile input, disabled/externally denied tools, and zero observed tool events.
- `opencode-run`: `opencode run` only when a wrapper proves no resume/session attach, `--pure`, disabled default/plugin discovery, a no-tool agent, and zero observed tool events.
- `custom-headless`: a host-owned runner that proves the common no-resume/no-tools/no-persistence boundary.

Runner audits marked as fixtures are intentionally not production evidence. If a runner audit includes `demo_fixture: true`, `fixture: true`, `mock_response: true`, or a model name that looks synthetic/mock/demo/fixture-only, `mdp run-receipt` blocks instead of returning `audit-grade`. Use those artifacts only for offline walkthroughs and tests.

If no runner audit is supplied and `--require-runner-audit` is omitted, the receipt can still be `audit-grade` from assertion flags, but `runner.assurance` is `asserted`. Do not use an asserted receipt as proof of verified model isolation. For production proposal review, require `headless-verified` or `stateless-api-verified` for the current invocation, then separately report the integration state from the canonical matrix.

## One-Thread UX, Two Planes

A ChatGPT, Codex, Claude Code, Cursor, or Copilot user should not have to manually reason about model context. The polished workflow can still appear as one thread, but implementation should keep two planes:

```text
Control plane:  user's chat/workshop thread and status messages
Evidence plane: local source files, source audit, prompt output, validation, fit/proof results, run receipt
```

For production proposal flows, same-conversation normalization should be labeled advisory unless a runner/MCP can create a fresh model invocation with only the prompt-declared payload.

See [Native API Model Runner](native-api-normalization-runner.md) for the
profile-neutral BYOK OpenAI driver and [Headless And Native Model
Runners](headless-normalization-runners.md) for Codex, Claude Code, Cursor, and
OpenCode compatibility recipes.

## Legacy Proposal Runner/MCP Direction

The following v0 flow remains for proposal compatibility. New GTM and proposal
model steps use `mdp run` plus the profile-neutral, path-only
`scripts/mdp-run-mcp-server.mjs` surface. That MCP server invokes the same CLI,
can inherit native credentials and permission only from its startup
environment for a parsed generative request, and adds no assurance.

`scripts/mdp-proposal-runner.mjs` and
`scripts/mdp-proposal-mcp-server.mjs` remain local compatibility surfaces:

1. stage supplied source files in customer-controlled storage;
2. extract bounded text and create `mdp.source-audit.v0`;
3. load `.mdp/prompts/normalize-opportunity.yaml`;
4. call the model in a fresh/stateless invocation with only declared inputs;
5. emit `mdp.runner-audit.v0` for the headless/stateless boundary, including the exact prompt-output hash produced by that invocation;
6. run `mdp validate-prompt-output --source-audit`;
7. call `mdp run-receipt --require-runner-audit`;
8. continue to `fit`, `route`, `author-proof-output`, `verify-output`, or `render-brief` as needed.

Inspect the local surface with:

```bash
node scripts/mdp-proposal-runner.mjs tools
node scripts/mdp-proposal-mcp-server.mjs
```

Pluxx continues to package skills, hooks, assets, and scripts for supported hosts. The local runner/MCP wrapper owns source staging and the runtime call into the native/headless boundary, while the CLI owns deterministic artifact checks. MCP transport alone is not audit-grade; dry-run/mock runner modes are valid for CI and demo fixtures only, and they must block or remain non-audit-grade when `--require-runner-audit` is used.

The MCP result envelope makes transport state explicit: consume its top-level
`mode`, `decision`, `audit_grade_eligible`, `runner_assurance`, `timed_out`, and
`runner_exit_status` fields. The wrapper uses canonical local paths, an explicit
child-environment allowlist, a bounded timeout/output budget, and redacted
diagnostics. Those controls reduce accidental context and credential exposure,
but they do not prove that a provider call occurred or replace the runner audit,
artifact hashes, or receipt decision. A timeout, termination, malformed result,
or `require_audit_grade` mismatch is a tool error and must remain blocked.
