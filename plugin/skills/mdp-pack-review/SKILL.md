---
name: mdp-pack-review
description: Use for read-only audit, validation, hardening, testing, or diagnosis of an existing Message Decision Pack itself, including structure, jobs, routes, prompts, gaps, evals, and installed parity. Do not edit unless review plus repair is explicit.
metadata:
  compatibility: Requires the mdp CLI on PATH. Native plugin helper scripts additionally require Node.js 18+; portable skill installs use the CLI-only path and do not assume PLUGIN_ROOT or MCP support.
---

# MDP Pack Review

Own the read-only maintenance phase of **Author and maintain** for versioned
decision context for agents. Flag graph database, agent runtime,
orchestration, persistent memory, universal graph, and source truth claims; a
decision graph is only a bounded projection. Produce evidence-backed findings;
do not silently repair the pack or take an authoring request from
`$mdp-pack-builder`.

## Communicate The Work

Follow the shared [Orient, Plan, Progress, Translate, Close contract](references/communication-contract.md).
Open by naming the selected review job; the exact pack and CLI evidence boundary; the findings or QA decision the user will receive; and what this
skill will not do. Keep updates to meaningful QA gates, blockers, and
decisions.

## Select One Review Mode

- Pack structure, evidence, jobs, and readiness: `structural` → [structural audit](references/structural-audit.md).
- Route, trigger, collision, or output behavior: `routing-eval` → [routing evals](references/routing-evals.md).
- Installed template/bundle parity: `installed-qa` → [installed template QA](references/installed-template-qa.md).

Load [review protocol](references/review-protocol.md) only after selecting a
mode that needs its deterministic or evidence gates. For execution evidence,
load the direct [managed workflow bundle handoff](references/workflow-bundle-handoff.md).
Before using the CLI, MCP, or a plugin helper, read [runtime compatibility](references/runtime-compatibility.md).
Managed resume/review requires an explicit run directory and fresh verification; never select ambient/latest state.
Do not load every reference by default or follow a second local-reference hop.

## Golden Review Path

1. Identify the exact pack or installed root; do not edit it.
2. Run `capabilities`, `skills --dir`, and `doctor --dir`.
3. Run only the selected narrow checks, then required strict gates.
4. Compare canonical and installed bytes when parity is in scope.
5. Return severity-ordered findings with command evidence and the smallest
   repair handoff.

Inspect CLI-resolved foundation before `.mdp/README.md`. For each exact canonical job ID, verify selected context and exclusions; optional or unrelated
foundation must not leak into selected context. Never invent product facts,
proof, or compliance to close a finding. Foundation readiness only vetoes broader readiness.

For conformance, require the full `conformance compile`, externally recorded trials, `conformance validate`, and `conformance assemble` flow. Deterministic `sufficient-for-job` is not
behavioral qualification. A behavioral evaluation alone is intermediate,
never report authority. Public results must exclude private content,
provider/session identifiers, evaluator rationale, reviewer identity, paths,
and private digests.

## Universal Authority And Ownership

The Rust CLI is the decision authority. Preserve or reduce its authority; never upgrade `blocked`, `no-draft`, `unavailable`, invalid, or unknown. New evidence requires a new CLI evaluation; user intent cannot override an existing result in place.


Never upgrade `blocked`, `no-draft`, `unavailable`, invalid, unknown, or
unassessed. If the user explicitly requests review plus repair, finish the
read-only review and state the transition to `$mdp-pack-builder`; do not touch
pack files in the review result. This skill does not enrich, review business
copy, certify compliance, submit work, or mutate downstream systems.

## Findings

Close Author and maintain review with severity, location, CLI evidence,
impact, repair recommendation, validation commands, reviewed roots, readiness
state, unresolved gaps, and an explicit optional builder handoff. Do not claim
that a finding has changed the pack.
