# MDP-387: Close Generated References Over Routed Authority

## Objective

Make every model-selected reference resolve against an explicit, domain-neutral
vocabulary derived from the exact declared and routed authority. Provider schema
projection and local validation must compile the same vocabulary and fail closed
with bounded diagnostics.

## Source

- Repository: `orchidautomation/message-decision-packs`
- Source ref: `origin/main`
- Source commit: `6c9d8c7cd4e256cab8f54c5b0670530028b96159`
- Linear issue: `MDP-387`

## Owned implementation surface

- A neutral output-schema reference annotation and compiler/walker.
- Provider schema projection for annotated scalar, array, and nested references.
- Local governed-output validation using the same compiled vocabularies.
- Annotation health/schema validation and affected canonical prompt templates.
- Sanitized GTM, support, recruiting, and proposal-shaped regression fixtures.

Primary files may include `cli/src/run_runtime.rs`,
`cli/src/commands/prompt_output.rs`, one new neutral compiler module,
`cli/src/models.rs`, `cli/src/commands/schemas.rs`,
`cli/src/commands/health.rs`, and affected authored templates.

## Forbidden and deferred surface

- Do not implement final-job prerequisite readiness (`MDP-386`).
- Do not implement general human-facing text guardrail scanning (`MDP-390`).
- Do not compose final-validator authority (`MDP-388`).
- Do not add GTM-specific field names or retain raw invalid model output.
- Do not modify outreach, CRM, HeyReach, cloud, or hosted-product behavior.

## Acceptance

1. A deterministic regression proves an invented accepted/selected claim or
   evidence reference currently escapes at least one provider/local boundary.
2. Explicit annotations close authority, entry, evidence, claim, angle, CTA,
   policy, template, nested-array, and domain-extension references without a
   field-name switch table.
3. Provider schema and local validation use one compiler and accept/reject the
   same exact vocabulary.
4. Unknown references fail closed with a bounded JSON path and categorical
   expected/observed information, never raw model content.
5. Vocabulary expansion is bounded; exceeding the bound blocks rather than
   silently omitting constraints.
6. Synthetic GTM, support, recruiting, and proposal fixtures pass.
7. Existing compatibility tests and the full Rust test suite remain green.

## Validation

- Focused red/green tests in runtime, prompt-output, schemas, and health modules.
- `cargo fmt --check`
- `cargo test`
- `make validate-version-sync`
- `git diff --check`

## Integration

This lane owns the shared output-field reference semantics. `MDP-390` is queued
behind this contract and must consume it rather than adding a competing
annotation mechanism. The root orchestrator owns rebases, review, PR creation,
and all lifecycle mutations. Brandon alone merges.
