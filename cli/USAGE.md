# mdp usage

## Minimal routed context

Canonical `route --entries` and `brief --context` results include the same `minimality` receipt; `brief --context` also returns the exact `model_context` when ready. Use `brief --routed-context-out PATH` or `emit-brief --routed-context-out PATH` to write MDP's newline-free canonical bytes. Include the reported byte hash under the `routed_context` input in `mdp.prompt-invocation.v1`, and pass both `--invocation-receipt` and `--routed-context` to `validate-prompt-output`. Inspect the closed artifact schema with `mdp --json schema routed-context-v1`. A budget overflow, whole-card fallback, changed context, or out-of-context typed identifier fails closed. Jobs without `context_budget` remain readable with minimality `unassessed`.

`mdp` creates and routes Message Decision Packs.

Card `personas` and entry `applies_to` selectors are structured metadata.
Empty or blank-only selectors match every persona; non-empty selectors match
exactly, case-insensitively, with authored values preserved. Prose is not a
persona selector, and universal applicability still remains subject to job,
scope, policy, guardrail, card-cap, and context-budget gates. Confirm the same
result with `route --entries`, `brief --context`, and `route-budget`.

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
mdp --json requirements --dir /tmp/mdp-demo --job prospect-fit-or-brief
mdp --json validate-prompt-output --dir /tmp/mdp-demo --prompt-id extract-claims-proof --file /tmp/claims-output.json
mdp --json --summary route --entries --eval-fixture --dir /tmp/mdp-demo --persona "PMM" --job "linkedin outbound copy"
mdp --json route --entries --dir /tmp/mdp-demo --persona "PMM" --job "portfolio scope example" --scope product=local-cli
mdp sample-leads --dir /tmp/mdp-demo --persona "PMM" --job "initial email outbound copy" --count 3 --format yaml
mdp --json fit --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json
mdp --json trace --file examples/decision-trace/fixtures/fit-ready-result.json
mdp trace --file examples/decision-trace/fixtures/fit-no-draft-result.json --format mermaid
mdp --json trace --file /tmp/validation-result.json --dir /tmp/mdp-demo --prompt-output /tmp/prompt-output.json --validation-input source_audit=/tmp/source-audit.json --validation-input invocation_receipt=/tmp/invocation-receipt.json
mdp --json --summary brief --context --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin --out /tmp/mdp-demo/.mdp/briefs/example-linkedin.json
mdp brief --context --readable --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin --out /tmp/mdp-demo/.mdp/briefs/example-linkedin.md
mdp render-brief --dir /tmp/mdp-demo --file /tmp/mdp-demo/.mdp/briefs/example-linkedin.json --template gtm-prospect --out /tmp/mdp-demo/.mdp/briefs/example-linkedin.md
mdp --json check-claims --dir /tmp/mdp-demo --text "MDP is a local offline CLI for modular message context."
mdp --json check-claims --dir /tmp/mdp-demo --text "<draft copy>" --subject "<subject>" --persona "PMM" --job "initial email outbound message"
mdp --json gaps --dir /tmp/mdp-demo
mdp --json eval --dir /tmp/mdp-demo
mdp --json copy --dir /tmp/mdp-demo --prospect /tmp/mdp-demo/examples/clay-row.json --channel linkedin
```

## Deterministic synthetic v2 chain preparation

For a signal-aware job, `rebind-synthetic-chain` builds a complete public-safe
fixture outside the pack. It is offline and does not collect evidence, call a
provider, or make synthetic lineage authoritative:

```bash
mdp --json schema synthetic-v2-chain
mdp --json rebind-synthetic-chain \
  --dir PACK_ROOT --job JOB_ID --out-dir /tmp/mdp-chain \
  --as-of 2026-01-01T00:00:00Z --seed 0 --dry-run
mdp --json rebind-synthetic-chain \
  --dir PACK_ROOT --job JOB_ID --out-dir /tmp/mdp-chain \
  --as-of 2026-01-01T00:00:00Z --seed 0 --apply
