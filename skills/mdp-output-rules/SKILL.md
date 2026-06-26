---
name: mdp-output-rules
description: Use to codify MDP output-rule cards for global style, punctuation, formatting, paragraph counts, structure constraints, and no-meta-commentary rules.
---

# MDP Output Rules

Create output rules that constrain generated text across routed copy and brief work. Use this for rules like no em dashes, exact paragraph counts, format requirements, or no explanatory commentary.

## Workflow

1. Validate the current pack.
2. Review current copy patterns, channel policies, CTAs, avoid-rules, and any user-provided style guidance.
3. Add explicit entries to `.mdp/cards/output-rules.yaml`.
4. Put forbidden punctuation, phrases, or formats in `avoid` so `mdp check-claims` can flag them.
5. Put structural requirements in the entry body. For exact paragraph counts, set `exact_paragraphs` on the entry so `mdp check-claims` can enforce it.
6. Validate the pack again.

## Output Rule Categories

Cover the categories that apply:

- punctuation bans, such as no em dashes
- exact paragraph requirements using `exact_paragraphs`
- sentence or bullet-count requirements described in the body
- channel formatting constraints
- plain-text defaults for outbound copy, including no links, attachments, images, HTML, or tracking parameters unless explicitly allowed
- initial cold email text constraints such as 90-125 word guidance, short non-clickbait subjects, and no fake Re:/Fwd: framing
- no fake personalization when the source context is not present
- no meta commentary, rationale, or drafting notes
- tone/style preferences not tied to claim safety
- required sections or omitted sections

Use `.mdp/cards/avoid-rules.yaml` instead for forbidden claims, compliance-sensitive language, category boundaries, bad-fit personas, and no-send criteria.
Use `.mdp/cards/channel-policies.yaml` for channel/lifecycle policy, `.mdp/cards/ctas.yaml` for ask boundaries, and `.mdp/cards/copy-patterns.yaml` for reusable message structure.

## Entry Requirements

Each output rule should include:

- the rule the generated text must follow
- when it applies
- blocked literals in `avoid` when deterministic checking is possible
- `exact_paragraphs` when the output must have a fixed paragraph count
- affected personas in `applies_to`
- evidence only when the rule comes from source material rather than user/editorial preference

## Validate

```bash
mdp --json validate --dir .
mdp --json route --entries --dir . --persona "<persona>" --job "<channel> outbound copy"
mdp --json brief --context --dir . --prospect <prospect.json> --channel <channel>
mdp --json check-claims --dir . --text "<draft copy>"
```

Check that output-rules appear in `required_load_order` and guardrail entries appear in `context.entries` for copy jobs. Use `check-claims` to test blocked literals such as em dashes.
