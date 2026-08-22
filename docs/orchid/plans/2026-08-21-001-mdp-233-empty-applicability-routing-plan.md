---
title: MDP-233 Empty Applicability Selectors as Universal Routing - Implementation Plan
type: bug
date: 2026-08-21
topic: universal-empty-applicability-routing
execution: code
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-233
linear_issues:
  - MDP-233
  - MDP-239
  - MDP-216
  - MDP-65
  - MDP-228
  - MDP-235
origin: "Linear MDP-233; base origin/main at 2cba9919483b5a7ba46efed53e3b5502b2abf477"
source_note: Public-safe plan using synthetic packs and empty-selector fixtures only. No Sanity, customer, provider, or private source material belongs in the implementation or validation artifacts.
---

# MDP-233 Empty Applicability Selectors as Universal Routing - Implementation Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Make empty card-level `personas` and entry-level `applies_to` selectors deterministic universal/no-selector matches wherever the public MDP contract promises that behavior. |
| Authority | The Rust CLI remains the sole authority for card discovery, entry routing, scope/policy compatibility, route-card caps, minimality, and terminal readiness. The installed skills and docs describe this authority; they do not implement a second matcher. |
| Selector semantics | Empty lists, and selector lists containing only blank values, are universal for the persona dimension. Non-empty values match the requested persona exactly and case-insensitively after trimming. Authored display values remain unchanged in output. |
| Prose boundary | Titles, descriptions, bodies, and tags are not persona declarations. Job/token matching remains a separate route signal; removing the current prose-as-persona fallback is part of the contract correction. |
| Compatibility | Keep the existing `mdp.v0`, route, brief, context, and route-budget contracts, reason/output fields, guardrail behavior, scope filtering, channel/lifecycle policy, and fail-closed card-cap/minimality gates. This is a behavioral fix, not a schema or format migration. |
| Sequencing | MDP-216 remains the owner of selector-reference validation. MDP-233 must land before the MDP-239 queue's dependent MDP-228 required-first allocation work and before MDP-235 route-budget summary work consumes route counts. |
| Product boundary | MDP remains a local/offline decision-context and routing contract. This change does not add a CRM, sequencer, enrichment provider, scraper, sender, model call, hosted service, or automatic pack migration. |
| Stop condition | Direct route, context/brief, fit applicability, route-budget, proof-output route binding, installed CLI smoke, docs, and skills all agree on universal empty selectors; strict undeclared-persona validation remains intact; focused and repository validation pass. |

## Product Contract

### Problem frame

The installed 0.1.73 capability and skill contracts already say that empty card
and entry selectors are universal/no-selector-compatible, and that prose is not
persona metadata. The runtime does not implement that contract consistently:

- `cli/src/routing.rs::select_cards_with_diagnostics` treats a card as a
  persona match only when a non-empty manifest card selector matches or the
  card description contains the requested persona. An empty card selector is
  therefore not universal, while a prose mention can incorrectly route a
  card.
- `cli/src/routing.rs::route_entry_details` computes `applies` with
  `.iter().any(...)`, so `applies_to: []` is false. A neutral empty-selector
  entry is then selected only when its title/body/selector text happens to
  overlap the job/persona or when it is a guardrail.
- The same route details feed `route --entries`, `emit-brief`, `brief
  --context`, route-budget preflight, and route-scoped claim checks. A false
  negative can become `not_applicable`, a missing gap, a larger full-card
  fallback, a different context digest, or an incorrect budget result.
- `commands/routing.rs::fit_prospect_with_signal_authority` has a separate
  `applies_to` predicate. It must at least honor universal empty fit rules so
  the public selector contract does not diverge between qualification and
  routing.
- `commands/proof_output.rs` already treats empty card/entry selectors as
  compatible in its route binding checks, but uses a private matcher and does
  not cover blank-only selectors. It must be brought under the same helper or
  parity-tested so a valid universal route cannot be rejected downstream.

The Sanity reproduction named by MDP-233 (`.mdp/cards/gaps.yaml#unresolved-public-authority`)
is represented in this plan only by a synthetic universal gap fixture. The
fixture must prove that the empty selector, not incidental prose, is what makes
the entry reachable.

### Scope

In scope:

- One crate-private, shared selector predicate for universal/no-selector and
  exact case-insensitive persona matching.
- Manifest card discovery for empty/blank `CardRef.personas`; job and tag
  matching remains separate and continues to work.
