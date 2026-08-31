---
name: mdp
description: Use for MDP CLI/operator questions, contract inspection, validation-command guidance, or an explicitly mixed workflow spanning multiple MDP skills. Do not use merely because a specialized builder, pack-review, GTM, or proposal request names MDP.
metadata:
  compatibility: Requires the mdp CLI on PATH. Native plugin helper scripts additionally require Node.js 18+; portable skill installs use the CLI-only path and do not assume PLUGIN_ROOT or MCP support.
---

# MDP

Coordinate explicit operator work for **versioned decision context for agents**. A decision graph is only a bounded projection, never a graph database, agent runtime, memory layer, or orchestration framework.
The Rust CLI is the source of truth. This coordinator owns CLI explanation,
contract inspection, and mixed-job routing; it does not own a specialized job.

## Communicate The Work

Follow the shared [Orient, Plan, Progress, Translate, Close contract](references/communication-contract.md).
Open by naming the operator-help, validation, or mixed-routing job; the exact
pack and approved evidence boundary; the decision or durable artifact the user will receive; and what this skill will not do. Keep updates to meaningful CLI
gates, blockers, and decisions.

## Route Before Loading Detail

Present MDP as two product journeys, not as five skills the user must learn:

- **Author and maintain** creates, explicitly edits, validates, and reviews
  durable pack authority. Route mutations to `$mdp-pack-builder`; route
  read-only pack QA to `$mdp-pack-review`.
- **Use and decide** selects an existing pack, exact job, and supplied input,
  then returns a bounded decision or workflow bundle without changing pack
  authority. Route GTM work to `$mdp-gtm-brief` and proposal work to
  `$mdp-proposal-review`.

Ask which journey is intended only when the request is genuinely ambiguous.
Absent explicit edit intent, default to Use and decide or read-only review.

- Creating or editing `.mdp/`: `$mdp-pack-builder`.
- Read-only pack audit, hardening, or installed QA: `$mdp-pack-review`.
- Supplied-prospect fit, bounded GTM context, or supplied-copy review:
  `$mdp-gtm-brief`.
- Supplied proposal-material review: `$mdp-proposal-review`.
- Stay here only for operator help, validation-command guidance, or a request
  that explicitly spans two or more of those owners.

Naming MDP does not override these ownership rules. Hand off one bounded phase
at a time; never let the coordinator silently perform the specialized work.

For a mixed request, declare the lane order before starting. Complete and
close one lane before crossing: a usage-discovered gap remains a bounded
finding until the user explicitly approves an Author and maintain follow-up.
Never treat a request to decide, audit, validate, or explain as permission to
edit durable pack authority. After an authoring handoff, rerun the CLI before
resuming use; do not reuse the earlier decision.

## Minimal Operator Journey

For human orientation, run `mdp status --dir PACK_ROOT` first. It is
observational, local/offline, and reports the exact next safe command. For
agent routing, inspect `mdp --json capabilities` first; never infer authority
from a human summary.

1. Identify the exact pack root and pass `--dir`; never assume the CWD.
2. Inspect `mdp --json capabilities`, `mdp --json skills`, and, when bound,
   `mdp --json skills --dir PACK_ROOT --job JOB_ID`.
3. Preserve every CLI blocker and return the canonical result plus the next
   permitted action.

For operator/validation mechanics, read [operator runtime](references/operator-runtime.md).
For a managed run, read [workflow bundle handoff](references/workflow-bundle-handoff.md).
For cold-model qualification only, read [cold-model conformance](references/cold-model-conformance.md).
For product boundaries only, read [mental model](references/mental-model.md).
Before using the CLI, MCP, or a plugin helper, read [runtime compatibility](references/runtime-compatibility.md).
Managed resume/review requires an explicit run directory and fresh verification; never select ambient/latest state.
Do not load all references by default, and references must not require a second
local-reference hop.

## Universal Authority Rules

Never upgrade `blocked`, `no-draft`, `unavailable`, invalid, or unknown to a
ready or usable result. New evidence requires a fresh CLI evaluation. Inspect `data.recommendation.product_foundation` before opening pack prose. Treat `.mdp/README.md` as secondary navigation only. Never substitute a natural-language job approximation for an exact job ID. Never invent missing product, ICP, proof, certification, compliance, or outcome facts. Foundation readiness only vetoes broader readiness.

For conformance, the customer-selected host owns provider/model selection and
the call; MDP does neither. Stop no-draft unless deterministic status is
`sufficient-for-job`. Treat `mdp.behavioral-evaluation.v1` as intermediate
only. The assembled result is the sole cross-phase `mdp.job-conformance.v1`
authority. No result grants drafting, sending, scheduling, CRM mutation, or
publication authority.

The Rust CLI is the decision authority. Preserve or reduce its authority; never upgrade `blocked`, `no-draft`, `unavailable`, invalid, or unknown. New evidence requires a new CLI evaluation; user intent cannot override an existing result in place.

## Closeout

Name the journey and report the pack root, selected owner/job, commands run,
readiness state, durable artifacts, unresolved gaps, next action, and
installed-versus-source uncertainty. Author and maintain closes only with
validated file changes or read-only findings; Use and decide closes with the
canonical decision, verified bundle when present, gaps, and next permitted
action.
