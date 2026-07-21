# Native API Normalization Runner

The strongest audit boundary for proposal/document normalization is a native stateless model call. The operator's chat remains the control plane, but the normalizer call is a separate request that contains only the prompt package and declared inputs.

This repo includes a small OpenAI reference runner:

```bash
node scripts/mdp-native-normalize-openai.mjs --request <request.json> --out <prompt-output.json> --runner-audit <runner-audit.json>
```

Pluxx packages repo scripts into installed host bundles, so a shipped plugin can call the same runner at `${PLUGIN_ROOT}/scripts/mdp-native-normalize-openai.mjs`. Source checkouts can use the relative `scripts/...` path shown here.

It is optional and BYOK. Installing MDP, validating packs, running evals, and using `mdp fit`, `route`, `brief`, `validate-prompt-output`, or `run-receipt` do not require an API key. A key is required only when this script performs a real model call.

The script requires Node.js 18+ for the built-in `fetch` API.

## What The Runner Owns

The native runner owns:

- one stateless OpenAI Responses API call;
- no `previous_response_id` and no `conversation` attachment;
- no model tools;
- `store: false`;
- Structured Outputs through `text.format` with `type: "json_schema"` and `strict: true`;
- writing the model's strict JSON prompt output;
- writing `mdp.runner-audit.v0` with `runner: "native-api"`.

The runner does not create or manage API keys, parse private PDFs, build source audits, decide fit, update packs, submit proposals, or prove semantic truth beyond the supplied artifacts.

## Request Contract

The host/plugin/MCP creates a request JSON file after it has staged source files, extracted bounded text, and loaded the selected MDP prompt contract.

```json
{
  "contract": "mdp.native-normalize-request.v0",
  "provider": "openai",
  "model": "<openai-model-id>",
  "prompt_id": "normalize-opportunity",
  "declared_inputs_only": true,
  "instructions": "You normalize supplied proposal material into the MDP prompt output contract. Return strict JSON only.",
  "input": [
    {
      "role": "user",
      "content": "{\"raw_opportunity\":{...},\"existing_pack_context\":{...},\"source_kind\":\"pdf-extraction\"}"
    }
  ],
  "prompt_output_schema": {
    "type": "object",
    "additionalProperties": false,
    "required": ["contract", "prompt_id", "source_summary", "normalized_prospect", "normalization_trace", "card_patches", "gaps", "rejected_claims"],
    "properties": {}
  }
}
```

Rules:

- `input` must include only prompt-declared payload fields.
- `input` must be either a single string payload or an array with exactly one plain `user` message; put system/developer guidance in `instructions`.
- Do not include prior chat messages, notes, brainstorms, or desired outcomes.
- Do not include `previous_response_id`, `conversation`, or tools.
- Keep private source documents outside the public repo; pass only the bounded extracted payload and local source-audit refs needed for validation.

## Offline Dry Run

A dry run validates the request shape and shows the API request preview without needing a key or making a network call:

```bash
node scripts/mdp-native-normalize-openai.mjs --request <request.json> --dry-run
```

## Real Run

For a real run, provide `OPENAI_API_KEY` through the operator's secure local environment and invoke:

```bash
node scripts/mdp-native-normalize-openai.mjs \
  --request <request.json> \
  --out <normalize-opportunity-output.json> \
  --runner-audit <runner-audit.json> \
  --response <openai-response.json>
```

`--response` is optional. Use it only in customer-controlled scratch if retaining the raw provider response is acceptable for that engagement.

The runner writes `runner-audit.json` similar to:

```json
{
  "contract": "mdp.runner-audit.v0",
  "runner": "native-api",
  "model": "<openai-model-id>",
  "isolated_invocation": true,
  "conversation_resume": false,
  "declared_inputs_only": true,
  "output_schema_used": true,
  "stateless_request": true,
  "prior_messages_included": false,
  "tools_disabled": true,
  "tool_invocations_observed": 0,
  "prompt_id": "normalize-opportunity",
  "prompt_output_sha256": "<prompt-output-sha256>",
  "endpoint": "/v1/responses",
  "store": false
}
```

## Downstream MDP Gate

After the native run, the deterministic flow is unchanged:

```bash
mdp --json validate-prompt-output \
  --dir <pack-root> \
  --prompt-id normalize-opportunity \
  --file <normalize-opportunity-output.json> \
  --source-audit <source-audit.json> \
  > <validation-result.json>

mdp --json run-receipt \
  --dir <pack-root> \
  --workflow proposal-review \
  --isolation isolated \
  --declared-inputs-only \
  --prompt-id normalize-opportunity \
  --prompt-output <normalize-opportunity-output.json> \
  --validation <validation-result.json> \
  --source-audit <source-audit.json> \
  --runner-audit <runner-audit.json> \
  --require-runner-audit \
  --out <run-receipt.json>
```

The validation result records artifact hashes for the prompt output and source audit. `run-receipt` compares those hashes to the supplied files, and also compares `runner-audit.prompt_output_sha256` to the supplied prompt output, so substituting the prompt output, source audit, or runner audit after the native run blocks audit-grade status. A valid native runner audit gives the receipt `runner.assurance: "stateless-api-verified"`.

## Test Mode

`--mock-response` exists only for offline tests. It writes `mock_response: true`, `isolated_invocation: false`, and `stateless_request: false` in the runner audit so it cannot be mistaken for audit-grade production evidence.

## Source Docs

The OpenAI Structured Outputs guide recommends schema-constrained output when the model should respond in a specific JSON shape, and the Responses API accepts `text.format` with `type: "json_schema"`, `strict: true`, and a JSON Schema. The Responses API also supports stateless calls with `store: false`, and conversation state is only attached when the request supplies conversation/previous-response fields.

- Structured Outputs: <https://developers.openai.com/api/docs/guides/structured-outputs>
- Responses API: <https://developers.openai.com/api/reference/responses/create>
