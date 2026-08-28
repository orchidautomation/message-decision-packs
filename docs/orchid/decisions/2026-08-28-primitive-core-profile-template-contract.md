# MDP-273 Primitive Core, Profile, and Template Contract

**Date:** 2026-08-28
**Issue:** MDP-273
**Status:** Approved by Brandon on 2026-08-28
**Decision type:** Public architecture and compatibility authority

## Decision

MDP has one closed, profile-neutral core. GTM and proposal are the only product
profiles registered in this sprint. Profiles select and describe core
primitives; templates package profiles; skills and hosts consume the published
contracts. The dependency direction is:

```text
templates -> profiles -> ten-primitive core -> versioned validation/runtime evidence
```

Core code may switch on a primitive or a versioned core contract. It must not
switch on `gtm`, `proposal`, or another profile. A profile-specific adapter is
selected outside the core and is the only permitted exception.

The canonical primitive vocabulary is closed to exactly these ten IDs:

1. `actors`
2. `decision-criteria`
3. `source-signals`
4. `needs-requirements`
5. `evidence-proof`
6. `boundaries`
7. `output-contracts`
8. `routing-jobs`
9. `gaps`
10. `evals`

No profile, template, host, user, or future plugin may add an eleventh
primitive or define an alternate spelling in this program.

This document authorizes no runtime, schema, CLI, template, skill, fixture, or
wire migration. It records the target authority for MDP-274 through MDP-280;
those issues must retain the v0 adapters and bytes described below.

## Ownership and registry boundaries

| Layer | Owns | May depend on | Must not own |
| --- | --- | --- | --- |
| Core | The ten IDs, versioned validation/runtime evidence, neutral decision-input concepts, and compatibility rules | No product profile | GTM/proposal vocabulary, product copy, or template inventories |
| Profile | Profile ID/version, mappings, product foundation, input requirements, jobs, and profile eval expectations | Core contracts | New primitive IDs or host behavior |
| Template | A complete initialized tree and its manifest/profile selection | One registered profile and core contracts | A second semantic contract, hidden runtime defaults, or a third shipped profile |
| Skill | Human-facing instructions and a declared profile/job route | Published profile and job contracts | A competing job registry or deterministic validation authority |
| Host | Invocation, transport, display, and bounded handoff | Published CLI contracts and receipts | Decision semantics, source truth, or ambient-context authority |

The profile registry is closed and declarative for this sprint: `gtm` and
`proposal` only. The registry must reject unknown product profiles and must not
package executable profile plugins. A neutral profile is permitted only as a
synthetic conformance fixture in MDP-279. It is not a registered profile, an
install option, a capability, a skill, a template, or a packaged artifact.

The template registry exposes `gtm` and `proposal`, matching the current
`AVAILABLE_TEMPLATES` contract. The basic tree is the GTM template; the
proposal tree is the proposal sample. A future template requires a separately
approved registry change and compatibility evidence.

## Resolved architecture seams

| Seam | Current v0 contract | Internal authority after implementation | Compatibility rule and forbidden interpretation |
| --- | --- | --- | --- |
| `CardKind` | Closed serialized enum on `CardRef` and `Card`; routing uses it for guardrails and priority. | Primitive mappings plus versioned routing policy; `CardKind` remains a loader/wire discriminator. | Preserve every current kind and serialized spelling. Do not remove kinds, make proposal semantics depend on GTM kinds, or treat a kind as a new primitive. |
| Actor/persona | `Manifest.personas`, `target_personas`, `operator_roles`, `persona_mappings`, card personas, and `LeadInputRequirements` fields use persona vocabulary. | Neutral actor concept, with persona as a preserved presentation and input label. | Add an actor adapter; preserve persona fields and matching behavior. Do not equate operator, buyer, subject, and persona without an explicit mapping. |
| Prospect/opportunity | `Prospect` is the shared input shape; proposal uses `input_contracts: opportunity` and may emit `normalized_opportunity`. | Neutral decision-input representation with profile adapters for prospect and opportunity. | Preserve `Prospect`, `lead_input_requirements`, `normalized_prospect`, and the proposal alias. `normalized_opportunity` is not a second core object. |
| Profile registry | `Manifest.profile` carries profile ID/version and current validation gates activation. | Closed declarative registry for GTM and proposal, selected before core routing. | Preserve profile IDs and `mdp.profile.v0`; reject unknown active profiles. Neutral fixture remains test-only. |
| Template registry | `init` accepts `gtm, proposal`; GTM and proposal inventories are built through separate functions, with proposal files manually enumerated. | One data-first descriptor, inventory, and transactional publication pipeline. | Preserve initialized paths and bytes (apart from the documented generated name/id substitution). Do not infer a profile from a card or allow an unregistered template. |

