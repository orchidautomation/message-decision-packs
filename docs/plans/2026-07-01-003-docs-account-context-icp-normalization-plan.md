---
title: MDP-50 Account Context and ICP Normalization - Plan
type: docs
date: 2026-07-01
topic: mdp-account-context-icp-normalization
execution: knowledge-work
linear_project: MDP: Domain Profile Foundation
linear_issues:
  - MDP-36
  - MDP-39
  - MDP-50
  - MDP-40
  - MDP-41
origin:
  - docs/plans/2026-07-01-001-docs-domain-profile-foundation-plan.md
  - docs/plans/2026-07-01-002-docs-card-extensibility-primitive-map-plan.md
---

# MDP-50 Account Context and ICP Normalization - Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Specify the GTM account/company context contract and normalized ICP shape after MDP-39 settled card extensibility. |
| Product authority | This artifact resolves the account-context planning gap left open by MDP-37, MDP-38, and MDP-39. |
| Core decision | Treat `account-context` as a GTM profile context facet and possible future profile-owned card ID, not a new core `CardKind`. |
| First implementation stance | Keep account context input-contract-backed and distributed across `normalize-prospect`, `source_summary`, prospect fields, `signals`, `gaps`, and `fit-rules`; do not add a new card or Rust enum variant in the first slice. |
| Canonical ICP | Account context plus persona/actor context plus relationship context plus prompt normalization plus fit/readiness gates. |
| Stop condition | Do not implement code, template, prompt, or skill changes from this plan without a follow-up implementation issue or PR scope. |

---

## Product Contract

### Summary

MDP should answer "what type of company is this pack for?" from reviewed account/company context before it falls back to fit rules.
Fit rules decide proceed, pause, disqualify, or ask for more context after the account, person, and relationship facts are normalized.

The first GTM profile slice should not add a new core card kind for account context.
MDP-39 already chose fixed `CardKind` plus profile-owned card IDs and manifest-level `primitive_map`.
MDP-50 applies that decision: account context is a profile contract that can be mapped through prompt contracts, prospect fields, signals, gaps, and eventually a profile-owned card ID if repeated implementation evidence proves it is needed.

### Product Boundary

Account context is pack-owned decision context.
It is not a company database, CRM record, enrichment output, scraper result, source ownership layer, sender, sequencer, or BI surface.

The only accepted account inputs are supplied or reviewed local context:

- user-provided rows, CSV rows, CRM exports, Clay or Deepline rows, spreadsheet rows, research notes, or sanitized fixtures;
- supplied company/domain/URL values that the CLI can canonicalize without lookup;
- source-backed signals with source, confidence, freshness, and uncertainty;
- bounded reviewed metadata in prospect `attributes`;
- prompt traces that expose what was used and what is missing.

### Account Context Name

Use `account-context` as the GTM profile term for the account/company facet.

Do not add a core `CardKind::AccountContext`.
Do not add `account-context` to the basic template's `cards` list until profile-aware validation and routing can consume profile-owned card IDs safely.

For the first implementation slice, `account-context` should be represented by:

| Surface | Role |
|---|---|
| `input_contracts.prospect.normalizes: [account, person, relationship]` | Declares that prospect normalization covers account context. |
| `.mdp/prompts/normalize-prospect.yaml` | Converts messy supplied rows into `source_summary`, `normalized_prospect`, `normalization_trace`, gaps, and no-draft readiness. |
| `normalized_prospect.company`, `company_domain`, `company_url`, `segment`, `signals`, `attributes` | Carries the fit-ready account fields the CLI already accepts. |
| `source_summary.account_name`, `company_name`, `company_domain`, `inputs_used`, `confidence` | Carries prompt-level account identity and provenance. |
| `cards/signals.yaml` | Explains how account signals should be interpreted. |
| `cards/gaps.yaml` | Keeps missing account proof visible. |
| `cards/fit-rules.yaml` | Gates readiness and disqualification after account/person/relationship context exists. |

If later evidence shows that account profile content needs a dedicated reviewed source, add a GTM profile-owned card ID named `account-context` using the nearest fixed core kind, probably `signals` or `positioning`, and map it through `primitive_map`.
That later card is profile vocabulary, not a core card kind.

### Canonical GTM ICP Shape

Canonical GTM ICP is composite context:

| ICP component | Required content | Current or first-slice home |
|---|---|---|
| Account context | What type of company/account this pack is for, supplied account identity, segment, source-backed account signals, account metadata, and missing account proof. | `source_summary`, prospect `company*`, `segment`, `signals`, bounded `attributes`, `signals` card, `gaps` card. |
| Persona or actor context | Who the work is for or about, explicit persona, title-to-persona mapping, and review status when mapping is weak. | `personas`, `persona_mappings`, `normalized_prospect.persona`, `normalization_trace.persona`. |
| Relationship context | Why this account/person matters now, trigger, background, source kind, source freshness, and route-relevant relationship notes. | `normalized_prospect.trigger`, `background`, `source_kind`, `signals`, `normalization_trace.preserved_raw_fields`. |
| Prompt normalization | How messy source rows become provider-neutral CLI input while preserving missing data and uncertainty. | `.mdp/prompts/normalize-prospect.yaml` and prompt-output validation. |
| Fit/readiness gates | Whether enough context exists to proceed, pause, disqualify, or ask for more information. | `lead_input_requirements`, `mdp fit`, `fit-rules`, `normalization_trace.fit_readiness`. |

