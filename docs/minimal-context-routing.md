# Minimal context routing

MDP compiles a job-specific context instead of handing a model the whole pack. A canonical job may declare `context_budget.max_entries` and `context_budget.max_bytes`. Jobs without that contract remain compatible, but their minimality status is `unassessed`.

`mdp route --entries` and `mdp brief --context` expose the same `minimality` receipt: status, the SHA-256 of the exact model-visible `mdp.routed-context.v1` object, authored and actual budgets, selected/excluded counts, safe excluded references, and fail-closed diagnostics. Excluded bodies are never included.

Required safety and output guardrails are selected before measurement. MDP blocks when they do not fit; it does not drop or truncate guardrails to satisfy a budget.

For a ready governed generation or review job, let MDP write the exact canonical `context.model_context` bytes and supply that file as the required `routed_context` prompt input:

```bash
mdp --json brief --dir PACK_DIR --prospect PROSPECT_JSON --job JOB_ID --context \
  --routed-context-out ROUTED_CONTEXT_JSON
```

The brief JSON reports the saved path, byte count, and SHA-256 under `data.routed_context_artifact`. The host includes that exact SHA-256 in `mdp.prompt-invocation.v1`:

```bash
mdp --json validate-prompt-output \\
  --dir PACK_ROOT \\
  --prompt-id PROMPT_ID \\
  --file OUTPUT_JSON \\
  --invocation-receipt PROMPT_RECEIPT_JSON \\
  --routed-context ROUTED_CONTEXT_JSON
```

The governed result must echo `context_sha256`. MDP rejects changed context bytes, a mismatched digest, authority excluded from that context, and claim/CTA/angle/evidence identifiers selected from the wrong card kind. A gap or refusal still binds the same context.

MDP remains the local compiler and validator. The customer-selected host owns model execution. MDP does not browse, enrich, select a provider, price model calls, send outreach, or mutate external systems.
