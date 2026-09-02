# MDP-343 — Owner Experience And Decision Governance Stack Plan

**Date:** 2026-09-02  
**Project:** MDP-343  
**Repository:** `orchidautomation/message-decision-packs`  
**Base branch:** `main`  
**Planning baseline:** `f54b1f09578f3a355b65a8fb9708e4e6df1c8018`  
**Planning branch:** `codex/mdp-owner-governance-plan`  
**Cumulative integration branch:** `codex/mdp-owner-governance-stack`  
**Risk:** Elevated — additive public pack contracts, authority/governance semantics, approval boundaries, and owner-facing CLI behavior

## 1. Objective

Ship MDP-345 through MDP-357 as one dependency-ordered, independently reviewed
same-repository stack while leaving `main` and production untouched. Every issue
uses a distinct issue-bound branch, worktree, tracked plan, and visible
`gpt-5.6-luna` medium execution lane. Sol owns planning, integration,
verification, review arbitration, and the cumulative pull request.

The accepted product authority is the Linear document **MDP Owner Experience
and Decision Artifact Contract v1** attached to MDP-344. The repository remains
the implementation authority for exact paths, symbols, compatibility, and tests.

## 2. Delivery Topology

1. Commit and push this project plan and the first executable child plan on
   `codex/mdp-owner-governance-plan`.
2. Create `codex/mdp-owner-governance-stack` from the pinned planning commit.
3. For each executable issue, create one `codex/mdp-<issue>-<slug>` branch and
   one separate worktree from the current cumulative head.
4. Dispatch exactly the pinned issue plan to Luna; the worker may implement but
   may not rescope product decisions.
5. Integrate the issue commit into `codex/mdp-owner-governance-stack`, run the
   issue-specific verification and repository regression gate, and bind review
   evidence to that exact head.
6. Push the cumulative branch and open or update one cumulative PR against
   `main`. Request `@codex review`; when Cubic.dev is enrolled or reports, wait
   for and address its findings too. Do not start the next dependent slice while
   a material review finding remains open.
7. Pin the next child plan against the new cumulative head and repeat.
8. Stop before merge, release, deployment, provider calls, or any external
   production mutation. Brandon retains the final merge decision.

No issue branch is merged directly into `main`. No force push or branch deletion
is part of this flow.

## 3. Dependency-Ordered Waves

| Order | Issue | Outcome | Entry gate |
|---:|---|---|---|
| 1 | MDP-345 | Temporal provenance and deterministic health semantics | Accepted MDP-344 contract and pinned child plan |
| 2 | MDP-346 | Job-scoped decision completeness and coverage | MDP-345 integrated and reviewed |
| 3 | MDP-347 | Canonical pack Overview | MDP-345 and MDP-346 integrated and reviewed |
| 4 | MDP-348 | Decision Proposal, minimal approval receipt, Review queue, and skill communication | MDP-345 through MDP-347 integrated and reviewed |
| 5 | MDP-349 | Conflict Case and human adjudication | MDP-348 integrated and reviewed |
| 6 | MDP-350 | Candidate change and behavioral-impact preview | MDP-347 through MDP-349 integrated and reviewed |
| 7 | MDP-351 | Actionable maintenance-health report | MDP-345, MDP-346, and MDP-350 integrated and reviewed |
| 8 | MDP-352 | Guided job-first pack creation | MDP-346 through MDP-350 integrated and reviewed |
| 9 | MDP-353 | Optional AI-assisted source intake preflight | MDP-345, MDP-348, and MDP-349 integrated and reviewed; provider calls remain unapproved |
| 10 | MDP-354 | Durable review, rationale, revocation, supersession, and ancestry history | MDP-345, MDP-348, and MDP-350 integrated and reviewed |
| 11 | MDP-355 | Conversational, owner-readable pack scenarios | MDP-346, MDP-347, and guided creation surfaces integrated |
| 12 | MDP-356 | Usage findings as non-authoritative maintenance input | Review queue, impact, and durable history integrated |
| 13 | MDP-357 | Fresh creator, owner, and maintainer acceptance | All implementation slices integrated and reviewed |

The order is deliberately serial even where Linear permits parallel work. The
contracts are cumulative, and a serial stack prevents two workers from defining
competing governance, readiness, or receipt vocabulary.

## 4. Stable Product Boundaries

- Manifest/card entries remain messaging authority.
- Decision groups index exact entry references and own only membership,
  ownership, affected-job, and review-policy authority; they never duplicate
  decision prose.
- Source ledger entries remain source-interpretation authority. Hashes establish
  byte identity, not factual truth.
- Overview, Decisions, Jobs, temporal health, maintenance health, and impact
  reports are deterministic projections and cannot upgrade readiness.
- Review queue artifacts are explicit, local, removable, and non-authoritative.
- AI outputs are candidates only. Every provider call requires a separate
  privacy/cost preflight and explicit authorization.
- Proposal approval binds one exact proposal, live pack, candidate, impact
  report, affected authority, reviewer, time, and rationale. Approval does not
  mutate the pack by itself.
- Existing packs remain valid and keep their current readiness when new
  governance metadata is absent; new projections report `unassessed` or
  `unknown` instead of inventing state.

## 5. Per-Slice Review And Verification Gate

Every issue must provide all of the following before the next issue starts:

1. A committed issue plan pinned by source ref, exact 40-character commit, and
   SHA-256.
2. An Orchid Work dispatch receipt proving Luna medium executed the pinned plan
   in the issue worktree.
3. Focused tests named in the child plan plus the repository regression gate
   required by the changed surfaces.
4. An Orchid verification receipt and Orchid review bound to the exact
   cumulative head.
5. A pushed cumulative branch and updated cumulative PR.
6. `@codex review` requested on the updated PR. Any material Codex or Cubic.dev
   finding is fixed on the same issue lane, reverified, repushed, and rereviewed
   before the slice closes.
7. Linear receives only public-safe PR/check/review evidence and the lifecycle
   state appropriate to the verified result.

## 6. Planning And Change Control

Only MDP-345 is executable in the first Commit-to-Build receipt. MDP-346 through
MDP-357 remain blocked in that receipt by their predecessor issue(s) and do not
receive speculative branches or Luna lanes. After each integration, Sol inspects
the new cumulative repository state, writes the next child plan on the planning
branch, creates a new immutable Commit-to-Build operation, reconciles Linear,
then materializes exactly one new issue lane.

A changed product decision, schema authority boundary, or destructive migration
stops execution and returns to Brandon. Ordinary implementation details and
test-guided corrections remain inside the pinned plan.

## 7. Validation And Closeout

The stack-level closeout requires:

- all MDP-345 through MDP-357 acceptance criteria mapped to tests or documented
  human journey evidence;
- compatibility fixtures for packs without governance metadata;
- creator, owner, and maintainer journeys using synthetic/public-safe data;
- one cumulative PR whose exact head has green required checks and no unresolved
  material Codex/Cubic finding;
- no committed runtime receipts, private Linear text, secrets, customer data,
  or local-only paths;
- a concise handoff identifying the PR, exact head, checks, review evidence,
  remaining human decisions, and the fact that merge/release were not performed.

**Project readiness:** `READY_TO_PIN` for MDP-345 only. Later issue execution is
intentionally gated on cumulative integration, fresh repository inspection, and
a distinct child-plan pin.

