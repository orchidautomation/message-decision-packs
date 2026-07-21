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

GTM quick demo:

```bash
mdp --json capabilities
mdp --json init --template gtm --name "Example Message Pack" --dir /tmp/mdp-demo --force
mdp --json init --template gtm --name "Example Message Pack" --dir /tmp/mdp-demo --dry-run
mdp --json validate --dir /tmp/mdp-demo
mdp --json validate-prompt-output --dir /tmp/mdp-demo --prompt-id extract-claims-proof --file /tmp/claims-output.json
mdp --json --summary route --entries --eval-fixture --dir /tmp/mdp-demo --persona "PMM" --job "linkedin outbound copy"
mdp --json route --entries --dir /tmp/mdp-demo --persona "PMM" --job "portfolio scope example" --scope product=local-cli
mdp sample-leads --dir /tmp/mdp-demo --persona "PMM" --job "initial email outbound copy" --count 3 --format yaml
mdp --json fit --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json
mdp --json --summary brief --context --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin --out /tmp/mdp-demo/.mdp/briefs/example-linkedin.json
mdp brief --context --readable --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin --out /tmp/mdp-demo/.mdp/briefs/example-linkedin.md
mdp render-brief --dir /tmp/mdp-demo --file /tmp/mdp-demo/.mdp/briefs/example-linkedin.json --template gtm-prospect --out /tmp/mdp-demo/.mdp/briefs/example-linkedin.md
mdp --json check-claims --dir /tmp/mdp-demo --text "MDP is a local offline CLI for modular message context."
mdp --json check-claims --dir /tmp/mdp-demo --text "<draft copy>" --subject "<subject>" --persona "PMM" --job "initial email outbound message"
mdp --json gaps --dir /tmp/mdp-demo
mdp --json eval --dir /tmp/mdp-demo
mdp --json copy --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin
```

Proposal quick path:

```bash
mdp --json init --template proposal --dir /tmp/mdp-proposal-demo --force
mdp --json validate --dir /tmp/mdp-proposal-demo
mdp --json eval --dir /tmp/mdp-proposal-demo
mdp --json validate-prompt-output --dir /tmp/mdp-proposal-demo --prompt-id normalize-opportunity --file <prompt-output.json>
mdp --json validate-prompt-output --dir /tmp/mdp-proposal-demo --prompt-id normalize-opportunity --file <prompt-output.json> --source-audit <source-audit.json>
mdp --json run-receipt --dir /tmp/mdp-proposal-demo --workflow proposal-review --isolation isolated --declared-inputs-only --prompt-id normalize-opportunity --prompt-output <prompt-output.json> --validation <validation-result.json> --source-audit <source-audit.json>
mdp --json route --entries --dir /tmp/mdp-proposal-demo --persona "Proposal Lead" --job "bid no bid review"
mdp --json author-proof-output --dir /tmp/mdp-proposal-demo --draft /tmp/mdp-proposal-demo/examples/proof-output-drafts/compliance-row.draft.json --out /tmp/mdp-proof-output.json
mdp --json verify-output --dir /tmp/mdp-proposal-demo --file /tmp/mdp-proof-output.json
mdp render-brief --dir /tmp/mdp-proposal-demo --file /tmp/mdp-proposal-demo/examples/proof-output/valid-binding.json --template proposal-review
mdp --json gaps --dir /tmp/mdp-proposal-demo
mdp --json check-claims --dir /tmp/mdp-proposal-demo --persona "Proposal Lead" --job "compliance review" --text "The sample team is CMMC compliant."
```

The proposal starter does not write a prospect row or fake lead fixtures. It is a synthetic proposal review pack for bid/no-bid, compliance, proof, red-team, and executive review jobs. Its `normalize-opportunity` prompt maps messy proposal/RFP context into bounded profile vocabulary and validated prompt-output fields; it does not submit, scrape, enrich, certify, or manage proposal work. Proposal packs need the same human-readable review-layer principle as prospect briefs, but should use opportunity/review metadata and proposal profile sections such as bid/no-bid read, compliance gaps, proof receipts, unsupported claims, red-team gaps, and `verify-output` status rather than prospect/outreach labels.

