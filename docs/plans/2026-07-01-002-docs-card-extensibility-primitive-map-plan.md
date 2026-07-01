---
title: MDP-39 Card Extensibility and Primitive Map Migration - Plan
type: docs
date: 2026-07-01
topic: mdp-card-extensibility-primitive-map
execution: knowledge-work
linear_project: MDP: Domain Profile Foundation
linear_issues:
  - MDP-36
  - MDP-39
  - MDP-50
  - MDP-40
  - MDP-43
origin: docs/plans/2026-07-01-001-docs-domain-profile-foundation-plan.md
---

# MDP-39 Card Extensibility and Primitive Map Migration - Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Decide the card extensibility path and primitive-map migration mechanics before any account-context card is implemented. |
| Product authority | This artifact resolves MDP-39 from the accepted Domain Profile Foundation plan. |
| Core decision | Keep `CardKind` fixed as the core loader and validation family; let profiles use card IDs, titles, file paths, jobs, prompts, evals, and manifest-level `primitive_map` as domain vocabulary. |
| Account-context stance | Account/company context is the GTM stress test, not implementation scope; MDP-50 owns the account source, naming, normalized ICP shape, extraction order, and account-only no-draft behavior. |
| Migration stance | No `mdp.v0` format migration is required for the chosen path; existing packs and current card filenames/kinds remain valid. |
| Stop condition | Do not add `account-context`, `company-context`, `accounts`, proposal-native card kinds, or arbitrary custom kinds from this planning branch. |

---

## Product Contract

### Summary

MDP should support domain profiles without turning every profile noun into a Rust enum variant.
The conservative path is a semantic split: MDP keeps fixed core card families for validation and legacy routing, while profiles own card IDs and primitive mappings for domain meaning.

For example, a future proposal profile may use card IDs such as `proposal-roles`, `bid-no-bid-rules`, `proof-library`, or `review-gates`, but those cards should still declare one of the existing core `CardKind` families until evidence proves the core families need a breaking redesign.
The profile vocabulary lives in `cards[].id`, card file names, card titles, `jobs`, prompts, evals, and `primitive_map`.

### Product Contract Preservation

The MDP-37/MDP-38 Product Contract is unchanged.
This artifact only resolves the MDP-39 mechanics left open by KTD6 and the account-context stress test.

### Requirements

- R1. Existing packs without profile metadata remain valid and keep current route, fit, brief, check-claims, gaps, and eval behavior.
- R2. Current `cards.kind` values and card filenames remain valid unless a later MDP-43 migration plan intentionally changes the pack format.
- R3. Domain profiles express semantic coverage through manifest-level `primitive_map`, not by adding one core enum variant for each domain noun.
- R4. Profile-owned card IDs and file paths are allowed to be domain-native while `kind` remains a fixed core loader family.
- R5. `primitive_map` maps primitive IDs to card IDs, prompt IDs, input contract IDs, eval fixtures, and jobs by stable identifiers.
- R6. Unknown primitive IDs are validation errors because they break the universal taxonomy.
- R7. Missing mappings for declared required primitives are warning-first until strict profile validation or activation mode is introduced.
- R8. Card-file-level `primitive` is deferred because it duplicates manifest authority and makes many-to-many primitive coverage harder to validate.
- R9. Extensible custom `kind` strings are rejected for the first profile slice because they weaken current schema, prompt, validation, and routing guarantees.
- R10. Account/company context must remain pack-owned decision context, not a CRM record, enrichment record, scraper output, or source ownership layer.

### Account-Context Stress Test

The current basic template already treats account context as composite supplied context:

