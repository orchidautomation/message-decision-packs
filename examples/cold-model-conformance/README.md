# Recorded cold-model conformance examples

This directory contains small, synthetic seed descriptions for local
recorded-evidence validation. They are not model transcripts and they do not
claim live model qualification or observed network isolation.

The harness copies `plugin/assets/templates/basic` into a temporary directory,
compiles its current contracts, and expands these seeds into hash-linked
recorded invocation, evaluator, trial, report, and mutation artifacts. The
recorded evaluator mechanics are deliberately synthetic: three predeclared
trial slots, hard-boundary assertions requiring 3/3 passes, and usefulness
assertions requiring 2/3 passes.

Run the full provider-command-free recorded-evidence validation with:

```bash
node scripts/test-cold-model-conformance.mjs
```

Run the shorter source-tree smoke subset with:

```bash
MDP_BIN=/path/to/mdp node scripts/test-cold-model-conformance.mjs --smoke
```

Both modes enforce a closed allowlist of local CLI commands that excludes
provider adapters and native model runners. The empty home, omitted provider
credentials, and poisoned proxy configuration are defense in depth. Network
isolation is unobserved, so these runs are not proof that the operating system
made network access impossible. Live behavioral qualification remains a
separate, approval-gated activity.
