# MDP Run Receipts

`mdp run-receipt` creates a local `mdp.run-receipt.v0` artifact for workflows where an agent host or runner normalized messy source material before deterministic MDP checks ran.

Use it when the operator wants an audit-grade proposal or document-review flow, especially when a PDF/doc extraction step produced a `mdp.source-audit.v0` ledger.

## What It Proves

The receipt records:

- whether the host runner reports a fresh/stateless model call (`--isolation isolated`);
- whether the host runner confirms only prompt-declared payload fields crossed into that model call (`--declared-inputs-only`);
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
  --out <run-receipt.json>
```

The validation result may be either the raw `data` object from `validate-prompt-output` or the full CLI wrapper:

```json
{
  "ok": true,
  "command": "validate-prompt-output",
  "data": {
    "valid": true,
    "prompt": {"id": "normalize-opportunity"},
    "source_audit": {"contract": "mdp.source-audit.v0"}
  }
}
```

## Decisions

| Decision | `valid` | Meaning |
| --- | --- | --- |
| `audit-grade` | `true` | Required artifacts exist, validation passed, source audit is present when required, and the runner confirmed an isolated declared-input-only model call. |
| `advisory` | `false` | Artifacts can be checked, but the model boundary was ambient or unknown, or declared-input-only was not confirmed. Treat review output as useful but not audit-grade. |
| `blocked` | `false` | Required artifacts are missing, malformed, failed validation, or source-audit use cannot be proven. Do not rely on the review until fixed. |

Validation-style CLI behavior applies: a non-`audit-grade` receipt prints the JSON result and exits nonzero.

## One-Thread UX, Two Planes

A ChatGPT, Codex, Claude Code, Cursor, or Copilot user should not have to manually reason about model context. The polished workflow can still appear as one thread, but implementation should keep two planes:

```text
Control plane:  user's chat/workshop thread and status messages
Evidence plane: local source files, source audit, prompt output, validation, fit/proof results, run receipt
```

For production proposal flows, same-conversation normalization should be labeled advisory unless a runner/MCP can create a fresh model invocation with only the prompt-declared payload.

## Runner/MCP Direction

`run-receipt` is the first deterministic receipt contract. A host-neutral runner or MCP should wrap the CLI with tools that:

1. stage supplied source files in customer-controlled storage;
2. extract bounded text and create `mdp.source-audit.v0`;
3. load `.mdp/prompts/normalize-opportunity.yaml`;
4. call the model in a fresh/stateless invocation with only declared inputs;
5. run `mdp validate-prompt-output --source-audit`;
6. call `mdp run-receipt`;
7. continue to `fit`, `route`, `author-proof-output`, `verify-output`, or `render-brief` as needed.

Pluxx continues to package skills and hooks for supported hosts. The runner/MCP owns runtime isolation, while the CLI owns deterministic artifact checks.
