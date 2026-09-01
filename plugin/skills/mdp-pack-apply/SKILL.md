---
name: mdp-pack-apply
description: Apply an existing Message Decision Pack to supplied inputs for the exact profile job selected by the local mdp CLI. Do not edit the pack, collect missing data, perform downstream actions, or infer a job from vertical terminology.
metadata:
  compatibility: Requires the mdp CLI on PATH. Native plugin helper scripts additionally require Node.js 18+; portable skill installs use the CLI-only path and do not assume PLUGIN_ROOT or MCP support.
---

# MDP Pack Apply

Own **Use and decide** for every supported profile. Apply durable pack authority
to supplied inputs without changing the pack. The Rust CLI resolves the profile,
exact job, readiness, prompt, routed context, artifacts, and verification state;
this skill never substitutes its own routing judgment.

## Communicate The Work

Follow the shared [Orient, Plan, Progress, Translate, Close contract](references/communication-contract.md).
Open by naming the exact selected job, the pack and supplied-input evidence boundary,
what decision or artifact the user will receive, and what this skill will not do.
Keep updates to meaningful readiness gates, blockers, and decisions.

## Resolve Before Loading Detail

Require `PACK_ROOT` and an exact canonical `JOB_ID`. Run:

```bash
mdp --json skills --dir PACK_ROOT --job JOB_ID
mdp --json requirements --dir PACK_ROOT --job JOB_ID
```

Proceed only when the recommendation is `mdp-pack-apply`, the exact requested
job is returned, and `pack_ready` is true. Never infer a fallback from the
request's industry, deliverable, or prose.

Then load only the direct job reference selected by CLI output:

- GTM `prospect-fit-or-brief` → [fit or brief](references/gtm-prospect-fit-or-brief.md)
- GTM `outbound-copy-brief` → [pre-draft context](references/gtm-outbound-copy-brief.md)
- GTM `outbound-copy-review` → [supplied-copy review](references/gtm-outbound-copy-review.md)
- Proposal `bid-no-bid-review` → [bid/no-bid](references/proposal-bid-no-bid.md)
- Proposal `compliance-review` → [compliance](references/proposal-compliance.md)
- Proposal `proof-review` → [proof](references/proposal-proof.md)
- Proposal `red-team-review` → [red team](references/proposal-red-team.md)

Do not load both. For CLI versus MCP, local/native-call consent, clean-context
evaluation, file ownership, run artifacts, or resume/verification behavior,
read [runtime compatibility](references/runtime-compatibility.md) and then the
task-specific [runtime and execution contract](references/runtime-execution.md).
For a managed run, read the direct [workflow bundle handoff](references/workflow-bundle-handoff.md).
Do not follow a second local-reference hop.

When the selected job requires a governed model step, load only the matching
execution contract: [GTM governed execution](references/gtm-governed-execution.md)
or [proposal governed review](references/proposal-governed-review.md). For a
proposal source/runner assurance question, load [proposal evidence path](references/proposal-evidence-path.md).
Load [proof-output drafting](references/proposal-proof-output-drafting.md) only
for a separately authorized rewrite after the review closes.

## Common Apply Path

1. Treat the selected pack and supplied inputs as immutable authority.
2. Run exact-job validation, gaps, and requirements. Stop on invalid, blocked,
   unavailable, ambiguous, or no-draft results.
3. Prepare only the CLI-selected normalized input, prompt invocation, and
   bounded routed context. Never expose the whole pack to the evaluation call.
4. Use the CLI directly, or the existing four-tool MCP adapter only when the
   host needs local stdio transport for evaluation.
5. Validate the governed output, claims where applicable, run receipt, and
   verification receipt before reporting a result.

The Rust CLI is the decision authority. Preserve or reduce its authority; never upgrade `blocked`, `no-draft`, `unavailable`, invalid, unknown, advisory, or unassessed.
New evidence requires a new CLI evaluation; user intent cannot override an existing result in place.

## Ownership And Boundaries

Never edit `.mdp/`, approved source inputs, or user-supplied evaluation files.
Write only CLI-owned artifacts to the exact run/output paths returned by the
CLI. Never select ambient or latest state for resume; require the explicit run directory and fresh verification.

This skill does not enrich, scrape, prospect, draft outreach, send, sequence,
update a CRM, certify compliance, invent proof, approve or submit a proposal,
or perform downstream actions. If use reveals an authority gap, return it and
offer a separate explicit handoff to `$mdp-pack-builder`.

For cold-model evidence, require a passing `conformance compile` before handing
anything to the external host, then assemble `mdp.job-conformance.v1` only after
the recorded trial validates. `not-sufficient-for-job` and
`not-qualified-for-job-under-envelope` remain no-draft.

For proposal-profile jobs: Never certify, invent proof, grant final approval, write, or submit proposals.
Never invent RFP text, requirements, deadlines, evaluator criteria, proof, certifications, compliance status, pricing, references, outcomes, past performance, or approvals.
Keep restricted pursuit material out of public paths and generated fixtures.
The result is decision support, not certification, legal advice, approval, or submission authority.

## Response

Close with the exact profile, job, pack, supplied-input boundary, canonical
decision or artifact, accepted and rejected evidence, gaps, run/receipt state,
and next permitted action. Never report pack files as changed.
