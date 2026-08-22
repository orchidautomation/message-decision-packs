# Message Decision Packs

**MDP is versioned decision context for agents.** A local `.mdp/` folder makes agent judgment explicit by storing source evidence, decision rules, approved claims or proof, routing contracts, output boundaries, gaps, and evals. The Rust CLI validates and routes that context; the plugin teaches supported agents how to use it.

MDP is a decision/context layer: it produces deterministic decisions and bounded, hash-bound traces from pack-owned authority. Its optional local BYOK driver can execute exactly one job-declared model step—normalization, generation, or review—then return the result to the same validation and receipt kernel. It is not an agent runtime, graph database, memory layer, orchestration framework, AI SDR, CRM, sequencer, enrichment provider, scraper, BI tool, proposal management system, or generic automation platform. It does not collect source data, sequence multi-step work, send messages, update external systems, calculate inference pricing, or prove that a source claim is true.

See [CONCEPTS.md](CONCEPTS.md) for the canonical vocabulary and assurance-state
boundaries used across the CLI, docs, and agent skills.

```text
message-decision-packs/
  cli/      # Rust `mdp` CLI
  plugin/   # Pluxx plugin package source: skills, assets, hooks, scripts
  docs/     # Current user and maintainer documentation
  examples/ # Canonical runnable and synthetic contract examples
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
mdp --json requirements --dir /tmp/mdp-demo --job prospect-fit-or-brief
mdp --json skills --dir /tmp/mdp-demo
mdp --json --summary route --entries --eval-fixture --dir /tmp/mdp-demo --persona "PMM" --job "linkedin outbound copy"
mdp --json route --entries --dir /tmp/mdp-demo --persona "PMM" --job "portfolio scope example" --scope product=local-cli
mdp sample-leads --dir /tmp/mdp-demo --persona "PMM" --job "initial email outbound copy" --count 3 --format yaml
mdp --json fit --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json
mdp --json trace --file examples/decision-trace/fixtures/fit-ready-result.json
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

Applicability selectors are structured routing metadata: empty or blank-only
card `personas` and entry `applies_to` lists are universal for persona
matching, while non-empty values match exactly and case-insensitively. Persona
words in prose are not selectors. Universal applicability still goes through
job/channel policy, scope, guardrail, route-card-cap, and context-budget gates;
use `route --entries`, `brief --context`, and `route-budget` together when
checking a pack.

Use `mdp --json --summary route-budget --dir PACK_DIR` for the bounded
`mdp.route-budget-summary.v1` rollup, or add exact `--job JOB_ID` and
`--persona PERSONA` selectors for one route projection. Summary output keeps
status counts, utilization, blockers, contributors, and safe-action guidance
but never includes route arrays or entry bodies. It also preserves aggregate
exclusion counts from required-first allocation and route-card caps. Full
route-budget output is the machine-readable authority; `job_id` is canonical
and the deprecated v0 `job` alias remains equal for compatibility.

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

[Proposal Flow Video Demo](examples/proposal-flow-video/README.md) is a synthetic, public-safe walkthrough for messy proposal sources → `mdp init --template proposal` → local proposal runner → source-audit/prompt-output/runner-audit artifacts → run-receipt gates → verified human-readable proposal review output, plus a Remotion project that renders the walkthrough as an MP4. The default demo uses offline mock mode and produces a blocked/non-audit-grade receipt; label it mock/non-audit-grade. The CLI blocks demo/fixture/mock/synthetic runner evidence from `audit-grade`. Real pilots must use native/headless runner evidence plus an audit-grade receipt for that invocation before claiming model-context isolation. The [canonical runner support matrix](docs/headless-normalization-runners.md#canonical-runner-support-matrix) separately records whether an integration is verified, recipe-only, unsupported, or fixture/mock-only; currently no runner integration is verified.

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
  traces/                # optional generated decision-trace views
  evals/*.yaml
```