The CLI prospect schema can stay stable for the first implementation slice.
Do not add a top-level `account` object until there is a separate provider-neutral account input contract and enough account-only workflow evidence to justify it.

### Primitive Mapping

When profile metadata lands, the GTM mapping should treat account context as shared coverage across primitives:

```yaml
primitive_map:
  actors:
    cards:
      - personas
    input_contracts:
      - prospect
  source-signals:
    cards:
      - signals
      - gaps
    prompts:
      - normalize-prospect-row
    input_contracts:
      - prospect
  decision-criteria:
    cards:
      - fit-rules
  gaps:
    cards:
      - gaps
    evals:
      - evals/fit-insufficient-context.yaml
  output-contracts:
    cards:
      - output-rules
      - copy-patterns
      - ctas
      - hooks
```

If a future `account-context` card ID is added, map it under `actors`, `source-signals`, and possibly `needs-requirements`; do not replace `fit-rules` with it.

### Extraction Order

Agents and future profile-aware routing should answer account or ICP questions in this order:

1. Account context from supplied source summary, company fields, segment, attributes, and reviewed account signals.
2. Source signals and gaps that explain confidence, freshness, missing proof, and uncertainty.
3. Persona or actor routes from explicit persona, title, and pack-owned mappings.
4. Prompt contracts that define how messy rows are normalized and what missing fields mean.
5. Fit/readiness gates that decide proceed, pause, disqualify, or ask for more context.

This order prevents `fit-rules` from becoming the primary source for ordinary company-profile answers while preserving `mdp fit` as the deterministic gate.

### Account-Only No-Draft Contract

Account-only input is valid source material, but it is not enough to produce a prospect brief or draft when the current input contract is `prospect`.

First-slice behavior should be:

- `normalize-prospect` may return `normalized_prospect.name: N/A` and `normalized_prospect.title: N/A` when the supplied input has no person-level data.
- `normalization_trace.fit_readiness.ready_for_mdp_fit` must be `false` for account-only input that lacks person name or title.
- `normalization_trace.missing_required` and `gaps` must include the missing person fields.
- `mdp fit` must treat `N/A` name or title as missing context through the existing `present()` semantics and return `insufficient-context` when the pack requires those fields.
- `mdp brief --context` should not be run for account-only input unless a later account input contract exists.
- Agents must ask for a person row, use the account context as planning/research context only, or return the fit gate's insufficient-context decision.

This preserves the current no-invented-contact rule while making account-only behavior testable.

---

## Planning Contract

### Key Decisions

- KTD1. Use `account-context` as the GTM profile context facet name.
- KTD2. Do not add a new core `CardKind`, card file, or manifest card ref for account context in the first implementation slice.
- KTD3. Keep the first implementation input-contract-backed and distributed across `source_summary`, `normalized_prospect`, `normalization_trace`, `signals`, `gaps`, and `fit-rules`.
- KTD4. Keep `Prospect` stable for the first slice; use existing fields and `attributes` for bounded reviewed metadata.
- KTD5. Treat prompt output as reviewable input to the CLI, not final fit or draft authority.
- KTD6. Use `N/A` as the explicit no-draft sentinel for account-only person fields, and ensure fit/readiness treats it as missing.
- KTD7. Add a dedicated account input contract only after account-only workflows need deterministic account evaluation without person fields.
- KTD8. Let MDP-40 own strict profile validation and eval-gate semantics after this account-context contract is accepted.

### Implementation Surface For Follow-Up Issues