Use `brief` for production GTM prospect handoff. Add `--out <path>` when the machine brief should be saved; otherwise the artifact is stdout-only. Use `render-brief` when an existing artifact needs a compact human layer. `gtm-prospect` renders `mdp.message-brief.v0`; `proposal-review` and `proof-report` render `mdp.proof-output.v0` through the proof verifier. `--format json` emits the structured `mdp.human-brief.v0` object; Markdown is generated from that object by default. Failed gates remain failed: no-draft prospect briefs and proof gaps do not become send-ready or reusable draft text. Use `copy` only for local demos. Source inventory lives in `.mdp/sources.yaml`, reusable extraction prompts live in `.mdp/prompts/*.yaml`, CTA guidance lives in `cards/ctas.yaml`, channel rules live in `cards/channel-policies.yaml`, approved claims live in `cards/claims.yaml`, global style and structure rules live in `cards/output-rules.yaml`, and durable unknowns live in `cards/gaps.yaml`. Entries can use `avoid` for blocked literals, `exact_paragraphs` for fixed paragraph counts, and `constraints` for deterministic output limits. Draft-text constraints such as word count, subject word count, subject avoid literals, max questions, and forbidden links, attachments, images, HTML, or tracking are enforced by `check-claims`; proof-output constraints under `constraints.proof_output` are enforced by `verify-output`.

Use `author-proof-output` when an agent needs to compile ordered proof-output segments without hand-writing pack identity or `output.text`. The input is a smaller `mdp.proof-output-draft.v0` file with `route`, `output.kind`, `output.format`, and ordered `segments`. The command fills loaded pack identity, joins segment text, runs `verify-output` including the embedded full-text `check-claims` layer, and writes `--out` only when the proof-output artifact is valid. Use `mdp --json schema proof-output-draft` for the draft contract.

Use `run-receipt` when a runner or agent host normalized proposal/doc material before deterministic MDP checks ran. For audit-grade proposal review, the host must create a fresh/stateless model call, pass only prompt-declared inputs, save the prompt output and validation result, and include the `mdp.source-audit.v0` ledger:

```bash
mdp --json run-receipt --dir . --workflow proposal-review --isolation isolated --declared-inputs-only --prompt-id normalize-opportunity --prompt-output <prompt-output.json> --validation <validation-result.json> --source-audit <source-audit.json> --out <run-receipt.json>
```

A receipt returns `decision: advisory` when normalization used the ambient conversation or when declared-input-only cannot be confirmed. It returns `decision: blocked` when required artifacts are missing, malformed, or failed validation. Use `mdp --json schema run-receipt` for the receipt contract.

Layer 1 rules are card body guidance an agent must read and follow. Layer 2 rules are structured constraints the CLI can enforce. For proposal `mdp.proof-output.v0` artifacts, packs can declare:

```yaml
constraints:
  proof_output:
    required_segment_kinds: [requirement_status, gap]
    min_segments:
      requirement_status: 1
      template_text: 1
    require_source_refs_for_claims: true
    max_connective_words: 18
```

These proof-output constraints are pack-owned card entry fields, not fields the model may put inside the generated proof-output artifact.

## JSON contract

All commands support `--json`; add `--summary` for compact status output. Run `mdp --json capabilities` when an agent or wrapper needs to inspect command names, coarse side effects, output contracts, `--out` support, dry-run support, strict-mode support, and stable error codes. Validation-style commands return structured data and exit nonzero when `data.valid` is false. Argument parse errors also return JSON when `--json` is present.

Selected write paths support `--dry-run` so agents can inspect local file writes before mutating a pack:

