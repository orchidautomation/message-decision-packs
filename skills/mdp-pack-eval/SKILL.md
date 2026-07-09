---
name: mdp-pack-eval
description: Use when the user wants to test whether a Message Decision Pack routes correctly across sample personas, jobs, channels, prospect rows, or copy tasks. Produces route checks, failure cases, and recommended card metadata fixes.
---

# MDP Pack Eval

## Profile Gate

Before using this skill against an existing pack, run:

```bash
mdp --json agent-surface --dir .
```

Use this skill only when the surface is legacy/generic or this skill is listed in `recommended_skills` or `allowed_skills` and is not listed in `blocked_skills`. If the surface blocks this skill, stop and reroute to an allowed or recommended skill named by the surface before editing or reviewing pack content.

Test whether the pack routes the right cards for realistic GTM tasks.

## Progressive References

Read `references/installed-template-qa.md` when the goal is to QA the installed release artifact, default GTM template, default proposal template, or a clean scratch pack produced by `mdp init`.

## Workflow

1. Validate structure:

```bash
mdp --json validate --dir .
mdp --json eval --dir .
```

Use `--strict` when warnings should fail the agent or CI gate:

```bash
mdp --json validate --strict --dir .
mdp --json eval --strict --dir .
```

`mdp fit`, `mdp check-claims`, and eval fixtures support profile-owned card IDs by falling back to card `kind` when canonical IDs such as `fit-rules`, `claims`, `avoid-rules`, or `output-rules` are absent.

For profile-aware packs, inspect `mdp --json validate --dir .` before and after `mdp eval`. `data.profile.activation_ready` is the activation summary. Missing `required_primitives` coverage or missing `profile_eval.required_categories` is warning-first in normal validation and fails with `--strict`; missing mapped card, prompt, input contract, job, or eval references are errors. Eval fixtures should include `profile_eval.category` for proceed, insufficient-context, refusal, unsafe-output, job-routing, and declared profile-specific categories such as account-context-present, account-context-missing, account-only-no-draft, and prompt-output-validation.

Passing evals only proves the declared fixture assertions. After retargeting personas, routes, profile jobs, or scenario vocabulary, also audit eval filenames, `id`, `persona`, `job`, expected titles, example prospect rows, and prompt `output_contract.example` blocks so stale starter labels do not keep passing with misleading names.

`mdp eval` can run `command: validate-prompt-output` fixtures with `prompt_id` or `prompt` plus inline `prompt_output`. Use this to prove normalization contracts reject invented account/person output before a prospect row reaches `mdp fit` or `mdp brief`.

`mdp eval` can also run `command: verify-output` fixtures with either inline `proof_output` or `proof_output_file`. Use this to prove generated claim-bearing text fails closed when a model invents card/source IDs, omits material bindings, smooths a gap, violates pack-owned `constraints.proof_output`, or includes unsupported full-text claims. Source IDs are not proof until `mdp verify-output` resolves them against the loaded pack.

2. Choose representative cases:

- each primary persona
- LinkedIn initial touch and LinkedIn follow-up
- initial email and email follow-up
- call prep
- source extraction or pack review if relevant
- account context present and missing cases when the profile declares account-context categories
- account-only no-draft behavior when company context exists but person/persona readiness is missing
- prompt-output validation for invented or out-of-contract normalized rows
- proof-output validation for valid bindings, fake IDs, missing bindings, malformed coverage, safe gaps, unsupported generated claims, and any configured Layer 2 proof-output constraints when the pack uses proof-carrying output
- one bad-fit or unsupported persona

3. Run routes:

```bash
mdp --json --summary route --entries --eval-fixture --dir . --persona "<persona>" --job "<job>"
```

4. When outbound-copy testing needs prospect-shaped inputs but no real or sanitized row was supplied, generate fake fixture leads:

```bash
mdp sample-leads --dir . --persona "<persona>" --job "<channel> outbound copy" --count 3 --format yaml
```

Treat these as synthetic examples only. They should have `source_kind: synthetic-example`, `synthetic: true`, and `do_not_contact: true`. For each fixture row, evaluate route, fit, brief context, and `check-claims`; draft only against `safe_personalization` and `known_gaps`, and never treat the fixture as a real prospect.

5. For prospect cases, run:

```bash
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel <channel>
```

## Evaluate

For each case, check:

- expected base cards loaded
- expected bounded context entries returned for prospect briefs
- fit-rules and positioning included when relevant
- claims included for copy jobs that use proof
- avoid-rules included for copy jobs
- output-rules included for copy jobs
- profile-owned card IDs still evaluate through their card kinds when running fit and claim checks
- ctas and channel-policies included for outbound/message/copy jobs
- initial vs follow-up channel-policy entries route separately for email and LinkedIn
- persona-specific cards included
- entry-level route is useful, not noisy
- irrelevant cards omitted
- route does not exceed policy limits
- eval fixtures pass
- profile eval categories cover the declared activation gates
- prompt-output validation fixtures reject invented people, unsupported values, or out-of-contract account context before fit/brief
- proof-output fixtures reject fake source/card IDs and unbound material generated claims before output text is treated as usable
- primitive map references point to existing cards, prompts, input contracts, jobs, and eval fixtures
- decision trace is understandable
- generated eval fixture scaffolds are reviewed before committing so tests encode intended behavior, not accidental routing noise

## Fixes

Recommend metadata edits to manifest card refs first: personas, tags, descriptions, and max route size. Edit card content only when routing exposes a real content gap.
