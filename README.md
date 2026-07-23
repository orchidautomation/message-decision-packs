# Message Decision Packs

Message Decision Packs (MDP) make agent judgment explicit. A local `.mdp/` folder stores source evidence, decision rules, approved claims or proof, routing contracts, output boundaries, gaps, and evals. The Rust CLI validates and routes that context; the plugin teaches supported agents how to use it.

MDP is a decision/context layer. It is not an AI SDR, CRM, sequencer, enrichment provider, scraper, BI tool, proposal management system, or generic automation platform. It does not send messages or update external systems.

```text
message-decision-packs/
  cli/      # Rust `mdp` CLI
  plugin/   # Pluxx plugin package source: skills, assets, hooks, scripts
  docs/     # Current user and maintainer documentation
  examples/ # One canonical runnable example: Eve on Vercel
```

## Install

Install the CLI and bundles for supported agent hosts:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

Install only the CLI:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y
```

Single-host flags are also available: `--codex`, `--claude-code`, `--cursor`, and `--opencode`. The installer uses the latest [GitHub release](https://github.com/orchidautomation/message-decision-packs/releases/latest) so the CLI and agent instructions stay version-aligned.

Verify the installation in a directory that contains a pack:

```bash
mdp --version
mdp --json doctor --dir .
mdp --json capabilities
```

See [Getting Started](docs/getting-started.md) for the complete first-run walkthrough.

## First Workflow

Create and validate the generic MDP reference pack:

```bash
mdp --json init --template gtm --dir /tmp/mdp-demo --force
mdp --json validate --dir /tmp/mdp-demo
mdp --json skills --dir /tmp/mdp-demo
mdp --json --summary route --entries --eval-fixture --dir /tmp/mdp-demo --persona "PMM" --job "linkedin outbound copy"
mdp --json route --entries --dir /tmp/mdp-demo --persona "PMM" --job "portfolio scope example" --scope product=local-cli
mdp sample-leads --dir /tmp/mdp-demo --persona "PMM" --job "initial email outbound copy" --count 3 --format yaml
mdp --json fit --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json
mdp --json --summary brief --context --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin --out /tmp/mdp-demo/.mdp/briefs/example-linkedin.json
mdp render-brief --dir /tmp/mdp-demo --file /tmp/mdp-demo/.mdp/briefs/example-linkedin.json --template gtm-prospect --out /tmp/mdp-demo/.mdp/briefs/example-linkedin.md
mdp --json check-claims --dir /tmp/mdp-demo --text "MDP is a local offline CLI for modular message context."
mdp --json check-claims --dir /tmp/mdp-demo --text "<draft copy>" --subject "<subject>" --persona "PMM" --job "initial email outbound message"
mdp --json check-claims --dir /tmp/mdp-demo --text "<draft copy>" --persona "PMM" --job "portfolio scope example" --scope product=local-cli
mdp --json gaps --dir /tmp/mdp-demo
mdp --json eval --dir /tmp/mdp-demo
```

For a real company, product, or project, use the target-aware path. A custom pack name does not establish the sold target by itself:

```bash
mdp --json init --template gtm --name "Example Company Messaging" --target-name "Example Company" --target-kind company --dir /tmp/example-company-mdp
mdp --json validate --dir /tmp/example-company-mdp
```

The target-aware scaffold records unsupported positioning, ICP, persona, pain, proof, hook, and CTA detail as gaps. Add `--exclude-term` for each prior target or starter noun that must not survive a retarget; validation reports the exact file and field when residue remains. `init --force` refuses to overwrite an existing different target because unreferenced old files could survive; use a clean directory or explicitly migrate and validate the existing pack.

The generated starter prospect is synthetic. Rows created by `sample-leads` are also marked `do_not_contact`. Never treat either as a real prospect. MDP should stop with `disqualified` or `insufficient-context` when evidence is too weak for drafting.

Current GTM packs can keep qualification policy in `manifest.yaml` `qualification_gates`, including required public-person resolution and source-backed fit or why-now signal coverage. Portfolio packs can declare product, capability, solution, or segment dimensions and scope existing card entries to them. Pass explicit `--scope dimension=value` selectors to `route` and route-scoped `check-claims`; `fit` and `brief` derive declared scope from prospect attributes. Routed output rules, including `exact_paragraphs`, are enforced only for the selected persona, job, and scope.

Proposal review uses the same local primitives with a different profile vocabulary:

```bash
mdp --json init --template proposal --dir /tmp/mdp-proposal --force
mdp --json validate --dir /tmp/mdp-proposal
mdp --json route --entries \
  --dir /tmp/mdp-proposal \
  --persona "Proposal Lead" \
  --job "bid no bid review"
