---
name: mdp-proposal-compliance-review
description: Use when the user wants to review supplied proposal, RFP, requirement, or answer-draft context against a proposal MDP pack's compliance rules. Produces review support with requirement coverage, gaps, risks, owner questions, and explicit human-review boundaries without certifying compliance.
---

# MDP Proposal Compliance Review

## Profile Gate

Before using this skill against an existing pack, run:

```bash
mdp --json agent-surface --dir .
```

Use this skill only when the surface is legacy/generic or this skill is listed in `recommended_skills` or `allowed_skills` and is not listed in `blocked_skills`. If the surface blocks this skill, stop and reroute to an allowed or recommended skill named by the surface before editing or reviewing pack content.

Use an existing proposal MDP pack to review supplied requirements, proposal outlines, answer drafts, or compliance-matrix notes. This skill provides review support only. It does not certify compliance and does not replace legal, procurement, security, or customer compliance review.

## Inputs

Required:

- pack directory
- approved requirement list, RFP snippets, compliance matrix, proposal outline, or answer draft
- review role or owner when known
- source notes for each requirement, clause, attachment, or reviewer instruction when available
- supplied answer, evidence, proof, owner, and status for each reviewed requirement when available

Useful:

- mandatory versus optional requirement labels
- submission instructions, evaluation factors, eligibility rules, and due dates
- accepted certifications, security attestations, references, or proof constraints
- known exclusions, assumptions, open questions, and reviewer notes

If required context is missing, return a gap list instead of filling the matrix with guesses. Do not invent RFP text, requirement IDs, compliance status, proof, certifications, legal interpretation, security posture, customer approvals, or past performance.

## Source Handling

- Keep raw restricted proposal material in ignored scratch or the user's local-only workspace.
- Do not commit customer-identifying, access-controlled, regulated, pricing, strategy, or past-performance material unless the user explicitly approves a sanitized artifact.
- Preserve provenance for each requirement: source kind, locator or note ID, freshness, confidence, and whether the status is directly sourced or interpreted.
- Treat synthetic and sanitized examples as examples, not real compliance evidence.

## Workflow

1. Check the CLI and pack:

```bash
command -v mdp
mdp --json doctor --dir .
mdp --json validate --dir .
```

2. If the supplied requirement or RFP context is messy, use `.mdp/prompts/normalize-opportunity.yaml` as the normalization scaffold and validate the output before relying on it:

```bash
mdp --json validate-prompt-output --dir . --prompt-id normalize-opportunity --file <prompt-output.json>
```

Read `normalization_trace.fit_readiness`, `gaps`, `signals`, and bounded `attributes` before building the matrix. If readiness is false, return a gap list instead of filling missing requirements or statuses.

3. Route the compliance context using the supplied review role when known. Default to `Solution Owner` for technical requirement coverage and `Proposal Lead` for proposal matrix coverage:

```bash
mdp --json --summary route --entries --dir . --persona "Solution Owner" --job "compliance review"
mdp --json gaps --dir .
```

Read the routed entries first. Open full card files only if the route output requires them or the review needs unresolved card detail.

4. Use these proposal cards when present:

- `requirements-matrix`
- `requirement-signals`
- `compliance-boundaries`
- `proposal-boundaries`
- `proposal-output-rules`
- `evaluation-criteria`
- `proof-library`
- `review-gates`
- `review-outputs`
- `proposal-roles`
- `gaps`

5. Build a requirement review matrix from supplied facts:

- requirement ID or short label
- requirement source and confidence
- mandatory, optional, or unclear obligation
- supplied answer or response path
- supporting evidence or proof
- coverage status
- gap or unsupported claim
- risk severity
- owner or reviewer question

6. Use conservative coverage statuses:

- `supported`: supplied answer and proof directly cover the requirement.
- `partial`: some response path exists, but proof, detail, scope, or owner is incomplete.
- `missing`: requirement has no supplied answer or response path.
- `unsupported`: answer claims support that the pack or source evidence does not substantiate.
- `out-of-scope`: requirement appears outside the offered scope or conflicts with boundaries.
- `needs-human-review`: legal, procurement, security, regulated-data, or customer-specific judgment is required.

Do not use `compliant` as a final status unless the user supplied that exact reviewed status from a named human source, and even then label it as supplied source language.

7. Check risky claim-bearing text before treating it as usable:

```bash
mdp --json check-claims --dir . --persona "Proposal Lead" --job "compliance review" --text "<claim-bearing text>"
```

Use `--strict` when warnings should block acceptance.

When a model or renderer produces claim-bearing compliance output with source, card, proof, or requirement IDs, require a `contract: mdp.proof-output.v0` artifact and run:

```bash
mdp --json verify-output --dir . --file <proof-output.json>
mdp verify-output --readable --dir . --file <proof-output.json>
```

Do not treat cited source IDs, card IDs, or requirement IDs as proof until `verify-output` returns `valid: true`. `verify-output` also applies pack-owned `constraints.proof_output`, including required segment kinds, minimum segment counts, claim source refs, and connective word limits. Use the readable output as the human review packet only; proof-output JSON remains the binding source of truth, and blocked readable output must not be reused as supported compliance language. Missing proof should remain a `gap` segment, not a supported compliance statement.

## Output Format

Return a concise compliance review packet:

- `review_status`: `ready-for-human-review`, `needs-more-info`, or `blocked`
- `review_owner`: person or role, or `unknown`
- `scope_reviewed`: supplied requirement set, draft section, matrix, or outline
- `source_notes`: source locator, freshness, and confidence summary
- `requirements`: list of `{id, requirement, source, obligation, supplied_answer, coverage_status, evidence, gap, risk, owner_or_question}`
- `unsupported_or_risky_claims`
- `missing_requirements_or_sources`
- `human_review_required`
- `next_questions`
- `claim_check_result` when claim-bearing text was reviewed
- `verify_output_result` when generated claim-bearing output included source or pack ID bindings
- `readable_review_result` when `mdp verify-output --readable` was produced

For `ready-for-human-review`, include remaining risks and the human reviewers needed. For `needs-more-info`, include the smallest source or owner inputs needed to rerun the review. For `blocked`, identify the decisive unsupported requirement, boundary conflict, or unavailable source.

## Boundaries

- Do not certify compliance or present the review as legal, procurement, security, or customer approval.
- Do not claim CMMC, NIST, CUI, privacy, accessibility, security, or regulatory compliance unless the supplied source explicitly supports the exact claim and the output labels it as source-provided.
- Do not infer that missing requirements are satisfied.
- Do not convert assumptions into supported answers.
- Do not submit proposals, update portals, send messages, update CRM/opportunity systems, or manage approval workflows.
- Do not place raw customer proposal material, access-controlled files, regulated content, pricing, or non-public strategy in public repo files, issues, PRs, or release notes.
