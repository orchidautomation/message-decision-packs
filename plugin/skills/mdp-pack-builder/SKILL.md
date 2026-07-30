---
name: mdp-pack-builder
description: Use when creating, initializing, reconstructing, or improving a Message Decision Pack from approved GTM, ICP, source, RFP, proposal, or capture material. Do not use for generic research, messaging strategy, proposal writing, or pack-only review.
---

# MDP Pack Builder

Build evidence-grounded `.mdp/` decision context. Use the CLI for deterministic structure and validation; use judgment only for interpreting approved source material and authoring explicit decisions.

## Intake Gate

1. Identify the pack root and intended profile: `gtm` or `proposal`.
2. For a real GTM pack, resolve the external company, product, or project being positioned separately from the pack display name. Record known aliases and prior-target or starter terms that must be excluded.
3. Classify each source as user-approved local material, approved corpus, public unauthenticated source, synthetic/sanitized example, needs approval, or excluded.
4. Ask for source authority when access or confidentiality would materially change the work. Never scrape gated sources or commit restricted source material.
5. Inspect the runtime and existing pack before editing:

```bash
mdp --json skills --dir PACK_ROOT
mdp --json doctor --dir PACK_ROOT
```

An invalid or absent pack still leaves this shared skill bootstrap-eligible.

## Initialize Or Inspect

For a new pack, preview then initialize:

```bash
mdp --json init --template gtm --name "PACK_NAME" --target-name "TARGET_NAME" --target-kind company --target-alias "TARGET_ALIAS" --exclude-term "PRIOR_TARGET" --dir PACK_ROOT --dry-run
mdp --json init --template gtm --name "PACK_NAME" --target-name "TARGET_NAME" --target-kind company --target-alias "TARGET_ALIAS" --exclude-term "PRIOR_TARGET" --dir PACK_ROOT
mdp --json init --template proposal --dir PACK_ROOT --dry-run
mdp --json init --template proposal --dir PACK_ROOT
```

Repeat `--target-alias` and `--exclude-term` as needed. A custom GTM pack name is not a substitute for `--target-name`; do not author into an ambiguous or previously targeted directory.

For an existing pack, run:

```bash
mdp --json validate --dir PACK_ROOT
mdp --json explain --dir PACK_ROOT
mdp --json gaps --dir PACK_ROOT
```

## Load Only The Needed References

- Read [references/source-intake.md](references/source-intake.md) when planning sources, extracting evidence, normalizing messy material, or mapping profile vocabulary to primitives.
- Read [references/decision-input-contracts.md](references/decision-input-contracts.md) when a job needs an explicit attempted-complete data contract before normalization, fit, routing, or drafting.
- Read [references/gtm-authoring.md](references/gtm-authoring.md) for ICP, personas, fit, signals, message angles, CTA policy, and GTM job bindings.
- Read [references/proposal-authoring.md](references/proposal-authoring.md) for proposal opportunity context, requirements, proof, confidentiality, and proposal job bindings.
- Read [references/boundaries-output.md](references/boundaries-output.md) when authoring claims, avoid rules, output constraints, or proof-carrying artifacts.

Do not read every reference by default.

## Authoring Loop

1. Preserve source receipts: source ID, file or URL, snippet, observed/as-of date, confidence, and approval class.
2. Map reviewed facts into universal primitives; keep profile terminology in labels and entries.
3. Separate observed evidence from inferred decisions. Put unresolved or unsupported material in gaps.
4. Keep every prospect-facing surface about the resolved external target. Pack, CLI, schema, prompt, card, eval, starter, and prior-target vocabulary is internal implementation context only.
5. When a job depends on collected or normalized data, author and bind its
   `decision_input_contracts` before writing the normalization prompt. Compile
   the job-specific questions and policy:

```bash
mdp --json requirements --dir PACK_ROOT --job JOB_ID
```

The compiled contract, not a generic finder or the normalization prompt, states
what data must be attempted. Keep collection and provider calls outside MDP.

6. Author prompts with explicit input and output contracts. Inspect the selected
   prompt's `output_contract.output_kind` and `output_contract.contract` before
   validating model-produced output:

   - Only when the selected prompt is the job-bound
     `decision-input-normalization` prompt producing
     `mdp.normalized-decision-input.v1`, require `data.available` to be `true`
     and hand the customer or host the complete exact requirements result as
     `DECISION_INPUT_REQUIREMENTS_JSON`, including
     `data.source_attempt_request_schema`,
     `data.collected_attempt_results_schema`,
     `data.normalized_output_schema`, and all contract/prompt receipts. The
     customer or host owns collection and paid normalization. It must construct
     one attempted-complete `SOURCE_ATTEMPT_REQUEST_JSON` matching the compiled
     request schema; populate the exact `contract`, `job_id`, and
     `decision_input_contracts` ID/version receipts; set a trusted UTC `as_of`;
     preserve the exact request bytes; and compute
     `SOURCE_ATTEMPT_REQUEST_SHA256`. It must execute every compiled attempt and
     record the exact statuses, values, evidence, timestamps, confidence, and
     errors in `COLLECTED_ATTEMPT_RESULTS_JSON`, matching
     `data.collected_attempt_results_schema`, then compute
     `COLLECTED_ATTEMPT_RESULTS_SHA256`. Invoke the bound prompt with:
     - `raw_row`: `COLLECTED_ATTEMPT_RESULTS_JSON`
     - `decision_input_requirements`: `DECISION_INPUT_REQUIREMENTS_JSON.data`
     - `source_attempt_request_sha256`: `SOURCE_ATTEMPT_REQUEST_SHA256`
     - `collected_attempt_results_sha256`:
       `COLLECTED_ATTEMPT_RESULTS_SHA256`

     Preserve both exact ledgers and pass them to validation:

