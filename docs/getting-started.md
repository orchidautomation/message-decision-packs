# Getting Started

For multi-file edits to an existing pack, do not author directly in the live
tree. Stage a complete candidate and use `mdp author preview` followed by
`mdp author apply`; see [Failure-safe pack authoring](pack-authoring.md).

Message Decision Packs (MDP) are local/offline files plus a local `mdp` CLI and agent plugin. MDP stores GTM messaging decisions and profile-specific review decisions as routing contracts, fit or review rules, approved claims, avoid-rules, output-rules, and evidence gaps. It does not send messages, update CRM, enrich leads, scrape data, sequence outbound, submit proposals, own approvals, or act as an AI SDR.

Start with `mdp` for a short author/use quickstart. `mdp status --dir PACK_ROOT`
is a read-only local health check: it performs no network discovery, login, or
authentication and reports a safe next command even when the pack is missing.
For agents, use `mdp --json status --dir PACK_ROOT` and
`mdp --json capabilities`; JSON is the authority, not a human summary.

If you want the mental model first, read [Conceptual Decision Flow](conceptual-decision-flow.md). It explains how a provider-neutral prospect/source row moves through fit, persona, pains, hooks, proof, CTA policy, avoid-rules, output-rules, and bounded context for drafting.

If one GTM pack needs shared company rules plus product-, capability-, solution-, or segment-specific pains, proof, hooks, and CTAs, read [Portfolio-Aware GTM Scope](portfolio-scope.md). Those dimensions filter the applicability of existing agnostic primitives; they do not create new primitives or require one pack per product.

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

The installer fetches the latest GitHub Release. `--cli` installs only the `mdp`
binary for your platform. An exact-version repeat is a no-op; use `--force-cli`
only for an intentional repair. `--agents` installs the CLI plus bundles for
hosts detected by Pluxx and ends with one installed/updated/unchanged/skipped/
failed summary. Missing hosts are normal skips. Single-host flags remain strict:
`--codex`, `--cursor`, `--claude-code`, and `--opencode` fail if their requested
installation cannot complete.

MDP `0.1.107` also ships a separate Agent Plugins v1 skills package. Set
`MDP_AGENT_PLUGINS_INSTALL_DIR` to an explicit absolute client-managed
destination and pass `--agent-plugins`. The installer refuses native-tree
overlaps and unknown nonempty destinations. There is no guessed generic Codex
import path. Cursor's documented local path can be used only in portable-only
mode, not together with `--agents` or `--cursor`. See
[Distribution](distribution.md) for the exact boundary and evidence matrix.
Ordinary `--agents` runs do not mention or guess a portable destination; setting
`MDP_AGENT_PLUGINS_INSTALL_DIR` explicitly opts that package into the run.

In `0.1.107`, `mdp-pack-apply` replaces the former `mdp-gtm-brief` and
`mdp-proposal-review` discovery entries. Rediscover the installed skills after
upgrade, then resolve an exact job with `mdp --json skills --dir PACK_ROOT
--job JOB_ID`; the removed names are not compatibility aliases.

## Verify

```bash
mdp --version
mdp --json doctor --dir .
```

`doctor` separates the running CLI installation, requested-pack structural
validity, profile activation, and job readiness. Missing, unreadable,
wrong-format, or structurally invalid packs return exit 1 and JSON `ok: false`.
A structurally valid pack returns exit 0 and JSON `ok: true`; activation can
still be reported as blocked without being mislabeled as structural corruption.
Doctor never assesses a specific job. Use `mdp check --dir PACK_ROOT --job
JOB_ID` for the authoritative job-readiness projection.

For one read-only, offline answer that composes the existing structural,
profile, job, input, and route-budget authorities, select an exact job:

```bash
mdp check --dir ./mdp-demo --job outbound-copy-brief
```

`mdp check` emits `mdp.readiness.v1`. It does not replace `doctor`, `validate`,
`skills`, `requirements`, or `route-budget`; its JSON cites the fields it
projected from each contributing contract. The gates use four states:
`true`, `false`, `unknown`, and `not-applicable`. In particular, omitting a
governed-input validation result is `unknown`, never `false` and never ready.
After validating a normalized input, supply the exact JSON result to include
that authority in the projection:

```bash
mdp check --dir ./mdp-demo --job outbound-copy-brief \
  --input-validation ./validation-result.json
```

