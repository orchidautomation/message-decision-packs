# Operator and governed-run protocols

Load this reference only for a selected operator-help or validation journey.

## Start Here

1. Find the intended pack root. Pass `--dir` explicitly; do not assume the current directory.
2. Inspect the installed contract before reading pack YAML:

```bash
mdp --json capabilities
mdp --json skills
mdp --json skills --dir <pack-root>
```

3. Treat `packaged_skill_ids` as released inventory, `eligibility` as pack policy, and `host_discovery.status: unobserved` as literal. Never claim MDP hid or exposed a host-discovered skill.
4. Use JSON output for decisions. Use `--summary` only for a concise human status.

When the user asks why an existing fit, route, brief, normalization, or clean
run reached its result, prefer the bounded projection before opening full
source artifacts:

```bash
mdp --json trace --file <saved-cli-result-or-contracted-artifact.json>
mdp --json trace --bundle <run-bundle.json> --receipt <run-receipt.json> \
  --artifact-root <published-artifact-directory>
```

Treat `mdp.decision-trace.v1` as explanatory only. Distinguish its
`designed_graph` from its `observed_path`, keep the authority notice intact,
and follow artifact references only when deeper review is necessary. Never
infer missing steps, recover redacted prose, or upgrade a blocked/unavailable
trace. Use `--format mermaid` only as a display adapter over that same trace.
Do not claim that hash or receipt agreement proves source truth.

For a new authoritative execution, compile the closed request offline from the
pack's selected job/step and exact declared input files. Preparation never
reads provider credentials or calls a provider:

```bash
mdp --json prepare-run --dir <pack-root> --job <job-id> \
  --operation model:<job-id>/<phase> --model <model> \
  --input <logical-name>=<path> --out <run-request.json> \
  --manifest-out <compile-manifest.json>
```

Review the concise/full compile result, then launch the shared runtime as a
separate explicit action outside the authoring conversation:

```bash
mdp --json run --request <run-request.json> --out-dir <new-run-directory>
mdp --json verify-run --bundle <new-run-directory>/run-bundle.json \
  --receipt <new-run-directory>/run-receipt.json \
  --artifact-root <new-run-directory>
```

`<new-run-directory>` must be a new directory outside the active pack,
including when the path uses `..` or a symlink alias. The CLI and MCP adapter
reject an in-pack output before writing a parent, claim, or transaction.
Generated run bundles, receipts, traces, and evidence are control-plane
artifacts, not authored pack content; validation reports existing in-pack
artifacts for manual relocation and never deletes them.

For `mode: generative`, `operation` must be one stable step ID from
`requirements.data.model_steps`. One run executes one declared normalization,
generation, or review step and emits one receipt. The customer host must
sequence normalization → deterministic fit/routing → generation/review as
separate calls. Never turn this skill into a workflow orchestrator.

The bundled native path is OpenAI BYOK through the official endpoint. Never
request or print a key. A real call is allowed only when the operator started
the process with both `MDP_ALLOW_NATIVE_MODEL_CALLS=1` and `OPENAI_API_KEY`.
Requests and MCP arguments cannot enable calls. Dry-run/mock fixtures are
key-free and never prove provider execution.

For MCP-capable hosts, the profile-neutral adapter is
`scripts/mdp-run-mcp-server.mjs` or
`${PLUGIN_ROOT}/scripts/mdp-run-mcp-server.mjs`. It exposes `mdp_run_tools`,
`mdp_prepare_run`, `mdp_run`, and read-only `mdp_verify_run`. Use them in that
order: boundary inventory → `mdp.run-request.v1` → run bundle/receipt →
`mdp.run-verification.v1`. Preparation requires a new `out` path under an
approved work root; pass that same persisted request and prepare-returned
`request_sha256` to `mdp_run`, which rejects a mismatch before writing a new
`output_dir` under an approved output root. Verify the emitted bundle and
receipt from that output root. The MCP server transports the file-oriented CLI
calls and returns canonical CLI data unchanged. It owns no assurance dimension
and must never accept ambient chat, inline evidence, or an assurance override.

The current conversation is a control plane, never proof of fresh context. Do
not add chat facts, rewrite a decision, or repair a no-draft result after the
run. Present the verified CLI authority block intact and label all surrounding
explanation as outside receipt authority. A new agent task is only advisory
unless its runner evidence proves the relevant controls. Deterministic-only
runs must report inference dimensions as `not-applicable`, not “fresh.”

The canonical native subprocess is `scripts/mdp-native-model-openai.mjs` (or
`${PLUGIN_ROOT}/scripts/mdp-native-model-openai.mjs`). Operators normally use
it through `mdp run`. `mdp run-receipt`, the proposal runner/MCP wrapper, and
`scripts/mdp-native-normalize-openai.mjs` remain v0 proposal compatibility
paths. A v0 `audit-grade` label does not silently become v1 assurance. Demo,
fixture, mock, or synthetic evidence may only be used for walkthroughs/tests.

