# Routing And Eval Audit

Read this when reviewing jobs, routes, prompts, or fixtures.

## Skill Routes

Use exact job resolution:

```bash
mdp --json skills --dir PACK_ROOT --job JOB_ID
```

Require the expected skill, `pack_ready: true`, and no missing primitives. Test profile-crossing and unknown IDs and confirm they produce no recommendation or fallback.

## Card Routes

Sample representative personas and jobs:

```bash
mdp --json route --entries --eval-fixture --dir PACK_ROOT --persona PERSONA --job JOB
```

Check selected cards, excluded cards, gaps, portfolio scope, and entry-level evidence.

## Prompt And Output Gates

Use `validate-prompt-output` for valid and adversarial normalization results, including `source_audit` fixtures for proposal PDF/doc extraction refs when applicable. Use `check-claims` for supplied claim-bearing text and `verify-output` for proof-carrying artifacts.

## Fixture Quality

Require both successful and failing cases. Include insufficient context, refusal/unsafe output, job routing, unsupported proof, prompt-output invention, and declared profile-specific categories. Prefer distinct scenario families over paraphrases.

For a targeted GTM pack, also require an isolation family:

- create Company A and Company B packs in separate clean roots, with Company A listed as an excluded term for Company B
- confirm Company B validation reports exact paths for intentionally injected Company A residue
- generate target-aware sample leads and JSON/readable briefs, save them under scanned pack paths, and confirm they do not produce contamination findings
- confirm required `mdp.*.vN` contracts, `.mdp/` paths, and `mdp <command>` receipts remain allowed as implementation metadata
- confirm direct or double-negated attempts to sell MDP/internal control-plane language are rejected
