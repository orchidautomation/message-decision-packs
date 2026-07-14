# Portfolio-Aware GTM Scope

One GTM Message Decision Pack can hold shared company messaging decisions plus product-, capability-, solution-, and segment-specific decisions.
These are applicability dimensions, not new MDP primitives.

The distinction is:

- a primitive says what kind of decision an entry contains;
- scope says where that decision is eligible.

A product-specific pain is still a need/requirement.
A product-specific proof point is still evidence/proof.
A product-specific CTA is still an output contract.
The existing agnostic primitives and `primitive_map` do not change.

## Why One Pack Can Be Better Than One Pack Per Product

Portfolio companies usually have decisions at several levels:

- company-wide category boundaries and prohibited claims;
- product positioning and proof;
- capabilities that may belong to one product;
- packaged solutions that combine capabilities for a use case;
- personas and market segments that cut across products;
- hooks and CTAs that are appropriate only for one intersection.

Putting every product in a separate pack duplicates global rules and forces a caller to choose a pack before it may know the right product.
Putting everything in one unscoped pack lets incompatible claims, pains, and CTAs leak into the same drafting context.
Entry scope provides the middle path: one pack, shared global entries, and deterministic portfolio isolation.

Use separate packs when the profiles have different governance, primitive mappings, source policy, owners, release cadence, or data-safety boundaries.
Scope is not a reason to combine unrelated businesses into one file tree.

## Manifest Contract

Declare canonical identifiers under the optional profile block:

```yaml
profile:
  id: gtm
  version: mdp.profile.v0
  context_dimensions:
    product:
      - core-platform
      - developer-platform
    capability:
      - workflow-routing
      - call-intelligence
    solution:
      - outbound-governance
      - sales-productivity
    segment:
      - enterprise
      - mid-market
  context_dimension_dependencies:
    capability:
      - product
    solution:
      - product
```

Dimension and value declarations use lowercase kebab-case identifiers.
The manifest is a registry, not a second positioning system; descriptions, claims, pains, and proof still belong in cards.

Dependencies are generic.
The example says any capability- or solution-scoped entry must also name a product, and a caller selecting capability or solution must select product too.
This prevents a capability from one product being blended with a different selected product.

## Entry Contract

Add optional `scope` beside `applies_to`:

```yaml
- id: developer-workflow-pain
  title: Developer workflow handoff
  body: The engineering team needs a deterministic interface between product context and agent workflows.
  applies_to:
    - GTM Engineering
  scope:
    product:
      - developer-platform
    capability:
      - workflow-routing
  evidence:
    - reviewed-product-source
  avoid: []
```

`applies_to` still means actor or persona applicability.
`scope` is enforced portfolio applicability.
`metadata` remains advisory and must not be used as a substitute for either field.

Unscoped entries are global.
Global company boundaries and universal output rules remain eligible across products.
Scope a guardrail when it truly applies only to one product; otherwise leave it global.

## Matching Semantics

Entry values are OR within one dimension and dimensions are AND across the entry.

```yaml
scope:
  product:
    - core-platform
    - developer-platform
  segment:
    - enterprise
```

This entry matches either named product, but only for the enterprise segment.

Runtime context may be narrower than an entry.
An entry scoped only to `product: core-platform` still matches when runtime also selects a compatible capability and segment.

V1 accepts one runtime value per dimension.
Repeat `--scope` to select different dimensions, not to blend two products into one drafting route.

Missing, unknown, or incompatible scope fails closed for scoped entries:

- `scope_dimension_unknown`: the profile does not declare the requested dimension;
- `scope_value_unknown`: the profile does not declare the requested value;
- `scope_dependency_missing`: a dependent dimension was selected without its required companion;
- `scope_dimension_missing`: a potentially relevant entry needs a dimension the caller omitted;
- `scope_value_mismatch`: selected and entry values do not intersect;
- `scope_segment_conflict`: top-level prospect segment conflicts with `attributes.segment`.

Global entries may still appear as bounded guardrails, but missing or invalid scope blocks a portfolio-sensitive draft.
Partial or unrelated scope also blocks when no compatible scoped decision survives; selecting a valid segment does not satisfy a product-scoped route.

## Direct CLI Routing

Use repeatable selectors with `route`, `emit-brief`, and route-scoped `check-claims`:

