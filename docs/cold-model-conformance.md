# Cold-model conformance

Cold-model conformance answers one bounded question: can a capable model with
no prior company knowledge complete one declared job using only an exact pack
release, the job's declared runtime inputs, and a recorded host envelope?

It does not certify an entire company or pack. Results are per release, per
job, per evaluator inventory, and per host/model envelope.

## Status vocabulary

Keep the proof planes separate:

- **`sufficient-for-job`** — the deterministic D1-D12 contract checks passed.
  This is permission to begin separately governed behavioral trials, not proof
  that a model performed the job well.
- **`unassessed`** — required deterministic or behavioral evidence is absent,
  incomplete, stale, or not applicable to a claimed result. It is not a pass.
- **`qualified-for-job-under-envelope`** — deterministic sufficiency passed and
  the required recorded trials met the hard-boundary and useful-completion
  thresholds under the named envelope.
- **`conformance-failure`** — one or more required deterministic or behavioral
  assertions failed. No authoritative draft or output may escape.

Foundation `ready`, prompt-output validity, run verification, and a behavioral
evaluation are inputs to this decision. None alone means sufficient or
qualified.

## Exact host workflow

Start by discovering the installed contract rather than copying a command list:

```bash
mdp --json capabilities
mdp --json schema conformance-candidate-v1
mdp conformance --help
```

Then follow this order:

1. **Discover.** Resolve the exact pack release and canonical job through
   `skills --job` and `requirements --job`. Stage a closed
   `mdp.conformance-candidate.v1` plus every referenced authority under one
   artifact root.
2. **Compile deterministic sufficiency.** Run:

   ```bash
   mdp --json conformance compile \
     --candidate <candidate.json> \
     --artifact-root <staged-root> \
     --out <staged-root>/deterministic.json
   ```

   Preserve the returned `mdp.deterministic-conformance.v1` bytes. Stop unless
   the deterministic result is sufficient. A failed or unassessed gate is
   no-draft and cannot be repaired with an external model call.
3. **Run the external host.** The customer-selected host—not MDP—chooses the
   provider/model, constructs the exact declared-input invocation, runs the
   model, and records `mdp.model-invocation-evidence.v1`, trial, and evaluator
   artifacts. The host must not expose evaluator answers or protected
   challenge material to the model.
4. **Validate recorded evidence.** Import the recorded files and run:

   ```bash
   mdp --json conformance validate \
     --candidate <candidate.json> \
     --evaluator-inventory <evaluator-inventory.json> \
     --lifecycle-policy <private-record-policy.json> \
     --deterministic <deterministic.json> \
     --invocation <invocation.json> \
     --trial <trial.json> \
     --verifier-receipt <verifier-receipt.json> \
     --evaluator-result <evaluator-result.json> \
     --out <behavioral.json>
   ```

   Repeat `--invocation`, `--trial`, `--verifier-receipt`,
   `--evaluator-result`, and `--publication-approval` as required by the
   predeclared inventory. This command
   validates supplied evidence only; it performs no model or network call.
   Its `mdp.behavioral-evaluation.v1` result is an intermediate authority and
   is not itself a private/public conformance report.
5. **Assemble the sole cross-phase authority.** Place the candidate,
   deterministic result, behavioral evaluation, and trials under one staged
   root, then run:

   ```bash
   mdp --json conformance assemble \
     --artifact-root <staged-root> \
     --candidate <candidate.json> \
     --deterministic <deterministic.json> \
     --behavioral <behavioral.json> \
     --trial <trial.json> \
     --out <staged-root>/job-conformance.json
   ```

   Save the returned `mdp.job-conformance.v1`. It is the sole hash-complete
   authority joining the release, job, candidate, deterministic result,
   behavioral result, and exact trial set.
6. **Project a report.** Only after assembly, run `conformance report` with
   `--visibility private` or `--visibility public` and a recorded RFC 3339
   `--generated-at`, and write the projection with `--out`. Reports are
   validated projections of the composite, not replacement authorities.
7. **Trace if needed.** Run `mdp --json trace --artifact-root <staged-root>
   --file <job-conformance.json>`. JSON and Mermaid traces are sanitized,
   explanatory projections of the same composite.

## Sampling and terminal behavior

The evaluator inventory predeclares the trial slots and their class. Hard
boundaries require 3/3 passing observations; useful completion requires 2/3.
Each slot freezes the requested and resolved model plus the exact candidate
prompt, declared inputs, and model-visible context digest. Verifier receipts
and publication approvals count only when they match trusted authority
descriptors predeclared by that inventory. Evidence timestamps use canonical
seconds-only UTC (`YYYY-MM-DDTHH:MM:SSZ`).
The evaluator may not select the best three after seeing results. A missing
required slot remains `unassessed`. A negative fixture counts only when the
exact expected bounded non-success state occurs and no usable output escapes.

Conformance never grants drafting, sending, scheduling, CRM mutation, or
publication authority. External model/provider calls remain customer-owned
and require separate authorization; those actions are not performed by MDP.

## Privacy and containment

Every path-backed member must remain under the declared staged root and pass
file-type, link, size, depth, and digest checks. Do not put raw customer data,
contact details, provider payloads, prompts, outputs, evaluator rationale,
reviewer identity, local paths, or unrestricted source prose in a public
report or trace.

The private composite may retain exact access-controlled evidence. Public
projections expose only sanitized metadata and synthetic digests or an exact
digest covered by a matching sanitized-public approval. They never expose
private or opaque evidence IDs. Hash agreement proves byte identity and linkage, not that a source
claim is true or that isolation happened unless the recorded evidence proves
the relevant control.

## Product boundary

MDP owns contracts, deterministic compilation, validation, assembly, reports,
and trace projection. It does not call a model, choose a provider, operate an
agent runtime, calculate provider pricing, browse, enrich, draft outreach,
send, schedule, or update a CRM. CLI subprocess access is the normative
integration surface for this conformance increment.

The committed offline suite validates recorded synthetic evidence and mutation
cases without credentials or network access. It is repeatability proof, not a
fresh provider trial or a claim about a maintained provider integration.
