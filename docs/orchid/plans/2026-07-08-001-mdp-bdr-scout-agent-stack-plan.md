# MDP BDR Scout Agent Stack Plan

Date: 2026-07-08
Status: proposed, revised to Vercel-first
Owner: Brandon / Orchid
Repo: message-decision-packs

## Goal

Build a turnkey "BDR Scout powered by Message Decision Packs" that can be deployed as a Vercel template. The scout wakes on a schedule, searches public sources for candidate leads or conversations, gathers evidence, normalizes each candidate into an MDP-compatible prospect row, runs MDP fit/route/brief gates, computes a normalized score, and writes a ledger row that can later sync to Attio, Salesforce, or another CRM.

This showcases MDP as the deterministic decision/context layer. The wrapper can be an AI BDR scout, but MDP itself remains a local/offline pack + CLI standard, not a sender, CRM, scraper, enrichment vendor, sequencer, or AI SDR.

## Revised recommendation

Use a **Vercel-first template stack** for v1:

1. **Vercel eve** for the agent-facing template shell: `agent/` instructions, tools, skills, durable sessions, and Vercel Agent Runs observability.
2. **Vercel Workflow DevKit / Workflows** for the bounded scheduled `scout-cycle` so discovery, evidence capture, MDP scoring, and ledger writes can retry and resume safely.
3. **Vercel Cron Jobs** to trigger `/api/cron/scout` on a UTC schedule.
4. **Fluid Compute** enabled for the project so function/workflow steps have the right runtime profile for longer AI + API work.
5. **Vercel AI Gateway** as the default model router for production, with provider failover, logs, spend controls, and low-friction Vercel OIDC auth.
6. **Vercel Observability + eve Agent Runs** for traces, tool calls, token usage, function invocations, external API calls, and AI Gateway requests.
7. **Neon Postgres via Vercel Marketplace** for the canonical relational ledger; **Vercel Blob** for brief/artifact/JSONL exports.
8. **Exa as primary discovery** using `@exalabs/ai-sdk`, Exa MCP, and/or Exa API. Exa is the best default for AI-native company/person/news search.
9. **Firecrawl as extraction fallback** using its Vercel Native marketplace integration when a URL needs clean markdown, structured JSON, screenshots, or JS-rendered scrape/interact behavior.
10. **Apify as optional hard-site actor layer**, not the default. Use it when you need an existing Store actor, marketplace scraper, long-running crawl, proxy-heavy job, dataset-oriented output, or Apify MCP tools inside a Vercel AI SDK agent.
11. **Vercel Connect** for delegated third-party credentials, especially Salesforce, Slack, GitHub, Snowflake, custom OAuth, and API-key providers when the scout becomes multi-tenant or user-authorized.
12. **JustBash** only for lightweight model-visible file/shell workspace behavior. It should not be the production isolation boundary for MDP or scraping.

Railway + Flue remains a solid engineering fallback, but it should not be the primary recommendation for this demo because the go-to-market engineering audience wants one-click Vercel deployment, Vercel templates, Vercel dashboard observability, and marketplace integrations.

## Key decision

**Make Vercel the primary product surface.** The demo should feel like:

```text
Deploy template -> add MDP pack -> connect Exa / Firecrawl / Neon -> schedule scout -> inspect ledger and briefs.
```

Not:

```text
Clone repo -> set up Railway -> wire cron -> provision Postgres -> deploy worker -> inspect files elsewhere.
```

The product value is faster adoption and a cleaner story: MDP becomes the pack contract that turns a generic Vercel agent into a company-specific GTM scout.

## Verified integration notes

### Vercel Connect

Vercel Connect is beta and is built for agents/services to act on third-party APIs without storing long-lived provider credentials as deployment env vars. Code on Vercel can request short-lived runtime tokens via OIDC; providers can be Vercel-managed connectors, custom OAuth connectors, or API-key connectors. Current built-in/Vercel-managed connector categories include Slack, GitHub, Snowflake, and Salesforce; customer-managed options include custom OAuth and static API keys.

Use Connect for:

- Salesforce sync on behalf of a team or user.
- Slack notifications or review flows.
- GitHub issue/PR/demo workflows.
- Future Attio integration through API-key or custom OAuth if no Vercel-managed connector exists.
- Multi-tenant Exa/Firecrawl/Apify keys if the product needs per-customer credentials managed outside normal env vars.

For a simple template, direct project env vars are still fine. Use Connect when the template becomes a reusable app that must safely act for different users/teams.

### Exa

Exa is the strongest first discovery layer. Its docs include a dedicated **AI SDK by Vercel** integration package, `@exalabs/ai-sdk`, exposing a `webSearch()` tool for AI SDK flows. Exa also provides MCP tools (`web_search_exa`, `web_fetch_exa`, advanced search, and async Exa Agent tools), and the Exa docs explicitly mention v0 by Vercel as an MCP client surface.

