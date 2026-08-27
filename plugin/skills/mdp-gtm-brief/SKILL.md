---
name: mdp-gtm-brief
description: Use when applying a GTM Message Decision Pack to a supplied prospect for fit/readiness, bounded pre-draft context, or review of supplied outbound copy. Do not use for pack editing, prospecting, enrichment, copywriting, sending, CRM, or proposals.
---

# MDP GTM Brief

Apply a GTM pack to supplied inputs for bounded decision support. Never enrich,
draft outreach, send, schedule, or update a CRM.

## Communicate The Work

Follow the shared [Orient, Plan, Progress, Translate, Close contract](../mdp/references/communication-contract.md).
Open by naming the selected GTM job; the exact pack and supplied-input evidence boundary; the fit decision, bounded brief, or review artifact the user will receive; and what this skill will not do. Keep updates to meaningful readiness
gates, blockers, and decisions.

## Select Exactly One Mode

- Fit or fit plus a prospect brief: `prospect-fit-or-brief` → [fit mode](references/prospect-fit-or-brief.md).
- Pre-draft context, not copy: `outbound-copy-brief` → [brief mode](references/outbound-copy-brief.md).
- Review copy supplied by the user: `outbound-copy-review` → [copy-review mode](references/outbound-copy-review.md).

Validate `mdp --json skills --dir PACK_ROOT --job JOB_ID`. Proceed only when
the exact recommendation is this skill and `pack_ready` is true; there is no
fallback job. Load [governed execution](references/governed-execution.md) only
when the selected mode requires normalized input, routed context, a model step,
or receipts. For a managed run, load the direct [workflow bundle handoff](../mdp/references/workflow-bundle-handoff.md).
Managed resume/review requires an explicit run directory and fresh verification; never select ambient/latest state.
Do not load every mode or follow a second local-reference hop.

## Golden GTM Path

1. Require the exact pack, job, target, channel, and supplied prospect/source
   inputs. Do not collect missing data here.
2. Run `validate`, `gaps`, and exact-job `requirements`.
3. Preserve the selected input contract and lineage. Detached input is allowed
   only when requirements declare no resolved Decision Input Contract.
4. Require a ready bounded routed-context artifact before a governed model
   task; never open excluded entries or the whole pack.
5. Validate the governed artifact, check claims where applicable, and return
   the canonical decision and gaps.

For cold-model evidence, require a passing `conformance compile` before handing
anything to the external host, then assemble `mdp.job-conformance.v1` after the
recorded trial is validated. `not-sufficient-for-job` and
`not-qualified-for-job-under-envelope` remain no-draft. Conformance never
authorizes drafting or sending.

## Universal Authority And Boundaries

The Rust CLI is the decision authority. Preserve or reduce its authority; never upgrade `blocked`, `no-draft`, `unavailable`, invalid, or unknown. New evidence requires a new CLI evaluation; user intent cannot override an existing result in place.


Never upgrade CLI blockers or infer fit from identity, prose, provider fields,
or unsupported signals. Use only supplied inputs and selected pack authority.
This skill is not a pack editor, enrichment provider, copywriter, sequencer, or
CRM operator.

## Response

Return the exact job and pack, canonical status, fit/brief/review result,
accepted and rejected evidence, gaps, minimality/receipt state, durable run
pointer when present, and the next permitted action.