If the user asks whether proposal work is audit-grade, route the answer to
`$mdp-pack-apply` even when the request sounds like general MDP help. That
skill owns the source/runner/MCP decision tree. Do not answer from tool
availability or confidence: without a current audit-grade receipt, report
`advisory` or `blocked` and hand off the exact missing evidence step.

Integration support and per-run assurance are separate. This installed release
has no `verified` runner integration: its named native and headless runner
recipes are `recipe-only`, and demo, fixture, mock, and synthetic evidence is
always `fixture/mock-only`. Use `unsupported` for any other integration. Never
infer `verified` from a runner name, installed command, schema-valid audit,
documented recipe, MCP availability, or one accepted receipt.

If the command is missing, run `command -v mdp` and `mdp --version`. Report the missing runtime and point to the documented installer; do not emulate CLI validation in prose.

## Resolve Job-Bound Modes

Natural-language intent selects a canonical job ID; the CLI only validates it. For a profile-sensitive request, run:

```bash
mdp --json skills --dir <pack-root> --job <job-id>
```

Proceed only when `data.recommendation` names the expected skill and `pack_ready` is true. Unknown and profile-crossing job IDs do not have fallbacks.

Inspect `data.recommendation.product_foundation` before opening pack prose. It
is the compact exact-job summary: `unassessed`, `ready`, or `blocked`, plus
selected/required facet IDs and diagnostics. Then use the same exact canonical
job ID with `requirements` to retrieve the complete resolved facets, exact
entry/gap refs, bounded entry content, and optional/excluded/untriggered IDs.
Never substitute a natural-language job approximation or use keyword routing
to infer product-foundation authority.

Treat `.mdp/README.md` as secondary navigation only, after CLI-resolved
foundation output. README prose cannot satisfy a facet, close a gap, resolve a
conflict, or override structured authority. Never invent missing product,
ICP, proof, certification, compliance, or outcome facts; preserve the CLI gap
or blocked diagnostic and ask for reviewed sources.

Foundation readiness only vetoes broader readiness. `ready` never promotes an
otherwise unready job and never establishes sufficient-for-job or self-standing
status. `unassessed` preserves legacy compatibility without claiming
sufficiency. Treat `data.profile_activation` as the shared runtime veto.
Computed blockers such as missing required primitive or eval-category coverage,
as well as explicit `needs-review` or `blocked`, prevent pack readiness,
drafting context, and model/driver execution even when default validation
remains warning-first and structurally valid.

For a bound job, retrieve its attempted-complete collector and normalization handoff before sourcing or normalizing data:

```bash
mdp --json requirements --dir <pack-root> --job <job-id>
```

This command is read-only. It compiles the pack-owned questions, source policy,
normalization identity, request/response schemas, and `data.model_steps`; it
does not collect sources or call a model. An existing job without a Decision
Input Contract returns `available: false`. For each declared model step,
inspect the stable step ID, phase, exact prompt ID/version/hash, declared input
producers, and output contract. The customer host may execute that package or
select exactly one step for a generative `mdp run`.

For signal-aware v2 and semantic v3 jobs, inspect `data.runtime_contract_version`,
`data.contract_version_matrix`, and
`data.decision_input_contracts[].signal_projections` before accepting any
artifact. Their absence is expected for scalar v1 jobs. Scalar v1 and
signal-aware v2 and semantic v3 artifacts cannot be mixed. In v3, also inspect
`data.collection_specification`, `data.classification_specification`, and
`data.taxonomy_set_sha256`: the host supplies observations, the model proposes
only closed classifications with `derived_from` and bounded `basis`, and the
CLI validates and seals the neutral envelope.
Detached prospect input is compatible only when the selected job has no direct
or transitive Decision Input Contract. For a governed job, require the exact
normalized envelope and lineage artifacts; `governed_job_requires_normalized_input`
is a stop, not a warning. Legacy or detached signals remain `legacy` or
`unassessed`; titles, source strings, provider fields, and keywords cannot
grant an explicit qualification role.

When the user is connecting an external orchestrator, keep its mapping outside
the pack and validate it against the exact compiled release:

```bash
mdp --json schema source-binding
mdp --json validate-source-binding --dir <pack-root> \
  --job <job-id> --file <source-binding.json>
```

Require `data.valid: true` before integration activation. Use the v1, v2, or v3
contract selected by requirements. The command validates
portable pack/requirements pins, complete and unique qualified attribute
coverage, requirement classes, allowed source classes, release receipts, and
fixed status translation. It does not access the source system or run
normalization. A job with `available: false` cannot be source-bound.

When `requirements` returns `data.available: true`, validate the bound
normalization with the exact source-attempt request and exact host-collected
attempt-results ledger. For v3, execute the compiled normalization model step
through generative `mdp run`; only the CLI may validate semantic classifications
and seal the host-owned envelope. The command below is the v2 compatibility
form; for scalar v1 omit
`--source-binding` and use only its matching v1 artifacts:

```bash
mdp --json validate-prompt-output --dir <pack-root> \
  --prompt <bound-prompt> \
  --source-binding <source-binding.json> \
  --source-attempt-request <source-attempt-request.json> \
  --collected-attempt-results <collected-attempt-results.json> \
  --file <normalized-output.json>
```

