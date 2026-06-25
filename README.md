# Message Decision Packs

Message Decision Packs (MDP) are modular, agent-readable GTM messaging packs. They give agents a small manifest, a source ledger, routed card files, and optional extraction prompt contracts for ICP, fit rules, personas, pains, signals, positioning, claims, motions, channel policy, hooks, CTA policy, avoid-rules, objections, gaps, and copy patterns.

This repo contains both the local CLI and the Pluxx source plugin for supported agent hosts:

```text
message-decision-packs/
  cli/      # Rust `mdp` CLI
  plugin/   # Pluxx source plugin with MDP skills, templates, and helper scripts
  docs/     # Project notes and distribution guidance
  examples/ # Canonical public-source example packs
```

MDP is a decision/context layer. It is not a sender, CRM, sequencer, enrichment provider, scraper, AI SDR, BI tool, or generic automation system.

For a deeper explanation of what this repo is, why it matters, and how to ask your agent to explain it accurately, read [What This Repo Is](docs/what-this-repo-is.md). For the conceptual model behind fit, routing, and bounded drafting context, see [Conceptual Decision Flow](docs/conceptual-decision-flow.md).

## Install

Latest release: [release page](https://github.com/orchidautomation/message-decision-packs/releases/latest)

One-command install:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

For the first-use walkthrough, see [Getting Started](docs/getting-started.md).

Canonical example: [Profound GTM Vetting](examples/profound-gtm-vetting/README.md) shows a complete public-source pack for how a company like Profound can codify ICP, target personas, fit rules, hooks, CTAs, guardrails, source-backed claims, prospect brief generation, gaps, and evals before any downstream agent drafts or executes outreach.

The Profound example also includes a runnable [Flue webhook agent scaffold](examples/profound-gtm-vetting/flue-webhook-agent/README.md) that accepts a webhook-style prospect row, writes ignored local scratch, runs `mdp fit` and `mdp brief --context`, and returns a draft contract or model draft without sending or updating external systems.

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
mdp --json --summary brief --context --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin --out /tmp/mdp-demo/.mdp/briefs/example-linkedin.json
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
  prompts/*.yaml
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

## Decision Flow

MDP routes messaging context as a decision tree. The prospect JSON is a provider-neutral normalized row: it can come from a user note, CSV, CRM export, Clay, Deepline, spreadsheet, or research workflow. It supplies the account/person context, including optional fields such as `persona`, `segment`, `signals`, `background`, `source_kind`, and `trigger`. If `persona` is present, MDP uses it; otherwise the CLI infers a persona from the prospect title. The `trigger` is the situational reason to write now, not a card by itself.

```text
prospect.json
  |
  +-- title/persona -> persona
  +-- trigger ------> why now
  +-- segment ------> market/context
  +-- signals ------> evidence or hypotheses
  |
  v
fit gate
  |
  +-- no fit/too thin -> no-draft
  |
  v
persona -> pains -> hooks -> claims/proof -> CTA/channel policy
                              |
                              v
                         avoid rules
                              |
                              v
                      bounded context.entries
```

With `brief --context`, the CLI reads the routed card files locally, selects the relevant entries, and gives the agent those entries first. Whole card paths stay in `context.full_card_required` only when the bounded entry set is not enough.

Agents should load the manifest first, use `.mdp/sources.yaml` to preserve source facts and interpretations, then load only routed context. For prospect briefs, prefer `mdp brief --context` and draft from `data.context.entries`; open `data.context.full_card_required` paths only when present. For route-only work, use cards returned by `mdp route` or `mdp route --entries`. Use `fit` before drafting from a prospect row and stop on `disqualified` or `insufficient-context` unless explicitly overridden. Use `check-claims` before approving copy, `gaps` to expose missing evidence, and `eval` to test route, fit, brief, and claim behavior.

Packs can declare `persona_mappings` in `.mdp/manifest.yaml` so prospect titles map into pack-owned personas before fit and brief routing. Explicit `prospect.persona` still wins. Legacy title fallbacks are reported as low-confidence and do not unlock the fit gate by themselves.

Use `--summary` for compact status output. Use `brief --out <path>` when a brief should be saved; otherwise the CLI marks the artifact as `stdout-only`. Starter `examples/clay-row.json` files are synthetic fixtures kept for compatibility and include `source_kind: synthetic-example` plus `synthetic: true`; the file name is not a requirement to use Clay.

Do not add a separate row-evaluation skill or workflow for fit. Normalize the supplied row into MDP prospect JSON, run `mdp fit`, stop on `disqualified` or `insufficient-context`, and only then run `mdp brief --context` when a brief is needed. True account-only evaluation is a schema/product question for a future provider-neutral account input, not a reason to invent a contact or bypass the prospect fit gate.

Extraction prompt contracts in `.mdp/prompts/*.yaml` define local/offline instructions for classifying supplied person, company, account, domain, row, or research data into strict JSON candidate entries. They use `format: mdp.prompt.v0` and output `contract: mdp.prompt-output.v0` with `card_patches`, `gaps`, `rejected_claims`, confidence, and provenance. They support full ICP extraction, but they do not browse, scrape, enrich, send, or update external systems. See [Prompt Extraction Contract](docs/prompt-extraction-contract.md) and `mdp --json schema prompt`.

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
- `mdp-prospect-brief`: turn provider-neutral prospect/source rows into fit decisions and briefs
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
