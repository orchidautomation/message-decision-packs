# MDP-390: Guard Declared Artifact Text Surfaces

## Objective

Make deterministic avoid and unsupported-claim checks cover every human-facing
text surface that a job explicitly declares. The same declaration must drive
provider schema projection, local governed-output validation, and direct
`check-claims` validation without inspecting identifiers or arbitrary metadata.

## Source

- Repository: `orchidautomation/message-decision-packs`
- Stacked base ref: `codex/mdp-387-references-replay` (PR #320)
- Stacked base commit: `7a1e8f84fabf715a56961b1e83b8aa710d047d94`
- Local incorporated base commit: `8865dc6`
- Linear issue: `MDP-390`

## Owned implementation surface

- A bounded, domain-neutral job declaration of JSON Pointer artifact text
  surfaces.
- One shared compiler/walker for declared scalar and array text values.
- Provider schema constraints and local governed-output validation derived from
  the same routed avoid authority.
- Structured artifact and repeatable generic-field inputs for `check-claims`,
  with legacy `--text`, `--file`, and `--subject` mapped into declared paths.
- Exact bounded `field_path` diagnostics that never include invalid raw text.
- Manifest health checks and synchronized authored/packaged templates.
- Sanitized GTM, support, recruiting, and proposal-shaped regressions.

Primary files may include `cli/src/models.rs`, `cli/src/cli.rs`,
`cli/src/app.rs`, `cli/src/commands/routing.rs`,
`cli/src/commands/prompt_output.rs`, `cli/src/run_runtime.rs`, one new neutral
text-surface module, health/schema surfaces, and affected templates.

## Forbidden and deferred surface

- Do not scan undeclared identifiers, routing references, evidence locators, or
  metadata.
- Do not hard-code email, GTM, support, recruiting, or proposal field names in
  the universal runtime.
- Do not weaken the MDP-387 closed-reference contract or create a competing
  annotation mechanism.
- Do not implement final-validator composition (`MDP-388`).
- Do not mutate outreach, CRM, HeyReach, provider, cloud, or hosted systems.

## Acceptance

1. A regression proves an avoid value present only in a declared subject-like
   field fails with that exact `field_path`.
2. Declared scalar and array text paths are traversed; undeclared identifiers
   and metadata are ignored.
3. Legacy body-only invocations retain their current behavior, while
   `--subject` maps to a declared subject path when the selected job owns it.
4. Structured artifacts and repeatable generic fields support new declared
   text surfaces without a core field-name change.
5. Provider schema projection, local governed-output validation, and direct
   checking use the same avoid vocabulary and declared paths.
6. Unsupported-claim findings carry exact field paths and never expose the raw
   invalid value.
7. Synthetic GTM, support, recruiting, and proposal shapes pass bounded tests.
8. Existing compatibility tests and the full Rust suite remain green.

## Validation

- Focused red/green tests for declaration validation, walking, provider schema
  projection, governed-output validation, CLI adapters, and health.
- `cargo fmt --check --manifest-path cli/Cargo.toml`
- `cargo test --manifest-path cli/Cargo.toml`
- `make validate-version-sync`
- `git diff --check`

## Integration

This is a true stacked change on MDP-387 PR #320 because it consumes the
domain-neutral schema compiler/walker substrate added there. The final PR must
target `codex/mdp-387-references-replay` until #320 merges; after merge it may
be rebased onto `main` without changing the MDP-390 semantic commits. Brandon
alone merges.