| Surface | Future change | Notes |
|---|---|---|
| `plugin/assets/templates/basic/.mdp/prompts/normalize-prospect.yaml` | Tighten account-context instructions, source-summary expectations, `N/A` account-only behavior, and fit-readiness trace fields. | Keep strict JSON and no external-system boundary. |
| `cli/src/commands/prompt_output.rs` | Add explicit validation for account-only `N/A` person fields, source-summary account fields, and `fit_readiness.ready_for_mdp_fit: false`. | Current validator already treats `normalized_prospect` as required and validates supported fields. |
| `cli/src/commands/routing.rs` | Add tests that `N/A` name/title are missing through `mdp fit` readiness. | Preserve current `Prospect` schema. |
| `cli/src/commands/schemas.rs` and `cli/src/starter.rs` | Keep schema and generated prompt examples in sync with account-only no-draft behavior. | Avoid adding a top-level `account` schema in this slice. |
| `plugin/assets/templates/basic/.mdp/cards/signals.yaml` | Clarify account signals as source-backed account context, not enough by themselves for fit. | Keep current card kind. |
| `plugin/assets/templates/basic/.mdp/cards/gaps.yaml` | Clarify missing company proof and missing person row behavior. | Preserve no-invention rule. |
| `plugin/assets/templates/basic/.mdp/cards/fit-rules.yaml` | Clarify fit/readiness separation from account-context description. | Fit gates after context. |
| `plugin/assets/templates/basic/.mdp/evals/*.yaml` | Add account-context and account-only fixtures after the prompt/fit contracts are implemented. | Coordinate with MDP-40. |
| `plugin/skills/mdp/SKILL.md` and `plugin/skills/mdp-prospect-brief/SKILL.md` | Update skill language to name account context and `N/A` no-draft behavior. | Keep `mdp fit` as source of truth. |
| `plugin/skills/mdp-icp-builder/SKILL.md` and `plugin/skills/mdp-create-pack/SKILL.md` | Update ICP authoring guidance so account context does not collapse into `fit-rules`. | Same PR as behavior changes. |
| Public docs | Update only when behavior changes land. | Docs-only planning can stop here. |

### Validation And Eval Strategy

Future implementation should add or update tests for these cases:

| Case | Expected result |
|---|---|
| Company-profile question with account context present | Agent/routing guidance uses account context, signals, and gaps before fit rules. |
| Company-profile question with account context absent | Surface `missing-company-proof` or equivalent gap; do not invent an ICP answer from generic fit copy. |
| Prospect row with company, domain, segment, trigger, source-backed signals, and valid person fields | `mdp fit` can return `fit` when fit rules match and readiness requirements are met. |
| Account-only row with no person name/title | Normalization emits `N/A` person fields, readiness false, missing person gaps; `mdp fit` returns `insufficient-context`; no draft or brief is produced. |
| Account-only row where the prompt invents a contact | Prompt-output validation fails or flags the output before it reaches `mdp fit`. |
| Prompt output with undeclared source fields | Prompt-output validation fails through `source_summary.inputs_used`. |
| Prompt output with account facts in `attributes` instead of signals | Validation or review guidance rejects raw evidence dumping; source-backed evidence belongs in `signals`. |
| Future profile metadata claims account context but maps no prompt, input contract, card, or gap | MDP-40 strict activation should fail; non-strict mode should warn. |

Run these checks after implementation:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
make validate
```

Planning-only changes do not require CLI behavior tests, but the artifact should be manually checked against current manifest, prompt, cards, skills, and the accepted MDP-39 plan.

### Sequencing

1. Implement the prompt/schema/fit readiness slice for account-only no-draft behavior.
2. Update starter/template prompt text and the generated prompt schema/example together.
3. Update GTM template cards only where account-context language affects signals, gaps, and fit-readiness boundaries.
4. Update MDP skills in the same PR as behavior or template changes.
5. Let MDP-40 add profile validation and eval-gate semantics using this plan's account cases.
6. Let MDP-41 update the profile-builder workflow so generated ICP work normalizes account, person, and relationship context together.
7. Defer a dedicated account input contract or `account-context` profile card until account-only workflows produce enough repeated evidence.

---

## Sources

- MDP-37/MDP-38 foundation artifact: `docs/plans/2026-07-01-001-docs-domain-profile-foundation-plan.md`.
- MDP-39 accepted plan: `docs/plans/2026-07-01-002-docs-card-extensibility-primitive-map-plan.md`.
- Linear issue: MDP-50.
- Current manifest/template: `plugin/assets/templates/basic/.mdp/manifest.yaml`.
- Account-context stress-test files: `plugin/assets/templates/basic/.mdp/prompts/normalize-prospect.yaml`, `plugin/assets/templates/basic/.mdp/cards/signals.yaml`, `plugin/assets/templates/basic/.mdp/cards/gaps.yaml`, `plugin/assets/templates/basic/.mdp/cards/fit-rules.yaml`.
- Prompt-contract decisions: `docs/orchid/decisions/2026-06-26-runtime-normalization-prompts.md`, `docs/orchid/decisions/2026-06-26-prompt-output-json-schemas.md`.
- CLI model, schema, prompt-output, fit, starter, and validation surfaces: `cli/src/models.rs`, `cli/src/commands/schemas.rs`, `cli/src/commands/prompt_output.rs`, `cli/src/commands/routing.rs`, `cli/src/starter.rs`, `cli/src/commands/health.rs`.
- Skills requiring updates when behavior changes: `plugin/skills/mdp/SKILL.md`, `plugin/skills/mdp-prospect-brief/SKILL.md`, `plugin/skills/mdp-icp-builder/SKILL.md`, `plugin/skills/mdp-create-pack/SKILL.md`.

The 2026-07-01 account-context decision note referenced by earlier planning was not present in the local `codex/mdp-company-account-context-decision` branch at planning time, so this artifact relies on merged plans and current repo contracts.