- Entry routing for empty/blank `Entry.applies_to`, with existing policy,
  channel/lifecycle, scope, guardrail, foundation, and card-cap checks retained.
- Fit applicability and proof-output route binding parity where those surfaces
  independently consume persona selectors.
- Synthetic unit, CLI, route-budget, brief/context, scope, guardrail, strict
  validation, and installed-artifact regression coverage.
- Public docs and installed skill guidance that make the selector boundary and
  the interaction with job matching, scope, and budgets explicit.

Out of scope:

- A new card or pack schema, a format/version bump, a selector migration
  command, or a rewrite of existing user cards.
- Fuzzy persona matching, title/description/body inference, substring persona
  matching, semantic ranking, or a new job classifier.
- Changes to `MDP-216`'s warning/strict validation policy for undeclared
  non-empty selectors. Empty values remain ignored by validation; they do not
  become undeclared-persona warnings.
- Bypassing `policy.max_cards_per_route`, context entry/byte budgets, scope
  requirements, channel policy, lifecycle compatibility, product-foundation
  gates, or no-draft behavior to make universal content fit.
- Provider calls, external research, generated/customer artifacts, or posting
  raw private inputs to Linear or the repository.

### Selector contract

The implementation should define one helper with semantics equivalent to:

```text
selector_is_universal(values): true when no value has non-whitespace content
selector_matches_persona(values, persona):
  selector_is_universal(values) OR
  any(trim(value).eq_ignore_ascii_case(trim(persona)))
```

Empty strings inside a mixed list are ignored; `['', 'PMM']` remains scoped to
PMM, while `[]` and `['', '  ']` are universal. The helper must not rewrite or
serialize the authored values. A universal selector only removes the persona
filter. The following gates continue after persona applicability is resolved:

1. card selection's independent job/tag candidate check and deterministic
   card priority/cap;
2. entry channel and lifecycle policy compatibility;
3. entry scope compatibility and portfolio-sensitive missing-scope blocking;
4. guardrail classification and product-foundation authority;
5. minimality entry/byte budgets and route-card cap diagnostics.

Thus a universal scoped entry can still be `scope_incompatible`, a universal
channel policy can still be incompatible with an email/follow-up job, and a
universal card can still be excluded by the configured card cap with the
existing `route_card_cap_excluded_applicable` diagnostic.

### Authority for duplicated card metadata

`manifest.cards[].personas` is the discovery-time selector because
`select_cards` intentionally operates from the manifest before opening card
files. Loaded `Card.personas` remains a validated card-level declaration and
is used by downstream route-binding checks. The plan must not add a second
card-loading/routing engine or silently prefer card prose. Synthetic fixtures
should author matching empty card selectors in both manifest and card files;
MDP-216 validation continues to report only non-empty undeclared selectors.

## Current Code Evidence at the Planning Base

The implementation branch starts from
`origin/main@2cba9919483b5a7ba46efed53e3b5502b2abf477`.

