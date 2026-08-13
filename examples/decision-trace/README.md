# Decision Trace Example

These public-safe fixtures demonstrate the two row-level outcomes without
customer data or raw transcripts.

```bash
mdp --json trace --file examples/decision-trace/fixtures/fit-ready-result.json
mdp trace --file examples/decision-trace/fixtures/fit-no-draft-result.json \
  --format mermaid
mdp --json schema decision-trace-v1
```

The ready fixture records the exact selected fit rule, source-artifact hash,
and projection-only authority notice. The no-draft fixture stops at a
missing-field gate, records the exact missing field, and exposes no output
authority. Both outputs are projections; the input artifacts remain the
decision source.

This example demonstrates why MDP is **versioned decision context for agents**:
pack-owned policy governs the result, the observed path is inspectable, and a
blocked input stays blocked. The Mermaid view is called a decision graph, but
MDP does not execute that graph, persist a universal graph, call a model, or
prove that the supplied source is true.
