---
name: mdp-gtm-brief
description: Use when applying a GTM Message Decision Pack to a supplied prospect for fit/readiness, bounded pre-draft context, or review of supplied outbound copy. Do not use for generic prospecting, enrichment, copywriting, sending, CRM, or proposals.
---

# MDP GTM Brief

Use a GTM pack for bounded decision support. Never enrich, draft outreach, send, schedule, or update a CRM.

## Select One Mode

Map the explicit user intent to one job ID:

- Fit decision or fit plus a prospect brief: `prospect-fit-or-brief`
- Pre-draft writing context, not copy: `outbound-copy-brief`
- Evaluation of copy the user supplied: `outbound-copy-review`

Validate the selected route before doing any profile work:

```bash
mdp --json capabilities
mdp --json skills --dir PACK_ROOT --job JOB_ID
```

Proceed only when `data.recommendation.skill_id` is `mdp-gtm-brief`, the returned `job_id` matches, and `pack_ready` is true. Otherwise report the diagnostics and stop or route pack repair to `$mdp-pack-builder`. There is no fallback job.

## Shared Gate

1. Require the exact pack root and supplied prospect/source context. Do not collect missing prospect data through this skill.
2. When the manifest declares a target, confirm the requested work is for that exact company, product, or project. Do not apply the pack to a different external target, and do not infer capabilities or fit from target identity alone.
3. Validate the pack before using it:

```bash
mdp --json validate --dir PACK_ROOT
mdp --json gaps --dir PACK_ROOT
```

4. Before using any pack-owned prompt, inspect the selected job's compiled
   requirements handoff:

```bash
mdp --json requirements --dir PACK_ROOT --job JOB_ID
```

For `outbound-copy-brief` or `outbound-copy-review`, ignore Decision Input
`data.available`; that field describes the normalization handoff, not whether
the selected generation or review job is runnable. Require
`data.model_task.status` to be exactly `ready`. Both modes require a supplied
prospect artifact: review copy alone is insufficient to compile bounded
context. With a prospect JSON file, run:

```bash
mdp --json brief --dir PACK_ROOT --prospect PROSPECT_JSON --job JOB_ID \
  --channel CHANNEL --context --routed-context-out ROUTED_CONTEXT_JSON
```

With a validated v2 normalized artifact, use the same lineage inputs from step
5:

```bash
mdp --json brief --dir PACK_ROOT --normalized-input OUTPUT_JSON \
  --prompt BOUND_PROMPT_PATH --source-binding SOURCE_BINDING_JSON \
  --source-attempt-request SOURCE_ATTEMPT_REQUEST_JSON \
  --collected-attempt-results COLLECTED_ATTEMPT_RESULTS_JSON --job JOB_ID \
  --channel CHANNEL --context --routed-context-out ROUTED_CONTEXT_JSON
```

Use only the exact compiled prompt, declared inputs, version, output schema,
and a `ready` minimal-context receipt. Require the routed-context artifact to
be saved; do not open excluded entries or the whole pack. The customer-selected
host owns the model call. Validate the returned
governed artifact with `--invocation-receipt PROMPT_INVOCATION_JSON` and
`--routed-context ROUTED_CONTEXT_JSON`, then run
`mdp check-claims` on generated or supplied copy. The host receipt must bind
the exact job, prompt ID/version/SHA-256, and per-declared-input SHA-256 values,
including the canonical routed-context bytes.

When this job is cold-model conformance evidence, require a passing
`conformance compile` before handing anything to the external host. After its
call, validate the recorded invocation/trial/evaluator artifacts and assemble
`mdp.job-conformance.v1`. The behavioral evaluation is intermediate, not
report authority. `sufficient-for-job` is deterministic only; only the
assembled result can be `qualified-for-job-under-envelope`. `unassessed` and
`conformance-failure` remain no-draft. Conformance never authorizes this skill
to draft or send copy.
If `data.model_task` is absent, state that no job-owned model task is declared,
include any available product-foundation diagnostics, and stop no-draft; do not
invent a model-task status or diagnostics. If `data.model_task` is present but
its status is not `ready`, report its exact diagnostics and stop no-draft.
Never enter the legacy prompt-output path or replace the contract with
skill-implied writing or review instructions.

