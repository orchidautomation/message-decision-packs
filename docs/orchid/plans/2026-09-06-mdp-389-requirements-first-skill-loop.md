# MDP-389 requirements-first shared skill loop

## Objective

Teach the released MDP skills one domain-neutral execution loop that establishes exact compiled requirements authority before evidence collection or generation, preserves attempt states, separates enforcement owners, and grants usability only after receipt verification and every job-declared final validator passes.

## Boundaries

- Author only under `plugin/skills/` and `plugin/skill-evals/`; generated host bundles follow the existing Pluxx build.
- Reuse released v0.1.119 CLI contracts. Do not add or speculate about CLI behavior.
- Keep collection provider-neutral and non-mutating. No outreach, CRM, enrichment-provider, proposal-submission, or HeyReach action.
- Keep semantic review with versioned model/human review; do not synthesize brittle deterministic validators.

## Implementation

1. Add one direct Apply reference defining requirements verification/recompilation, minimal evidence collection, attempt-state preservation, enforcement-owner classification, final readiness, and a bounded handoff.
2. Update Apply to load that reference for every selected job and make the loop the common precondition for GTM and proposal execution.
3. Update coordinator, builder, and review entrypoints with the cross-skill handoff boundaries needed to preserve compiled authority and avoid treating prompts as proof.
4. Add train/validation behavioral cases for requirements reuse/staleness, declared-but-unenforced constraints, semantic review, final-validator closure, and GTM/support/recruiting/proposal domain shapes.
5. Update the suite inventory and validate source skills, behavioral corpus, Pluxx-generated parity, packaging, and formatting.

## Acceptance

- Presented requirements are reused only when exact pack digest, job ID, contract version, requirements hash/receipt, and current CLI projection agree; otherwise the skill reports `compiled-now`.
- Evidence work targets only missing prerequisites and preserves found, not-found, stale, conflicting, rejected, and failed attempt states.
- Mechanical rules require schema/CLI enforcement; declared-but-unenforced blocks final readiness.
- Semantic rules route to versioned model and responsible human review.
- Usable output requires a passing explicit `verify-run` plus every ordered validator declared in the run bundle/receipt.
- Behavioral cases demonstrate the same loop across GTM, support, recruiting, and proposal terminology without adding profile-specific core logic.
