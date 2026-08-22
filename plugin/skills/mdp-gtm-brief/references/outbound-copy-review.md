# Outbound Copy Review

Read this only for `outbound-copy-review` and only when the user supplies copy.

## Workflow

1. Establish the prospect fit and bounded context used by the draft.
   Run `mdp requirements --dir PACK_ROOT --job outbound-copy-review` first. If
   the job resolves any Decision Input Contract, require its exact validated
   normalized input and lineage artifacts; detached prospect input is terminally
   blocked. Use a detached prospect only when the resolved contract list is
   empty.
   Require minimality `ready`, preserve the exact context digest, and use only
   `context.model_context`. A blocked/unassessed budget or whole-card fallback
   stops governed review.
   When `optional_kind_quotas` is declared, require the shared
   `minimality.allocation` receipt to report `required-first`, reservations,
   and deterministic optional exclusions. Never accept quota displacement of
   channel policies, gaps, guardrails, foundation references, or any
   evidence-backed entry; quota diagnostics must remain body-free. With no
   quota map, preserve the legacy classification and receipt behavior.
   For a prompt declaring `mdp.governed-host-envelope.v1`, review the
   semantic payload only; MDP injects and validates deterministic provenance
   after generation, and model-supplied envelope fields must fail closed.
2. Run the deterministic check with the relevant route selectors:

```bash
mdp --json check-claims --dir PACK_ROOT --file COPY_FILE --subject SUBJECT --persona PERSONA --job JOB
```

Add every required `--scope DIMENSION=VALUE`. Use `--strict` when advisory constraint warnings should block acceptance.

3. Review routed-card fidelity, evidence, safe personalization, claims, avoid rules, output constraints, CTA fit, channel fit, and unresolved gaps.
4. Return a compact scorecard: pass, revise, or blocked; CLI issues; unsupported statements; boundary violations; and the smallest safe correction.

## Boundary

Do not enrich missing context, turn the task into unsolicited copywriting, or imply send approval. Rewrite only when explicitly requested, and re-run the same checks on the revision.
