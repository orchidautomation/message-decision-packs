---
name: mdp-prospect-brief
description: Use to convert an existing provider-neutral prospect/source row into local MDP prospect JSON, run `mdp fit` and `mdp brief`, and produce an agent-readable decision contract. Does not enrich or send.
---

# MDP Prospect Brief

Turn an existing prospect/source row into clean MDP prospect JSON and, when fit allows, an MDP brief. This skill prepares context; it does not enrich, update systems, or send messages.

This is the row-evaluation path for MDP. Do not create a parallel evaluator in the skill layer: `mdp fit` owns the fit decision, and `mdp brief --context` owns the routed brief contract.

Accepted source rows can come from a user note, CSV, CRM export, Clay, Deepline, a spreadsheet, public research notes, or any other supplied row-like input. Clay is one possible source, not the default mental model.

When the pack has `.mdp/prompts/normalize-prospect.yaml`, use it as the upstream normalization contract for messy rows. The prompt should return `normalized_prospect` plus `normalization_trace`; save `normalized_prospect` as the local prospect JSON that feeds the CLI. Do not let the prompt decide final fit.

## Prospect Shape

Check the schema:

```bash
mdp --json schema prospect
```

Minimum admission fields:

- `name`
- `title`
- `company`

New lead workflows should also supply `company_domain` as the stronger account key. The CLI canonicalizes supplied domains and URLs such as `https://www.apple.com/` to `apple.com`; it does not browse, DNS-check, enrich, or infer a domain from company name.

Preferred fields:

- `company_domain`
- `linkedin_url`
- `company_url`
- `background`
- `trigger`
- `persona`
- `segment`
- `signals` with source, confidence, freshness, and state_as when available
- `attributes` for bounded reviewed metadata such as fiscal year or segment tier
- `source_kind` and `synthetic` when the row is generated, sanitized, private scratch, or sourced from a known row system

Packs may declare readiness requirements in `.mdp/manifest.yaml` with `lead_input_requirements.required_fields`, `required_signal_fields`, and `required_attributes`. Treat `mdp fit` as the source of truth for missing or invalid readiness details.

Use provider-neutral `source_kind` values unless a specific source matters:

- `user-provided-row`
- `csv-row`
- `crm-export-row`
- `clay-row`
- `deepline-row`
- `private-scratch-row`
- `sanitized-example`
- `synthetic-example`

If the input is account-only and does not include a person name and title, do not invent a contact. Ask for the missing person fields or return an insufficient-context decision from the fit gate.

## Workflow

1. Normalize the source row into a small JSON file under a repo-ignored agent artifacts directory or another ignored scratch path. Prefer the pack-owned `.mdp/prompts/normalize-prospect.yaml` contract when available; otherwise manually map to `mdp --json schema prospect`.
2. Keep only useful, non-sensitive fields needed for routing and copy; redact private data before committing any example.
3. Do not treat LinkedIn URL presence as proof of any claim.
4. Preserve raw evidence and uncertainty from the row. Do not smooth away disqualifying asks such as scraping contacts, auto-sending sequences, enrichment, or CRM updates.
5. Preserve row provenance with `source_kind`; mark fictional starter/demo rows with `source_kind: synthetic-example` and `synthetic: true`; do not present them as real prospects.
6. Check fit before drafting or creating a copy brief:

```bash
mdp --json fit --dir . --prospect <prospect.json>
```

7. If the user only asked whether the row should be messaged, return the `mdp fit` decision, matched rules, disqualifiers, `context.missing_requirements`, `context.invalid_requirements`, and gaps. Do not draft.
8. If status is `disqualified` or `insufficient-context`, stop before drafting unless the user explicitly overrides.
9. Run the brief:

```bash
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel <channel>
```

Use `--out .mdp/briefs/<brief-name>.json` when the user expects a durable created brief file. Add `--dry-run` first when an agent should preview the file write before mutating the pack. Without `--out`, the brief is stdout-only.

10. Read `data.context.entries` first if drafting is requested and `data.draft_status` is `ready`. Open `data.context.full_card_required` paths only when present.

## Response

Return:

- normalized prospect fields
- normalization trace when produced by a prompt contract
- inferred persona
- fit status and disqualifiers
- missing or invalid readiness requirements
- required card load order
- whether the brief was saved or stdout-only
- decision trace
- gaps or assumptions

Do not send, sequence, enrich, or update CRM from this skill. If the user wants execution, produce an explicit handoff for a separate exact-action tool outside MDP.