```bash
mdp --json init --name "Message Pack" --dir . --dry-run
mdp --json brief --context --dir . --prospect <prospect.json> --channel linkedin --out .mdp/briefs/example.json --dry-run
mdp --json emit-brief --dir . --persona "PMM" --job "linkedin outbound copy" --out .mdp/briefs/route.json --dry-run
mdp --json pack --dir . --out /tmp/mdp-pack.json --dry-run
mdp --json author-proof-output --dir . --draft examples/proof-output-drafts/compliance-row.draft.json --out /tmp/proof-output.json --dry-run
mdp --json run-receipt --dir . --workflow proposal-review --isolation isolated --declared-inputs-only --prompt-id normalize-opportunity --prompt-output <prompt-output.json> --validation <validation-result.json> --source-audit <source-audit.json> --out <run-receipt.json> --dry-run
```

Use `--strict` on validation/checking flows when warnings should fail an agent or CI gate:

```bash
mdp --json validate --strict --dir .
mdp --json validate-prompt-output --strict --dir . --prompt-id extract-claims-proof --file /tmp/claims-output.json
mdp --json check-claims --strict --dir . --text "<draft copy>" --subject "<subject>" --persona "PMM" --job "initial email outbound message"
mdp --json eval --strict --dir .
```

JSON errors use stable top-level codes where the CLI can classify the failure. Run `mdp --json capabilities` for the current complete command, side-effect, and error-code inventory instead of relying on a copied partial list.

`profile.id` and canonical `jobs[].skill_id` bindings are skill-routing metadata. Use `mdp --json skills --dir .` for pack eligibility and `mdp --json skills --dir . --job <job-id>` for one deterministic recommendation. A profile is activation-ready only when `mdp --json validate --dir .` reports `data.profile.activation_ready: true`. Profile-aware manifests declare `required_primitives`, `primitive_map`, `input_contracts`, closed profile jobs, and `profile_eval.required_categories`; validation rejects unknown primitive IDs, unknown or profile-incompatible job/skill pairs, and missing mapped card, prompt, input contract, job, or eval references. Missing required primitive or eval-category coverage is warning-first by default and fails with `--strict`. Eval fixtures can run `command: validate-prompt-output` with `prompt_id` or `prompt` plus inline `prompt_output` and optional `source_audit`, so profile activation can prove normalization contracts before rows reach `mdp fit` or `mdp brief`.

Universal primitive IDs are `actors`, `decision-criteria`, `source-signals`, `needs-requirements`, `evidence-proof`, `boundaries`, `output-contracts`, `routing-jobs`, `gaps`, and `evals`. Keep domain terms such as account context or opportunity context in profile-owned card IDs, input contracts, prompts, jobs, and eval fixtures unless a future format explicitly adds a new core card kind.

Portfolio terms do not add primitives. A GTM profile may declare `profile.context_dimensions` such as `product`, `capability`, `solution`, or `segment`, plus generic `context_dimension_dependencies`. Card entries use `scope` to narrow where their existing primitive decision applies. Matching is OR within an entry dimension and AND across dimensions; unscoped entries are global. V1 accepts one runtime value per dimension.

Use repeatable `--scope dimension=value` selectors on `route`, `emit-brief`, and route-scoped `check-claims`. Prospect-driven `fit` and `brief` derive declared scope from scalar `attributes`; a declared `segment` dimension uses the top-level prospect `segment`. Portfolio-sensitive outputs draft from bounded `entry_route.matches` or `context.entries`, not shared card files. Missing/invalid scope blocks drafting, and `verify-output` returns `proof_output_scope_unsupported` for scoped packs until proof artifacts can carry scope. See [Portfolio-Aware GTM Scope](../docs/portfolio-scope.md) for the complete contract and rollout checklist.

