# MDP BDR Scout Agent Stack Plan

Date: 2026-07-08
Status: proposed
Owner: Brandon / Orchid
Repo: message-decision-packs

## Goal

Build a demoable "BDR Scout powered by Message Decision Packs" that wakes up on a schedule, searches public sources for candidate leads or conversations, gathers evidence, normalizes the candidate into an MDP-compatible prospect row, runs MDP fit/route/brief gates, computes a normalized score, and appends a ledger row that can later sync to Attio, Salesforce, or another CRM.

This should showcase MDP as the deterministic decision/context layer. The wrapper can be an AI BDR scout, but MDP itself remains a local/offline pack + CLI standard, not a sender, CRM, scraper, enrichment vendor, sequencer, or AI SDR.

## Recommendation

Use this stack for v1:

1. **Flue on Node.js** for the agent/workflow harness.
2. **Railway Cron Job + Railway Postgres** for the first deployed scheduler and durable ledger.
3. **MDP CLI** as trusted application code for `fit`, `brief --context`, `render-brief`, `check-claims`, `gaps`, and schema validation.
4. **JustBash / Flue virtual sandbox** only for lightweight model-visible workspace work.
5. **Ollama via OpenAI-compatible API** as the default configurable local/remote model endpoint.
6. **Search/extraction tools behind narrow application tools or MCP**: start with Exa or Tavily for search and Firecrawl for clean page extraction; add Browserbase only when a real browser is required.
7. **JSONL export plus Postgres canonical ledger**: JSONL makes the demo inspectable; Postgres gives idempotency, dedupe, and CRM sync safety.

Use **Vercel eve + Vercel Workflows/Cron/Sandbox** as the Vercel-native v2 path, not the first implementation, unless the demo must be branded as fully Vercel-native from day one.

## Why this stack

The desired product loop is a bounded recurring operation, not an infinite shell session. A cron-triggered Flue workflow maps cleanly to: discover candidates, collect evidence, normalize, call MDP, score, write ledger, exit. Railway Cron Jobs are designed for services that run, finish, close resources, and exit. Railway also gives a normal container environment where the Rust `mdp` binary and Node dependencies can coexist without serverless packaging friction.

Flue is the cleanest agent-forward layer for v1 because it already treats workflows as finite inspectable background operations, supports tools, skills, MCP connections, and a JustBash-powered virtual sandbox, while still running on ordinary Node. It lets the application own credentials and authorization boundaries, while the model works through bounded tools.

Vercel eve is compelling, but it is beta and bundles a stronger platform choice: Vercel Functions, Workflows, Sandbox, AI Gateway, Connect, and dashboard observability. That is excellent for a polished Vercel-native follow-on, but heavier than needed while the demo is still proving the MDP agent pattern.

## Technology roles

| Technology | Use in this project | Recommendation |
| --- | --- | --- |
| Flue | Agent workflow harness, tools, skills, MCP, workflow filesystem, Node target | **Use for v1** |
| Railway Cron | Scheduled deployed worker that starts, does bounded work, exits | **Use for v1 deploy** |
| Railway Postgres | Canonical ledger, run state, dedupe keys, CRM sync state | **Use for v1 deploy** |
| MDP CLI | Deterministic pack-owned gates and brief context | **Core requirement** |
| JustBash | Lightweight virtual bash/file workspace inside Flue; useful for model-visible scratch, CSV/JSON transforms | **Use narrowly** |
| Ollama | Local/remote OpenAI-compatible model endpoint | **Use as default configurable provider** |
| Exa or Tavily | Search candidate discovery and source finding | **Pick one for v1; keep provider interface swappable** |
| Firecrawl | URL scrape/extract to markdown/structured JSON; optional MCP path | **Use for extraction first** |
| Browserbase / Stagehand | Real browser sessions, dynamic pages, logged-in or JS-heavy flows | **Defer until needed** |
| Vercel eve | Filesystem-first durable backend AI agent framework on Vercel | **Use for Vercel-native v2** |
| Vercel Workflows/Cron/Sandbox | Durable workflow, time trigger, full microVM sandbox | **Use with eve or Vercel-native alternate** |
| Daytona | Fast full-computer sandbox for isolated OS/toolchain workloads | **Use as remote sandbox only when needed** |
| Crabbox | Local-loop remote testbox/execution control plane | **Use for dev/proof runs, not production scout runtime** |
| Blacksmith Testboxes | Agent-first CI microVMs inside GitHub Actions jobs | **Use for CI/dev validation, not production scout runtime** |
| Secure Exec | Lightweight secure Node.js code execution via V8 isolates | **Optional future for model-written JS snippets** |
| Rivet Actors / agentOS | Long-lived agents with durable state; agent runtime experiments | **Future always-on mode, not v1** |

## Product boundaries

### In scope

