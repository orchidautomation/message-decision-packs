# MDP-275 — Primitive-driven shared authority selection

Status: `READY_TO_PIN` (revision 2 after execution conflict recovery)

## 1. Context and current behavior

The approved architecture decision in
`docs/orchid/decisions/2026-08-28-primitive-core-profile-template-contract.md`
keeps `CardKind` as a v0 loader and wire discriminator, but assigns shared
semantic authority to the closed primitive map plus a versioned compatibility
adapter.

Repository inspection on cumulative head
`4f7c5b43139748edb5d95508459339eae48a1425` confirms:

- `cli/src/primitives.rs::PrimitiveId` is the single typed authority for the ten
  canonical primitive IDs.
- `cli/src/models.rs::Manifest` exposes `primitive_map` and profile jobs;
  `ProfileJob.required_primitives` declares the authority each job needs.
- `cli/src/routing.rs::select_cards_with_diagnostics` currently treats
  `CardKind` as the authority for universal guardrails and priority. Persona and
  token matching decide the remaining candidates.
- `cli/src/routing.rs::apply_selection_authority`, `entry_status`,
  `is_context_guardrail`, and related helpers also classify shared routing
  behavior with GTM-shaped kinds.
- The shipped GTM and proposal manifests already map profile-native card IDs to
  primitives and declare required primitives for every job. Some cards
  intentionally appear in more than one primitive (for example GTM
  `positioning` and `channel-policies`), so a reverse index must support a set
  of primitives rather than demanding global one-to-one membership.
- Existing health validation rejects unknown primitive IDs and dangling mapping
  references before profile activation. Routing must still fail closed if it is
  called directly with an invalid in-memory manifest.
- Existing route, minimal-context, route-cap, budget, hash, and no-draft tests in
  `cli/src/routing.rs` are the compatibility authority for current output.

Confirmed compatibility requirement: valid GTM and proposal fixtures must keep
their current selected order, guardrail behavior, route caps, context budgets,
public JSON shape, and canonical context hashes. `CardKind` remains serialized
unchanged.

## 2. Objective, scope, out of scope, and assumptions

### Objective

Make shared routing resolve card authority from `Manifest.primitive_map` and the
selected `ProfileJob.required_primitives`, with `CardKind` used only by an
explicit v0 compatibility adapter where the current manifest lacks enough
granularity to preserve guardrail and priority behavior.

### In scope

- A private, typed reverse index from card ID to one or more `PrimitiveId`
  values.
- A private primitive routing-policy layer used by card selection and shared
  entry authority classification.
- Structured fail-closed routing through the existing validation and profile
  activation gate for malformed, incomplete, or conflicting primitive
  authority.
- Legacy fallback for manifests with no profile/primitive contract.
- Focused proposal-native and adversarial tests plus all existing routing and
  compatibility tests.

### Out of scope

- Removing, renaming, or changing serialized `CardKind` values.
- Changing manifest, schema, template, CLI JSON, receipt, or routed-context
  contracts.
- Adding a third profile or profile-specific switches in shared routing.
- Actor/persona or prospect/opportunity migration (MDP-276).
- Job/skill registry work (MDP-277), template registry work (MDP-278), or
  packaging/release work.
- Editing shipped GTM or proposal template bytes.

### Decisions and assumptions

1. Multiple primitive memberships are valid and are merged deterministically.
   They are not ambiguous by themselves.
2. A conflict means the active profile cannot prove its declared authority: an
   unknown primitive, a mapped unknown card, an unknown required primitive, or
   a required primitive with no mapped card. Existing health validation and
   `profile_activation_decision` own the structured fail-closed result. The
   routing index must not replace that result with an early `anyhow` error.
   Transient supplemental/unmapped cards used by compatibility and pressure
   tests may still route through the v0 adapter, but cannot satisfy a required
   primitive or become primitive-derived authority.
3. A manifest with no profile metadata and an empty primitive map is a legacy
   pack. It preserves the current `CardKind` behavior exactly.
4. For an active primitive contract, primitive membership and job-required
   primitives determine semantic authority/classification. Existing
   persona/job applicability continues to determine candidate eligibility so
   selected order and hashes do not drift. The v0 CardKind adapter preserves
   the existing fine-grained guardrail and priority distinctions that are not
   represented in `PrimitiveMapping`; it may route compatibility-only cards but
   may not claim that they satisfy primitive requirements.
5. No valid-pack output may drift. If a primitive-derived implementation would
   change a current GTM/proposal route, retain the existing output through the
   compatibility adapter and prove the primitive boundary with synthetic tests
   instead of rewriting fixtures.

