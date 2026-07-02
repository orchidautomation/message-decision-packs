---
name: mdp-proposal-red-team-gap-review
description: Use when the user wants to red-team or gap-review supplied proposal, RFP, capture, compliance-matrix, or answer-draft material against a proposal MDP pack. Produces prioritized gaps, risks, evidence references, affected sections, owner questions, and next actions without rewriting the proposal or claiming final review authority.
---

# MDP Proposal Red-Team Gap Review

## Profile Gate

Before using this skill against an existing pack, run:

```bash
mdp --json agent-surface --dir .
```

Use this skill only when the surface is legacy/generic or this skill is listed in `recommended_skills` or `allowed_skills` and is not listed in `blocked_skills`. If the surface blocks this skill, stop and reroute to an allowed or recommended skill named by the surface before editing or reviewing pack content.

Use an existing proposal MDP pack to red-team supplied proposal material and surface the highest-risk gaps, weak inferences, unsupported claims, contradictions, and reviewer questions. This skill provides review support only. It does not replace proposal leadership, legal, procurement, security, compliance, or customer review.

## Inputs

Required:

- pack directory
- supplied proposal, RFP, capture, requirement, matrix, outline, or answer-draft material approved for local review
- review scope, proposal section, or decision point
- reviewer role or issue owner when known
- source notes for requirements, proof, boundaries, evaluator criteria, or known gaps when available

Useful:

- evaluation factors, scoring weights, must-answer sections, and due dates
- bid/no-bid rules, compliance boundaries, proof constraints, and customer reference limits
- reviewer notes, SME questions, open assumptions, and prior review findings
- proposed claims, win themes, differentiators, or answer text that may need claim checking

If the supplied material is too thin, return a prioritized source-gap list. Do not invent RFP text, requirements, proof, certifications, evaluator priorities, customer references, scores, past performance, or final approval status.

## Source Handling

- Keep raw restricted proposal material in ignored scratch or the user's local-only workspace.
- Do not commit customer-identifying, access-controlled, regulated, pricing, strategy, or past-performance material unless the user explicitly approves a sanitized artifact.
- Preserve provenance for each finding: source kind, locator or note ID, freshness, confidence, and whether the finding is directly sourced or interpreted.
- Treat synthetic and sanitized examples as examples, not real proposal evidence.

## Workflow

1. Check the CLI and pack:

```bash
command -v mdp
mdp --json doctor --dir .
mdp --json validate --dir .
```

2. Route the red-team context using the supplied review role when known. Default to `Executive Reviewer` for cross-section risk review and `Proposal Lead` for section-level ownership:

```bash
mdp --json --summary route --entries --dir . --persona "Executive Reviewer" --job "red team gap review"
mdp --json gaps --dir .
```

Read the routed entries first. Open full card files only if the route output requires them or the review needs unresolved card detail.

3. Use these proposal cards when present:

- `proposal-boundaries`
- `compliance-boundaries`
- `bid-no-bid-rules`
- `requirement-signals`
- `requirements-matrix`
- `evaluation-criteria`
- `proof-library`
- `review-gates`
- `review-outputs`
- `proposal-output-rules`
- `proposal-roles`
- `opportunity-context`
- `gaps`

4. Build a prioritized gap register from supplied facts:

- affected section, requirement, or decision point
- issue type: missing source, unsupported claim, weak inference, contradiction, boundary risk, unanswered requirement, owner gap, or output-contract gap
- source evidence and pack reference
- confidence and uncertainty
- severity and why it matters
- owner, reviewer, or SME question
- suggested next action

5. Use conservative severity labels:

- `blocker`: must be resolved before the material is used or reviewed further.
- `high`: likely to damage compliance, evaluation fit, proof posture, bid/no-bid confidence, or reviewer trust.
- `medium`: should be resolved before final review, but may not block an early draft.
- `low`: clarity, structure, or traceability issue that still needs an owner.
- `watch`: plausible issue that depends on missing source or reviewer judgment.

Use `low-confidence-inference` for points that may be true but are not adequately sourced. Use `missing-information` when the absence of source material is the core issue. Do not convert either into a supported claim.

6. Check risky claim-bearing text before treating it as usable:

```bash
mdp --json check-claims --dir . --persona "Executive Reviewer" --job "red team gap review" --text "<claim-bearing text>"
```

Use `--strict` when warnings should block acceptance.

## Output Format

Return a concise red-team gap review packet:

- `review_status`: `ready-for-human-review`, `needs-more-info`, or `blocked`
- `review_owner`: person or role, or `unknown`
- `scope_reviewed`: proposal section, matrix, outline, claim set, or decision point
- `source_notes`: source locator, freshness, and confidence summary
- `priority_order`: highest-risk categories first
- `gaps`: list of `{severity, issue_type, issue, affected_section, evidence, pack_reference, confidence, owner_or_question, suggested_next_action}`
- `unsupported_or_risky_claims`
- `contradictions_or_boundary_risks`
- `missing_sources_or_requirements`
- `human_review_required`
- `claim_check_result` when claim-bearing text was reviewed

For `ready-for-human-review`, include remaining known risks and reviewer questions. For `needs-more-info`, include the smallest source or owner inputs needed to rerun the review. For `blocked`, identify the decisive unsupported claim, missing source, contradiction, or boundary conflict.

## Boundaries

- Do not rewrite the proposal wholesale.
- Do not present the review as final approval or final red-team authority.
- Do not create automated scores unless the customer-defined rubric is supplied.
- Do not invent requirements, proof, certifications, customer results, evaluator priorities, scores, or past performance.
- Do not smooth missing information into confident critique.
- Do not submit proposals, update portals, send messages, update CRM/opportunity systems, or manage approval workflows.
- Do not place raw customer proposal material, access-controlled files, regulated content, pricing, or non-public strategy in public repo files, issues, PRs, or release notes.
