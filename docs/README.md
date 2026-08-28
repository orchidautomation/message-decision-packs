# MDP Documentation

These are the current public docs:

- [Getting Started](getting-started.md): installation and first use.
- [Local MCP](local-mcp.md): the canonical four-tool local stdio path and bounded proposal v0 migration guidance.
- [Conceptual Decision Flow](conceptual-decision-flow.md): pack layers, routing, fit, briefs, and drafting boundaries.
- [Portfolio-Aware GTM Scope](portfolio-scope.md): scope existing primitives by product, capability, solution, or segment within one pack.
- [Prompt Contracts](prompt-extraction-contract.md): normalization and extraction schemas.
- [Decision Input Contracts](decision-input-contracts.md): attempted-complete job inputs, source-attempt policy, normalized envelopes, and no-draft behavior.
- [Cold-model Conformance](cold-model-conformance.md): deterministic sufficiency, recorded behavioral trials, composite authority, private/public reports, and privacy boundaries.
- [Behavioral skill evaluations](skill-behavioral-evals.md): clean-context skill trials, baseline/previous comparisons, sanitized aggregates, and blind human review.
- [Runner Receipts](run-receipts.md): context-isolation receipt contract for audit-grade proposal workflows.
- [Compatibility Proposal Runner Surface](proposal-runner.md): bounded v0 command/MCP compatibility for existing proposal consumers.
- [Deterministic Proposal Evidence Harness](proposal-evidence-harness.md): network-free synthetic contract and negative-boundary proof for CI.
- [Headless Normalization Runners](headless-normalization-runners.md): native/headless runner recipes for Codex, Claude Code, Cursor, OpenCode, and the bundled local stdio MCP wrapper.
- [Native API Normalization Runner](native-api-normalization-runner.md): optional BYOK OpenAI reference runner for stateless Structured Outputs normalization.
- [Proof-Output Drafting](proof-output-drafting.md): draft-helper workflow for verified proof-output artifacts.
- [Agent Hook Guidance](agent-hook-guidance.md): safe activation and validation hooks.
- [Distribution](distribution.md): Pluxx bundles, release assets, installers, and updates.
- [Skill Evals](skill-evals.md): skill trigger and output-eval fixtures.
- [CLI Usage](../cli/USAGE.md): detailed commands; `mdp --json capabilities` is the machine-readable source of truth.
- [Concepts](../CONCEPTS.md): canonical product, evidence, assurance, and public-safety vocabulary.

The root [README](../README.md) is the product overview. [llms.txt](../llms.txt) and [llms-full.txt](../llms-full.txt) are the curated agent briefings.

## Contributor Workflow Material

`docs/orchid/` is the durable Orchid Relay workspace for public-safe plans,
decisions, reviews, QA evidence, and reusable project history. It is contributor
context, not canonical product documentation. Private, raw, temporary, or bulky
agent artifacts belong in the gitignored `.agent-artifacts/` directory instead.

Do not copy commands or positioning from workflow history into current docs
without checking the implementation, `mdp --json capabilities`, and the latest
release. Obsolete code examples belong in Git history rather than under
`docs/orchid/`.
