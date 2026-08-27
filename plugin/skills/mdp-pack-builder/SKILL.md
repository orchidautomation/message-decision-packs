---
name: mdp-pack-builder
description: Use when creating, initializing, reconstructing, or improving a Message Decision Pack from approved GTM, ICP, source, RFP, proposal, or capture material. Do not use for generic research, messaging strategy, proposal writing, or pack-only review.
---

# MDP Pack Builder

Build evidence-grounded `.mdp/` decision context. Use the CLI for deterministic structure and validation; use judgment only for interpreting approved source material and authoring explicit decisions.

## Communicate The Work

Follow the shared [Orient, Plan, Progress, Translate, Close contract](../mdp/references/communication-contract.md).
Open by naming the source-plan, source-extract, GTM-authoring, or
proposal-authoring job; the exact pack and approved-source evidence boundary;
the pack files and readiness handoff the user will receive; and that this skill
will not scrape gated sources, invent authority, draft outreach or proposals,
send, or mutate downstream systems. Keep later updates to meaningful
validation gates, blockers, and authoring decisions.

## Authority Monotonicity

The Rust CLI is the decision authority. Preserve or reduce its authority; never upgrade `blocked`, `no-draft`, `unavailable`, invalid, or unknown results to ready, needs-review, transport success that implies decision success, or usable governed generation. New evidence requires a new CLI evaluation; user intent cannot override an existing result in place.

## Intake Gate

1. Identify the pack root and intended profile: `gtm` or `proposal`.
2. For a real GTM pack, resolve the external company, product, or project being positioned separately from the pack display name. Record known aliases and prior-target or starter terms that must be excluded.
3. Classify each source as user-approved local material, approved corpus, public unauthenticated source, synthetic/sanitized example, needs approval, or excluded.
4. Ask for source authority when access or confidentiality would materially change the work. Never scrape gated sources or commit restricted source material.
5. Inspect the runtime and existing pack before editing:

```bash
mdp --json skills --dir PACK_ROOT
mdp --json doctor --dir PACK_ROOT
```

An invalid or absent pack still leaves this shared skill bootstrap-eligible.

## Initialize Or Inspect

For a new pack, preview then initialize:

```bash
mdp --json init --template gtm --name "PACK_NAME" --target-name "TARGET_NAME" --target-kind company --target-alias "TARGET_ALIAS" --exclude-term "PRIOR_TARGET" --dir PACK_ROOT --dry-run
mdp --json init --template gtm --name "PACK_NAME" --target-name "TARGET_NAME" --target-kind company --target-alias "TARGET_ALIAS" --exclude-term "PRIOR_TARGET" --dir PACK_ROOT
mdp --json init --template proposal --dir PACK_ROOT --dry-run
mdp --json init --template proposal --dir PACK_ROOT
```

Repeat `--target-alias` and `--exclude-term` as needed. A custom GTM pack name is not a substitute for `--target-name`; do not author into an ambiguous or previously targeted directory.

For an existing pack, run:

```bash
mdp --json validate --dir PACK_ROOT
mdp --json explain --dir PACK_ROOT
mdp --json gaps --dir PACK_ROOT
```

## Stage Every Multi-File Change

Never make a multi-file authoring pass directly in the live pack. Copy the
complete pack to a separate candidate directory outside the live tree, make
all intended `.mdp/` edits there, refresh candidate-owned projections, and
then seal a preview:

```bash
mdp --json readme refresh --dir CANDIDATE_ROOT
mdp --json author preview --dir PACK_ROOT \
  --candidate CANDIDATE_ROOT --out CHANGE_SET_JSON
```

`author preview` uses the normal pack validator, writes no live-pack files but
always creates the required `--out` change-set file,
and returns bounded `created`, `changed`, `unchanged`, and `deleted` path lists
without file bodies. Review those lists and the candidate itself. Do not treat
a generated candidate or a successful preview as reviewed authority.

Only after review, apply that exact sealed change set:

```bash
mdp --json author apply --dir PACK_ROOT \
  --candidate CANDIDATE_ROOT --change-set CHANGE_SET_JSON
```

