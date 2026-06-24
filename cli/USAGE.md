# mdp usage

`mdp` creates and routes Message Decision Packs.

A pack is a local `.mdp/` folder:

```text
.mdp/
  manifest.yaml
  cards/personas.yaml
  cards/pains.yaml
  cards/motions.yaml
  cards/hooks.yaml
  cards/avoid-rules.yaml
  cards/copy-patterns.yaml
  cards/ctas.yaml
examples/
  clay-row.json
```

Quick demo:

```bash
mdp --json init --template rillet --name "Rillet Message Pack" --dir /tmp/mdp-rillet-demo --force
mdp --json validate --dir /tmp/mdp-rillet-demo
mdp --json route --dir /tmp/mdp-rillet-demo --persona "VP Finance" --job "linkedin outbound copy"
mdp --json brief --dir /tmp/mdp-rillet-demo --prospect /tmp/mdp-rillet-demo/examples/clay-row.json --channel linkedin
mdp --json copy --dir /tmp/mdp-rillet-demo --prospect /tmp/mdp-rillet-demo/examples/clay-row.json --channel linkedin
```

Use `brief` for production handoff. Use `copy` only for local demos. CTA guidance lives in `cards/ctas.yaml` and should route for outbound/message/copy jobs.

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
3. Run `mdp --json brief --prospect <row.json> --channel linkedin`.
4. Read only the files in `data.required_load_order`.
5. Draft from the brief plus routed cards.

`mdp` is not a sender, CRM, sequencer, lead enricher, scraper, or AI SDR. It is the local decision contract layer.
