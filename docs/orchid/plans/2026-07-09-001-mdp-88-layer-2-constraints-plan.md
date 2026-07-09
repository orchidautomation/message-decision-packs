---
title: MDP-88 Layer 2 Constraints - Implementation Plan
type: docs
date: 2026-07-09
topic: mdp-layer-2-constraints
execution: implementation-ready
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
linear_issues:
  - MDP-88
related_issues:
  - MDP-75
  - MDP-77
  - MDP-78
source_note: Public-safe implementation plan using synthetic proposal fixtures only.
---

# MDP-88 Layer 2 Constraints - Implementation Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Add the first safe slice of declarative Layer 2 constraints so MDP packs can deterministically gate structured proposal proof-output artifacts. |
| Product boundary | MDP remains a local/offline decision-context and validation-contract layer. It is not a sender, sequencer, CRM, scraper, enrichment provider, BI tool, proposal-management platform, legal reviewer, compliance certifier, or generic automation system. |
| First slice | Implement proof-output-specific constraints for segment coverage, source-ref requirements, and connective limits. Do not implement a general arbitrary prose rule engine. |
| Constraint ownership | Constraints are pack-owned card entry fields. Model-produced `mdp.proof-output.v0` artifacts do not get to declare the policy used to validate themselves. |
| Primary command | `mdp verify-output` owns proof-output constraints. `mdp check-claims` continues to own unstructured generated-text checks. |
| Stop condition | A Blocks agent should be able to implement this slice, open a PR, run validation, and update `MDP-88` without needing new product decisions. |

---

## Product Contract

MDP already has Layer 1 guidance, which agents read, and Layer 2 constraints, which the CLI can deterministically validate.
The MDP-88 first slice should expand Layer 2 for structured proposal proof-output artifacts without making arbitrary prose executable.

Requirements:

- Packs can declare proof-output constraints on card entries using the existing `constraints` field.
- `mdp verify-output` reads pack-owned constraints selected by the artifact route and loaded pack context.
- The verifier fails deterministically when a proof-output artifact omits required segment coverage, claim source refs, or connective limits.
- Existing `check-claims` behavior remains backward-compatible for unstructured text.
- Unknown constraint keys become explicit validation warnings rather than silent no-ops.
- Proposal examples and eval fixtures stay synthetic and public-safe.
- Agent-facing skills describe which rules are enforceable and which remain review guidance.

Non-goals:

- Arbitrary natural-language rule enforcement.
- Hosted policy service or external policy registry.
- Regex-heavy compliance review that pretends to understand proposal semantics.
- Proposal submission or approval workflow behavior.
- Broad GTM outbound constraint expansion in this slice.

---

## Current Behavior Verified

| Surface | Current fact | Planning implication |
|---|---|---|
| `cli/src/models.rs` | `Entry` has sibling `exact_paragraphs` and nested `EntryConstraints` fields for word count, subject words, subject avoid literals, max questions, and forbidden links/attachments/images/html/tracking. | Add proof-output constraints under `EntryConstraints` rather than inventing a second card field. Preserve `exact_paragraphs` for compatibility. |
| `cli/src/commands/routing.rs` | `check-claims` applies global output-rule entries, counts paragraphs, gathers `constraints.*` hits, and can also apply route-scoped non-output-rule constraints when `--persona` and `--job` are passed. | Keep unstructured text checks in `check-claims`; do not move them into `verify-output`. |
| `cli/src/commands/schemas.rs` | `constraints_schema()` currently enumerates only draft-text constraints. The proof-output JSON schema has no pack-owned constraint concept because artifacts should not own validation policy. | Extend card-entry `constraints` schema, not `mdp.proof-output.v0` artifact schema. |
| `cli/src/commands/proof_output.rs` | `ProofOutputArtifact`, `ProofSegment`, and `ProofRef` use `serde(deny_unknown_fields)`. Verification already checks pack/profile/hash, route compatibility, full segmentation text equality, fake IDs, binding sufficiency, gap smoothing, connective risk, and embedded `check-claims`. | Add a verifier pass after existing segment/ref validation to apply pack-owned proof-output constraints. |
| `cli/src/commands/evals.rs` | Eval fixtures already support `command: verify-output`, inline `proof_output`, `proof_output_file`, `expect_valid`, and `expect_issue_codes_contains`. | Add focused proposal fixtures rather than building a new eval runner. |
| `plugin/assets/templates/proposal/.mdp/cards/*` | Proposal template already has `proposal-output-rules`, `review-outputs`, `requirements-matrix`, `proof-library`, and review route cards. | Put first-slice constraints on `proposal-output-rules` or route-selected `review-outputs` entries. |
| `plugin/assets/templates/proposal/.mdp/evals/*` | Proposal template already has proof-output valid/fail fixtures. | Extend this fixture set with required segment/source-ref/connective constraint failures. |
| `plugin/skills/*` | Proposal skills already instruct agents to use `mdp verify-output` before trusting model-selected IDs. `mdp-output-rules` documents current deterministic output constraints. | Update skills so Blocks and future agents distinguish proof-output constraints from plain draft-text constraints. |

