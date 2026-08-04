# MDP-178 Clean-Context Architecture Review

Date: 2026-08-03
Reviewed artifacts:

- `docs/orchid/plans/2026-08-03-001-feat-unified-clean-context-runtime-plan.md`
- `docs/orchid/decisions/2026-08-03-unified-clean-context-runtime.md`
- `docs/orchid/decisions/2026-07-21-runner-receipts-and-context-isolation.md`

## Review Scope

Six independent review lenses checked the requirements and architecture for internal coherence, feasibility against the current repositories, product fit, security, scope control, and adversarial failure modes. The review was requirements-level: it challenged missing or conflicting authority boundaries without requiring implementation details that belong to MDP-179 and later issues.

## Findings Applied

- Required isolation controls now fail closed; only optional or observational properties may remain unknown and downgrade assurance.
- Deterministic GTM operations now use the shared runtime without invoking a model or claiming fresh inference.
- Run identity now changes only when a hash-bound authoritative artifact changes; explanation and presentation outside the receipt do not mutate the original run.
- Real-provider action-time approval is scoped to MDP-maintained proof and evaluation calls; customer hosts may bind a pre-approved execution policy.
- Invocation consumes one immutable content-addressed snapshot, with safe staging and pre-invocation identity verification.
- Receipt replay classification now requires expected identity, freshness policy, and a durable host consumption ledger; standalone verification is integrity-only.
- Secret injection, private artifact lifecycle, transactional failure publication, and structural prompt-injection boundaries are explicit.
- Driver executable or image, configuration, dependency build, and provider endpoint identity are bound when observable and otherwise remain attested or unknown.
- Sentinel tests require enforcement-layer and outbound-request evidence; absence from generated text is not proof.
- Clean-run assurance explicitly begins at the frozen boundary and does not cleanse ambient influence already embedded in a pack, prompt, or selected input.
- The MVP reuses bounded headless-runner adapters and does not become a portable container manager.
- GTM no-draft behavior preserves valid attempted-complete and pack-declared optional or conditional evidence semantics.

## Residual Planning Risks

- MDP-179 must choose one canonical implementation authority across the current Rust CLI and JavaScript proposal runtime without duplicating canonicalization or receipt semantics.
- Platform containment differs on macOS and Linux; each unsupported or unobservable control must produce a specific assurance downgrade.
- Host-mode replay protection requires durable state owned by the host, while CLI-only verification remains limited to integrity and compatibility.
- Provider-internal storage, policy, model aliases, and caching remain outside local proof unless the provider exposes verifiable metadata.
- Upstream pack and source provenance may be attested or unknown; the runtime must display that limitation without absorbing source collection.
- MDP-185 through MDP-187 must remain independently gated so hosted scope cannot delay the local runtime.

## Review Result

The requirements and ADR are suitable as the MDP-178 architecture authority after the applied changes. They do not authorize an unqualified `audit-grade` claim, a generalized hosted API, or a separate proposal/GTM runtime. MDP-179 must convert the product contract into an implementation-ready contract and compatibility plan before code execution begins.