Apply refuses exact live or candidate paths that changed after preview. A
handled publication failure rolls back all MDP-owned writes and reports the
rolled-back paths; it never overwrites a concurrent edit. Preserve the
candidate and change-set file for inspection when apply is refused or recovery
is indeterminate. Files outside `.mdp/` and runtime output directories are not
author-managed. This workflow is filesystem-native and does not require or
create a Git repository, commit, or branch.

Every newly initialized or one-prompt-generated GTM pack must keep the starter
prospect Decision Input Contract or replace it with an equally complete
pack-specific contract before it is presented as governed or self-standing.
Bind all prospect-driven canonical jobs directly or transitively through
`input_contracts[]`, then require `requirements.data.available: true` for each
exact job ID. An empty declaration is not a placeholder that passes this gate.
Do not infer a contract from prompt prose, field names, `signals`, or
`lead_input_requirements`. Existing unbound packs remain compatible and
unassessed; upgrading them is an explicit authoring decision.

## Load Only The Needed References

- Read [references/source-intake.md](references/source-intake.md) when planning sources, extracting evidence, normalizing messy material, or mapping profile vocabulary to primitives.
- Read [references/decision-input-contracts.md](references/decision-input-contracts.md) when a job needs an explicit attempted-complete data contract before normalization, fit, routing, or drafting.
- Read [references/gtm-authoring.md](references/gtm-authoring.md) for ICP, personas, fit, signals, message angles, CTA policy, and GTM job bindings.
- Read [references/proposal-authoring.md](references/proposal-authoring.md) for proposal opportunity context, requirements, proof, confidentiality, and proposal job bindings.
- Read [references/boundaries-output.md](references/boundaries-output.md) when authoring claims, avoid rules, output constraints, or proof-carrying artifacts.

Do not read every reference by default.

## Author Product Foundation

Use the ten existing primitives and card kinds as authority. Under
`profile.product_foundation.facets`, index exact existing card/entry refs and
explicit gap refs; do not copy product claims into the manifest. Under each
exact canonical `jobs[].id`, classify facet IDs as `required`, `conditional`,
`optional`, or `excluded`. Static conditional facts are limited to exact
`manifest_id`, `profile_id`, or `job_id` equality. Record only explicit
structural `conflicts_with`; never infer semantic conflicts or choose a
precedence winner from prose.

After each job binding, inspect the CLI-resolved foundation first:

```bash
mdp --json skills --dir PACK_ROOT --job JOB_ID
mdp --json requirements --dir PACK_ROOT --job JOB_ID
```

Use exact canonical job IDs. Confirm required and triggered facets are the only
selected context and optional, excluded, unrelated-job, and false conditional
facets do not leak. Preserve unsupported target, ICP, claims, proof,
certification, integration, outcome, RFP, and past-performance facts as gaps;
never invent them to make a job ready.

Scaffold or update `.mdp/README.md` only as concise secondary navigation over
structured authority. It cannot satisfy readiness. Because it is inside
`.mdp/`, changing it changes the portable pack hash even when foundation
resolution is unchanged. Do not turn it, the registry, or a new card kind into
a company wiki or an eleventh primitive. The generated README owns one
machine-generated inventory block delimited by `<!-- mdp:readme-inventory v1 begin -->`
and `<!-- mdp:readme-inventory v1 end -->`; it projects exact card entry
counts, prompt ids, and source ids from loaded structured authority. Never
hand-author or hand-maintain numeric inventory inside or outside that block.
After any card, prompt, or source change, regenerate only the owned block:

```bash
mdp --json readme refresh --dir PACK_ROOT
```

Do not finish the authoring loop while `mdp --json readme check --dir PACK_ROOT`
reports `stale`; a fresh generated pack must keep its owned inventory fresh,
and `validate --strict` treats `readme_inventory_drift` as a blocker. Legacy
READMEs without the owned marker remain orientation-only and unassessed.

Foundation `ready` is veto-only: it never establishes sufficient-for-job or
self-standing status and never overrides another failed gate or explicit
`profile_eval.activation.status: needs-review|blocked`.

## Classify Established Authority And Boundaries, Not Gaps

A selected required gap is a hard veto, so the gap list is for genuinely
unresolved job insufficiency, not for recording every limit or policy. Before
classifying a facet, decide whether the source establishes authority, an
approved boundary, or a real hole.

