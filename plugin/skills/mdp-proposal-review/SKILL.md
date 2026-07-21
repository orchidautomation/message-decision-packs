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

For `normalize-opportunity`, keep `normalized_prospect` as the required compatibility object. If `normalized_opportunity` is present, treat it as a proposal-readable alias that must match exactly, not as a separate opportunity schema. `source_summary.inputs_used` names declared prompt inputs only; source locators and proof notes belong in `signals[].source`, provenance, gaps, and normalization trace.

If PDF/doc extraction produced a bounded `mdp.source-audit.v0` ledger, include it:

```bash
mdp --json validate-prompt-output --dir PACK_ROOT --prompt-id PROMPT_ID --file OUTPUT_JSON --source-audit SOURCE_AUDIT_JSON
```

For audit-grade review, require a runner receipt after validation:

```bash
mdp --json run-receipt --dir PACK_ROOT --workflow proposal-review --isolation isolated --declared-inputs-only --prompt-id normalize-opportunity --prompt-output OUTPUT_JSON --validation VALIDATION_JSON --source-audit SOURCE_AUDIT_JSON --runner-audit RUNNER_AUDIT_JSON --require-runner-audit
```

`run-receipt` is audit-grade only when the host runner reports a fresh/stateless model call and declared-input-only payload. It also compares validation-result artifact hashes to the supplied prompt-output and source-audit files and compares the runner-audit `prompt_output_sha256` to the supplied prompt output, so a validation result or runner audit from a different run must block review. Prefer the optional BYOK native API runner (`scripts/mdp-native-normalize-openai.mjs` in source checkouts, `${PLUGIN_ROOT}/scripts/mdp-native-normalize-openai.mjs` in installed Pluxx bundles) when available because it calls the model outside the current chat with Structured Outputs, no tools, no conversation resume, and `store: false`. Do not ask for or create an API key unless the operator explicitly chooses a real native run; installs, dry-runs, mock tests, validation, fit, and receipts without a real model call do not need one. For paid pilots, require `mdp.runner-audit.v0` from a native API runner or a hardened headless runner such as Claude `--bare -p`, Codex `exec`, Cursor `-p` with tools externally denied, or OpenCode `run` with `--pure` and a no-tool agent. If normalization happened in the current conversation, treat the review as advisory even when validation passes. Treat missing source-audit refs, snippet mismatches, missing/invalid runner audit, missing/nonzero tool invocation counts, mismatched validation or runner-audit hashes, or a non-audit-grade receipt as blockers for confident proposal review; keep the issue in gaps or reviewer questions instead of smoothing it into a sourced fact.

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