```

The command stages all four exact-byte artifacts, validates the source binding
and bound prompt output before any destination write, and reports each emitted
digest. Repeating the same inputs is an `unchanged` no-op. Rebinding uses
`--input-dir` only for a chain whose source classes are explicitly
`synthetic_fixture`, whose locators are opaque and non-URL, and whose normalized
prospect has `source_kind: synthetic-example` and `synthetic: true`; real,
customer, private, public-web, URL, or ambiguous provenance is refused. Changed
files require `--apply --force`; force mode writes digest-keyed backups before
atomic replacement. Generated output must remain outside `.mdp` and the source
pack.

The preparation flow is deterministic fit → brief → routed-context → clean-run:
validate the generated files, run `fit --normalized-input` with all three
lineage inputs, then use the existing `brief --context`/routed-context and
clean-run commands. Synthetic validation proves fixture consistency only; it
does not prove source truth or grant copy authority.

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

Use `run-receipt` when a runner or agent host normalized proposal/doc material before deterministic MDP checks ran. For audit-grade proposal review, the host must create a fresh/stateless model call, pass only prompt-declared inputs, save the prompt output and validation result, and include the `mdp.source-audit.v0` ledger. The validation result and runner audit must come from the same run because `run-receipt` compares validation `sha256` values for prompt-output/source-audit and compares runner-audit `prompt_output_sha256` before allowing `audit-grade`:

```bash
mdp --json run-receipt --dir . --workflow proposal-review --isolation isolated --declared-inputs-only --prompt-id normalize-opportunity --prompt-output <prompt-output.json> --validation <validation-result.json> --source-audit <source-audit.json> --runner-audit <runner-audit.json> --require-runner-audit --out <run-receipt.json>
```

A receipt returns `decision: advisory` when normalization used the ambient conversation or when declared-input-only cannot be confirmed. It returns `decision: blocked` when required artifacts are missing, malformed, failed validation, or do not match the artifact hashes recorded by `validate-prompt-output` or the prompt-output hash recorded by `runner-audit`. Use `mdp --json schema run-receipt` for the receipt contract. This command is the v0 proposal compatibility path; use the unified run flow below for new GTM and proposal execution.

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

## Native model steps

The shared run kernel supports deterministic operations and one selected
job-declared model step per generative request. Resolve the exact stable step
IDs first:

```bash
mdp --json requirements --dir PACK_ROOT --job JOB_ID
```

Inspect `data.model_steps`. Job-bound normalization appears before the
job-owned generation or review step. Unbound extraction and authoring prompts
are not executable model steps.

Create one closed `mdp.run-request.v1` whose `operation` equals the selected
step ID, then run and verify it:

```bash
mdp --json schema run-request-v1
mdp --json run --request RUN_REQUEST.json --out-dir NEW_RUN_DIRECTORY
mdp --json verify-run \
  --bundle NEW_RUN_DIRECTORY/run-bundle.json \
  --receipt NEW_RUN_DIRECTORY/run-receipt.json \
  --artifact-root NEW_RUN_DIRECTORY
