# MDP-388 final governed readiness

## Objective

Make a generative artifact final and publishable by MDP only after every deterministic post-generation validator declared by its selected job has completed successfully.

## Boundary

- Writable repository: `orchidautomation/message-decision-packs`.
- `orchidautomation/mdp-for-mdp` is read-only.
- No outreach, CRM, HeyReach, publishing, or hosted-product mutation.
- Provider execution is not required for implementation proof; fixtures remain synthetic and local.

## Compatibility

- Existing jobs that omit post-generation validators retain their current prompt-output validation behavior.
- Existing v1 run receipts remain readable. New final-validation fields are additive and default to legacy semantics when absent.
- A generated artifact is never silently rewritten. Rejected provider output remains subject to the existing private-output retention policy and is not copied into the public artifact set.

## Implementation

1. Add a profile-neutral job declaration for zero or more required deterministic post-generation validators. Each entry has a stable pack-owned identifier and a closed engine identifier; the initial engine evaluates the job's declared human-text surfaces against its exact routed policy and claim authority.
2. Validate declarations during pack health: bounded identifiers, unique IDs, supported deterministic engine, model-task ownership, and required declared text surfaces.
3. Extract a reusable validator path from the MDP-390 text-surface checker that consumes the exact generated artifact plus the already-bound routed context. Do not re-route, infer personas, scan metadata, or hard-code GTM field names.
4. In native generation, treat prompt-output validation as generation integrity, then execute every declared validator before constructing success artifacts or the final decision. Persist one content-bound result per declared validator plus a compact final-validation summary.
5. Project explicit artifact state (`generated-pending-validation`, `valid`, or `rejected`) and validation authorities into run receipts. Emit final governed output and `validation-passed` decision authority only when the complete declared set passed.
6. Extend `verify-run` and schemas so missing, failed, duplicate, reordered, or hash-mismatched required validation evidence cannot verify as final. Old receipts without the additive fields retain their historical meaning and are not upgraded into the new final-authority claim.
7. Return bounded categorical validator ID/reason/path diagnostics, never rejected prose or arbitrary provider text. Do not add automatic or unbounded retry behavior.
8. Update canonical starter manifests and documentation. Use synthetic GTM, support, recruiting, and proposal fixtures to prove the same engine over different declared text paths and routed rules.

## Red-to-green proof

- A schema-valid generated artifact with two questions reaches prompt-output success but fails its declared routed-text validator and is not final.
- A clean artifact passes the same declared validator and becomes final with separate generation and validation evidence.
- A required result that is absent, failed, duplicated, reordered, or changed makes `verify-run` fail with the same stable validator ID.
- Zero-validator legacy jobs remain compatible and do not gain a false claim that undeclared policy checks ran.
- GTM subject/body, support title/response, recruiting summary, and proposal findings fixtures use one domain-neutral contract.
- Rejected diagnostics are bounded and contain no generated prose.

## Validation

- Focused model/job schema and health tests.
- Focused routed-text validator and native runtime state-transition tests.
- Focused run receipt and `verify-run` compatibility/tamper tests.
- `cargo fmt --check` and full `cargo test`.
- `make validate-template validate-asset-sync validate-version-sync`.
- `git diff --check`.

## Risks

- Changing the meaning of success can break consumers that equate provider completion with final authority; the receipt state and compatibility rules must be explicit.
- Re-routing during validation could drift from provider constraints; runtime must consume the already-bound routed context.
- Publishing raw failed output through diagnostics would violate the existing privacy boundary; only declared categorical metadata may leave the private run area.