## 3. Acceptance mapping

| Acceptance criterion | Implementation | Validation |
| --- | --- | --- |
| Shared routing is explainable from `primitive_map` and job requirements | Build the typed reverse index and resolve each selected card against the selected job's typed required primitives before applying the v0 adapter. | New unit tests show mapping/job changes control semantic authority while misleading proposal-native IDs do not. |
| Proposal authority does not depend on pretending requirements are pains or outputs are copy patterns | Derive authority roles from `needs-requirements`, `output-contracts`, `boundaries`, and the other primitives; keep the kind only as a compatibility discriminator. | Proposal-native fixture asserts equivalent selection/classification with non-GTM card IDs and primitive-derived roles. |
| GTM selection, budgets, hashes, and guardrails remain compatible | Preserve current order and fine-grained v0 behavior through the adapter; do not add fields to valid outputs. | Existing routing suite, exact context hash tests, route-cap pressure tests, GTM/proposal evals, and full Rust suite pass unchanged. |
| Missing, ambiguous, or conflicting mappings fail closed | Let existing health/profile activation validation reject incomplete or dangling active-profile authority; the index never treats invalid or unmapped references as primitive authority. Accept intentional multi-membership. | Adversarial route tests assert structured blocked status and existing diagnostic codes for unknown primitive, dangling card, and empty required mapping; multi-membership remains stable. |
| Legacy packs retain compatibility | Select the old adapter path only when both profile metadata and primitive mapping are absent. | Existing empty-map synthetic manifest tests remain byte/shape compatible; add an explicit legacy fallback assertion. |

## 4. Affected files and symbols

### Worker-owned implementation surface

- `cli/src/routing.rs`
  - Add private typed reverse-index and routing-policy structures.
  - Update `select_cards_with_diagnostics`, `route_entry_details`, and internal
    callers to propagate authority-resolution failures.
  - Update shared guardrail/priority/selection-class helpers to receive resolved
    primitive authority before invoking the v0 adapter.
  - Add focused tests inside the existing `#[cfg(test)]` module.
- `cli/src/primitives.rs`
  - Read-only by default. A small visibility/helper change is allowed only if
    routing cannot consume the existing `PrimitiveId::{ALL,names,as_str}` and
    `FromStr` contract without duplication.

### Sol-owned integration surface

- `docs/orchid/plans/2026-08-28-003-mdp-275-primitive-driven-routing-plan.md`
  is immutable during worker execution.
- `docs/orchid/qa/2026-08-28-mdp-275-execution-receipt.json` may be added after
  worker completion to record public-safe execution evidence.
- The cumulative PR body and Linear lifecycle evidence are updated only by the
  Sol orchestrator after exact-head verification.

### Forbidden worker surfaces

Do not edit `cli/src/models.rs`, command schemas, template manifests/assets,
plugin skills, Cargo files, documentation, fixtures outside `routing.rs`, or any
other CLI source. Escalate a plan conflict rather than widening scope.

## 5. Ordered implementation steps

1. **Index primitive authority.**
   - Parse each `manifest.primitive_map` key as `PrimitiveId`.
   - Build deterministic `card_id -> BTreeSet<PrimitiveId>` and
     `PrimitiveId -> mapped card IDs` indexes.
   - Index only mappings that resolve to current manifest cards. Preserve
     unresolved references for the existing health gate to diagnose; do not
     promote them into routing authority and do not early-error.
   - Allow transient supplemental cards without mappings to use compatibility
     applicability, but never count them as primitive authority.
   - Preserve intentional multi-membership without last-write-wins behavior.

2. **Resolve the selected job contract.**
   - Resolve the exact `ProfileJob` for the requested job ID.
   - Parse its `required_primitives` through `PrimitiveId`.
   - Resolve every recognized declared primitive that has mapped authority.
     Leave unknown/empty authority to the existing validation and profile
     activation blocker so CLI output remains structured and compatible.
   - Use the legacy adapter only for a manifest with no profile and no map.

3. **Introduce primitive routing roles.**
   - Map the ten primitives into private, profile-neutral roles needed by shared
     routing (actor/guardrail, decision, evidence, requirement, output,
     routing-job, gap, and eval-only/non-card authority).
   - Resolve a card's role set from its primitive set and the selected job's
     requirements.
   - Do not switch on `gtm`, `proposal`, profile-specific card IDs, or template
     names.

