# MDP Prompt Contracts

MDP prompt files are local, reusable prompt contracts for two jobs:

- Normalize messy supplied rows into provider-neutral MDP prospect JSON before the CLI runs.
- Classify supplied person, company, account, domain, row, or research context into strict JSON candidate data for MDP cards.

They live under `.mdp/prompts/*.yaml` and use `format: mdp.prompt.v0`. They are not scrapers, enrichers, senders, hosted ingestion jobs, or execution workflows. They define how an agent should transform supplied context into reviewable, source-preserving JSON that either feeds the CLI or proposes pack edits.

The prompts work at both person and company/account level. A full ICP extraction can use `person_data`, `company_data`, and `account_data` together to propose persona, fit, pain, hook, CTA, avoid-rule, claim, and gap candidates.

## File Shape

Each prompt file declares:

- `target_card_kinds`: one or more MDP card kinds the prompt may populate or consult.
- `inputs`: named inputs with explicit defaults and missing-data behavior.
- `instructions`: model-facing rules for using only supplied input.
- `output_contract`: strict JSON output requirements, a compact schema reference, optional inline JSON Schema, and a safe example.

Every prompt-output reference must resolve back to a declared prompt input. `source_summary.inputs_used`, candidate-entry `evidence`, and candidate-entry `provenance` should use declared input names directly or field-qualified forms such as `raw_row.company` or `source_notes: supplied note`.

`output_contract.schema_ref` names the authoritative response contract. Starter prompt files keep that reference compact by default. Use `mdp init --include-output-schemas` when an agent host or model API needs a literal JSON Schema object in each prompt file under `output_contract.schema`. `output_contract.example` is still useful as a model-friendly reference, but it does not replace the schema contract.

Prompt outputs use `contract: mdp.prompt-output.v0` and must include:

- `source_summary`: domain, company/person/account labels when known, inputs used, and confidence.
- `card_patches`: candidate entries grouped by target card.
- `gaps`: missing data that blocks stronger entries.
- `rejected_claims`: unsupported claims the agent refused to promote.

Runtime normalization prompts set `output_contract.output_kind: prospect-normalization` and also require:

- `normalized_prospect`: the exact JSON shape accepted by `mdp --json schema prospect`.
- `normalization_trace`: persona mapping, fit-readiness, missing fields, and raw-field preservation notes.

For normalization prompts, `card_patches` should stay empty. The prompt prepares runtime input; it does not edit cards.

Candidate entries carry normal MDP entry fields:

```json
{
  "id": "claim-local-decision-context",
  "title": "Local decision context",
  "body": "Supplied source material describes the product as local decision context for GTM messaging.",
  "applies_to": ["PMM", "GTM Engineering"],
  "evidence": ["source_notes"],
  "avoid": [],
  "constraints": {
    "word_count": {"min": 50, "max": 125, "target_min": 75, "target_max": 110},
    "subject_words": {"min": 3, "max": 6},
    "subject_avoid": ["Re:", "Fwd:"],
    "max_questions": 1,
    "forbid_links": true
  }
}
```

They also carry review metadata:

```json
{
  "confidence": "medium",
  "provenance": ["source_notes: supplied source notes"],
  "status": "needs-review",
  "notes": []
}
```

Only the normal MDP entry fields should be copied into `.mdp/cards/*.yaml` after review. `constraints` is a normal optional MDP entry field when the source explicitly calls for deterministic output limits. Keep confidence, provenance, status, gaps, and rejected claims in the review artifact or source ledger.

## Safe Defaults

Use safe defaults instead of inventing facts:

- Missing strings: `"N/A"`.
- Missing arrays: `[]`.
- Missing or weak support: `confidence: "unknown"` and `status: "gap"`.
- Unsupported claims: put them in `rejected_claims`, not `card_patches`.

Validation rejects prompt files that do not require strict JSON output, omit both `schema_ref` and an inline response schema, use the wrong schema reference for the prompt output kind, let an inline response root accept extra keys, omit provenance/confidence fields, or include non-gap example entries with a real body and no evidence or provenance.

Treat prompt output as reviewable artifact data, not automatic pack truth. Run `mdp --json validate-prompt-output --prompt-id <id> --file <output.json>` before applying reviewed card entries or promoting normalization output into the runtime CLI flow.

## Starter Prompt Contracts

The basic template includes a runtime prompt contract for:

- Prospect/source row normalization into MDP prospect JSON.

It also includes extraction prompt contracts for:

