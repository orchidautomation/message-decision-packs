# Getting Started

Message Decision Packs (MDP) are local/offline files plus a local `mdp` CLI and agent plugin. MDP stores GTM messaging decisions and profile-specific review decisions as routing contracts, fit or review rules, approved claims, avoid-rules, output-rules, and evidence gaps. It does not send messages, update CRM, enrich leads, scrape data, sequence outbound, submit proposals, own approvals, or act as an AI SDR.

If you want the mental model first, read [Conceptual Decision Flow](conceptual-decision-flow.md). It explains how a provider-neutral prospect/source row moves through fit, persona, pains, hooks, proof, CTA policy, avoid-rules, output-rules, and bounded context for drafting.

## Install

Install the CLI and supported agent bundles:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

Install only the `mdp` CLI:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y
```

Portable shell fallback:

```bash
curl -fsSL https://mdp.orchidlabs.dev/install.sh | bash -s -- --agents -y
```

CLI-only portable shell fallback:

```bash
curl -fsSL https://mdp.orchidlabs.dev/install.sh | bash -s -- --cli -y
```

The installer fetches the latest GitHub Release. `--cli` installs only the `mdp` binary for your platform. `--agents` installs the CLI once, then installs Pluxx-generated bundles for supported agent hosts. Single-host flags are also available: `--codex`, `--cursor`, `--claude-code`, and `--opencode`.

## Verify

```bash
mdp --version
mdp --json doctor --dir .
```

If `mdp` is not found, make sure the install directory printed by the installer is on `PATH`, then restart your agent host.

Supported agent bundles package activation and validation hooks where the host supports them: detect `.mdp/`, surface MDP guidance, then run focused validation after relevant pack edits. Do not make hooks silently generate full briefs, enrich leads, or write private scratch outside documented ignored paths. See [Agent Hook Guidance](agent-hook-guidance.md).

## Create A Starter Pack

```bash
mdp --json init --template gtm --name "Example Message Pack" --dir ./mdp-demo --force
mdp --json validate --dir ./mdp-demo
mdp --json eval --dir ./mdp-demo
```

Available templates are:

- `gtm`: the generic GTM messaging starter.
- `proposal`: the synthetic proposal reference profile for bid/no-bid, compliance, proof, red-team, and executive review workflows.

For the proposal reference profile:

```bash
mdp --json init --template proposal --dir ./mdp-proposal-demo --force
mdp --json validate --dir ./mdp-proposal-demo
mdp --json eval --dir ./mdp-proposal-demo
mdp --json validate-prompt-output --dir ./mdp-proposal-demo --prompt-id normalize-opportunity --file <prompt-output.json>
mdp --json verify-output --dir ./mdp-proposal-demo --file ./mdp-proposal-demo/examples/proof-output/valid-binding.json
mdp --json route --entries --dir ./mdp-proposal-demo --persona "Proposal Lead" --job "bid no bid review"
mdp --json gaps --dir ./mdp-proposal-demo
```

The proposal starter does not create prospect rows or outbound fixtures. It is a synthetic proposal review profile for bid/no-bid, compliance, proof, red-team, and executive review workflows. Its `normalize-opportunity` prompt normalizes messy proposal/RFP context into bounded profile vocabulary for local validation; `verify-output` checks proof-carrying generated text against real pack IDs before the text is trusted. Neither command submits, scrapes, enriches, certifies, or manages proposal work.

The starter creates:

```text
mdp-demo/
  .mdp/
    manifest.yaml
    sources.yaml
    briefs/
    cards/
    evals/
    prompts/
  examples/