| Surface | Current role | MDP-39 implication |
|---|---|---|
| `cli/src/models.rs` `Prospect` | Carries `company`, `company_domain`, `company_url`, `background`, `trigger`, `segment`, `signals`, and bounded `attributes`. | Account context can enter the CLI without a new card kind. |
| `plugin/assets/templates/basic/.mdp/prompts/normalize-prospect.yaml` | Normalizes messy person, company, account, CRM, CSV, Clay, Deepline, spreadsheet, or research rows and refuses to invent contacts for account-only input. | Prompt contracts are part of profile input coverage and should be mappable in `primitive_map`. |
| `plugin/assets/templates/basic/.mdp/cards/signals.yaml` | Includes `company-context-signal` for company website, hiring, funding, product, and stack clues. | Account/company facts may be covered by source-signal cards before a dedicated account card exists. |
| `plugin/assets/templates/basic/.mdp/cards/gaps.yaml` | Includes `missing-company-proof`. | Missing account context should stay visible as a gap. |
| `plugin/assets/templates/basic/.mdp/cards/fit-rules.yaml` | Gates insufficient context and disqualifying execution asks. | Fit/readiness remains downstream from account/persona/relationship context. |

MDP-50 may choose a future GTM profile-owned account card ID, keep account coverage distributed across existing cards and prompt contracts, or use both.
Whichever option it chooses should not require a new core `CardKind`.

### Scope Boundaries

#### In Scope

- Deciding the first card extensibility path.
- Deciding whether `primitive_map` lives at manifest level or card level.
- Deciding whether custom domain card kinds are accepted now.
- Naming migration mechanics and backward compatibility rules.
- Using account/company context as the concrete stress test.
- Identifying downstream CLI, template, prompt, skill, and eval surfaces.

#### Out Of Scope

- Implementing any code, template, prompt, skill, or eval change from this artifact.
- Adding an `account-context`, `company-context`, or `accounts` card.
- Choosing the account-context card/source name, normalized account-plus-persona ICP input shape, extraction order, or account-only no-draft behavior.
- Adding proposal, hiring, support, partnerships, customer success, or legal profile templates.
- Making MDP a CRM, sequencer, enrichment provider, scraper, BI tool, AI SDR, or generic automation system.

---

## Planning Contract

### Key Decisions

- KTD1. Keep `CardKind` fixed in the first profile implementation slice. The enum remains the core loader family used by current manifest/card schemas, prompt target kind enums, validation, starter generation, and legacy route priority.
- KTD2. Treat profile-owned card IDs as the domain vocabulary. A proposal profile can name `cards[].id: bid-no-bid-rules` while using the nearest fixed core `kind` and mapping that card to `decision-criteria`.
- KTD3. Put primitive authority in manifest-level `primitive_map`. The manifest already owns the card list, prompt list, jobs, and profile metadata, so it is the right place to validate coverage without opening every card file.
- KTD4. Do not add `primitive` to card files in the first slice. A single card can cover multiple primitives, and one primitive can require multiple cards, prompts, input contracts, or evals.
- KTD5. Do not accept arbitrary custom `kind` strings yet. Current Rust deserialization, JSON Schemas, prompt schemas, prompt-output validation, route priority, and starter generation all assume the fixed enum.
- KTD6. Profile jobs should route by required primitives first, then fall back to existing GTM route behavior when no profile job applies. This preserves current packs while giving profile-aware jobs a stable coverage model.
- KTD7. Keep `format: mdp.v0` for additive profile metadata until a later accepted change breaks current pack semantics. MDP-43 is not required by this MDP-39 choice, but remains the owner if custom kinds or format changes are accepted later.
- KTD8. New profile-aware templates should require a new-enough CLI. Older CLIs may ignore unknown manifest fields during normal reads but current validation reports unknown fields in `issues`, so profile metadata should not be added to the basic template before validator support exists.

### Recommended Manifest Shape

The first implementation should extend the foundation sketch around identifier mappings rather than kind extensibility:

```yaml
profile:
  id: gtm
  label: GTM Messaging
  profile_version: mdp.profile.v0
  boundary: decision-pack-not-execution

required_primitives:
  - actors
  - decision-criteria
  - source-signals
  - needs-requirements
  - evidence-proof
  - boundaries
  - output-contracts
  - routing-jobs
  - gaps
  - evals

primitive_map:
  actors:
    cards:
      - personas
    input_contracts:
      - prospect
  source-signals:
    cards:
      - signals
    input_contracts:
      - prospect
    prompts:
      - normalize-prospect-row
  decision-criteria:
    cards:
      - fit-rules
  gaps:
    cards:
      - gaps
    evals:
      - evals/fit-insufficient-context.yaml

input_contracts:
  - id: prospect
    schema_ref: mdp.input.prospect.v0
    prompt: prompts/normalize-prospect.yaml
    normalizes:
      - account
      - person
      - relationship

jobs:
  - id: initial-email
    label: Initial email
    required_primitives:
      - actors
      - decision-criteria
      - source-signals
      - evidence-proof
      - boundaries
      - output-contracts
      - routing-jobs
      - gaps
```

