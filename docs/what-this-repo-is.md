# What This Repo Is

Message Decision Packs, or MDP, is a local standard, CLI, and agent plugin system for keeping GTM messaging context structured, reviewable, and reusable.

The short version:

```text
MDP turns "paste a bunch of context and ask an AI to write GTM copy" into a local, testable decision system.
```

It gives AI coding agents a shared source of truth for:

- who the ICP is
- what personas matter
- what counts as good fit
- what signals are meaningful
- what pains and triggers matter
- what claims are approved
- what claims are not allowed
- what CTAs are acceptable
- what channel rules apply
- what output style and structure rules apply
- what gaps should be surfaced instead of invented

MDP is not the system that sends messages. It is the decision layer that tells an agent what context to load, what it is allowed to say, and when it should stop.

## Why This Exists

The common GTM AI workflow is fragile:

```text
Paste product context.
Paste prospect context.
Ask for a message.
Correct the claims manually.
Repeat the same work in the next thread.
```

That works for demos. It breaks when multiple people, agents, prompts, or tools are involved.

The failure modes are predictable:

- every thread rebuilds the source of truth from scratch
- claims drift over time
- weak prospect signals get treated as facts
- fit logic lives in someone's head
- CTAs change depending on who prompted the model
- unsupported proof gets smoothed into confident copy
- gaps disappear because the model wants to be helpful
- review becomes manual and inconsistent

MDP fixes that by moving the judgment into local files and CLI contracts.

Instead of asking the model to remember the messaging system, the repo gives the model a structured operating layer:

```text
Create a pack.
Store decisions in cards.
Route only the cards needed for the job.
Check fit before drafting.
Check claims before approval.
Keep gaps explicit.
Run evals when behavior changes.
```

The point is not just better copy. The point is making agent-assisted GTM work more inspectable, repeatable, and safer.

## Why It Matters

MDP matters because AI agents are very good at producing polished output from thin context.

That is useful when the source material is strong. It is dangerous when the source material is weak.

A Message Decision Pack gives the agent a quality floor:

- The source ledger separates source facts from interpretation.
- Fit rules stop the agent before it drafts for a bad or thin prospect.
- Approved claims tell the agent what it may say.
- Avoid rules tell the agent what it must not say.
- Output rules tell the agent how generated text must be structured and styled.
- Gaps preserve missing evidence instead of hiding it.
- Evals make routing and behavior testable.
- Briefs create a durable handoff between decision context and drafting.

This is the practical operator value:

```text
The agent can move faster without turning GTM judgment into vibes.
```

## This Repo Is For Everyone Because Of Pluxx

This repo is not tied to one agent host.

The core is host-agnostic:

- `.mdp/` packs are local files.
- `mdp` is a local CLI with JSON contracts.
- the pack schema is agent-readable.
- the workflow does not require a hosted API.
- the same pack can be inspected, validated, routed, and tested outside any single AI host.

Pluxx is what makes the agent layer portable.

This repo uses `pluxx.config.ts` to build and distribute agent bundles for:

- Claude Code
- Cursor
- Codex
- OpenCode

