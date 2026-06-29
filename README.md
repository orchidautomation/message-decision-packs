# Message Decision Packs

Message Decision Packs (MDP) are modular, agent-readable GTM messaging packs. They give agents a small manifest, a source ledger, routed card files, and optional extraction prompt contracts for ICP, fit rules, personas, pains, signals, positioning, claims, motions, channel policy, hooks, CTA policy, avoid-rules, output-rules, objections, gaps, and copy patterns.

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

Canonical examples:

- [MDP for MDP](examples/mdp-for-mdp/README.md) is the fastest public demo for explaining what a pack stores, how messy source rows become normalized context, why no-draft states matter, and how route, fit, brief, claim-check, gaps, and eval commands work together.
- [Profound GTM Vetting](examples/profound-gtm-vetting/README.md) shows a complete public-source pack for how a company like Profound can codify ICP, target personas, fit rules, hooks, CTAs, guardrails, output rules, source-backed claims, prospect brief generation, gaps, and evals before any downstream agent drafts or executes outreach.

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
mdp --json capabilities
mdp --json init --template gtm --name "Example Message Pack" --dir /tmp/mdp-demo --dry-run
mdp --json init --template gtm --name "Example Message Pack" --dir /tmp/mdp-demo --force
mdp --json validate --dir /tmp/mdp-demo
mdp --json --summary route --entries --eval-fixture --dir /tmp/mdp-demo --persona "PMM" --job "linkedin outbound copy"
mdp sample-leads --dir /tmp/mdp-demo --persona "PMM" --job "initial email outbound copy" --count 3 --format yaml
mdp --json fit --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json
mdp --json --summary brief --context --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin --out /tmp/mdp-demo/.mdp/briefs/example-linkedin.json
mdp --json check-claims --dir /tmp/mdp-demo --text "MDP is a local offline CLI for modular message context."
mdp --json check-claims --dir /tmp/mdp-demo --text "<draft copy>" --subject "<subject>" --persona "PMM" --job "initial email outbound message"
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
  cards/output-rules.yaml
  cards/copy-patterns.yaml
  cards/objections.yaml
  cards/gaps.yaml
  evals/*.yaml
examples/
  clay-row.json
```

## Decision Flow

MDP routes messaging context as a decision tree. The prospect JSON is a provider-neutral normalized row: it can come from a user note, CSV, CRM export, Clay, Deepline, spreadsheet, or research workflow. Packs now include a runtime normalization prompt contract, `.mdp/prompts/normalize-prospect.yaml`, so upstream agents can turn messy source rows into the exact prospect JSON shape the CLI ingests. The CLI still owns the deterministic fit, route, brief, and claim-check decisions.

The prospect JSON supplies the account/person context, including optional fields such as `persona`, `segment`, `signals`, `background`, `source_kind`, and `trigger`. If `persona` is present, MDP uses it; otherwise the CLI infers a persona from pack-owned title mappings. The `trigger` is the situational reason to write now, not a card by itself.

```text
messy source row
  |
  v
.mdp/prompts/normalize-prospect.yaml
  |
  v
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
                         output rules
                              |
                              v
                      bounded context.entries
```

With `brief --context`, the CLI reads the routed card files locally, selects the relevant entries, and gives the agent those entries first. Whole card paths stay in `context.full_card_required` only when the bounded entry set is not enough.

## Channel Rule Taxonomy

Keep outbound rules split by responsibility:

- `channel-policies.yaml`: channel and lifecycle policy, such as email initial, email follow-up, LinkedIn initial touch, LinkedIn follow-up, call prep, and agent briefs.
- `output-rules.yaml`: global text and formatting constraints, such as plain text by default, no links/HTML/tracking by default, no fake personalization, initial email 90-125 word guidance, subject guidance, paragraph counts, and no meta commentary.
- `ctas.yaml`: ask boundaries and reply paths, including soft asks, calendar-second policy, owner-routing questions, and false-urgency limits.
- `copy-patterns.yaml`: reusable message structures, such as trigger or hypothesis -> proof gap -> approved angle -> one soft CTA.

Use human-readable `body` text for these policies. Use `avoid` terms when a literal should be caught by guardrail checks, `exact_paragraphs` when a fixed paragraph count is truly required, and `constraints` for deterministic output checks such as word count, subject word count, subject avoid literals, max questions, and forbidden links, attachments, images, HTML, or tracking.

Agents should load the manifest first, use `.mdp/sources.yaml` to preserve source facts and interpretations, then load only routed context. Agents and wrappers can run `mdp --json capabilities` to inspect command contracts, coarse side effects, dry-run support, strict-mode support, and stable JSON error codes before driving the CLI. For prospect briefs, prefer `mdp brief --context` and draft from `data.context.entries`; open `data.context.full_card_required` paths only when present. For route-only work, use cards returned by `mdp route` or `mdp route --entries`. Routed entries can include structured `constraints` for deterministic output checks such as word count, subject word count, subject avoid literals, max questions, and forbidden links, attachments, images, HTML, or tracking. Use `fit` before drafting from a prospect row and stop on `disqualified` or `insufficient-context` unless explicitly overridden. Use `check-claims` before approving copy to catch unsupported claims, avoid-rule hits, output-rule hits, hard constraint violations in `guardrail_hits`, advisory target misses in `constraint_warnings`, and text-only limitations in `unchecked_constraints`; add `--strict` when advisory warnings should fail an agent or CI gate. Use `gaps` to expose missing evidence, and use `eval` to test route, fit, brief, and claim behavior.

For outbound-copy testing when no real or intentionally sanitized prospect row exists, generate clearly fake fixtures first:

```bash
mdp sample-leads --dir . --persona "PMM" --job "initial email outbound copy" --count 3 --format yaml
```

`sample-leads` emits 2 to 5 deterministic synthetic example fixture rows with `source_kind: synthetic-example`, `synthetic: true`, `do_not_contact: true`, route context, `safe_personalization`, and `known_gaps`. Save one fixture row to ignored scratch if you need to pass it to `mdp fit` or `mdp brief --context`, route each fixture through MDP, draft only against the safe personalization and stated gaps, then run `check-claims`. Never enrich, research, upload, sequence, contact, or treat these fixture leads as real prospects.

## Extensions

Pack authors can add advisory custom annotations to card entries with `metadata`:

```yaml
entries:
  - id: linkedin-initial-touch
    title: LinkedIn initial touch
    body: Keep the opener short and use one sourced trigger.
    applies_to: [PMM]
    evidence: [README.md]
    metadata:
      owner: pmm
      review_status: draft
      source_priority: 2
```

`metadata` is preserved in `mdp route --entries` and `mdp brief --context` output so agents can see it. The CLI does not enforce unknown metadata keys. Put enforceable rules in first-class fields such as `avoid`, `exact_paragraphs`, fit rules, claims, channel policies, or output rules.

Arbitrary fields outside the supported schema are not extension points. Serde may still parse those YAML files, but `mdp validate` warns that unsupported fields are ignored. Move advisory custom data under entry `metadata` instead.

Channels are open strings. Add custom channels to `manifest.yaml` `supported_channels`, then write matching channel-policy entries and route with jobs or brief channels that use the same words. For example, `supported_channels: [linkedin, email, partner-intro]` lets `partner intro outbound message` match a `Partner intro` channel-policy entry without adding a new Rust enum variant.

Packs can declare `persona_mappings` in `.mdp/manifest.yaml` so prospect titles and direct persona/job commands map into pack-owned personas before fit, route, sample lead generation, and brief routing. Outputs include `requested_persona` and `persona_resolution` when an alias is resolved so agents can see that `Growth Engineer` routed as the canonical pack persona. Legacy title fallbacks are reported as low-confidence and do not unlock the fit gate by themselves.

`mdp fit` currently owns the binary fit gate: `fit`, `disqualified`, or `insufficient-context`. A future first-class qualification scorecard should be additive to the `mdp.fit.v0` contract, for example an optional `qualification_stage` or `scorecard` object derived from fit-rule entries, matched evidence, missing context, and disqualifiers. Do not overload card `metadata` or agent skills as a parallel scoring system; keep qualification scoring behind an explicit CLI/schema extension so route, brief, eval, and downstream agents can test it consistently.

Use `--summary` for compact status output. Use `--dry-run` before selected write commands when an agent needs to preview local file writes: `init`, `brief --out`, `emit-brief --out`, and `pack --out`. Use `brief --out <path>` when a brief should be saved; otherwise the CLI marks the artifact as `stdout-only`. Starter `examples/clay-row.json` files are synthetic fixtures kept for compatibility and include `source_kind: synthetic-example` plus `synthetic: true`; the file name is not a requirement to use Clay. `mdp sample-leads` creates additional fake testing rows with the same synthetic-example provenance only; it does not generate real leads, enrich data, browse the web, write to a CRM, or imply any person or account exists.

Do not add a separate row-evaluation skill or workflow for fit. Normalize the supplied row into MDP prospect JSON, run `mdp fit`, stop on `disqualified` or `insufficient-context`, and only then run `mdp brief --context` when a brief is needed. True account-only evaluation is a schema/product question for a future provider-neutral account input, not a reason to invent a contact or bypass the prospect fit gate.

Prompt contracts in `.mdp/prompts/*.yaml` define local/offline instructions for two related jobs:

- Runtime normalization prompts, such as `normalize-prospect.yaml`, turn messy supplied rows into `normalized_prospect` JSON plus a trace that can feed `mdp fit` and `mdp brief`.
- Extraction prompts classify supplied person, company, account, domain, row, or research data into reviewable `card_patches`, `gaps`, `rejected_claims`, confidence, and provenance for pack authors.

Both use `format: mdp.prompt.v0` and output `contract: mdp.prompt-output.v0`. Each prompt carries `output_contract.schema_ref`, a compact reference to its JSON output contract, plus a safe example. Use `mdp init --include-output-schemas` when you need starter prompt files with full inline JSON Schemas under `output_contract.schema`. They do not browse, scrape, enrich, send, sequence, or update external systems. See [Prompt Extraction Contract](docs/prompt-extraction-contract.md) and `mdp --json schema prompt`.

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
- `mdp-output-rules`: codify global style, punctuation, formatting, and structure constraints
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
