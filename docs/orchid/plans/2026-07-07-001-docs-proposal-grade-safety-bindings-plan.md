---
title: MDP-75 and MDP-76 Proposal-Grade Safety Bindings - Plan
type: docs
date: 2026-07-07
topic: proposal-grade-safety-bindings
execution: knowledge-work
linear_issues:
  - MDP-75
  - MDP-76
  - MDP-61
  - MDP-63
  - MDP-65
  - MDP-74
  - MDP-14
  - MDP-20
  - MDP-26
source_note: Public-safe design artifact using synthetic examples only.
---

# MDP-75 and MDP-76 Proposal-Grade Safety Bindings - Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Define the MDP-75 proof-carrying output binding contract first, then define how MDP-76 can later use that contract for bounded proposal rendering. |
| Product boundary | MDP remains a local decision-context and governance layer. It is not a blank-page proposal generator, hosted RFP platform, CRM, sequencer, enrichment provider, scraper, BI tool, or generic automation system. |
| Primary sequencing | MDP-75 must settle the proof-output schema and deterministic verifier before MDP-76 implements any renderer. |
| Core safety move | Material generated text must be segmented and bound to existing approved pack IDs. Missing proof becomes a gap, not confident prose. |
| Runtime stance | The verifier is deterministic. It validates JSON shape, text coverage, ID existence, route compatibility, binding sufficiency, and existing claim/output guardrails. It does not ask an AI judge whether prose is safe. |
| Stop condition | Do not implement renderer behavior from this artifact. Create narrower implementation issues after the MDP-75 contract is accepted. |

---

## Product Contract

### Summary

Current `mdp check-claims` is necessary but not enough for proposal-grade safety.
It can reject known avoid terms, unsupported categories, output-rule violations, and claim gaps after text exists, but it cannot prove that each material sentence was derived from approved pack truth.

The MDP-native answer is a proof-carrying output artifact.
An agent or future renderer may produce text, but it must also produce a complete segmentation map that binds every material span to approved MDP IDs or marks it as a gap.
The CLI then verifies the map deterministically and runs the existing claim/output checks as a second layer.

### Requirements

**Proof-carrying output contract**

- R1. The output artifact must include the generated text and a complete ordered segmentation of that text.
- R2. Every material claim-bearing segment must bind to existing approved pack references, such as claim, evidence, source-signal, requirement, output-template, or source IDs.
- R3. Connective text may remain lightly authored, but it must be bounded, classified, and checked against existing guardrails.
- R4. Gaps must stay explicit when proof, requirement context, source text, or approval is missing.

**Verifier behavior**

- R5. The verifier must reject malformed artifacts, incomplete coverage, fake IDs, stale or incompatible IDs, unbound material segments, and route-incompatible bindings.
- R6. The verifier must remain deterministic and must not require an LLM at runtime.
- R7. Existing avoid-rule, unsupported-claim, output-rule, route-context, and eval checks remain active after proof verification.
- R8. The first CLI boundary should verify proof-output artifacts, not render proposal drafts.

**Renderer dependency**

- R9. The MDP-76 renderer can only be designed as a dependent model until MDP-75 defines accepted proof-output semantics.
- R10. The renderer must use approved claim, evidence, requirement, source, and template IDs as inputs and return gaps instead of inventing missing proof.

### Scope Boundaries

In scope for MDP-75:

- a minimal proof-output schema;
- typed ID references against existing pack cards, entries, sources, prompts, jobs, and route context;
- verifier semantics and error classes;
- synthetic pass/fail fixture plan;
- a likely CLI boundary.

In scope for MDP-76 design only:

- how a future proposal renderer would select and materialize approved IDs;
- how renderer output would reuse the MDP-75 proof-output contract;
- when the renderer refuses or returns gaps.

Out of scope:

- blank-page proposal generation;
- runtime AI judging;
- hosted RFP or proposal-management platform behavior;
- external proposal submission, approval workflow, legal review, procurement review, CRM updates, or customer communication;
- durable opportunity/pursuit core schema unless MDP-26 changes after evidence;
- non-public source material in public fixtures, examples, issues, docs, or PRs;
- implementation of the renderer in this planning slice.

