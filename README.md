# Message Decision Packs

**Message Decision Packs (MDP) is versioned decision context for agents.**

An MDP pack is a local `.mdp/` directory containing source references, decision
rules, approved proof, routing contracts, output boundaries, gaps, and evals.
The Rust CLI validates that context and returns deterministic, inspectable
decisions. The plugin teaches supported agent hosts how to use those decisions.

MDP is not an agent runtime, CRM, sequencer, enrichment provider, scraper, BI
tool, proposal-management system, or generic automation framework. It does not
collect data, send messages, update external systems, or prove that a source is
true. An optional local BYOK driver can execute one pack-declared model step at
a time; the result still returns to the same validation and receipt boundary.

## Install

Install the CLI and agent bundles:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

Install only the CLI:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y
```

Single-host flags are available for Codex, Claude Code, Cursor, and OpenCode.
The installer uses the latest
[GitHub release](https://github.com/orchidautomation/message-decision-packs/releases/latest)
so the CLI and agent instructions remain version-aligned.

## Quick Start

Create and inspect a synthetic GTM pack:

```bash
mdp --json init --template gtm --dir /tmp/mdp-demo --force
mdp --json validate --dir /tmp/mdp-demo
mdp --json capabilities
mdp --json requirements --dir /tmp/mdp-demo --job prospect-fit-or-brief
mdp --json route --entries \
  --dir /tmp/mdp-demo \
  --persona "PMM" \
  --job "linkedin outbound copy"
```

For a real company or project, identify the target explicitly:

```bash
mdp --json init \
  --template gtm \
  --name "Example Company Messaging" \
  --target-name "Example Company" \
  --target-kind company \
  --dir /tmp/example-company-mdp
mdp --json validate --dir /tmp/example-company-mdp
```

Generated prospects and sampled leads are synthetic and marked
`do_not_contact`. MDP should return `disqualified` or `insufficient-context`
when evidence is too weak for drafting.

See [Getting Started](docs/getting-started.md) for fit, brief, claim-checking,
proposal, and first-run workflows.

## How It Works

A pack keeps human-readable orientation next to structured authority:

```text
.mdp/
  manifest.yaml       # profile, jobs, policies, and routing contracts
  sources.yaml        # reviewed source registry
  prompts/*.yaml      # optional model-step contracts
  cards/*.yaml        # modular decisions, evidence, and boundaries
  evals/*.yaml        # pack checks
  briefs/             # generated local review artifacts
  traces/             # optional decision-trace views
```

The normal flow is:

1. Discover the CLI contract with `mdp --json capabilities`.
2. Compile exact job requirements before collecting or inspecting context.
3. Validate supplied inputs and source lineage.
4. Run deterministic fit and routing gates.
5. Draft only from routed brief context.
6. Check claims and preserve the resulting receipts.

Agents should load the manifest first and use routed entries instead of reading
every card. A pack README is secondary navigation; it is not decision authority.
See [Concepts](CONCEPTS.md) for the canonical vocabulary and assurance states.

## Examples

- [Eve on Vercel](examples/ai-sdr-eve-vercel/README.md) is the canonical
  runnable integration. Eve provides the runtime; MDP provides local decision
  context and gates. The example does not send outreach or mutate a CRM.
- [Clay Audiences](examples/clay-audiences-self-serve-enterprise-expansion/README.md)
  is a synthetic reference pack for attempted-complete inputs and source
  lineage.
- Other directories under `examples/` are synthetic contract or test fixtures.
  They are not alternate hosted products or recommended runtime architectures.

## Repository Layout

```text
cli/      Rust `mdp` CLI
plugin/   canonical plugin source, skills, templates, and hooks
docs/     user and maintainer documentation
examples/ synthetic integrations and contract fixtures
scripts/  validation, packaging, and compatibility tooling
```

`plugin/skills/` is the authored skill source. Pluxx packages that source for
supported agent hosts; it is the distribution layer, not the MDP runtime or a
hosted MDP service.

## Documentation

- [Getting Started](docs/getting-started.md)
- [Pack Authoring](docs/pack-authoring.md)
- [Decision Input Contracts](docs/decision-input-contracts.md)
- [Minimal Context Routing](docs/minimal-context-routing.md)
- [Decision Traces](docs/decision-traces.md)
- [Local MCP](docs/local-mcp.md)
- [Native Model Runner](docs/native-api-normalization-runner.md)
- [Distribution](docs/distribution.md)
- [CLI Usage](cli/USAGE.md)

Agents can use [llms.txt](llms.txt) for a short briefing or
[llms-full.txt](llms-full.txt) for fuller operating context.

## Validation

From the repository root:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- \
  --json validate --dir plugin/assets/templates/basic
make validate
```

The Eve integration has its own checks:

```bash
cd examples/ai-sdr-eve-vercel
npm ci
npm run check
```

## License

This source-available repository uses the [Elastic License 2.0](LICENSE).
Local/offline and internal use are allowed under its terms. Offering a hosted
or managed service that exposes a substantial set of MDP functionality requires
a separate commercial license; see [Commercial Use](COMMERCIAL.md).
