# Local MCP: One Canonical Path

MDP has one default MCP story: the profile-neutral local stdio adapter at
`scripts/mdp-run-mcp-server.mjs` (or
`${PLUGIN_ROOT}/scripts/mdp-run-mcp-server.mjs` in an installed agent bundle).
It exposes the same CLI-owned clean-run lifecycle for GTM, proposal, and future
profiles.

```bash
node "${PLUGIN_ROOT}/scripts/mdp-run-mcp-server.mjs"
```

Before startup, the operator must configure existing approved directories with
`MDP_MCP_PACK_ROOTS`, `MDP_MCP_INPUT_ROOTS`, `MDP_MCP_APPROVAL_ROOTS`,
`MDP_MCP_WORK_ROOTS`, `MDP_MCP_OUTPUT_ROOTS`, and `MDP_MCP_CONSENT_ROOTS`.
Requests cannot widen those roots. A generative call additionally requires the
server to start with `MDP_ALLOW_NATIVE_MODEL_CALLS=1` and `OPENAI_API_KEY`, plus
a matching out-of-band, one-shot consent record. Never pass credentials as MCP
arguments.

## The four stages

Use these tools in order:

| Stage | Tool | Input | Produced artifact | Next stage |
|---|---|---|---|---|
| Inspect | `mdp_run_tools` | No arguments | `mdp.run-mcp-tools.v1` boundary inventory | `mdp_prepare_run` |
| Prepare | `mdp_prepare_run` | Pack directory, exact job/model step, declared input paths, and required new `out` path under an approved work root | Persisted `mdp.run-request.v1` and optional compile manifest under the work root | `mdp_run` |
| Run | `mdp_run` | Request path and a new output directory | `run-bundle.json`, `run-receipt.json`, and declared artifacts | `mdp_verify_run` |
| Verify | `mdp_verify_run` | Bundle and receipt paths, plus optional artifact root | `mdp.run-verification.v1` | Return the verified CLI authority unchanged |

The adapter accepts explicit local paths, not ambient chat, inline source
bodies, or assurance overrides. `mdp_prepare_run` requires `out`, persists that
request under `MDP_MCP_WORK_ROOTS`, and does not call a provider. `mdp_run`
accepts the same work-root request, executes exactly one request, and writes its
run directory under `MDP_MCP_OUTPUT_ROOTS`. `mdp_verify_run` reads that bundle,
receipt, and artifact root from the same approved output boundary and is
otherwise read-only.

MCP is transport, not authority. The Rust `mdp` CLI remains the sole authority
for request parsing, staging, execution, terminal state, assurance, hashes,
validation, receipts, and verification. MCP availability does not prove fresh
context, isolation, provider execution, source truth, replay safety, or audit
grade.

## Proposal v0 compatibility

`scripts/mdp-proposal-mcp-server.mjs`, `mdp_proposal_tools`, and
`mdp_proposal_run` remain packaged only so existing proposal integrations are
not stranded. They are compatibility-only and are not a second beginner or
default MCP surface.

Migrate a proposal integration by replacing the v0 pipeline call with:

1. `mdp_prepare_run` for the exact proposal job/model step and approved files;
2. `mdp_run` with the resulting request and a new output directory; and
3. `mdp_verify_run` with the emitted bundle and receipt.

Keep the v0 adapter only while a consumer still depends on its source-intake or
legacy receipt envelope. A v0 result cannot promote CLI v1 authority or
assurance. See [Compatibility Proposal Runner Surface](proposal-runner.md) for
that bounded contract.
