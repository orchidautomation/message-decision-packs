# MDP Documentation

These are the current public docs:

- [Getting Started](getting-started.md): installation and first use.
- [Conceptual Decision Flow](conceptual-decision-flow.md): pack layers, routing, fit, briefs, and drafting boundaries.
- [Portfolio-Aware GTM Scope](portfolio-scope.md): scope existing primitives by product, capability, solution, or segment within one pack.
- [Prompt Contracts](prompt-extraction-contract.md): normalization and extraction schemas.
- [Runner Receipts](run-receipts.md): context-isolation receipt contract for audit-grade proposal workflows.
- [Local Proposal Runner Surface](proposal-runner.md): host-neutral local command surface for staged source audit, native/headless normalization, validation, receipts, and review probes.
- [Headless Normalization Runners](headless-normalization-runners.md): native/headless runner recipes for Codex, Claude Code, Cursor, OpenCode, and the bundled local stdio MCP wrapper.
- [Native API Normalization Runner](native-api-normalization-runner.md): optional BYOK OpenAI reference runner for stateless Structured Outputs normalization.
- [Proof-Output Drafting](proof-output-drafting.md): draft-helper workflow for verified proof-output artifacts.
- [Proposal Flow Video Demo](../examples/proposal-flow-video/README.md): runnable synthetic walkthrough from messy proposal sources to verified review artifacts.
- [Agent Hook Guidance](agent-hook-guidance.md): safe activation and validation hooks.
- [Distribution](distribution.md): Pluxx bundles, release assets, installers, and updates.
- [Skill Evals](skill-evals.md): skill trigger and output-eval fixtures.
- [CLI Usage](../cli/USAGE.md): detailed commands; `mdp --json capabilities` is the machine-readable source of truth.

The root [README](../README.md) is the product overview. [llms.txt](../llms.txt) and [llms-full.txt](../llms-full.txt) are the curated agent briefings.

## Historical Material

`docs/orchid/` contains durable planning, decision, QA, and review artifacts. `docs/plans/` contains shipped implementation plans from the earlier repository layout. Historical narrative and evaluation files carry an explicit banner and are not canonical product documentation.

Do not copy stale commands or positioning from historical material into current docs without checking the implementation, `mdp --json capabilities`, and the latest release.
