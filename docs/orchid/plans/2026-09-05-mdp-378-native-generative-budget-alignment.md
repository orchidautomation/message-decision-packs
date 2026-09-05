# MDP-378: native generative budget alignment

## Objective

Align routed-context readiness with the fixed native generative execution
boundary. A governed generation job must not report ready when its maximum
routed-context allocation plus the selected prompt already exceeds the native
aggregate input limit, and both preparation and runtime must enforce the exact
prompt-plus-declared-input byte total before any provider execution.

## Implementation

- Raise the fixed native aggregate generative input ceiling from 128 KiB to
  256 KiB everywhere the run-request schema, compiler, and runtime enforce it.
- Keep prompt plus every declared input in the aggregate calculation during
  `prepare-run` and runtime preflight. Preserve fail-closed staging and provider
  boundaries.
- Extend the bounded overflow diagnostic to expose actual bytes, configured
  limit, and bytes over without exposing content or local paths.
- Extend route-budget/readiness for model-task jobs so the declared maximum
  routed-context bytes plus the selected prompt bytes are compared with the
  native ceiling. Report the statically reserved bytes and unknown runtime
  headroom explicitly; do not claim that runtime-sized auxiliary inputs fit.
- Preserve existing normalization, privacy, retention, tool-free execution,
  endpoint allowlist, request binding, and receipt behavior.

## Acceptance criteria

1. Aggregate input totals below and exactly at 256 KiB pass; totals above it
   fail before provider invocation.
2. The 157,155-byte routed-context plus 18,343-byte prompt shape, with required
   inputs kept within the remaining headroom, passes `prepare-run` using the
   feature-branch binary.
3. Overflow output includes actual bytes, limit bytes, and bytes over.
4. Strict route-budget/readiness blocks a job when maximum routed context plus
   its prompt cannot fit, and otherwise reports reserved/unknown headroom.
5. Existing execution policy, privacy, retention, allowlist, and receipt tests
   remain green.

## Verification gates

- `cargo fmt --manifest-path cli/Cargo.toml -- --check`
- Focused compiler/runtime tests at below/equal/above 256 KiB.
- Focused route-budget/readiness tests for impossible and reserved-headroom
  jobs.
- Build `cli/target/debug/mdp` on the feature branch and exercise synthetic
  inputs matching the reported 1Password byte shape directly with that binary.
- `cargo test --manifest-path cli/Cargo.toml -- --test-threads=1`
- No provider call, outreach, release, install, deployment, or supporting-repo
  mutation is authorized by this plan. A real OpenAI-backed dogfood run remains
  a separate action-time approval boundary.

## Ownership

The implementation lane owns:

- `cli/src/run_request_compiler.rs`
- `cli/src/run_runtime.rs`
- `cli/src/routing.rs`
- `cli/src/commands/routing.rs`
- `cli/src/commands/schemas.rs`
- focused CLI fixtures/tests colocated with those modules

It must not edit release metadata, generated plugin bundles, private dogfood
artifacts, `mdp-for-mdp`, provider credentials, or unrelated lifecycle records.
