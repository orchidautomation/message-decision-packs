# MDP-273 Primitive-Core, Profile, Template, and Compatibility Contract Plan

Date: 2026-08-28
Issue: MDP-273
Status: implementation-ready plan
Consumer: Orchid Work execution lane and human architecture review

## 1. Context and current behavior

MDP already exposes a domain-profile contract, but several shared implementation
seams still encode GTM-era names or repeat profile-specific tables:

- `cli/src/models.rs` defines `Manifest.required_primitives`,
  `Manifest.primitive_map`, `Profile`, `ProfileJob`, the closed `CardKind` enum,
  and the shared `Prospect` input shape.
- `cli/src/commands/health.rs::validate_profile_activation` validates primitive
  coverage, while `validate_profile_jobs` checks jobs against the global routing
  catalog. The ten primitive strings are also repeated in validation helpers.
- `cli/src/commands/schemas.rs` repeats primitive IDs in generated schemas and
  exposes current manifest, input, and normalization wire contracts.
- `cli/src/starter.rs` constructs the GTM manifest, primitive mapping, jobs,
  prompts, and evals in code.
- `cli/src/skill_catalog.rs` owns the global packaged-skill list and the seven
  current GTM/proposal job routes.
- `cli/src/routing.rs` assigns guardrail behavior and route priority by
  `CardKind`, so proposal-native cards inherit semantics through GTM-shaped
  kinds.
- `cli/src/commands/prompt_output.rs` preserves `normalized_opportunity` as a
  proposal-readable alias that must exactly match `normalized_prospect`.
- `cli/src/commands/init.rs` constructs GTM content and a manually enumerated
  `PROPOSAL_TEMPLATE_FILES` inventory through different paths, even though both
  now use the transactional publication kernel.

The repository already ships and tests GTM and proposal behavior. MDP-273 must
freeze the ownership and compatibility decisions that MDP-274 through MDP-280
will implement. It must not change runtime, schema, CLI, template, or plugin
behavior itself.

## 2. Objective, scope, out of scope, and assumptions

### Objective

Add one public-safe architecture decision under `docs/orchid/decisions/` that
is sufficiently explicit for downstream implementation without re-deciding the
core/profile/template boundaries.

### In scope

- Fix the canonical ontology at exactly ten primitive IDs.
- Define executable ownership for core, profile, template, skill, and host
  layers and the allowed dependency direction.
- Resolve the `CardKind`, actor/persona, prospect/opportunity, profile registry,
  and template registry questions for this sprint.
- Record a field-by-field preserve/additive-adapter/defer disposition for
  manifest fields, cards, normalization artifacts, jobs, skills, CLI JSON,
  routed context, receipts/traces, and initialized template bytes.
- Map decisions to existing validation seams and required downstream tests.
- State the human approval gate that blocks MDP-274.

### Out of scope

- Rust, schema, template, skill, fixture, generated documentation, or runtime
  behavior changes.
- A third active profile, Support or Recruiting implementation, an eleventh
  primitive, arbitrary user-defined primitives, or executable profile plugins.
- Removal or renaming of existing v0 fields, aliases, card kinds, commands,
  job IDs, skill IDs, output fields, receipts, or template files.
- Merge, release, deployment, installation, or production mutation.

### Confirmed assumptions

- GTM and proposal are the only registered product profiles in this sprint.
- A neutral profile may exist only as a test fixture for MDP-279 and must not
  enter installation, capability, template, or skill inventories.
- The existing project scope and dependency chain remain accurate; current
  repository evidence does not require a parent or project scope change.

## 3. Acceptance mapping

| Acceptance criterion | Decision content | Validation |
| --- | --- | --- |
| Close `CardKind`, actor/persona, prospect/opportunity, profile registry, and template registry questions | Add a dedicated decision table naming the current contract, target internal authority, compatibility adapter, and forbidden interpretation for each seam. | Compare every cited symbol and field with current `main`; document review confirms no unresolved downstream choice. |
| Keep exactly ten canonical primitives | List the closed IDs once in the decision and prohibit profile- or user-defined additions in this program. | Compare against health/schema/starter definitions; public-artifact lint passes. |
| Give every GTM/proposal contract a preserve/migrate/defer disposition | Add the compatibility matrix covering manifests, cards/assets, input/normalization, jobs/skills, commands/JSON, routing/compiled context, receipts/traces, and template bytes. | Matrix review verifies every MDP-273 surface is present and no breaking migration is authorized. |
| Keep a third active profile out of scope | Separate the MDP-279 neutral conformance fixture from product registration, packaging, capabilities, and installation. | Decision contains an explicit non-shipping fixture boundary. |
| Let downstream issues implement without redesign | Assign each implementation consequence and required proof to MDP-274 through MDP-280. | Every downstream issue has an owned decision and validation row; MDP-274 remains blocked until human approval. |

## 4. Affected files and symbols

### Planned change

- `docs/orchid/decisions/2026-08-28-primitive-core-profile-template-contract.md`
  - New public architecture authority, compatibility matrix, downstream
    implementation map, validation map, rollback policy, and approval gate.

### Read-only authorities cited by the decision

- `cli/src/models.rs`: `Manifest`, `Profile`, `ProfileJob`, `PrimitiveMapping`,
  `CardKind`, `LeadInputRequirements`, and `Prospect`.
- `cli/src/commands/health.rs`: `validate_profile_activation`,
  `validate_profile_jobs`, primitive-map validation, lead-input validation, and
  normalized-opportunity compatibility checks.
