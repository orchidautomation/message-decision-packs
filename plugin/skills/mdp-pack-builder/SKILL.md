---
name: mdp-pack-builder
description: Use when the user intends to create, initialize, reconstruct, or edit a Message Decision Pack from approved GTM, source, RFP, proposal, or capture material. Do not use for read-only pack audit or for applying a pack to a prospect or proposal.
---

# MDP Pack Builder

Own the mutation phase of **Author and maintain**. Build evidence-grounded
`.mdp/` decision context from approved material; never take a read-only review
request from `$mdp-pack-review` or infer edit permission from a usage gap.

## Communicate The Work

Follow the shared [Orient, Plan, Progress, Translate, Close contract](../mdp/references/communication-contract.md).
Open by naming the selected authoring job; the exact pack and approved-source evidence boundary; the files and readiness handoff the user will receive; and
what this skill will not do. Keep updates to meaningful validation gates,
blockers, and authoring decisions.

## Select One Authoring Mode

- Source inventory and authority plan: `source-plan` → [source intake](references/source-intake.md).
- Extract approved material into pack primitives: `source-extract` → [source intake](references/source-intake.md).
- Author a GTM pack: `gtm-authoring` → [GTM authoring](references/gtm-authoring.md).
- Author a proposal pack: `proposal-authoring` → [proposal authoring](references/proposal-authoring.md).

Load [Decision Input Contracts](references/decision-input-contracts.md) only
when the selected job needs governed normalization. Load [boundaries and
output](references/boundaries-output.md) only for claims, avoid rules, output
constraints, or proof-carrying artifacts. After mode selection and before any
edit, load [safe authoring](references/safe-authoring.md). These are direct,
one-level references; do not load every reference by default.

## Golden Authoring Path

1. Identify `PACK_ROOT`, profile, target, and exact approved sources.
2. Inspect `mdp --json skills --dir PACK_ROOT` and `mdp --json doctor --dir PACK_ROOT`.
3. Work in a complete candidate outside the live pack. Use `author preview`
   and then `author apply`; never make an unsealed multi-file live edit.
4. Refresh/check generated README regions and validate the candidate.
5. Run exact-job `skills`, `requirements`, strict validation, gaps, and eval.

Use exact canonical job IDs. Product foundation facets must index exact existing card/entry refs rather than duplicate prose. Keep README only as concise secondary navigation. Preserve unsupported facts as gaps; never invent them to make a job ready. Foundation `ready` is veto-only and cannot promote
an otherwise unready job.

## Universal Authority And Ownership

The Rust CLI is the decision authority. Preserve or reduce its authority; never upgrade `blocked`, `no-draft`, `unavailable`, invalid, or unknown. New evidence requires a new CLI evaluation; user intent cannot override an existing result in place.


The Rust CLI is authoritative. Never upgrade a blocker or reuse an old result
after evidence changes. Mutate only when edit intent is explicit. A request to
audit, diagnose, validate, or test an existing pack without changes belongs to
`$mdp-pack-review`; report findings first if repair is also requested.

Never scrape gated sources, commit restricted material, invent authority,
draft outreach or proposals, send, submit proposals, or mutate downstream
systems.

An explicit authoring handoff may start after a use or review result, but it
must name the approved finding, source boundary, intended files, and edit
request. Treat that as a new lane: preview and apply safely, validate the new
authority, and require fresh CLI evaluation before any Use and decide work
resumes.

## Response

Close Author and maintain with the selected mode, pack/candidate roots, source
classes, changed files, preview/apply status, validation/readiness state,
durable artifacts, and remaining gaps. Do not return a usage decision from the
authoring result.