Agents should load the manifest first, preserve source provenance, and use routed entries instead of reading every card. Raw `mdp.prompt-output.v0` is untrusted and cannot provide decision-trace authority; only a successful `mdp.prompt-output-validation.v1` receipt supplied with the exact pack, output, and validator-input bytes can be projected as validated. For prompt outputs, `source_summary.inputs_used` names declared prompt inputs only; field paths, snippets, URLs, PDF/page locators, and review notes belong in evidence/provenance fields such as `signals[].source`, entry `provenance`, and normalization trace. For GTM rows, normalize supplied data before running the deterministic fit gate. Proposal normalization keeps `normalized_prospect` for compatibility and may include `normalized_opportunity` only as an exact alias. Draft only from `brief --context` output, then run `check-claims`. For source-bound generated output, use `author-proof-output` to compile draft segments when helpful, then use `mdp.proof-output.v0` and `verify-output` before treating cited IDs as proof.

Profiles may declare a product-foundation registry whose facets reference exact
existing card entries and explicit gaps, then classify those facets per
canonical job as required, conditional, optional, or excluded. Resolve the
exact job through `skills --job` and `requirements --job`; never infer
foundation authority from free-text jobs. Status is `unassessed`, `ready`, or
`blocked`. Foundation readiness is veto-only: it can block broader readiness,
but it never establishes sufficient-for-job or self-standing status. See
[Product Foundations](docs/product-foundations.md).

Each initialized pack also carries `.mdp/README.md` as human orientation.
Treat it as secondary navigation after CLI-resolved structured authority. It
cannot satisfy a facet or gap. Because it is a regular `.mdp/` file, changing
it changes the portable pack hash even though it does not change resolver or
readiness authority.

Jobs that need an attempted-complete data policy can bind versioned
`decision_input_contracts`. Start with `mdp --json capabilities`, then run
`mdp --json requirements --dir <pack> --job <job-id>` before inspecting cards
or collecting data. Requirements compiles the exact questions, source policy,
attempt statuses, provenance, confidence, freshness, normalization schemas,
signal projections, conflict rules, version matrix, and no-draft boundary.
Structured repeated signal observations exist only in the opt-in v2 envelope;
scalar-only v1 and detached prospect signals remain readable as legacy or
unassessed context and cannot satisfy explicit roles. The contract tells a
customer host what to attempt. MDP performs no collection. The host may execute
the declared normalization step itself or use the optional local BYOK native
driver, one selected step and one receipt at a time. See
[Decision Input Contracts](docs/decision-input-contracts.md) and the synthetic
[Clay Audiences example](examples/clay-audiences-self-serve-enterprise-expansion/README.md).

## Cold-model conformance

For one exact release and job, MDP can compile deterministic sufficiency,
validate externally recorded behavioral evidence, assemble one hash-linked
`mdp.job-conformance.v1` authority, and project private or sanitized public
reports. Discover the installed surface with `mdp --json capabilities` and
`mdp conformance --help`.

The required order is discover → deterministic compile → stop unless
sufficient → customer-hosted model call → recorded-evidence validation →
composite assembly → report or trace. `sufficient-for-job` is the deterministic
gate; `qualified-for-job-under-envelope` additionally requires the declared
behavioral trial thresholds. Missing evidence is `unassessed`; a failed
required assertion is `not-sufficient-for-job` or
`not-qualified-for-job-under-envelope`, depending on the proof plane.

The conformance commands never make the behavioral-trial model call. The
optional local run driver is a separate execution surface and does not turn a
native run into behavioral qualification. MDP does not calculate provider
pricing or grant drafting/sending authority. See
[Cold-model Conformance](docs/cold-model-conformance.md).

External orchestrators can bind their fields to one exact compiled job through
the provider-neutral v1 or signal-aware v2 source-binding contract selected by
the compiled requirements. Use `mdp --json schema source-binding`, then
`mdp --json validate-source-binding --dir <pack> --job <job-id> --file
<binding.json>`. For a v2 job, preserve the exact binding, request, collected
results, prompt, and normalized envelope, validate the chain, and pass it to
`fit` or `brief` through `--normalized-input`; do not extract and edit a
detached prospect. `lineage-validated` means internal chain consistency only,
not host authenticity or source truth. Bindings remain integration-owned and
outside `.mdp`; source binding itself performs no provider call or
orchestration. A customer host may later select one declared model step for a
separate native run.

