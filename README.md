# Message Decision Packs

Message Decision Packs (MDP) are modular, agent-readable GTM messaging packs. They give agents a small manifest, a source ledger, and routed card files for ICP, fit rules, personas, pains, signals, positioning, claims, motions, channel policy, hooks, CTA policy, avoid-rules, objections, gaps, and copy patterns.

This repo contains both the local CLI and the Pluxx source plugin for supported agent hosts:

```text
message-decision-packs/
  cli/      # Rust `mdp` CLI
  plugin/   # Pluxx source plugin with MDP skills, templates, and helper scripts
  docs/     # Project notes and distribution guidance
```

MDP is a decision/context layer. It is not a sender, CRM, sequencer, enrichment provider, scraper, AI SDR, BI tool, or generic automation system.

For a deeper explanation of what this repo is, why it matters, and how to ask your agent to explain it accurately, read [What This Repo Is](docs/what-this-repo-is.md).

## Install

Latest release: [release page](https://github.com/orchidautomation/message-decision-packs/releases/latest)

One-command install:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

For the first-use walkthrough, see [Getting Started](docs/getting-started.md).

Portable shell fallback:

```bash
curl -fsSL https://mdp.orchidlabs.dev/install.sh | bash -s -- --agents -y
```

Copy-paste installers - pick the AI tool you use:

```bash
# Claude Code
curl -fsSL https://github.com/orchidautomation/message-decision-packs/releases/latest/download/install-claude-code.sh | bash

# Cursor
curl -fsSL https://github.com/orchidautomation/message-decision-packs/releases/latest/download/install-cursor.sh | bash

# Codex
curl -fsSL https://github.com/orchidautomation/message-decision-packs/releases/latest/download/install-codex.sh | bash

# OpenCode
curl -fsSL https://github.com/orchidautomation/message-decision-packs/releases/latest/download/install-opencode.sh | bash

# All of the above
curl -fsSL https://github.com/orchidautomation/message-decision-packs/releases/latest/download/install-all.sh | bash
```

The release installers install the plugin bundle and prepare the local `mdp` CLI if it is not already on `PATH`. For noninteractive installs, set `MDP_VERSION`, `MDP_INSTALL_DIR`, or `MDP_DOWNLOAD_URL` before running the installer.

Direct downloads:

- [Claude Code plugin](https://github.com/orchidautomation/message-decision-packs/releases/latest/download/message-decision-packs-claude-code-latest.tar.gz)
- [Cursor plugin](https://github.com/orchidautomation/message-decision-packs/releases/latest/download/message-decision-packs-cursor-latest.tar.gz)
- [Codex plugin](https://github.com/orchidautomation/message-decision-packs/releases/latest/download/message-decision-packs-codex-latest.tar.gz)
- [OpenCode plugin](https://github.com/orchidautomation/message-decision-packs/releases/latest/download/message-decision-packs-opencode-latest.tar.gz)
- [`mdp` for Apple Silicon macOS](https://github.com/orchidautomation/message-decision-packs/releases/latest/download/mdp-aarch64-apple-darwin)
- [`mdp` for Intel macOS](https://github.com/orchidautomation/message-decision-packs/releases/latest/download/mdp-x86_64-apple-darwin)
- [`mdp` for x86_64 Linux](https://github.com/orchidautomation/message-decision-packs/releases/latest/download/mdp-x86_64-unknown-linux-gnu)

## CLI

Build and test:

```bash
cd cli
cargo test
cargo run -- --help
```

Install locally:

```bash
make -C cli install-local
mdp --json doctor --dir .
```

Create a pack:

```bash
mdp --json init --template gtm --name "Example Message Pack" --dir /tmp/mdp-demo --force
mdp --json validate --dir /tmp/mdp-demo
mdp --json --summary route --entries --eval-fixture --dir /tmp/mdp-demo --persona "PMM" --job "linkedin outbound copy"
mdp --json fit --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json
mdp --json --summary brief --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin --out /tmp/mdp-demo/.mdp/briefs/example-linkedin.json
mdp --json check-claims --dir /tmp/mdp-demo --text "MDP is a local offline CLI for modular message context."
mdp --json gaps --dir /tmp/mdp-demo
mdp --json eval --dir /tmp/mdp-demo
```

## Pack Layout

A pack is a local `.mdp/` folder:

```text
.mdp/
  manifest.yaml
  sources.yaml
  briefs/
  cards/personas.yaml
  cards/positioning.yaml
  cards/fit-rules.yaml
  cards/signals.yaml
  cards/pains.yaml
  cards/claims.yaml
  cards/motions.yaml
  cards/channel-policies.yaml
  cards/hooks.yaml
  cards/ctas.yaml
  cards/avoid-rules.yaml
  cards/copy-patterns.yaml
  cards/objections.yaml
  cards/gaps.yaml
  evals/*.yaml
examples/
  clay-row.json
```

Agents should load the manifest first, use `.mdp/sources.yaml` to preserve source facts and interpretations, then only load the cards returned by `mdp route`, `mdp route --entries`, or `mdp brief`. Use `fit` before drafting from a prospect row and stop on `disqualified` or `insufficient-context` unless explicitly overridden. Use `check-claims` before approving copy, `gaps` to expose missing evidence, and `eval` to test route, fit, brief, and claim behavior.

Packs can declare `persona_mappings` in `.mdp/manifest.yaml` so prospect titles map into pack-owned personas before fit and brief routing. Explicit `prospect.persona` still wins. Legacy title fallbacks are reported as low-confidence and do not unlock the fit gate by themselves.

Use `--summary` for compact status output. Use `brief --out <path>` when a brief should be saved; otherwise the CLI marks the artifact as `stdout-only`. Starter `examples/clay-row.json` files are synthetic fixtures and include `source_kind: synthetic-example` plus `synthetic: true`.

## Agent Plugin

The plugin source lives in `plugin/` and includes skills for creating, reviewing, routing, and using MDPs. Pluxx packages that source for supported agent hosts, including Claude Code, Cursor, Codex, and OpenCode. See [pluxx.dev](https://pluxx.dev) and [orchidautomation/pluxx](https://github.com/orchidautomation/pluxx) for the Pluxx project.

Important skills include:

- `mdp-lfg`: master orchestrator for end-to-end MDP work
- `mdp-create-pack`: create or improve a pack
- `mdp-icp-builder`: sharpen ICP/personas/fit logic
- `mdp-source-extract`: turn source material into card entries
- `mdp-message-angles`: codify hooks and angles
- `mdp-cta-builder`: codify CTA and reply-path policy
- `mdp-avoid-rules`: enforce category and claim boundaries
- `mdp-prospect-brief`: turn enriched rows into briefs
- `mdp-copy-brief`: produce model-ready writing contracts
- `mdp-copy-eval`: evaluate generated copy against the pack
- `mdp-pack-review` and `mdp-pack-eval`: QA the pack and routing behavior

## Validation

From the repo root:

```bash
make validate
```

This validates the Rust CLI, the bundled template pack, and, when local Codex validator scripts are available, the plugin and skill metadata.

## Status

This is an MVP local/offline implementation. No auth is required. No hosted API, sending, CRM update, enrichment writeback, scraping, sequencing, or public package release workflow is included.