- `cli/src/commands/schemas.rs`: manifest, primitive, profile-job, prospect, and
  prompt-output schema generation.
- `cli/src/skill_catalog.rs`: `PACKAGED_SKILL_IDS`, `JOB_ROUTE_SPECS`, and
  `route_spec`.
- `cli/src/routing.rs`: `is_base_guardrail`, `card_priority`, guardrail
  classification, and `CardKind`-based route behavior.
- `cli/src/commands/prompt_output.rs`:
  `validate_normalized_opportunity_alias` and normalized output projection.
- `cli/src/commands/init.rs`: `AVAILABLE_TEMPLATES`,
  `PROPOSAL_TEMPLATE_FILES`, GTM/proposal inventory builders, and transactional
  publication entry points.
- `cli/src/starter.rs`: GTM primitive, job, prompt, eval, and starter builders.
- `plugin/assets/templates/basic/` and `plugin/assets/templates/proposal/`:
  canonical initialized template trees whose bytes are compatibility evidence.

The implementation lane must not edit any read-only authority listed above.

## 5. Ordered implementation steps

1. Re-check every cited file and symbol at the pinned plan commit so the decision
   distinguishes shipped behavior from proposed downstream behavior.
2. Write the decision statement and an ownership table for core, profiles,
   templates, skills, and hosts.
3. Freeze the dependency rule:
   `templates -> profiles -> ten-primitive core -> versioned validation/runtime evidence`.
   Shared core may switch on primitive or versioned core contracts, but never on
   `gtm`, `proposal`, or another profile except through an explicitly registered
   adapter selected outside the core.
4. Resolve the five named architecture seams conservatively:
   - keep `CardKind` as a v0 wire/loader compatibility field while moving shared
     semantic authority to primitive mappings;
   - use actor as the neutral internal concept while preserving persona fields;
   - use a neutral internal decision-input representation while preserving
     `Prospect`, `lead_input_requirements`, `normalized_prospect`, and the exact
     proposal alias behavior;
   - replace the global job table with a closed declarative registry containing
     only GTM and proposal;
   - replace separate init implementations with one data-first template
     descriptor/inventory/publication pipeline while preserving template bytes.
5. Add the field-by-field compatibility matrix. Every row must name the current
   public surface, this sprint's disposition, future internal authority, and
   behavioral proof needed before the disposition is accepted.
6. Map each implementation consequence to MDP-274 through MDP-280, including
   negative and conformance proof. Do not move implementation into MDP-273.
7. Record rollback: the decision is inert documentation until implemented;
   downstream changes remain revertible as one cumulative PR because existing
   v0 adapters and bytes are retained.
8. Add a `Proposed pending Brandon approval` status. Human acceptance of this
   decision is the explicit gate before MDP-274 may become ready.

## 6. Tests and validation

### Focused checks for MDP-273

```bash
python3 -m unittest scripts/test_public_artifact_lint.py
python3 scripts/lint-public-artifacts.py
git diff --check
```

Manual review must additionally verify:

- every cited path and symbol exists at the exact branch head;
- the ten primitive IDs match current manifest/schema/validation behavior;
- each named compatibility surface has one explicit disposition;
- no private Linear commentary, customer data, unsupported product claim,
  absolute local path, or future-profile activation appears in the decision;
- no file outside the plan and decision artifact changed.

### Downstream proof required by the decision

MDP-274 through MDP-280 own code and behavioral tests. Their cumulative exact
head must ultimately run `cargo fmt --check`, the full Rust suite, strict GTM
and proposal validation/evals, profile conformance, template-byte parity,
plugin/asset/version parity, public-artifact lint, and full CI-equivalent
validation. MDP-273 records these obligations but does not run an unchanged
full suite as evidence for a documentation-only diff.

## 7. Compatibility and migration behavior

This issue authorizes no wire migration. Existing v0 manifests, fixed card
kinds, persona/prospect vocabulary, lead-input requirements, normalization
fields and aliases, GTM/proposal job and skill IDs, command names, JSON shapes,
routing output, receipts/traces, and default initialized template bytes remain
compatibility obligations.

Downstream work may add neutral internal types, explicit adapters, registries,
generated authorities, and tests. It must defer any removal or semantic change
to a separately approved versioned migration.

## 8. Risks, safety boundaries, rollout, observability, and rollback

- **Ambiguous authority:** a vague ADR would move architectural choices into
  implementation. Mitigation: every seam gets an explicit target and forbidden
  interpretation.
- **Accidental breaking scope:** documentation could imply existing fields are
  deprecated or removable. Mitigation: compatibility rows use precise
  preserve/additive-adapter/defer dispositions.
- **Third-profile leakage:** examples could be read as product activation.
  Mitigation: only GTM/proposal are registered; the neutral fixture is test-only
  and non-packaged.
- **Public/private leakage:** Linear is private control-plane context.
  Mitigation: cite public repository artifacts and issue key only; do not copy
  private strategy or customer material.

Rollout is the human acceptance of the decision followed by sequential
implementation of MDP-274 through MDP-280. Observability is repository review,
the focused lint result, and the exact plan/decision commit. Rollback before
implementation is deletion or reversion of this inert documentation; rollback
after downstream implementation is the one cumulative PR revert while v0
adapters remain intact.

## 9. Blockers and readiness verdict

No repository or dependency blocker prevents authoring MDP-273. The decision
must be reviewed and approved by Brandon before MDP-274 is made ready.

**Verdict: `READY_TO_PIN` for the MDP-273 documentation lane.**