I did **not** verify a Vercel Marketplace native Exa integration page. Treat Exa as:

- first-class with AI SDK and MCP;
- not yet assumed to be Vercel Marketplace-native;
- configured by `EXA_API_KEY` or Vercel Connect API-key connector when needed.

### Firecrawl

Firecrawl is Vercel-native in the Marketplace. Installing the integration exposes `FIRECRAWL_API_KEY` to the project. Its marketplace page positions it for AI agents, web automation, search, scrape, and interact. Firecrawl should be the first extraction/scrape provider when Exa search results need reliable page content, JS-rendered content, structured data, screenshots, or interaction.

### Apify

Apify is powerful for marketplace scrapers, long-running crawls, proxy-heavy extraction, and dataset outputs. Its Vercel AI SDK integration guide shows an agent using the Apify MCP server at `https://mcp.apify.com` with `APIFY_TOKEN`, `experimental_createMCPClient`, and Vercel AI SDK `generateText()` tools. That means Apify can be surfaced as AI SDK tools in the same agent stack when a Store actor is the easiest way to get a dataset.

The lower-level interface is still the Apify API/JS client: run an Actor/task, wait or receive a webhook, then retrieve dataset items. Apify should be optional because it adds another token, another platform, and often a heavier compliance/usage story than Exa + Firecrawl. Keep Vercel AI Gateway as the default model path; Apify OpenRouter is useful if a user wants one Apify token to cover tool + model costs, but it should not replace the template's default Vercel-native model routing.

## Vercel-first architecture

```text
Vercel Template: mdp-bdr-scout
  |
  +-- Next.js app shell
  |     - dashboard: runs, candidates, ledger, briefs
  |     - settings: MDP pack, source queries, score threshold, CRM sync
  |
  +-- eve agent project
  |     - agent/instructions.md: scout behavior and boundaries
  |     - agent/tools/search.ts: Exa search tool
  |     - agent/tools/extract.ts: Firecrawl / fetch / Apify adapter
  |     - agent/tools/mdp.ts: trusted app-owned MDP adapter
  |     - agent/skills/*: evidence gathering, normalization, scoring review
  |
  +-- Vercel Cron
  |     - /api/cron/scout
  |     - starts scout workflow, returns quickly
  |
  +-- Vercel Workflow
  |     - scout-cycle workflow
  |     - retryable steps for search, extract, normalize, mdp, score, persist
  |
  +-- MDP execution layer
  |     - native packaged mdp binary if Vercel Functions can execute it reliably
  |     - Vercel Sandbox fallback for full OS CLI execution
  |     - no model-directed arbitrary shell
  |
  +-- Storage
        - Neon Postgres: canonical ledger and dedupe
        - Vercel Blob: brief artifacts, source receipts, JSONL export
```

## MDP execution strategy

The biggest technical spike is not search or scheduling; it is the `mdp` CLI on Vercel.

Implement an adapter with two execution modes:

1. **Native function mode**
   - Build step downloads or packages `mdp-x86_64-unknown-linux-gnu`.
   - Function copies pack/prospect scratch to `/tmp`.
   - Trusted app code runs `mdp --json fit`, `mdp --json brief --context`, `mdp render-brief`, and `mdp check-claims`.
   - This is fastest if Vercel packaging and function execution are reliable.

2. **Vercel Sandbox mode**
   - Workflow step creates a Vercel Sandbox microVM.
   - Writes the pack, prospect JSON, and any source receipt files.
   - Downloads or mounts the `mdp` binary.
   - Runs MDP commands in the sandbox and reads artifacts back.
   - This is slower and costs more, but it gives a full OS, root-capable isolated environment, and better parity for arbitrary CLI execution.

The implementation should start with native mode and keep sandbox mode as the escape hatch. If native mode is flaky, flip the default to sandbox.

## Workflow shape

```text
/api/cron/scout
  -> start scoutCycleWorkflow({ packId, scheduleId })
  -> return 202 { runId }

scoutCycleWorkflow
  1. loadPackConfig
  2. discoverCandidatesWithExa
  3. extractEvidenceWithFirecrawlOrApify
  4. normalizeProspectRows
  5. runMdpFitAndBrief
  6. scoreCandidate
  7. persistLedgerRow
  8. optionallyCreateReviewNotification
```

Each network/API operation should be a workflow step with clear retry behavior. Use fatal errors for schema/pack mistakes and retryable errors for 429/5xx provider failures. The cron entrypoint must also be idempotent and lock-protected because Vercel cron delivery is best-effort and can occasionally miss or duplicate an invocation.

## Data contract

### Canonical ledger row

