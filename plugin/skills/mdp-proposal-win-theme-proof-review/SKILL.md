---
name: mdp-proposal-win-theme-proof-review
description: Use when proposing or reviewing proposal win themes against approved proof, evaluation criteria, customer needs, and proposal MDP constraints. Produces proof-grounded themes, missing evidence, risks, and SME questions without inventing claims.
---

# MDP Proposal Win Theme And Proof Review

## Profile Gate

Before using this skill against an existing pack, run:

```bash
mdp --json agent-surface --dir .
```

Use this skill only when the surface is legacy/generic or this skill is listed in `recommended_skills` or `allowed_skills` and is not listed in `blocked_skills`. If the surface blocks this skill, stop and reroute to an allowed or recommended skill named by the surface before editing or reviewing pack content.

Use an existing proposal MDP pack to propose, review, or tighten win themes only when they are grounded in supplied evaluation criteria, requirement signals, approved proof, and pack boundaries. This skill is review support, not full proposal drafting.

## Inputs

Required:

- pack directory
- supplied opportunity, RFP, evaluator, or requirement context
- proposed theme, differentiator, proof point, or draft section when reviewing existing material
- approved proof, source notes, certification language, references, or explicit proof gaps when available
- target reviewer role or SME owner when known

Useful:

- evaluation factors, scoring weights, buyer pains, required outcomes, and must-answer sections
- competitor, incumbent, partner, or delivery constraints
- approved metrics, case examples, reference constraints, and claim-use rules
- SME questions, open proof gaps, and reviewer notes

If proof is missing, weak, or not approved for the reviewed claim, return `needs-more-proof` or `blocked`. Do not invent metrics, references, certifications, customer results, implementation history, evaluator priorities, or past performance.

## Source Handling

- Keep raw restricted proposal material in ignored scratch or the user's local-only workspace.
- Do not commit customer-identifying, access-controlled, regulated, pricing, strategy, or past-performance material unless the user explicitly approves a sanitized artifact.
- Preserve provenance for each proof point: source kind, locator or note ID, freshness, confidence, and whether the claim is directly sourced or interpreted.
- Treat synthetic and sanitized examples as examples, not real proof.

## Workflow

1. Check the CLI and pack:

```bash
command -v mdp
mdp --json doctor --dir .
mdp --json validate --dir .
```

2. If the supplied opportunity, evaluator, requirement, or proof context is messy, use `.mdp/prompts/normalize-opportunity.yaml` as the normalization scaffold and validate the output before relying on it:

```bash
mdp --json validate-prompt-output --dir . --prompt-id normalize-opportunity --file <prompt-output.json>
```

Read `normalization_trace.fit_readiness`, `gaps`, `signals`, and bounded `attributes` before reviewing proof. If readiness is false or proof is missing, return `needs-more-proof` or `blocked` rather than inventing support.

3. Route the win-theme proof context using the supplied review role when known. Default to `Solution Owner` for proof review and `Proposal Lead` for theme shape:

```bash
mdp --json --summary route --entries --dir . --persona "Solution Owner" --job "win theme proof review"
mdp --json gaps --dir .
```

Read the routed entries first. Open full card files only if the route output requires them or the review needs unresolved card detail.

4. Use these proposal cards when present:

- `proof-library`
- `evaluation-criteria`
- `requirement-signals`
- `requirements-matrix`
- `opportunity-context`
- `proposal-boundaries`
- `compliance-boundaries`
- `proposal-output-rules`
- `review-gates`
- `review-outputs`
- `proposal-roles`
- `gaps`

5. Build a proof-grounded theme matrix:

- evaluator need or requirement
- proposed theme or differentiator
- approved supporting proof
- claim language allowed by the proof
- missing proof or SME question
- risk if used as written
- recommended action

6. Use conservative theme statuses:

- `proof-backed`: the theme is supported by supplied proof and pack constraints.
- `needs-more-proof`: the theme may be plausible but lacks enough evidence, source confidence, or owner acceptance.
- `unsupported`: the theme makes a claim the pack or supplied source does not substantiate.
- `boundary-risk`: the theme touches compliance, security, regulated data, customer reference, or restricted proof boundaries.
- `draft-only`: the theme can be explored as a hypothesis, but must not be presented as an approved claim.

Separate proposed themes from approved claims. A theme can be useful without being approved copy.

7. Check risky claim-bearing text before treating it as usable:

```bash
mdp --json check-claims --dir . --persona "Solution Owner" --job "win theme proof review" --text "<claim-bearing text>"
```

Use `--strict` when warnings should block acceptance.

When a model or renderer produces generated theme/proof text with source, card, proof, or requirement IDs, require a `contract: mdp.proof-output.v0` artifact and run:

```bash
mdp --json verify-output --dir . --file <proof-output.json>
mdp verify-output --readable --dir . --file <proof-output.json>
```

Do not treat a model-selected source ID or proof ID as proof until `verify-output` resolves it against the pack, applies pack-owned `constraints.proof_output`, and the embedded full-text claim check is clean. Use the readable output as the human review packet only; proof-output JSON remains the binding source of truth, and blocked readable output must not be reused as approved proposal prose. Missing proof should remain a `gap` segment or `needs-more-proof`, not approved claim language.

## Output Format

Return a concise proof review packet:

- `review_status`: `ready-for-draft`, `needs-more-proof`, or `blocked`
- `review_owner`: person or role, or `unknown`
- `scope_reviewed`: proposed themes, draft section, proof set, or evaluator criteria
- `source_notes`: source locator, freshness, and confidence summary
- `themes`: list of `{theme, evaluator_need, status, approved_claims, supporting_proof, missing_proof, risk, next_question, recommended_action}`
- `unsupported_or_risky_claims`
- `missing_proof`
- `sme_questions`
- `claim_check_result` when claim-bearing text was reviewed
- `verify_output_result` when generated claim-bearing output included source or pack ID bindings
- `readable_review_result` when `mdp verify-output --readable` was produced

For `ready-for-draft`, include the exact claim language that is supported and any limits. For `needs-more-proof`, include the smallest proof or SME input needed. For `blocked`, identify the unsupported claim, boundary conflict, or missing source that blocks use.

## Boundaries

- Do not write a full proposal section unless the user asks after the proof review.
- Do not treat proposed themes as approved claims.
- Do not invent metrics, named references, certifications, customer outcomes, implementation history, evaluator priorities, or past performance.
- Do not use generic value claims when the pack requires sourced proof.
- Do not claim compliance, security, procurement, or customer approval unless supplied source material explicitly supports the exact claim and the output labels it as source-provided.
- Do not submit proposals, update portals, send messages, update CRM/opportunity systems, or manage approval workflows.
- Do not place raw customer proposal material, access-controlled files, regulated content, pricing, or non-public strategy in public repo files, issues, PRs, or release notes.
