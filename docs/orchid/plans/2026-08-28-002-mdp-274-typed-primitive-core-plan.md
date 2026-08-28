# MDP-274 Typed Primitive Core Contract Plan

Date: 2026-08-28
Issue: MDP-274
Status: implementation-ready plan
Consumer: Orchid Work execution lane and cumulative PR review

## 1. Context and current behavior

MDP already treats ten kebab-case primitive IDs as a closed vocabulary, but the
production authority is duplicated:

- `cli/src/commands/health.rs::KNOWN_PRIMITIVES` owns a string slice used by
  profile, job, and eval validation and by unknown-value diagnostics.
- `cli/src/commands/schemas.rs::primitive_ids` independently owns the same ten
  strings and their order for exported manifest schemas.
- `cli/src/starter.rs::gtm_required_primitives` repeats the complete list, while
  `gtm_primitive_map` repeats individual IDs as map keys.
- `cli/src/commands/evals.rs` imports the health constant to validate profile
  eval fixture primitives, making one command module depend on another command
  module for core vocabulary.
- `cli/src/main.rs` declares crate modules; there is no neutral primitive module.

Current manifests intentionally store primitive IDs as strings. That wire shape
is a compatibility contract, so MDP-274 must centralize internal authority
without changing serialized YAML, JSON schemas, ordering, validation codes, or
human-readable expected-value diagnostics.

The architecture decision in
`docs/orchid/decisions/2026-08-28-primitive-core-profile-template-contract.md`
was approved by Brandon on 2026-08-28. It fixes exactly ten primitives and
forbids profile vocabulary or an extensible eleventh-primitive path in this
program.

## 2. Objective, scope, out of scope, and assumptions

### Objective

Introduce one typed, profile-neutral Rust authority for the ten primitive IDs,
then make health validation, eval validation, schema generation, and GTM starter
construction derive their primitive vocabulary from it.

### In scope

- Add a closed `PrimitiveId` type with canonical ordering, stable wire names,
  iteration, parsing, display, and serde behavior.
- Make each full-vocabulary production consumer derive from the type rather
  than maintaining its own list.
- Use typed variants when constructing canonical GTM required primitives and
  primitive-map keys.
- Preserve string-backed manifest fields and all public serialized values.
- Add exact-vocabulary, order, parse/serialize, unknown-value, consumer-parity,
  and template-byte regression tests.

### Out of scope

- Changing `Manifest.required_primitives`, `Manifest.primitive_map`, jobs, or
  eval fixtures from strings to typed wire fields.
- Primitive-driven card routing or replacement of `CardKind` (MDP-275).
- Neutral decision-input adapters (MDP-276), profile/job registries (MDP-277),
  template registries (MDP-278), or the conformance fixture (MDP-279).
- New primitive IDs, aliases, profile-specific primitive variants, migrations,
  releases, installation, deployment, or template byte changes.

### Confirmed assumptions

- The exact canonical order is `actors`, `decision-criteria`, `source-signals`,
  `needs-requirements`, `evidence-proof`, `boundaries`, `output-contracts`,
  `routing-jobs`, `gaps`, `evals`.
- Current public manifests and schemas must continue accepting and emitting
  these string values in this order where ordering is observable.
- The existing template parity tests are the authority for generated GTM and
  proposal file bytes.

## 3. Acceptance mapping

| Acceptance criterion | Implementation | Proof |
| --- | --- | --- |
| One production definition owns all ten IDs | Add `cli/src/primitives.rs::PrimitiveId` and remove the full lists from health, schemas, and starter. | Exact core vocabulary test plus a source audit that no second full ten-ID array remains. |
| Schemas expose the same ten values in canonical order | Build schema enum values from `PrimitiveId` rather than `primitive_ids`. | Focused schema tests compare the exported arrays with the exact canonical vector. |
| Unknown IDs fail closed | Parse/validate every primitive reference through `PrimitiveId`; preserve current issue codes and expected-list text. | Unit parse rejection plus existing and focused health/eval unknown-primitive tests. |
| Basic GTM and proposal artifacts remain compatible | Construct GTM required IDs and primitive-map keys from typed variants while retaining string-backed models. | Existing basic/proposal generated-template parity tests and strict validation/eval commands pass. |
| No profile vocabulary enters the core type | Keep the module limited to the approved ten IDs and generic conversion behavior. | Module review and exact-vocabulary negative assertions contain no `gtm`, `proposal`, persona, prospect, or opportunity variant. |

## 4. Affected files and symbols

### Production changes

- `cli/src/primitives.rs` (new)
  - Own `PrimitiveId`, canonical `ALL` order, wire-name conversion,
    `FromStr`/`Display`, serde conversion, and parse error behavior.
  - Keep each wire spelling declared in one macro-backed definition so enum,
    order, parser, serializer, and diagnostics cannot drift independently.
- `cli/src/main.rs`
  - Register the neutral `primitives` module.
- `cli/src/commands/health.rs`
  - Remove `KNOWN_PRIMITIVES`.
  - Make profile, job, primitive-map, and eval-reference checks call the typed
    parser and use the canonical names for diagnostics.
- `cli/src/commands/evals.rs`
  - Import `PrimitiveId` directly instead of depending on health's constant;
    retain current diagnostic codes, paths, and messages.
- `cli/src/commands/schemas.rs`
  - Remove `primitive_ids` and derive both required-primitive and primitive-map
    schema enums from `PrimitiveId` in canonical order.
- `cli/src/starter.rs`
  - Derive `gtm_required_primitives` from all typed IDs and use variants for the
    canonical primitive-map keys without altering mapping contents.

### Test changes

- Unit tests colocated in `cli/src/primitives.rs` cover exact count/order,
  display/parse, serde round trips, and unknown rejection.