- **Established authority** belongs in `entries` of the matching facet kind.
  Approved terminology, scoped proof, allowed outcomes, differentiators, and
  approved motions/CTAs are entries in `claims`, `outcomes`, `differentiators`,
  `proof_boundaries`, `terminology`, and related facets.
- **Approved boundary or partial-but-usable authority** belongs in `entries` of
  a guardrail facet (`proof_boundaries`, `product_exclusions`, or the relevant
  `avoid`/`output` cards). A rule such as “case-specific outcomes are allowed;
  generic averages are prohibited” is one avoid/output entry, not a gap. So is
  “approved terminology is set; naming outside it requires review” and
  “case-led proof is allowed; portfolio-wide extrapolation is prohibited.” The
  facet is `ready` because the boundary is explicit authority.
- **Genuine unresolved insufficiency** belongs in `gaps`. Use a gap ref only
  when no approved source establishes the required authority at all — missing
  alternatives, missing outcomes, missing proof, or missing certifications.
  A gap must name what is not established and who must resolve it, never what is
  already approved and bounded.

Do not represent an approved boundary as a gap to “be safe.” That misrepresents a
ready job as incomplete and blocks the pack while describing established
authority as unresolved. Conversely, do not close a real gap by relabeling
missing authority as a boundary; an explicit selected required gap is still a
veto, and a closed-looking gap that points at unresolved authority still blocks.
Keep this classification in the builder and reviewer; the Rust CLI never infers
gap meaning from prose, and it never auto-closes a gap from keywords such as
“approved” or “resolved.”

## Authoring Loop

1. Preserve source receipts: source ID, file or URL, snippet, observed/as-of date, confidence, and approval class.
2. Map reviewed facts into universal primitives; keep profile terminology in labels and entries.
3. Separate observed evidence from inferred decisions. Put unresolved or unsupported material in gaps.
4. Keep every prospect-facing surface about the resolved external target. Pack, CLI, schema, prompt, card, eval, starter, and prior-target vocabulary is internal implementation context only.
5. For a new generated GTM pack, and whenever any job depends on collected or
   normalized data, author and bind its
   `decision_input_contracts` before writing the normalization prompt. Compile
   the job-specific questions and policy:

```bash
mdp --json requirements --dir PACK_ROOT --job JOB_ID
```

The compiled contract, not a generic finder or the normalization prompt, states
what data must be attempted. Keep collection and provider calls outside MDP.
When the decision needs repeated sourced signals, declare
`signal_projections` beside scalar attributes. Each projection needs a stable
qualified ID, profile-owned kind, explicit closed roles, attribute
contributors, bounded cardinality, conservative conflict policy, and decision
effects. Use only `fit`, `why-now`, `person-resolution`, and `disqualifier`
roles. Never derive roles from prose or provider fields, and never use
last-write-wins, newest, highest-confidence, or another positive winner rule.
Signal-aware normalization must declare `mdp.normalized-decision-input.v2`;
structured observations never belong in the legacy prospect signal array.
The canonical prospect-driven jobs are `prospect-fit-or-brief`,
`outbound-copy-brief`, and `outbound-copy-review`; all three must compile the
same intended minimum prospect contract before the pack is called governed.

If an external orchestrator will consume the job, hand its integration owner
the complete requirements result plus `mdp --json schema source-binding`.
Keep the resulting version-compatible `mdp.source-binding.v1` or
`mdp.source-binding.v2` artifact outside `.mdp`, and require:

```bash
mdp --json validate-source-binding --dir PACK_ROOT \
  --job JOB_ID --file SOURCE_BINDING_JSON
```

before activation. Do not invent provider enums, credentials, legacy provider
IDs, execution roles, or row-result storage in the pack. The binding must pin
the portable pack and requirements digests, binding and normalization releases,
and exact Decision Input Contract receipts.

