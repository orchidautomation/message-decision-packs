# Minimal context routing

MDP compiles a job-specific context instead of handing a model the whole pack. A canonical job may declare `context_budget.max_entries` and `context_budget.max_bytes`. Jobs without that contract remain compatible, but their minimality status is `unassessed`.

`mdp route --entries` and `mdp brief --context` expose the same `minimality` receipt: status, the SHA-256 of the exact model-visible `mdp.routed-context.v1` object, authored and actual budgets, selected/excluded counts, safe excluded references, and fail-closed diagnostics. Excluded bodies are never included.

Required safety and output guardrails are selected before measurement. MDP blocks when they do not fit; it does not drop or truncate guardrails to satisfy a budget.

Jobs may additionally declare an opt-in `context_budget.optional_kind_quotas`
map, for example:

```yaml
context_budget:
  max_entries: 64
  max_bytes: 65536
  optional_kind_quotas:
    hooks: 6
    pains: 8
    ctas: 4
```

These are maximums for supporting entries only. The allocator reserves
guardrails, selected product-foundation entries and gaps, evidence-backed
entries, channel policies, and explicitly required output entries first. A
quota never removes those reservations, and a quota omission is not a
failure. The shared
`minimality.allocation` receipt reports the required count, required counts by
kind, selected/excluded optional counts, and each quota's reservation and
utilization. Quota exclusions use the body-free
`optional_kind_quota_exceeded` reason. Unknown or protected kinds, zero values,
and non-integer declarations fail validation. `channel-policies` and `gaps`
are always protected and cannot be quota kinds; evidence on any applicable
entry also makes that entry required. Omitting the map preserves the exact
legacy selection/classification and receipt path; the allocation receipt is
only added when quotas are enabled.

The manifest's `policy.max_cards_per_route` is also fail-closed. If the cap
would exclude an otherwise applicable card, the route is blocked with a
`route_card_cap_excluded_applicable` diagnostic. The `route_card_cap` receipt
names the cap, selected card IDs/kinds, excluded applicable card IDs/kinds, and
the deterministic `max_cards_per_route_reached` reason without exposing entry
bodies. Base guardrails remain selected; they are never silently evicted to
make room under the cap.

A generation-time `mdp route-budget` preflight evaluates every declared
canonical job that carries a `context_budget` against every relevant manifest
persona using the default (unfiltered) portfolio scope. It fails when any
route's selected entry count or canonical byte size exceeds the declared
budget, and reports selected/excluded counts, reason-code distributions, and
the largest contributing cards without leaking entry bodies. Legacy packs
without a declared `context_budget` remain `unassessed` and valid so their
runtime fail-closed behavior is preserved. `validate --strict` runs the same
preflight and surfaces overflow as blocking errors, so the builder's existing
strict gate catches persona-wide `applies_to` stamping before generation
handoff. The preflight never raises `max_entries` or `max_bytes`, truncates,
or ranks away required guardrails; the fix is narrower structured
applicability.

For bounded triage, `mdp --json --summary route-budget --dir PACK_DIR` emits
`mdp.route-budget-summary.v1`: validity, ready/blocked/unassessed counts,
entry and byte utilization percentages, bounded blocker/contributor metadata,
aggregate exclusion counts (including required-first optional quota
exclusions), and one safe next action. It contains no route array or entry body. Exact
projections are available with `--job JOB_ID`, `--persona PERSONA`, or both;
selectors are manifest-owned exact matches and the intersection is ANDed.
The full output remains the authority. New route records use canonical
`job_id`; the deprecated v0 `job` alias is retained and must be equal. When
overflow is not safely narrowable, the summary says `review_required_authority`;
operators must never truncate, drop guardrails, inflate budgets, or open a full
card to hide an overflow.

For a ready governed generation or review job, let MDP write the exact canonical `context.model_context` bytes and supply that file as the required `routed_context` prompt input:

```bash
mdp --json brief --dir PACK_DIR --prospect PROSPECT_JSON --job JOB_ID --context \
  --routed-context-out ROUTED_CONTEXT_JSON
```

The brief JSON reports the saved path, byte count, and SHA-256 under `data.routed_context_artifact`. The host includes that exact SHA-256 in `mdp.prompt-invocation.v1`:

`routed_context` is the exact saved `mdp.routed-context.v1` model-context
object. Its closed v1 envelope has no top-level `status` or `draft_status`
readiness field. The generative runtime revalidates the schema, canonical
bytes, selected job, serialized scope/persona, and recompilation from the
staged pack before model execution; a blocked or changed context remains
`no-draft:policy-blocked`.

```bash
mdp --json validate-prompt-output \
  --dir PACK_ROOT \
  --prompt-id PROMPT_ID \
  --file OUTPUT_JSON \
  --invocation-receipt PROMPT_RECEIPT_JSON \
  --routed-context ROUTED_CONTEXT_JSON
```

For a migrated governed prompt, MDP injects `context_sha256` from the exact staged routed-context bytes after the model returns its semantic payload. Legacy prompts may still echo it. In both paths MDP rejects changed context bytes, a mismatched digest, authority excluded from that context, and claim/CTA/angle/evidence identifiers selected from the wrong card kind. A gap or refusal still binds the same context.

MDP remains the local compiler and validator. The customer-selected host owns model execution. MDP does not browse, enrich, select a provider, price model calls, send outreach, or mutate external systems.
