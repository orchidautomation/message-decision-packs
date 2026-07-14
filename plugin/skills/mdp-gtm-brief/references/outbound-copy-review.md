# Outbound Copy Review

Read this only for `outbound-copy-review` and only when the user supplies copy.

## Workflow

1. Establish the prospect fit and bounded context used by the draft.
2. Run the deterministic check with the relevant route selectors:

```bash
mdp --json check-claims --dir PACK_ROOT --file COPY_FILE --subject SUBJECT --persona PERSONA --job JOB
```

Add every required `--scope DIMENSION=VALUE`. Use `--strict` when advisory constraint warnings should block acceptance.

3. Review routed-card fidelity, evidence, safe personalization, claims, avoid rules, output constraints, CTA fit, channel fit, and unresolved gaps.
4. Return a compact scorecard: pass, revise, or blocked; CLI issues; unsupported statements; boundary violations; and the smallest safe correction.

## Boundary

Do not enrich missing context, turn the task into unsolicited copywriting, or imply send approval. Rewrite only when explicitly requested, and re-run the same checks on the revision.