```json
{
  "contract_version": "mdp_scout_candidate/v0",
  "run_id": "wrun_or_workflow_id",
  "pack_id": "profound-gtm-vetting",
  "candidate": {
    "name": "Jane Doe",
    "title": "VP Marketing",
    "company": "ExampleCo",
    "company_domain": "example.com",
    "linkedin_url": null,
    "source_kind": "public_web"
  },
  "evidence": [
    {
      "url": "https://example.com/news/product-launch",
      "title": "ExampleCo launches ...",
      "observed_at": "2026-07-08T00:00:00Z",
      "snippet": "bounded source-backed evidence",
      "content_hash": "sha256:...",
      "confidence": 0.78
    }
  ],
  "mdp": {
    "fit_status": "fit",
    "persona": "Marketing Leader",
    "route": "initial outbound",
    "brief_json_url": "blob://.../brief.json",
    "brief_md_url": "blob://.../brief.md",
    "gaps": []
  },
  "score": {
    "overall": 84,
    "components": {
      "mdp_fit": 30,
      "evidence_quality": 20,
      "trigger_relevance": 18,
      "persona_match": 10,
      "recency": 6
    },
    "rationale": ["Score must be explainable from MDP output and evidence receipts."]
  },
  "actions": {
    "outreach_sent": false,
    "crm_sync_status": "not_enabled"
  }
}
```

### Tables

- `scout_runs`: run id, schedule id, status, timestamps, provider usage, errors, stats.
- `lead_candidates`: deduped account/person/trigger row, score, MDP fit status, brief refs, CRM sync state.
- `evidence_sources`: source URL receipts, snippets, hashes, extractor provider, confidence.
- `ledger_exports`: JSONL/CSV/CRM export batches.
- `pack_configs`: pack id, source queries, thresholds, provider settings, schedule id.

## Scoring model

Suggested 100-point score:

- **MDP fit gate: 0 or 30** — full points only when `mdp fit` passes; no-fit candidates can be recorded but not qualified.
- **Evidence quality: 0-25** — direct source, recency, specificity, source count, source credibility.
- **Trigger relevance: 0-20** — evidence matches pack signals and "why now".
- **Persona/account match: 0-15** — title/persona/segment/domain align with MDP pack.
- **Operational confidence: 0-10** — enough data for safe brief, no major missing fields, not duplicate/stale.

Every component must include a short reason and the evidence IDs it depends on. If evidence is thin, the row should be `too_thin` rather than inflated.

## File plan for implementation

Create a deployable Vercel template example:

```text
examples/mdp-bdr-scout-vercel/
  README.md
  package.json
  tsconfig.json
  vercel.json
  next.config.ts
  app/
    page.tsx
    runs/page.tsx
    candidates/page.tsx
    api/cron/scout/route.ts
    api/runs/[runId]/route.ts
    api/candidates/route.ts
  agent/
    agent.ts
    instructions.md
    tools/search.ts
    tools/extract.ts
    tools/mdp.ts
    tools/ledger.ts
    skills/scout-evidence/SKILL.md
    skills/normalize-prospect/SKILL.md
  workflows/
    scout-cycle.ts
  src/
    mdp/runner.ts
    mdp/native-runner.ts
    mdp/sandbox-runner.ts
    providers/exa.ts
    providers/firecrawl.ts
    providers/apify.ts
    scoring/score-candidate.ts
    storage/db.ts
    storage/blob.ts
    schemas/candidate.ts
    schemas/ledger.ts
  samples/
    seed-queries.jsonl
    public-source-fixture.json
    candidate-ledger-row.json
```

Then mark the existing example as legacy or replace it:

```text
examples/profound-gtm-vetting/flue-webhook-agent/README.md
```

Add a short note that the webhook draft example is historical webhook-adapter context, while `examples/mdp-bdr-scout-vercel/` is the current demo path.

## Environment variables and marketplace resources

Required for template users:

```bash
EXA_API_KEY=...
AI_GATEWAY_API_KEY=... # optional if Vercel OIDC is used in deployed project
DATABASE_URL=...       # Neon marketplace integration
BLOB_READ_WRITE_TOKEN=...
```

Recommended / optional:

```bash
FIRECRAWL_API_KEY=...      # Vercel Native marketplace integration
APIFY_TOKEN=...            # optional Apify MCP / Vercel AI SDK / Actor API fallback
MDP_RUNNER_MODE=native     # native | sandbox
SCOUT_MIN_SCORE=70
SCOUT_MAX_CANDIDATES=25
CRM_SYNC_ENABLED=false
SALESFORCE_CONNECTOR_ID=... # Vercel Connect
ATTIO_CONNECTOR_ID=...      # Vercel Connect custom/API-key connector if configured
OLLAMA_BASE_URL=...         # local/dev fallback only, not default Vercel production path
OLLAMA_API_KEY=...
```