For a native model step, one `mdp.run-request.v1` selects one stable step ID.
The shared Rust runtime freezes the prompt and declared inputs, calls the
profile-neutral OpenAI BYOK subprocess, validates the returned artifact, and
emits one receipt. The same path covers the shipped basic GTM and proposal
templates. The customer host explicitly sequences separate normalization,
deterministic fit/routing, and generation/review runs; MDP does not chain them
automatically. Real calls are default-deny and require both
`MDP_ALLOW_NATIVE_MODEL_CALLS=1` and `OPENAI_API_KEY` in the process environment;
dry-run and mock validation are key-free and do not prove a provider call. The
official OpenAI Responses endpoint is the only bundled native endpoint.

The older proposal runner, proposal MCP wrapper, `mdp run-receipt`, and
`scripts/mdp-native-normalize-openai.mjs` remain v0 compatibility surfaces.
The profile-neutral local stdio MCP surface is
`scripts/mdp-run-mcp-server.mjs`; it transports file paths to the same CLI and
adds no authority or isolation assurance.

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

MDP ships five job-shaped skills: `mdp` for explicit CLI/operator and mixed
work, including source-binding validation; `mdp-pack-builder` for pack
authoring; `mdp-pack-review` for the pack artifact and supplied integration
bindings; `mdp-gtm-brief` for the three GTM fit/brief/copy-review jobs; and
`mdp-proposal-review` for the four proposal review jobs. Source binding does not
add a sixth skill because it is a CLI contract between pack authoring/review
and integration-owned execution. `mdp --json skills --dir <pack> --job
<job-id>` validates pack eligibility and the exact job route; host discovery
remains separate and host-managed.

See [Distribution](docs/distribution.md) for the release and update contract and [Agent Hook Guidance](docs/agent-hook-guidance.md) for activation/validation boundaries.

## Documentation

- [Minimal context routing](docs/minimal-context-routing.md) — job budgets, exact routed-context digests, invocation binding, and selected-authority enforcement.

- [Getting Started](docs/getting-started.md): install, create, route, fit, brief, and validate.
- [Portfolio-Aware GTM Scope](docs/portfolio-scope.md): product, capability, solution, and segment scoping inside one pack.
- [Product Foundations](docs/product-foundations.md): exact per-job product authority, readiness, compatibility, and README orientation.
- [Conceptual Decision Flow](docs/conceptual-decision-flow.md): layer ownership and deterministic decision boundaries.
- [Prompt Contracts](docs/prompt-extraction-contract.md): normalization and extraction schemas.
- [Job-owned Prompt Contracts](docs/job-prompt-contracts.md): versioned generation/review instructions, declared input producers, compiled host packages, and exact governed-artifact schemas.
- [Decision Input Contracts](docs/decision-input-contracts.md): attempted-complete data questions, source-attempt policy, normalization envelopes, and no-draft outcomes.
- [Decision Traces](docs/decision-traces.md): bounded JSON and Mermaid projections of existing decision authority.
- [Runner Receipts](docs/run-receipts.md): unified v1 run receipts plus the legacy proposal receipt contract.
- [Local Proposal Runner Surface](docs/proposal-runner.md): host-neutral local command surface for source audit, native/headless normalization, validation, receipts, and review probes.
- [Deterministic Proposal Evidence Harness](docs/proposal-evidence-harness.md): synthetic CI proof for positive contract acceptance and fail-closed ambient/mock/hash/injection/unsupported-proof cases.
- [Headless And Native Model Runners](docs/headless-normalization-runners.md): the canonical native path plus compatibility recipes for Codex, Claude Code, Cursor, and OpenCode.
- [Native API Model Runner](docs/native-api-normalization-runner.md): optional BYOK OpenAI driver for one declared normalization, generation, or review step.
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
