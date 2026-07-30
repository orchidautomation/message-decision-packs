# Prospect Fit Or Brief

Read this only for `prospect-fit-or-brief`.

## Prospect Contract

Inspect the current shape when needed:

```bash
mdp --json schema prospect
```

Signals carry observed evidence and provenance. Attributes carry bounded reviewed row metadata. Do not use attributes as invented evidence.

For legacy `mdp.prompt-output.v0` normalization, `source_summary.inputs_used`
should name exact declared inputs such as `raw_row` or
`existing_pack_context`. Field paths, URLs, snippets, and other source locators
belong in `signals[].source` and `normalization_trace`, not in `inputs_used`.

## Workflow

1. Run `mdp --json requirements --dir PACK_ROOT --job prospect-fit-or-brief`.
2. Normalize supplied source material using the pack prompt when present.
3. Branch on `data.available` from requirements:
   - When `true`, instantiate `data.source_attempt_request_schema`. Populate its
     exact `contract`, `job_id`, and `decision_input_contracts` ID/version
     receipts; set a trusted UTC `as_of`; and record at least one attempt for
     every compiled attribute during collection. Preserve those exact request
     bytes and validate the decision-input envelope against that file. Stop
     before extracting `normalized_prospect` unless validation passes and
     top-level `outcome` is exactly `ready`.
   - When `false`, validate the legacy `mdp.prompt-output.v0` output without a
     source-attempt request. Stop before extracting `normalized_prospect` unless
     validation passes and
     `normalization_trace.fit_readiness.ready_for_mdp_fit` is exactly `true`.
4. Run the CLI-owned decision:

```bash
mdp --json fit --dir PACK_ROOT --prospect PROSPECT_JSON
```

5. If the user asked only for fit, return status, matched rules, disqualifiers, qualification gates, missing/invalid requirements, and gaps.
6. If the user asked for a brief and fit permits it, run:

```bash
mdp --json --summary brief --context --dir PACK_ROOT --prospect PROSPECT_JSON --channel CHANNEL
```

Use `--out BRIEF_JSON --dry-run` before a requested durable write. Use `--readable` only when the user wants Markdown.

## Fail Closed

- Insufficient or disqualified means no draft-ready brief.
- Missing person readiness means no invented contact.
- Unknown contract values remain validation issues or gaps; do not silently coerce them.