For production Vercel, use AI Gateway as the default. Keep Ollama as a local/developer fallback because self-hosted Ollama is not a natural Vercel template default.

## Implementation sequence

1. **Create Vercel template spike**
   - Scaffold `examples/mdp-bdr-scout-vercel/` from an eve or Next.js + Workflow template.
   - Add `vercel.json` crons and `fluid: true`.
   - Add a README with one-click deployment assumptions.

2. **Spike MDP CLI execution on Vercel**
   - Try native packaged binary first.
   - If unreliable, implement Vercel Sandbox runner and make it default.
   - Document binary packaging/build steps clearly.

3. **Implement discovery providers**
   - Exa first: AI SDK tool and direct API wrapper.
   - Firecrawl second: Vercel Marketplace env var + search/scrape/interact wrapper.
   - Apify optional: Vercel AI SDK/MCP tools for agent use plus Actor run + dataset retrieval wrapper.

4. **Implement workflow + ledger**
   - Cron route starts workflow.
   - Workflow writes run/candidate/evidence/brief rows.
   - Dashboard lists runs and candidates.

5. **Implement MDP gates and scoring**
   - Normalize prospect JSON.
   - Run `mdp fit` and `mdp brief --context`.
   - Render Markdown brief.
   - Claim-check any generated narrative.
   - Score with evidence-bound component rationale.

6. **Add Vercel Connect sync points**
   - Salesforce connector as first CRM sync candidate.
   - Attio via API-key/custom OAuth connector if available or via direct env var if not.
   - Keep sync disabled by default.

7. **Publish template-ready docs**
   - One-click deploy path.
   - Required integrations and env vars.
   - How to upload/create an MDP pack.
   - How to inspect Observability, Agent Runs, and AI Gateway usage.

## Validation plan

Minimum gates for the implementation PR:

```bash
npm install
npm run typecheck
npm run scout:sample
mdp --json doctor --dir .
mdp --json validate --dir <chosen-demo-pack>
mdp --json fit --dir <chosen-demo-pack> --prospect <chosen-demo-pack>/examples/<sample-prospect>.json
```

Known constraint on 2026-07-08: the current committed example packs fail the installed `mdp 0.1.34` prompt-output validation checks because several extraction prompt examples need declared `inputs_used` values and/or explicit output schemas. The implementation PR should either repair the selected pack's prompt contracts first or use a new demo pack that passes validation before making it the scout default.

Vercel-specific validation:

```bash
vercel build
# Cron jobs are API routes; test locally or against a preview/prod URL.
curl http://localhost:3000/api/cron/scout
# If CRON_SECRET is enabled, include: -H "Authorization: Bearer $CRON_SECRET"
```

If Rust tooling is available in the environment, also run:

```bash
cargo test --manifest-path cli/Cargo.toml
```

## Open questions

1. Should the template start from eve directly, or from the Lead Agent / Workflow DevKit template and add eve only after the first working scout?
2. Should native MDP binary execution be a hard requirement, or is Vercel Sandbox acceptable as the default MDP runner?
3. Which first pack should power the public demo: MDP-for-MDP, Profound, Orchid, or a new synthetic GTM engineering pack?
4. Should v1 score account-level leads, person-level leads, or both?
5. Should Salesforce be the first Vercel Connect CRM sync, with Attio as an API-key connector fallback?
6. Should Apify be shown in the default UX, or hidden under "advanced extraction" until users need it?

## Source notes

- Vercel eve: https://vercel.com/docs/eve
- Vercel Workflow: https://vercel.com/docs/workflows and https://useworkflow.dev
- Vercel Cron Jobs: https://vercel.com/docs/cron-jobs
- Vercel Fluid Compute: https://vercel.com/docs/fluid-compute
- Vercel AI Gateway: https://vercel.com/docs/ai-gateway
- Vercel Sandbox: https://vercel.com/docs/sandbox
- Vercel Connect: https://vercel.com/docs/connect
- Vercel Observability: https://vercel.com/docs/observability
- Vercel Templates: https://vercel.com/templates
- Firecrawl Vercel Marketplace: https://vercel.com/marketplace/firecrawl
- Exa Search API: https://exa.ai/docs/reference/search-api-guide
- Exa AI SDK by Vercel: https://exa.ai/docs/reference/vercel
- Exa MCP: https://exa.ai/docs/reference/exa-mcp
- Firecrawl docs: https://docs.firecrawl.dev/introduction
- Apify Vercel AI SDK integration: https://docs.apify.com/integrations/vercel-ai-sdk
- Apify API integration: https://docs.apify.com/academy/api/run-actor-and-retrieve-data-via-api
- JustBash: https://justbash.dev/
- Ollama OpenAI compatibility: https://docs.ollama.com/api/openai-compatibility