---

## Planning Contract

### Key Technical Decisions

- KTD1. MDP-75 should introduce a proof-output artifact, not a prose-only convention. A prose convention would still leave the CLI unable to verify IDs and coverage.
- KTD2. Use complete ordered segmentation as the first contract. Sparse annotations cannot prove that unannotated text is harmless without a semantic judge.
- KTD3. Segment references should use existing pack surfaces first: card entry IDs, source ledger IDs, prompt IDs, input contract IDs, persona labels, and job labels.
- KTD4. The canonical verifier should join segment text and compare it to `output.text`. Offsets may be emitted as diagnostics later, but ordered text segments avoid early UTF-8 and markdown-range brittleness.
- KTD5. Segment kinds control required bindings. A `claim` segment needs approved proof and evidence; a `requirement_status` segment needs a requirement binding; a `gap` segment needs a missing-proof or missing-source explanation.
- KTD6. Connective segments are allowed only for low-risk glue. They cannot carry numbers, compliance/security language, customer proof, capability claims, final authority claims, or terms caught by current guardrails.
- KTD7. The first CLI should be a dedicated proof-output verifier that reuses `check-claims` after proof validation. Overloading `check-claims` first would blur post-draft scanning with proof-map validation.
- KTD8. MDP-76 should render material proposal text from approved IDs after MDP-75, while allowing the model to choose IDs, ordering, and gap questions. The model is not trusted to validate its own IDs.
- KTD9. Do not force a durable opportunity schema. Proposal requirement and opportunity context can stay profile-owned through the current proposal template and MDP-26 decision gate.

### Relationship To Adjacent Issues

| Issue | Relationship |
|---|---|
| MDP-61 | Clarifies prompt-output provenance and profile aliases. MDP-75 should reuse the distinction between declared inputs, source/provenance references, and CLI enforcement. |
| MDP-63 | Improves literal matching false positives. MDP-75 does not replace negation-aware guardrails; verifier output still flows through claim checks. |
| MDP-65 | Ranks and caps route/brief context. MDP-75 can require route-compatible IDs without changing how human summaries are capped. |
| MDP-74 | Adds adversarial variants to deterministic guardrails. MDP-75 remains a proof layer; MDP-74 remains the phrase/category regression layer. |
| MDP-14 | Existing proposal eval fixtures prove routing, gaps, and unsupported-claim behavior. MDP-75 follow-up fixtures should extend this with proof-output pass/fail cases. |
| MDP-20 | Proposal template initialization is shipped. MDP-75 should use that synthetic template for fixtures rather than adding non-public examples. |
| MDP-26 | Opportunity/pursuit schema remains future and evidence-gated. MDP-75 and MDP-76 should not depend on a new core opportunity object. |

---

## Proof-Output Schema

### Contract Shape

The first proof-carrying output contract should be named `mdp.proof-output.v0`.
It is an artifact produced by an agent, prompt, or future renderer and consumed by the deterministic CLI.

```json
{
  "contract": "mdp.proof-output.v0",
  "pack": {
    "id": "proposal-mdp-sample",
    "profile_id": "proposal",
    "pack_hash": "optional-pack-hash"
  },
  "route": {
    "persona": "Proposal Lead",
    "job": "compliance review"
  },
  "output": {
    "kind": "proposal-review-section",
    "format": "markdown",
    "text": "Requirement status: Gap. The sample pack has no approved certification proof."
  },
  "coverage": {
    "mode": "full-segmentation",
    "material_policy": "bound-or-gap"
  },
  "segments": [
    {
      "id": "seg-001",
      "kind": "template_text",
      "text": "Requirement status: ",
      "refs": [
        {
          "type": "card_entry",
          "role": "renders",
          "card_id": "review-outputs",
          "entry_id": "compliance-matrix",
          "kind": "copy-patterns",
          "primitive": "output-contracts"
        }
      ]
    },
    {
      "id": "seg-002",
      "kind": "gap",
      "text": "Gap. The sample pack has no approved certification proof.",
      "gap": {
        "code": "missing-approved-proof",
        "reason": "Certification proof is not present in the synthetic proof library."
      },
      "refs": [
        {
          "type": "card_entry",
          "role": "constrains",
          "card_id": "proof-library",
          "entry_id": "unsupported-certifications",
          "kind": "claims",
          "primitive": "boundaries"
        },
        {
          "type": "source",
          "role": "supports-gap",
          "source_id": "synthetic-proof-inventory"
        }
      ]
    }
  ]
}
```

