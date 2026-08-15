# Decision Traces

`mdp trace` creates a bounded, read-only explanation from an existing MDP
decision artifact. It does not run policy, make a new decision, mutate a pack,
or replace the source artifact.

The stable vocabulary is:

- **Decision trace** — the complete `mdp.decision-trace.v1` projection.
- **Designed graph** — the relevant policy and gate relationships.
- **Observed path** — the facts recorded for one decision.
- **Decision graph** — a JSON or Mermaid visualization of those two views.

MDP's product category is **versioned decision context for agents**. A decision
graph is only the bounded visualization of designed policy and one observed
path. It is not a persistent graph store, workflow engine, model invocation,
or source-of-truth database.

| Phrase | Use |
| --- | --- |
| Versioned decision context for agents | Primary product category |
| Decision/context layer | Compatible architectural shorthand |
| Decision graph | JSON or Mermaid visualization of designed policy and one observed path |
| Graph database, agent runtime, memory layer, or orchestration framework | Not MDP |

## Inspect a saved result

Save the normal `--json` wrapper from `fit`, `route`, `brief`, or `emit-brief`,
then project it:

```bash
mdp --json trace --file <saved-result.json>
mdp trace --file <saved-result.json> --format mermaid
mdp trace --file <saved-result.json> --format mermaid \
  --out .mdp/traces/example.mmd
```

Supported raw decision artifacts carry an explicit supported contract, such as
`mdp.fit.v0`, `mdp.brief.v0`, `mdp.message-brief.v0`, or
`mdp.run-execution.v1`. A raw route result has no embedded contract, so use its
saved CLI wrapper. Ambiguous JSON is rejected instead of guessed.

Raw `mdp.prompt-output.v0` is deliberately different: it is model-produced,
untrusted data and never receives decision authority from `trace`, even when it
self-declares readiness. Trace it only through a successful
`mdp.prompt-output-validation.v1` receipt while supplying the exact pack and
output bytes:

```bash
mdp --json validate-prompt-output --strict \
  --dir <pack-root> \
  --prompt-id <prompt-id> \
  --file <prompt-output.json> > <validation-result.json>

mdp --json trace \
  --file <validation-result.json> \
  --dir <pack-root> \
  --prompt-output <prompt-output.json>
```

When validation used additional file inputs, pass every exact file again by
the receipt's logical name, for example
`--validation-input source_audit=<source-audit.json>`. The trace adapter
recomputes the portable pack digest, canonical prompt digest and job binding,
the output byte hash, every supplied validator-input byte hash, and the
receipt's canonical binding digest. It projects the validator outcome; it does
not rerun prompt-output validation or reinterpret self-declared readiness.

Stable unavailable diagnostics distinguish raw output
(`raw-prompt-output-untrusted`), unsuccessful validation
(`prompt-output-validation-invalid`), missing exact-byte bindings
(`prompt-output-validation-unbound`), pack/prompt/job/input disagreement
(`prompt-output-validation-mismatch`), changed output bytes
(`prompt-output-tampered`), and a changed receipt binding
(`prompt-output-validation-receipt-tampered`).

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

## Inspect a job-conformance journey

`mdp.job-conformance.v1` is the sole cross-phase authority for one exact
release, job, fixture, deterministic evaluation, behavioral evaluation, and
trial set. Assemble it only from members staged under one artifact root:

```bash
mdp --json conformance assemble \
  --artifact-root <staged-root> \
  --candidate candidate.json \
  --deterministic deterministic.json \
  --behavioral behavioral.json \
  --trial trials/trial-1.json

mdp --json conformance report \
  --artifact-root <staged-root> \
  --conformance job-conformance.json \
  --visibility public \
  --generated-at 2026-08-13T12:00:00Z

mdp trace \
  --artifact-root <staged-root> \
  --file job-conformance.json \
  --format mermaid
```

Assembly and projection re-open every path-backed member through the shared
containment boundary and recompute its digest. Missing links, cross-job,
cross-fixture, cross-release, and changed-byte substitutions fail closed. An
opaque artifact ID can record private external evidence, but cannot satisfy a
deterministic authority role.

The private and public reports, JSON trace, and Mermaid trace are projections
of the composite. None becomes a competing authority. Public reports and safe
traces never copy paths, people or company content, prompts, inputs, outputs,
provider or session identifiers, evaluator rationale, reviewer identity, or
private digests. Synthetic digests may be shown. A sanitized-public digest is
shown only when a contained approval receipt covers that exact digest;
changing the bytes invalidates the approval.

Do not trace the intermediate `mdp.behavioral-evaluation.v1` as though it were
a conformance report. First assemble and validate the exact
`mdp.job-conformance.v1` member set. A trace explains that composite; it never
promotes `unassessed` to sufficient, promotes sufficient to qualified, or
grants drafting/sending authority.

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

## Resolution boundary

The source artifact remains the complete authority. Fit traces expose exact
matched rule IDs and missing or disqualifying reasons. Route and brief source
artifacts retain their exact entry-level load order, exclusions, and reason
codes, while the v1 trace projection summarizes that routed selection as a
bounded count. Inspect the referenced source artifact when an audit requires
the complete selected-entry record; do not infer omitted entries from the
visualization.
