---
name: mdp-proposal-pack-builder
description: Use when the user wants an agent to build or improve a proposal, RFP, capture, bid/no-bid, compliance, proof, red-team, or executive-review Message Decision Pack from approved source material. Produces a validated proposal pack draft or an explicit missing-information list.
---

# MDP Proposal Pack Builder

Build a proposal reference-profile pack from approved source material. This skill coordinates source intake, extraction, pack edits, validation, and review. It does not submit proposals, manage approvals, certify compliance, replace legal/procurement/security review, or create approved proposal content.

For bid/no-bid review against an existing proposal pack and supplied pursuit context, use `$mdp-proposal-bid-no-bid-review` instead. For compliance review against existing requirements, matrices, or answer drafts, use `$mdp-proposal-compliance-review` instead. For win-theme or proof review against existing themes, differentiators, or claim-bearing draft text, use `$mdp-proposal-win-theme-proof-review` instead. For red-team or gap review against supplied proposal material, use `$mdp-proposal-red-team-gap-review` instead.

## Profile Gate

Before editing an existing pack, run:

```bash
mdp --json agent-surface --dir .
```

Use this skill only when the surface is legacy/generic during new-pack creation or `profile.id` is `proposal` and `mdp-proposal-pack-builder` is allowed. If the surface blocks `mdp-proposal-pack-builder`, stop and reroute to the allowed/recommended skill named in the surface. GTM packs should use GTM pack and ICP skills, not proposal review cards.

`profile.id: proposal` and `profile.agent_surface` only route skills. Full proposal profile activation is reported by `mdp --json validate --dir .` in `data.profile.activation_ready` and depends on the proposal `primitive_map`, `opportunity` input contract, profile jobs, and categorized `profile_eval` fixtures. Keep opportunity context, requirements, proof, and review jobs as proposal profile vocabulary; do not add proposal-specific core card kinds.

## Intake Gate

Before writing pack files, identify the destination directory and classify the source material:

- `synthetic-example`: fictional workshop or demo material.
- `sanitized-example`: intentionally reviewed and stripped of customer-identifying or confidential detail.
- `user-provided-approved`: material the user says can be used in the local pack.
- `private-scratch`: raw proposal, RFP, customer, pricing, past-performance, or strategy material that must stay out of public commits.
- `public-source`: public material with a current source URL or citation.

If the user has not approved the material for the pack, stop and return a missing-information list. If the material appears confidential, regulated, access-controlled, or customer-identifying, keep it in ignored private scratch and do not commit it unless the user explicitly approves a sanitized version.

Required intake:

- opportunity or review scenario
- customer, agency, evaluator, partner, incumbent, and internal owner identities when known
- RFP or requirement snippets approved for use
- due dates, procurement vehicle, submission instructions, and review mode when known
- bid/no-bid gates and disqualifiers
- evaluation criteria or scoring factors
- proof, certifications, references, past performance, differentiators, and approval status
- compliance, security, privacy, and confidentiality boundaries
- desired review outputs, such as bid/no-bid brief, compliance matrix, proof review, red-team gap report, or executive brief
- known gaps, unknowns, and human reviewers

Never invent missing proof, compliance status, certifications, customer results, references, pricing, deadlines, evaluator criteria, RFP text, or past performance. Put missing or weak information in `gaps.yaml`.

## Workflow

1. Check the CLI and current pack:

```bash
command -v mdp
mdp --json capabilities
mdp --json doctor --dir .
```

2. If no pack exists, initialize the proposal template:

```bash
mdp --json init --template proposal --dir . --dry-run
mdp --json init --template proposal --dir .
```

Use the shipped synthetic template at `plugin/assets/templates/proposal` as the reference shape. Preserve profile-owned card IDs and review jobs unless the user explicitly asks for a new proposal profile.
Preserve the template's `required_primitives`, `primitive_map`, `input_contracts`, profile `jobs`, and `profile_eval.required_categories` unless the source material justifies a reviewed change.

3. Build or update `.mdp/sources.yaml` before writing cards. Record source kind, locator or note ID, approved use, freshness, confidence, direct source claims, interpretation, and gaps. Keep raw source text out of public paths unless it is synthetic or intentionally sanitized.

4. When source material arrives as messy opportunity, RFP, capture, requirement, compliance-matrix, or proof notes, use the pack-owned `.mdp/prompts/normalize-opportunity.yaml` contract as the normalization scaffold. Pass `existing_pack_context` with proposal personas, `lead_input_requirements.value_contracts`, `attribute_definitions`, source policy, proposal cards, and review jobs. Validate the model output before relying on it:

```bash
mdp --json validate-prompt-output --dir . --prompt-id normalize-opportunity --file <prompt-output.json>
```

Use only the manifest-declared enum values and attributes from the validated output. If `normalization_trace.fit_readiness.ready_for_mdp_fit` is false, keep the missing context in gaps and do not smooth it into accepted card entries.

5. Extract candidate entries into a review artifact first. Do not treat extracted rules as accepted until the user or designated reviewer accepts them.

Map source material into proposal cards:

- `proposal-roles` (`personas`): customer, agency, evaluator, buyer, proposal owner, solution owner, reviewer, incumbent, partner.
- `opportunity-context` (`signals`): RFP title, source, due date, vehicle, timeline, incumbent clues, budget clues, source snippets, and provenance.
- `requirement-signals` (`signals`): requirement snippets, submission instructions, compliance clauses, and source confidence.
- `requirements-matrix` (`pains`): stated needs, obligations, must-answer sections, and solution requirements.
- `bid-no-bid-rules` (`fit-rules`): proceed, pause, decline, escalation, and no-bid rules.
- `evaluation-criteria` (`fit-rules`): scoring factors and evaluator decision rules.
- `proof-library` (`claims`): approved proof, references, certifications, differentiators, and claim-use rules.
- `compliance-boundaries` and `proposal-boundaries` (`avoid-rules`): no-invention rules, confidentiality limits, unsupported claims, compliance exclusions, and external-execution boundaries.
- `review-gates` (`motions`): bid/no-bid review, compliance review, win-theme proof review, red-team gap review, and executive brief routing jobs.
- `proposal-output-rules` (`output-rules`): deterministic output shape, no submission, no legal/compliance certification language, and review-needed markers.
- `review-outputs` (`copy-patterns`): reusable brief, matrix, proof review, risk report, and executive summary structures.
- `gaps` (`gaps`): missing RFP sections, missing proof, weak source confidence, unclear owners, unresolved compliance questions, and privacy blockers.
- `.mdp/evals/*.yaml`: proceed, insufficient-context, refusal/escalation, unsafe-output, public-safety, job-routing, and prompt-output validation cases.

6. Edit the pack in slices:

- Source ledger, roles, opportunity context, requirement signals, and gaps.
- Bid/no-bid rules, evaluation criteria, compliance boundaries, and proof library.
- Review gates, output rules, review output patterns, and eval candidates.

7. Validate after each meaningful slice:

```bash
mdp --json validate --dir .
mdp --json gaps --dir .
mdp --json eval --dir .
```

8. Test representative proposal routes:

```bash
mdp --json --summary route --entries --eval-fixture --dir . --persona "Proposal Lead" --job "bid no bid review"
mdp --json --summary route --entries --eval-fixture --dir . --persona "Solution Owner" --job "compliance review"
mdp --json --summary route --entries --eval-fixture --dir . --persona "Executive Reviewer" --job "red team gap review"
mdp --json --summary route --entries --eval-fixture --dir . --persona "Executive Reviewer" --job "executive brief"
```

9. Run claim and boundary checks for risky generated or proposed text:

```bash
mdp --json check-claims --dir . --persona "Proposal Lead" --job "compliance review" --text "<claim-bearing text>"
```

Use `--strict` when warnings should block acceptance.

## Human Review

Before treating the pack as usable, present a review packet:

- sources used and source approval status
- entries added or changed by card
- extracted assumptions that need human acceptance
- unsupported claims moved to gaps or avoid-rules
- compliance, privacy, or proof risks
- eval fixtures added or still missing
- validation, gaps, eval, route, and claim-check results

If required source material is missing or cannot be used safely, output an explicit missing-information list instead of filling the pack with guesses.

## Boundaries

- Do not commit raw customer proposal material, private RFPs, access-controlled source text, pricing strategy, private win themes, customer names, transcripts, or local-only access material.
- Do not claim the pack is compliant, legally approved, procurement approved, secure for regulated data, or a replacement for compliance review.
- Do not send, submit, upload, scrape, enrich, update CRM, manage approvals, or automate proposal workflow execution.
- Do not describe MDP as a proposal management platform or automated proposal writer.
- Do not make extracted rules accepted without human review.

## Response

End with:

- pack path and source classification
- files changed
- missing-information list or review packet
- validation and eval result
- representative route tested
- claim-check result for any risky proof or compliance text
- human review items that must be accepted before the pack is usable