Use `mdp --json schema prompt` to inspect the reusable prompt contract. Prompt outputs use `contract: mdp.prompt-output.v0` and must match the contract named by each prompt's `output_contract.schema_ref`; starter prompts can inline the full JSON Schema with `mdp init --include-output-schemas`. Extraction prompts preserve `card_patches`, `gaps`, `rejected_claims`, confidence, and provenance; normalization prompts preserve `normalized_prospect`, `normalization_trace`, gaps, and empty `card_patches`. Proposal normalization may also include `normalized_opportunity` as an exact alias of `normalized_prospect`, but existing consumers should continue to read `normalized_prospect`. Prompt files are local decision contracts, not browsing, scraping, enrichment, sending, sequencing, or CRM-update workflows.

Treat model-produced prompt output as untrusted review input. Run `mdp --json validate-prompt-output` before copying reviewed `card_patches` into cards or saving `normalized_prospect` for `mdp fit` and `mdp brief`. `source_summary.inputs_used` must name exact declared prompt inputs; source paths, snippets, PDF/page locators, URLs, and field-level provenance belong in candidate `evidence`/`provenance`, `signals[].source`, `normalization_trace.preserved_raw_fields`, or `normalization_trace.missing_required[].source_evidence`. For proposal PDF/doc normalization, pass `--source-audit <source-audit.json>` to check source refs and ref-plus-snippet citations against a bounded `mdp.source-audit.v0` extraction ledger backed by `.mdp/sources.yaml` source IDs. The validator rejects markdown-wrapped JSON, wrong prompt identity, undeclared input references, wrong card kinds, fake-person normalization, candidate ID collisions with existing card entries, normalized opportunity aliases that diverge from `normalized_prospect`, normalized values outside pack-owned value contracts, missing or non-boolean `normalization_trace.fit_readiness.ready_for_mdp_fit`, prompt outputs that claim `ready_for_mdp_fit: true` while missing manifest `lead_input_requirements.required_fields`, `required_signal_fields`, or `required_attributes`, and audited source refs/snippets that do not exist in the supplied source audit.

Prompt-output validation proves the artifact matches the prompt contract and that its readiness claim is internally consistent with the pack input policy. It does not replace `mdp fit`; run `mdp fit` on the reviewed normalized prospect to get the final fit, disqualified, or insufficient-context decision.

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
  value_contracts:
    segment:
      type: string
      enum:
        - agent-assisted GTM
    source_kind:
      type: string
      enum:
        - user-provided-row
        - csv-row
        - crm-export-row
        - clay-row
        - deepline-row
        - private-scratch-row
        - sanitized-example
        - synthetic-example
  attribute_definitions:
    fiscal_year:
      type: string
      description: Optional reviewed account metadata.
```

`mdp fit` reports `data.context.missing_requirements`, `data.context.invalid_requirements`, and the compatibility `data.context.missing` list. Use `attributes` only for bounded reviewed metadata such as fiscal year or segment tier; put evidence and provenance in `signals[].source`. Use `value_contracts` and `attribute_definitions` when prompt outputs need exact enum, type, date, or date-time validation.

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
8. Run `mdp --json check-claims` before approval; add `--strict` when advisory target-range misses should fail the gate. It reports unsupported claims plus avoid-rule, output-rule, exact paragraph, and hard structured-constraint guardrail hits. Include `--subject`, `--persona`, and `--job` when checking routed subject, paragraph, or channel constraints. Target-range misses appear in `constraint_warnings`; actual attachments, embedded images, and send-surface tracking may appear in `unchecked_constraints` because they cannot be proven from a single draft body. For `mdp.proof-output.v0` proposal review artifacts, run `mdp --json verify-output`; it also applies pack-owned `constraints.proof_output`.

Generated starter rows and `sample-leads` rows are synthetic examples. They include `source_kind: synthetic-example`, `synthetic: true`, and must not be presented as real prospects. Production rows can come from a user note, CSV, CRM export, Clay, Deepline, spreadsheet, or research workflow after they are normalized into MDP prospect JSON.

Direct persona/job commands resolve pack-owned persona aliases before routing. Check `requested_persona` and `persona_resolution` in JSON output when the route used an alias.

`mdp` is not a sender, CRM, sequencer, lead enricher, scraper, or AI SDR. It is the local decision contract layer.