- Public-source lead/conversation discovery.
- Evidence capture with source URLs, titles, hashes, timestamps, and bounded snippets.
- Candidate normalization into MDP-compatible prospect JSON.
- MDP fit, routing, brief, claim-check, gaps, and route explanation.
- Normalized score and rationale.
- Append-only ledger rows and optional CRM-ready export.
- Human-readable brief files for qualified candidates.
- Dry-run mode with sample search fixtures.

### Out of scope for v1

- Sending outreach.
- Sequencer writes.
- Automated CRM mutation by default.
- Scraping prohibited sources or bypassing access controls.
- Private LinkedIn scraping.
- Buying enrichment data by default.
- Treating MDP as the hosted scout service itself.

## Architecture

```text
Railway Cron or local CLI
  |
  v
Flue workflow: scout-cycle
  |
  +-- source discovery tools
  |     - search provider query
  |     - optional RSS/news/source-specific feeds
  |
  +-- evidence extraction tools
  |     - Firecrawl / fetch / Browserbase when needed
  |     - receipts: url, title, fetched_at, quote/snippet, hash, confidence
  |
  +-- candidate normalizer
  |     - LLM produces strict candidate JSON
  |     - schema validation rejects unsupported fields
  |
  +-- MDP deterministic gates
  |     - mdp fit
  |     - mdp brief --context
  |     - mdp render-brief
  |     - mdp check-claims for generated summaries
  |
  +-- scorer
  |     - normalized 0-100 score
  |     - component rationale bound to evidence and MDP output
  |
  +-- ledger writer
        - Postgres canonical rows
        - JSONL export for demo/readability
        - optional CRM sync state, disabled by default
```

## Runtime shape

### Local demo command

```bash
npm run scout:sample
npm run scout -- --pack examples/profound-gtm-vetting --query "AI search monitoring teams hiring" --limit 10
```

### Railway production command

```bash
npm run scout:cycle
```

The process should terminate after each run. If the previous cron execution is still active, Railway skips the new execution, so the worker must close DB connections and exit cleanly.

### Future always-on mode

Do not start with a never-ending loop. Model "always looking" as frequent bounded cycles with persisted state. If the product needs live continuous attention later, move the same state machine into Rivet Actors or a persistent Flue agent session.

## Data contract

### Canonical ledger row

```json
{
  "contract_version": "mdp_scout_candidate/v0",
  "run_id": "scout_2026_07_08_001",
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
    "brief_json_path": ".agent-artifacts/scout/runs/.../brief.json",
    "brief_md_path": ".agent-artifacts/scout/runs/.../brief.md",
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

### Postgres tables

- `scout_runs`: one row per scheduled run, config, status, timing, stats, error summary.
- `lead_candidates`: deduped candidate rows, normalized person/company fields, score, MDP fit status, brief refs, CRM sync state.
- `evidence_sources`: one-to-many source receipts for each candidate.
- `ledger_exports`: export batches for JSONL/CSV/CRM.

### Dedupe key

Use a stable key such as:

```text
lower(company_domain) + ":" + normalized_person_name_or_title + ":" + source_url_hash
```

When person identity is unknown, dedupe at account + trigger/source level and mark the candidate as account-level.

## Scoring model

A normalized score should be deterministic enough to compare candidates across runs while still explaining uncertainty.

Suggested 100-point score:

- **MDP fit gate: 0 or 30** — full points only when `mdp fit` passes; no-fit candidates can be recorded but not qualified.
- **Evidence quality: 0-25** — direct source, recency, specificity, source count, source credibility.
- **Trigger relevance: 0-20** — evidence matches pack signals and "why now".
- **Persona/account match: 0-15** — title/persona/segment/domain align with MDP pack.
- **Operational confidence: 0-10** — enough data for safe brief, no major missing fields, not duplicate/stale.

Every component must include a short reason and the evidence IDs it depends on. If evidence is thin, the row should be `too_thin` rather than inflated.

## File plan for implementation

Create a new example instead of extending the current webhook scaffold:

```text
examples/mdp-bdr-scout/
  README.md
  package.json
  tsconfig.json
  flue.config.ts
  sample.env
  samples/
    seed-queries.jsonl
    public-source-fixture.json
    candidate-ledger-row.json
  src/
    app.ts
    workflows/scout-cycle.ts
    tools/search.ts
    tools/extract.ts
    tools/mdp.ts
    tools/ledger.ts
    tools/crm-export.ts
    scoring/score-candidate.ts
    schemas/candidate.ts
    schemas/ledger.ts
    providers/ollama.ts
    providers/search-provider.ts
  skills/
    scout-evidence/SKILL.md
    normalize-prospect/SKILL.md
```

Then mark the existing example as legacy or replace it:

```text
examples/profound-gtm-vetting/flue-webhook-agent/README.md
```

Add a short note that the webhook draft example is retained for historical webhook adapter context, while `examples/mdp-bdr-scout/` is the current demo path.

## Environment variables

```bash
MDP_BIN=/usr/local/bin/mdp
MDP_PACK_DIR=examples/profound-gtm-vetting
DATABASE_URL=postgres://...
LEDGER_JSONL_PATH=.agent-artifacts/scout/ledger.jsonl

