# Decision Traces

`mdp trace` creates a bounded, read-only explanation from an existing MDP
decision artifact. It does not run policy, make a new decision, mutate a pack,
or replace the source artifact.

The stable vocabulary is:

- **Decision trace** — the complete `mdp.decision-trace.v1` projection.
- **Designed graph** — the relevant policy and gate relationships.
- **Observed path** — the facts recorded for one decision.
- **Decision graph** — a JSON or Mermaid visualization of those two views.

## Inspect a saved result

Save the normal `--json` wrapper from `fit`, `route`, `brief`, or `emit-brief`,
then project it:

```bash
mdp --json trace --file <saved-result.json>
mdp trace --file <saved-result.json> --format mermaid
mdp trace --file <saved-result.json> --format mermaid \
  --out .mdp/traces/example.mmd
```

Supported raw artifacts carry an explicit supported contract, such as
`mdp.fit.v0`, `mdp.brief.v0`, `mdp.message-brief.v0`,
`mdp.prompt-output.v0`, or `mdp.run-execution.v1`. A raw route result has no
embedded contract, so use its saved CLI wrapper. Ambiguous JSON is rejected
instead of guessed.

## Inspect v1 run authority

```bash
mdp --json trace \
  --bundle <run-bundle.json> \
  --receipt <run-receipt.json> \
  --artifact-root <published-artifact-directory>
```

This form reuses `verify-run`. Invalid hashes, mismatched authority, or
no-draft leakage produce a blocked projection. Without `--artifact-root`, the
trace states that published artifact bytes were not recomputed. The run receipt
and verification remain authoritative in either case.

## Safety and limits

The projector allowlists labels and references. It does not copy prospect
bodies, prompt text, card bodies, generated output, raw customer prose, or
absolute paths. V1 accepts at most 1 MiB per source file, 256 combined nodes,
512 combined edges, 120 UTF-8 bytes per label, and 256 KiB of Mermaid output.
Truncation is explicit. Missing, malformed, unsupported, or oversized sources
return a sanitized `unavailable` projection.

`.mdp/traces` is only an optional generated-output convention when `--out` is
passed. It is excluded from portable pack identity and is never an authority
store.

Synthetic examples live in [`examples/decision-trace`](../examples/decision-trace/README.md).