- ICP/persona and fit candidates.
- Pains and triggers.
- Hooks and message angles.
- Claims and proof.
- Fit and disqualification rules.
- Avoid rules.
- Output rules.
- CTA and channel policy.
- Gaps and missing evidence.

## Runtime Normalization Loop

1. Run `mdp --json validate --dir <pack>` before using prompt files.
2. Load `.mdp/prompts/normalize-prospect.yaml`.
3. Supply the messy row as `raw_row`, plus relevant `existing_pack_context` from the manifest, persona mappings, fit-rules, signals, and avoid-rules.
4. Run the prompt with strict JSON output matching `output_contract.schema_ref`, or `output_contract.schema` when the prompt file was generated with inline schemas.
5. Save `normalized_prospect` to ignored scratch as `<prospect>.json`.
6. Run:

```bash
mdp --json fit --dir <pack> --prospect <prospect>.json
```

Do not let the normalization prompt silently decide final fit. It should preserve ambiguity and disqualifying source language, then the CLI applies the deterministic gate.

## Card Extraction Loop

1. Run `mdp --json validate --dir <pack>` before using prompt files.
2. Pick the prompt whose `target_card_kinds` match the card area you want to populate.
3. Fill `company_domain`, `company_data`, `person_data`, `account_data`, `source_notes`, and `existing_pack_context` from user-provided or local pack context.
4. Run the prompt with strict JSON output matching `output_contract.schema_ref`, or `output_contract.schema` when the prompt file was generated with inline schemas.
5. Run `mdp --json validate-prompt-output --prompt-id <id> --file <output.json>`.
6. Review `card_patches`, `gaps`, and `rejected_claims`.
7. Copy only reviewed MDP entry fields into cards, then run `mdp --json validate` again.

Use `mdp --json schema prompt` to inspect the machine-readable prompt contract.

## Runtime Normalization Output Example

```json
{
  "contract": "mdp.prompt-output.v0",
  "prompt_id": "normalize-prospect-row",
  "source_summary": {
    "company_domain": "example.com",
    "company_name": "ExampleCo",
    "person_name": "Alex Rivera",
    "person_title": "Revenue Operations Lead",
    "account_name": "ExampleCo",
    "inputs_used": ["raw_row", "existing_pack_context"],
    "confidence": "medium"
  },
  "normalized_prospect": {
    "name": "Alex Rivera",
    "title": "Revenue Operations Lead",
    "company": "ExampleCo",
    "source_kind": "user-provided-row",
    "synthetic": false,
    "company_url": "https://example.com",
    "background": "Source row says the team is standardizing campaign qualification data across CRM exports, spreadsheets, and research notes.",
    "trigger": "Standardizing prospect qualification data before routing new campaigns.",
    "persona": "Revenue Operations",
    "segment": "B2B GTM operations",
    "signals": [
      {
        "id": "qualification-data-standardization",
        "title": "Standardizing prospect qualification data",
        "source": "raw_row.operations_note",
        "confidence": "medium",
        "freshness": "N/A",
        "state_as": "supplied"
      }
    ]
  },
  "normalization_trace": {
    "persona": {
      "source": "existing_pack_context.persona_mappings",
      "matched_keywords": ["revenue operations"],
      "confidence": "medium",
      "needs_review": false
    },
    "fit_readiness": {
      "has_trigger": true,
      "has_persona": true,
      "has_segment": true,
      "has_signals": true,
      "has_signal_source": true,
      "ready_for_mdp_fit": true
    },
    "preserved_raw_fields": ["raw_row.name", "raw_row.title", "raw_row.company", "raw_row.operations_note"],
    "missing_required": []
  },
  "card_patches": [],
  "gaps": [],
  "rejected_claims": []
}
```

## Full ICP Input Example

```json
{
  "company_domain": "example.com",
  "company_data": "ExampleCo sells workflow tooling to GTM teams and is standardizing campaign qualification data.",
  "person_data": "Alex Rivera, Revenue Operations Lead. Owns CRM hygiene, campaign routing, and prospect qualification workflows.",
  "account_data": "Mid-market B2B SaaS. Trigger: consolidating qualification context across CRM exports, spreadsheets, and research notes.",
  "source_notes": ["User-provided account research note from 2026-06-25."],
  "existing_pack_context": "Current MDP pack has personas, fit-rules, pains, hooks, claims, avoid-rules, output-rules, CTAs, channel policies, and gaps cards."
}
```