LLM_PROVIDER=ollama
OLLAMA_BASE_URL=http://localhost:11434/v1/
OLLAMA_API_KEY=ollama
OLLAMA_MODEL=gpt-oss:20b

SEARCH_PROVIDER=exa # or tavily
EXA_API_KEY=...
TAVILY_API_KEY=...
FIRECRAWL_API_KEY=...
BROWSERBASE_API_KEY=...

SCOUT_QUERY_LIMIT=10
SCOUT_MAX_CANDIDATES=25
SCOUT_MIN_SCORE=70
CRM_SYNC_ENABLED=false
ATTIO_API_KEY=...
SALESFORCE_CLIENT_ID=...
```

For local Ollama, OpenAI-compatible clients require an `apiKey` value but Ollama ignores it. For a hosted Ollama-compatible endpoint, treat the key as a normal secret and never write it to logs or artifacts.

## Implementation sequence

1. **Retire the old demo path**
   - Add a README note to `examples/profound-gtm-vetting/flue-webhook-agent/` saying it is legacy.
   - Link to the new scout example.

2. **Scaffold `examples/mdp-bdr-scout/`**
   - Node + Flue target.
   - Sample fixtures that run without paid APIs.
   - Contract-only mode that calls MDP and writes ledger rows without model/search credentials.

3. **Implement provider interfaces**
   - `SearchProvider.search(query)` returns URLs and snippets.
   - `Extractor.extract(url)` returns markdown/text, metadata, hash, and retrieval status.
   - `ModelProvider.generateStructured()` targets Ollama/OpenAI-compatible endpoints first.

4. **Implement MDP adapter**
   - Write normalized prospect JSON to ignored scratch.
   - Run `mdp --json fit`.
   - Run `mdp --json --summary brief --context`.
   - Render a Markdown brief for humans.
   - Run claim check for any generated prose.

5. **Implement scoring + ledger**
   - Score from MDP result + evidence receipts.
   - Append JSONL.
   - Insert/update Postgres with dedupe key.
   - Keep `CRM_SYNC_ENABLED=false` by default.

6. **Add deployment docs**
   - Local sample.
   - Railway Cron setup.
   - Optional Vercel/eve alternate path.
   - Secret handling and rate-limit guidance.

7. **Add tests**
   - Schema validation fixtures.
   - Scoring fixtures.
   - MDP adapter contract tests with sample pack.
   - Ledger idempotency tests.
   - Contract-only end-to-end sample run.

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

If Rust tooling is available in the environment, also run:

```bash
cargo test --manifest-path cli/Cargo.toml
```

## Vercel-native alternative

If the demo must be Vercel-first, use:

- Vercel eve for the agent project structure.
- Vercel Cron to trigger the scout cycle.
- Vercel Workflows for durable multi-step state.
- Vercel Sandbox for any full OS code execution.
- Vercel AI Gateway for model routing.
- Vercel Connect for external credentials.
- Vercel Observability for run traces.

This is more integrated and demo-friendly for a Vercel audience, but it is beta and less portable than the Flue + Railway v1.

## Open questions

1. Which first customer/company pack should power the demo: MDP-for-MDP, Profound, Orchid, or a new synthetic BDR pack?
2. Which source class should v1 watch: company news, hiring pages, founder posts, GitHub activity, Reddit/HN, podcast/news mentions, or search-query batches?
3. Should v1 sync to Attio first, Salesforce first, or only emit a CRM-ready file?
4. Should the demo score account-level leads, person-level leads, or both?
5. Which model should be the Ollama default for structured extraction and which should be used for brief prose?

## Source notes

- Flue docs: https://flueframework.com/docs/getting-started/quickstart/, https://flueframework.com/docs/guide/workflows/, https://flueframework.com/docs/guide/tools/, https://flueframework.com/docs/guide/sandboxes/, https://flueframework.com/docs/guide/schedules/
- Vercel docs: https://vercel.com/docs/eve, https://vercel.com/docs/workflows, https://vercel.com/docs/cron-jobs, https://vercel.com/docs/sandbox
- Railway Cron docs: https://docs.railway.com/cron-jobs
- JustBash docs: https://justbash.dev/ and https://github.com/vercel-labs/just-bash
- Daytona docs: https://www.daytona.io/docs/
- Crabbox docs: https://crabbox.sh/
- Blacksmith Testboxes docs: https://docs.blacksmith.sh/blacksmith-testbox/overview
- Rivet / Secure Exec / agentOS docs: https://rivet.dev/docs/actors/, https://secureexec.dev/, https://agentos-sdk.dev/
- Ollama OpenAI compatibility: https://docs.ollama.com/api/openai-compatibility
- Search/extraction candidates: https://exa.ai/docs/reference/getting-started, https://docs.tavily.com/documentation/api-reference/endpoint/search, https://docs.firecrawl.dev/introduction, https://docs.browserbase.com/welcome/introduction
