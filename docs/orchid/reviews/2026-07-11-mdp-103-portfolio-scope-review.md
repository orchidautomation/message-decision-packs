# MDP-103 Portfolio Scope Implementation Review

## Why This Change Exists

MDP needs to support organizations whose GTM decisions vary by product, capability, solution, persona, and segment without forcing one pack per product or weakening the existing agnostic primitive model.

The motivating examples point to two different portfolio shapes:

- Attio presents one platform with distinct capabilities such as Sequences, Call Intelligence, Workflows, reporting, and developer tooling. Treating every capability as a separate product would create artificial pack boundaries.
- Sendoso presents a platform with capabilities and solutions that can matter differently to GTM engineers, VPs of Sales, account executives, and other actors. Its MCP is an access/capability surface, not automatically a standalone product.

Research sources used for the product decision are recorded in the implementation plan and include Attio pricing/product announcements and Sendoso platform, solution, and MCP pages. No company-specific strategy or customer data is embedded in the public runtime or fixtures.

## Product Decision

Keep one MDP pack capable of representing a portfolio. Add profile-owned applicability dimensions and entry-owned enforced scope:

```yaml
profile:
  context_dimensions:
    product: [core-platform, developer-platform]
    capability: [workflow-routing, call-intelligence]
  context_dimension_dependencies:
    capability: [product]
```

```yaml
scope:
  product: [developer-platform]
  capability: [workflow-routing]
```

Product, capability, solution, and segment are not universal primitives. They qualify entries that still represent actors, needs, evidence, boundaries, outputs, jobs, gaps, or evals. This preserves MDP's domain-agnostic ontology and avoids creating a second product-information system.

## Safety Invariants

The implementation is considered correct only when all of these remain true:

1. Scope is enforced separately from actor/persona applicability.
2. Unscoped entries remain global.
3. Scoped entries match OR within one dimension and AND across dimensions.
4. V1 accepts one runtime value per dimension and refuses blended values.
5. Dependent dimensions require their declared companion dimensions.
6. Scope is never inferred from persona, title, company prose, signals, or free-form keywords.
7. Portfolio-sensitive drafting uses bounded entries only. Shared card paths remain audit metadata because a card may contain entries for several products.
8. Missing, invalid, partial, or unrelated scope blocks drafting when no compatible scoped decision survives.
9. A blocked context retains only compatible global guardrails, not global hooks, claims, CTAs, or other draftable decisions.
10. Claim approval, avoid rules, output rules, paragraph rules, and structured constraints use the same scope filter.

## Review Findings and Resolutions

The pre-PR Orchid/CE review found several issues that the first implementation and its initial green test run did not expose. They are documented here because they are reusable failure modes for future scoped-routing work.

### Fit-ready brief with blocked context

Problem: prospect brief readiness was computed from fit before bounded scoped context was built. A prospect could therefore return top-level `draft_status: ready` while `context.status` was `blocked`.

Resolution: final brief readiness now requires both fit success and ready bounded context. The no-draft reason, decision, and agent instruction use that final status. A regression covers an unscoped fit success followed by a portfolio context block.

### Partial or unrelated scope marked ready

Problem: route and claim readiness originally checked only whether any valid scope value was selected. Selecting `segment` could therefore bypass a missing required `product` selector.

Resolution: the routing kernel now tracks relevant scoped candidates and compatible scoped decisions. Portfolio-sensitive output is ready only when the selected scope is valid and at least one required scoped decision survives. Scoped-only guardrail routes use compatible scoped guardrails when there is no scoped decision candidate.

### Scoped guardrails missed on direct routes

Problem: guardrail candidacy was coupled to whether full bounded context was being rendered. Direct `route` calls could miss a scoped avoid/output rule that had no persona or job text match, leaving a whole-card load path exposed.

Resolution: guardrail candidacy is computed independently of context serialization. It participates in sensitivity, filtering, and gaps for direct and bounded routes.

### Blocked context exposed non-guardrail decisions

Problem: a blocked context preserved every compatible global entry even though the policy said only global guardrails were inspectable.

Resolution: blocked context now projects only entries selected as guardrails and reports counts that match the retained entries.

### Summary and schema surfaces hid safety state

Problem: full JSON carried scope state, but `--summary`, `schema brief`, and `agent-surface` omitted fields agents needed to discover selectors or determine whether drafting was blocked.

Resolution:

- route, fit, brief, emit-brief, and check-claims summaries preserve scope and blocking state;
- brief/context schemas describe scope, portfolio sensitivity, diagnostics, and bounded context;
- `agent-surface` exposes the current pack's dimension registry and dependencies;
- copy-brief guidance uses full JSON when bounded entry bodies are required.

### Eval coverage could pass vacuously

Problem: validation counted any fixture with a non-empty `scope` list as selected-scope coverage, even if it asserted no inclusion or exclusion. Missing-scope coverage likewise needed no diagnostic assertion.

Resolution: selected-scope coverage now requires a route fixture with both compatible inclusion and incompatible exclusion assertions. Missing-scope coverage requires blocked/no-draft plus a stable scope or entry-gap assertion. The starter adds product A, product B, and missing-scope fixtures for both direct routes and prospect briefs.

### Malformed selector error drift

Problem: malformed selectors used the generic `mdp_error` envelope and multiple `=` characters were normalized into a value.

Resolution: selectors require exactly one `=` and all selector parsing failures classify as `invalid_argument`.

## Explicit V1 Boundaries

- Runtime selection is scalar per dimension. Multi-product drafting is not supported.
- Dependencies express required companion dimensions, not a full value-level product/capability graph.
- Scope is entry-level; card-level scope optimization is deferred.
- `mdp.proof-output.v0` cannot carry scope. `verify-output` returns `proof_output_scope_unsupported` for packs containing scoped entries rather than accepting ambiguous bindings.
- MDP remains local decision context and routing infrastructure. This change does not add sending, sequencing, enrichment, CRM writes, scraping, or orchestration.

## Verification Evidence

Before PR creation, run and record:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --strict --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json eval --strict --dir plugin/assets/templates/basic --summary
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/proposal
cargo run --manifest-path cli/Cargo.toml -- --json eval --dir plugin/assets/templates/proposal --summary
make validate
git diff --check
```

The final PR description must link the implementation plan, this review, and MDP-103; report the exact test/eval counts; and retain the V1 proof-output limitation as an explicit residual boundary.