The map keys are primitive IDs, not card kinds.
The values reference stable manifest-owned IDs and repo-relative fixture paths.
Cards can appear under multiple primitives when their content spans several semantic roles.

### Option Analysis

| Option | Decision | Rationale |
|---|---|---|
| Fixed `CardKind` plus manifest `primitive_map` | Accept | Preserves current enum-backed validation and lets profiles use domain-native card IDs without a format migration. |
| Extensible custom `kind` strings | Reject for first slice | Requires changing deserialization, schemas, prompt target kinds, prompt output validation, routing priority, tests, starter generation, and skill guidance in one move. |
| Profile-owned aliases as a separate manifest registry | Defer | `cards[].id`, file paths, titles, and `primitive_map` already provide profile vocabulary; a second alias registry would add mismatch risk. |
| Card/file `primitive` field | Defer | Duplicates manifest-level authority and cannot represent many-to-many primitive coverage cleanly without extra merge rules. |
| New fixed core kind for `account-context` | Reject for MDP-39 | Account/company context is a GTM profile concern until MDP-50 proves a reusable core concept. |

### Migration Mechanics

| Phase | Mechanic | Compatibility result |
|---|---|---|
| Current `mdp.v0` packs | No metadata required. Current `cards.kind`, filenames, prompts, evals, and skills stay valid. | No migration. |
| Profile-aware CLI support | Add optional manifest structs for `profile`, `required_primitives`, `primitive_map`, `input_contracts`, and `jobs`; add schema and validation awareness. | Existing packs still validate; profile packs require the new CLI for clean validation. |
| Basic/GTM profile adoption | Add profile metadata only after validation can distinguish advisory warnings from errors. | No card rename and no card kind change. |
| Domain profile templates | Use profile-owned card IDs and nearest fixed core `kind`; map semantics through `primitive_map`. | New domains avoid new enum variants. |
| Future custom-kind acceptance | If later evidence requires arbitrary `kind`, route to MDP-43 for a format migration and JSON migration UX. | Migration is explicit, not implied by MDP-39. |

### Validation And Routing Rules

- Unknown primitive IDs should be errors in all modes.
- `primitive_map` references to missing card IDs, prompt IDs, input contracts, job IDs, or eval fixture paths should be errors.
- Declared required primitives with no mapped source should be warnings in non-strict mode and errors in strict or activation mode.
- `cards[].kind` remains required and enum-backed.
- `cards[].id` becomes the primary profile vocabulary for domain-specific cards.
- Profile-aware routing should load cards mapped by a job's `required_primitives` before applying current persona/job token matching.
- Current route behavior remains the fallback when a pack has no profile jobs or the caller uses legacy persona/job routing.
- Eval fixtures should assert mapped card IDs, load paths, prompt contracts, and no-draft or gap behavior rather than custom `kind` strings.

### Implementation Surface For Follow-Up Issues