---

## Technical Design

### Constraint Taxonomy

Use three classes of Layer 2 constraints.

| Class | Command owner | Examples | MDP-88 first slice |
|---|---|---|---|
| Draft-text constraints | `check-claims` | word count, subject words, max questions, links/html/tracking, exact paragraphs | Preserve only. Do not expand in this implementation. |
| Proof-output constraints | `verify-output` | required segment kinds, source refs for claim segments, connective word limits | Implement now. |
| Pack/prompt/input constraints | `validate`, `validate-prompt-output`, `fit`, `eval` | pack-owned enums, required fields, route/eval coverage, prompt-output schemas | Preserve only. Use evals to prove new verifier behavior. |

### Schema Shape

Extend `EntryConstraints` with an optional nested proof-output constraint block:

```yaml
constraints:
  proof_output:
    required_segment_kinds:
      - requirement_status
      - gap
    min_segments:
      requirement_status: 1
      template_text: 1
    require_source_refs_for_claims: true
    max_connective_words: 18
```

Field semantics:

| Field | Type | Applies to | Failure code |
|---|---|---|---|
| `required_segment_kinds` | array of proof-output segment kind strings | whole artifact | `proof_output_required_segment_missing` |
| `min_segments` | map of segment kind to minimum count | whole artifact | `proof_output_segment_count_violation` |
| `require_source_refs_for_claims` | boolean | each `claim` segment | `proof_output_claim_source_ref_missing` |
| `max_connective_words` | integer | each `connective` or `formatting` segment | `proof_output_connective_too_long` |

Use existing segment kind names:

```text
claim
requirement_status
template_text
gap
connective
formatting
```

Do not implement `required_sections` as markdown heading matching in this slice.
For proof-output artifacts, segment kinds are the machine-readable section source of truth.
Readable proposal review headings are a rendering layer, not the validation source.

### Route And Constraint Selection

`mdp verify-output` should collect proof-output constraints from pack cards, not from the artifact.

Recommended selection policy:

1. Load the existing pack inventory.
2. Validate `artifact.route` as it does today.
3. If a route is present, collect `constraints.proof_output` from route-selected card entries.
4. Always collect proof-output constraints from output-rule cards resolved by `CardKind::OutputRules`, because those are global generated-output rules.
5. De-duplicate by `(card_id, entry_id, constraint_path)` so a renamed profile-owned output-rules card does not double-apply when route-selected.
6. If a route is absent, apply only global output-rule proof-output constraints and skip route-selected review-output constraints.

This mirrors the existing `check-claims` split between global output rules and route-scoped entries.

### Unknown Constraint Keys

Current behavior can silently ignore unknown `constraints` fields after deserialization.
That is unsafe for Layer 2 because a pack author may believe a rule is enforced when it is not.

Implement raw validation for known constraint keys:

- top-level known keys: `word_count`, `subject_words`, `subject_avoid`, `max_questions`, `forbid_links`, `forbid_attachments`, `forbid_images`, `forbid_html`, `forbid_tracking`, `proof_output`;
- nested `proof_output` known keys: `required_segment_kinds`, `min_segments`, `require_source_refs_for_claims`, `max_connective_words`.

Default policy:

- `mdp validate --dir .`: warn with code `unsupported_constraint_field`.
- `mdp validate --strict --dir .`: fail through existing strict warning behavior.
- `mdp verify-output`: do not fail because of unrelated unknown constraints if `mdp validate` already surfaces them, but never apply unknown keys.

If implementation discovers that adding this warning breaks existing fixtures, add explicit fixture updates instead of reverting to silent ignore.

---

## Implementation Units

### Unit 1: Model And Schema Contract

Files:

- `cli/src/models.rs`
- `cli/src/commands/schemas.rs`
- `cli/src/starter.rs`
- `cli/src/commands/prompt_output.rs` if prompt-output card patch validation has an explicit supported entry field list that needs the nested constraint shape.

Tasks:

