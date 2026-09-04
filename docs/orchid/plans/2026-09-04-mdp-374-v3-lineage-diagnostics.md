# MDP-374: v3 lineage and provider-parity fix

## Objective

Make v3 normalization fail closed when model-controlled evidence references do
not belong to the collected source-attempt result set, while keeping the
provider projection and local validator aligned. Diagnostics must identify a
bounded category/path without retaining model output.

## Implementation

- Permit optional `derived_from` on rejected claims and validate every claim,
  gap, and classification reference against collected attempt IDs.
- Keep provider projection schema-compatible with OpenAI structured-output
  constraints and deduplicate set-like reference arrays before validation.
- Preserve the existing host-owned envelope boundary and content-free
  diagnostic projection.
- Add deterministic regression coverage for rejected-claim lineage.
- Keep aggregate input-byte budget failures bounded and categorical at the
  compiler boundary; no raw provider output or private evidence is retained.

## Verification gates

1. Unit and validator tests pass on this exact branch.
2. Full Rust test suite and formatting/lint checks pass.
3. The packaged candidate is verified before merge.
4. After release, run the separately authorized native OpenAI canaries against
   the exact installed artifact; do not infer live-provider success from local
   tests.

## Non-goals

This change does not make provider calls, release/install the CLI, mutate
HeyReach, or retain raw model responses.
