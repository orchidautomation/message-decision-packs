> Archived example: retained for historical context only. The active deployable template is `examples/ai-sdr-eve-vercel/`.

# Profound MDP Flue Webhook Agent

> Legacy note: this Flue webhook draft remains useful historical adapter context, but the current turnkey deployable path is `examples/ai-sdr-eve-vercel/`.

This is a runnable Flue example for operationalizing the Profound MDP pack without turning MDP into hosted execution infrastructure.

It accepts a webhook-style prospect row, writes the raw payload and normalized prospect JSON to local ignored scratch, runs the `mdp` CLI for fit and `brief --context`, stages `brief.json` inside the Flue workflow filesystem, and optionally asks a model to draft a response. It does not send, schedule, enrich, scrape, update a CRM, or write to a sequencer.

## Why Flue Here

Flue is the better immediate fit for this repo because the core operation is local and file-oriented: accept JSON, write a prospect file, run the local `mdp` CLI, stage brief artifacts, and return a draft or draft contract. The active Eve template has since become the Vercel-first option. Keep this archive only for webhook-adapter design context.

## Run Locally

From this directory:

```bash
npm install
cargo build --manifest-path ../../../cli/Cargo.toml
MDP_BIN=../../../cli/target/debug/mdp npm run draft:sample
```

The sample uses `draftMode: "contract-only"`, so it validates the webhook-to-CLI path without requiring model provider setup.

For model drafting, change `draftMode` in `sample-webhook.json` to `"model"` and configure the provider required by the model in `src/workflows/draft-response.ts`.

## Webhook Route

Start the dev server:

```bash
MDP_BIN=../../../cli/target/debug/mdp npm run dev
```

Then send the sample payload to the application-owned webhook route using the URL printed by Flue:

```bash
curl -X POST "http://127.0.0.1:3583/webhooks/mdp/prospect" \
  -H "content-type: application/json" \
  --data @sample-webhook.json
```

Add request verification middleware before using this route outside local development.

## Input Contract

The workflow accepts one of these shapes:

- `{ "prospect": { ... } }`
- `{ "row": { ... } }`
- `{ "data": { "prospect": { ... } } }`
- `{ "data": { "row": { ... } } }`
- a top-level prospect row containing `name`, `title`, and `company`

Preferred prospect fields are the same as `mdp --json schema prospect`: `name`, `title`, `company`, `company_url`, `linkedin_url`, `background`, `trigger`, `persona`, `segment`, `signals`, `source_kind`, and `synthetic`.

## Artifacts

Each run writes ignored local scratch under the Profound pack directory:

```text
<ignored-scratch>/flue-webhook-agent/<delivery-id>/
  webhook.json
  prospect.json
  brief.json
  draft-prompt.md
  draft.txt
  claim-check.json
```

Do not commit production scratch because webhook payloads can contain private prospect or customer data.
