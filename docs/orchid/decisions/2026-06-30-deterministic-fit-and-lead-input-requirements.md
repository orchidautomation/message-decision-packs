# Deterministic Fit And Lead Input Requirements

Date: 2026-06-30

## Decision

Keep `mdp fit` deterministic. Do not move lead qualification into an LLM call inside the CLI.

Use LLMs upstream through pack-owned prompt contracts to normalize messy source rows into provider-neutral prospect JSON. The CLI should then apply explicit, auditable pack rules to the normalized row:

```text
messy lead row / CSV / CRM export / research note
  -> .mdp/prompts/normalize-prospect.yaml
  -> normalized_prospect + normalization_trace + gaps
  -> mdp fit
  -> mdp brief --context
  -> agent draft
  -> mdp check-claims
```

The next product slice should make the deterministic boundary less brittle by adding pack-owned lead input requirements and clearer source-column normalization, not by hiding judgment inside `mdp fit`.

## Prior Behavior

Before the lead-input workflow batch, the prospect schema required only:

```text
name
title
company
```

The fit gate is stricter than the parser. It currently treats a row as draft-ready only when it has:

```text
trigger
persona
segment
signals
signal.source
```

Then it checks the normalized row against pack-owned fit-rule entries. A row can parse successfully and still return `insufficient-context`. That is intentional, but the documentation should explain it more plainly.

`mdp fit` does not call an LLM. It reads the manifest, fit-rules card, persona mappings, and prospect JSON; builds a local text haystack from known prospect fields; checks fit-rule matches and avoid terms; and reports `fit`, `insufficient-context`, or `disqualified`.

## Why Not Put An LLM In `mdp fit`

An LLM inside `mdp fit` would make the command feel smarter on thin or messy rows, but it would weaken MDP's core contract:

- fit decisions would become provider-dependent instead of local and repeatable;
- CI and eval fixtures would be harder to trust;
- pack authors could not easily diff why qualification behavior changed;
- missing evidence might be smoothed over instead of surfaced as a gap;
- users could mistake model plausibility for pack-owned truth.

The better split is:

```text
AI normalizes ambiguity.
CLI enforces reviewed decisions.
```

The upstream prompt can interpret messy columns, preserve uncertainty, and produce a normalization trace. The CLI should fail closed when the normalized row lacks the pack's required evidence.

## Implemented Input Requirement Contract

The prior required-field story was too implicit. Users will reasonably ask:

```text
What columns do I need in my lead list before MDP can qualify and draft?
```

MDP now answers that directly with a pack-owned input requirement contract that can describe required prospect fields, required signal fields, and optional custom attributes. For example:

```yaml
lead_input_requirements:
  required_fields:
    - name
    - title
    - company_domain
    - trigger
    - segment
  required_signal_fields:
    - source
  required_attributes:
    - fiscal_year
```

Then `mdp fit` reports missing input requirements in a stable JSON shape:

```json
{
  "status": "insufficient-context",
  "context": {
    "missing": [
      "company_domain",
      "trigger",
      "signals.source",
      "attributes.fiscal_year"
    ]
  }
}
```

This keeps qualification deterministic while giving users a concrete data-readiness checklist.

## Domain Normalization

MDP should prefer a normalized company domain in prospect input. Company name can stay useful for human-readable drafts, but domain is a better stable account key.

The CLI includes a helper that canonicalizes domains from common supplied inputs:

```text
https://www.apple.com/ -> apple.com
www.apple.com -> apple.com
apple.com/about -> apple.com
```

The helper should not browse or verify ownership. It should only normalize a supplied value. If the user supplies only a company name and no domain, MDP should not invent a domain.

## Custom Attributes

The prospect input supports a bounded custom attribute map for pack-specific segmentation:

```json
{
  "name": "Alex Rivera",
  "title": "VP Finance",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "segment": "enterprise SaaS",
  "trigger": "FY2027 planning cycle",
  "attributes": {
    "fiscal_year": "FY2027"
  },
  "signals": [
    {
      "id": "planning-cycle",
      "title": "FY2027 planning cycle",
      "source": "crm.fiscal_year",
      "confidence": "medium"
    }
  ]
}
```

The CLI preserves attributes in `fit` and `brief` outputs. Fit rules may later match against attributes explicitly, but unsupported arbitrary top-level fields should not silently become qualification inputs.

