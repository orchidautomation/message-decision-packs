---
name: mdp-proposal-review
description: Use when applying an existing proposal MDP to supplied RFP, capture, requirement, proof, matrix, or draft material for bid/no-bid, compliance, proof, or red-team review. Never certify, invent proof, grant final approval, write, or submit proposals.
---

# MDP Proposal Review

Use an approved proposal pack to produce bounded review support for supplied pursuit material.

## Select One Mode

Map explicit user intent to one job ID:

- Bid/no-bid decision support: `bid-no-bid-review`
- Requirement/compliance coverage: `compliance-review`
- Win-theme and proof support: `proof-review`
- Prioritized adversarial gap review: `red-team-review`

Validate the selected route first:

```bash
mdp --json skills --dir PACK_ROOT --job JOB_ID
```

Proceed only when `data.recommendation.skill_id` is `mdp-proposal-review`, the returned `job_id` matches, and `pack_ready` is true. Otherwise report the diagnostics and stop or route pack repair to `$mdp-pack-builder`. There is no fallback job.

## Source And Safety Gate

1. Require the exact pack root, supplied review material, review scope, and known owner.
2. Use only supplied or explicitly approved sources. Keep restricted pursuit material out of public paths and generated fixtures.
3. Never invent RFP text, requirements, deadlines, evaluator criteria, proof, certifications, compliance status, pricing, references, outcomes, past performance, or approvals.
4. Validate pack and gaps:

```bash
mdp --json validate --dir PACK_ROOT
mdp --json gaps --dir PACK_ROOT
```

5. When messy opportunity material uses a pack prompt, validate its complete output before review:

```bash
mdp --json validate-prompt-output --dir PACK_ROOT --prompt-id PROMPT_ID --file OUTPUT_JSON
```

## Review Loop

1. Load only the selected reference:
   - [references/bid-no-bid.md](references/bid-no-bid.md)
   - [references/compliance.md](references/compliance.md)
   - [references/proof.md](references/proof.md)
   - [references/red-team.md](references/red-team.md)
2. Route bounded context using the pack-appropriate persona and review job label when entry-level evidence is needed:

```bash
mdp --json --summary route --entries --dir PACK_ROOT --persona PERSONA --job JOB
```

3. Preserve source locator, freshness, confidence, pack references, gaps, and owner questions.
4. Check any supplied claim-bearing text:

```bash
mdp --json check-claims --dir PACK_ROOT --file REVIEW_TEXT --persona PERSONA --job JOB
```

5. When producing proof-carrying output, prefer the draft helper over hand-writing the final artifact:

```bash
mdp --json author-proof-output --dir PACK_ROOT --draft PROOF_OUTPUT_DRAFT_JSON --out PROOF_OUTPUT_JSON
```

The draft helper only fills pack identity, joins ordered segment text, and runs verification. It does not source-audit, approve proof, or bypass the verifier.

6. Verify any generated proof-carrying artifact before treating its bindings as valid:

```bash
mdp --json verify-output --dir PACK_ROOT --file PROOF_OUTPUT_JSON
```

Use `--readable` only when the user wants the human-readable review artifact. Read [references/proof-output-drafting.md](references/proof-output-drafting.md) before creating or repairing proof-output drafts.

## Boundaries

- Every result is decision or review support, not certification, legal advice, approval, or submission authority.
- Missing evidence produces `needs-more-info`, a gap, or a blocked status—not a plausible assumption.
- Do not update portals, CRM/opportunity systems, messages, approval workflows, or proposal files beyond the review artifact the user requested.
- Do not rewrite a proposal or section unless the user separately asks after the review; revalidate any resulting claims.

## Response

Return the selected mode’s packet, the job route, sources reviewed, CLI checks, unsupported claims, gaps, named human review, and smallest next inputs. State the limits of the review explicitly.