### Segment Kinds

| Segment kind | Meaning | Required bindings |
|---|---|---|
| `claim` | Material product, proof, outcome, capability, compliance, security, delivery, or customer-proof statement. | At least one approved `card_entry` with `kind: claims` or primitive `evidence-proof`, plus source/evidence backing when the entry requires it. |
| `requirement_status` | Proposal requirement, compliance, response obligation, or evaluation-factor status. | A requirement or source-signal `card_entry`, a status value from the relevant output contract, and any proof/gap binding needed by that status. |
| `template_text` | Output structure or boilerplate controlled by an output contract. | An output-contract reference, usually `review-outputs` or `proposal-output-rules`. |
| `gap` | Missing proof, missing source text, missing requirement clarity, weak evidence, or reviewer decision needed. | A gap code, reason, and at least one constraining reference or source showing why the gap exists. |
| `connective` | Low-risk grammar, heading glue, transitions, or punctuation. | No proof binding required, but verifier applies stricter text limits and existing guardrails. |

### Typed References

Use typed references instead of one overloaded ID string.
The verifier should reject unknown fields and unknown reference types in strict mode.

| Ref type | Required fields | Resolves against |
|---|---|---|
| `card_entry` | `card_id`, `entry_id`, optional `kind`, optional `primitive`, `role` | Manifest card refs plus `.mdp/cards/*.yaml` entries. |
| `source` | `source_id`, `role` | `.mdp/sources.yaml`. |
| `prompt_input` | `prompt_id`, `input_name`, `role` | `.mdp/prompts/*.yaml` declared inputs. |
| `input_contract` | `input_contract_id`, `role` | Manifest `input_contracts`. |
| `route` | `persona`, `job`, `role` | Manifest personas/jobs and route context. |

The `role` is small and declarative:

- `supports` for proof-bearing support;
- `constrains` for avoid, boundary, or status-limiting rules;
- `renders` for template/output-shape text;
- `requires` for requirement or source-signal obligations;
- `supports-gap` for proof that something is missing or unsupported.

---

## Verifier Semantics

The verifier should fail closed and return a machine-readable decision.
Recommended decisions are `proof-safe`, `needs-revision`, and `blocked`.

### Verification Passes

1. Parse the artifact as raw JSON and require `contract: mdp.proof-output.v0`.
2. Load the pack manifest, cards, sources, prompts, input contracts, and route context.
3. Verify `pack.id` and `profile_id` match the loaded pack. If `pack_hash` is present and stale, emit a stale-pack failure.
4. Join `segments[].text` in order and require exact equality with `output.text`. Whitespace outside segments is not allowed except if represented by a segment.
5. Require every segment to have a stable unique `id`, known `kind`, non-empty `text`, and valid kind-specific fields.
6. Resolve every typed reference. Unknown card IDs, entry IDs, source IDs, prompt IDs, input names, personas, or jobs fail deterministically.
7. Check compatibility. A ref that claims `kind: claims` must resolve to a claims card; a ref that claims `primitive: evidence-proof` must be covered by `primitive_map`; a segment bound to a route should be loadable or compatible with that route.
8. Check binding sufficiency by segment kind. Material `claim` and `requirement_status` segments cannot pass with only template or connective refs.
9. Apply connective constraints. Connective text should be short, non-material, and free of current avoid terms, unsupported-claim categories, numbers tied to outcomes, compliance/security language, customer-proof language, and final authority claims.
10. Run existing `check-claims` and route-scoped output constraints against the full `output.text`.
11. Return structured issues with codes, paths, refs, and severity.

### Failure Modes

