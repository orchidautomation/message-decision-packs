# Managed Workflow Bundle Handoff

Use this contract for every normal MDP workflow that prepares, runs, verifies,
reviews, or resumes an authoritative execution. The caller supplies only the
exact pack root, job/step, model when required, and approved input/source
files. The skill privately manages all intermediate artifacts.

## Input and authority boundary

- Resolve the exact pack, job, stable step, model, and declared inputs before
  doing work. Never infer a pack or select an ambient latest run.
- Create one fresh, invocation-owned scratch root with current-user-only
  permissions (for example `0700`). Keep requirements, source ledgers,
  normalization output, routed context, prompt/invocation receipts, compile
  manifests, and request files inside it. Pass paths between processes; never
  copy source bodies, prompt bodies, or private intermediate paths through
  chat or a public report.
- Select a new durable output directory outside scratch and outside the pack.
  It must not already exist and must not be a caller-owned broad temp root.
- Use the direct CLI as the canonical transport. MCP is optional and must
  receive the same existing path-only authority inputs; it never adds
  authority or becomes a dependency.

## Managed sequence

Run the existing authority path in this order:

```bash
mdp --json requirements --dir PACK_ROOT --job JOB_ID
mdp --json prepare-run --dir PACK_ROOT --job JOB_ID \
  --operation model:JOB_ID/STEP --model MODEL \
  --input LOGICAL_NAME=INPUT_PATH --out SCRATCH/run-request.json \
  --manifest-out SCRATCH/compile-manifest.json
mdp --json run --request SCRATCH/run-request.json --out-dir NEW_RUN_DIRECTORY
mdp --json verify-run --bundle NEW_RUN_DIRECTORY/run-bundle.json \
  --receipt NEW_RUN_DIRECTORY/run-receipt.json \
  --artifact-root NEW_RUN_DIRECTORY
```

Use the exact compiled model step and input declarations. One run executes one
declared step and emits one receipt. Preserve the CLI decision, terminal,
gaps, and receipt identity exactly; a wrapper cannot upgrade a blocked,
no-draft, unavailable, invalid, or unknown result.

## Explicit resume/review

Resume or review accepts one explicit run directory from the caller. Refuse a
missing, ambiguous, in-pack, overwritten, or ambient/latest pointer. Locate
only the named `run-bundle.json` and `run-receipt.json` inside that directory,
run `verify-run` with that directory as `--artifact-root`, and stop on any
verification failure or wrong artifact root. Never scan the filesystem for the
newest run and never consume an unverified result.

## Cleanup and retention

Put one cleanup boundary around the entire invocation. Remove only the exact
scratch root owned by this invocation on success, canonical no-draft/blocked
result, handled failure, timeout, or cancellation. Concurrent invocations use
distinct roots and must not remove one another's files. The durable run
directory, verified bundle/receipt, and explicitly bounded diagnostics selected
for handoff are the only permitted survivors. Never recursively remove a pack,
durable output parent, repository, or broad temporary directory.

## User-visible handoff

Return one bounded block and no intermediate bodies or private scratch paths:

```text
MDP workflow handoff
run_directory: NEW_RUN_DIRECTORY
verification: verified | failed | blocked
decision: <canonical CLI decision>
terminal: <canonical CLI terminal>
gaps: <canonical gap refs or none>
retention: scratch discarded; durable run retained | bounded diagnostics retained
bundle: NEW_RUN_DIRECTORY/run-bundle.json
receipt: NEW_RUN_DIRECTORY/run-receipt.json
next_action: <one permitted next action or stop>
```

If no durable run exists, return `run_directory: none`, the canonical decision
and gaps, `retention: scratch discarded`, and the permitted next action. Do not
turn a no-draft or blocked result into a polished output because the user asks.

## Advanced explicit-artifact parity

Advanced workflows may supply exact lineage, source-binding, normalized,
routed-context, prompt, or invocation artifacts. Keep them private, validate
their hashes and contract versions with the same gates, and produce the same
canonical handoff fields. Explicit artifacts are never a bypass around
`requirements`, `prepare-run`, `run`, or `verify-run`; v0 proposal runner/MCP
paths remain labeled compatibility only. The same authority, privacy, cleanup,
and resume rules apply.
