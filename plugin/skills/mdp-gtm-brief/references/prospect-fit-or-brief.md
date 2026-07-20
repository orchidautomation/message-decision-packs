# Prospect Fit Or Brief

Read this only for `prospect-fit-or-brief`.

## Prospect Contract

Inspect the current shape when needed:

```bash
mdp --json schema prospect
```

Signals carry observed evidence and provenance. Attributes carry bounded reviewed row metadata. Do not use attributes as invented evidence.

When using a normalization prompt, `source_summary.inputs_used` should name exact declared inputs such as `raw_row` or `existing_pack_context`. Field paths, URLs, snippets, and other source locators belong in `signals[].source` and `normalization_trace`, not in `inputs_used`.

## Workflow

1. Normalize supplied source material into a prospect JSON file using the pack prompt when present.
2. Validate the full prompt output before using its nested prospect object.
3. Run the CLI-owned decision:

```bash
mdp --json fit --dir PACK_ROOT --prospect PROSPECT_JSON
```

4. If the user asked only for fit, return status, matched rules, disqualifiers, qualification gates, missing/invalid requirements, and gaps.
5. If the user asked for a brief and fit permits it, run:

```bash
mdp --json --summary brief --context --dir PACK_ROOT --prospect PROSPECT_JSON --channel CHANNEL
```

Use `--out BRIEF_JSON --dry-run` before a requested durable write. Use `--readable` only when the user wants Markdown.

## Fail Closed

- Insufficient or disqualified means no draft-ready brief.
- Missing person readiness means no invented contact.
- Unknown contract values remain validation issues or gaps; do not silently coerce them.