| Surface | Future change owner | Notes |
|---|---|---|
| `cli/src/models.rs` | Implementation slice after MDP-39 and MDP-40 | Add optional profile metadata structs while keeping `CardKind` fixed. |
| `cli/src/commands/schemas.rs` | Implementation slice after MDP-39 and MDP-40 | Emit profile metadata schemas and keep existing card kind enums. |
| `cli/src/commands/health.rs` | MDP-40 | Add profile validation and warning semantics that do not make advisory warnings fail ordinary validation. |
| `cli/src/routing.rs` | Implementation slice after MDP-40 | Add profile job/primitive-aware routing while preserving current fallback behavior. |
| `cli/src/starter.rs` | Implementation slice after MDP-40 | Keep generated starter manifest and template assets in sync. |
| `plugin/assets/templates/basic/.mdp/manifest.yaml` | Implementation slice after MDP-40 | Add profile metadata only after validator support lands. |
| `plugin/assets/templates/basic/.mdp/prompts/normalize-prospect.yaml` | MDP-50 | Account-plus-persona ICP normalization details stay downstream. |
| `plugin/skills/` and `skills/` | Same PR as behavior changes | Feature change hygiene requires skill instructions to match CLI/template contracts. |
| `plugin/assets/templates/basic/.mdp/evals/*.yaml` | MDP-40 and MDP-50 | Add primitive coverage and account-context cases after the account contract exists. |

### Test Strategy For Future Implementation

- `cli/src/commands/health.rs`: existing basic template validates unchanged.
- `cli/src/commands/health.rs`: profile metadata with known primitive IDs and existing card IDs validates in non-strict mode.
- `cli/src/commands/health.rs`: unknown primitive ID fails with an error.
- `cli/src/commands/health.rs`: missing mapping for a declared required primitive is warning-first in non-strict mode and an error in strict or activation mode.
- `cli/src/commands/health.rs`: `primitive_map` references to missing cards, prompts, input contracts, jobs, or eval fixtures fail with clear paths.
- `cli/src/commands/schemas.rs`: manifest schema exposes profile metadata without expanding the card `kind` enum.
- `cli/src/routing.rs`: profile job routing includes cards mapped through required primitives and preserves current `select_cards` behavior for packs without profile jobs.
- `cli/src/starter.rs`: generated starter profile metadata matches `plugin/assets/templates/basic/.mdp/manifest.yaml` once the template adopts the profile fields.
- `plugin/assets/templates/basic/.mdp/evals/*.yaml`: route, fit, brief, and claim evals remain green after profile metadata is added.

---

## Downstream Decisions

| Issue | MDP-39 output used as input | Decision still owned downstream |
|---|---|---|
| MDP-50 | Account/company context should not require a new core `CardKind`; it can be a profile-owned card ID, distributed mapping, input contract, or combination. | Account source/name, normalized account-plus-persona ICP input shape, extraction order, account-only no-draft behavior, prompt/template/skill/eval changes. |
| MDP-40 | Profile validation should operate over manifest-level `primitive_map` and fixed card kinds. | Warning semantics, strict activation gates, JSON issue shape, eval minimums, and account-context gap cases. |
| MDP-43 | No format migration required by the chosen path. | Migration UX only if a later accepted approach breaks `mdp.v0`, accepts arbitrary custom kinds, or renames existing card kinds/files. |
| Implementation slices | Use fixed `CardKind`, profile-owned IDs, and primitive-map routing. | Exact Rust structs, schema shape, validator messages, starter metadata, and route integration. |

---

## Sources

- MDP-37/MDP-38 foundation artifact: `docs/plans/2026-07-01-001-docs-domain-profile-foundation-plan.md`.
- Linear issue: MDP-39.
- Repo instructions: `AGENTS.md`.
- Current manifest/template: `plugin/assets/templates/basic/.mdp/manifest.yaml`.
- Account-context stress-test files: `plugin/assets/templates/basic/.mdp/prompts/normalize-prospect.yaml`, `plugin/assets/templates/basic/.mdp/cards/signals.yaml`, `plugin/assets/templates/basic/.mdp/cards/gaps.yaml`, `plugin/assets/templates/basic/.mdp/cards/fit-rules.yaml`.
- CLI model/schema/validation/routing surfaces: `cli/src/models.rs`, `cli/src/commands/schemas.rs`, `cli/src/commands/health.rs`, `cli/src/routing.rs`, `cli/src/starter.rs`.
- Prompt-contract decisions: `docs/orchid/decisions/2026-06-26-runtime-normalization-prompts.md`, `docs/orchid/decisions/2026-06-26-prompt-output-json-schemas.md`.
- Public product boundary docs: `README.md`, `cli/USAGE.md`, `docs/what-this-repo-is.md`.
