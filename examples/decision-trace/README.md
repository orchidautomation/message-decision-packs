# Decision Trace Example

These public-safe fixtures demonstrate the two row-level outcomes without
customer data or raw transcripts.

```bash
mdp --json trace --file examples/decision-trace/fixtures/fit-ready-result.json
mdp trace --file examples/decision-trace/fixtures/fit-no-draft-result.json \
  --format mermaid
mdp --json schema decision-trace-v1
mdp --json schema prompt-output-validation-v1
```

The ready fixture drives a trace projection that records the exact selected
fit rule and computes the source-artifact hash and projection-only authority
notice. The no-draft fixture stops at a
missing-field gate, records the exact missing field, and exposes no output
authority. Both outputs are projections; the input artifacts remain the
decision source.

The synthetic `prompt-output-ready.json` fixture demonstrates the separate
prompt-output trust boundary. Validate it against the basic template, save the
JSON wrapper, and trace the receipt together with the exact pack and output:

```bash
mdp --json validate-prompt-output --strict \
  --dir plugin/assets/templates/basic \
  --prompt-id normalize-prospect-row \
  --file examples/decision-trace/fixtures/prompt-output-ready.json \
  > /tmp/mdp-prompt-output-validation.json

mdp --json trace \
  --file /tmp/mdp-prompt-output-validation.json \
  --dir plugin/assets/templates/basic \
  --prompt-output examples/decision-trace/fixtures/prompt-output-ready.json
```

Tracing `prompt-output-ready.json` directly is unavailable because raw model
output cannot self-certify validation or decision readiness.

This example demonstrates why MDP is **versioned decision context for agents**:
pack-owned policy governs the result, the observed path is inspectable, and a
blocked input stays blocked. The Mermaid view is called a decision graph, but
MDP does not execute that graph, persist a universal graph, call a model, or
prove that the supplied source is true.