| Surface | Current fact | Planned seam |
|---|---|---|
| `cli/src/routing.rs::select_cards_with_diagnostics` | Base guardrails are selected first. Other cards match on manifest card persona or job/tag overlap; card description text is also treated as a persona match. Empty card selectors are not universal. | Replace the persona branch with the shared universal/exact helper; remove description-as-persona fallback; preserve job/tag matching, priority, cap ordering, and existing cap diagnostics. |
| `cli/src/routing.rs::route_entry_details` | `applies` is a non-empty `.iter().any(...)`; `persona_match` searches title/body/`applies_to` text; `ChannelPolicies` additionally requires job overlap. | Treat empty/blank entry selectors as persona-applicable, use exact non-empty selector matching, stop using prose as a persona predicate, and leave policy/scope/guardrail branches unchanged. |
| `cli/src/routing.rs::entry_context_value` / `entry_summary` | Output already carries authored `applies_to`, selection reason, reason codes, scope, evidence, and body/no-body variants. | Preserve output shape and authored values. Universal matches may use the existing `persona applies`/`persona_applicability` reason vocabulary; do not add a contract version. |
| `cli/src/commands/routing.rs::fit_prospect_with_signal_authority` | Fit rules use a separate haystack/segment `applies_to` predicate and keyword fallback. Empty selectors currently match no prospect. | Add the shared universal predicate while preserving existing fit haystack/segment and disqualifier semantics. Do not turn fit into the route matcher. |
| `cli/src/commands/proof_output.rs::validate_card_entry_ref` | Route binding already exempts empty card/entry selectors, but has a private case-insensitive helper and only checks `.is_empty()`. | Reuse the shared selector helper (or prove exact equivalent) for blank-only and non-empty parity; preserve guardrail-role exemptions and fake/incompatible-ref errors. |
| `cli/src/commands/health.rs::validate_persona_selector` | Empty/blank values are skipped; non-empty undeclared selectors are warnings and strict validation blockers. Existing MDP-216 tests prove case-insensitive matching and prose is not a selector. | Keep behavior. Add a regression that the universal fixture is warning-free while an undeclared non-empty fixture still fails strict validation. |
| `cli/src/commands/briefs.rs` | `emit_brief_scoped` and prospect briefs call `entry_context_with_runtime_scoped`; context entries and minimality are downstream of route details. | No independent matching logic. Add parity tests proving universal entries are present in `brief --context` and excluded lists do not contain their `not_applicable` reference. |
| `cli/src/routing.rs::route_budget_preflight` | Preflight calls `entry_route_scoped` for each declared persona/job and measures selected entries/bytes; universal guardrails are intentionally not dropped. | Add universal-entry coverage proving every declared persona sees the entry and that universal content counts toward budget/overflow outcomes. |
| `cli/src/commands/capabilities.rs` | `persona_reference_integrity.empty_selector_behavior` already states universal/no-selector compatibility and `prose_behavior` already rejects prose interpretation. | Treat these fields as the contract oracle; keep them exact and add assertions only if implementation changes require clarification. |
| `plugin/skills/mdp/SKILL.md`, `mdp-pack-builder`, `mdp-pack-review` | Installed skills already state empty selectors are universal and prose is not a declaration, but routing-eval guidance does not require a neutral universal-gap regression. | Clarify the full runtime boundary and require route/context/route-budget proof so the installed guidance cannot pass from validation-only evidence. |

## Technical Design and Implementation Units

### U1. Centralize selector semantics in the routing kernel

**Likely files and symbols**

- `cli/src/routing.rs`: add crate-private `selector_is_universal` and
  `selector_matches_persona` beside `tokens`/matching helpers. Normalize only
  for comparison; preserve authored strings in `Value` projections.
- `cli/src/routing.rs::select_cards_with_diagnostics`: use the helper against
  `CardRef.personas`. Remove `card.description.to_lowercase().contains(p)` as
  a persona match. Keep `card.description` and `card.tags` in `job_match` so
  job routing remains independent. Preserve base guardrail selection,
  candidate sort, selected order, cap behavior, and the existing reason/error
  vocabulary.
- `cli/src/routing.rs::route_entry_details`: compute persona applicability
  through the helper; build job tokens from title/body (not selector values)
  so an actor label cannot masquerade as job prose. Remove the
  `entry_text.contains(&persona_lower)` persona fallback. Keep the existing
  `ChannelPolicies` job/lifecycle gate, `entry_policy_compatible`, scope
  handling, guardrails, `not_applicable`, `scope_incompatible`, full-card
  fallback, and minimality input shape.
- `cli/src/routing.rs::match_reason` and context reason codes: retain
  `persona applies`, `entry job match`, and existing reason-code enums for
  compatibility. `persona_text_match` remains readable for legacy artifacts/
  schema compatibility but must not be emitted by the new persona selector
  path.
- `cli/src/commands/routing.rs::fit_prospect_with_signal_authority`: add the
  universal branch to the existing fit applicability predicate. Keep non-empty
  fit selectors' current haystack/segment behavior, keyword matches,
  scope-filtering, avoid/disqualifier logic, and fit statuses unchanged.
- `cli/src/commands/proof_output.rs::validate_card_entry_ref`: call the shared
  helper for card/entry route compatibility, or document and test an exact
  equivalent if module visibility makes reuse inappropriate. Blank-only
  selectors must have the same universal behavior as empty lists.

**Design rules**

- Do not make an empty selector a global override for job/channel/scope or
  card-cap policy. Universal means universal only on the persona dimension.
- Do not infer a persona from card/entry title, description, body, tags, or
  arbitrary metadata. Job matching can still use its existing card/entry job
  tokens, but selector lists are not part of that prose haystack.
- Do not introduce a new JSON `reason`, diagnostic code, schema version, or
  migration field unless a focused test proves an existing closed contract
  cannot represent the corrected result.

**Covers:** MDP-233 AC1–AC4, MDP-216 preservation, and the runtime portions of
AC5–AC7.

### U2. Preserve policy, scope, guardrails, and budget boundaries