| Code | Failure | Expected verifier response |
|---|---|---|
| `proof_output_malformed` | JSON is invalid or the root shape is wrong. | `blocked` |
| `proof_output_contract_unknown` | `contract` is missing or not `mdp.proof-output.v0`. | `blocked` |
| `proof_output_text_mismatch` | Joined segment text does not exactly equal `output.text`. | `blocked` |
| `proof_output_unclassified_text` | Text exists outside segments. | `blocked` |
| `proof_output_unknown_segment_kind` | Segment kind is not recognized. | `blocked` |
| `proof_output_unknown_ref` | Referenced card, entry, source, prompt, input, persona, or job does not exist. | `blocked` |
| `proof_output_stale_pack` | Optional pack hash does not match the loaded pack. | `needs-revision` or `blocked` under strict mode |
| `proof_output_incompatible_ref` | Claimed kind, primitive, persona, job, or route compatibility does not match the resolved ref. | `blocked` |
| `proof_output_insufficient_binding` | A material segment lacks required proof, evidence, requirement, or source refs. | `blocked` |
| `proof_output_fake_id` | The model invented a plausible but unresolved ID. | `blocked` |
| `proof_output_gap_smoothed` | Segment states missing proof as if it were supported. | `blocked` |
| `proof_output_connective_too_risky` | Connective text contains material claim language or guardrail terms. | `needs-revision` |
| `claim_check_guardrail_hit` | Existing avoid/output guardrail fired on full text. | `needs-revision` |
| `claim_check_unsupported_claim` | Existing unsupported-claim category fired on full text. | `needs-revision` |

### Hallucinated ID Handling

A hallucinated ID is not a warning.
It is a verifier failure because the proof map is only useful when references resolve to pack-owned truth.

Examples:

- `card_id: "approved-certifications"` fails when no manifest card has that ID.
- `entry_id: "cmmc-level-2-approved"` fails when the `proof-library` card has no such entry.
- `source_id: "city-rfp"` fails when `.mdp/sources.yaml` does not declare that source.
- `template_id: "executive-proposal"` fails unless represented as a real output-contract card entry or future template registry entry.

The failure should include the JSON pointer path and the unresolved ID so an agent can repair the artifact without guessing.

---

## Likely CLI Boundary

MDP-75 should prefer a new verifier command named `verify-output`.
The likely invocation shape is:

```bash
mdp --json verify-output --dir ./proposal-pack --file ./proof-output.json
```

Optional flags for the first implementation issue:

- `--persona` and `--job` to override or require route context;
- `--strict` to treat stale pack hashes and advisory route incompatibilities as failures;
- `--summary` to emit counts and the final decision without full issue detail.

The command should internally reuse the same guardrail checks as `check-claims` after proof-output validation.
`check-claims` remains the lower-level post-draft scanner for plain text.
Later, a convenience alias could route proof-output artifacts through `check-claims`, but the first implementation should keep the contracts separate.

Future eval fixtures should add cases for:

- valid proof-output with claim, requirement, template, source, and gap refs;
- fake card ID;
- fake source ID;
- stale pack hash;
- material claim marked as connective;
- requirement status with no requirement ref;
- unsupported certification gap that passes because it is represented as a gap;
- unsupported certification claim that fails because it is represented as a claim.

---

## MDP-76 Renderer Model

MDP-76 should depend on MDP-75 by making the renderer emit `mdp.proof-output.v0`, not raw prose.
The renderer should be an assembly model over approved IDs.

### Renderer Input

A bounded renderer request should contain:

- pack and profile identity;
- persona and job;
- section or output-template ID;
- selected requirement IDs;
- selected claim/proof/evidence IDs;
- desired output kind, such as `compliance-matrix-row`, `win-theme-proof-note`, or `executive-risk-summary`;
- known gaps to preserve.

The LLM may help select candidate IDs and order them, but it is not trusted to validate those IDs.
The CLI or renderer must validate selections before producing material text.

### Renderer Behavior

The renderer can:

- materialize approved template text from `review-outputs` or future output-template entries;
- include approved claim/proof language from claims or evidence-proof entries;
- attach source and requirement refs to every material segment;
- insert explicit gap segments when proof or source context is missing;
- allow bounded connective text for headings and transitions.

The renderer must not:

