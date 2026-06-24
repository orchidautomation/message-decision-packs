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
