# Outbound Copy Brief

Read this only for `outbound-copy-brief`.

## Workflow

1. Run `mdp requirements --dir PACK_ROOT --job outbound-copy-brief` and inspect
   the resolved Decision Input Contracts. If any are present, require the exact
   validated normalized input and lineage artifacts compiled for that job;
   never extract or substitute a detached prospect. If none are present, the
   selected job may use the supplied legacy prospect JSON. Stop on insufficient
   or disqualified fit.
2. Build bounded pre-draft context. For a truly ungoverned job:

```bash
mdp --json brief --context --dir PACK_ROOT --prospect PROSPECT_JSON --channel CHANNEL \
  --job outbound-copy-brief --routed-context-out ROUTED_CONTEXT_JSON
```

For a governed v2 job, replace `--prospect PROSPECT_JSON` with the exact
`--normalized-input`, `--prompt`, `--source-binding`,
`--source-attempt-request`, and `--collected-attempt-results` artifacts. For
another supported normalized contract version, follow the exact argument set
compiled by `mdp requirements`. Treat
`governed_job_requires_normalized_input` as terminal.

Require `data.context.minimality.status: ready` and
`data.routed_context_artifact.status: saved`; do not load
excluded bodies or open `full_card_required` paths. The downstream host must
hash those exact bytes in `mdp.prompt-invocation.v1`, and validation must pass
both `--invocation-receipt` and `--routed-context`.

For a prompt declaring `mdp.governed-host-envelope.v1`, provide only the
semantic authority, artifact, gap, and rejected-claim fields. The host owns the
deterministic prompt, job, context, receipt, and input-inventory envelope; do
not echo or invent those values.

When the selected job declares `context_budget.optional_kind_quotas`, also
inspect `data.context.minimality.allocation`. It must show the
`required-first` strategy, required reservations, and deterministic optional
selection/exclusion counts. Channel policies, gaps, guardrails, foundation
references, and every evidence-backed entry remain reserved; quota exclusions
must contain no entry bodies. Jobs without quotas retain the legacy selection
and receipt behavior.

3. Return a writing contract containing the audience/persona, fit rationale, safe personalization, approved claims/proof, message angles, CTA policy, avoid rules, output constraints, and known gaps.
4. If no prospect object is required for the pack-owned route, use `mdp emit-brief` with the exact persona, job, and required scope.

## Boundary

The output is a brief, not outbound copy. Do not write subject lines, opening lines, emails, DMs, sequences, or send instructions. A downstream writer must remain within the brief and run claim checks on any draft.

If this brief is part of a cold-model trial, deterministic
`sufficient-for-job` must exist before the external host call. Preserve and
validate host invocation and evaluator evidence, then assemble the sole
`mdp.job-conformance.v1` authority before making a qualification claim. An
intermediate behavioral evaluation is not a report, and neither conformance
nor a trace grants drafting or sending authority.