- invent certification, compliance, security, customer proof, past performance, pricing, or approval claims;
- draft final proposal prose from a blank page;
- smooth a gap into a supported statement;
- treat a model-selected ID as valid until the verifier resolves it;
- require a hosted proposal workspace or durable opportunity schema.

### Synthetic Example

Synthetic request:

```json
{
  "persona": "Proposal Lead",
  "job": "compliance review",
  "template_ref": {
    "card_id": "review-outputs",
    "entry_id": "compliance-matrix"
  },
  "requirement_refs": [
    {
      "card_id": "requirements-matrix",
      "entry_id": "must-answer-sections"
    }
  ],
  "proof_refs": [
    {
      "card_id": "proof-library",
      "entry_id": "approved-synthetic-proof"
    }
  ],
  "known_gaps": [
    {
      "code": "missing-certification-proof",
      "ref": {
        "card_id": "proof-library",
        "entry_id": "unsupported-certifications"
      }
    }
  ]
}
```

Possible rendered text:

```text
Requirement status: Gap. The synthetic sample can discuss phased rollout planning and training readiness, but it has no approved certification proof.
```

The first sentence can be template-bound.
The phased rollout and training readiness language must bind to `proof-library:approved-synthetic-proof`.
The certification sentence must bind to `proof-library:unsupported-certifications` as a gap, not as a supported claim.

If the user asks for "state that we are CMMC compliant," the renderer should return a gap/refusal artifact instead of proposal text:

```text
Gap: the pack has no approved certification proof for CMMC compliance.
```

---

## Follow-Up Implementation Shape

After MDP-75 is accepted, create a narrower implementation issue with this likely sequence:

1. Add the `mdp.proof-output.v0` schema model and parser.
2. Add `mdp verify-output` with deterministic coverage, reference, compatibility, and binding checks.
3. Reuse current `check-claims` and route-scoped output checks after proof validation.
4. Add synthetic proposal fixtures under the proposal template eval suite.
5. Update relevant docs and proposal skills so agents request proof-output artifacts and fail closed on missing or fake IDs.

After that lands and proves useful, MDP-76 can become a separate implementation plan for the renderer.
That later plan should still start with synthetic fixtures and should not add a hosted proposal editor, external workflow, or durable opportunity schema.

---

## Validation

This artifact is docs-only.
It does not change CLI behavior, plugin behavior, skill instructions, templates, or runtime assets.

Validation for this change:

- inspect the artifact for public-safety and scope boundaries;
- run `git diff --check`;
- skip `make validate` unless a later change touches templates, skills, or CLI behavior.

## Sources

- Linear MDP-75 and MDP-76 issue descriptions and comments.
- Adjacent Linear issues MDP-61, MDP-63, MDP-65, MDP-74, MDP-14, MDP-20, and MDP-26.
- `AGENTS.md`.
- `docs/what-this-repo-is.md`.
- `docs/prompt-extraction-contract.md`.
- `docs/plans/2026-07-01-001-docs-domain-profile-foundation-plan.md`.
- `docs/plans/2026-07-01-006-docs-proposal-reference-profile-template-plan.md`.
- `docs/plans/2026-07-01-007-docs-proposal-privacy-public-repo-guardrails-plan.md`.
- `docs/orchid/decisions/2026-06-26-prompt-output-json-schemas.md`.
- `docs/orchid/decisions/2026-06-26-runtime-normalization-prompts.md`.
- `plugin/assets/templates/proposal/README.md`.
- `plugin/assets/templates/proposal/.mdp/manifest.yaml`.
- `plugin/assets/templates/proposal/.mdp/cards/proof-library.yaml`.
- `plugin/assets/templates/proposal/.mdp/cards/requirements-matrix.yaml`.
- `plugin/assets/templates/proposal/.mdp/cards/review-outputs.yaml`.
- `plugin/assets/templates/proposal/.mdp/cards/proposal-output-rules.yaml`.
- `plugin/skills/mdp-proposal-pack-builder/references/proposal-boundaries.md`.
- `plugin/skills/mdp-proposal-pack-builder/references/proposal-opportunity-context.md`.
- `plugin/skills/mdp-lfg/references/prompt-output-validation.md`.
