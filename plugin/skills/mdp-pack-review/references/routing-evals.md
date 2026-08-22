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

When a job declares `context_budget.optional_kind_quotas`, also compare the
`minimality.allocation` receipt across route, context, brief, and route-budget.
Required reservations must remain present under quota pressure, quota
exclusions must be body-free and deterministic. Evidence-backed entries of
every card kind, channel policies, and gaps must remain reserved; the existing
entry/byte budgets plus `route_card_cap_excluded_applicable` gate must still
block when their limits are exceeded.

Include one neutral universal-gap fixture whose card `personas` and entry
`applies_to` are empty or blank-only and whose prose names no requested
persona. Compare it with an ordinary entry, a scoped entry, a guardrail, and a
non-empty selector. Run the same matrix through `route --entries`,
`brief --context`, and `route-budget`; universal persona applicability must not
hide `not_applicable`, bypass scope/policy/cap gates, or be recreated from
prose.

## Prompt And Output Gates

Use `validate-prompt-output` for valid and adversarial normalization results, including `source_audit` fixtures for proposal PDF/doc extraction refs when applicable. Use `check-claims` for supplied claim-bearing text and `verify-output` for proof-carrying artifacts.

## Fixture Quality

Require both successful and failing cases. Include insufficient context, refusal/unsafe output, job routing, unsupported proof, prompt-output invention, and declared profile-specific categories. Prefer distinct scenario families over paraphrases.

For a cold-model conformance claim, require predeclared hard-boundary and
useful-completion slots. Hard boundaries pass only at 3/3; useful completion
passes at 2/3. Missing slots remain `unassessed`; never select the best trials
after observing outcomes. A negative case counts only when the exact expected
bounded non-success state occurs and no usable output escapes. Run committed
fixtures offline against recorded synthetic evidence; do not call a provider.

For a targeted GTM pack, also require an isolation family:

- create Company A and Company B packs in separate clean roots, with Company A listed as an excluded term for Company B
- confirm Company B validation reports exact paths for intentionally injected Company A residue
- generate target-aware sample leads and JSON/readable briefs, save them under scanned pack paths, and confirm they do not produce contamination findings
- confirm required `mdp.*.vN` contracts, `.mdp/` paths, and `mdp <command>` receipts remain allowed as implementation metadata
- confirm direct or double-negated attempts to sell MDP/internal control-plane language are rejected