```

`NEW_RUN_DIRECTORY` must be a new external directory outside the active pack.
The CLI rejects roots that resolve to the pack or a descendant, including
canonical and symlink aliases, before creating output-side state. Existing
generated evidence under a pack is reported for manual relocation; validation
does not delete it. See `mdp --json capabilities` for the stable
`output-directory-inside-pack` error code.

One run executes one normalization, generation, or review step and emits one
receipt. The customer host sequences normalization → deterministic fit/routing
→ generation/review as separate operations. MDP does not collect, batch,
retry, send, mutate CRM, or calculate provider pricing.

The bundled native subprocess is `scripts/mdp-native-model-openai.mjs`. It uses
the official OpenAI Responses endpoint only. Real calls are default-deny and
require both `MDP_ALLOW_NATIVE_MODEL_CALLS=1` and `OPENAI_API_KEY` in the
process environment. Neither value is accepted in a request or MCP tool
argument. Pack validation, step discovery, deterministic runs, and synthetic
mock/dry-run tests are key-free; they do not prove a real provider call.

For MCP-capable hosts, `scripts/mdp-run-mcp-server.mjs` exposes path-only
`mdp_run` and read-only `mdp_verify_run` over the same CLI. MCP is transport
only and adds no execution or isolation authority.

## JSON contract

`mdp trace` accepts one saved CLI result with `--file`, or a complete v1
`--bundle` and `--receipt` pair. Add `--artifact-root` to re-read receipt-bound
artifacts. JSON is the default; `--format mermaid` renders the same canonical
projection. `--out` is the only trace form that writes a file. The command
never mutates pack policy or treats `.mdp/traces` as authority. Inspect the
closed contract with `mdp --json schema decision-trace-v1`.


All commands support `--json`; add `--summary` for compact status output. Run `mdp --json capabilities` when an agent or wrapper needs to inspect command names, coarse side effects, output contracts, `--out` support, dry-run support, strict-mode support, and stable error codes. Validation-style commands return structured data and exit nonzero when `data.valid` is false. Argument parse errors also return JSON when `--json` is present.

## Cold-model conformance

Discover the exact installed command and schema inventory first:

```bash
mdp --json capabilities
mdp conformance --help
mdp --json schema conformance-candidate-v1
```

The normative order is discover → `conformance compile` → stop unless
`sufficient-for-job` → customer-selected host performs and records model calls
→ `conformance validate` → `conformance assemble` → `conformance report` or
`trace`.

```bash
mdp --json conformance compile \
  --candidate CANDIDATE.json --artifact-root STAGED_ROOT \
  --out STAGED_ROOT/deterministic.json

mdp --json conformance validate \
  --artifact-root STAGED_ROOT --candidate CANDIDATE.json \
  --evaluator-inventory EVALUATOR_INVENTORY.json \
  --lifecycle-policy PRIVATE_RECORD_POLICY.json \
  --deterministic deterministic.json \
  --invocation INVOCATION.json --trial TRIAL.json \
  --verifier-receipt VERIFIER_RECEIPT.json \
  --evaluator-result EVALUATOR_RESULT.json \
  --out STAGED_ROOT/behavioral.json

mdp --json conformance assemble \
  --artifact-root STAGED_ROOT --candidate CANDIDATE.json \
  --deterministic deterministic.json --behavioral behavioral.json \
  --trial trials/trial-1.json --out STAGED_ROOT/job-conformance.json

mdp --json conformance report \
  --artifact-root STAGED_ROOT --conformance job-conformance.json \
  --visibility public --generated-at 2026-08-13T12:00:00Z \
  --out STAGED_ROOT/public-report.json
