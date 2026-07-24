# Proposal Demo Go/No-Go Gate

Use this gate before recording or sharing the public proposal-flow demo.
It separates a safe walkthrough from proof about a real model invocation or
real client-source handling.

The person publishing or presenting the video owns the final go/no-go decision.
A green technical check does not approve customer material or expand the claims
the demo may make.

## Decision table

| State | Runner and evidence | Allowed presentation | Required action |
| --- | --- | --- | --- |
| **Green — safe synthetic demo** | The checked-in synthetic fixtures run in `mock` mode. `run-receipt` returns `blocked` because the audit contains mock/fixture evidence. No customer or private material is present. | Show the local artifact chain and say that MDP blocks mock evidence from audit-grade. Describe local-first review support, deterministic validation, gap surfacing, and human review. | Use the safe narration below. Keep the blocked receipt visible. |
| **Yellow — real invocation, synthetic sources** | A native/headless provider was actually invoked with synthetic sources. The exact invocation has machine-observed runner evidence and a matching receipt. The integration may still be `recipe-only` in the support matrix. | Demonstrate a real invocation and report the receipt's decision and assurance exactly. An `audit-grade` receipt describes that invocation only; it does not make the integration verified or prove client-source isolation. | Confirm the installed/source-tree state, support-matrix state, source classification, hashes, and receipt before recording. Use qualified narration. |
| **Red — no-go** | Any source is private/customer material without explicit source-intake approval; the receipt is absent, advisory, blocked unexpectedly, or mismatched; runner evidence is hand-authored; sensitive paths are visible; or the narration overclaims verification, isolation, compliance, approval, or automation. | Do not record, publish, or present the run as proof. | Stop, remove or re-approve the inputs, regenerate evidence through the runner, fix the failing gate, redact the screen, and repeat this checklist. |

MCP transport is not a fourth state. A local MCP wrapper may invoke the same
runner surface, but tool availability and schema-valid JSON do not prove that a
fresh, declared-input-only model call happened.

## Preflight checklist

### Build and execution identity

- [ ] Record whether the command uses the source-tree CLI or an installed
  release asset.
- [ ] If an installed asset is claimed, record the installed `mdp --version`
  and release tag. Do not present an unreleased source checkout as shipped.
- [ ] Record the runner mode as `mock`, `dry-run`, `native`, or `headless`.
- [ ] Check the
  [canonical runner support matrix](headless-normalization-runners.md#canonical-runner-support-matrix)
  before using `verified`, `maintained`, or `supported`.
- [ ] Treat dry-run as a request preview and mock mode as a fixture test, never
  as model-invocation evidence.

### Evidence and source state

- [ ] Confirm every visible input is synthetic or explicitly sanitized.
- [ ] Confirm a `source-audit` exists and every cited raw ref resolves.
- [ ] For any non-synthetic input, require the separate source-intake approval
  record for the exact hash, pack source ID, privacy class, and review purpose.
  A file path and a source-audit are not approval.
- [ ] Inspect `proposal-runner-result.json`, `runner-audit.json`, and
  `run-receipt.json` from the same run.
- [ ] Read the receipt decision aloud exactly: `audit-grade`, `advisory`, or
  `blocked`.
- [ ] For the checked-in mock demo, require `decision: blocked`,
  `mock_response: true`, and the `runner_audit_mock_response` issue. That block
  is the expected safe result.
- [ ] Never reuse fixture, hand-authored, or prior-run audit JSON as evidence
  for a real invocation.

### Public claims

- [ ] Describe MDP as local/offline decision context and review support, not as
  a proposal writer, submission system, CRM, sequencer, or execution platform.
- [ ] Say that deterministic checks validate the supplied artifacts; do not say
  they prove the semantic truth of proposal claims.
- [ ] Do not claim CMMC/NIST compliance, approved CUI handling, guaranteed
  security, legal/procurement bypass, proposal approval, or replacement of
  compliance/proposal-management review.
- [ ] Do not imply that an `audit-grade` receipt verifies an entire integration.
  Receipt assurance is per invocation; integration support comes from the
  support matrix.
- [ ] Keep unsupported proof, certification, pricing, deadline, and past
  performance items visible as gaps or questions.

### Screen-recording hygiene

- [ ] Use a fresh, dedicated work directory containing synthetic assets only.
- [ ] Hide terminal history, machine/user identity, absolute home/repository
  paths, environment values, provider credentials, unrelated tabs, and desktop
  notifications.
- [ ] Do not open raw customer files, private transcripts, non-public RFPs,
  customer names, or provider request/response payloads containing private
  values.
- [ ] Review generated JSON and readable output before recording; stop if it
  contains unexpected source text or machine-local metadata.
- [ ] Record only the bounded artifact directory needed for the walkthrough.

## Safe narration for the current demo

> This is a synthetic, offline mock walkthrough of MDP's local proposal-review
> evidence flow. It shows how source, validation, runner-audit, and receipt
> artifacts are staged and how the CLI blocks mock evidence from audit-grade
> status. It does not demonstrate a verified runner integration, real
> client-source isolation, compliance, proposal approval, or automated proposal
> writing or submission.

## Before saying “audit-grade”

All of the following must be true for the exact invocation shown:

1. The run is a real native/headless invocation, not dry-run, mock, fixture, or
   hand-authored evidence.
2. The runner produced the required audit artifact and observed the relevant
   invocation facts rather than copying operator assertions.
3. Prompt output, validation, source audit, runner audit, source-intake approval
   when applicable, and pack manifest are bound to the same hashes.
4. `mdp run-receipt --require-runner-audit` returns `decision: audit-grade` with
   a schema-accepted verified assurance value.
5. The narration limits “audit-grade” to that invocation and states the
   integration's separate support-matrix status.
6. A human has reviewed the sources, output, redactions, receipt, and narration
   and has approved the video for its intended audience.

The local runner now emits and receipt-binds source-intake ledgers and enforces
owned work-directory reuse. Public recordings must still use synthetic sources
until the remaining machine-observed runner and MCP allowlist/error-redaction
gates are implemented, validated, and explicitly cleared by a human reviewer.

## Current demo verification

From the repository root:

```bash
bash examples/proposal-flow-video/scripts/run-demo.sh
```

The default run is acceptable only when it completes successfully and its
summary reports:

```text
runner mode:         mock / audit eligible: False
receipt decision:    blocked
mock response:       True
presentation gate:   GREEN — safe synthetic demo; use the required mock narration
```

The exact additional summary text may change, but the mock run must remain
blocked from audit-grade.