**Likely files and symbols**

- `cli/src/routing.rs::route_entry_details`: retain the ordering that records
  `policy_incompatible`/`not_applicable`, computes scope compatibility, adds
  scope gaps, and only then emits matches/context entries. Add explicit tests
  for a universal scoped entry and a universal channel-policy entry.
- `cli/src/routing.rs::route_budget_preflight`: prove universal entries appear
  for every declared persona, contribute to `actual_entries`/`actual_bytes`,
  and can produce the existing overflow/near-budget statuses. Do not change
  budget limits or rank away universal requirements.
- `cli/src/routing.rs::context_minimality` and route-card-cap projection:
  preserve safe body-free exclusions, deterministic context digesting, and
  `route_card_cap_excluded_applicable` when universal cards consume the cap.
- `cli/src/commands/routing.rs::check_claims_scoped`: rely on the shared
  context result; verify that universal output/avoid/constraint guardrails
  continue to apply while a universal non-guardrail gap is routed as context,
  not as a guardrail.

**Behavioral matrix**

| Selector | Persona | Job/policy | Scope | Expected result |
|---|---|---|---|---|
| `applies_to: []` | any declared persona | compatible ordinary entry | unscoped | `matches` and bounded context; no `not_applicable` exclusion |
| `applies_to: [pMm]` | `PMM` | compatible | unscoped | selected by exact case-insensitive match; authored `pMm` preserved |
| `applies_to: [PMM]` | `Buyer` | no other job match | unscoped | `not_applicable`; prose mentioning PMM does not rescue it |
| `applies_to: []` | any | incompatible channel/lifecycle | unscoped | existing policy exclusion; universal persona does not bypass policy |
| `applies_to: []` | any | compatible | incompatible scope | existing `scope_incompatible` gap/exclusion; no silent global fallback |
| `applies_to: []` | any | compatible | compatible scope | selected and counted in portfolio-sensitive route |
| universal card | any | card is candidate | cap exceeded | existing cap diagnostic/block; no cap bypass |

**Covers:** AC2–AC4, AC6, MDP-65 context-budget behavior, and MDP-228's
required-first allocation input assumptions.

### U3. Add synthetic routing fixtures and focused Rust regressions

**Likely files and symbols**

- `cli/src/routing.rs` test module helpers (`temp_pack`,
  `narrow_starter_route_candidates_for_tests`, `add_supplemental_persona_card_for_tests`):
  stop using `personas: []` as an accidental way to make a card *not*
  discoverable. Use a declared, nonmatching synthetic persona (for example
  `PM`) where a cap fixture needs exclusion; reserve empty selectors for
  explicit universal tests.
- `cli/src/routing.rs` tests: add table-driven cases for universal card,
  universal gap, ordinary non-empty selector, case-insensitive selector,
  prose-only non-match, blank-only selector, universal scoped entry,
  incompatible channel/lifecycle entry, guardrail classification, and
  route-card cap/minimality. Assert both `matches` and `context_entries`, the
  absence/presence of exact exclusion reason codes, authored selector values,
  and no entry bodies in safe excluded projections.
- `cli/src/commands/routing.rs` tests: extend direct `route --entries` and
  route-scoped `check-claims` coverage with a synthetic universal gap whose
  title/body/job text contains no requested persona. Add a fit fixture proving
  an empty fit selector follows the shared universal branch without changing
  disqualifier behavior.
- `cli/src/commands/briefs.rs` tests: add a `brief --context` regression using
  the same synthetic gap and a generous test budget. Assert the entry is in
  `context.entries`, context remains ready when all other gates are ready, and
  `context.minimality.excluded` has no `not_applicable` record for it.
- `cli/src/commands/proof_output.rs` tests: add/extend a route-binding fixture
  with empty and blank-only card/entry selectors; assert a compatible
  universal reference remains accepted while a non-empty wrong-persona
  reference still fails.
- `cli/src/commands/health.rs` tests: preserve the existing MDP-216
  `persona_references_match_case_insensitively_and_ignore_empty_or_prose_values`
  behavior and add a strict-validation assertion that the universal fixture
  has no undeclared-selector warning while the existing `Architect` fixture
  still fails strict mode.
- `cli/tests/fixtures/persona-references/universal-gap-card.yaml`: add a
  public-safe synthetic `gaps` card with empty card `personas` and an
  `unresolved-public-authority`-style entry with `applies_to: []`, neutral
  prose, plus at least one non-empty scoped comparison entry. Keep all values
  synthetic and body text free of customer/provider claims.
