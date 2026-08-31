# Prospect Fit Or Brief

Read this only for `prospect-fit-or-brief`.

## Prospect Contract

Inspect the current shape when needed:

```bash
mdp --json schema prospect
```

Signals carry observed evidence and provenance. Attributes carry bounded reviewed row metadata. Do not use attributes as invented evidence.

For legacy `mdp.prompt-output.v0` normalization, `source_summary.inputs_used`
should name exact declared inputs such as `raw_row` or
`existing_pack_context`. Field paths, URLs, snippets, and other source locators
belong in `signals[].source` and `normalization_trace`, not in `inputs_used`.

## Workflow

For the normal path, follow Managed Workflow Bundle Handoff.
Keep requirements, source-attempt requests, collected results, normalization,
routing, and prompt receipts inside the invocation-owned restricted scratch
root. The user supplies only the exact pack, job, and approved prospect/source
inputs and receives one verified durable run directory or the canonical
no-draft/blocked result. The lineage choreography below is an advanced
implementation detail; do not ask the user to carry its intermediate paths or
bodies through chat.

1. Run `mdp --json requirements --dir PACK_ROOT --job prospect-fit-or-brief`.
2. Branch on `data.available` from requirements:
   - When `true`, this skill must not collect missing prospect data or run the
     customer-funded normalization call.
     Inspect the runtime version matrix, compiled collection specification,
     selected taxonomies, and signal projections first. For v2 or v3, require
     the exact `SOURCE_BINDING_JSON`; roles never come from keywords. For v3,
     the host supplies researched title, responsibilities, company-fit, and
     separate why-now observations without pre-assigning persona or segment.
     Run the resolved normalization step through generative `mdp run` so the
     CLI—not the provider—validates classification lineage and seals the
     neutral envelope.
     - If all four artifacts—`SOURCE_BINDING_JSON`,
       `SOURCE_ATTEMPT_REQUEST_JSON`, `COLLECTED_ATTEMPT_RESULTS_JSON`, and
       `OUTPUT_JSON`—are already supplied,
       validate them immediately with the bound prompt.
     - If any artifact is missing, hand the customer or host the exact
       complete `mdp --json requirements` result as
       `DECISION_INPUT_REQUIREMENTS_JSON`, including
       `data.source_attempt_request_schema`,
       `data.collected_attempt_results_schema`,
       `data.normalized_output_schema`, and all contract/prompt receipts, plus
       the bound prompt; then stop.
       Require the host to instantiate the request, populate its exact
       `contract`, `job_id`, and `decision_input_contracts` ID/version receipts;
       and set a trusted UTC `as_of`. The host must preserve those exact request
       bytes as `SOURCE_ATTEMPT_REQUEST_JSON`, compute their SHA-256 as
       `SOURCE_ATTEMPT_REQUEST_SHA256`, execute every compiled attempt, and
       record the statuses, values, evidence, timestamps, confidence, and
       errors in a separate attempted-complete
       `COLLECTED_ATTEMPT_RESULTS_JSON` ledger. Invoke the bound prompt with all
       four required inputs:
       - `raw_row`: `COLLECTED_ATTEMPT_RESULTS_JSON`
       - `decision_input_requirements`: `DECISION_INPUT_REQUIREMENTS_JSON.data`
       - `source_attempt_request_sha256`: `SOURCE_ATTEMPT_REQUEST_SHA256`
       - `collected_attempt_results_sha256`:
         `COLLECTED_ATTEMPT_RESULTS_SHA256`

       Resume only when the host returns all three exact artifacts: the
       preserved request file, the collected-results ledger used as `raw_row`,
       and the normalized output.
     - For either the already-supplied or resumed path, keep the exact binding,
       request, results, prompt, and output bytes together.
     - For v3, verify the normalization run bundle and receipt, then use its
       exact sealed output. The provider semantic payload is not itself a
       normalized decision input. For v1/v2, validate the compatibility
       envelope against the exact source binding, request, and collected-results
       files and require its governed ready outcome.
     - For v2 or v3, do not extract a detached prospect. Run `mdp --json fit` with
       `--normalized-input OUTPUT_JSON`, `--prompt BOUND_PROMPT_PATH`,
       `--source-binding SOURCE_BINDING_JSON`, `--source-attempt-request
       SOURCE_ATTEMPT_REQUEST_JSON`, `--collected-attempt-results
       COLLECTED_ATTEMPT_RESULTS_JSON`, and `--job prospect-fit-or-brief`.
   - When `false`, normalize supplied source material with the selected legacy
     pack prompt, then validate the `mdp.prompt-output.v0` output without a
     source-attempt request. Stop before extracting `normalized_prospect`
     unless validation passes and
     `normalization_trace.fit_readiness.ready_for_mdp_fit` is exactly `true`.
3. For the legacy path only, first confirm the selected job has no direct or
   transitive Decision Input Contract. Always pass the selected job so the CLI
   cannot silently choose or ignore one, then run:

```bash
mdp --json fit --dir PACK_ROOT --job JOB_ID --prospect PROSPECT_JSON
```

4. If the user asked only for fit, return status, matched rules, disqualifiers, qualification gates, missing/invalid requirements, and gaps.
5. If the user asked for a brief and fit permits it, preserve the same runtime-version boundary used for fit.

For v2 or v3, keep the verified envelope attached:

```bash
mdp --json --summary brief --context --dir PACK_ROOT --normalized-input OUTPUT_JSON --prompt BOUND_PROMPT_PATH --source-binding SOURCE_BINDING_JSON --source-attempt-request SOURCE_ATTEMPT_REQUEST_JSON --collected-attempt-results COLLECTED_ATTEMPT_RESULTS_JSON --job prospect-fit-or-brief --channel CHANNEL
```

For a truly ungoverned legacy job only, use the detached prospect:

```bash
mdp --json --summary brief --context --dir PACK_ROOT --job JOB_ID --prospect PROSPECT_JSON --channel CHANNEL
```

Use `--out BRIEF_JSON --dry-run` before a requested durable write. Use `--readable` only when the user wants Markdown.

## Fail Closed

- Insufficient or disqualified means no draft-ready brief.
- Missing person readiness means no invented contact.
- Unknown contract values remain validation issues or gaps; do not silently coerce them.
- `governed_job_requires_normalized_input` is terminal. Do not retry through
  detached input, extract `normalized_prospect`, or draft from the blocked context.
- Report `lineage-validated`, `legacy`, and `unassessed` exactly. Lineage
  validation proves internal linkage, not host authenticity or truth.
- For a lineage-validated v2 or v3 normalized input, accepted signal observations are
  the canonical signal set for readiness and qualification. Do not require a
  duplicate `normalized_prospect.signals[]` value to satisfy a pack's legacy
  `signals`/`signals.source` readiness declarations; scalar/v1 and explicitly
  supported legacy paths remain bounded to their prospect `signals[]` input.
- Preserve conflict receipts and stop no-draft on unresolved conflict. Never
  choose a newest or highest-confidence positive winner.