6. Author prompts with explicit input and output contracts. Inspect the selected
   prompt's `output_contract.output_kind` and `output_contract.contract` before
   validating model-produced output:

   - For every self-standing generation or review job, bind exactly one
     `jobs[].model_task` to a versioned `mdp.prompt.v1` prompt. Declare its
     kind, role, objective, ordered procedure, every input producer, selection
     and evidence rules, ambiguity and provenance policy, negative examples,
     final checklist, and exact `governed-artifact` JSON Schema. Use
     `requirements --job` to inspect the compiled prompt/hash. Do not put this
     behavior only in a skill, and do not add a model runner to MDP.

   - Declare positive `jobs[].context_budget.max_entries` and `max_bytes` for
     every self-standing generation/review job. Require `routed_context` as a
     pack-produced prompt input and `context_sha256` in governed output. Never
     meet a budget by dropping safety/output guardrails or permitting whole-card access.
     A persona label is a structural selector, not a pack-wide inclusion switch.
     Scope `applies_to` to the exact persona/job that needs the entry; do not
     stamp a single persona across case-study, claims, or hooks authority to
     make it reachable. Keep card `personas` and entry `applies_to` selectors
     declared in `manifest.personas`, favor structured `scope` over prose
     persona mentions, and select evidence by the entry's declared job relevance.
     Leave selectors empty only when the card or entry is genuinely global for
     every persona. Empty or blank-only selectors are universal for persona
     applicability; they do not suppress a card or bypass job, scope, policy,
     guardrail, cap, or context-budget gates. Never rely on persona words in
     titles or prose to make a selector match.
     Before declaring the pack finished, run the deterministic route-budget
     preflight for every declared persona/job route:

     ```bash
     mdp --json route-budget --strict --dir PACK_ROOT
     ```

     It fails when any route's selected entry count or canonical byte size
     exceeds the declared budget and reports the reason-code distribution and
     largest contributing cards without leaking entry bodies. `validate --strict`
     runs the same preflight; both must be green before a greenfield generation
     claim. Narrow applicability to fit the budget; do not raise limits.

     For bounded investigation after the unfiltered gate, use the exact
     projections `mdp --json --summary route-budget --dir PACK_ROOT`,
     `--job JOB_ID`, and `--persona PERSONA`. Summary output is body-free and
     reports status counts, entry/byte utilization, bounded contributors,
     aggregate exclusion counts, and a safe next action. The full route matrix
     remains authoritative; required-first exclusions stay visible, and agents
     must never truncate, drop guardrails, or open a full card to conceal
     overflow.

     When a supporting kind is intentionally high-volume, a job may add
     `context_budget.optional_kind_quotas` for safe kinds such as `hooks`,
     `pains`, or `ctas`. These are optional-entry maximums, not required
     reservations: guardrails, selected foundation entries/gaps,
     channel policies, every evidence-backed entry, and explicitly required
     output entries are retained first. `channel-policies` and `gaps` are
     protected and cannot be quota kinds. Inspect `minimality.allocation` and
     the body-free `optional_kind_quota_exceeded` exclusions, and do not use
     quotas to hide required authority or to bypass `max_entries`,
     `max_bytes`, or `max_cards_per_route`.

   - Only when the selected prompt is the job-bound
     `decision-input-normalization` prompt producing
     `mdp.normalized-decision-input.v1` or
     `mdp.normalized-decision-input.v2`, require `data.available` to be `true`
     and hand the customer or host the complete exact requirements result as
     `DECISION_INPUT_REQUIREMENTS_JSON`, including
     `data.source_attempt_request_schema`,
     `data.collected_attempt_results_schema`,
     `data.normalized_output_schema`, and all contract/prompt receipts. The
     customer or host owns collection and paid normalization. It must construct
     one attempted-complete `SOURCE_ATTEMPT_REQUEST_JSON` matching the compiled
     request schema; populate the exact `contract`, `job_id`, and
     `decision_input_contracts` ID/version receipts; set a trusted UTC `as_of`;
     preserve the exact request bytes; and compute
     `SOURCE_ATTEMPT_REQUEST_SHA256`. It must execute every compiled attempt and
     record the exact statuses, values, evidence, timestamps, confidence, and
     errors in `COLLECTED_ATTEMPT_RESULTS_JSON`, matching
     `data.collected_attempt_results_schema`, then compute
     `COLLECTED_ATTEMPT_RESULTS_SHA256`. Invoke the bound prompt with:
     - `raw_row`: `COLLECTED_ATTEMPT_RESULTS_JSON`
     - `decision_input_requirements`: `DECISION_INPUT_REQUIREMENTS_JSON.data`
     - `source_attempt_request_sha256`: `SOURCE_ATTEMPT_REQUEST_SHA256`
     - `collected_attempt_results_sha256`:
       `COLLECTED_ATTEMPT_RESULTS_SHA256`

     Preserve both exact ledgers and pass them to validation:

     The command below is the v2 form. For scalar v1, omit
     `--source-binding` and keep every artifact on the v1 matrix row.