Learn more about Pluxx at [pluxx.dev](https://pluxx.dev) or the [orchidautomation/pluxx](https://github.com/orchidautomation/pluxx) GitHub repo.

That matters because the product should not depend on one agent host winning.

The durable thing is the pack:

```text
.mdp/manifest.yaml
.mdp/sources.yaml
.mdp/cards/*.yaml
.mdp/evals/*.yaml
```

The CLI enforces the contracts:

```text
mdp validate
mdp route
mdp fit
mdp brief
mdp check-claims
mdp gaps
mdp eval
```

Pluxx translates the agent-facing skills and plugin package into the host-specific shapes people actually use. The goal is not fake parity across every host. The goal is to preserve the same intent and expose the best honest equivalent in each agent environment.

The honest framing is:

```text
MDP is the standard and local runtime.
Pluxx is the distribution layer that makes it usable across agent hosts.
```

That is why this repo can serve Claude Code users, Cursor users, Codex users, and OpenCode users without rewriting the whole workflow for each tool.

## What Is In The Repo

The repo has three main parts:

```text
message-decision-packs/
  cli/      # Rust mdp CLI
  plugin/   # Pluxx source plugin with skills, templates, scripts, and assets
  docs/     # design notes, install notes, and user journey docs
```

### `cli/`

The Rust CLI is the local runtime.

It owns:

- pack initialization
- pack validation
- setup health checks
- routing logic
- prospect fit checks
- claim checks
- gap listing
- eval fixtures
- brief generation
- portable pack hashes
- stable JSON output for agents and scripts

The important commands are:

```bash
mdp --json doctor --dir .
mdp --json validate --dir .
mdp --json --summary route --entries --dir . --persona "PMM" --job "linkedin outbound copy"
mdp --json fit --dir . --prospect <prospect.json>
mdp --json --summary brief --dir . --prospect <prospect.json> --channel linkedin
mdp --json check-claims --dir . --text "<draft copy>"
mdp --json gaps --dir .
mdp --json eval --dir .
```

### `plugin/`

The plugin is the agent-facing workflow layer.

It contains skills that teach supported agents how to create, inspect, improve, route, and use Message Decision Packs.

Important skills include:

- `mdp-lfg`: orchestrates multi-step MDP work
- `mdp-create-pack`: creates or improves a pack
- `mdp-icp-builder`: sharpens ICP, personas, and fit logic
- `mdp-source-extract`: turns source material into card entries
- `mdp-message-angles`: codifies hooks and angles
- `mdp-cta-builder`: codifies CTA and reply-path policy
- `mdp-avoid-rules`: enforces category and claim boundaries
- `mdp-output-rules`: codifies global style and structure constraints
- `mdp-prospect-brief`: turns provider-neutral prospect/source rows into fit decisions and briefs
- `mdp-copy-brief`: produces model-ready writing contracts
- `mdp-copy-eval`: evaluates generated copy against the pack
- `mdp-pack-review`: reviews pack quality
- `mdp-pack-eval`: tests routing and pack behavior

### `plugin/assets/templates/basic`

This is the neutral starter pack.

It shows the intended `.mdp/` structure:

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
  cards/output-rules.yaml
  cards/copy-patterns.yaml
  cards/objections.yaml
  cards/gaps.yaml
  evals/*.yaml
examples/
  clay-row.json
```

The example prospect row is synthetic and kept at `examples/clay-row.json` for compatibility with starter scripts and tests. It is only there to test the workflow. Real or sensitive prospect data should live in ignored scratch unless someone intentionally commits a sanitized example. Clay, CSV, CRM export, spreadsheet, Deepline, or user-provided notes are all just possible sources for the same normalized MDP prospect JSON.

## How The Workflow Works

A normal MDP workflow looks like this:

1. Install the CLI and agent bundle.
2. Create or open a `.mdp/` pack.
3. Add source material to `.mdp/sources.yaml`.
4. Fill the cards with personas, fit rules, claims, avoid rules, output rules, CTAs, and gaps.
5. Validate the pack.
6. Route a persona and job to the minimum required cards.
7. Use the pack-owned normalization prompt when needed to convert a messy prospect/source row into the expected prospect JSON shape.
8. Run fit before drafting.
9. Generate a brief only when fit is acceptable.
10. Draft from the brief and routed cards.
11. Run claim checks before approval.
12. Add evals when behavior should become repeatable.

The key design choice is progressive disclosure.

The agent should not load the whole pack by default. It should load the manifest first, then use the CLI route or brief output to decide which card files matter for the current job.

That keeps the context smaller and easier to audit.

## What A Pack Actually Stores

A pack stores messaging decisions.

Examples:

- "This persona cares about source-of-truth drift."
- "This trigger matters only if there is a current project, hiring signal, or tool migration."
- "This claim is approved because it is backed by source X."
- "Do not say this integrates with Salesforce unless that claim is explicitly approved."
- "Do not draft if the prospect row has no trigger, persona, segment, signal, or source."
- "For LinkedIn, use a low-friction compare-notes CTA instead of a hard demo ask."
- "If the evidence is missing, put it in gaps instead of making it sound true."

That is the distinction:

```text
The pack stores judgment and prompt contracts.
Upstream agents use prompt contracts to normalize messy data.
The CLI turns reviewed judgment into decision contracts.
The agent uses those contracts to decide what to load and what to do next.
```

There should not be a separate row-evaluation skill that reimplements fit. The row path is: normalize supplied row-like context into MDP prospect JSON, preferably through `.mdp/prompts/normalize-prospect.yaml`; run `mdp fit`; stop on a no-draft decision; and use `mdp brief --context` only after fit allows a message brief.

## What MDP Is Not

Do not describe MDP as:

- an AI SDR
- a CRM
- a sequencer
- a lead enrichment tool
- a scraper
- a BI tool
- a generic automation system
- a tool that sends emails or LinkedIn messages
- a tool that updates Salesforce, HubSpot, Clay, or a sequencer

Those actions can exist in a separate toolchain, but they are outside MDP.

MDP stops at:

- pack creation
- validation
- routing
- fit checks
- claim checks
- gaps
- evals
- briefs
- local/demo copy

That boundary is important. It keeps the system inspectable and prevents a messaging context repo from pretending to be execution infrastructure.

## How To Explain It To A Friend

Use this version:

```text
This repo is a local standard and CLI for turning GTM messaging strategy into structured files that AI agents can safely use.

Instead of pasting the same product, ICP, claim, and CTA context into every AI thread, you put that judgment into a Message Decision Pack. The CLI validates it, routes only the relevant cards for a task, checks whether a prospect has enough context, generates a brief, and checks draft copy for unsupported claims.

It is not a sender, CRM, sequencer, scraper, or AI SDR. It is the decision layer before those systems.

Because the repo uses Pluxx, the same agent workflow can be distributed to Claude Code, Cursor, Codex, and OpenCode. The durable asset is the local pack and CLI contract, not one specific AI host. Pluxx lives at https://pluxx.dev and https://github.com/orchidautomation/pluxx.
```

## Prompt To Give Your Agent

If someone has this repo open in a supported agent host and wants an accurate explanation, give the agent this:

```text
Please explain this repo in depth for someone new to it.

Read these first:
- README.md
- docs/what-this-repo-is.md
- docs/getting-started.md
- cli/USAGE.md
- cli/src/cli.rs
- cli/src/commands/routing.rs
- cli/src/commands/briefs.rs
- plugin/skills/mdp/SKILL.md
- plugin/assets/templates/basic/.mdp/manifest.yaml
- pluxx.config.ts

Explain:
1. What Message Decision Packs are.
2. Why this repo exists.
3. How the local mdp CLI and .mdp pack files work.
4. How agents are supposed to use route, fit, brief, check-claims, gaps, and eval.
5. Why Pluxx matters and why this repo is for users across supported agent hosts, including Claude Code, Cursor, Codex, and OpenCode.
6. What MDP is not.
7. The first workflow a new user should try.

Do not describe MDP as an AI SDR, CRM, sequencer, enrichment provider, scraper, BI tool, or generic automation system.
Do not imply it sends messages, updates CRM, scrapes data, enriches leads, or has a hosted API.
Keep the explanation practical and grounded in the files in this repo.
```

## The One-Line Product Story

```text
MDP turns GTM prompting into a local, testable decision system that can travel across agent hosts.
```
