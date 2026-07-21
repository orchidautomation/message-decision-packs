# MDP Run Receipts

`mdp run-receipt` creates a local `mdp.run-receipt.v0` artifact for workflows where an agent host or runner normalized messy source material before deterministic MDP checks ran.

Use it when the operator wants an audit-grade proposal or document-review flow, especially when a PDF/doc extraction step produced a `mdp.source-audit.v0` ledger.

## What It Proves

The receipt records:

- whether the host runner reports a fresh/stateless model call (`--isolation isolated`);
- whether the host runner confirms only prompt-declared payload fields crossed into that model call (`--declared-inputs-only`);
- optional `mdp.runner-audit.v0` evidence from a native API, Codex headless, Claude headless, Cursor headless, OpenCode headless, or custom headless runner;
- hashes and byte counts for the pack manifest, prompt output, validation result, source audit, and any extra artifacts;
- whether `validate-prompt-output` succeeded;
- whether the proposal source audit was present and used by validation.

It does not prove the semantic truth of claims beyond the supplied artifacts, and it cannot itself create model context isolation. The host runner owns that boundary; the CLI records and gates the declared boundary.

## Proposal Review Command

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

## Decisions

| Decision | `valid` | Meaning |
| --- | --- | --- |
| `audit-grade` | `true` | Required artifacts exist, validation passed, source audit is present when required, and the runner confirmed an isolated declared-input-only model call. |
| `advisory` | `false` | Artifacts can be checked, but the model boundary was ambient or unknown, or declared-input-only was not confirmed. Treat review output as useful but not audit-grade. |
| `blocked` | `false` | Required artifacts are missing, malformed, failed validation, validation hashes do not match the supplied artifacts, or source-audit use cannot be proven. Do not rely on the review until fixed. |

Validation-style CLI behavior applies: a non-`audit-grade` receipt prints the JSON result and exits nonzero.

## Runner Audit

`mdp.runner-audit.v0` is the host-owned artifact that makes the isolation claim reviewable. Get its schema with:

```bash
mdp --json schema runner-audit
```

For proposal pilots, prefer `--require-runner-audit`. This blocks the receipt unless the supplied runner audit proves one of the supported isolated modes:

- `native-api`: a direct stateless API request with no prior messages and no tools. The bundled optional reference is `scripts/mdp-native-normalize-openai.mjs` in source checkouts and `${PLUGIN_ROOT}/scripts/mdp-native-normalize-openai.mjs` in installed Pluxx bundles; see [Native API Normalization Runner](native-api-normalization-runner.md).
- `codex-exec`: `codex exec` in a sterile working directory with ephemeral output, read-only sandboxing, no resume, prompt-input audit, and zero observed tool events.
- `claude-print`: `claude --bare -p` with no session persistence, no resume/continue, structured output, disabled tools, and zero observed tool events.
- `cursor-print`: `cursor-agent -p` only when a wrapper proves no resume, no `--force`, sterile input, disabled/externally denied tools, and zero observed tool events.
- `opencode-run`: `opencode run` only when a wrapper proves no resume/session attach, `--pure`, disabled default/plugin discovery, a no-tool agent, and zero observed tool events.
- `custom-headless`: a host-owned runner that proves the common no-resume/no-tools/no-persistence boundary.

If no runner audit is supplied and `--require-runner-audit` is omitted, the receipt can still be `audit-grade` from assertion flags, but `runner.assurance` is `asserted`. For production proposal review, use `headless-verified` or `stateless-api-verified`.

## One-Thread UX, Two Planes

A ChatGPT, Codex, Claude Code, Cursor, or Copilot user should not have to manually reason about model context. The polished workflow can still appear as one thread, but implementation should keep two planes:

```text
Control plane:  user's chat/workshop thread and status messages
Evidence plane: local source files, source audit, prompt output, validation, fit/proof results, run receipt
```

For production proposal flows, same-conversation normalization should be labeled advisory unless a runner/MCP can create a fresh model invocation with only the prompt-declared payload.

See [Native API Normalization Runner](native-api-normalization-runner.md) for the BYOK OpenAI reference runner and [Headless Normalization Runners](headless-normalization-runners.md) for Codex, Claude Code, Cursor, and OpenCode recipes.

## Runner/MCP Direction

`run-receipt` is the first deterministic receipt contract. A host-neutral runner or MCP should wrap the CLI with tools that:

1. stage supplied source files in customer-controlled storage;
2. extract bounded text and create `mdp.source-audit.v0`;
3. load `.mdp/prompts/normalize-opportunity.yaml`;
4. call the model in a fresh/stateless invocation with only declared inputs;
5. emit `mdp.runner-audit.v0` for the headless/stateless boundary;
6. run `mdp validate-prompt-output --source-audit`;
7. call `mdp run-receipt --require-runner-audit`;
8. continue to `fit`, `route`, `author-proof-output`, `verify-output`, or `render-brief` as needed.

Pluxx continues to package skills and hooks for supported hosts. The runner/MCP owns runtime isolation, while the CLI owns deterministic artifact checks.