```

## Route Context

Ask which cards matter for a persona and job:

```bash
mdp --json --summary route --entries --eval-fixture --dir ./mdp-demo --persona "PMM" --job "linkedin outbound copy"
```

For the proposal reference profile:

```bash
mdp --json --summary route --entries --eval-fixture --dir ./mdp-proposal-demo --persona "Executive Reviewer" --job "red team gap review"
```

Agents should load only the returned cards instead of reading the entire pack by default.

Use the returned `eval_fixture` as a scaffold for route tests. Review it before committing so evals encode intended behavior, not accidental routing noise.

For outbound-copy testing without a real or intentionally sanitized prospect row, generate fake fixture leads before drafting:

```bash
mdp sample-leads --dir ./mdp-demo --persona "PMM" --job "initial email outbound copy" --count 3 --format yaml
```

These rows are deterministic synthetic example fixtures with `source_kind: synthetic-example`, `synthetic: true`, and `do_not_contact: true`. Route, fit, and brief each fixture before drafting. Use only `safe_personalization` and `known_gaps` for personalization assumptions, then run `check-claims`. Never treat fixture leads as real prospects.

## Use A Prospect Or Source Row For GTM

Keep private prospect data in ignored scratch unless you intentionally commit a sanitized example. A row can come from a user note, CSV, CRM export, Clay, Deepline, spreadsheet, or research workflow after it is normalized into MDP prospect JSON.

For messy upstream rows, use the pack-owned runtime prompt contract:

```text
.mdp/prompts/normalize-prospect.yaml
```

That prompt asks an upstream agent to return strict JSON with `normalized_prospect`, `normalization_trace`, `gaps`, and empty `card_patches`. When invoking it, include the relevant pack context: personas, `persona_mappings`, `lead_input_requirements.value_contracts`, `attribute_definitions`, `allow_undeclared_attributes`, fit rules, signal definitions, avoid-rules, output rules, and source policy. Validate the full prompt output before saving `normalized_prospect` as the prospect JSON file that the CLI will ingest:

```bash
mdp --json validate-prompt-output --dir ./mdp-demo --prompt-id normalize-prospect-row --file ./mdp-demo/scratch/normalize-output.json
```

For proposal packs, use `.mdp/prompts/normalize-opportunity.yaml` the same way for messy opportunity, RFP, capture, requirement, compliance-matrix, proof, or bid/no-bid context. Include proposal personas, value contracts, attribute definitions, source policy, proposal cards, and review jobs in `existing_pack_context`, then run:

```bash
mdp --json validate-prompt-output --dir ./mdp-proposal-demo --prompt-id normalize-opportunity --file <prompt-output.json>
```

If `normalization_trace.fit_readiness.ready_for_mdp_fit` is false, keep the missing context in gaps and structured `normalization_trace.missing_required` entries. Do not invent proof, certifications, compliance status, deadlines, RFP text, past performance, pricing, evaluator criteria, approval status, or person context.

Minimum parser admission is still `name`, `title`, and `company`, but the starter pack's fit-ready requirements are stricter:

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
  required_attributes: []
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

For a real lead row, prefer this shape:

```json
{
  "name": "Alex Rivera",
  "title": "Revenue Operations Lead",
  "company": "ExampleCo",
  "company_domain": "example.com",
  "persona": "GTM Engineering",
  "segment": "agent-assisted GTM",
  "trigger": "standardizing outbound context before agents draft or route campaign briefs",
  "attributes": {
    "fiscal_year": "FY2027"
  },
  "signals": [
    {
      "id": "revops-owner-context-standardization",
      "title": "RevOps owner standardizing campaign context",
      "source": "source row note",
      "confidence": "medium"
    }
  ]
}
```

`company_domain` is canonicalized only from supplied domain-like values. `https://www.apple.com/` becomes `apple.com`; MDP does not browse, DNS-check, enrich, or infer a domain from `company`. Use `attributes` for bounded reviewed metadata like fiscal year or segment tier, and use `signals[].source` for evidence. Prompt output and `fit` readiness also enforce pack-owned value contracts, so values such as `persona`, `segment`, `source_kind`, date/date-time attributes, enum attributes, and declared attributes must match the manifest. If a source row contains an out-of-contract value, preserve it in `gaps` or `normalization_trace`; do not silently rename it into a blessed value.

Then check fit before drafting:

```bash
mdp --json fit --dir ./mdp-demo --prospect ./mdp-demo/examples/clay-row.json
```

If a prospect row has no explicit `persona`, the CLI can use pack-owned `.mdp/manifest.yaml` `persona_mappings` to map title keywords to personas. Unmapped title fallbacks are reported as low-confidence and still require review.

Direct persona/job commands such as `route`, `emit-brief`, and `sample-leads` use the same pack-owned persona mappings. JSON output includes `requested_persona` and `persona_resolution` when an alias is resolved.

If fit returns `disqualified` or `insufficient-context`, do not draft unless the user explicitly overrides.

When fit is acceptable, build the brief:

```bash
mdp --json --summary brief --context --dir ./mdp-demo --prospect ./mdp-demo/examples/clay-row.json --channel linkedin --out ./mdp-demo/.mdp/briefs/example-linkedin.json
```

Draft from the brief's `context.entries`, the prospect context, and any paths in `context.full_card_required`. Use `--out` when the brief should exist as a file; without it, the CLI reports the artifact as stdout-only.

