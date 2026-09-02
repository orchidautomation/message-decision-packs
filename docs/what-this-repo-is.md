# What This Repo Is

Message Decision Packs (MDP) is a local standard for giving agents versioned,
reviewable decision context. A pack records what evidence was reviewed, which
rules and claims are approved, how a job selects context, what output is
allowed, and which gaps must remain explicit.

This repository is the reference implementation and distribution source for
that standard. It contains the `.mdp` pack contracts, the local Rust CLI, the
canonical agent plugin and skills, synthetic contract fixtures, and the
configuration used to package release artifacts.

MDP is a decision layer, not an external-action system. It does not collect
data, enrich leads, send messages, update a CRM, submit proposals, or own an
approval workflow.

## The parts of MDP

### The standard and `.mdp` packs

A Message Decision Pack is a local `.mdp/` directory. Its human-readable YAML
and generated JSON artifacts describe source lineage, decision entries,
routing jobs, output contracts, gaps, and evals. GTM and proposal review are
profiles over the same domain-agnostic primitives rather than separate
engines.

The manifest is the entry point. Agents and integrations should compile an
exact job and load the routed entries instead of treating every file in the
pack as prompt context. See [Concepts](../CONCEPTS.md) for the canonical
vocabulary and [Conceptual Decision Flow](conceptual-decision-flow.md) for how
the pieces compose.

### The Rust CLI

The local `mdp` CLI is the contract authority. It initializes and validates
packs, exposes capabilities and job requirements, resolves skills and routed
context, evaluates deterministic gates, produces briefs and traces, checks
claims and structured output, and verifies run artifacts. It emits stable JSON
contracts for agents and integrations.

The command surface changes as contracts evolve, so this explainer does not
copy a command inventory. Use:

```bash
mdp --json capabilities
```

Then consult [CLI Usage](../cli/USAGE.md) for task-specific examples.

MDP also includes an optional local BYOK driver for one pack-declared model
step at a time. That bounded subprocess returns to the same validation and
receipt boundary; it does not turn the CLI into an agent runtime or workflow
orchestrator.

### The canonical plugin and skills

`plugin/` is the authored agent-facing source. Its four public skills cover
the stable operator jobs:

- `mdp` for CLI help and mixed MDP coordination;
- `mdp-pack-builder` for evidence-grounded pack creation and improvement;
- `mdp-pack-review` for reviewing pack structure and decision quality; and
- `mdp-pack-apply` for applying a CLI-selected job to supplied inputs.

`plugin/skill-inventory.json` records that public inventory, and
`plugin/skills/` is the only authored skill tree. The CLI decides whether a
skill is eligible for an exact pack job; host discovery alone does not.

### Pluxx distribution

[Pluxx](https://github.com/orchidautomation/pluxx) compiles the canonical
plugin source into the portable skills package and host-native bundles shipped
with MDP releases. Pluxx is the distribution layer, not the MDP standard, CLI,
or a hosted MDP service.

Package format, installation, native enhancements, and host-specific evidence
are separate claims. Refer to [Distribution](distribution.md) for the current
release contract and compatibility matrix rather than inferring support from a
generated archive or copied files.

### External runtimes

Customer-controlled hosts and integrations own model/provider execution,
connectors, credentials, collection, batching, retries, sequencing, and
external side effects. They may call MDP contracts, but they do not gain
decision authority merely by transporting or executing them. MDP owns the
pack-derived decision and validation boundary; the external runtime owns what
happens outside it.

## Repository layout

```text
cli/      Rust CLI and its schemas, contracts, and tests
plugin/   canonical plugin source, skills, templates, hooks, and assets
docs/     user and maintainer documentation
examples/ synthetic reference packs and conformance fixtures
scripts/  validation, packaging, installer, and compatibility tooling
deploy/   source configuration for the small release-asset redirect service
```

The contract fixtures are deliberately synthetic. The Clay Audiences example
is the reference pack for attempted-complete inputs and source lineage; other
examples primarily exercise conformance and regression behavior. They are not
hosted applications or recommended runtime architectures.

Root `assets/` mirrors canonical plugin assets for validation and packaging.
Maintainers author skills only under `plugin/skills/`; generated host bundles
and installed copies are outputs, not additional sources of truth.

## Boundaries

MDP is not a CRM, sequencer, enrichment provider, scraper, BI tool, AI SDR,
proposal-management system, graph database, persistent memory layer, agent
runtime, or general orchestration framework. Hashes, traces, and receipts can
show contract consistency and the boundary a run enforced; they do not prove
that a source is true or authorize an external action.

The repository is source-available under the [Elastic License 2.0](../LICENSE).
See [Commercial Use](../COMMERCIAL.md) for the hosted or managed-service
boundary.

## How this document fits the docs

This page is the durable product-and-repository map: it answers what MDP is,
how its maintained parts relate, and where its runtime boundary ends.

- The root [README](../README.md) is the short product overview and install
  entry point.
- [Getting Started](getting-started.md) is the first-use guide.
- [Concepts](../CONCEPTS.md) is the canonical vocabulary and assurance model.
- [Distribution](distribution.md) owns current packaging, installation, and
  compatibility details.
- [`llms.txt`](../llms.txt) and [`llms-full.txt`](../llms-full.txt) are curated
  agent briefings shipped with releases.

This Markdown file is repository documentation, not a release asset or vanity
URL. The `mdp.orchidlabs.dev` redirect configuration serves the installer and
the two `llms` briefings; it does not serve this page. Read it through the
repository or rendered GitHub documentation.
