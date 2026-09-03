# Native API Model Runner

MDP ships one profile-neutral local BYOK driver for every job-declared model
step in the basic GTM and proposal templates. A selected step may normalize
messy input, generate a governed artifact, or review an artifact. The same
`mdp run` state machine, validation rules, terminal states, and receipt format
apply to every phase.

One run always means **one selected declared model step and one receipt**. The
customer host sequences the larger workflow:

```text
normalization run
  -> deterministic fit and minimal-context routing
  -> generation or review run
```

MDP does not automatically chain those steps, collect source data, retry
providers, batch records, send outreach, submit proposals, mutate CRM, or
calculate model pricing.

## Discover The Declared Steps

Use the exact canonical job ID. `requirements` resolves the job-bound
normalization prompt and the job-owned generation or review prompt into stable
step IDs and phase order:

```bash
mdp --json requirements --dir PACK_ROOT --job JOB_ID
```

Inspect `data.model_steps`. A native run's `operation` must equal one emitted
`step_id`. MDP does not infer a step from a filename, free-text instruction, or
skill prose. Unbound authoring and extraction prompts are not executable model
steps.

### Compile the normalization model context

For a v3 normalization step, compile the bounded model-facing projection in
addition to the full requirements report:

```bash
mdp --json requirements \
  --dir PACK_ROOT \
  --job JOB_ID \
  --model-context > REQUIREMENTS_RESPONSE.json
```

The command's `data` value is the exact
`mdp.requirements-model-context.v1` artifact. Persist that value (rather than
the surrounding CLI response) as `decision-input-requirements.json`. It
contains the job-scoped collection specification, closed classification
taxonomies and definitions, semantic output contract, no-draft policy, and
the `requirements_sha256`/`taxonomy_set_sha256` bindings. The full
`mdp.requirements.v2` result remains the host's validation authority; the
bounded projection is the only requirements artifact sent to the model.

The normalization request supplies four lineage-bound inputs:

```text
decision-input-requirements
source-binding
source-attempt-request
collected-attempt-results
```

The host collects the attempted evidence using its own tools, invokes the
model for semantic classifications only, then seals the v3 envelope and runs
deterministic fit/routing. `prepare-run` rejects a missing, tampered, stale, or
cross-pack model-context artifact before a provider call. Keep the projection
under the 128 KiB native input limit; a larger pack must be narrowed at the
requirements/route boundary rather than truncated.

The provider-facing v3 schema is job-specialized and must remain semantically
equivalent to local validation: a classified status requires `value`, whereas
ambiguous, no-match, and unsupported statuses omit it; gap and rejected-claim
items keep their explicit fields. If the local boundary rejects a provider
payload, the run carries only a bounded rejection code plus sanitized path,
expected category, and observed category. It never carries raw model output or
unbounded schema error text.

The shipped templates use one resolver and driver contract:

- basic GTM: `normalize-prospect-row`, `generate-outbound-copy-v1`, and
  `review-outbound-copy-v1` as selected by their jobs;
- proposal: `normalize-opportunity` and the four job-owned review prompts.

## Canonical CLI Run

Create one closed `mdp.run-request.v1` that names the released pack, selected
job, stable model-step operation, exact prompt, declared input files, driver,
model, and execution policy. Inspect the installed schema instead of copying a
request shape from documentation:

```bash
mdp --json schema run-request-v1
mdp --json run --request RUN_REQUEST.json --out-dir NEW_RUN_DIRECTORY
mdp --json verify-run \
  --bundle NEW_RUN_DIRECTORY/run-bundle.json \
  --receipt NEW_RUN_DIRECTORY/run-receipt.json \
  --artifact-root NEW_RUN_DIRECTORY
```

The CLI freezes the pack, prompt, and declared inputs before it invokes the
driver. It creates the versioned `mdp.driver-request.v2`, launches the bundled
subprocess with a cleared and allowlisted environment, validates the returned
artifact against the pack-owned output contract, applies the relevant
deterministic gates, and publishes no usable output on failure.

Native identity fields in the public run request are declarations only. MDP
recomputes the driver configuration identity from the observed bundled script,
Node executable, and fixed launch policy, and recomputes the model-parameter
identity from the exact prepared request and closed provider policy. Stale or
arbitrary SHA-shaped declarations are rejected before the driver starts. The
sealed bundle and runner audit carry the observed hashes plus a bounded,
secret-free projection; `verify-run` recomputes those projection hashes and
keeps the exact provider request-body SHA as separate full-body evidence.

Inspect `mdp --json schema driver-request-v2` and `driver-result-v2` when
implementing or auditing that boundary. They are runtime driver contracts, not
operator request formats.

The canonical subprocess is `scripts/mdp-native-model-openai.mjs`. Installed
Pluxx bundles provide the same file under
`${PLUGIN_ROOT}/scripts/mdp-native-model-openai.mjs`. The Rust runtime invokes
it through the driver protocol; operators should normally use `mdp run`, not
construct `mdp.driver-request.v2` or the subprocess envelope by hand.

## Real Calls Are Default-Deny

The bundled transport uses only the official OpenAI Responses endpoint. A real
call requires both environment values to exist before the CLI or MCP server
starts:

```bash
export MDP_ALLOW_NATIVE_MODEL_CALLS=1
export OPENAI_API_KEY='<operator-supplied-secret>'
mdp --json run --request RUN_REQUEST.json --out-dir NEW_RUN_DIRECTORY
```

