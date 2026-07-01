---
name: mdp
description: Use when the user explicitly wants to create, validate, inspect, route, or use a Message Decision Pack, `.mdp/` pack, MDP CLI, MDP skills, or MDP brief. Prefer `mdp` CLI before reading `.mdp/` YAML.
---

# MDP

For fuzzy or multi-step MDP work, use `$mdp-lfg` first, then route to the narrower skill or CLI command.

Use Message Decision Packs as the source of messaging decisions, not as the execution system. The pack stores ICP, fit rules, personas, pains, signals, positioning, claims, hooks, channel policies, avoid rules, output rules, CTA rules, objections, gaps, copy patterns, and prompt contracts. Prompt contracts normalize messy upstream rows or propose reviewed card entries; the `mdp` CLI validates, routes, checks fit, checks claims and output guardrails, lists gaps, and runs eval fixtures. Draft only after the CLI returns the relevant cards and fit is acceptable.

## First Check

From the workspace that contains or should contain a pack:

```bash
command -v mdp
mdp --json capabilities
mdp --json doctor --dir .
```

If `mdp` is missing, say the CLI is not installed and ask whether to install or locate it. When only the CLI is needed, suggest:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y
```

When the user also wants supported agent/plugin bundles, suggest:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

Do not fake validation by reading YAML manually.

Use `mdp --json capabilities` before advanced agent orchestration to inspect command contracts, coarse side effects, `--out` support, dry-run support, strict-mode support, and stable JSON error codes. Keep `--json` on whenever another agent, script, or tool will parse output.

## Create Or Improve A Pack

For a new generic pack:

```bash
mdp --json init --name "Message Pack" --dir . --dry-run
mdp --json init --name "Message Pack" --dir .
```

For a neutral demo:

```bash
mdp --json init --template gtm --name "Example Message Pack" --dir . --dry-run
mdp --json init --template gtm --name "Example Message Pack" --dir .
```

When brainstorming the pack, help fill these files:

- `.mdp/manifest.yaml`
- `.mdp/sources.yaml`
- `.mdp/cards/personas.yaml`
- `.mdp/cards/positioning.yaml`
- `.mdp/cards/fit-rules.yaml`
- `.mdp/cards/signals.yaml`
- `.mdp/cards/pains.yaml`
- `.mdp/cards/claims.yaml`
- `.mdp/cards/motions.yaml`
- `.mdp/cards/channel-policies.yaml`
- `.mdp/cards/hooks.yaml`
- `.mdp/cards/avoid-rules.yaml`
- `.mdp/cards/output-rules.yaml`
- `.mdp/cards/copy-patterns.yaml`
- `.mdp/cards/ctas.yaml`
- `.mdp/cards/objections.yaml`
- `.mdp/cards/gaps.yaml`
- `.mdp/evals/*.yaml`
- `.mdp/prompts/normalize-prospect.yaml`

For outbound rules, preserve the channel taxonomy. Put channel and lifecycle policy in `channel-policies.yaml` (`email-initial-touch`, `email-follow-up`, `linkedin-initial-touch`, `linkedin-follow-up`, plus non-copy entries such as `call-prep` and `agent-brief`). Put generated-text constraints such as plain text by default, no links/HTML/tracking by default, subject guidance, and no fake personalization in `output-rules.yaml`. Put ask boundaries such as soft asks, calendar-second policy, and owner-routing questions in `ctas.yaml`. Put reusable narrative structures such as trigger or hypothesis -> proof gap -> approved angle -> one soft CTA in `copy-patterns.yaml`.

After edits:

```bash
mdp --json validate --dir .
```

Use `--strict` when warnings should fail an agent or CI gate:

```bash
mdp --json validate --strict --dir .
```

Pack extensions must use supported surfaces. Put advisory custom annotations on card entries under `metadata`; `mdp route --entries` and `mdp brief --context` surface that metadata for agents, but the CLI does not enforce unknown metadata keys. Do not add arbitrary sibling fields to entries, cards, or manifests as if they were supported contract fields; `mdp validate` warns that unsupported fields are ignored. For custom channels, add the channel string to `.mdp/manifest.yaml` `supported_channels`, then write matching channel-policy entries.

## Use A Prospect Or Source Row

Convert an existing prospect/source row, CSV row, CRM export row, research note, Clay/Deepline row, spreadsheet row, or user-provided source row into a small JSON file under a repo-ignored agent artifacts directory or another ignored scratch path unless the user explicitly wants to commit a sanitized example. Prefer the pack-owned `.mdp/prompts/normalize-prospect.yaml` contract for messy rows; save its `normalized_prospect` output as the file that feeds `mdp fit`. Do not commit private prospect data. Check the expected shape:

```bash
mdp --json schema prospect
```

Minimum admission fields remain `name`, `title`, and `company` for compatibility. New lead workflows should also supply `company_domain` as the stronger account key. The CLI canonicalizes supplied domain-like values such as `https://www.apple.com/` to `apple.com`; it does not browse, DNS-check, enrich, or infer a domain from the company name.

Prefer adding `company_domain`, `linkedin_url`, `company_url`, `background`, `trigger`, `persona`, `segment`, structured `signals`, bounded `attributes`, `source_kind`, and `synthetic` when relevant. Use `attributes` only for reviewed metadata such as fiscal year or segment tier; put source evidence in `signals[].source`.

Packs may declare readiness requirements in `.mdp/manifest.yaml`:

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

Use provider-neutral `source_kind` values unless the source itself matters: `user-provided-row`, `csv-row`, `crm-export-row`, `clay-row`, `deepline-row`, `private-scratch-row`, `sanitized-example`, or `synthetic-example`. Clay is one possible source, not the default MDP mental model.

Normalization prompts may map messy titles into pack-owned personas and signals, but they must preserve raw evidence, uncertainty, missing fields, and disqualifying execution asks. When using any `.mdp/prompts/*.yaml` prompt contract, treat `output_contract.schema_ref` as the response contract; if the prompt includes `output_contract.schema`, give that literal schema to the model or host. `output_contract.example` is only a reference. The CLI still owns final fit and route decisions.

If the input is account-only and lacks a person name and title, do not invent a contact. Ask for the missing person fields or return the fit gate's insufficient-context decision.

If `persona` is missing, the CLI can resolve it from pack-owned `.mdp/manifest.yaml` `persona_mappings.title_keywords`. Treat `persona_resolution.source: builtin.title_keywords` or `fallback` as review-needed; those weak fallbacks do not make a prospect fit-ready by themselves.

Generated starter rows include `source_kind: synthetic-example` and `synthetic: true`. Treat those as demo fixtures, not real prospects. For production work, use a real row in ignored scratch or an intentionally sanitized example.

## Outbound Testing Without A Prospect

When the user wants cold email, LinkedIn, or outbound-copy testing but has not supplied a real or intentionally sanitized prospect row, generate fake fixtures before drafting:

```bash
mdp sample-leads --dir . --persona "<persona>" --job "<channel> outbound copy" --count 3 --format yaml
```

Use this workflow:

1. Generate 2 to 5 fixture leads.
2. Save one fixture row to ignored scratch if a CLI command needs `--prospect`.
3. Route each fixture through MDP, then run `mdp fit` and `mdp brief --context`.
4. Draft only against `safe_personalization` and `known_gaps`; label weak context as a hypothesis.
5. Run `mdp check-claims` before treating copy as ready.
6. Never treat fixture leads as real prospects.

Fixture leads are `source_kind: synthetic-example`, `synthetic: true`, and `do_not_contact: true`. Do not enrich, research, upload, sequence, send to, or imply they represent real people or accounts.

Run fit first and stop on `disqualified` or `insufficient-context` unless the user explicitly overrides. If the user only asked whether a row should be messaged, return the `mdp fit` decision, matched rules, disqualifiers, `context.missing_requirements`, `context.invalid_requirements`, and gaps instead of drafting or creating a parallel evaluation.

Then create a brief:

```bash
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel <channel>
```

If the user expects a created artifact, save it explicitly:

```bash
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel <channel> --out .mdp/briefs/<brief-name>.json
```

Preview the brief artifact write before mutating the pack when needed:

```bash
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel <channel> --out .mdp/briefs/<brief-name>.json --dry-run
```

Read `data.context.entries` first. Open `data.context.full_card_required` paths only when present. Draft only when `data.draft_status` is `ready`.

## Route Without A Prospect

```bash
mdp --json --summary route --entries --eval-fixture --dir . --persona "VP Finance" --job "linkedin outbound copy"
mdp --json emit-brief --dir . --persona "VP Finance" --job "linkedin outbound copy"
```

Direct persona/job commands resolve pack-owned persona aliases before routing. Check `requested_persona` and `persona_resolution` before drafting so alias resolution stays visible.

Use `load_order` or `required_load_order` as the progressive-disclosure contract.


Before drafting from a prospect row, check fit:

```bash
mdp --json fit --dir . --prospect <prospect.json>
```

Before approving generated copy, check claims and guardrails:

```bash
mdp --json check-claims --dir . --text "<draft copy>"
```

When route-specific constraints or subject rules matter, include the subject and route:

```bash
mdp --json check-claims --dir . --text "<draft copy>" --subject "<subject>" --persona "<persona>" --job "<channel> outbound copy"
```

Add `--strict` when advisory constraint warnings should fail the approval gate.

For pack QA:

```bash
mdp --json gaps --dir .
mdp --json eval --dir .
mdp --json eval --strict --dir .
```

Use `--summary` for compact status instead of piping JSON into one-off scripts.

## Demo Copy

For local demos only:

```bash
mdp --json copy --dir . --prospect <prospect.json> --channel <channel>
```

For production-quality output, use `brief` and draft from the returned contract and routed cards.

## Agent Framework Wrappers

Frameworks such as Flue or Vercel Eve may wrap MDP for webhook admission, durable runs, filesystem staging, model drafting, and artifact collection. Keep that layer as an adapter around the CLI:

1. Verify and normalize the inbound event in trusted application code.
2. Use `.mdp/prompts/normalize-prospect.yaml` when an upstream AI normalizer is needed; preserve its `normalization_trace`.
3. Write the raw payload and normalized prospect JSON to ignored scratch.
4. Run `mdp --json fit` before drafting.
5. Run `mdp --json brief --context` and draft only from the returned brief/context.
6. Run `mdp --json check-claims` before treating draft text as ready. Include `--subject`, `--persona`, and `--job` when routed constraints cover subject length, word count, max questions, or forbidden links/html/tracking.

Do not move fit logic, route selection, claim checks, or card interpretation into the framework layer. Do not let the framework wrapper send, schedule, enrich, scrape, update a CRM, or write to a sequencer unless the user explicitly asks for that separate system action outside MDP.

## Boundaries

- Do not send LinkedIn messages or emails.
- MDP stops at pack, route, fit, claims, gaps, evals, and brief contracts.
- Sending, scheduling, enriching, CRM updates, Clay/Deepline writes, or sequencer work requires a separate exact-action handoff/tool outside MDP and explicit user approval.
- Do not call MDP an AI SDR, CRM, sequencer, enrichment provider, BI tool, or generic automation system.
- Do not invent missing claims. Surface gaps in the brief.
- Keep `--json` on when another agent, script, or tool will parse the output.
- Use `--dry-run` before selected local write paths (`init`, `brief --out`, `emit-brief --out`, `pack --out`) when mutation should be previewed.
- Handle JSON failures by `error.code` first. Stable codes include `pack_not_found`, `invalid_manifest`, `missing_card`, `unsupported_claim`, `insufficient_context`, `write_conflict`, `invalid_argument`, and fallback `mdp_error`.
