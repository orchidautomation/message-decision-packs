# Decision Trace Example

These public-safe fixtures demonstrate the two row-level outcomes without
customer data or raw transcripts.

```bash
mdp --json trace --file examples/decision-trace/fixtures/fit-ready-result.json
mdp trace --file examples/decision-trace/fixtures/fit-no-draft-result.json \
  --format mermaid
mdp --json schema decision-trace-v1
```

The ready fixture records a selected fit rule. The no-draft fixture stops at a
missing-field gate and exposes no output authority. Both outputs are projections;
the input artifacts remain the decision source.
