---
title: MDP-167 Portable Source-Binding Validation - Plan
type: feat
date: 2026-07-30
execution: code
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-167
linear_issues:
  - MDP-167
  - MDP-150
---

# MDP-167 Portable Source-Binding Validation - Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Let an external orchestrator bind its fields to one job's compiled Decision Input Contract and prove that the binding is complete, unique, compatible, and pinned to the exact pack and requirements release. |
| Product shape | Add a provider-neutral `mdp.source-binding.v1` JSON Schema, a deterministic `validate-source-binding` CLI command, and portable digests in `mdp.requirements.v1`. |
| Responsibility boundary | MDP owns requirements, schema, digests, and validation. Integrations own source access, provider credentials, orchestration, normalization execution, field storage, and per-record results. |
| Skill boundary | Preserve the five official skills. Extend `mdp-pack-builder`, `mdp-pack-review`, and the existing `mdp` operator route; do not add a sixth skill. |
| Compatibility | Existing packs remain valid. Jobs without Decision Input Contracts continue to report requirements unavailable; source-binding validation fails closed because there is nothing exact to bind. |
| Public safety | Use only synthetic, provider-neutral fixtures. Clay remains one adapter proof, not public schema vocabulary. |

## Product Contract

### Requirements

- R1. `mdp schema source-binding` emits the public `mdp.source-binding.v1` JSON Schema.
- R2. `mdp --json validate-source-binding --dir PACK_ROOT --job JOB_ID --file BINDING.json` performs no network or model calls.
- R3. `mdp requirements` includes a portable SHA-256 of the `.mdp` source tree and a canonical SHA-256 of the compiled requirements payload.
- R4. A binding pins the pack ID, version, content digest, requirements digest, job ID, and every selected Decision Input Contract ID/version.
- R5. Every compiled attribute appears exactly once, keyed by Decision Input Contract ID plus attribute ID. Missing, duplicate, and unknown bindings fail.
- R6. Each binding repeats the compiled requirement class and selects one allowed source class. Drift or incompatibility fails.
- R7. Multiple Decision Input Contracts per job are supported without assuming globally unique attribute IDs.
- R8. Reusing one orchestrator field key for multiple requirements is allowed.
- R9. The fixed translation is explicit: missing/null/empty/whitespace → `not_found`; false/zero → `observed`; inapplicable → `not_applicable`; inaccessible/unmapped → `blocked`; runtime failure → `error`.
- R10. Provider system names, field keys, and acquisition modes are non-empty strings rather than Clay-specific or closed provider enums.
- R11. Source-binding documents remain integration-owned artifacts outside `.mdp` packs.
- R12. Existing skill inventory and routing stay coherent: build authors requirements, review audits bindings, and `mdp` routes operators to the CLI.

### Acceptance Examples

- A complete synthetic binding pinned to the current pack and requirements release validates.
- A stale pack or requirements digest fails with a stable issue code.
- A missing, duplicate, or unknown `(contract_id, attribute_id)` fails with a stable issue code.
- A mismatched requirement class or disallowed source class fails.
- A job with multiple compatible Decision Input Contracts validates when every
  compiled attribute is present under its qualified contract/attribute key.
- Two requirements using the same external field key validate.
- A job without compiled Decision Input Contracts returns `available: false` and cannot validate a binding.
- A synthetic Clay-shaped adapter and a second non-Clay adapter both satisfy the same public contract without provider-specific schema fields.

## Technical Design

### Canonical digests

- Hash each authored regular file under `.mdp` as raw bytes, excluding generated
  local artifacts under `.mdp/briefs/` and `.mdp/traces/`.
- Sort portable POSIX relative paths and hash canonical records containing the relative path and file SHA-256.
- Reject symlinks and non-regular entries so the same source tree yields the same digest across checkout paths.
- Canonicalize the requirements JSON by recursively sorting object keys while preserving array order, then hash the payload before adding its `requirements_sha256` field.

### Binding identity

The uniqueness key is `(decision_input_contract_id, attribute_id)`. Existing
pack validation still rejects cross-contract attribute/output-path collisions;
qualification keeps the integration contract unambiguous across otherwise
compatible multi-contract jobs. A source descriptor contains:

- `field_key`
- `source_class`
- `system_of_record`
- `acquisition_mode`

The descriptor says how an integration obtains a value; it does not grant access or execute collection.

### Validation result

Return a checked JSON payload with `contract`, `status`, `valid`, `available`, exact pin receipts, diagnostics, and boundary metadata. Validation-style failures exit non-zero through the existing checked-output path.

## Implementation Units

### U1. Digests and requirements receipts

- Add reusable canonical JSON and portable `.mdp` tree digest helpers.
- Add pack content and requirements digests to ready, unavailable, and invalid requirements outputs where computable.
- Cover path independence, byte changes, and deterministic canonicalization.

### U2. Public schema and CLI validator

- Add `source-binding` to `SchemaTarget`.
- Add `ValidateSourceBinding` CLI parsing and app dispatch.
- Implement generic schema validation plus job-specific exact coverage, pin, contract-version, requirement, and source-class checks.
- Return stable diagnostics and preserve duplicate field-key support.

### U3. Fixtures and compatibility proof

- Add provider-neutral synthetic source-binding fixtures for the existing Clay example and a second generic orchestration shape.
- Add negative tests for stale pins, coverage errors, incompatible source classes, duplicate qualified keys, and multi-contract jobs.
- Confirm legacy jobs fail closed with `available: false`.

### U4. User-facing routing and docs

- Document the compile → map → validate → integrate workflow and the MDP/integration ownership boundary.
- Update `mdp`, `mdp-pack-builder`, and `mdp-pack-review` guidance without changing the canonical five-skill inventory.
- Update CLI capabilities/help and skill contract tests where required.

## Verification

1. Focused Rust tests for digests, schema, command parsing, and source-binding validation.
2. Strict validation of the starter and synthetic example packs.
3. Manual validation of both synthetic adapter fixtures.
4. Skill contract and packaging validation.
5. `make validate`.
6. Review for correctness, compatibility, data integrity, CLI contract stability, and scope containment before commit/PR.

## Non-Goals

- Provider calls, credentials, scraping, enrichment, normalization model execution, CRM writes, hosted APIs, or orchestration.
- Storing integration bindings in the MDP pack.
- A public provider taxonomy or Clay-specific schema vocabulary.
- A sixth plugin skill.
