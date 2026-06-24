# mdp usage

`mdp` creates and routes Message Decision Packs.

A pack is a local `.mdp/` folder:

```text
.mdp/
  manifest.yaml
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
  evals/linkedin-copy-route.yaml
examples/
  clay-row.json
```

Quick demo:

```bash
mdp --json init --template gtm --name "Example Message Pack" --dir /tmp/mdp-demo --force
mdp --json validate --dir /tmp/mdp-demo
mdp --json route --entries --dir /tmp/mdp-demo --persona "PMM" --job "linkedin outbound copy"
mdp --json fit --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json
mdp --json brief --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin
mdp --json check-claims --dir /tmp/mdp-demo --text "MDP is a local offline CLI for modular message context."
mdp --json gaps --dir /tmp/mdp-demo
mdp --json eval --dir /tmp/mdp-demo
mdp --json copy --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin
```

Use `brief` for production handoff. Use `copy` only for local demos. CTA guidance lives in `cards/ctas.yaml`, channel rules live in `cards/channel-policies.yaml`, approved claims live in `cards/claims.yaml`, and durable unknowns live in `cards/gaps.yaml`.

## JSON contract

All commands support `--json`.

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
2. Convert the Clay, Deepline, CSV, or enrichment row into `mdp schema prospect`.
3. Run `mdp --json fit --prospect <row.json>` and stop if it returns `disqualified` or `insufficient-context`.
4. Run `mdp --json brief --prospect <row.json> --channel linkedin`.
5. Read only the files in `data.required_load_order`.
6. Draft from the brief plus routed cards, then run `mdp --json check-claims` before approval.

`mdp` is not a sender, CRM, sequencer, lead enricher, scraper, or AI SDR. It is the local decision contract layer.
