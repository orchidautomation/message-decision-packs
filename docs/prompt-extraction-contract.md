# MDP Prompt Extraction Contract

MDP prompt files are local, reusable extraction prompt contracts for classifying supplied person, company, account, domain, row, or research context into strict JSON candidate data for MDP cards.

They live under `.mdp/prompts/*.yaml` and use `format: mdp.prompt.v0`. They are not scrapers, enrichers, senders, hosted ingestion jobs, or execution workflows. They only define how an agent should classify user-provided context into reviewable candidate entries, gaps, and rejected claims.

The prompts work at both person and company/account level. A full ICP extraction can use `person_data`, `company_data`, and `account_data` together to propose persona, fit, pain, hook, CTA, avoid-rule, claim, and gap candidates.

## File Shape

Each prompt file declares:

- `target_card_kinds`: one or more MDP card kinds the prompt may populate.
- `inputs`: named inputs with explicit defaults and missing-data behavior.
- `instructions`: model-facing rules for using only supplied input.
- `output_contract`: strict JSON output requirements and a safe example.

Prompt outputs use `contract: mdp.prompt-output.v0` and must include:

- `source_summary`: domain, company/person/account labels when known, inputs used, and confidence.
- `card_patches`: candidate entries grouped by target card.
- `gaps`: missing data that blocks stronger entries.
- `rejected_claims`: unsupported claims the agent refused to promote.

Candidate entries carry normal MDP entry fields:

```json
{
  "id": "claim-local-decision-context",
  "title": "Local decision context",
  "body": "Supplied source material describes the product as local decision context for GTM messaging.",
  "applies_to": ["PMM", "GTM Engineering"],
  "evidence": ["source_notes"],
  "avoid": []
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

Only the normal MDP entry fields should be copied into `.mdp/cards/*.yaml` after review. Keep confidence, provenance, status, gaps, and rejected claims in the review artifact or source ledger.

## Safe Defaults

Use safe defaults instead of inventing facts:

- Missing strings: `"N/A"`.
- Missing arrays: `[]`.
- Missing or weak support: `confidence: "unknown"` and `status: "gap"`.
- Unsupported claims: put them in `rejected_claims`, not `card_patches`.

Validation rejects prompt files that do not require strict JSON output, omit provenance/confidence fields, or include non-gap example entries with a real body and no evidence or provenance.

## Starter Extraction Prompts

The basic template includes extraction prompt contracts for:

- ICP/persona and fit candidates.
- Pains and triggers.
- Hooks and message angles.
- Claims and proof.
- Fit and disqualification rules.
- Avoid rules.
- Output rules.
- CTA and channel policy.
- Gaps and missing evidence.

## Agent Loop

1. Run `mdp --json validate --dir <pack>` before using prompt files.
2. Pick the prompt whose `target_card_kinds` match the card area you want to populate.
3. Fill `company_domain`, `company_data`, `person_data`, `account_data`, `source_notes`, and `existing_pack_context` from user-provided or local pack context.
4. Run the prompt with strict JSON output.
5. Review `card_patches`, `gaps`, and `rejected_claims`.
6. Copy only reviewed MDP entry fields into cards, then run `mdp --json validate` again.

Use `mdp --json schema prompt` to inspect the machine-readable prompt contract.

## Full ICP Input Example

```json
{
  "company_domain": "example.com",
  "company_data": "ExampleCo sells workflow tooling to GTM teams and is standardizing agent-assisted outbound context.",
  "person_data": "Alex Rivera, GTM Engineering Lead. Owns Clay, Codex, and Claude Code workflow quality.",
  "account_data": "Mid-market B2B SaaS. Trigger: consolidating message context across AI-assisted GTM workflows.",
  "source_notes": ["User-provided account research note from 2026-06-25."],
  "existing_pack_context": "Current MDP pack has personas, fit-rules, pains, hooks, claims, avoid-rules, output-rules, CTAs, channel policies, and gaps cards."
}
```
