# Eve-native MDP AI SDR Scout

This is the intended Vercel/Eve example for an autonomous, schedule-backed AI SDR/BDR scout powered by Message Decision Packs (MDP).

The bundled seller and prospect are explicitly synthetic. They demonstrate the workflow without exposing a customer pack, implying real buyer intent, or approving real-world product, customer, security, certification, or compliance claims.

The important split:

- **Eve** is the autonomous runtime: instructions, schedules, sandbox, skills, tools, durable sessions, and future MCP/connections.
- **MDP** is the local/offline decision pack: ICP, source strategy, fit rules, brief context, claims, writing rules, evals, and normalized ledger contracts.

MDP is not a CRM, sender, sequencer, enrichment provider, scraper, or hosted SDR product. This example only prepares reviewed scout evidence and CRM-ready ledger rows.

[![Deploy with Vercel](https://vercel.com/button)](https://vercel.com/new/clone?repository-url=https%3A%2F%2Fgithub.com%2Forchidautomation%2Fmessage-decision-packs&root-directory=examples%2Fai-sdr-eve-vercel&project-name=mdp-eve-scout&repository-name=mdp-eve-scout&env=EXA_API_KEY%2CCRON_SECRET&envDescription=EXA_API_KEY+enables+live+public+discovery.+CRON_SECRET+protects+the+scheduled+scout+endpoint.+FIRECRAWL_API_KEY+and+APIFY_TOKEN+are+optional+follow-up+lanes+you+can+add+after+deploy.&envLink=https%3A%2F%2Fgithub.com%2Forchidautomation%2Fmessage-decision-packs%2Ftree%2Fmain%2Fexamples%2Fai-sdr-eve-vercel%23environment-variables)

Use the button to clone and deploy only `examples/ai-sdr-eve-vercel` as a Vercel project. The template prompts for `EXA_API_KEY` and `CRON_SECRET`; optional Firecrawl and Apify keys can be added after deploy.

## Shape

```text
examples/ai-sdr-eve-vercel/
├── .mdp/                         # operator-authored MDP pack used by local CLI/plugin workflows
├── agent/
│   ├── agent.ts                   # Eve runtime config
│   ├── instructions.md            # always-on autonomous scout policy
│   ├── schedules/weekday-scout.md # Eve schedule, compiled to Vercel Cron
│   ├── skills/                    # MDP plugin skills ported into Eve load_skill surface
│   ├── tools/                     # bounded MDP/search/ledger tools
│   └── sandbox/workspace/.mdp/    # same pack seeded into Eve sandbox workspace
├── samples/                       # public-safe fixture discovery input
└── scripts/run-fixture.ts          # local smoke test without live keys
```

## Environment variables

| Variable | Required? | Purpose |
| --- | --- | --- |
| `EXA_API_KEY` | Required for live runs | Enables public Exa discovery and person-resolution searches. `dryRun: true` works without it. |
| `CRON_SECRET` | Required for protected production runs | Protects `/scout/run`; Vercel Cron sends `Authorization: Bearer $CRON_SECRET`. |
| `MDP_SCOUT_MODEL` | Optional | Model id for Eve/Vercel AI Gateway. Defaults to `xai/grok-4.5`. |
| `FIRECRAWL_API_KEY` | Optional | Accepted-URL cleanup only; not used for broad discovery. |
| `APIFY_TOKEN` | Optional follow-up | Reserved for reviewed Apify MCP/Actor lanes; not enabled by default. |
| `SCOUT_MAX_CANDIDATES` / `SCOUT_MIN_SCORE` | Optional | Runtime limits and qualification threshold. |

## Runtime loop

```text
Eve schedule -> load MDP scout instructions -> load source strategy -> target 3 qualified people -> continue across approved account-discovery prompts until target/bounded exhaustion -> render the MDP person-resolution query_template -> resolve public person/role owner -> run MDP fit/brief gates -> score -> append ledger row
```

The agent should call typed tools such as `load_source_strategy`, `discover_candidates`, `extract_evidence`, `mdp_validate`, `mdp_fit`, `mdp_create_brief`, `mdp_check_claims`, and `append_ledger`. Generic sandbox `bash` remains available through Eve, but the production MDP path should prefer bounded tools.


## Public demo and installer continuity

Use the Orchid Labs vanity domain for MDP-facing install and demo commands:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y

curl -L -X POST https://mdp.orchidlabs.dev/eve/scout/run \
  -H 'content-type: application/json' \
  -d '{"dryRun":true,"includeRows":true,"limit":1}'
```

`https://mdp.orchidlabs.dev/eve` redirects to the deployed Eve scout. `https://mdp.orchidlabs.dev/eve/docs` redirects to the canonical Vercel Eve docs. Prefer the Vercel docs URL for references because `eve.dev` is an upstream vanity domain and may be unstable during rollout.

## Deterministic scout endpoint

For smoke tests, Vercel Cron, or operator-triggered runs that should not require a model turn, the example exposes a custom Eve channel endpoint:

```bash
# Public-safe fixture smoke test; this does not call live providers.
curl -X POST "$DEPLOYMENT_URL/scout/run" \
  -H 'content-type: application/json' \
  -d '{"dryRun":true,"includeRows":true,"limit":1}'
```

`dryRun: true` is the only path that uses the public-safe fixture. Omit `dryRun` to use live Exa when `EXA_API_KEY` is configured; protected live/Cron runs without Exa fail closed with `qualified: 0` and do not append fixture rows. Live Exa runs now do two passes: account trigger discovery, then public person/role resolution. The live run target is driven by `.mdp/source-strategy.json` via `run_policy.minimum_qualified_people_per_run` (3 in this example), so Eve keeps searching the approved strategy prompts until it has enough qualified people or reports bounded exhaustion. The person lookup is driven by `.mdp/source-strategy.json` via `exa-person-role-resolution.query_template`, so the operator-authored MDP pack controls where Eve looks for person-level resolution.

Qualification is owned by native `mdp fit`, using `.mdp/manifest.yaml` `qualification_gates`: public person resolution plus 1-3 source-backed signals with fit and why-now coverage. Eve gathers evidence, normalizes candidate JSON, calls `mdp fit`/`mdp brief`, and only appends rows when MDP returns `fit`, no MDP gaps remain, the score clears the threshold, evidence is present, and blocked evidence sources are absent. `SCOUT_REQUIRE_PERSON=false` can let account-only discoveries continue into diagnostic fit/brief evaluation, but qualification still follows the MDP gate. The response is `mdp.scout-run-response.v0` and includes the run id, selected query, all attempted queries, target qualified count, discovery passes, exhaustion state, provider, fallback reason, qualified count, and ledger path. The endpoint never sends outreach or writes CRM records.

Hosted production runs fail closed unless `CRON_SECRET` is configured and the request includes the matching bearer header. Vercel Cron targets `/scout/run` on the weekday schedule in `vercel.json` and automatically sends `Authorization: Bearer $CRON_SECRET`. For manual live runs, callers may also send the same secret in `x-mdp-scout-secret`.

## Local fixture run

```bash
cd examples/ai-sdr-eve-vercel
npm install
npm run check
```

The fixture run uses `samples/synthetic-public-source-fixture.json`, reports the dry-run fallback reason, and writes `artifacts/scout-ledger.jsonl`. Non-dry-run executions require `EXA_API_KEY`; without it they return zero qualified rows rather than writing demo data.

## Native MDP CLI mode

If the `mdp` CLI is installed in the app runtime, test the bounded native path:

```bash
MDP_RUNNER_MODE=native npm run scout:fixture
```

The Eve sandbox also receives the same `.mdp` under `/workspace/.mdp`, including Synthetic Vendor person-resolution fit cards, gaps, sources, and source-strategy query templates, so a future Vercel Sandbox bootstrap can install the CLI there and let the agent run CLI commands through sandbox `bash`. This first slice keeps `simulated` as the deployment-safe default.

## Eve schedule

`agent/schedules/weekday-scout.md` runs at `0 14 * * 1-5` UTC. Hosted Vercel builds compile this to Vercel Cron through Eve.

## Live keys

For live discovery/extraction, set these in Vercel env vars; do not commit or paste secrets into chat:

```bash
# Required for live public discovery. Without this, non-dry-run scout executions fail closed.
EXA_API_KEY=...

# Optional accepted-URL cleanup. The agent skips this lane when absent.
FIRECRAWL_API_KEY=...

# Optional advanced lane. Apify MCP/Actor execution is documented but deferred from v1.
APIFY_TOKEN=...
```

Provider behavior:

| Provider | Current Eve path | Required for local checks? |
| --- | --- | --- |
| Exa | Local Vercel AI SDK `tool()` wrapper around Exa search with `x-exa-integration: vercel-ai-sdk` | No for dry-run checks; yes for live/Cron discovery |
| Firecrawl | Optional accepted-URL `tool()` wrapper for cleanup after Exa/operator acceptance | No |
| Apify | Optional future MCP/Actor lane; the source strategy may describe it but the v1 Eve adapter does not execute it | No |
| Fixture | Public-safe local candidate/evidence bundle | Yes, always available |

The upstream `@exalabs/ai-sdk` and `firecrawl-aisdk` packages currently declare `ai@^6` peer dependencies while Eve `0.22.1` uses `ai@7`, so this example keeps clean installs by using small local `ai@7` `tool()` wrappers. Swap to first-party packages when they publish `ai@7`-compatible releases.

Model routing uses Vercel AI Gateway by default via `MDP_SCOUT_MODEL`; the example default is `xai/grok-4.5`. Add `AI_GATEWAY_API_KEY` only when running outside Vercel.

## Current limitations

- CRM sync and outreach are disabled by design.
- Persistent storage is still local JSONL/ephemeral filesystem; add Vercel Blob/Neon in a follow-up.
- `MDP_RUNNER_MODE=simulated` is the default; production CLI-in-sandbox install is a follow-up once the deployment target and binary policy are settled.
- Apify execution is intentionally deferred until an approved MCP/Actor adapter can enforce source allowlists, result caps, and budget limits.
- Persistent ledger storage and review packets are still follow-ups; this slice writes local JSONL only.
