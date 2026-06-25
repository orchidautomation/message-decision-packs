# mdp usage

`mdp` creates and routes Message Decision Packs.

A pack is a local `.mdp/` folder:

```text
.mdp/
  manifest.yaml
  sources.yaml
  briefs/
  prompts/*.yaml
  cards/personas.yaml
  cards/positioning.yaml
  cards/fit-rules.yaml
  cards/signals.yaml
  cards/pains.yaml
  cards/claims.yaml
  cards/motions.yaml
  cards/channel-policies.yaml
  cards/hooks.yaml
  cards/avoid-rules.yaml
  cards/copy-patterns.yaml
  cards/ctas.yaml
  cards/objections.yaml
  cards/gaps.yaml
  evals/*.yaml
examples/
  clay-row.json
```

Quick demo:

```bash
mdp --json init --template gtm --name "Example Message Pack" --dir /tmp/mdp-demo --force
mdp --json validate --dir /tmp/mdp-demo
mdp --json --summary route --entries --eval-fixture --dir /tmp/mdp-demo --persona "PMM" --job "linkedin outbound copy"
mdp --json fit --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json
mdp --json --summary brief --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin --out /tmp/mdp-demo/.mdp/briefs/example-linkedin.json
mdp --json check-claims --dir /tmp/mdp-demo --text "MDP is a local offline CLI for modular message context."
mdp --json gaps --dir /tmp/mdp-demo
mdp --json eval --dir /tmp/mdp-demo
mdp --json copy --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin
```

Use `brief` for production handoff. Add `--out <path>` when the brief should be saved; otherwise the artifact is stdout-only. Use `copy` only for local demos. Source inventory lives in `.mdp/sources.yaml`, reusable extraction prompts live in `.mdp/prompts/*.yaml`, CTA guidance lives in `cards/ctas.yaml`, channel rules live in `cards/channel-policies.yaml`, approved claims live in `cards/claims.yaml`, and durable unknowns live in `cards/gaps.yaml`.

## JSON contract

All commands support `--json`; add `--summary` for compact status output. Validation-style commands return structured data and exit nonzero when `data.valid` is false. Argument parse errors also return JSON when `--json` is present.

Use `mdp --json schema prompt` to inspect the reusable extraction prompt contract. Prompt outputs use `contract: mdp.prompt-output.v0` and must preserve `card_patches`, `gaps`, `rejected_claims`, confidence, and provenance. Prompt files classify supplied person, company, account, domain, row, or research data into candidate MDP entries. They are local decision contracts, not browsing, scraping, enrichment, sending, or CRM-update workflows.

Success:

```json
{"ok": true, "command": "route", "data": {}}
```

Error:

```json
{"ok": false, "error": {"code": "mdp_error", "message": "message", "details": []}}
```

## Agent handoff

1. Run `mdp --json doctor` and `mdp --json validate`.
2. Convert the Clay, Deepline, CSV, or enrichment row into `mdp schema prospect`. Use explicit `persona` when known; otherwise `.mdp/manifest.yaml` can define `persona_mappings` from title keywords to pack personas.
3. Run `mdp --json fit --prospect <row.json>` and stop if it returns `disqualified` or `insufficient-context`.
4. Run `mdp --json --summary brief --prospect <row.json> --channel linkedin --out .mdp/briefs/<brief-name>.json` when a durable brief file is needed.
5. Stop if `data.draft_status` is `no-draft`.
6. Read only the files in `data.required_load_order`.
7. Draft from the brief plus routed cards, then run `mdp --json check-claims` before approval.

Generated starter rows are synthetic examples. They include `source_kind: synthetic-example` and `synthetic: true`; do not present them as real prospects.

`mdp` is not a sender, CRM, sequencer, lead enricher, scraper, or AI SDR. It is the local decision contract layer.
