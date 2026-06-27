# AGENTS.md

## Project Truth

This repo contains Message Decision Packs (MDP): a local/offline standard, CLI, and Codex plugin for modular GTM messaging context.

MDP stores decision context and routing contracts. It is not execution infrastructure.

Do not describe MDP as:

- AI SDR
- CRM
- sequencer
- enrichment provider
- scraper
- BI tool
- generic automation system

## Repo Shape

- `cli/`: Rust `mdp` CLI.
- `plugin/`: Codex plugin and skills.
- `plugin/assets/templates/basic`: neutral example pack.
- `docs/`: distribution and design notes.

## Linear Coordination

- Default Linear team: `MDP`.
- Team overview: https://linear.app/orchid-automation/team/MDP/overview
- Issue prefix: `MDP`.

Use the MDP Linear team for this repo's bugs, feature work, planning, and closeout. When work is tied to a Linear issue, include the issue ID in branch names, PR titles/descriptions, and relevant comments, for example `MDP-123`.

This is a public-facing OSS repo. Keep public GitHub references limited to issue identifiers, public links, and sanitized summaries; do not expose private Linear discussion details, customer data, raw transcripts, tokens, or local-only paths in commits, PRs, or public docs.

## Feature Change Hygiene

When adding, changing, or removing MDP behavior, update the matching agent-facing skill instructions in `plugin/skills/` in the same change. The CLI contract, starter/template pack, docs, and skills should not drift apart.

## Validation

Prefer narrow checks first:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
make validate
```

If local Codex plugin validators are unavailable, say that plugin validation was skipped and still run the CLI/template checks.

## Safety

- Do not commit secrets, private customer data, browser sessions, tokens, raw transcripts, or local auth files.
- Do not publish packages, create public repos, or push public releases without explicit approval.
- Keep generated scratch under `.agent-artifacts/` and do not commit it.
