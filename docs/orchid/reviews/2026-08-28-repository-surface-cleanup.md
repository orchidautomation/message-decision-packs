# Repository Surface Cleanup Audit

Date: 2026-08-28
Linear: MDP-282

> Superseded by the [MDP-341 repository streamlining audit](2026-09-02-repository-streamlining-audit.md). The Eve example retained by this audit was removed on 2026-09-02.

## Decision

Keep `docs/orchid/` as Orchid Relay's durable, public-safe workflow namespace.
Keep it outside canonical product navigation. Store private, raw, temporary, or
bulky work in `.agent-artifacts/`; obsolete code belongs in Git history.

## Removed

| Surface | Reason |
|---|---|
| `docs/plans/` | Superseded planning layout duplicated `docs/orchid/plans/`. |
| `docs/solutions/` | Empty legacy scaffold duplicated `docs/orchid/solutions/`. |
| `docs/orchid/history/archived-examples/` | Contained two superseded runnable applications; Git already preserves them. |
| `examples/proposal-flow-video/` | Obsolete mock/v0 Remotion walkthrough competed with the current runtime story. |
| `docs/proposal-demo-go-no-go.md` | Applied only to the removed video walkthrough. |
| `docs/pluxx-distribution-evaluation.md` | Explicitly historical pre-release evaluation superseded by `docs/distribution.md`. |

The proposal runner's six contract fixtures moved to
`scripts/fixtures/proposal-runner/` because current CLI and script tests still
exercise them. Demo-only scripts and video assets were removed.

## Retained Examples

| Example | Classification | Reason retained |
|---|---|---|
| `examples/ai-sdr-eve-vercel/` | Canonical runnable integration | Current Eve runtime example and independently validated package. |
| `examples/clay-audiences-self-serve-enterprise-expansion/` | Synthetic reference pack | Current v2 decision-input and source-lineage contract coverage. |
| `examples/cold-model-conformance/` | Test fixture | Used by recorded-evidence conformance validation. |
| `examples/decision-trace/` | Test fixture | Used by decision-trace and prompt-output authority checks. |
| `examples/route-budget/` | Test fixture | Used by route-budget, allocation, and installed-parity checks. |
| `examples/run-conformance/` | Test fixture | Used by shared run-conformance validation. |

All retained examples were changed within the current August 2026 contract
work and are referenced by current tests or canonical documentation. Their
presence is functional rather than historical.

## Documentation Boundary

- `README.md` is the concise product entry point.
- `docs/README.md` indexes canonical product documentation.
- `docs/orchid/` stores sanitized contributor workflow artifacts.
- Linear remains the private control plane.
- `.agent-artifacts/` remains gitignored local scratch.