1. Add `ProofOutputConstraints` to `cli/src/models.rs`.
2. Add `proof_output: ProofOutputConstraints` to `EntryConstraints` with default/skip serialization.
3. Update `EntryConstraints::is_empty()`.
4. Keep `exact_paragraphs` as a sibling field for backward compatibility.
5. Extend `constraints_schema()` with `proof_output`.
6. Extend starter/schema output helpers so generated docs and prompt extraction contracts mention proof-output constraints accurately.

Recommended Rust model shape:

```rust
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProofOutputConstraints {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) required_segment_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) min_segments: BTreeMap<String, usize>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) require_source_refs_for_claims: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_connective_words: Option<usize>,
}
```

Acceptance checks:

- `mdp --json schema card` exposes the nested proof-output constraint shape.
- Existing basic and proposal templates still validate.
- Existing `exact_paragraphs` tests still pass unchanged.

### Unit 2: Pack Validation For Unknown Constraint Fields

Files:

- `cli/src/commands/health.rs`
- `cli/src/commands/schemas.rs`
- tests in `cli/src/commands/health.rs` or the existing schema/health test module.

Tasks:

1. Add raw YAML/JSON card-entry constraint key inspection during pack validation.
2. Emit `unsupported_constraint_field` with severity `warning` for unsupported top-level or nested proof-output keys.
3. Preserve current non-strict warning behavior; strict mode should fail through existing strict warning promotion.
4. Add tests for:
   - unknown top-level constraint key;
   - unknown nested `proof_output` constraint key;
   - valid nested `proof_output` constraints.

Do not add `serde(deny_unknown_fields)` to `EntryConstraints` as the first move.
That would turn a recoverable validation warning into a pack parse failure and may make repair harder for users.

### Unit 3: Verifier Constraint Collection

Files:

- `cli/src/commands/proof_output.rs`

Tasks:

1. Add a helper such as `collect_proof_output_constraints(inventory, route) -> Vec<ProofOutputConstraintRule>`.
2. For route-present artifacts, include route-selected entries with non-empty `constraints.proof_output`.
3. Always include output-rule cards by `CardKind::OutputRules` when their entries define proof-output constraints.
4. Preserve profile-owned card ID behavior by using card kind where canonical IDs differ.
5. De-duplicate collected rules.

Suggested internal rule metadata:

```text
card_id
entry_id
title
route_scoped: bool
constraints: ProofOutputConstraints
```

This metadata should appear in issue messages where practical so Blocks can repair the right pack entry.

### Unit 4: Verifier Constraint Evaluation

Files:

- `cli/src/commands/proof_output.rs`

Tasks:

1. Count segments by `SegmentKind`.
2. Apply `required_segment_kinds`.
3. Apply `min_segments`.
4. Apply `require_source_refs_for_claims` to each `claim` segment. A claim segment passes only when `bindings.source_refs` is non-empty after ref resolution.
5. Apply `max_connective_words` to `connective` and `formatting` segments.
6. Add issues using deterministic codes:
   - `proof_output_required_segment_missing`
   - `proof_output_segment_count_violation`
   - `proof_output_claim_source_ref_missing`
   - `proof_output_connective_too_long`
7. Keep existing issue codes and decisions stable for current fixtures.

Decision policy:

- Required segment and claim-source failures are `error` and should make the verifier `blocked`.
- Connective word limit failures are `error` when the rule is declared. They should make the verifier `needs-revision` or `blocked` according to existing `decision_for` behavior; do not create a new decision value.

### Unit 5: Proposal Template Constraints And Fixtures

Files:

- `plugin/assets/templates/proposal/.mdp/cards/proposal-output-rules.yaml`
- `plugin/assets/templates/proposal/.mdp/cards/review-outputs.yaml`
- `plugin/assets/templates/proposal/examples/proof-output/*.json`
- `plugin/assets/templates/proposal/.mdp/evals/*.yaml`
- `cli/src/starter.rs` if starter-generated proposal template content mirrors these files.

Tasks:

1. Add a global proposal-output rule entry for proof-output review packets.
2. Add route/output-specific constraints on review-output entries where appropriate.
3. Keep fixture text synthetic and generic.
4. Add or retarget proof-output fixtures:
   - valid artifact satisfying constraints;
   - missing required `requirement_status` or `gap` segment;
   - claim segment lacking source ref when required;
   - connective/formatting segment exceeding `max_connective_words`.
5. Add eval fixtures with `command: verify-output`, `expect_valid: false`, and `expect_issue_codes_contains` for each new failure code.

Example card-entry shape:

```yaml
- id: compliance-review-proof-output
  title: Compliance review proof-output shape
  body: Compliance review proof-output must include requirement status and explicit gaps when proof is missing.
  applies_to:
    - Proposal Lead
    - Solution Owner
  evidence: []
  avoid: []
  constraints:
    proof_output:
      required_segment_kinds:
        - requirement_status
        - gap
      min_segments:
        requirement_status: 1
      require_source_refs_for_claims: true
      max_connective_words: 18
```

