# MDP-374: runtime aggregate input-budget diagnostic

## Objective

Close the remaining deterministic byte-budget gap from the merged MDP-374
implementation. Preparation and runtime must account for the selected prompt
and every declared input as one bounded aggregate before a native provider call.
When that aggregate exceeds the execution policy, return the existing safe
`input-too-large` reason with observed and limit counts instead of the generic
`declared-input-refused` or `binding unavailable` fallback.

## Implementation

- Include the selected prompt bytes in `mdp prepare-run`'s aggregate budget.
- Add one host-generated runtime policy diagnostic for aggregate prompt plus
  declared-input overflow, with no input contents or filesystem paths, and
  admit the new bounded code in the canonical schema.
- Preserve the existing staging, source-integrity, request-hash, and provider
  boundaries; a budget refusal must happen before native provider execution.
- Keep the existing scalar `input-too-large` code stable across compiler and
  receipt-free runtime results.

## Acceptance criteria

1. A prompt plus declared inputs whose aggregate exceeds the policy returns
   `input-too-large` with bounded observed and limit counts.
2. An in-budget aggregate keeps the current staging and request behavior.
3. The receipt-free runtime authority block remains valid against the closed
   canonical schema and contains no input content or local path.
4. Compiler and runtime tests cover the overflow boundary and preserve the
   existing per-file safety checks.

## Verification gates

- `cargo fmt --manifest-path cli/Cargo.toml -- --check`
- Focused compiler/runtime tests for the aggregate budget refusal.
- `cargo test --manifest-path cli/Cargo.toml`
- No provider call, release, install, deployment, private canary, or external
  system mutation is part of this plan.

## Ownership

- `cli/src/run_request_compiler.rs`: aggregate preparation accounting and tests.
- `cli/src/run_runtime.rs`: receipt-free runtime diagnostic and tests.
- `cli/src/commands/schemas.rs`: closed policy-diagnostic vocabulary.
- `cli/src/commands/run.rs`: receipt-free failure-envelope assertion.

The worker must not edit release metadata, generated bundles, private fixtures,
or unrelated lifecycle records.