5. For `prospect-fit-or-brief` normalization only, branch on
   Decision Input `data.available`:
   - When `true`, do not collect or normalize inside this skill.
     Inspect `data.runtime_contract_version`,
     `data.contract_version_matrix`, and every compiled signal projection. Do
     not mix v1/v2 artifacts or infer roles from prose. For v2, require the
     exact `SOURCE_BINDING_JSON` selected by requirements.
     - If all four artifacts—`SOURCE_BINDING_JSON`,
       `SOURCE_ATTEMPT_REQUEST_JSON`, `COLLECTED_ATTEMPT_RESULTS_JSON`, and
       `OUTPUT_JSON`—are already supplied,
       validate them immediately with the returned bound prompt.
     - If any artifact is missing, hand the customer or host the exact
       complete `mdp --json requirements` result as
       `DECISION_INPUT_REQUIREMENTS_JSON`, including
       `data.source_attempt_request_schema`,
       `data.collected_attempt_results_schema`,
       `data.normalized_output_schema`, and the contract/prompt receipts, plus
       the returned bound prompt; then stop. Require the customer or host to
       instantiate the request, populate
       its exact `contract`, `job_id`, and `decision_input_contracts` ID/version
       receipts, and set a trusted UTC `as_of`. The host must preserve those
       exact request bytes as `SOURCE_ATTEMPT_REQUEST_JSON`, compute their
       SHA-256 as `SOURCE_ATTEMPT_REQUEST_SHA256`, execute every compiled
       attempt, and record the statuses, values, evidence, timestamps,
       confidence, and errors in a separate attempted-complete
       `COLLECTED_ATTEMPT_RESULTS_JSON` ledger. Invoke the bound prompt with all
       four required inputs:
       - `raw_row`: `COLLECTED_ATTEMPT_RESULTS_JSON`
       - `decision_input_requirements`: `DECISION_INPUT_REQUIREMENTS_JSON.data`
       - `source_attempt_request_sha256`: `SOURCE_ATTEMPT_REQUEST_SHA256`
       - `collected_attempt_results_sha256`:
         `COLLECTED_ATTEMPT_RESULTS_SHA256`

       Resume only after the host returns all three exact artifacts: the
       preserved request file, the collected-results ledger used as `raw_row`,
       and the normalized output.
     - For either the already-supplied or resumed path, validate:

       The command below is the v2 form. For scalar v1, omit
       `--source-binding` and keep every artifact on the v1 matrix row.

     ```bash
     mdp --json validate-prompt-output --dir PACK_ROOT --prompt BOUND_PROMPT_PATH \
       --source-binding SOURCE_BINDING_JSON \
       --source-attempt-request SOURCE_ATTEMPT_REQUEST_JSON \
       --collected-attempt-results COLLECTED_ATTEMPT_RESULTS_JSON \
       --file OUTPUT_JSON
     ```

     Continue only when validation passes and the top-level `outcome` is exactly
     `ready`. Every other outcome stops no-draft before extracting
     `normalized_prospect`.
     For v2, do not extract `normalized_prospect`. Run the lineage-aware path:

     ```bash
     mdp --json fit --dir PACK_ROOT --job prospect-fit-or-brief \
       --normalized-input OUTPUT_JSON --prompt BOUND_PROMPT_PATH \
       --source-binding SOURCE_BINDING_JSON \
       --source-attempt-request SOURCE_ATTEMPT_REQUEST_JSON \
       --collected-attempt-results COLLECTED_ATTEMPT_RESULTS_JSON
     ```

     Use the same lineage flags with `brief --context`. Report each accepted or
     rejected projection ID, explicit role, authority class, and bounded
     diagnostic. Preserve all conflict receipts. Unresolved
     `require-agreement` conflicts stop human-review/no-draft;
     `any-disqualifies` may only disqualify. Never choose a positive winner.
     `lineage-validated` means internal chain consistency only, not host
     authenticity or observation truth.
   - When `false`, retain the legacy `mdp.prompt-output.v0` normalization path.
     This compatibility path never authorizes `outbound-copy-brief` or
     `outbound-copy-review`. Validate
     without a source-attempt request:

     ```bash
     mdp --json validate-prompt-output --dir PACK_ROOT \
       --prompt-id PROMPT_ID --file OUTPUT_JSON
     ```

     Continue only when validation passes and
     `normalization_trace.fit_readiness.ready_for_mdp_fit` is exactly `true`
     before extracting `normalized_prospect`.
6. Never invent a person, title, signal, date, persona, segment, or required attribute. Account-only context stays insufficient/no-draft when the pack requires person readiness.
7. Treat synthetic fixtures as `do_not_contact`; they are for testing only.

## Load The Mode Reference

- Read [references/prospect-fit-or-brief.md](references/prospect-fit-or-brief.md) for normalization, fit, and optional prospect brief generation.
- Read [references/outbound-copy-brief.md](references/outbound-copy-brief.md) for a bounded pre-draft writing contract.
- Read [references/outbound-copy-review.md](references/outbound-copy-review.md) for reviewing supplied copy.

Load only the selected mode.

## Common Rules

- `mdp fit` owns fit, insufficient-context, and disqualified decisions.
- `mdp brief --context` owns bounded GTM context. Include only routed entries, safe personalization, and known gaps.
- `mdp check-claims` owns deterministic claim and output-rule checks for supplied text.
- Supply every required portfolio `--scope` dimension; never silently choose a product, brand, region, or offer.
- A passing claim check is not approval to send.
- Preserve CLI diagnostics and gaps verbatim enough for the next reviewer to act.
- Legacy or detached prospect signals remain `legacy` or `unassessed` and
  cannot satisfy explicit `fit`, `why-now`, `person-resolution`, or
  `disqualifier` roles through keywords or source prose.
- When decision authority must be isolated from a context-rich authoring chat,
  create a deterministic `mdp.run-request.v1` and use `mdp run`; do not call a
  model driver. Verify the returned bundle and receipt before presenting the
  decision. Inference assurance is `not-applicable`, while upstream source and
  normalization provenance remain separate limitations.
- MDP returns qualification and bounded context. Campaign drafting, table
  batching, retries, enrichment, outbound, and CRM actions remain host-owned.

## Response

Report the job ID, pack/profile state, input source, CLI decision, evidence used, gaps, and next allowed action. State explicitly that no enrichment, drafting, sending, scheduling, or CRM mutation occurred.
