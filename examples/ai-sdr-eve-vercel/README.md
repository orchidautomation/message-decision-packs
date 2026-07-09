# Eve-native MDP AI SDR Scout

This is the intended Vercel/Eve example for an autonomous, schedule-backed AI SDR/BDR scout powered by Message Decision Packs (MDP).

The important split:

- **Eve** is the autonomous runtime: instructions, schedules, sandbox, skills, tools, durable sessions, and future MCP/connections.
- **MDP** is the local/offline decision pack: ICP, source strategy, fit rules, brief context, claims, writing rules, evals, and normalized ledger contracts.

MDP is not a CRM, sender, sequencer, enrichment provider, scraper, or hosted SDR product. This example only prepares reviewed scout evidence and CRM-ready ledger rows.

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

## Runtime loop

```text
Eve schedule -> load MDP scout instructions -> load source strategy -> discover evidence -> run MDP fit/brief gates -> score -> append ledger row
```

The agent should call typed tools such as `load_source_strategy`, `discover_candidates`, `mdp_validate`, `mdp_fit`, `mdp_create_brief`, `mdp_check_claims`, and `append_ledger`. Generic sandbox `bash` remains available through Eve, but the production MDP path should prefer bounded tools.

## Local fixture run

```bash
cd examples/ai-sdr-eve-vercel
npm install
npm run check
```

With no provider keys, the fixture run uses `samples/profound-public-source-fixture.json` and writes `artifacts/scout-ledger.jsonl`.

## Native MDP CLI mode

If the `mdp` CLI is installed in the app runtime, test the bounded native path:

```bash
MDP_RUNNER_MODE=native npm run scout:fixture
```

The Eve sandbox also receives `.mdp` under `/workspace/.mdp`, so a future Vercel Sandbox bootstrap can install the CLI there and let the agent run CLI commands through sandbox `bash`. This first slice keeps `simulated` as the deployment-safe default.

## Eve schedule

`agent/schedules/weekday-scout.md` runs at `0 14 * * 1-5` UTC. Hosted Vercel builds compile this to Vercel Cron through Eve.

## Live keys

For live discovery/extraction, set these in Vercel env vars; do not commit or paste secrets into chat:

```bash
EXA_API_KEY=...
APIFY_TOKEN=...
```

Model routing should use Vercel AI Gateway by default via `MDP_SCOUT_MODEL`. Add `AI_GATEWAY_API_KEY` only when running outside Vercel.

## Current limitations

- CRM sync and outreach are disabled by design.
- Persistent storage is still local JSONL/ephemeral filesystem; add Vercel Blob/Neon in a follow-up.
- `MDP_RUNNER_MODE=simulated` is the default; production CLI-in-sandbox install is a follow-up once the deployment target and binary policy are settled.
- MDP-86 will make the source-strategy prompt blocks more agent-directive; this example is structured to consume that output when it lands.