- `cli/tests/fixtures/persona-references/README.md`: document that the
  universal fixture proves structured emptiness, while the declared and
  undeclared fixtures prove exact matching and MDP-216 warning/strict policy.

**Fixture assertions**

The synthetic family must cover all five required classes in one reviewable
set: universal gap, ordinary entry, guardrail, scoped entry, and non-empty
selector. It must include neutral prose so a test fails if implementation
accidentally reintroduces title/body persona inference. Use temporary pack
roots and clean only test-owned directories.

**Covers:** AC1–AC5, AC7, MDP-216 acceptance, and no-private-data safety.

### U4. Installed-artifact regression and public contract guidance

**Likely files and symbols**

- `scripts/release-install-smoke.sh`: create an isolated initialized pack,
  replace its synthetic `gaps.yaml` with
  `cli/tests/fixtures/persona-references/universal-gap-card.yaml`, make the
  manifest card reference's `personas` empty, declare a synthetic `Buyer`,
  and invoke the installed binary through `route --entries` and the relevant
  `brief --context`/`emit-brief` path. Assert the universal gap appears for a
  persona whose prose does not mention it, while the non-empty comparison
  entry remains excluded. Retain the existing declared/undeclared MDP-216
  checks and strict failure assertion.
- `scripts/test_public_artifact_lint.py` or the existing release smoke
  assertions: ensure the new fixture contains no private path, customer data,
  provider body, credential, or raw transcript. Do not commit generated
  release output.
- `docs/portfolio-scope.md`: add an "Applicability selector semantics"
  subsection after the `applies_to` contract: empty/blank card and entry
  selectors are universal for persona applicability; non-empty selectors are
  exact case-insensitive values; prose is not a selector; scope and job/policy
  gates still apply.
- `cli/USAGE.md` and `README.md`: add a concise cross-link/example so direct
  route, brief/context, and route-budget users know that empty selectors are
  intentionally global and may still consume context budgets.
- `plugin/skills/mdp/SKILL.md`: make the existing selector paragraph explicit
  about route/brief/route-budget behavior and the fact that universal does not
  bypass scope, guardrails, or caps.
- `plugin/skills/mdp-pack-builder/SKILL.md`: tell authors to leave genuinely
  global selectors empty, use explicit non-empty selectors for persona scope,
  and never rely on persona words in prose or empty selectors to suppress a
  card/entry. Keep the MDP-65 warning about persona stamps and budgets.
- `plugin/skills/mdp-pack-review/SKILL.md` and
  `plugin/skills/mdp-pack-review/references/routing-evals.md`: require a
  neutral universal-gap route plus ordinary/scoped/guardrail/non-empty
  comparisons, and require route/context/route-budget output proof rather
  than validation-only evidence.
- `cli/src/commands/capabilities.rs`: keep the existing
  `empty_selector_behavior` and `prose_behavior` fields exact; add or update
  only a focused assertion if wording needs to name the route surfaces. Do
  not widen the capability contract or add a new error code.

**Installed behavior contract**

The installed binary and packaged skills must agree on the same matrix. The
smoke test must use the installed artifact, not `cargo run` from the source
checkout, and must prove a synthetic gap is routable through the public route
surface. Any package/template mirror update must be generated or copied by the
repository's existing asset-sync workflow, never hand-installed into a second
skill root.

**Covers:** AC5–AC7 and the installed-skill/CLI parity requirement.

### U5. Contract, schema, and regression review handoff

The implementation should not change the closed output schemas, but it must
prove that corrected outputs remain valid:

- `cli/src/commands/schemas.rs`: run route/context/brief schema tests against
  universal entries, preserving required `applies_to`, reason, selection,
  selection-class, and reason-code fields. Retain `persona_text_match` in
  compatibility schemas even if the corrected matcher no longer emits it.
- `cli/src/commands/capabilities.rs`: verify selector integrity metadata still
  reports empty universal and prose-not-selector behavior.
- `cli/src/routing.rs` and `cli/src/commands/routing.rs`: compare direct route,
  `--entries`, brief/context, fit, check-claims, and route-budget outputs from
  one synthetic pack. Assert the same universal entry identity is selected,
  excluded only for policy/scope reasons, or counted in minimality as expected.
- `cli/src/commands/health.rs`: run default and strict validation and confirm
  MDP-216's undeclared-persona warning/block remains unchanged.

