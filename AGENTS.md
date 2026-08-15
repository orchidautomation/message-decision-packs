# MDP Agent Guide

## Product Boundary

Message Decision Packs (MDP) is a local/offline standard, Rust CLI, and Codex
plugin for modular GTM messaging context. It stores decision context and routing
contracts. It is not a CRM, sequencer, enrichment provider, scraper, BI tool,
AI SDR, or generic automation system.

Repository layout:

- `cli/`: Rust CLI.
- `plugin/`: canonical plugin and authored skills.
- `plugin/assets/templates/`: starter packs.
- `docs/`: design and distribution documentation.

## Working Rules

- Preserve existing work. Do not revert unrelated changes.
- Default to one root agent and direct implementation. Use Compound Engineering
  or subagents only when Brandon explicitly asks or the change involves auth,
  payments, data migration, production mutation, or major architecture.
- Use a branch for substantive changes. When work is tied to Linear, include the
  MDP issue ID in the branch name or PR title; do not repeat it ceremonially.
- Update only affected surfaces. Change skills, templates, or docs when their
  user-facing contract or guidance actually changes.
- `plugin/skills/` is the only authored skill source. Pluxx owns generated host
  bundles.
- Run targeted checks while implementing. Treat green full CI for an exact
  commit as authoritative; do not repeat the same full validation without a
  changed tree, missing coverage, or a concrete failure to diagnose.

## Shipping

- One requested change should produce at most one PR.
- A merge completes the coding task unless Brandon explicitly requests a
  release or the original PR carries explicit release intent.
- Do not create a separate release-only PR by default. A release-worthy feature
  PR should include its version bump before merge.
- Release CI owns packaging and published-installer smoke testing. Do not
  reinstall the local CLI or agent bundles unless Brandon asks for a local
  upgrade or the change is specifically host-dependent.
- Do not automatically babysit, monitor, or repair a PR after handoff unless
  Brandon asks or a required check reports a concrete failure.
- Do not add AI autofix labels unless Brandon explicitly asks for automated
  same-branch repair on that PR.
- The release runbook lives in `docs/distribution.md`; keep procedural detail
  there instead of duplicating it here.

## Safety

- Never commit secrets, customer data, private documents, transcripts, browser
  state, tokens, cookies, auth files, or local-only paths.
- Public examples and fixtures must be synthetic, generic, or explicitly
  sanitized. Never invent proof, certifications, compliance status, or past
  performance.
- Do not publish, deploy, merge, or mutate external systems without explicit
  authorization for that action.
- Keep agent-authored diagnostic artifacts under `.agent-artifacts/` and do not
  commit them. Normal tool-managed build and cache directories are exempt.
