# CLI Operator Reference

Read this when selecting an MDP command.

## Discovery And Health

```bash
mdp --version
mdp --json capabilities
mdp --json skills
mdp --json skills --dir PACK_ROOT
mdp --json doctor --dir PACK_ROOT
```

`skills` reports released inventory and pack eligibility. It does not observe host discovery.

## Contracts And Inspection

```bash
mdp --json schema skills
mdp --json validate --dir PACK_ROOT
mdp --json requirements --dir PACK_ROOT --job JOB
mdp --json schema source-binding
mdp --json validate-source-binding --dir PACK_ROOT --job JOB --file SOURCE_BINDING_JSON
mdp --json explain --dir PACK_ROOT
mdp --json gaps --dir PACK_ROOT
mdp --json route --entries --dir PACK_ROOT --persona PERSONA --job JOB
```

Prefer CLI output to direct YAML inference. Read pack files only when authoring or when the CLI identifies the exact card or contract needing review.

For product authority, pass one exact canonical manifest job ID to both
`skills --job` and `requirements --job`. Inspect the compact skills summary,
then the complete requirements `product_foundation` projection and exact
selected refs. Unknown or free-text jobs remain `unassessed`; do not guess a
nearby job or select foundation entries through keyword overlap.

Read `.mdp/README.md` only after the CLI projection and only for navigation.
It cannot close a gap or override a diagnostic. Foundation `ready` is a
veto-only result and never means sufficient-for-job or self-standing.

`requirements` is the read-only, job-bound handoff to collectors and
customer-selected hosts. It compiles Decision Input questions and schemas plus
`data.model_steps`: stable IDs for each job-bound normalization, generation,
or review prompt with exact prompt/version/hash, input producers, selected
product foundation, and output schema. It never performs research or a model
call. Existing jobs without a Decision Input Contract keep `available: false`;
inspect model steps separately.

`validate-source-binding` checks one integration-owned version-compatible
`mdp.source-binding.v1` or signal-aware `mdp.source-binding.v2` mapping against the exact pack and requirements
digests, job, contract versions, and qualified attributes. It rejects stale,
missing, duplicate, unknown, or incompatible mappings, permits external
field-key reuse, and performs no source access or execution.

Inspect the requirements version matrix before choosing artifacts. V1 is
scalar-only; v2 adds structured repeated observations and exact
binding/request/results lineage. Validate the full v2 chain and pass it to
`fit` or `brief` with `--normalized-input` and all lineage flags. Detached
prospects remain legacy/unassessed. Explicit roles never come from keywords,
and `lineage-validated` proves internal consistency only, not authenticity or
truth. Preserve conflicts and stop no-draft unless the compiled conservative
policy deterministically disqualifies.

## Deterministic Gates

### Cold-model qualification

Discover with `capabilities`, schema inspection, and `mdp conformance --help`.
The exact flow is discover → `conformance compile` → stop unless sufficient →
external host call → `conformance validate` → `conformance assemble` →
`conformance report` or `trace`.

Use first-class `--out` files for the exact chain: compile to
`deterministic.json`; pass that file with `validate --deterministic` and write
`behavioral.json`; assemble those authorities plus every repeated `--trial`
into `job-conformance.json`; then project a private/public report with its own
`--out`. Pass the same staged root to every command through `--artifact-root`.
Validation also requires the candidate, evaluator inventory, lifecycle policy,
and every predeclared repeated `--invocation`,
`--trial`, and `--verifier-receipt`. External calls remain customer-owned and
separately authorized.
Trial slots freeze requested/resolved model identity and the exact candidate
prompt, input list, and context digest. Verifier and publication evidence must
match trusted authority descriptors already frozen in the evaluator inventory.

All four conformance commands make no model calls. Without `--out` they are
read-only; `--out` writes only the declared local authority or projection. The
behavioral evaluation returned by `validate` is intermediate, not report
authority. Only `mdp.job-conformance.v1` is the cross-phase authority; reports
and traces are projections. Stop no-draft on failed or unassessed required
gates. Conformance never authorizes drafting, sending, scheduling, CRM
mutation, or publication. Public projections must omit paths, raw content,
identities, provider/session data, evaluator rationale, reviewer identity, and
private hashes.

