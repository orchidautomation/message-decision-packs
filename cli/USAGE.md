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
  cards/output-rules.yaml
  cards/copy-patterns.yaml
  cards/ctas.yaml
  cards/objections.yaml
  cards/gaps.yaml
  evals/*.yaml
  examples/
  clay-row.json
```

The starter fixture path is kept for compatibility. It is a synthetic provider-neutral prospect/source row, not a Clay dependency.

Quick demo:

```bash
mdp --json init --template gtm --name "Example Message Pack" --dir /tmp/mdp-demo --force
mdp --json validate --dir /tmp/mdp-demo
mdp --json --summary route --entries --eval-fixture --dir /tmp/mdp-demo --persona "PMM" --job "linkedin outbound copy"
mdp sample-leads --dir /tmp/mdp-demo --persona "PMM" --job "initial email outbound copy" --count 3 --format yaml
mdp --json fit --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json
mdp --json --summary brief --context --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin --out /tmp/mdp-demo/.mdp/briefs/example-linkedin.json
mdp --json check-claims --dir /tmp/mdp-demo --text "MDP is a local offline CLI for modular message context."
mdp --json check-claims --dir /tmp/mdp-demo --text "<draft copy>" --subject "<subject>" --persona "PMM" --job "initial email outbound message"
mdp --json gaps --dir /tmp/mdp-demo
mdp --json eval --dir /tmp/mdp-demo
mdp --json copy --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin
```

Use `brief` for production handoff. Add `--out <path>` when the brief should be saved; otherwise the artifact is stdout-only. Use `copy` only for local demos. Source inventory lives in `.mdp/sources.yaml`, reusable extraction prompts live in `.mdp/prompts/*.yaml`, CTA guidance lives in `cards/ctas.yaml`, channel rules live in `cards/channel-policies.yaml`, approved claims live in `cards/claims.yaml`, global style and structure rules live in `cards/output-rules.yaml`, and durable unknowns live in `cards/gaps.yaml`. Entries can use `avoid` for blocked literals, `exact_paragraphs` for fixed paragraph counts, and `constraints` for deterministic output limits such as word count, subject word count, subject avoid literals, max questions, and forbidden links, attachments, images, HTML, or tracking.

## JSON contract

All commands support `--json`; add `--summary` for compact status output. Validation-style commands return structured data and exit nonzero when `data.valid` is false. Argument parse errors also return JSON when `--json` is present.

Use `mdp --json schema prompt` to inspect the reusable prompt contract. Prompt outputs use `contract: mdp.prompt-output.v0` and must match the contract named by each prompt's `output_contract.schema_ref`; starter prompts can inline the full JSON Schema with `mdp init --include-output-schemas`. Extraction prompts preserve `card_patches`, `gaps`, `rejected_claims`, confidence, and provenance; normalization prompts preserve `normalized_prospect`, `normalization_trace`, gaps, and empty `card_patches`. Prompt files are local decision contracts, not browsing, scraping, enrichment, sending, sequencing, or CRM-update workflows.

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
2. If outbound-copy testing needs lead-specific inputs and no real or sanitized prospect row was supplied, generate 2 to 5 fake fixtures:

```bash
mdp sample-leads --dir . --persona "PMM" --job "initial email outbound copy" --count 3 --format yaml
```

3. Convert the supplied user note, CSV, CRM export, Clay, Deepline, spreadsheet, or other source row into `mdp schema prospect`. Use explicit `persona` when known; otherwise `.mdp/manifest.yaml` can define `persona_mappings` from title keywords to pack personas. For fixture testing, save one generated row to ignored scratch before passing it as `--prospect`.
4. Run `mdp --json fit --prospect <row.json>` and stop if it returns `disqualified` or `insufficient-context`.
5. Run `mdp --json --summary brief --context --prospect <row.json> --channel linkedin --out .mdp/briefs/<brief-name>.json` when a durable brief file is needed.
6. Stop if `data.draft_status` is `no-draft`.
7. Draft from `data.context.entries` first; for generated fixtures, draft against `safe_personalization` and `known_gaps` and never imply the fixture is a real prospect. Open `data.context.full_card_required` paths only when present.
8. Run `mdp --json check-claims` before approval; it reports unsupported claims plus avoid-rule, output-rule, and hard structured-constraint guardrail hits. Include `--subject`, `--persona`, and `--job` when checking routed subject or channel constraints. Target-range misses appear in `constraint_warnings`; actual attachments, embedded images, and send-surface tracking may appear in `unchecked_constraints` because they cannot be proven from a single draft body.

Generated starter rows and `sample-leads` rows are synthetic examples. They include `source_kind: synthetic-example`, `synthetic: true`, and must not be presented as real prospects. Production rows can come from a user note, CSV, CRM export, Clay, Deepline, spreadsheet, or research workflow after they are normalized into MDP prospect JSON.

Direct persona/job commands resolve pack-owned persona aliases before routing. Check `requested_persona` and `persona_resolution` in JSON output when the route used an alias.

`mdp` is not a sender, CRM, sequencer, lead enricher, scraper, or AI SDR. It is the local decision contract layer.