When a human needs to review the prospect without reading the JSON contract, render the same brief as Markdown:

```bash
mdp brief --context --readable --dir ./mdp-demo --prospect ./mdp-demo/examples/clay-row.json --channel linkedin --out ./mdp-demo/.mdp/briefs/example-linkedin.md
```

Readable briefs are review artifacts. The machine source of truth remains `mdp --json brief --context`. The Markdown begins with top-of-file YAML frontmatter for prospect metadata, including `tags` derived from tag-like values such as persona, segment, and source kind, then starts the body with `# Prospect Brief: ...`. The body separates fit/readiness, evidence receipts, gaps and caveats, safe angle, guardrails, copy, follow-up research, and validation/source outputs. If draft copy is present in a future brief payload, it is rendered as Markdown blockquotes.

The same review-layer principle applies to proposal packs, but the artifact should not be called a prospect brief or use prospect/outreach labels. A proposal-readable artifact should use opportunity/review frontmatter, profile-owned proposal vocabulary, and sections such as bid/no-bid read, compliance gaps, requirement status, proof or win-theme receipts, unsupported claims, red-team gaps, and `verify-output` status. Keep that work as a profile-aware proposal review artifact over routed MDP context and proof validation; do not turn it into blank-page proposal generation, proposal management software, legal/procurement approval, or automated submission.

Briefs include `runtime_context` at the top level, and `brief --context` also includes the same object under `context.runtime_context`. It contains `now_utc`, `date_utc`, `timezone: UTC`, and a local-time policy. Use it as run metadata only; fiscal year, renewal date, event date, and campaign-window fields should still come from pack-declared attributes or supplied source context.

The generated `examples/clay-row.json` is a synthetic fixture, not a real prospect. It includes `source_kind: synthetic-example` and `synthetic: true`. The fixture name is kept for compatibility; Clay is not required and is not the default source system.

The prospect/source row is where the situational trigger comes from. `trigger` is optional, but when present it should describe why the outreach is timely. The pack then decides how to use that input:

```text
prospect row
  |
  +-- normalize-prospect prompt -> provider-neutral JSON
  |
  +-- title/persona -> choose persona
  +-- trigger ------> why now
  +-- signals ------> evidence/hypotheses
  |
  v
fit gate
  |
  +-- blocked -> no draft
  |
  v
persona -> pains -> hooks -> claims/proof -> CTA/channel policy
                              |
                              v
                         avoid rules
                              |
                              v
                         output rules
```

`brief --context` makes the selected path explicit in `context.entries`, so agents draft from the relevant persona, pain, hook, proof, CTA, channel, avoid-rule, and output-rule entries instead of loading every card in the pack.

When adding channel rules, keep the starter taxonomy intact: `channel-policies` for channel/lifecycle rules, `output-rules` for generated-text and formatting constraints, `ctas` for ask boundaries and reply paths, and `copy-patterns` for reusable structures like trigger or hypothesis -> proof gap -> approved angle -> one soft CTA.

Do not create a separate row evaluator for this step. The workflow is pack-owned prompt normalization, `mdp fit`, and then `mdp brief --context` only when fit allows it. If the input is account-only and lacks a person name and title, ask for a person row or treat the prospect brief as insufficient-context instead of inventing a contact. Use structured `normalization_trace.missing_required` entries to explain which fields were not available in the source row.

## Source Ledger

Use `.mdp/sources.yaml` before bulk card writing. Add public URLs, user-provided docs, or note identifiers, then separate direct source claims from interpretations and gaps. Cards should cite source ids, URLs, or document names from the ledger when possible.

## Check Claims

Before approving copy, run:

```bash
mdp --json check-claims --dir ./mdp-demo --text "<draft copy>"
```

Unsupported claims, execution claims, compliance/security claims, named-customer claims, quantified outcome claims, and output-rule hits such as blocked punctuation should be fixed or backed with source evidence before use.

## Update

Rerun the installer:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

For CLI-only installs, rerun:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y
```

To check whether your local CLI/plugin version is current:

```bash
scripts/check-update.sh
```

## Long-Tail Skill Clients

For skill-aware agents that are not first-class Pluxx release targets, `skills.sh` can install the `SKILL.md` files only:

```bash
npx skills add https://github.com/orchidautomation/message-decision-packs --skill '*' --agent '*' -g -y
```

This does not install the `mdp` CLI. Use the MDP installer for the full CLI plus agent bundle setup.