## Compatibility matrix

Disposition values are deliberately limited to **preserve** (same public
contract), **additive-adapter** (new neutral authority may be added while the
old surface remains usable), and **defer** (no removal or semantic migration
in this program).

| Surface | GTM and proposal current public surface | Disposition | Future authority | Required proof |
| --- | --- | --- | --- | --- |
| Manifest identity | `format`, `id`, `name`, `version`, description, provenance, policy | preserve | Versioned manifest contract | Both templates validate and serialize without field loss. |
| Profile metadata | `Manifest.profile`, `Profile.id`, label/version, dimensions, product foundation | additive-adapter | Closed profile registry and neutral profile adapter | GTM/proposal activation remains valid; unknown and fixture-only profiles are rejected from packaging. |
| Primitive declaration | `required_primitives`, `primitive_map`, `PrimitiveMapping` | additive-adapter | Closed ten-ID core registry | Health and generated schemas accept exactly the ten IDs and reject an eleventh. |
| Cards and assets | `CardRef`, `Card`, closed `CardKind`, card YAML trees, proposal-native card IDs | preserve | Primitive-to-card mapping | All current card files parse, retain IDs/kinds/bytes, and route with equivalent guardrails. |
| Actor/persona fields | Persona arrays, mappings, target/operator roles, card persona selectors | additive-adapter | Actor model with persona projection | Existing persona selection and unsupported-persona behavior are unchanged; actor mapping has negative ambiguity coverage. |
| GTM input | `Prospect`, `input_contracts: prospect`, `mdp.input.prospect.v0` | preserve | Neutral decision-input adapter | Existing prospect schema, required fields, source classes, and normalization tests remain green. |
| Proposal input | `input_contracts: opportunity`, proposal requirements/proof/risk inputs, `mdp.input.opportunity.v0` | additive-adapter | Same neutral decision-input authority through proposal adapter | Proposal normalization retains required signals/attributes and does not leak opportunity-only semantics into GTM. |
| Lead requirements | `lead_input_requirements`, `LeadInputRequirements`, value/attribute contracts | preserve | Core input readiness contract | Health validation and readiness gates report the same missing/invalid fields. |
| Normalization | `normalized_prospect`, normalization trace, preserved raw fields and source audit | preserve | Versioned normalized decision-input contract | Deterministic normalization invariants, source references, and no-fake-person checks remain green. |
| Opportunity alias | `normalized_opportunity` exactly equals `normalized_prospect` for proposal output | preserve | Proposal-readable alias adapter | Mismatched, non-object, non-proposal, and absent-alias cases retain current diagnostics; equality is exact. |
| Jobs | `ProfileJob`, manifest `jobs`, seven current route IDs, required primitives and input contracts | additive-adapter | Closed profile/job registry | All seven current routes validate; duplicate, wrong-profile, unknown-job, unknown-primitive, and wrong-skill cases fail. |
| Skills | `PACKAGED_SKILL_IDS`, bootstrap IDs, GTM/proposal skill IDs | preserve | Skill metadata bound to registry route specs | Packaged IDs and route bindings remain unchanged; no neutral fixture skill is packaged. |
| Commands and JSON | Existing init, health, schemas, route, fit, brief, gap, prompt-output, and verification commands and JSON fields | preserve | Versioned CLI/schema authorities | Existing command fixtures and generated schemas remain compatible; new fields are additive only. |
| Routing and compiled context | `is_base_guardrail`, `card_priority`, route selection, routed-context output and budgets | additive-adapter | Primitive-aware routing policy with `CardKind` adapter | GTM and proposal route order, guardrails, caps, exclusions, and compiled-context hashes are equivalent for v0 inputs. |
| Receipts and traces | Prompt/invocation receipts, normalization traces, governed outputs, hashes, and run evidence | preserve | Versioned evidence chain | Receipt verification, hash binding, failure states, and public-safe diagnostics remain valid. |
| GTM initialized bytes | Basic `.mdp` manifest, cards, prompts, evals, sources, examples, and README | preserve | Descriptor-generated GTM inventory | Byte parity for canonical files, plus explicit proof for generated pack name/id substitution. |
| Proposal initialized bytes | Proposal `.mdp` manifest, cards, prompts, evals, sources, examples, and README | preserve | Descriptor-generated proposal inventory | Byte parity for canonical proposal files and transactional publication parity. |
| Template selection | `AVAILABLE_TEMPLATES` and `PROPOSAL_TEMPLATE_FILES` | additive-adapter | Single descriptor/inventory registry | `init --template gtm` and `--template proposal` produce the same validated trees and no third option. |
| Third profile | No shipped third product profile; neutral conformance data may be synthetic | defer | Future separately approved profile registry entry | MDP-279 proves generic conformance only; install, capabilities, inventories, and package parity show no third profile. |
| Breaking migration | No authorized rename/removal of v0 fields, IDs, commands, routes, skills, kinds, aliases, or files | defer | Separately versioned migration | A compatibility scan finds no removed/renamed v0 surface and no changed default bytes. |