- Focused tests in the affected command/starter modules prove schema, health,
  eval, and generated-template parity against the shared authority.

No plugin asset, checked-in template, public schema file, documentation surface,
or Cargo dependency should change for this issue.

## 5. Ordered implementation steps

1. Add the closed primitive module using one macro invocation that declares the
   ten `(variant, wire-name)` pairs. Generate `PrimitiveId`, `ALL`, `as_str`,
   iteration/name helpers, `FromStr`, `Display`, and serde implementations from
   that declaration. This makes deletion, reordering, or spelling changes
   visible at the one authority.
2. Add exact module tests. Assert the count and ordered wire vector literally,
   round-trip every variant through string and JSON, and reject empty, unknown,
   alternate-case, underscore, and profile-specific values. These assertions
   are the mutation sentinel for a changed core definition.
3. Register the module in `cli/src/main.rs`, then remove
   `health::KNOWN_PRIMITIVES`. Refactor validation helpers to parse values with
   `PrimitiveId` rather than accepting a duplicated known-string set. Preserve
   the existing returned `BTreeSet<String>` where downstream reporting depends
   on original string values.
4. Update primitive-map, job, and profile-eval reference validation to use the
   same parser. Build every expected-value diagnostic from the canonical name
   iterator so unknown IDs still fail closed with the same ordered list.
5. Update `cli/src/commands/evals.rs` to consume the core type directly. Add or
   retain negative coverage proving unknown fixture primitives produce the
   existing error contract.
6. Remove `schemas::primitive_ids` and make both schema enum sites consume the
   canonical ordered names. Strengthen schema tests to assert the complete
   vector at both surfaces rather than only first/last samples.
7. Refactor `gtm_required_primitives` to collect every typed ID and replace the
   ten canonical `gtm_primitive_map` key literals with typed variants. Do not
   convert product-owned card, prompt, input, job, or eval identifiers.
8. Add a consumer-conformance test that compares the generated starter's
   required list and map key set with the core authority. Combined with the
   literal core test and full schema-vector tests, a deleted, renamed, or
   reordered primitive fails at every full-vocabulary consumer.
9. Run formatting, focused tests, strict template validation/evals, exact
   generated-template parity, public-artifact lint, and the full Rust suite.
   Inspect the final diff for any profile-specific core vocabulary or generated
   artifact drift.

## 6. Tests and validation

### Focused automated checks

```bash
cargo fmt --manifest-path cli/Cargo.toml --check
cargo test --manifest-path cli/Cargo.toml primitives
cargo test --manifest-path cli/Cargo.toml commands::schemas::tests
cargo test --manifest-path cli/Cargo.toml commands::health::tests
cargo test --manifest-path cli/Cargo.toml commands::evals::tests
cargo test --manifest-path cli/Cargo.toml generated_basic_starter_matches_plugin_template
cargo test --manifest-path cli/Cargo.toml generated_proposal_starter_matches_plugin_template_pack_files
```

### Template behavior and regression checks

```bash
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic --strict
cargo run --manifest-path cli/Cargo.toml -- --json eval --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/proposal --strict
cargo run --manifest-path cli/Cargo.toml -- --json eval --dir plugin/assets/templates/proposal
cargo test --manifest-path cli/Cargo.toml
python3 -m unittest scripts/test_public_artifact_lint.py
python3 scripts/lint-public-artifacts.py
git diff --check
```

### Manual/source proof

- Confirm the new core module contains exactly ten approved variants and no
  profile-specific terms.
- Confirm no independent full ten-ID production list remains in health,
  schemas, starter, or eval validation.
- Confirm changed files stay within the listed Rust surfaces and this pinned
  plan; generated assets and template bytes remain unchanged.

## 7. Compatibility and migration behavior

This is an internal consolidation. Manifest models remain string-backed, so
existing YAML/JSON readers, writers, unknown-field behavior, and raw bytes do
not require migration. The typed parser is a validation authority, not a new
wire version. Diagnostic codes, paths, severity, expected-value order, schema
enum order, GTM starter output, and proposal assets remain stable.

No alias is added for alternate case, underscore spellings, or product terms.
Those values remain invalid rather than silently normalizing into a primitive.

## 8. Risks, safety boundaries, rollout, observability, and rollback

- **Ordering drift:** deriving a set instead of an ordered list could reorder
  schema values or starter YAML. Mitigation: preserve `ALL` order and compare
  complete ordered vectors in tests.
- **Diagnostic drift:** replacing string membership with parsing could alter
  issue codes or messages. Mitigation: retain callers' codes/paths and use the
  canonical name list in the existing message template.
- **Serde drift:** derive-only naming could diverge from explicit wire names.
  Mitigation: serialize and deserialize through the same declared wire-name
  mapping and test every variant.
- **Over-broad typing:** converting public manifest fields now would expand the
  migration boundary. Mitigation: keep external models as strings and type only
  the internal authority/consumption path.
- **Template mutation:** refactoring starter keys could alter bytes. Mitigation:
  run exact basic/proposal parity plus strict validation/evals.

Rollout is the cumulative PR only; there is no deployment or feature flag.
Observability is exact test output, unchanged template bytes, unchanged schema
vectors, and unknown-value diagnostics. Rollback is a revert of the MDP-274
commits; the public string-backed v0 contract remains intact throughout.

## 9. Blockers and readiness verdict

Brandon approved MDP-273 and the Linear blocking relation was cleared. The
working branch includes current `origin/main`, the approved decision, and no
unresolved repository or product decision blocks this lane.

**Verdict: `READY_TO_PIN` for one plan-pinned MDP-274 Luna implementation lane.**
