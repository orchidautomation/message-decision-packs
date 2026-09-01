# Job-owned prompt contracts

MDP separates two questions:

1. The product foundation answers: what exact product authority does this job need?
2. The job-owned prompt answers: what may a model do with that authority, what inputs may it use, and what exact result must it return?

The customer-selected host runs the model. MDP remains the local contract, compilation, routing, and validation layer. It does not choose a provider, make a model call, browse, enrich, send, schedule, or update a CRM.

Cold-model conformance does not change this boundary. It compiles sufficiency
before the host call, then validates the host's recorded invocation and trial
evidence. Prompt readiness or prompt-output validity alone is not
`sufficient-for-job` or `qualified-for-job-under-envelope`; the assembled
`mdp.job-conformance.v1` remains conformance authority. See
[Cold-model Conformance](cold-model-conformance.md).

## Job binding

A canonical generation or review job opts in with one explicit binding:

```yaml
jobs:
- id: outbound-copy-brief
  skill_id: mdp-pack-apply
  model_task:
    kind: generation
    prompt: generate-outbound-copy-v1
```

The referenced prompt must exist, use `format: mdp.prompt.v1`, declare a non-blank version, and have the same `kind`. An opted-in job fails closed when any of those facts drift. A legacy job with no binding remains valid but its model task is `unassessed`; it is not self-standing for model generation.

## Prompt v1

`mdp.prompt.v1` makes the model step inspectable. It declares:

- `kind`: normalization, generation, or review;
- `version`, role, objective, and ordered procedure;
- every input, its producer, default, and missing-data behavior;
- selection, ambiguity, provenance, and evidence rules;
- negative examples and a final checklist;
- a strict output contract and exact JSON Schema.

Input producers use the closed vocabulary `host`, `pack`, `runtime`, `source`, and `prior-step`. This describes responsibility; it does not authorize MDP to fetch or create the input.

## Compile and inspect

```bash
mdp --json skills --dir PACK_ROOT --job outbound-copy-brief
mdp --json requirements --dir PACK_ROOT --job outbound-copy-brief
```

`skills` shows that a model task is declared and points the operator to `requirements`. `requirements` returns the exact prompt ID, version, canonical prompt hash, declared inputs and producers, instructions, output schema, product foundation, and the host boundary. Compilation is read-only and makes no model or network call.

## Validate output

Generation and review prompts use `output_kind: governed-artifact`. Their output remains inside the existing `mdp.prompt-output.v0` envelope, but each prompt owns an exact inline schema for its `artifact`.

```bash
mdp --json validate-prompt-output \
  --dir PACK_ROOT \
  --prompt-id generate-outbound-copy-v1 \
  --invocation-receipt PROMPT_INVOCATION.json \
  --file MODEL_OUTPUT.json
```

Before the model call, the customer-selected host creates an `mdp.prompt-invocation.v1` receipt. It names the exact job, prompt ID/version/SHA-256, and every supplied non-metadata input with a SHA-256. The host supplies the exact receipt content as `prompt_receipt` and its detached byte hash separately as `invocation_receipt_sha256`; a receipt cannot contain its own hash. Those two metadata inputs do not appear inside the receipt's `inputs` array. For prompts declaring `mdp.governed-host-envelope.v1`, the model returns only semantic fields; MDP injects the prompt, job, context, receipt, and input identities from this immutable authority after generation.

The validator accepts governed artifacts only against the canonical prompt under `.mdp/prompts`; an external same-ID file cannot replace it. Every final result carries `job_id`, `prompt_version`, `prompt_sha256`, and `invocation_receipt_sha256`, so stale output or a changed host receipt fails closed. A host-envelope model response containing any owned field is rejected before wrapping; duplicate, undeclared, malformed, or missing required ready-state input receipts are also rejected. A prompt may bind to only one canonical job. Prompts without the envelope declaration retain the legacy model-echo path until explicitly migrated and versioned.

The validator also rejects malformed JSON, the wrong prompt or contract, missing fields, unexpected fields, and artifact values outside the prompt schema. Artifact identifiers must occur in the result's exact `selected_authority` subset, not merely somewhere in the job foundation; duplicate bare identifiers across selected card-qualified references are ambiguous and rejected. A `ready` result must have host receipts for every required non-metadata input, report those exact inputs plus `prompt_receipt` and `invocation_receipt_sha256` once each in `source_summary.inputs_used`, select at least one authority reference, carry no gaps, and contain substantive generation fields with no `N/A` placeholders. When a structured artifact gap includes `pack_reference`, it must be `N/A` or an exact card-qualified reference in `selected_authority`. `selected_authority`, `gaps`, and `rejected_claims` remain explicit for non-success results.

For generated prose, shape validation is not claim approval. Run the existing `check-claims` or `verify-output` path required by the job before treating the wording as governed output.

## Compatibility

- `mdp.prompt.v0` normalization and extraction prompts remain valid.
- Newly initialized GTM and proposal packs ship their normalization prompts as versioned `mdp.prompt.v1` contracts with explicit input producers and rule sections.
- Jobs without `model_task` remain valid and unassessed.
- Adding a binding opts that job into fail-closed prompt identity, version, kind, and output-contract validation.
- Prompt contracts do not change the MDP host-conformance boundary or create a second execution system.
