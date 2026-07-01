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
mdp --json capabilities
mdp --json init --template gtm --name "Example Message Pack" --dir /tmp/mdp-demo --force
mdp --json init --template gtm --name "Example Message Pack" --dir /tmp/mdp-demo --dry-run
mdp --json validate --dir /tmp/mdp-demo
mdp --json validate-prompt-output --dir /tmp/mdp-demo --prompt-id extract-claims-proof --file /tmp/claims-output.json
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

All commands support `--json`; add `--summary` for compact status output. Run `mdp --json capabilities` when an agent or wrapper needs to inspect command names, coarse side effects, output contracts, `--out` support, dry-run support, strict-mode support, and stable error codes. Validation-style commands return structured data and exit nonzero when `data.valid` is false. Argument parse errors also return JSON when `--json` is present.

Selected write paths support `--dry-run` so agents can inspect local file writes before mutating a pack:

```bash
mdp --json init --name "Message Pack" --dir . --dry-run
mdp --json brief --context --dir . --prospect <prospect.json> --channel linkedin --out .mdp/briefs/example.json --dry-run
mdp --json emit-brief --dir . --persona "PMM" --job "linkedin outbound copy" --out .mdp/briefs/route.json --dry-run
mdp --json pack --dir . --out /tmp/mdp-pack.json --dry-run
```

Use `--strict` on validation/checking flows when warnings should fail an agent or CI gate:

```bash
mdp --json validate --strict --dir .
mdp --json validate-prompt-output --strict --dir . --prompt-id extract-claims-proof --file /tmp/claims-output.json
mdp --json check-claims --strict --dir . --text "<draft copy>" --subject "<subject>" --persona "PMM" --job "initial email outbound message"
mdp --json eval --strict --dir .
```

JSON errors use stable top-level codes where the CLI can classify the failure: `pack_not_found`, `invalid_manifest`, `missing_card`, `unsupported_claim`, `insufficient_context`, `write_conflict`, `invalid_argument`, and fallback `mdp_error`.

Use `mdp --json schema prompt` to inspect the reusable prompt contract. Prompt outputs use `contract: mdp.prompt-output.v0` and must match the contract named by each prompt's `output_contract.schema_ref`; starter prompts can inline the full JSON Schema with `mdp init --include-output-schemas`. Extraction prompts preserve `card_patches`, `gaps`, `rejected_claims`, confidence, and provenance; normalization prompts preserve `normalized_prospect`, `normalization_trace`, gaps, and empty `card_patches`. Prompt files are local decision contracts, not browsing, scraping, enrichment, sending, sequencing, or CRM-update workflows.

Treat model-produced prompt output as untrusted review input. Run `mdp --json validate-prompt-output` before copying reviewed `card_patches` into cards or saving `normalized_prospect` for `mdp fit` and `mdp brief`. The validator rejects markdown-wrapped JSON, wrong prompt identity, undeclared input references, wrong card kinds, fake-person normalization, and candidate ID collisions with existing card entries.

Prospect input keeps a compatibility path for `name`, `title`, and `company`, but new lead workflows should prefer `company_domain` as the account key. `mdp fit` canonicalizes supplied domain-like values such as `https://www.apple.com/` to `apple.com`; it does not infer a domain from a company name. Packs can declare deterministic readiness requirements in `manifest.yaml`:

```yaml
lead_input_requirements:
  required_fields:
    - name
    - title
    - company_domain
    - trigger
    - persona
    - segment
    - signals
  required_signal_fields:
    - source
  required_attributes:
    - fiscal_year
```

`mdp fit` reports `data.context.missing_requirements`, `data.context.invalid_requirements`, and the compatibility `data.context.missing` list. Use `attributes` only for bounded reviewed metadata such as fiscal year or segment tier; put evidence and provenance in `signals[].source`.

Success:

```json
{"ok": true, "command": "route", "data": {}}
```

Error:

```json
{"ok": false, "error": {"code": "mdp_error", "message": "message", "details": []}}
```

## Agent handoff

1. Run `mdp --json capabilities`, then `mdp --json doctor` and `mdp --json validate`.
2. If outbound-copy testing needs lead-specific inputs and no real or sanitized prospect row was supplied, generate 2 to 5 fake fixtures:

```bash
mdp sample-leads --dir . --persona "PMM" --job "initial email outbound copy" --count 3 --format yaml
```

3. Convert the supplied user note, CSV, CRM export, Clay, Deepline, spreadsheet, or other source row into `mdp schema prospect`. Preserve `company_domain` when supplied, add `trigger`, `segment`, sourced `signals`, and bounded `attributes` when the pack requires them. Use explicit `persona` when known; otherwise `.mdp/manifest.yaml` can define `persona_mappings` from title keywords to pack personas. For fixture testing, save one generated row to ignored scratch before passing it as `--prospect`.
4. Run `mdp --json fit --prospect <row.json>` and stop if it returns `disqualified` or `insufficient-context`.
5. Run `mdp --json --summary brief --context --prospect <row.json> --channel linkedin --out .mdp/briefs/<brief-name>.json` when a durable brief file is needed.
6. Stop if `data.draft_status` is `no-draft`.
7. Draft from `data.context.entries` first; for generated fixtures, draft against `safe_personalization` and `known_gaps` and never imply the fixture is a real prospect. Open `data.context.full_card_required` paths only when present.
8. Run `mdp --json check-claims` before approval; add `--strict` when advisory target-range misses should fail the gate. It reports unsupported claims plus avoid-rule, output-rule, and hard structured-constraint guardrail hits. Include `--subject`, `--persona`, and `--job` when checking routed subject or channel constraints. Target-range misses appear in `constraint_warnings`; actual attachments, embedded images, and send-surface tracking may appear in `unchecked_constraints` because they cannot be proven from a single draft body.

Generated starter rows and `sample-leads` rows are synthetic examples. They include `source_kind: synthetic-example`, `synthetic: true`, and must not be presented as real prospects. Production rows can come from a user note, CSV, CRM export, Clay, Deepline, spreadsheet, or research workflow after they are normalized into MDP prospect JSON.

Direct persona/job commands resolve pack-owned persona aliases before routing. Check `requested_persona` and `persona_resolution` in JSON output when the route used an alias.

`mdp` is not a sender, CRM, sequencer, lead enricher, scraper, or AI SDR. It is the local decision contract layer.