## Prompt And Schema Consistency

Every prospect input change must update the full contract surface in the same slice. If MDP adds `company_domain`, `attributes`, or pack-owned lead input requirements, the change is incomplete until these stay aligned:

- Rust prospect model and parsing.
- `mdp --json schema prospect`.
- Runtime normalization prompt contract.
- Prompt output schema generation.
- Starter/template pack examples.
- Eval fixtures for fit, brief, and prompt outputs.
- Agent-facing skills that tell Codex, Claude Code, and other hosts how to normalize rows.
- User docs that explain required lead-list columns and no-draft states.

The prompt YAML should be robust enough that a host can populate it, validate the returned JSON, and then pass `normalized_prospect` to the CLI without the CLI failing on shape drift.

The required lifecycle should be:

```text
schema or prompt contract changes
  -> regenerate/update starter prompt contracts
  -> validate prompt files
  -> validate model-produced prompt output before CLI ingestion
  -> run fit/brief/eval fixtures
  -> update agent-facing skills and docs
```

Today `mdp validate` catches prompt structure, schema references, strict JSON intent, and example safety. It does not yet catch every semantic drift case, such as prompt examples citing undeclared inputs or examples citing evidence while `source_summary.inputs_used` stays empty. Those checks belong in the prompt-output hardening slice.

## Hook And Lint Guardrails

Use hooks as guardrails around drift, not as hidden regeneration machinery.

Recommended host behavior:

```text
Prompt/schema/skill/template file changed.
Run focused validation.
If validation detects drift, feed the failure back to the agent.
Agent updates the dependent files explicitly.
Run validation again.
```

For Codex and Claude Code, useful hook triggers are:

- file changes under `cli/src/models.rs`, `cli/src/commands/schemas.rs`, `cli/src/starter.rs`, or `cli/src/commands/health.rs`;
- file changes under `.mdp/prompts/` or `plugin/assets/templates/basic/.mdp/prompts/`;
- file changes under `plugin/skills/`;
- file changes under docs that describe prospect schema, prompt output, or fit behavior.

The hook should run focused checks such as:

```bash
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json eval --dir plugin/assets/templates/basic
cargo test --manifest-path cli/Cargo.toml
```

For release-impacting changes, `make validate` remains the repo-level gate.

Do not make the hook silently rewrite prompt files or skills. Silent regeneration would hide contract drift from review. The agent should make the dependent edits in the normal diff so reviewers can see why the CLI, prompt, skill, docs, and evals changed together.

## Documentation Changes Needed

Update user-facing docs to state:

- MDP accepts minimal rows, but minimal rows are usually not draft-ready.
- Prompt contracts normalize supplied data; they do not invent missing evidence.
- `mdp fit` is deterministic and does not call an LLM.
- Thin lead lists should produce gap reports, not fake personalization.
- Users who want batch copy should either provide richer columns or run an upstream research/enrichment workflow before MDP.
- Extra source columns should be mapped into known prospect fields, `signals`, or `attributes`.

The batch-lead story should be framed as admission and readiness:

```text
100 rows in
  -> normalize
  -> fit/readiness report
  -> draft only for rows that pass
  -> gap summary for the rest
```

## Implemented Requirements

1. Added `company_domain` to the prospect schema and normalization prompt output.
2. Added deterministic domain canonicalization for supplied URLs/domains.
3. Added bounded `attributes` to the prospect schema for pack-specific segmentation values.
4. Added pack-owned `lead_input_requirements`.
5. Made `mdp fit` include missing required fields, signal fields, attributes, and invalid supplied domains.
6. Added focused tests for domain normalization, required attributes, invalid domains, and invalid requirement declarations.
7. Updated README, getting started docs, conceptual decision flow, CLI usage, hook guidance, starter templates, and MDP plugin skills.

Remaining follow-up:

1. Consider broader prompt/schema drift checks when a prompt example cites undeclared inputs or stale `inputs_used`.
2. Decide later whether MDP needs a first-class batch readiness report for 100-row workflows.

## Boundary

This decision does not add enrichment, scraping, sending, sequencing, CRM updates, or hosted qualification. Those can exist as upstream or downstream adapters, but MDP remains the local decision contract layer.
