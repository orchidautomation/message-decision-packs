# Proposal Evidence Path

For the normal v1 path, first follow [Managed Workflow Bundle Handoff](../../mdp/references/workflow-bundle-handoff.md).
Keep source intake, requirements, normalization, routed context, and receipts
inside one restricted invocation scratch root. Return one verified durable run
directory or canonical blocked/advisory gaps. The v0 runner/MCP choreography
below is an advanced compatibility path; it must not make the user shuttle
intermediate bodies through chat.

Read this before normalizing proposal material or answering whether a review is
audit-grade. The evidence path is a decision gate, not an optional confidence
upgrade.

## Decide Before Normalizing

1. **Are there explicit local source files?**
   - No: audit-grade is blocked. Ambient chat, pasted text, a source ID, or a
     source-audit ledger alone is not an approved source.
   - If the operator selects pasted text, export only that selection as a
     bounded candidate. Show its preview and hash, then wait for human approval.
2. **Is each exact source hash human-approved for this pack and
   `proposal-review` purpose?**
   - No: create or report candidate intake state and stop before a real native
     run. An agent may not approve it.
3. **Does the operator require audit-grade?**
   - No: an ambient review of supplied material may continue only as
     `assurance: advisory`. Do not describe ambient normalization as isolated,
     receipt-backed, or audit-grade.
   - Yes: continue only through the local runner or MCP evidence path.
4. **Can this host call the local runner/MCP and a schema-accepted
   native/headless boundary?**
   - No: return `assurance: blocked`, name the missing runtime/evidence, and
     provide the smallest command handoff below. Do not silently fall back to
     same-chat normalization.
5. **Did the current invocation produce an audit-grade receipt?**
   - Yes only when the result reports `decision: "audit-grade"`,
     `audit_grade_eligible: true`, a verified runner assurance, matching
     artifact hashes, and a valid runner audit required by the receipt.
   - Any dry-run, mock, fixture, advisory, blocked, malformed, failed, or
     timed-out result is not audit-grade.

## V0 Compatibility: Source Checkout Commands

First create candidate intake and inspect the declared-input-only request:

```bash
node scripts/mdp-proposal-runner.mjs run \
  --pack PACK_ROOT \
  --workdir WORKDIR \
  --source SOURCE_FILE \
  --source-id SOURCE_ID \
  --source-kind private-scratch-opportunity \
  --dry-run
```

After a human approves the exact candidate hashes in an
`mdp.source-intake.v0` ledger, a real invocation is:

```bash
node scripts/mdp-proposal-runner.mjs run \
  --pack PACK_ROOT \
  --workdir NEW_WORKDIR \
  --source SOURCE_FILE \
  --source-intake APPROVED_SOURCE_INTAKE_JSON \
  --source-audit SOURCE_AUDIT_JSON \
  --model MODEL_ID \
  --require-audit-grade
```

`--source-audit` is optional when the runner can create the ledger from the
supplied file, but it never replaces `--source` or human source approval.

## V0 Compatibility: Installed Plugin Commands

Use the same arguments against the installed bundle:

```bash
node "${PLUGIN_ROOT}/scripts/mdp-proposal-runner.mjs" run \
  --pack PACK_ROOT \
  --workdir WORKDIR \
  --source SOURCE_FILE \
  --source-id SOURCE_ID \
  --source-kind private-scratch-opportunity \
  --dry-run

node "${PLUGIN_ROOT}/scripts/mdp-proposal-runner.mjs" run \
  --pack PACK_ROOT \
  --workdir NEW_WORKDIR \
  --source SOURCE_FILE \
  --source-intake APPROVED_SOURCE_INTAKE_JSON \
  --source-audit SOURCE_AUDIT_JSON \
  --model MODEL_ID \
  --require-audit-grade
```

Do not substitute a host-specific or guessed plugin root. Use the installed
host's actual `PLUGIN_ROOT`.

## MCP Path

Launch the canonical profile-neutral local stdio adapter from source or the
installed bundle:

```bash
node scripts/mdp-run-mcp-server.mjs
node "${PLUGIN_ROOT}/scripts/mdp-run-mcp-server.mjs"
```

Use `mdp_run_tools` → `mdp_prepare_run` → `mdp_run` → `mdp_verify_run`. The
stages produce the boundary inventory, `mdp.run-request.v1`, run
bundle/receipt, and `mdp.run-verification.v1`. Pass paths only; MCP adds no
authority or isolation assurance. `mdp-proposal-mcp-server.mjs` and its two
tools remain compatibility-only for existing v0 consumers and must not be
presented as a second default path.

For v1, pass the exact approved source/input files as `logical_name=path`
mappings to `mdp_prepare_run`, require its `out` path under an approved work
root, then pass that same request path to `mdp_run`. Verify the emitted
`run-bundle.json` and `run-receipt.json` from the approved output root. Raw
proposal text and ambient chat are never MCP arguments. Treat any MCP tool
error or invalid CLI verification as blocked.

## Read Results, Not Vibes

For the canonical MCP path, consume the CLI-owned `mdp.run-execution.v1`
authority block and terminal state, then require `mdp.run-verification.v1`
`valid: true`. Return those authorities unchanged.

### V0 compatibility result fields

Only an existing `mdp_proposal_run` consumer should use the v0
`source_paths`/`source_audit_path` arguments and consume these strict top-level
fields:

- `mode`
- `decision`
- `audit_grade_eligible`
- `runner_assurance`
- `timed_out`
- `termination_signal`
- `runner_exit_status`

For a v0 direct runner call, read `proposal-runner-result.json` and its referenced
`mdp.run-receipt.v0`. MCP transport, tool availability, an installed command,
schema-valid JSON, or a runner identifier never upgrades the result.
Report integration support separately from this invocation using only the
canonical support matrix states.

When asked “is this audit-grade?”, answer in this order:

1. `assurance`: `audit-grade`, `advisory`, or `blocked`;
2. the current receipt decision and runner assurance, or state that no current
   receipt exists;
3. source paths/intake/audit artifacts actually checked;
4. missing or failed gates;
5. the smallest next command or human approval needed.

Do not proceed into a confident proposal packet when audit-grade was requested
and the assurance is advisory or blocked.