No new output contract is justified unless a schema test demonstrates that an
existing field cannot represent the corrected universal match. Keep any new
implementation details internal and deterministic.

## Ordered Implementation Steps

1. Reconfirm the clean implementation base and read the MDP-233/MDP-239
   contract before editing. Preserve the current Linear state, labels, and
   blocker relationships. Do not change issue status, phase, delegation, or
   parent-index metadata as part of implementation.
2. Add and unit-test the shared universal/exact selector helpers. Decide the
   blank-only behavior in the helper tests before touching callers; mixed
   blank/non-empty lists must remain scoped to their non-empty values.
3. Update manifest card discovery and entry route details. Remove only the
   persona interpretation of card/entry prose; retain independent job/tag
   matching and all policy, scope, guardrail, foundation, cap, and minimality
   gates. Keep output reasons and authored selectors stable.
4. Update the fit applicability branch and proof-output route binding to use
   the same selector semantics. Verify that this does not turn fit keyword
   matching into route matching or weaken proof-output role/card-kind checks.
5. Repair route test helpers that previously relied on `personas: []` to make a
   card undiscoverable. Add the synthetic universal card/gap fixture and the
   Rust route, context/brief, fit, proof, scope, guardrail, cap, budget, and
   MDP-216 validation regressions.
6. Add the installed-artifact smoke fixture and run route plus brief/context
   through the installed binary. Keep temporary packs and outputs outside the
   repository; assert the comparison non-empty selector is not selected by
   neutral prose.
7. Update the canonical scope/usage docs and the three installed MDP skill
   surfaces. Keep the guidance agent-facing Markdown unwrapped and use the
   existing skill packaging/asset-sync validators.
8. Run focused Rust/schema/Node or shell checks, then the full repository
   `make validate` gate. Inspect generated route-budget fixtures and release
   smoke outputs for accidental tracked changes; restore only test-generated
   files and do not revert unrelated user work.
9. Run `git diff --check`, review the diff for private data, accidental schema
   changes, prose fallback, cap/budget bypass, or state mutation, commit the
   plan/implementation changes on the task branch as appropriate, and hand off
   the exact pushed ref/commit/path. This MDP-233 task is plan-only for this
   worker; the implementation PR remains a later execution step.

## Verification Contract

The implementation PR should run the following from the repository root. All
fixtures and generated reports remain synthetic and temporary.