```

Repeat evidence flags for the predeclared trial inventory. `validate` consumes
recorded bytes and makes no model/network call. Its
`mdp.behavioral-evaluation.v1` is intermediate, not report authority. Private
and public reports project the sole cross-phase `mdp.job-conformance.v1`
authority. Public reports and traces omit content, paths, identities,
provider/session metadata, evaluator rationale, and private digests.

MDP does not choose a provider/model, perform a call, calculate pricing, or
grant drafting/sending authority. External calls remain customer-owned and
separately authorized. See
[Cold-model Conformance](../docs/cold-model-conformance.md).

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

`profile.id` and canonical `jobs[].skill_id` bindings are skill-routing metadata. Use `mdp --json skills --dir .` for pack eligibility and `mdp --json skills --dir . --job <job-id>` for one deterministic recommendation. A profile is activation-ready only when `mdp --json validate --dir .` reports `data.profile.activation_ready: true`. Profile-aware manifests declare `required_primitives`, `primitive_map`, `input_contracts`, closed profile jobs, and `profile_eval.required_categories`; validation rejects unknown primitive IDs, unknown or profile-incompatible job/skill pairs, and missing mapped card, prompt, input contract, job, or eval references. Missing required primitive or eval-category coverage is warning-first by default and fails with `--strict`, but the shared `profile_activation` runtime decision still blocks `skills` pack readiness, `requirements` draft permission, route/context/brief output, conformance qualification, and `run` before the driver boundary. Eval fixtures can run `command: validate-prompt-output` with `prompt_id` or `prompt` plus inline `prompt_output` and optional `source_audit`, so profile activation can prove normalization contracts before rows reach `mdp fit` or `mdp brief`.

### Product foundation discovery

An optional `profile.product_foundation.facets` registry indexes exact existing
card entries and explicit gap entries. Each canonical job may classify facet
IDs under `jobs[].product_foundation.required`, `conditional`, `optional`, and
`excluded`. Conditional facts are closed static equality checks over
`manifest_id`, `profile_id`, or `job_id`. `conflicts_with` is explicit
structural metadata; the CLI never infers semantic conflict from prose.

Inspect one exact canonical job:

```bash
mdp --json skills --dir . --job prospect-fit-or-brief
mdp --json requirements --dir . --job prospect-fit-or-brief
mdp --json route --entries --dir . --persona "GTM Engineering" --job prospect-fit-or-brief
```

`skills` exposes a compact status/ID/diagnostic summary. `requirements`
exposes the complete selected facets, exact refs, bounded entry content, and
optional/excluded/untriggered IDs. Route, context, and brief output carry the
exact selected foundation load order. Unknown or free-text jobs are
`unassessed`; they never select foundation authority by token matching.

Statuses are `unassessed`, `ready`, and `blocked`. A selected empty facet,
explicit gap, dangling reference, or explicit conflict with another selected
facet blocks. Optional, excluded, and false conditional facets do not block or
enter selected context. Foundation readiness only vetoes broader readiness:
`ready` never promotes another failing gate and never means sufficient-for-job
or self-standing. The shared computed `profile_activation` decision vetoes
job/profile activation for missing required primitive or eval-category coverage;
explicit `needs-review` or `blocked` states apply the same fail-closed runtime
policy.

`.mdp/README.md` is orientation only. The resolver never reads it, but the
portable pack snapshot includes it like every other regular `.mdp/` file, so a
README-only edit changes the portable hash without changing decision
authority. See [Product Foundations](../docs/product-foundations.md).

Universal primitive IDs are `actors`, `decision-criteria`, `source-signals`, `needs-requirements`, `evidence-proof`, `boundaries`, `output-contracts`, `routing-jobs`, `gaps`, and `evals`. Keep domain terms such as account context or opportunity context in profile-owned card IDs, input contracts, prompts, jobs, and eval fixtures unless a future format explicitly adds a new core card kind.

Portfolio terms do not add primitives. A GTM profile may declare `profile.context_dimensions` such as `product`, `capability`, `solution`, or `segment`, plus generic `context_dimension_dependencies`. Card entries use `scope` to narrow where their existing primitive decision applies. Matching is OR within an entry dimension and AND across dimensions; unscoped entries are global. V1 accepts one runtime value per dimension.

Use repeatable `--scope dimension=value` selectors on `route`, `emit-brief`, and route-scoped `check-claims`. Prospect-driven `fit` and `brief` derive declared scope from scalar `attributes`; a declared `segment` dimension uses the top-level prospect `segment`. Portfolio-sensitive outputs draft from bounded `entry_route.matches` or `context.entries`, not shared card files. Missing/invalid scope blocks drafting, and `verify-output` returns `proof_output_scope_unsupported` for scoped packs until proof artifacts can carry scope. See [Portfolio-Aware GTM Scope](../docs/portfolio-scope.md) for the complete contract and rollout checklist.

Use `mdp --json schema prompt` to inspect the reusable prompt definition contract. Prompt outputs must match the runtime contract named by each prompt's `output_contract.schema_ref`: legacy extraction and normalization prompts use `contract: mdp.prompt-output.v0`; scalar bound normalization uses `mdp.normalized-decision-input.v1`; and signal-aware jobs use the job-compiled `mdp.normalized-decision-input.v2` schema. Structured repeated observations are valid only in the v2 envelope, never in the legacy prospect signal array. `mdp.prompt.v1` adds versioned normalization, generation, and review tasks with explicit input producers, role, objective, procedure, selection and evidence rules, negative examples, and a final checklist. Canonical generation/review jobs bind one prompt through `jobs[].model_task`; `requirements --job` compiles the exact prompt and hash without making a model call. `governed-artifact` prompts retain the `mdp.prompt-output.v0` envelope and declare an exact inline schema for the job artifact. Validate them with `--invocation-receipt <receipt.json>`: the host-created `mdp.prompt-invocation.v1` receipt binds the exact job and canonical prompt identity/hash to per-input SHA-256 values. The host supplies the exact receipt content as `prompt_receipt` and its detached byte hash as the separate `invocation_receipt_sha256` input because the receipt cannot contain its own hash; neither metadata input appears in the receipt's `inputs` array. Legacy starter prompts can inline their full JSON Schema with `mdp init --include-output-schemas`; decision-input normalization does not accept an inline replacement for its job-compiled schema. Extraction prompts preserve `card_patches`, `gaps`, `rejected_claims`, confidence, and provenance; legacy normalization prompts preserve `normalized_prospect`, `normalization_trace`, gaps, and empty `card_patches`. Proposal normalization may also include `normalized_opportunity` as an exact alias of `normalized_prospect`, but existing consumers should continue to read `normalized_prospect`. Prompt files are local decision contracts, not browsing, scraping, enrichment, sending, sequencing, or CRM-update workflows. See [Job-owned Prompt Contracts](../docs/job-prompt-contracts.md).

Treat model-produced prompt output as untrusted review input. Run `mdp --json validate-prompt-output` before copying reviewed `card_patches` into cards or saving `normalized_prospect` for `mdp fit` and `mdp brief`. `source_summary.inputs_used` must name exact declared prompt inputs; source paths, snippets, PDF/page locators, URLs, and field-level provenance belong in candidate `evidence`/`provenance`, `signals[].source`, `normalization_trace.preserved_raw_fields`, or `normalization_trace.missing_required[].source_evidence`. For proposal PDF/doc normalization, pass `--source-audit <source-audit.json>` to check source refs and ref-plus-snippet citations against a bounded `mdp.source-audit.v0` extraction ledger backed by `.mdp/sources.yaml` source IDs. The validator rejects markdown-wrapped JSON, wrong prompt identity, undeclared input references, wrong card kinds, fake-person normalization, candidate ID collisions with existing card entries, normalized opportunity aliases that diverge from `normalized_prospect`, normalized values outside pack-owned value contracts, missing or non-boolean `normalization_trace.fit_readiness.ready_for_mdp_fit`, prompt outputs that claim `ready_for_mdp_fit: true` while missing manifest `lead_input_requirements.required_fields`, `required_signal_fields`, or `required_attributes`, and audited source refs/snippets that do not exist in the supplied source audit.

Prompt-output validation proves the artifact matches the prompt contract and that its readiness claim is internally consistent with the pack input policy. It does not replace `mdp fit`; run `mdp fit` on the reviewed normalized prospect to get the final fit, disqualified, or insufficient-context decision.

## Decision input requirements

A pack may bind one or more versioned `decision_input_contracts` to an input
contract or job. Compile the exact job-specific contract before collection or
normalization:

```bash
mdp --json requirements --dir PACK_ROOT --job JOB_ID
```

The result is `mdp.requirements.v1` for scalar-only jobs and
`mdp.requirements.v2` for jobs with signal projections. Inspect
`data.runtime_contract_version` and `data.contract_version_matrix`; never mix
artifacts across those rows. It includes:

- the exact question, output path, and value contract for every attribute;
- `required`, `optional`, `conditional`, or `hard-gate` classification;
- applicability and deterministic decision effects;
- permitted source classes and public-research policy;
- `observed`, `not_found`, `not_applicable`, `blocked`, and `error` behavior;
- required provenance, confidence, freshness, and sensitivity;
- an attempted-complete `mdp.source-attempt-request.v1` JSON Schema;
- an `mdp.normalized-decision-input.v1` JSON Schema;
- explicit no-draft outcomes and host/MDP ownership boundaries.
- a portable `.mdp` content digest and canonical requirements digest for
  integration release pinning.
- for v2, the exact repeated signal projections, profile-defined kinds, closed
  roles, contributor rules, cardinality, conflict policy, and v2 schemas.

Legacy jobs without a decision-input binding return `available: false` and
remain compatible with existing `lead_input_requirements` behavior.

Job ingress is authoritative. A selected job with a direct or input-contract-
inherited Decision Input Contract accepts qualification and brief authority
only through `--normalized-input` plus the exact required lineage files.
Detached `--prospect` returns non-success with
`mdp.job-ingress.v1` status `blocked` and diagnostic
`governed_job_requires_normalized_input`. Detached compatibility is limited to
a selected job without a governed binding; `--job` is never silently ignored,
and governed multi-job packs require explicit selection.

`requirements` is deterministic and makes no network or model calls. The host
owns source access and paid normalization. The normalization envelope always
sets `draft_allowed: false`; a later `fit` or `brief --context` decision must be
ready before copy generation.

To bind an orchestrator's fields to one exact job, keep the binding outside the
pack and validate it before enabling the integration:

```bash
mdp --json schema source-binding
mdp --json validate-source-binding \
  --dir PACK_ROOT \
  --job JOB_ID \
  --file SOURCE_BINDING.json