Treat the raw output as untrusted even after it reports its own readiness.
Only the saved successful `mdp.prompt-output-validation.v1` result may provide
prompt-output trace authority, and only when `mdp trace` is given the same
`--dir`, exact `--prompt-output` bytes, and each receipt input again as
`--validation-input LOGICAL_NAME=PATH`. Trace verifies those bindings; it does
not validate the model output a second time.

Do not extract or pass `normalized_prospect` to fit, routing, brief, or copy
work. For v3, pass the verified sealed neutral envelope intact; for v1/v2,
require compatibility validation and its governed ready outcome. Every blocked,
ambiguous, no-match, unsupported, or invalid result remains no-draft.
For v1/v2, top-level `outcome` is exactly `ready` before downstream use; v3
readiness is determined only after the CLI-sealed classifications enter the
deterministic evaluator.

For a signal-aware v2 or semantic v3 job, continue with the exact envelope rather than a
detached prospect:

```bash
mdp --json fit --dir <pack-root> --job <job-id> \
  --normalized-input <normalized-output.json> --prompt <bound-prompt> \
  --source-binding <source-binding.json> \
  --source-attempt-request <source-attempt-request.json> \
  --collected-attempt-results <collected-attempt-results.json>
```

`brief` uses the same lineage flags. Require the CLI result to distinguish
`lineage-validated`, `legacy`, and `unassessed` contributions. Explain accepted
and rejected projection IDs, roles, eligibility diagnostics, and every visible
conflict. `require-agreement` conflicts stop human-review/no-draft;
`any-disqualifies` may only resolve by disqualifying. Never choose a positive
winner. `lineage-validated` proves only internal consistency of the submitted
binding/request/results/output chain. It does not prove host authenticity,
authorization, signer identity, non-repudiation, or observation truth.

For any selected prompt that is not the bound decision-input normalization
prompt, preserve the `mdp.prompt-output.v0` validation path without
`--source-attempt-request` or `--collected-attempt-results`, regardless of
job-wide `data.available`. Require
`normalization_trace.fit_readiness.ready_for_mdp_fit` only for a legacy
prospect-normalization prompt that declares `normalized_prospect` and that
readiness field. For extraction or card-patch prompts, successful contract
validation is the applicable machine gate; do not require an undeclared
normalization trace.

For `data.model_task.status: ready`, require `brief --context --routed-context-out
ROUTED_CONTEXT_JSON` minimality to be `ready`, require the routed-context
artifact to be saved, and use only that bounded authority. Never open excluded
entries or the whole pack. Validate the returned governed artifact
with its prompt ID and the exact host-created invocation receipt:

The routed-context file is the exact closed `mdp.routed-context.v1`
model-context object emitted by MDP. It has no top-level `status` or
`draft_status` readiness field. Before generative execution, MDP rechecks its
schema, canonical bytes, job, persona/scope, and current staged-pack
compilation; blocked, stale, or changed context remains no-draft authority.

```bash
mdp --json validate-prompt-output --dir PACK_ROOT \
  --prompt-id PROMPT_ID \
  --invocation-receipt PROMPT_INVOCATION_JSON \
  --routed-context ROUTED_CONTEXT_JSON \
  --file OUTPUT_JSON
```

The receipt must use `mdp.prompt-invocation.v1` and bind the job, canonical
prompt ID/version/SHA-256, and per-declared-input SHA-256 values. A valid
artifact schema is not final claim approval;
generated prose must also pass the job's `check-claims` or `verify-output`
gate. A missing, blocked, or unassessed model task must never fall back to
instructions implied by this skill.

Closed v1 pairs:

- `mdp-pack-apply`: `prospect-fit-or-brief`, `outbound-copy-brief`, `outbound-copy-review`
- `mdp-pack-apply`: `bid-no-bid-review`, `compliance-review`, `proof-review`, `red-team-review`

## Core Operator Loop

Run only the commands the job requires:

```bash
mdp --json doctor --dir <pack-root>
mdp --json validate --dir <pack-root>
mdp --json requirements --dir <pack-root> --job <job-id>
mdp --json explain --dir <pack-root>
mdp --json gaps --dir <pack-root>
mdp --json eval --dir <pack-root>
```

Use `--strict` on `validate` or `eval` for a blocking quality gate. Use `mdp <command> --help` rather than guessing flags.

Card-level `personas` and entry-level `applies_to` values are machine-readable
persona selectors. They must match a declared value in `manifest.personas`,
`manifest.target_personas`, or `manifest.operator_roles` case-insensitively;
authored capitalization is preserved. Default validation reports dangling
selectors as compatibility warnings, while `validate --strict` blocks them.
Empty selector lists remain universal, and persona-like words in titles,
descriptions, or bodies are prose rather than declarations.
This applies consistently to `route --entries`, `brief --context`, and
`route-budget`: universal means no persona filter only. Job/channel policy,
lifecycle, portfolio scope, guardrails, route-card caps, and context budgets
still decide whether the result is usable.