```bash
mdp --json validate-prompt-output --dir PACK_ROOT --prompt BOUND_PROMPT_PATH \
  --source-binding SOURCE_BINDING_JSON \
  --source-attempt-request SOURCE_ATTEMPT_REQUEST_JSON \
  --collected-attempt-results COLLECTED_ATTEMPT_RESULTS_JSON \
  --file OUTPUT_JSON
```

     Stop before extracting or passing normalized data unless validation
     succeeds and the envelope's top-level `outcome` is exactly `ready`.
     Preserve the exact request and collected-results bytes and both SHA-256
     receipts with the normalized result.
     For v2, validate the complete binding/request/results/output chain and
     pass it to `fit` or `brief` through `--normalized-input`; do not extract an
     editable prospect and claim retained lineage. Preserve all observation
     receipts and conflicts. `lineage-validated` means internal consistency,
     not host authenticity, authorization, or source truth.

   - For every other prompt output kind or contract, retain the normal
     `mdp.prompt-output.v0` path without a source-attempt request, regardless of
     the job-wide `data.available` value:

```bash
mdp --json validate-prompt-output --dir PACK_ROOT --prompt-id PROMPT_ID --file OUTPUT_JSON
```

     For a legacy prospect-normalization prompt whose declared output includes
     `normalized_prospect` and
     `normalization_trace.fit_readiness.ready_for_mdp_fit`, require the exact validated
     `normalization_trace.fit_readiness.ready_for_mdp_fit` field to equal
     `true` before fit, brief, routing, or drafting. For other v0 output kinds
     such as card-patch/extraction envelopes, successful contract validation is
     the applicable gate; do not require a normalization trace that the prompt
     does not declare.

     For a `governed-artifact` result, successful schema validation proves the
     declared artifact shape only. Generated prose must still pass the
     applicable `check-claims` or `verify-output` gate.

Legacy prompt output contracts use `source_summary.inputs_used` for exact declared input names only. Put source paths, snippets, page locators, URLs, and proof notes in candidate `evidence`/`provenance`, `signals[].source`, `normalization_trace.preserved_raw_fields`, or `normalization_trace.missing_required[].source_evidence`. The prompt owns extraction/normalization, the manifest owns allowed values and readiness policy, the CLI owns enforcement, and downstream writers own wording only.

For proposal PDF/doc extraction, keep the source-audit ledger bounded and local/customer-controlled, then validate raw-field and snippet refs before using normalized opportunity facts:

```bash
mdp --json validate-prompt-output --dir PACK_ROOT --prompt-id normalize-opportunity --file OUTPUT_JSON --source-audit SOURCE_AUDIT_JSON
```

If this pack-build flow is also proving a sample proposal-review run, create a receipt from a fresh/stateless normalization call. Same-conversation normalization can inform authoring, but it is not audit-grade:

```bash
mdp --json run-receipt --dir PACK_ROOT --workflow proposal-review --isolation isolated --declared-inputs-only --prompt-id normalize-opportunity --prompt-output OUTPUT_JSON --validation VALIDATION_JSON --source-audit SOURCE_AUDIT_JSON --runner-audit RUNNER_AUDIT_JSON --require-runner-audit
```

Use `mdp --json schema runner-audit` for the host-owned native/headless runner evidence. Prefer the canonical CLI path for a proposal sample run. For MCP-capable hosts, use `scripts/mdp-run-mcp-server.mjs` or `${PLUGIN_ROOT}/scripts/mdp-run-mcp-server.mjs` and call `mdp_run_tools` → `mdp_prepare_run` → `mdp_run` → `mdp_verify_run`. Require prepare `out` under the approved work root, pass that request and prepare-returned `request_sha256` to run, and verify the emitted bundle/receipt from the approved output root. Those stages produce the boundary inventory, `mdp.run-request.v1`, run bundle/receipt, and `mdp.run-verification.v1`; MCP adds no authority or isolation assurance. The proposal runner and proposal MCP remain compatibility-only for existing v0 source-intake/receipt consumers. The lower-level optional BYOK native reference runner at `scripts/mdp-native-normalize-openai.mjs` or `${PLUGIN_ROOT}/scripts/mdp-native-normalize-openai.mjs` is also v0 compatibility; dry-run/mock checks require no API key, while a real model call requires the operator's secure `OPENAI_API_KEY`. Pluxx-packaged skills can route users toward the canonical run path, but pack authoring alone does not prove the model context boundary.

