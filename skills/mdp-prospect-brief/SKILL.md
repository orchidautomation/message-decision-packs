---
name: mdp-prospect-brief
description: Use when the user has a Clay, Deepline, CSV, LinkedIn, enrichment, or research row and wants to normalize it into an MDP prospect JSON file, run `mdp brief`, and produce an agent-readable prospect/message decision contract.
---

# MDP Prospect Brief

Turn an enriched prospect row into a clean MDP brief. This skill prepares context; it does not send messages.

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

## Workflow

1. Normalize the source row into a small JSON file under a repo-ignored agent artifacts directory or another ignored scratch path.
2. Keep only useful, non-sensitive fields needed for routing and copy; redact private data before committing any example.
3. Do not treat LinkedIn URL presence as proof of any claim.
4. Check fit before drafting:

```bash
mdp --json fit --dir . --prospect <prospect.json>
```

5. If status is `disqualified` or `insufficient-context`, stop before drafting unless the user explicitly overrides.
6. Run the brief:

```bash
mdp --json brief --dir . --prospect <prospect.json> --channel linkedin
```

7. Read only `data.required_load_order` if drafting is requested and `data.draft_status` is `ready`.

## Response

Return:

- normalized prospect fields
- inferred persona
- fit status and disqualifiers
- required card load order
- decision trace
- gaps or assumptions

Do not send, sequence, enrich, or CRM-update without explicit user approval and a separate tool.