4. **Make selection primitive-aware while preserving v0 output.**
   - Use required primitive membership as semantic authority and
     classification, not as a new candidate-expansion or candidate-exclusion
     rule for v0 packs.
   - Retain persona and job/tag matching as the existing applicability signals.
   - Route primitive semantics through a clearly named v0 CardKind adapter for
     the fine-grained guardrail and priority distinctions not present in the
     manifest.
   - Preserve valid-pack card order, reason strings where already selected,
     route-cap receipts, and public output shape.

5. **Move shared entry classification behind resolved authority.**
   - Pass the resolved card authority into guardrail, selection-class, required
     status, and reason-code decisions that currently inspect `CardKind`
     directly.
   - Keep `card_kind` in all existing output fields unchanged.
   - Where a v0 distinction has no primitive-level representation, use the
     compatibility adapter explicitly rather than adding profile logic.

6. **Add adversarial and compatibility tests.**
   - Proposal-native IDs prove primitive membership, not GTM naming, controls
     authority.
   - Multi-membership is stable and deterministic.
   - Unknown primitive, dangling card, and required primitive without mapped
     cards produce the existing structured validation/profile-activation block.
     A transient unmapped supplemental card remains compatibility-only and
     cannot satisfy required primitive authority.
   - Legacy empty-map manifests preserve selection and ordering.
   - Existing GTM/proposal outputs, cap pressure, minimality, and hashes remain
     unchanged.

7. **Stop on unexplained drift.**
   - Any valid GTM/proposal selected-card, route-cap, budget, reason, or context
     hash drift is a plan conflict. Do not update golden outputs to accept it.

## 6. Tests and validation

Run on the worker commit and again on the final integrated exact head:

1. `cargo fmt --manifest-path cli/Cargo.toml --check`
2. `cargo test --manifest-path cli/Cargo.toml routing::tests`
3. `cargo test --manifest-path cli/Cargo.toml commands::evals::tests`
4. `cargo test --manifest-path cli/Cargo.toml generated_basic_starter_matches_plugin_template`
5. `cargo test --manifest-path cli/Cargo.toml generated_proposal_starter_matches_plugin_template_pack_files`
6. Strict validate and eval for initialized GTM and proposal starter packs.
7. `cargo test --manifest-path cli/Cargo.toml`
8. `python3 -m unittest scripts/test_public_artifact_lint.py`
9. `python3 scripts/lint-public-artifacts.py`
10. `git diff --check`
11. Deterministic source audit: no shared routing switch on profile IDs or
    proposal/GTM card IDs; all active-profile authority passes through the
    typed primitive index; no shipped template bytes changed.

The optional Clippy component is not installed in the pinned Rust toolchain and
is not required proof. Do not install it as part of this issue.

## 7. Compatibility and migration behavior

- No schema or data migration.
- No manifest, card, template, JSON, or serialized `CardKind` change.
- Current valid GTM and proposal routes must remain byte/shape/hash compatible.
- Active-profile malformed authority fails closed through existing
  health/profile-activation diagnostics rather than an early routing exception.
- Transient compatibility-only cards can still be selected by the v0 adapter,
  but do not contribute primitive authority.
- Legacy packs with neither profile metadata nor a primitive map use the exact
  previous compatibility path.
- No release, install, deploy, migration command, or third-profile activation.

## 8. Risks, safety boundaries, rollout, observability, and rollback

Risk is **Elevated** because this changes shared selection authority and can
affect context hashes or no-draft behavior.

Safety boundaries:

- Do not update expected outputs to hide drift.
- Do not expose entry bodies in new errors.
- Do not infer profiles from card kinds or IDs.
- Do not broaden the worker-owned file set.
- Do not merge, release, deploy, or mutate production.

Rollout is limited to the cumulative feature branch and PR #236. Existing
route/minimality receipts, strict evals, and CI are the observable proof.

Rollback is a clean revert of the MDP-275 implementation commit(s); no persisted
data or published contract requires reverse migration.

## 9. Blockers and readiness verdict

MDP-273 is approved, MDP-274 is integrated and verified, MDP-275 has no current
Linear blocker, the cumulative branch contains current `origin/main`, and the
affected repository contracts are inspected.

The initial execution attempt surfaced a plan/repository conflict: routing tests
intentionally rename and add cards without synchronously rewriting
`primitive_map`, expecting the existing health gate and compatibility path to
remain authoritative. Revision 2 resolves that conflict by preserving
structured health/profile blocking and treating transient unmapped cards as
compatibility-only. No implementation commit was accepted from the stopped
attempt. Linear is now consistently `In Progress` with `phase:in-progress`.

Readiness verdict: `READY_TO_PIN`.