### Unit 6: Docs And Agent-Facing Skills

Files:

- `cli/USAGE.md`
- `docs/conceptual-decision-flow.md`
- `docs/what-this-repo-is.md`
- `docs/prompt-extraction-contract.md`
- `plugin/skills/mdp/SKILL.md`
- `plugin/skills/mdp-output-rules/SKILL.md`
- `plugin/skills/mdp-copy-eval/SKILL.md`
- `plugin/skills/mdp-pack-eval/SKILL.md`
- proposal skills that mention `verify-output`:
  - `plugin/skills/mdp-proposal-pack-builder/SKILL.md`
  - `plugin/skills/mdp-proposal-compliance-review/SKILL.md`
  - `plugin/skills/mdp-proposal-bid-no-bid-review/SKILL.md`
  - `plugin/skills/mdp-proposal-red-team-gap-review/SKILL.md`
  - `plugin/skills/mdp-proposal-win-theme-proof-review/SKILL.md`

Tasks:

1. Explain the Layer 1 vs Layer 2 distinction.
2. Document that proof-output constraints are pack-owned and enforced by `verify-output`.
3. State that arbitrary prose remains guidance unless mapped to a supported structured constraint.
4. Update examples to use synthetic proposal proof-output only.
5. Ensure skills route proof-output shape/rule work to `mdp-output-rules` or proposal-specific review skills as appropriate.

---

## Test Plan

### Rust Unit Tests

Run:

```bash
cargo test --manifest-path cli/Cargo.toml
```

Add targeted tests for:

- parsing `constraints.proof_output`;
- `EntryConstraints::is_empty()` with proof-output fields;
- schema includes proof-output constraints;
- validation warns for unknown top-level constraints;
- validation warns for unknown nested proof-output constraints;
- `verify-output` fails when a required segment kind is missing;
- `verify-output` fails when `min_segments` is not met;
- `verify-output` fails when a claim lacks a source ref and the pack rule requires source refs;
- `verify-output` fails when connective text exceeds the declared word limit;
- existing proof-output valid-binding fixture still passes after constraints are added or is updated intentionally.

### Template Validation

Run:

```bash
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/proposal
cargo run --manifest-path cli/Cargo.toml -- --json eval --strict --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json eval --strict --dir plugin/assets/templates/proposal
```

### Full Repo Validation

Run:

```bash
make validate
```

If local plugin validators are unavailable, state the skipped validator clearly and still run CLI/template checks.

---

## Blocks Handoff

### Suggested Linear State

After this plan is linked, `MDP-88` can move from `phase:planned` to `phase:ready-for-agent`.

Recommended delegate route:

```text
Orchid Agent Stack -> linear-lifecycle -> ce-work
```

### Branch And PR

Suggested branch:

```text
blocks/mdp-88-layer-2-proof-output-constraints
```

PR title:

```text
MDP-88: Add proof-output Layer 2 constraints
```

Because `message-decision-packs` is an enrolled Orchid repo, add GitHub label:

```text
ai:autofix-enabled
```

unless Brandon explicitly opts out or the PR cannot be safely repaired on the same branch.

### Execution Notes For Blocks

- Start from current `origin/main`.
- Read `AGENTS.md`.
- Do not commit private data or raw proposal documents.
- Keep fixtures synthetic and public-safe.
- Keep scope to proof-output constraints only; defer GTM sentence/bullet/CTA count expansion.
- Update CLI, template, docs, and skills in the same PR so the contract does not drift.
- If a schema naming choice feels ambiguous, prefer the smallest backward-compatible name and document it rather than widening into a generic rules engine.

### Done Means

- `MDP-88` has an implementation PR.
- The PR includes CLI/schema/template/docs/skill updates for the proof-output first slice.
- The proposal template has pass/fail eval coverage for every new issue code.
- Validation commands above have run, or failures are documented with cause.
- The issue is updated with PR link, validation, residual risk, and release/install closeout expectation.

---

## Follow-Up Backlog

Do not include these in the first PR unless they fall out trivially:

- Draft-text `sentence_count`, `bullet_count`, and `line_count` for `check-claims`.
- Structured CTA count or CTA family constraints for outbound copy.
- Markdown heading or section-order checks for unstructured proposal text.
- Additional source-safety constraints around public/source labels.
- Pack hash/source freshness strictness beyond current `pack_hash` check.
- A broader constraint authoring UX or prompt-contract extraction path for new constraint families.