## Downstream implementation and validation obligations

| Issue | Owns | Must prove |
| --- | --- | --- |
| MDP-274 | Centralize the ten primitives as one typed core contract. | Exactly ten IDs, no profile switch in core, schema/health authority alignment, and negative unknown-primitive coverage. This issue is blocked until Brandon approves this decision. |
| MDP-275 | Make shared authority selection/routing primitive-driven instead of CardKind-driven. | Existing `CardKind` guardrails, priority, route caps, context budgets, output shape, and hashes are preserved for GTM/proposal fixtures. |
| MDP-276 | Introduce neutral decision-input internals with GTM/proposal compatibility adapters, including actor/persona and prospect/opportunity seams where applicable. | Prospect v0 and opportunity v0 remain valid; normalized alias is exact for proposal and absent/non-proposal elsewhere; lead requirements remain enforced; actor mapping handles ambiguity without silently changing selection. |
| MDP-277 | Declarative closed per-profile job/skill registry. | Seven route specs and five packaged skill IDs remain compatible; wrong profile, duplicate, unknown, and fixture-only route tests fail closed. |
| MDP-278 | Unified data-first GTM/proposal template descriptor, inventory, and init/publication pipeline. | Basic and proposal inventories share one publication path, validate transactionally, preserve canonical bytes/paths, and reject unregistered templates. |
| MDP-279 | Cross-profile conformance gate and test-only neutral fixture. | Fixture is synthetic and test-only, exercises the ten-core contract, and is absent from installation, capabilities, templates, skills, and packaging inventories. |
| MDP-280 | Extension-boundary documentation, exact-commit integration validation, and cumulative PR handoff. | The cumulative downstream head documents extension boundaries, validates the exact integration commit, and hands off one cumulative PR with the required compatibility evidence. |

The cumulative downstream head must additionally run `cargo fmt --check`, the
full Rust suite, strict GTM and proposal validation/evals, profile conformance,
template-byte parity, plugin/asset/version parity, public-artifact lint, and
full CI-equivalent validation. MDP-273 records these obligations but does not
claim them as evidence for this documentation-only change.

## Safety, rollout, and rollback

This decision is inert documentation until implemented. Rollout is Brandon's
acceptance followed by MDP-274 through MDP-280 in dependency order. No issue
may remove a v0 surface or alter canonical initialized bytes under this
decision. Before implementation, rollback is a document revert. After
implementation, the cumulative PR can be reverted while the retained v0
adapters and artifacts continue to provide the compatibility boundary.

Public artifacts must remain synthetic, generic, and free of customer data,
private control-plane commentary, unsupported claims, credentials, or local
machine paths.

## Approval gate

**Approved by Brandon on 2026-08-28.** Brandon explicitly accepted the closed
ten-primitive core, the two-profile registry, the neutral test-only fixture
boundary, and every matrix disposition above. The architecture gate is clear;
MDP-274 may proceed without reopening these decisions.