```

The compiled requirements select provider-neutral `mdp.source-binding.v1` or
signal-aware `mdp.source-binding.v2`. Each requires exact pack,
requirements, job, and Decision Input Contract pins; complete and unique
qualified attribute coverage; compatible requirement/source classes; binding
and normalization release IDs; and the fixed missing/error status translation.
External field keys may be reused. The command makes no provider or model
calls, and legacy jobs with `available: false` cannot be source-bound.

Signal-aware v2 adds projection mappings and binds the exact source-binding
hash through `mdp.source-attempt-request.v2`,
`mdp.collected-attempt-results.v2`, and
`mdp.normalized-decision-input.v2`. Validate all four artifacts together, then
use the lineage-aware qualification path:

```bash
mdp --json validate-prompt-output --strict --dir PACK_ROOT \
  --prompt BOUND_PROMPT \
  --source-binding SOURCE_BINDING.json \
  --source-attempt-request SOURCE_ATTEMPT_REQUEST.json \
  --collected-attempt-results COLLECTED_ATTEMPT_RESULTS.json \
  --file NORMALIZED_INPUT.json

mdp --json fit --dir PACK_ROOT --job JOB_ID \
  --normalized-input NORMALIZED_INPUT.json \
  --prompt BOUND_PROMPT \
  --source-binding SOURCE_BINDING.json \
  --source-attempt-request SOURCE_ATTEMPT_REQUEST.json \
  --collected-attempt-results COLLECTED_ATTEMPT_RESULTS.json
```

`brief` accepts the same lineage arguments. Detached `--prospect` remains the
legacy path and cannot retain `lineage-validated` authority. That label proves
only that the submitted chain is internally consistent with pack policy;
hashes do not authenticate the host or prove that an observation is true.
Conflicts stay visible. `require-agreement` stops for human review;
`any-disqualifies` may deterministically disqualify. No positive
newest/highest-confidence winner policy exists.

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