```bash
mdp --json route \
  --dir . \
  --persona "GTM Engineering" \
  --job "initial email outbound message" \
  --scope product=developer-platform \
  --scope capability=workflow-routing \
  --entries

mdp --json emit-brief \
  --dir . \
  --persona "GTM Engineering" \
  --job "agent brief" \
  --scope product=developer-platform \
  --scope capability=workflow-routing

mdp --json check-claims \
  --dir . \
  --text "<draft>" \
  --persona "GTM Engineering" \
  --job "initial email outbound message" \
  --scope product=developer-platform \
  --scope capability=workflow-routing
```

For portfolio-sensitive routes, use bounded `entry_route.matches` or `context.entries` as drafting context.
`load_order` and `required_load_order` are empty because shared card files may contain entries for several products.
The `route` array still records audit metadata about how cards were considered; it is not a scope-filtered drafting payload.
`skills --dir <pack>` exposes the active profile's declared dimensions, dependencies, eligibility, and deterministic job routes. `--summary` preserves scope plus draft/check blocking state without including full entry bodies.

## Prospect-Driven Fit and Briefs

`fit` and `brief` derive declared dimensions from prospect `attributes`:

```json
{
  "name": "Taylor Lee",
  "title": "GTM Engineering Lead",
  "company": "ExampleCo",
  "segment": "enterprise",
  "attributes": {
    "product": "developer-platform",
    "capability": "workflow-routing"
  }
}
```

Scope attributes are scalar strings in V1 and should also be declared under `lead_input_requirements.attribute_definitions` when the pack wants input validation.
If the profile declares the `segment` dimension, the top-level `segment` field is authoritative and is normalized to the declared kebab-case identifier.
Do not duplicate a conflicting segment in `attributes`.

The CLI does not infer product from company prose, title, persona, signals, or keywords.
If product context is absent, the correct output is a scope gap and a no-draft state, not a guessed product.

Fit rules are scope-filtered before positive terms or disqualifiers are evaluated.
Briefs automatically include bounded context when the selected route is portfolio-sensitive, even if the caller did not pass `--context`.

## Claims and Proof

`check-claims` filters approved claims, avoid terms, output rules, paragraph rules, and structured constraints before evaluating the draft.
A protected integration claim approved only for product A cannot approve product B copy.
The checker remains category-trigger-based; entry scope does not turn every sentence into a universal claim allowlist.

`verify-output` artifacts do not carry portfolio scope in V1.
To prevent ambiguous card-entry bindings, `verify-output` returns `proof_output_scope_unsupported` when the pack contains scoped entries.
Use scoped route, fit, brief, and claim-check workflows until a future proof-output contract can carry and validate the same scope explicitly.

## Evaluation and Rollout

A pack with scoped entries should include at least:

- a selected product A route that includes A and excludes B;
- a selected product B route that includes B and excludes A;
- a missing-scope route that is blocked and reports `scope_dimension_missing`;
- a scope-aware claim or fit regression when those cards contain scoped entries.
- product A, product B, and missing-scope prospect-brief fixtures when briefs consume scoped entries.

Eval fixtures accept `scope` as a list of `dimension=value` selectors.
They can assert nested diagnostics with `expect_scope_issue_codes_contains` and `expect_entry_gap_reasons_contains`.

```yaml
id: developer-platform-route
command: route
persona: GTM Engineering
job: portfolio scope example
scope:
  - product=developer-platform
expect_entry_titles_contains:
  - Developer platform angle
expect_entry_titles_excludes:
  - Core platform angle
```

`mdp validate` warns when it finds scoped entries without a selected-scope route that asserts both inclusion and exclusion plus a missing-scope route that asserts blocking and a stable scope gap.
Ship scoped-entry changes and their consumer selector updates together.
Existing packs with no scoped entries retain their current route, fit, brief, and claim-check behavior.

## Authoring Checklist

1. Keep `required_primitives` and `primitive_map` agnostic.
2. Declare the smallest stable dimension registry.
3. Add dependencies for capability- or solution-like dimensions that require a product-like companion.
4. Scope the entry, not the whole primitive or profile.
5. Leave truly company-wide rules unscoped.
6. Add prospect attribute contracts when fit/brief will derive scope from rows.
7. Add inclusion, exclusion, and missing-scope evals.
8. Draft only from bounded entries on portfolio-sensitive routes.
9. Run `mdp validate`, `mdp eval`, and scoped `mdp check-claims` before activation.