`MDP_ALLOW_NATIVE_MODEL_CALLS=1` is an out-of-band operator permission. A run
request, prompt, or MCP tool argument cannot enable it. The key is passed only
to the bounded driver process and must never appear in a request, pack, prompt,
fixture, log, stdout result, or receipt.

The driver sends one Responses request with Structured Outputs, `store: false`,
no tools, and no conversation or previous-response attachment. Custom OpenAI
origins are not supported by this canonical path. Raw provider request and
response envelopes and failed model output are not published as run artifacts.

Generative requests that declare `routed_context` must supply the exact saved
`mdp.routed-context.v1` bytes emitted by `brief --context` or `emit-brief
--routed-context-out`. The runtime does not read a top-level `status` or
`draft_status`; it validates the closed schema, canonical bytes, job/scope
binding, and current staged-pack compilation before the native driver boundary.

`store: false` is a request setting, not a promise that every provider-side
retention category is zero. The customer remains responsible for provider
terms, account policy, and data handling.

## Offline And Mock Validation

Installing MDP, inspecting schemas, resolving steps, validating packs, and
running deterministic commands require no API key. Repository tests exercise
the native subprocess with synthetic mock responses and no network call.

The subprocess also has a direct dry-run/mock test interface:

```bash
node scripts/mdp-native-model-openai.mjs --request DRIVER_REQUEST.json --dry-run
node scripts/mdp-native-model-openai.mjs \
  --request DRIVER_REQUEST.json \
  --mock-response SYNTHETIC_OPENAI_RESPONSE.json
```

That interface consumes the bounded subprocess request contract, not the
public `mdp.run-request.v1`. Dry-run and mock success prove request construction
and parsing only. They do not prove a provider call, model quality, fresh
context, or a verified integration.

## MCP Parity

For MCP-capable hosts, launch the profile-neutral local stdio server:

```bash
node scripts/mdp-run-mcp-server.mjs
```

It exposes one canonical four-stage path:

1. `mdp_run_tools` inventories the boundary and the next stages.
2. `mdp_prepare_run` compiles a pack, exact job/model step, and declared input
   paths into a required persisted `mdp.run-request.v1`. Pass `out` under an
   approved work root; `manifest_out` is optional under the same role. Prepare
   also returns the exact persisted `request_sha256`.
3. `mdp_run` requires that existing work-root request path, prepare-returned
   `request_sha256`, and a new output directory. It freezes the request and
   rejects a digest mismatch before execution, then returns the CLI-owned bundle
   and receipt.
4. `mdp_verify_run` reads the resulting bundle and receipt from the approved
   output root and returns the terminal CLI verification.

Configure the local server with explicit `MDP_MCP_PACK_ROOTS`,
`MDP_MCP_INPUT_ROOTS`, `MDP_MCP_WORK_ROOTS`, `MDP_MCP_OUTPUT_ROOTS`, and
`MDP_MCP_CONSENT_ROOTS` before startup. Every server startup requires all five
root roles; only a generative run consumes consent. For that run, the operator
creates an out-of-band, one-shot consent record under the consent root, bound
to the provider, purpose, exact prepared request and declared-source hashes,
output root, expiry, and nonce, then passes only its `consent_id` to `mdp_run`.
Tool arguments cannot manufacture consent or authorize provider access. The
prepare-to-run handoff stays under the `work` role; it is not re-authorized as
an input file. These tools do not accept inline source text, credentials, or an
enable flag. Only a parsed generative request with valid one-shot consent may
inherit the key and native-call permission that were present when the server
started.

`timeout_ms` bounds normal prepare, publication, and verification work. If
failure cleanup has entered the finite descriptor-relative identity transaction,
the supervisor sends TERM but never SIGKILLs that remove helper; the helper
finishes unlinking the owned inode or restoring an unrelated leaf before the
pending TERM takes effect. A failure response may therefore follow the normal
deadline by that finite safety finalization instead of stranding filesystem
state.

MCP is transport only. It invokes the same CLI and returns canonical CLI data
unchanged; it adds no execution, validation, or isolation authority.

## Failure And Assurance

Missing permission, missing credentials, an unsupported endpoint, a timeout,
provider refusal, malformed response, invalid schema, mismatched hash, or a
failed deterministic gate produces a bounded `no-draft:*` result. Partial or
invalid model text must not be used as copy or review authority.

A receipt describes one invocation. It does not establish that the integration
is publicly `verified`, prove the truth of supplied source claims, authorize
sending, or qualify a cold-model behavioral trial. Do not claim real provider
verification from repository tests, dry runs, mock fixtures, or MCP transport.

## Legacy Compatibility

`scripts/mdp-native-normalize-openai.mjs`, `scripts/mdp-proposal-runner.mjs`,
`scripts/mdp-proposal-mcp-server.mjs`, and `mdp run-receipt` remain v0 proposal
compatibility surfaces. New GTM and proposal execution should use the shared
`mdp run` kernel and `scripts/mdp-run-mcp-server.mjs`. Compatibility artifacts
do not silently become v1 receipts or stronger assurance.

## Provider References

- [OpenAI Structured Outputs](https://platform.openai.com/docs/guides/structured-outputs)
- [OpenAI Responses API](https://platform.openai.com/docs/api-reference/responses)
- [OpenAI endpoint retention defaults](https://platform.openai.com/docs/models/default-usage-policies-by-endpoint)
