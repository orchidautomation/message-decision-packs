# Cold-model conformance protocol

Load this reference only when the user asks about job conformance or qualification.

## Cold-model Conformance

When asked whether an exact released job is self-standing or qualified, run
`capabilities`, inspect `schema conformance-candidate-v1`, then compile the
closed candidate:

```bash
mdp --json conformance compile --candidate CANDIDATE_JSON --artifact-root STAGED_ROOT --out STAGED_ROOT/deterministic.json
```

Stop no-draft unless deterministic status is `sufficient-for-job`. For the
behavioral trial, the customer-selected host owns provider/model selection and
the call; MDP does neither. The conformance commands do not invoke that trial
and MDP does not calculate pricing. After the host returns recorded
invocation, trial, verifier-receipt, and evaluator evidence, chain the exact
compiled authority with `conformance validate --artifact-root STAGED_ROOT
--candidate CANDIDATE_JSON --deterministic deterministic.json --out
behavioral.json`, plus the required evaluator inventory, lifecycle policy, and
all predeclared repeated evidence flags. Validation does not invoke a model;
use the complete runnable form in references/cli-operator.md
instead of guessing required arguments.
Accept verifier receipts and publication approvals only when the CLI confirms
they match the evaluator inventory's predeclared trusted authority descriptors;
do not treat self-declared verifier or reviewer identity as proof.

Treat `mdp.behavioral-evaluation.v1` as intermediate only. Use `conformance
assemble --out STAGED_ROOT/job-conformance.json` to create the sole cross-phase `mdp.job-conformance.v1` authority, then use `conformance report --out ...` or
`trace` only as projections. Keep
`sufficient-for-job`, `not-sufficient-for-job`,
`qualified-for-job-under-envelope`, `not-qualified-for-job-under-envelope`, and
`unassessed` distinct. No result grants drafting, sending, scheduling,
CRM mutation, or publication authority. Public output must omit paths, raw
content, identities, provider/session data, evaluator rationale, reviewer
identity, and private digests.

Read references/cli-operator.md for command selection or artifact-write rules. Read references/mental-model.md when explaining product boundaries, pack primitives, or responsibility splits.
After a validated fix yields a reusable engineering lesson, read
references/compound-learning.md before
capturing it.

