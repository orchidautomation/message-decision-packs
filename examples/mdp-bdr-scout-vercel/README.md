# MDP BDR Scout for Vercel

This is the Vercel-first demo path for a light AI BDR/SDR scout powered by Message Decision Packs (MDP).

The scout is a wrapper around MDP. MDP remains the local/offline decision and context standard; this example does **not** make MDP a CRM, sender, sequencer, scraper, enrichment provider, or hosted SDR product.

## What this slice proves

- A scheduled Vercel-style scout cycle can discover candidates from provider adapters.
- Candidate and evidence data can be normalized into an explicit ledger row contract.
- MDP fit/brief outputs are represented through a trusted runner interface rather than model-directed arbitrary shell.
- Exa, Firecrawl, and Apify roles are separated and swappable.
- A local dry run works without provider credentials.

## Recommended production stack

- Vercel eve for the agent-facing template shell.
- Vercel Workflow DevKit for durable, retryable scout cycles.
- Vercel Cron Jobs for scheduled starts.
- Fluid Compute for longer AI/API work.
- Vercel AI Gateway for production model routing, budgets, and observability.
- Neon Postgres for the canonical ledger.
- Vercel Blob for brief/source artifacts and JSONL exports.
- Vercel Connect for delegated Salesforce, Slack, GitHub, Snowflake, custom OAuth, and API-key connectors.

## Provider roles

| Provider | Role | Default? |
| --- | --- | --- |
| Exa | AI-native company/person/news discovery through API, AI SDK, or MCP | Yes |
| Firecrawl | URL extraction to markdown/structured JSON/screenshots/JS-rendered content | Yes, as fallback |
| Apify | Store actors, hard-site scraping, long-running crawls, datasets, and Vercel AI SDK/MCP tools | Optional advanced |

## Run the offline sample

This sample path uses only Node.js built-ins and the committed fixture.

```bash
cd examples/mdp-bdr-scout-vercel
npm run scout:sample
```

To prove the trusted local MDP CLI path against the repo pack:

```bash
npm run scout:sample:native
```

Expected output:

- prints a normalized ledger row summary;
- writes `artifacts/scout-ledger.jsonl` locally;
- does not call Exa, Firecrawl, Apify, AI Gateway, or a CRM.

Run the structural and TypeScript checks:

```bash
npm run check:scaffold
npm run typecheck
```

`check:scaffold` validates the fixture, required file layout, and dry-run contract without live credentials. `typecheck` generates Next route types with `next typegen` and then runs `tsc --noEmit`.

## Install for Vercel development

```bash
npm install
cp env.example .env.local
npm run dev
```

Then test the cron route locally. The route starts a Vercel Workflow run and returns status/stream URLs:

```bash
curl http://localhost:3000/api/cron/scout
```

If `CRON_SECRET` is set, include:

```bash
curl http://localhost:3000/api/cron/scout \
  -H "Authorization: Bearer $CRON_SECRET"
```

## MDP runner modes

`MDP_RUNNER_MODE=simulated` is the safe default for this first template slice. It proves the ledger and scoring contract without shelling out.

Planned modes:

- `native`: trusted application code runs `mdp --json fit` and `mdp --json brief --context` against scratch files under `/tmp`. This is validated locally by `npm run scout:sample:native`; set `MDP_PACK_DIR` when the pack is not the repo root.
- `sandbox`: a Vercel Sandbox microVM receives the pack/prospect files, runs `mdp`, and returns artifacts. This remains gated behind project credentials and binary download policy.

The model never receives a generic shell tool for MDP execution.

## Output contract

The canonical row shape lives in `src/schemas/ledger.ts` and sample output lives in `samples/candidate-ledger-row.json`.

```text
candidate + evidence -> MDP fit/brief context -> normalized score -> append-only ledger row
```

## Current limitations

- This is a scaffolded first slice, not a deployed Vercel template yet.
- Live Exa/Firecrawl/Apify adapters are implemented with credential gates, but production usage still needs provider allowlists, budget limits, and source policy review.
- CRM sync is explicitly disabled by default.
- Current legacy example packs have known prompt-contract validation issues under `mdp 0.1.34`; use a passing pack before making live MDP validation a release gate.