```bash
mdp --json validate-prompt-output --dir PACK_ROOT --prompt BOUND_PROMPT_PATH \
  --source-attempt-request SOURCE_ATTEMPT_REQUEST_JSON \
  --collected-attempt-results COLLECTED_ATTEMPT_RESULTS_JSON \
  --file OUTPUT_JSON
```

     Stop before extracting or passing normalized data unless validation
     succeeds and the envelope's top-level `outcome` is exactly `ready`.
     Preserve the exact request and collected-results bytes and both SHA-256
     receipts with the normalized result.

   - For every other prompt output kind or contract, retain the normal
     `mdp.prompt-output.v0` path without a source-attempt request, regardless of
     the job-wide `data.available` value:

```bash
mdp --json validate-prompt-output --dir PACK_ROOT --prompt-id PROMPT_ID --file OUTPUT_JSON
```

     For a legacy prospect-normalization prompt whose declared output includes
     `normalized_prospect` and
     `normalization_trace.fit_readiness.ready_for_mdp_fit`, require the exact validated
     `normalization_trace.fit_readiness.ready_for_mdp_fit` field to equal
     `true` before fit, brief, routing, or drafting. For other v0 output kinds
     such as card-patch/extraction envelopes, successful contract validation is
     the applicable gate; do not require a normalization trace that the prompt
     does not declare.

Legacy prompt output contracts use `source_summary.inputs_used` for exact declared input names only. Put source paths, snippets, page locators, URLs, and proof notes in candidate `evidence`/`provenance`, `signals[].source`, `normalization_trace.preserved_raw_fields`, or `normalization_trace.missing_required[].source_evidence`. The prompt owns extraction/normalization, the manifest owns allowed values and readiness policy, the CLI owns enforcement, and downstream writers own wording only.

For proposal PDF/doc extraction, keep the source-audit ledger bounded and local/customer-controlled, then validate raw-field and snippet refs before using normalized opportunity facts:

```bash
mdp --json validate-prompt-output --dir PACK_ROOT --prompt-id normalize-opportunity --file OUTPUT_JSON --source-audit SOURCE_AUDIT_JSON
```

If this pack-build flow is also proving a sample proposal-review run, create a receipt from a fresh/stateless normalization call. Same-conversation normalization can inform authoring, but it is not audit-grade:

```bash
mdp --json run-receipt --dir PACK_ROOT --workflow proposal-review --isolation isolated --declared-inputs-only --prompt-id normalize-opportunity --prompt-output OUTPUT_JSON --validation VALIDATION_JSON --source-audit SOURCE_AUDIT_JSON --runner-audit RUNNER_AUDIT_JSON --require-runner-audit
```

Use `mdp --json schema runner-audit` for the host-owned native/headless runner evidence. Prefer the host-neutral local proposal runner at `scripts/mdp-proposal-runner.mjs` in source checkouts or `${PLUGIN_ROOT}/scripts/mdp-proposal-runner.mjs` in installed Pluxx bundles when proving a proposal sample run; it stages sources, builds the declared-input-only request, calls the native runner, validates, receipts, and then runs review probes. For MCP-capable hosts, use the bundled local stdio MCP wrapper at `scripts/mdp-proposal-mcp-server.mjs` or `${PLUGIN_ROOT}/scripts/mdp-proposal-mcp-server.mjs`, which exposes `mdp_proposal_tools` and file/path-only `mdp_proposal_run`. This is not a hosted or remote MCP service, and MCP transport alone does not prove audit-grade isolation. The lower-level optional BYOK native reference runner at `scripts/mdp-native-normalize-openai.mjs` or `${PLUGIN_ROOT}/scripts/mdp-native-normalize-openai.mjs` still owns the stateless API call; dry-run/mock checks require no API key, while a real model call requires the operator's secure `OPENAI_API_KEY`. Pluxx-packaged skills can route users toward the runner, but pack authoring alone does not prove the model context boundary.

Runner contract acceptance and integration support are separate. Consult [canonical runner support matrix](https://github.com/orchidautomation/message-decision-packs/blob/main/docs/headless-normalization-runners.md#canonical-runner-support-matrix) and use only `verified`, `recipe-only`, `unsupported`, or `fixture/mock-only`. Pack authoring, a documented recipe, a schema-valid audit, or MCP transport does not prove a verified integration.

7. Bind each agent-routable job to exactly one canonical `skill_id`. Use only the closed v1 pairs documented in the profile reference.
8. Add realistic pack eval fixtures for proceed, insufficient context, refusal/unsafe output, job routing, and target-isolation failure when the manifest declares a target. Decision-input examples also need synthetic expected outcomes for ready, insufficient-context, disqualified, human-review, malformed, and provider-error.
9. Validate, fix, and repeat:

```bash
mdp --json validate --dir PACK_ROOT
mdp --json gaps --dir PACK_ROOT
mdp --json eval --dir PACK_ROOT
mdp --json validate --strict --dir PACK_ROOT
mdp --json eval --strict --dir PACK_ROOT
```

Do not finish while normal validation has errors. Use strict validation as the final authoring gate unless the user explicitly accepts documented warnings.

## Boundaries

- Build decision context, not source-collection infrastructure or execution automation.
- Do not invent claims, contacts, personas, proof, certifications, compliance status, past performance, pricing, deadlines, or approvals.
- Target identity proves only what is explicitly stated in its cited direct claim. Unsupported category, capability, ICP, outcome, or proof belongs in gaps.
- Do not add old skill aliases, custom routable job IDs, obsolete surface metadata, or host visibility policy.
- Keep public fixtures synthetic or explicitly sanitized.

## Response

Report the profile, sources accepted/excluded, files changed, job bindings, commands run, validation/eval state, and remaining gaps or required human review.
