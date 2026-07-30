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

4. Before using a pack normalization prompt, inspect the selected job's
   Decision Input Contract handoff:

```bash
mdp --json requirements --dir PACK_ROOT --job JOB_ID
```

5. Branch on `data.available`:
   - When `true`, do not collect or normalize inside this skill.
     - If both `SOURCE_ATTEMPT_REQUEST_JSON` and `OUTPUT_JSON` are already
       supplied, validate them immediately with the returned bound prompt.
     - If either artifact is missing, hand the customer or host the exact
       complete `mdp --json requirements` result as
       `DECISION_INPUT_REQUIREMENTS_JSON`, including
       `data.source_attempt_request_schema`, `data.normalized_output_schema`,
       and the contract/prompt receipts, plus the returned bound prompt; then
       stop. Require the customer or host to instantiate the request, populate
       its exact `contract`, `job_id`, and `decision_input_contracts` ID/version
       receipts, set a trusted UTC `as_of`, record at least one
       attempt for every compiled attribute during collection, preserve and
       hash those exact request bytes, and normalize that request with the
       bound prompt. The host must pass
       `DECISION_INPUT_REQUIREMENTS_JSON.data` as the prompt's
       `decision_input_requirements` input. Resume only after the host returns
       both the preserved request file and normalized output.
     - For either the already-supplied or resumed path, validate:

     ```bash
     mdp --json validate-prompt-output --dir PACK_ROOT --prompt BOUND_PROMPT_PATH \
       --source-attempt-request SOURCE_ATTEMPT_REQUEST_JSON --file OUTPUT_JSON
     ```

     Continue only when validation passes and the top-level `outcome` is exactly
     `ready`. Every other outcome stops no-draft before extracting
     `normalized_prospect`.
   - When `false`, retain the legacy `mdp.prompt-output.v0` path. Validate
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

## Response

Report the job ID, pack/profile state, input source, CLI decision, evidence used, gaps, and next allowed action. State explicitly that no enrichment, drafting, sending, scheduling, or CRM mutation occurred.
