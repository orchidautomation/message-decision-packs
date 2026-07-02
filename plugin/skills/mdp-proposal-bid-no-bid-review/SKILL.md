---
name: mdp-proposal-bid-no-bid-review
description: Use when the user wants to assess a supplied proposal, RFP, capture, or pursuit context against a proposal MDP pack's bid/no-bid rules. Produces bid, no-bid, or needs-more-info decision support with blockers, gaps, and follow-up questions.
---

# MDP Proposal Bid/No-Bid Review

## Profile Gate

Before using this skill against an existing pack, run:

```bash
mdp --json agent-surface --dir .
```

Use this skill only when the surface is legacy/generic or this skill is listed in `recommended_skills` or `allowed_skills` and is not listed in `blocked_skills`. If the surface blocks this skill, stop and reroute to an allowed or recommended skill named by the surface before editing or reviewing pack content.

Use an existing proposal MDP pack to assess whether a pursuit should proceed, stop, or wait for more context. This skill provides decision support only. A human owner still makes the pursuit decision.

## Inputs

Required:

- pack directory
- opportunity or pursuit context approved for local review
- review owner or decision owner when known
- customer, agency, evaluator, partner, incumbent, and internal owner identities when known
- source snippets or notes for requirements, eligibility, submission instructions, due date, and evaluation factors
- proof and compliance claims only when approved and sourced

Useful:

- procurement vehicle, budget clues, timeline, incumbent clues, partner constraints, and conflict checks
- must-have capabilities, security/privacy obligations, and staffing constraints
- known disqualifiers, no-bid rules, or escalation criteria
- open questions and missing documents

If required context is missing, return `needs-more-info`. Do not invent RFP text, deadlines, evaluator criteria, compliance status, pricing, proof, past performance, or customer results.

## Source Handling

- Keep raw private proposal material in ignored scratch or the user's private workspace.
- Do not commit customer-identifying, access-controlled, pricing, strategy, or past-performance material unless the user explicitly approves a sanitized artifact.
- Preserve provenance: source kind, locator or note ID, freshness, confidence, and whether a fact is directly sourced or interpreted.
- Treat synthetic and sanitized examples as examples, not real opportunities.

## Workflow

1. Check the CLI and pack:

```bash
command -v mdp
mdp --json doctor --dir .
mdp --json validate --dir .
```

2. Route the bid/no-bid context:

```bash
mdp --json --summary route --entries --dir . --persona "Proposal Lead" --job "bid no bid review"
mdp --json gaps --dir .
```

Read the routed entries first. Open full card files only if the route output requires them or the review needs unresolved card detail.

3. Use these proposal cards when present:

- `bid-no-bid-rules`
- `evaluation-criteria`
- `opportunity-context`
- `requirement-signals`
- `requirements-matrix`
- `proposal-roles`
- `compliance-boundaries`
- `proposal-boundaries`
- `proof-library`
- `review-gates`
- `gaps`

4. Build a review matrix from supplied facts:

- source-backed facts
- matched proceed criteria
- matched no-bid or pause criteria
- unresolved eligibility, compliance, proof, timeline, or owner questions
- assumptions that need human acceptance
- source gaps that block a confident decision

5. Decide the support status:

- `no-bid`: a hard disqualifier, prohibited request, policy bypass, unrecoverable mandatory requirement gap, or unapproved proof/compliance claim is present.
- `needs-more-info`: required source text, eligibility, evaluation criteria, due date, proof, owner, or compliance context is missing or too weak.
- `bid`: proceed criteria are met, no hard disqualifier is matched, proof and requirements are sufficiently sourced, and remaining gaps are non-blocking.

When in doubt, choose `needs-more-info` and list the exact questions.

6. Check risky claim-bearing text before presenting it as usable:

```bash
mdp --json check-claims --dir . --persona "Proposal Lead" --job "bid no bid review" --text "<claim-bearing text>"
```

Use `--strict` when warnings should block acceptance.

## Output Format

Return a concise decision brief:

- `status`: `bid`, `no-bid`, or `needs-more-info`
- `confidence`: `low`, `medium`, or `high`
- `decision_owner`: person or role, or `unknown`
- `rationale`: source-backed bullets only
- `matched_proceed_criteria`
- `matched_disqualifiers_or_pause_rules`
- `blockers`
- `missing_evidence`
- `follow_up_questions`
- `required_human_review`
- `source_notes`
- `claim_check_result` when claim-bearing text was reviewed

For `bid`, include the remaining risks and human sign-off needed. For `no-bid`, include the decisive blocker and whether it is reversible. For `needs-more-info`, include the smallest set of inputs needed to rerun the review.

## Boundaries

- Do not present the result as final executive approval.
- Do not ignore disqualifiers because the opportunity looks attractive.
- Do not calculate pricing, margin, or pursuit ROI unless the pack or user supplies explicit criteria and evidence.
- Do not scrape portals, update CRM/opportunity systems, submit proposals, send messages, or manage approval workflows.
- Do not claim legal, compliance, security, or procurement approval.
- Do not smooth missing evidence into a confident `bid`.
