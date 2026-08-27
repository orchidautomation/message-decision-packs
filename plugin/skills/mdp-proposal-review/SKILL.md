---
name: mdp-proposal-review
description: Use when applying an existing proposal MDP to supplied RFP, capture, requirement, proof, matrix, or draft material for bid/no-bid, compliance, proof, or red-team review. Never certify, invent proof, grant final approval, write, or submit proposals.
---

# MDP Proposal Review

Own the proposal branch of **Use and decide**. Apply an approved proposal pack
to supplied pursuit material for bounded review support without changing
durable pack authority. This is review, not pack authoring or proposal
production.

## Communicate The Work

Follow the shared [Orient, Plan, Progress, Translate, Close contract](../mdp/references/communication-contract.md).
Open by naming the selected proposal job; the exact pack and supplied-material evidence boundary; the review packet and assurance decision the user will receive; and what this skill will not do. Keep updates to meaningful evidence
gates, blockers, and decisions.

## Select Exactly One Mode

- Bid/no-bid support: `bid-no-bid-review` → [bid/no-bid](references/bid-no-bid.md).
- Requirement coverage: `compliance-review` → [compliance](references/compliance.md).
- Theme and proof support: `proof-review` → [proof](references/proof.md).
- Adversarial gap review: `red-team-review` → [red team](references/red-team.md).

Validate `mdp --json skills --dir PACK_ROOT --job JOB_ID`. Proceed only when
the exact recommendation is this skill and `pack_ready` is true; there is no
fallback. Read [evidence path](references/evidence-path.md) only when assurance
or source/runner choice is at issue. Read [governed review](references/governed-review.md)
only after mode selection. Read [proof-output drafting](references/proof-output-drafting.md)
only if the user separately requests a rewrite after review. For a managed run, load the direct [workflow bundle handoff](../mdp/references/workflow-bundle-handoff.md).
Managed resume/review requires an explicit run directory and fresh verification; never select ambient/latest state.
Do not load every
reference or follow a second local-reference hop.

## Golden Review Path

1. Require the exact pack, job, scope, owner, and supplied approved sources.
2. Run `validate`, `gaps`, and exact-job `requirements`.
3. Preserve source approval, hashes, privacy, purpose, normalization, and
   lineage. Ambient chat is not evidence.
4. Require a ready model task and minimal routed context before governed
   execution; otherwise stop with exact diagnostics.
5. Validate the returned artifact and receipts, then apply the selected mode's
   evidence rules and human-review boundary.

## Universal Safety And Authority

The Rust CLI is the decision authority. Preserve or reduce its authority; never upgrade `blocked`, `no-draft`, `unavailable`, invalid, or unknown. New evidence requires a new CLI evaluation; user intent cannot override an existing result in place.


Never invent RFP text, requirements, deadlines, evaluator criteria, proof, certifications, compliance status, pricing, references, outcomes, past performance, or approvals. Keep restricted pursuit material out of public paths and generated fixtures. The result is decision support, not certification, legal advice, approval, or submission authority.

Never upgrade `blocked`, `no-draft`, `unavailable`, invalid, unknown, advisory,
or unassessed. Never certify, invent proof, grant final approval, write, or
submit proposals. Rewriting supplied material is a separate explicit request
and every changed claim must be revalidated.

If use reveals missing proof, policy, or source authority, preserve it as a
bounded gap or finding. Do not add evidence, edit `.mdp/`, auto-approve a
source, or repair the pack. Offer the smallest explicit Author and maintain
handoff to `$mdp-pack-builder`; resume this usage job only after that separate
edit is validated and the CLI is rerun.

## Response

Close Use and decide with the mode/job, reviewed pack and source boundary,
assurance state, canonical decision support, evidence-backed findings,
gaps/questions, verified bundle/receipt state, human owner, and next permitted
action. Never report pack files as changed.