Human output names the first blocking or unknown gate and the smallest safe
next action. Use `mdp --json check ...` for the complete machine contract and
`mdp schema readiness-v1` for its schema. Paths, input bodies, and low-level
diagnostic messages are not copied into readiness output.

If `mdp` is not found, make sure the install directory printed by the installer is on `PATH`, then restart your agent host.

Native agent bundles package activation and validation hooks where the host
supports and proves them: detect `.mdp/`, surface MDP guidance, then run focused
validation after relevant pack edits. The Agent Plugins portable package has no
hooks or portable MCP declaration. Do not make hooks silently generate full
briefs, enrich leads, or write private scratch outside documented ignored
paths. See [Agent Hook Guidance](agent-hook-guidance.md).

## Run Outside The Authoring Conversation

Building a pack in Codex, Claude Code, Clay, DeepLine, or another context-rich
host is useful. Running the decision in that same conversation cannot prove
that prior messages, host instructions, discovered files, tools, or retrieval
were excluded. Telling the model to ignore earlier context is not isolation.

For an authoritative local run, first compile the exact request offline from a
selected job/step and declared input paths, then launch the shared runtime:

```bash
mdp --json prepare-run --dir <pack-root> --job <job-id> \
  --operation model:<job-id>/<phase> --model <model> \
  --input <logical-name>=<path> --out <run-request.json> \
  --manifest-out <compile-manifest.json>
mdp --json run --request <run-request.json> --out-dir <new-run-directory>
mdp --json verify-run \
  --bundle <new-run-directory>/run-bundle.json \
  --receipt <new-run-directory>/run-receipt.json \
  --artifact-root <new-run-directory>
```

Keep `<new-run-directory>` outside the pack that produced the request (for
example, `/tmp/mdp-clean-run-<id>` or a customer-controlled job workdir). A
path equal to or beneath the active pack is refused before any output parent,
claim, or transaction is created. Generated run evidence is control-plane
output, not authored pack content; validation reports legacy in-pack evidence
with a move-outside-pack diagnostic and does not delete it.

The result separates declared, observed, enforced, verified, unknown,
redacted, unsupported, and not-applicable evidence. Deterministic GTM runs do
not call a model and therefore mark inference dimensions `not-applicable`.
Generative proposal or generic-driver runs report only the boundary their
runner actually enforced. A new coding-agent task or MCP call is transport, not
proof by itself.

Return the verified authority block to the original conversation unchanged.
Any additional evidence, rewritten decision, or “improved” qualification needs
a new run and receipt. Hosts such as Clay own rows, batching, retries,
credentials, source collection, and downstream actions; MDP owns the frozen
decision contract and validation.

### Use the canonical local MCP path

MCP-capable hosts use one default adapter for every profile:

```bash
node "${PLUGIN_ROOT}/scripts/mdp-run-mcp-server.mjs"
```

Call `mdp_run_tools` to inspect the boundary, then use `mdp_prepare_run` with a
required new `out` path under `MDP_MCP_WORK_ROOTS` to persist
`mdp.run-request.v1`, then pass both that path and prepare's `request_sha256` to
`mdp_run`. The adapter freezes the request and rejects a digest mismatch before
execution; a matched request produces `run-bundle.json` and
`run-receipt.json`, and `mdp_verify_run` to produce
`mdp.run-verification.v1` from those files under `MDP_MCP_OUTPUT_ROOTS`. Each result names the next permitted stage. The
server must start with operator-approved `MDP_MCP_*_ROOTS`; provider-capable
runs also require startup permission, a credential, and a matching one-shot
consent record. Tool arguments cannot grant either permission.

The canonical adapter supports identity-bound prepare publication on Linux and
macOS. It fails closed on Windows or another unsupported host; use the CLI
directly on those hosts.

The MCP adapter transports file-oriented CLI calls and adds no decision
authority or isolation assurance. The older proposal MCP is compatibility-only
and is not part of beginner/default discovery. Existing consumers can follow
the bounded migration in [Local MCP](local-mcp.md).

## Create A Starter Pack

```bash
mdp --json init --template gtm --dir ./mdp-demo --force
mdp --json validate --dir ./mdp-demo
mdp --json eval --dir ./mdp-demo
```

Available templates are:

- `gtm`: the generic GTM messaging starter.
- `proposal`: the synthetic proposal reference profile for bid/no-bid, compliance, proof, red-team, and executive review workflows.

The default GTM template is an intentional MDP reference/demo. To create a pack for an external target, resolve the target first and pass it explicitly:

```bash
mdp --json init --template gtm --name "Example Company Messaging" --target-name "Example Company" --target-kind company --dir ./example-company-mdp
mdp --json validate --dir ./example-company-mdp
```

`--target-kind` accepts `company`, `product`, or `project`. Repeat `--target-alias` for supported external names and `--exclude-term` for every prior target or starter term that must not survive. Initial scaffolding does not accept category, capability, or outcome terms because it has no source receipt capable of proving them; add those to `manifest.target.external_terms` only after adding the supporting source IDs and claims to `.mdp/sources.yaml`. Existing packs without `manifest.target` remain compatible. A custom `--name` without `--target-name` now stops before writing because a display name is not enough to identify what is being positioned. Target-aware `init --force` also refuses a different existing target; use a clean directory or explicitly migrate the old target, lexicon, cards, prompts, examples, and evals before validation.

For the proposal reference profile:

```bash
mdp --json init --template proposal --dir ./mdp-proposal-demo --force
mdp --json validate --dir ./mdp-proposal-demo
mdp --json eval --dir ./mdp-proposal-demo
mdp --json validate-prompt-output --dir ./mdp-proposal-demo --prompt-id normalize-opportunity --file <prompt-output.json>
mdp --json validate-prompt-output --dir ./mdp-proposal-demo --prompt-id normalize-opportunity --file <prompt-output.json> --source-audit <source-audit.json>
mdp --json run-receipt --dir ./mdp-proposal-demo --workflow proposal-review --isolation isolated --declared-inputs-only --prompt-id normalize-opportunity --prompt-output <prompt-output.json> --validation <validation-result.json> --source-audit <source-audit.json> --runner-audit <runner-audit.json> --require-runner-audit
mdp --json verify-output --dir ./mdp-proposal-demo --file ./mdp-proposal-demo/examples/proof-output/valid-binding.json
mdp --json author-proof-output --dir ./mdp-proposal-demo --draft ./mdp-proposal-demo/examples/proof-output-drafts/compliance-row.draft.json --out /tmp/mdp-proof-output.json
mdp --json route --entries --dir ./mdp-proposal-demo --persona "Proposal Lead" --job "bid no bid review"
mdp --json gaps --dir ./mdp-proposal-demo
```

The proposal starter also includes proof-output draft examples under `examples/proof-output-drafts/`. Use `mdp author-proof-output` to compile a draft into verified `mdp.proof-output.v0` JSON, then keep `mdp verify-output`/the embedded `check-claims` result as the machine source of truth. See [Proof-Output Drafting](proof-output-drafting.md).

The proposal starter does not create prospect rows or outbound fixtures. It is a synthetic proposal review profile for bid/no-bid, compliance, proof, red-team, and executive review workflows. Its `normalize-opportunity` prompt normalizes messy proposal/RFP context into bounded profile vocabulary for local validation; `verify-output` checks proof-carrying generated text against real pack IDs before the text is trusted. Neither command submits, scrapes, enriches, certifies, or manages proposal work.

Production runs should use the canonical `mdp run` path directly or through
the profile-neutral local stdio MCP adapter and require the exact CLI-owned
evidence appropriate to the selected contract. The older proposal runner/MCP
remains compatibility-only; its synthetic fixtures live under
`scripts/fixtures/proposal-runner/` for tests, not as a recommended demo.

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

Before routing a profile-sensitive job, use its exact canonical manifest ID
and inspect the resolved product foundation:

```bash
mdp --json skills --dir ./mdp-demo --job prospect-fit-or-brief
mdp --json requirements --dir ./mdp-demo --job prospect-fit-or-brief
```

Read the CLI result before `.mdp/README.md`. Foundation status is
`unassessed`, `ready`, or `blocked`; selected gaps, missing references, empty
facets, and selected explicit conflicts block. `ready` only clears this one
veto and never proves the pack sufficient-for-job or self-standing. A targeted
starter normally remains `needs-review` with explicit product/ICP/proof gaps
until reviewed sources replace those gaps. Never invent the missing facts.

The README is secondary orientation. Editing it changes the portable pack hash
because it lives under `.mdp/`, but it cannot change resolver or readiness
authority. See [Product Foundations](product-foundations.md).

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

Before choosing the legacy row path, inspect the installed CLI and the exact
job contract:

```bash
mdp --json capabilities
mdp --json requirements --dir ./mdp-demo --job prospect-fit-or-brief
```

If requirements returns `runtime_contract_version: v2`, use its exact public
version matrix and schemas. Structured observations belong only in the v2
normalized envelope. Preserve and validate the exact source-binding, request,
collected-results, prompt, and normalized-output chain, then run:

```bash
mdp --json fit --dir PACK_ROOT --job JOB_ID \
  --normalized-input NORMALIZED_INPUT.json \
  --prompt BOUND_PROMPT \
  --source-binding SOURCE_BINDING.json \
  --source-attempt-request SOURCE_ATTEMPT_REQUEST.json \
  --collected-attempt-results COLLECTED_ATTEMPT_RESULTS.json
```

`brief --context` accepts the same lineage arguments. Do not extract an edited
prospect and pass it through `--prospect`. Detached input is compatible only
for a selected job with no direct or transitive Decision Input Contract. A
governed job fails closed with `governed_job_requires_normalized_input` and no
draft authority. Explicit `fit`, `why-now`,
`person-resolution`, and `disqualifier` roles come from pack projections, not
keywords. Stale, weak, blocked, errored, malformed, or unresolved-conflict
observations stay ineligible and no-draft.

For a complete synthetic Clay-shaped example, run the commands in
[Clay Audiences self-serve enterprise expansion](../examples/clay-audiences-self-serve-enterprise-expansion/README.md).
It demonstrates the v2 binding/request/results chain without browsing, Clay
access, credentials, enrichment, CRM writes, drafting, sending, or scheduling.
For a manual legacy-to-v2 conversion, follow
[Decision Input Contracts](decision-input-contracts.md#manual-legacy-to-v2-adoption).

For researched evidence, begin with the selected job's compiled collection and classification contract:

```text
.mdp/prompts/normalize-prospect.yaml
```

Run `requirements` first. The compiled artifact tells the host what evidence to collect and supplies the exact closed taxonomies, definitions, indicators, exclusions, contributor requirements, and hashes the normalization model must use:

```bash
mdp --json requirements --dir ./mdp-demo --job prospect-fit-or-brief
```

For a current v3 normalization producer, also compile its bounded model
context:

```bash
mdp --json requirements \
  --dir ./mdp-demo \
  --job prospect-fit-or-brief \
  --model-context
```

Persist only the response's `data` object as
`decision-input-requirements.json`. That object is the
`mdp.requirements-model-context.v1` contract. It carries the exact taxonomy
definitions and enum values the model may classify, while the full
`mdp.requirements.v2` response remains the host-side authority. The host then
passes this artifact, the source binding, the source-attempt request, and the
collected-attempt-results ledger to the normalization step. The model returns
semantic classifications and explanations only; MDP seals provenance and
hashes before deterministic fit and routing.

The host may use local files, a CRM, a browser, a customer agent, or another approved tool to fulfill that provider-neutral collection specification. It returns attempted-complete evidence with stable attempt IDs and provenance. The normalization model returns only `classifications`, `gaps`, and `rejected_claims`; the runtime validates every enum and evidence reference, then host-wraps the neutral `mdp.normalized-decision-input.v3` envelope. The model never echoes hashes and never chooses fit, route, pursuit, approval, or draft authority.

Proposal packs use `.mdp/prompts/normalize-opportunity.yaml` through the same v3 mechanism. Buyer, requirement, proof, timing, policy-conflict, and source-safety facts are observed. Proposal stage and category are classified from the compiled taxonomy. Pursue, review, or decline remains a deterministic policy result.

```bash
mdp --json requirements --dir ./mdp-proposal-demo --job bid-no-bid-review
```

Use the canonical `mdp_run_tools` → `mdp_prepare_run` → `mdp_run` → `mdp_verify_run` path for new local CLI/MCP integrations. Receipts and verification remain the assurance boundary. The older `validate-prompt-output`, `normalized_prospect`, `normalized_opportunity`, and `existing_pack_context` proposal workflow remains readable only through its explicitly labeled compatibility runner; new v3 producers must not emit those aliases.

Ambiguous, no-match, unsupported, missing, stale, conflicting, or ineligible evidence stays explicit and blocks deterministic downstream authority as defined by the pack. Do not invent proof, certifications, compliance status, deadlines, RFP text, past performance, pricing, evaluator criteria, approval status, or person context.

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

If fit returns `disqualified` or `insufficient-context`, do not draft from that result. Supply new evidence and run a new MDP evaluation; only a new ready result can grant draft authority.

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

This legacy skills-only path is not evidence that a client supports or
discovers the released Agent Plugins package. Treat host discovery, portable
package placement, and CLI installation as separate checks.
