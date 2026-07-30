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
2. Branch on `data.available` from requirements:
   - When `true`, this skill must not collect missing prospect data or run the
     customer-funded normalization call.
     - If both `SOURCE_ATTEMPT_REQUEST_JSON` and `OUTPUT_JSON` are already
       supplied, validate them immediately with the bound prompt.
     - If either artifact is missing, hand the customer or host the exact
       complete `mdp --json requirements` result as
       `DECISION_INPUT_REQUIREMENTS_JSON`, including
       `data.source_attempt_request_schema`, `data.normalized_output_schema`,
       and all contract/prompt receipts, plus the bound prompt; then stop.
       Require the host to instantiate the request, populate its exact
       `contract`, `job_id`, and `decision_input_contracts` ID/version receipts;
       set a trusted UTC `as_of`; record at least one attempt for every compiled
       attribute during collection; preserve and hash those exact request
       bytes; and normalize that exact request with the bound pack prompt. The
       host must pass `DECISION_INPUT_REQUIREMENTS_JSON.data` as the prompt's
       `decision_input_requirements` input. Resume only when the host returns
       both the preserved request file and normalized output.
     - For either the already-supplied or resumed path, validate the envelope
       against the exact request file. Stop before extracting
       `normalized_prospect` unless validation passes and top-level `outcome`
       is exactly `ready`.
   - When `false`, normalize supplied source material with the selected legacy
     pack prompt, then validate the `mdp.prompt-output.v0` output without a
     source-attempt request. Stop before extracting `normalized_prospect`
     unless validation passes and
     `normalization_trace.fit_readiness.ready_for_mdp_fit` is exactly `true`.
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