Runner contract acceptance and integration support are separate. Consult [canonical runner support matrix](https://github.com/orchidautomation/message-decision-packs/blob/main/docs/headless-normalization-runners.md#canonical-runner-support-matrix) and use only `verified`, `recipe-only`, `unsupported`, or `fixture/mock-only`. Pack authoring, a documented recipe, a schema-valid audit, or MCP transport does not prove a verified integration.

7. Bind each agent-routable job to exactly one canonical `skill_id`. Use only the closed v1 pairs documented in the profile reference.
8. Add realistic pack eval fixtures for proceed, insufficient context, refusal/unsafe output, job routing, and target-isolation failure when the manifest declares a target. Decision-input examples also need synthetic expected outcomes for ready, insufficient-context, disqualified, human-review, malformed, and provider-error.
   Declare every non-empty card-level `personas` and entry-level `applies_to`
   selector in the manifest's declared `personas`, `target_personas`, or
   `operator_roles`. Matching is case-insensitive and preserves
   authored display values. Keep universal selectors empty; do not promote
   role words found only in titles or prose into manifest personas.
9. Validate, fix, and repeat:

```bash
mdp --json validate --dir PACK_ROOT
mdp --json gaps --dir PACK_ROOT
mdp --json eval --dir PACK_ROOT
mdp --json validate --strict --dir PACK_ROOT
mdp --json route-budget --strict --dir PACK_ROOT
mdp --json eval --strict --dir PACK_ROOT
```

Do not finish while normal validation has errors. Use strict validation as the final authoring gate unless the user explicitly accepts documented warnings.

## Post-Build Job Conformance Loop

After strict validation passes, prove exact-job readiness for every advertised
canonical job. A pack is not complete while any advertised job is `pack_ready:
false`; do not describe it as complete, ready, or shippable in that state.

For each exact canonical `jobs[].id`:

```bash
mdp --json skills --dir PACK_ROOT --job JOB_ID
mdp --json requirements --dir PACK_ROOT --job JOB_ID
```

Inspect, for every job:

- `product_foundation.status` — `ready`, `blocked`, or `unassessed`.
- `selected_facet_ids` and, from `requirements`, each selected facet's exact
  `entry_refs` and `gap_refs`. Confirm approved authority is classified as
  entries and only genuine unresolved insufficiency appears as `gap_refs`.
- `pack_ready`, missing primitives, profile activation, and the compiled model
  task. A `false` value is a real blocker, not a warning to restate as done.

Run the loop for every advertised job even when one job is already ready; a
single ready job never authorizes calling the pack complete while a sibling job
is blocked. Re-author entries/gaps and re-run the loop instead of upgrading a
`false` result in prose.

## Boundaries

- Build decision context, not source-collection infrastructure or execution automation.
- Preserve engine-owned input/output limits, safe display-field allowlists,
  control-character rejection, renderer escaping, and locator non-dereference.
- Do not invent claims, contacts, personas, proof, certifications, compliance status, past performance, pricing, deadlines, or approvals.
- Target identity proves only what is explicitly stated in its cited direct claim. Unsupported category, capability, ICP, outcome, or proof belongs in gaps.
- Do not add old skill aliases, custom routable job IDs, obsolete surface metadata, or host visibility policy.
- Keep public fixtures synthetic or explicitly sanitized.

## Response

Report the profile, sources accepted/excluded, files changed, job bindings, commands run, validation/eval state, and remaining gaps or required human review.

The handoff must report exact-job readiness honestly. List each advertised
canonical job with its `product_foundation.status`, selected facet IDs, entry
refs, gap refs, and `pack_ready`. If any advertised job is `pack_ready: false` or
foundation `blocked`, state that explicitly and name the unresolved gap or
failed gate; do not describe the pack as complete, ready, or shippable while a
job remains blocked. A pack with every advertised job `pack_ready: false` is
blocked, not complete, even when `validate --strict` reports no issues.