| Gate | Command or proof | Coverage |
|---|---|---|
| Formatting and diff hygiene | `cargo fmt --manifest-path cli/Cargo.toml -- --check`; `git diff --check` | U1–U5 |
| Routing kernel | `cargo test --manifest-path cli/Cargo.toml routing` | Universal/exact/prose/job/policy/scope/cap/minimality behavior |
| Command consumers | `cargo test --manifest-path cli/Cargo.toml commands::routing`; `cargo test --manifest-path cli/Cargo.toml commands::briefs`; `cargo test --manifest-path cli/Cargo.toml proof_output` | route `--entries`, fit/check-claims, brief/context, proof route binding |
| Persona validation | `cargo test --manifest-path cli/Cargo.toml health` (or the exact persona-reference filters) | MDP-216 undeclared warning/strict blocker remains intact; empty values stay warning-free |
| Schema/capability contract | `cargo test --manifest-path cli/Cargo.toml schemas`; `cargo test --manifest-path cli/Cargo.toml capabilities` | Existing closed route/context/brief schemas and capability metadata remain valid |
| CLI fixture proof | `cargo run --manifest-path cli/Cargo.toml -- --json route --entries ...`; `cargo run --manifest-path cli/Cargo.toml -- --json brief --context ...`; `cargo run --manifest-path cli/Cargo.toml -- --json route-budget --strict ...` against the synthetic pack | Same universal entry identity and no accidental `not_applicable`; budget counts universal content |
| Installed artifact | `bash -n scripts/release-install-smoke.sh`; `make validate-installers` (or the repository's exact release smoke target) | Packaged CLI routes the synthetic universal gap; existing install/persona tests remain green |
| Skill/docs packaging | `make validate-skills validate-skill-contracts validate-skill-evals validate-skill-packaging validate-asset-sync` | Installed skill guidance and mirrors match authored source |
| Full repository gate | `make validate` | Rust, CLI, template, route-budget, MCP, public-artifact, installer, skill, and packaging regressions |
| Static safety review | Inspect `git diff` and `git status`; confirm no source paths, bodies, credentials, provider data, or generated reports are tracked | Public-safe plan/fixture boundary |

If a gate is unavailable because of sandbox/dependency/network state, report the
exact command and reason. Do not replace a missing validation run with an
unverified claim. Generated route-budget fixtures and temporary install packs
must be outside the committed diff after validation.

## Dependencies, Risks, and Blocker Awareness

### Dependencies and sequencing

| Dependency | Relationship | Execution rule |
|---|---|---|
| MDP-216 | Completed selector-reference integrity and strict validation owner. | Reuse its declared-persona authority and exact warning/block behavior. Do not weaken it to make universal routing pass. |
| MDP-65 | Existing generated-route applicability and context-budget guardrails. | Universal entries/cards count as applicable authority; preserve minimality, route-card-cap, and no-draft behavior. Do not raise budgets or drop guardrails. |
| MDP-239 | Parent execution index for the MDP 0.1.73 sanity/friction queue. | Keep MDP-233 in its current Phase 0 planned/backlog state; this plan supplies the exact implementation artifact and does not delegate the index. |
| MDP-228 | Existing research child whose required-first allocator depends on correct selected/excluded route sets. | MDP-233 is the routing dependency. Do not implement allocator changes here; the plan must expose stable selected/excluded counts for its later tests. |
| MDP-235 | Later route-budget summary/filtering child that follows MDP-233. | Preserve stable route-budget reason distributions and identity fields so MDP-235 can summarize them without reinterpreting applicability. |
| MDP-226 | Neighboring routed-context readiness work. | No hard dependency for this selector correction; do not change its canonical context predicate. Verify context output remains schema-valid. |
| Base | `origin/main@2cba9919483b5a7ba46efed53e3b5502b2abf477`. | Rebase/refresh before implementation if main advances; do not implement on the dirty canonical checkout. |

### Risks and mitigations

- **Universal cards pressure the route cap.** Treating an empty card selector
  as universal can add a card to every route. Keep deterministic card priority
  and the existing cap diagnostic; add a universal-card cap fixture and never
  silently evict an applicable card.
- **Universal entries increase context size.** This is correct authority, but
  it can change a ready route to budget-blocked. Count universal entries and
  bytes before generation, preserve body-free exclusions, and document that
  authors should scope non-global authority rather than raise limits.
- **Removing prose fallback exposes latent pack drift.** A pack that relied on
  persona words in prose may route fewer entries. This is an intentional
  contract correction; the remediation is an explicit selector or a deliberate
  job/tag match, not fuzzy matching or an automatic content rewrite.
- **Channel/lifecycle regression.** A universal `applies_to` value must not
  make an email policy apply to LinkedIn or a follow-up policy apply to an
  initial job. Keep `entry_policy_compatible` and add explicit matrix tests.
- **Scope leakage.** A universal persona selector is not a universal portfolio
  scope. Continue to emit `scope_incompatible` gaps and block missing/invalid
  portfolio scope exactly as before.
- **Duplicated card metadata drift.** Manifest card refs drive discovery while
  loaded card metadata is validated/bound downstream. Keep both fixture copies
  aligned and do not introduce a root-aware second selector engine.
- **Cross-surface matcher drift.** Fit and proof-output have independent
  consumers. Reuse the helper where possible and compare route/context/fit/
  proof behavior in focused tests.
- **Output compatibility drift.** Do not add a new reason code or remove the
  legacy schema enum merely because the corrected path no longer emits prose
  matches. Keep old receipts/fixtures readable and new outputs schema-valid.
- **Fixture contamination.** Synthetic gaps must contain neutral prose and no
  real claims. Run public-artifact lint and keep installed/route-budget outputs
  temporary.

### Blocker state at planning time

No implementation blocker is known after the clean `origin/main` baseline.
MDP-233 itself has existing blocker relationships to MDP-228 and MDP-235; they
remain unchanged and are not resolved by this plan commit. The issue remains
`Backlog` / `phase:planned` / `delegate:codex` unless the owner changes it in a
separate Linear action. No status, phase, label, delegation metadata, or issue
relation mutation is part of this handoff.

## Compatibility and Rollback

### Compatibility contract

- `mdp.v0`, `mdp.route.v0`, `mdp.context.v0`, `mdp.brief.v0`,
  `mdp.message-brief.v0`, `mdp.route-budget.v0`, and their existing JSON
  schemas remain version-compatible.
- `CardRef.personas` and `Entry.applies_to` remain optional string arrays.
  Empty/blank-only values now have the documented universal persona behavior;
  non-empty selectors retain exact case-insensitive matching and authored
  capitalization.
- Existing job/tag matches, guardrails, product-foundation requirements,
  channel/lifecycle policy, entry scope, route caps, minimality budgets, output
  reason codes, and safe exclusion redaction remain unchanged.
- Existing packs with intentional empty selectors may route additional global
  entries/cards and may surface previously hidden budget/cap diagnostics. That
  is the expected correctness change. Packs that depended on prose persona
  fallback should add explicit selectors or job tags; no automatic migration is
  performed.
- MDP-216 default warnings and strict validation for undeclared non-empty
  selectors remain exactly as before. Empty selectors do not require a manifest
  persona declaration.
- Existing proof-output artifacts with route bindings remain readable; the
  route compatibility check becomes more consistent for blank-only universal
  selectors without upgrading any proof/output authority.

### Rollback

- Revert the single implementation PR/commit if a release blocker appears. No
  database, pack-format, or irreversible file migration is introduced.
- During rollback or staged rollout, operators may add explicit persona
  selectors to a pack that must remain narrow, or use the prior CLI with the
  existing pack. Do not delete entries, rewrite customer packs, add a global
  prose fallback, or raise context/card budgets as a workaround.
- Revert docs, skills, and synthetic fixtures together with the runtime change
  if the contract is rolled back; leaving the installed guidance ahead of the
  binary would recreate the same drift this issue fixes.
- No Linear status, phase, label, delegation, or blocker relation rollback is
  needed because this plan does not mutate those fields.

## Acceptance Mapping

| MDP-233 acceptance criterion | Planned implementation and proof |
|---|---|
| `applies_to: []` matches every declared persona without prose/job overlap | U1 shared selector helper and `route_entry_details`; U3 neutral universal-gap tests call route details for multiple declared personas and assert the same entry identity is selected. |
| Non-empty selectors retain exact, case-insensitive persona matching | U1 trims only for comparison; U3 tests `pMm`/`PMM` selection, wrong-persona exclusion, and authored-value preservation. |
| Card-level empty persona selectors follow the same documented semantics | U1 updates `select_cards_with_diagnostics`; U3 tests matching empty manifest/card metadata and U4 installed CLI smoke routes a universal card for a persona absent from the fixture prose. |
| `not_applicable` is not emitted for a compatible universal entry | U2/U3 assert the entry is in `matches`/`context.entries`, absent from `excluded` with `not_applicable`, and still excluded only for explicit scope/policy incompatibility. |
| Synthetic fixtures cover universal gaps, ordinary entries, guardrails, scoped entries, and non-empty selectors | U3 adds the table-driven Rust fixture family and `universal-gap-card.yaml`; each class has positive/negative assertions and neutral prose. |
| `route --entries`, `brief --context`, `route-budget`, docs, and installed skills agree | U2/U3 direct parity tests, U4 scope/usage/skill updates, route-budget count/overflow assertions, and U4 installed-artifact smoke use the same fixture and selector matrix. |
| Existing MDP-216 undeclared-persona validation remains intact | U3 retains health tests and U4 retains installed declared/undeclared fixtures, default warning output, and strict validation failure for `Architect`. |
| Scoped universal entries do not bypass portfolio isolation | U2 matrix and U3 route tests assert compatible scope selection, incompatible scope gaps, and missing-scope blocking. |
| Guardrails remain deterministic and are not reclassified as ordinary universal entries | U2/U3 assert avoid/output/fit guardrails keep `selection: guardrail`, existing reason codes, and safe context behavior. |
| Route-card caps and context budgets remain fail-closed | U2 route-budget/cap tests assert universal content counts toward budgets and cap exclusions produce existing diagnostics instead of silent omission. |

## Definition of Done

- Empty/blank card and entry selectors are universal only on the persona
  dimension; non-empty selectors are exact case-insensitive matches and prose
  is never a persona declaration.
- Route, context/brief, fit applicability, check-claims, route-budget, and
  proof-output route binding share or prove the same selector semantics.
- Universal entries/cards remain subject to policy, scope, guardrail,
  foundation, card-cap, and minimality gates; no authority is silently dropped.
- Synthetic focused tests, installed CLI smoke, strict MDP-216 validation, docs,
  and installed skills all pass; no private/generated artifact enters the repo.
- The implementation PR can link MDP-233 and preserve the existing
  `sync:pr-link-only` contract. This plan does not add delegation branding,
  autofix labels, status transitions, issue relations, or implementation code.
