---
name: mdp-output-rules
description: Use to codify MDP output-rule cards for global style, punctuation, formatting, word counts, subject constraints, question limits, link/html/tracking restrictions, paragraph counts, structure constraints, and no-meta-commentary rules.
---

# MDP Output Rules

## Profile Gate

Before using this skill against an existing pack, run:

```bash
mdp --json agent-surface --dir .
```

Use this skill only when the surface is compatibility/generic or this skill is listed in `recommended_skills` or `allowed_skills` and is not listed in `blocked_skills`. If the surface blocks this skill, stop and reroute to an allowed or recommended skill named by the surface before editing or reviewing pack content.

Create output rules that constrain generated text across routed copy and brief work. Use this for rules like no em dashes, word-count ranges, subject length, exact paragraph counts, max questions, forbidden links/html/tracking, format requirements, or no explanatory commentary.

## Workflow

1. Validate the current pack.
2. Review current copy patterns, channel policies, CTAs, avoid-rules, and any user-provided style guidance.
3. Add explicit entries to `.mdp/cards/output-rules.yaml`.
4. Put forbidden punctuation, phrases, or formats in `avoid` so `mdp check-claims` can flag literal hits.
5. Put draft-text deterministic limits in entry `constraints`: `word_count`, `subject_words`, `subject_avoid`, `max_questions`, `forbid_links`, `forbid_attachments`, `forbid_images`, `forbid_html`, and `forbid_tracking`.
6. Put structured proof-output limits in `constraints.proof_output` only when the generated artifact is `mdp.proof-output.v0`: `required_segment_kinds`, `min_segments`, `require_source_refs_for_claims`, and `max_connective_words`.
7. Put structural requirements in the entry body when they need human interpretation. For exact paragraph counts, set `exact_paragraphs` on the entry so `mdp check-claims` can enforce it.
8. Validate the pack again.

## Output Rule Categories

Cover the categories that apply:

- punctuation bans, such as no em dashes
- body word count min/max and target ranges using `constraints.word_count`
- subject word count and blocked subject literals using `constraints.subject_words` and `constraints.subject_avoid`
- max question counts using `constraints.max_questions`
- no links, attachments, images, HTML, or tracking using `constraints.forbid_*`
- proof-output segment coverage, claim source refs, and connective word limits using `constraints.proof_output`
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
- structured `constraints` for deterministic limits when the rule can be checked from draft text or supplied subject
- `exact_paragraphs` when the output must have a fixed paragraph count
- affected personas in `applies_to`
- evidence only when the rule comes from source material rather than user/editorial preference

`constraints.word_count` and `constraints.subject_words` support `min`, `max`, `target_min`, and `target_max`. Min/max violations fail `check-claims`; target misses are reported as `constraint_warnings`. If subject rules exist, pass `--subject`. If `constraints` or `exact_paragraphs` live on channel-policy or CTA entries rather than global output-rules, pass `--persona` and `--job` so `check-claims` can apply only the routed entries. `forbid_attachments`, `forbid_images`, and `forbid_tracking` can detect text references, but `check-claims` also reports `unchecked_constraints` because actual send metadata is outside a single draft body.

`constraints.proof_output` is enforced by `mdp verify-output`, not `check-claims`. It applies to pack-owned card entries selected by the artifact route plus global output-rule cards. Use segment kinds as the machine-readable section contract; do not describe unsupported markdown heading matching or arbitrary prose rules as enforceable Layer 2 constraints.

## Validate

```bash
mdp --json validate --dir .
mdp --json route --entries --dir . --persona "<persona>" --job "<channel> outbound copy"
mdp --json brief --context --dir . --prospect <prospect.json> --channel <channel>
mdp --json check-claims --dir . --text "<draft copy>" --subject "<subject>" --persona "<persona>" --job "<channel> outbound copy"
mdp --json verify-output --dir . --file <proof-output.json>
```

Check that output-rules appear in `required_load_order` and guardrail entries appear in `context.entries` for copy jobs. Use `check-claims` to test blocked literals, hard constraint violations, target warnings, and unchecked metadata caveats.