mdp --json author-proof-output \
  --dir /tmp/mdp-proposal \
  --draft /tmp/mdp-proposal/examples/proof-output-drafts/compliance-row.draft.json \
  --out /tmp/mdp-proof-output.json
mdp --json verify-output \
  --dir /tmp/mdp-proposal \
  --file /tmp/mdp-proof-output.json
mdp --json run-receipt \
  --dir /tmp/mdp-proposal \
  --workflow proposal-review \
  --isolation isolated \
  --declared-inputs-only \
  --prompt-id normalize-opportunity \
  --prompt-output /tmp/normalize-opportunity-output.json \
  --validation /tmp/normalize-opportunity-validation.json \
  --source-audit /tmp/source-audit.json \
  --runner-audit /tmp/runner-audit.json \
  --require-runner-audit \
  --out /tmp/mdp-proposal-run-receipt.json
```

The proposal profile supports review and gap surfacing. It does not replace compliance, legal, procurement, proposal management, or human approval.

## Proposal Video Walkthrough

[Proposal Flow Video Demo](examples/proposal-flow-video/README.md) is a synthetic, public-safe walkthrough for messy proposal sources → `mdp init --template proposal` → source-audit/prompt-output artifacts → runner-audit/run-receipt proof gates → verified human-readable proposal review output, plus a Remotion project that renders the walkthrough as an MP4. The included runner-audit fixture is demo-only and the CLI blocks it from `audit-grade`; real pilots should replace it with the native/headless runner or MCP-produced `mdp.runner-audit.v0` artifact before calling a review audit-grade.

## Canonical GTM Runtime Example: Eve on Vercel

[AI SDR Eve on Vercel](examples/ai-sdr-eve-vercel/README.md) shows how an Eve runtime can:

- load an MDP pack and source strategy;
- gather bounded public-source evidence through approved provider tools;
- target three qualified people per live run, continuing across approved strategy prompts until the target or bounded exhaustion;
- let pack-owned `qualification_gates` require person-level and source-backed evidence;
- run MDP validation, fit, brief, and claim gates;
- append reviewed ledger rows without sending outreach or syncing a CRM.

The example is a runtime around MDP, not MDP itself. Its committed pack and fixtures are synthetic and safe to inspect. Use the deploy button in the example README or visit [mdp.orchidlabs.dev/eve](https://mdp.orchidlabs.dev/eve).

## The Pack Model

A pack is a local folder:

```text
.mdp/
  manifest.yaml
  sources.yaml
  source-strategy.json   # optional reviewed discovery plan
  prompts/*.yaml         # optional normalization/extraction contracts
  cards/*.yaml           # modular decisions and boundaries
  briefs/                # generated local review artifacts
  evals/*.yaml
```

Agents should load the manifest first, preserve source provenance, and use routed entries instead of reading every card. For prompt outputs, `source_summary.inputs_used` names declared prompt inputs only; field paths, snippets, URLs, PDF/page locators, and review notes belong in evidence/provenance fields such as `signals[].source`, entry `provenance`, and normalization trace. For GTM rows, normalize supplied data before running the deterministic fit gate. Proposal normalization keeps `normalized_prospect` for compatibility and may include `normalized_opportunity` only as an exact alias. Draft only from `brief --context` output, then run `check-claims`. For source-bound generated output, use `author-proof-output` to compile draft segments when helpful, then use `mdp.proof-output.v0` and `verify-output` before treating cited IDs as proof.

For audit-grade proposal normalization, the runner or host must make a fresh/stateless model call and pass only prompt-declared inputs. `mdp run-receipt` records that host-owned boundary plus local artifact hashes for the source audit, prompt output, validation result, runner audit, and downstream files, and blocks if the validation-result hashes do not match the supplied prompt-output/source-audit artifacts, if the runner-audit prompt-output hash does not match the supplied prompt output, or if the runner audit is marked demo/fixture/mock/synthetic. Same-conversation normalization without a required runner-audit receipt is advisory even when the JSON validates.

Profiles express domain language over ten universal primitives:

| Primitive | GTM examples | Proposal examples |
|---|---|---|
| `actors` | personas | proposal roles |
| `decision-criteria` | fit and disqualification rules | bid/no-bid and evaluation criteria |
| `source-signals` | account/person signals | opportunity and requirement signals |
| `needs-requirements` | pains and readiness needs | requirements matrix |
| `evidence-proof` | positioning and approved claims | proof library and past performance |
| `boundaries` | avoid-rules and objections | compliance and proposal boundaries |
| `output-contracts` | output rules, hooks, CTAs, patterns | review outputs and response rules |
| `routing-jobs` | motions and channel policies | review gates and jobs |
| `gaps` | missing evidence or owner context | unsupported requirements or proof |
| `evals` | fit, route, brief, and copy checks | review, proof, and safety checks |

Profile vocabulary belongs in the manifest, cards, prompts, input contracts, jobs, and eval fixtures. It does not create a separate MDP engine for every domain.

## Plugin Distribution

The full repository is the product/plugin contract: CLI behavior, docs, canonical templates/assets, authored skills, install/release assets, repo scripts, and Pluxx config stay in lockstep. Authored skills live under `plugin/skills`, and [Pluxx](https://pluxx.dev) packages canonical source into release bundles for Claude Code, Cursor, Codex, and OpenCode. The public MDP installer combines those bundles with the matching Rust CLI binary; Pluxx is the packaging layer, not the CLI runtime or a hosted MDP service.

MDP ships five job-shaped skills: `mdp` for explicit CLI/operator and mixed work, `mdp-pack-builder` for pack authoring, `mdp-pack-review` for the pack artifact itself, `mdp-gtm-brief` for the three GTM fit/brief/copy-review jobs, and `mdp-proposal-review` for the four proposal review jobs. `mdp --json skills --dir <pack> --job <job-id>` validates pack eligibility and the exact job route; host discovery remains separate and host-managed.

See [Distribution](docs/distribution.md) for the release and update contract and [Agent Hook Guidance](docs/agent-hook-guidance.md) for activation/validation boundaries.

## Documentation

- [Getting Started](docs/getting-started.md): install, create, route, fit, brief, and validate.
- [Portfolio-Aware GTM Scope](docs/portfolio-scope.md): product, capability, solution, and segment scoping inside one pack.
- [Conceptual Decision Flow](docs/conceptual-decision-flow.md): layer ownership and deterministic decision boundaries.
- [Prompt Contracts](docs/prompt-extraction-contract.md): normalization and extraction schemas.
- [Runner Receipts](docs/run-receipts.md): context-isolation receipt contract for audit-grade proposal workflows.
- [Headless Normalization Runners](docs/headless-normalization-runners.md): native/headless runner recipes for Codex, Claude Code, Cursor, OpenCode, and future MCP wrappers.
- [Native API Normalization Runner](docs/native-api-normalization-runner.md): optional BYOK OpenAI reference runner for stateless Structured Outputs normalization.
- [Proof-Output Drafting](docs/proof-output-drafting.md): draft-helper workflow for verified proof-output artifacts.
- [Agent Hook Guidance](docs/agent-hook-guidance.md): safe activation and post-edit validation.
- [Distribution](docs/distribution.md): releases, Pluxx bundles, installers, and updates.
- [Skill Evals](docs/skill-evals.md): trigger and output-eval fixtures.
- [CLI Usage](cli/USAGE.md): detailed command workflows; `mdp --json capabilities` is the current machine-readable command contract.

Agents can use [llms.txt](llms.txt) for a short briefing or [llms-full.txt](llms-full.txt) for fuller operating context. Released copies are also available at `https://mdp.orchidlabs.dev/llms.txt` and `https://mdp.orchidlabs.dev/llms-full.txt`.

## Validation

From the repo root:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
make validate
```

The Eve example has its own checks:

```bash
cd examples/ai-sdr-eve-vercel
npm ci
npm run check
```

## License And Status

This source-available repository uses the [Elastic License 2.0](LICENSE). Local/offline and internal use are allowed under its terms. Offering a hosted or managed service that exposes a substantial set of MDP functionality requires a separate commercial license; see [Commercial Use](COMMERCIAL.md).

MDP is an MVP local/offline implementation. There is no hosted MDP API, sending, CRM mutation, enrichment writeback, scraping, sequencing, or proposal submission workflow in the core product.
