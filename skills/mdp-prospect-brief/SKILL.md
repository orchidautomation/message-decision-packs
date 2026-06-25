---
name: mdp-prospect-brief
description: Use to convert an existing prospect row into local MDP prospect JSON, run `mdp fit` and `mdp brief`, and produce an agent-readable decision contract. Does not enrich or send.
---

# MDP Prospect Brief

Turn an existing prospect row into a clean MDP brief. This skill prepares context; it does not enrich, update systems, or send messages.

## Prospect Shape

Check the schema:

```bash
mdp --json schema prospect
```

Minimum fields:

- `name`
- `title`
- `company`

Preferred fields:

- `linkedin_url`
- `company_url`
- `background`
- `trigger`
- `persona`
- `segment`
- `signals` with source, confidence, freshness, and state_as when available
- `source_kind` and `synthetic` when the row is a generated or sanitized example

## Workflow

1. Normalize the source row into a small JSON file under a repo-ignored agent artifacts directory or another ignored scratch path.
2. Keep only useful, non-sensitive fields needed for routing and copy; redact private data before committing any example.
3. Do not treat LinkedIn URL presence as proof of any claim.
4. Mark fictional starter/demo rows with `source_kind: synthetic-example` and `synthetic: true`; do not present them as real prospects.
5. Check fit before drafting:

```bash
mdp --json fit --dir . --prospect <prospect.json>
```

6. If status is `disqualified` or `insufficient-context`, stop before drafting unless the user explicitly overrides.
7. Run the brief:

```bash
mdp --json --summary brief --context --dir . --prospect <prospect.json> --channel <channel>
```

Use `--out .mdp/briefs/<brief-name>.json` when the user expects a durable created brief file. Without `--out`, the brief is stdout-only.

8. Read `data.context.entries` first if drafting is requested and `data.draft_status` is `ready`. Open `data.context.full_card_required` paths only when present.

## Response

Return:

- normalized prospect fields
- inferred persona
- fit status and disqualifiers
- required card load order
- whether the brief was saved or stdout-only
- decision trace
- gaps or assumptions

Do not send, sequence, enrich, or update CRM from this skill. If the user wants execution, produce an explicit handoff for a separate exact-action tool outside MDP.
