# Prompt Output JSON Schemas

## Decision

MDP prompt contracts should carry compact response schema references under `output_contract.schema_ref` by default. Starter generation can inline explicit response JSON Schemas under `output_contract.schema` when requested with `mdp init --include-output-schemas`.

The schema reference is the authoritative output contract for model or host execution. Inline schemas are useful for hosts that accept a literal JSON Schema object, but they make prompt files much larger, so they are opt-in. The example remains useful for model grounding and human review, but examples do not replace schemas.

## Rationale

The prior contract required strict JSON and a safe example, but "return JSON" is too loose for agents that need repeatable downstream parsing. Many model hosts can accept schema-constrained JSON directly, and hosts that cannot can still include the schema in prompt text.

This keeps MDP provider-neutral while making prompt outputs more concrete:

- Default prompt files stay readable and diffable with `schema_ref`.
- `--include-output-schemas` materializes the full schema when a host needs it.
- `contract` and `prompt_id` are fixed with `const`.
- Required top-level keys are explicit.
- Card-patch prompts narrow `card_patches.kind` to the prompt's target card kinds.
- Prospect-normalization prompts require `normalized_prospect` and `normalization_trace`.
- Context-normalization prompts require `normalized_context`, `normalization_trace`, and `review_handoff`; they avoid prospect/fit vocabulary while preserving the same strict local validation boundary.
- Normalization schemas force `card_patches` to be empty because they prepare runtime input rather than editing cards.
- Root objects reject extra keys with `additionalProperties: false`.

## Boundary

MDP still does not execute sending, scraping, enrichment, sequencing, CRM updates, or generic automation. The schema only describes local prompt output that either feeds the CLI or proposes reviewed pack entries.