Use [canonical runner support matrix](https://github.com/orchidautomation/message-decision-packs/blob/main/docs/headless-normalization-runners.md#canonical-runner-support-matrix) for integration state. The only states are `verified`, `recipe-only`, `unsupported`, and `fixture/mock-only`; schema acceptance, a recipe, MCP availability, or one valid receipt does not promote a row.

- `validate-prompt-output`: validate model-produced normalization or governed-artifact output against the exact selected prompt. Its file-based result is `mdp.prompt-output-validation.v1`, binding the pack, canonical prompt, job when unambiguous, exact validator-input hashes, exact prompt-output bytes, and validator outcome. Raw `mdp.prompt-output.v0` remains untrusted and must never be treated as trace or decision authority. To inspect validated output, run `trace --file VALIDATION_JSON --dir PACK_ROOT --prompt-output OUTPUT_JSON` and repeat each validator file as `--validation-input LOGICAL_NAME=PATH`; trace only verifies those immutable bindings and does not rerun validation. Governed artifacts require `--invocation-receipt` with the host-created `mdp.prompt-invocation.v1` job/prompt/input-hash receipt and, when declared, `--routed-context` with the exact canonical `mdp.routed-context.v1` bytes. Pass `--source-audit` for proposal PDF/doc extraction ledgers when raw-field/snippet citations must resolve. Generated prose still requires `check-claims` or `verify-output`.
- `run-receipt`: record and gate the host-owned context boundary plus artifact hashes; audit-grade proposal review requires `--isolation isolated`, `--declared-inputs-only`, successful validation whose artifact hashes match the supplied prompt-output and source-audit files, a runner audit whose prompt-output hash matches the supplied prompt output and reports `tool_invocations_observed: 0`, source audit when documents/PDFs were normalized, and for production pilots `--runner-audit ... --require-runner-audit`.
- `scripts/mdp-proposal-runner.mjs` (or `${PLUGIN_ROOT}/scripts/mdp-proposal-runner.mjs` in installed bundles): host-neutral local proposal runner surface. Use `tools` to inspect local runner steps. Use `run --dry-run` for request hygiene, `run --mock-response` for fixture safety, and real `run --model ...` only when the operator chose a real native call.
- Treat `scripts/lib/proposal-runner-*.mjs` as bundled internal implementation modules. Invoke the runner or MCP entrypoint rather than importing those modules as a public API.
- `scripts/mdp-proposal-mcp-server.mjs` (or `${PLUGIN_ROOT}/scripts/mdp-proposal-mcp-server.mjs` in installed bundles): local stdio MCP wrapper exposing `mdp_proposal_tools` and file/path-only `mdp_proposal_run`. It is not hosted/remote, does not accept raw chat text as source evidence, and dry-run/mock runs are never audit-grade.
- `scripts/mdp-native-model-openai.mjs` (or `${PLUGIN_ROOT}/scripts/mdp-native-model-openai.mjs`): internal profile-neutral OpenAI Responses subprocess for one selected declared model step. Operators normally invoke it through `mdp run`. Real calls require both startup environment values `MDP_ALLOW_NATIVE_MODEL_CALLS=1` and `OPENAI_API_KEY`; mock/dry-run validation is key-free and does not prove a call. The official endpoint is fixed. `scripts/mdp-native-normalize-openai.mjs` is a v0 compatibility adapter.
- `schema driver-request-v2` and `schema driver-result-v2`: inspect the closed versioned CLI-to-subprocess boundary. These are driver contracts, not substitutes for operator-authored `run-request-v1`.
- `fit`: decide fit, insufficient context, or disqualification for supplied GTM prospect JSON.
- `brief --context`: build bounded GTM decision context after fit permits it.
- `check-claims`: test supplied claim-bearing text and output constraints.
- `author-proof-output`: compile proof-output drafts into verified proof-output JSON; writes only after verifier success.
- `verify-output`: verify proof-carrying output against loaded pack IDs.
- `eval`: run committed pack fixtures.

Do not reproduce these decisions manually in a skill.

## Offline Preparation, Clean Runs, And Decision Authority

Use `prepare-run` for the normal generative path. It resolves the canonical
model step, reads exact declared local files, derives all request identities,
and emits the unchanged `mdp.run-request.v1` without a provider call:

```bash
mdp --json prepare-run --dir PACK_ROOT --job JOB \
  --operation model:JOB/PHASE --model MODEL \
  --input LOGICAL_NAME=PATH --out RUN_REQUEST_JSON \
  --manifest-out COMPILE_MANIFEST_JSON
```

The default output is concise; `--full` includes the compiler manifest and
request projection. Provider authorization is explicitly
`required-at-execution`. Do not hand-author execution IDs, prompt paths,
pack-release IDs, policy hashes, driver hashes, or model-parameter hashes.
Hand-authored requests are compatibility/negative-test fixtures only.

Use one file-oriented v1 request for both proposal and GTM:

```bash
mdp --json schema run-request-v1
mdp --json run --request RUN_REQUEST_JSON --out-dir NEW_RUN_DIRECTORY
mdp --json verify-run --bundle NEW_RUN_DIRECTORY/run-bundle.json \
  --receipt NEW_RUN_DIRECTORY/run-receipt.json \
  --artifact-root NEW_RUN_DIRECTORY
```

Use an external customer-controlled scratch/work directory for
`NEW_RUN_DIRECTORY`; it must not equal or descend from `PACK_ROOT`. The CLI
and MCP adapter canonicalize the relationship, reject unsafe paths before
writing, and return `output-directory-inside-pack`. Existing generated
directories under a pack are reported for manual relocation; validation never
deletes them.

For MCP-capable coding-agent hosts, launch the profile-neutral local stdio
adapter from the source checkout or installed plugin bundle:

```bash
node scripts/mdp-run-mcp-server.mjs
node "${PLUGIN_ROOT}/scripts/mdp-run-mcp-server.mjs"
```

It exposes `mdp_run_tools`, `mdp_run`, and read-only `mdp_verify_run`.
`mdp_run` accepts only `request_path`, a new `output_dir`, and an optional
bounded `timeout_ms`. `mdp_verify_run` accepts existing bundle and receipt paths,
an optional artifact root, and the same bounded deadline. Each tool spawns the
matching CLI command as a separate process with a bounded environment, stdin,
output buffer, and deadline, then returns the canonical CLI data object unchanged.
It does not accept inline requests, raw source bodies, ambient chat, provider
credentials, native-call enable flags, or assurance overrides. For a parsed
generative request only, it may inherit `OPENAI_API_KEY` and
`MDP_ALLOW_NATIVE_MODEL_CALLS` if they were present when the server started.
Configure the host to start this server; do not paste evidence into its tool
arguments.

For a generative request, `operation` must equal one stable step ID from
`requirements.data.model_steps`. One run means one selected declared model
step and one receipt. The host separately sequences normalization,
deterministic fit/routing, and generation/review; MDP does not batch, retry,
collect, send, mutate CRM, or calculate inference pricing.

On timeout or output overflow, the adapter closes the isolated process group
before recovery. It removes staging state only when the CLI's bounded
`mdp.run-recovery-claim.v1` names the exact transaction for the requested
output and execution ID and the claim/transaction pass file-type, link,
ownership, component-name, and canonical-parent checks. A missing or malformed
claim fails closed and never authorizes wildcard cleanup.

The runtime must create `NEW_RUN_DIRECTORY`; never point it at an existing
workdir. Keep the request, released pack digest, declared input manifest,
prompt, execution policy, driver/model identity, audit, output, decision,
compiled context, validation, and receipt together. A non-success state is
`no-draft` and must not expose partial model output or decision authority.

The original conversation may explain the result but must not add evidence or
change the authoritative decision. Copy the CLI-rendered authority block
verbatim. Treat contradictory ambient prose as unreceipted commentary.

The MCP process boundary keeps the surrounding agent in the control plane; it
does not itself prove fresh model context or isolation. The Rust CLI remains
the sole authority for request parsing, staging, execution, terminal state,
assurance, validation, artifact hashes, and receipts. Never promote an
assurance dimension because the invocation used MCP.

For `no-draft:policy-blocked`, read the CLI-owned
`authority_block.diagnostics` array for bounded troubleshooting context. It
uses stable stage/gate/category values, logical input names, safe contract-field
pointers, and allowlisted expected/observed values. It distinguishes malformed
JSON, wrong contract, missing or disallowed fields, readiness failure, stale
binding, and internal contract mismatch. It never includes source bodies,
private paths, credentials, parser text, or partial output. Keep
`reason_codes`, terminal state, and null decision/hash fields authoritative;
MCP must copy diagnostics without summarizing or reclassifying them. Routed
context is the canonical `mdp.routed-context.v1` artifact and does not require
top-level `status` or `draft_status` fields.

`verify-run` is integrity-only; it does not establish freshness from external
host state. A table/job host that needs replay protection must atomically
consume the verified receipt in host-owned durable storage. `consume-run` is a
local conformance reference with explicit rollback and cloning limitations.

## Artifact Writes

Preview commands that support `--dry-run` before writing:

```bash
mdp --json init --template gtm --name PACK_NAME --target-name TARGET_NAME --target-kind company --target-alias TARGET_ALIAS --exclude-term PRIOR_TARGET --dir PACK_ROOT --dry-run
mdp --json brief --context --dir PACK_ROOT --prospect PROSPECT_JSON --out BRIEF_JSON --dry-run
mdp --json emit-brief --dir PACK_ROOT --persona PERSONA --out BRIEF_JSON --dry-run
mdp --json pack --dir PACK_ROOT --out PACK_JSON --dry-run
mdp --json author-proof-output --dir PACK_ROOT --draft PROOF_OUTPUT_DRAFT_JSON --out PROOF_OUTPUT_JSON --dry-run
mdp --json run-receipt --dir PACK_ROOT --workflow proposal-review --isolation isolated --declared-inputs-only --prompt-id normalize-opportunity --prompt-output OUTPUT_JSON --validation VALIDATION_JSON --source-audit SOURCE_AUDIT_JSON --runner-audit RUNNER_AUDIT_JSON --require-runner-audit --out RUN_RECEIPT_JSON --dry-run
```

For a named GTM pack, pass `--target-name` explicitly. Repeat `--target-alias` and `--exclude-term` when needed; never force-retarget an existing pack directory.

Write a durable artifact only when the user asks for one or the task requires a repository change.
