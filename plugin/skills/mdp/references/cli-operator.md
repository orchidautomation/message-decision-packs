# CLI Operator Reference

Read this when selecting an MDP command.

## Discovery And Health

```bash
mdp --version
mdp --json skills
mdp --json skills --dir PACK_ROOT
mdp --json doctor --dir PACK_ROOT
```

`skills` reports released inventory and pack eligibility. It does not observe host discovery.

## Contracts And Inspection

```bash
mdp --json schema skills
mdp --json validate --dir PACK_ROOT
mdp --json explain --dir PACK_ROOT
mdp --json gaps --dir PACK_ROOT
mdp --json route --entries --dir PACK_ROOT --persona PERSONA --job JOB
```

Prefer CLI output to direct YAML inference. Read pack files only when authoring or when the CLI identifies the exact card or contract needing review.

## Deterministic Gates

- `validate-prompt-output`: validate model-produced normalization output; pass `--source-audit` for proposal PDF/doc extraction ledgers when raw-field/snippet citations must resolve.
- `run-receipt`: record and gate the host-owned context boundary plus artifact hashes; audit-grade proposal review requires `--isolation isolated`, `--declared-inputs-only`, successful validation, source audit when documents/PDFs were normalized, and for production pilots `--runner-audit ... --require-runner-audit`.
- `scripts/mdp-native-normalize-openai.mjs` (or `${PLUGIN_ROOT}/scripts/mdp-native-normalize-openai.mjs` in installed bundles): optional BYOK reference runner for OpenAI Responses API normalization. Use `--dry-run` or `--mock-response` for offline validation without a key; real native calls require `OPENAI_API_KEY` and still must be followed by `validate-prompt-output` and `run-receipt`.
- `fit`: decide fit, insufficient context, or disqualification for supplied GTM prospect JSON.
- `brief --context`: build bounded GTM decision context after fit permits it.
- `check-claims`: test supplied claim-bearing text and output constraints.
- `author-proof-output`: compile proof-output drafts into verified proof-output JSON; writes only after verifier success.
- `verify-output`: verify proof-carrying output against loaded pack IDs.
- `eval`: run committed pack fixtures.

Do not reproduce these decisions manually in a skill.

## Artifact Writes

Preview commands that support `--dry-run` before writing:

```bash
mdp --json init --template gtm --name PACK_NAME --target-name TARGET_NAME --target-kind company --target-alias TARGET_ALIAS --exclude-term PRIOR_TARGET --dir PACK_ROOT --dry-run
mdp --json brief --context --dir PACK_ROOT --prospect PROSPECT_JSON --out BRIEF_JSON --dry-run
mdp --json emit-brief --dir PACK_ROOT --persona PERSONA --out BRIEF_JSON --dry-run
mdp --json pack --dir PACK_ROOT --out PACK_JSON --dry-run
mdp --json author-proof-output --dir PACK_ROOT --draft PROOF_OUTPUT_DRAFT_JSON --out PROOF_OUTPUT_JSON --dry-run
mdp --json run-receipt --dir PACK_ROOT --workflow proposal-review --isolation isolated --declared-inputs-only --prompt-id normalize-opportunity --prompt-output OUTPUT_JSON --validation VALIDATION_JSON --source-audit SOURCE_AUDIT_JSON --runner-audit RUNNER_AUDIT_JSON --require-runner-audit --out RUN_RECEIPT_JSON --dry-run
```

For a named GTM pack, pass `--target-name` explicitly. Repeat `--target-alias` and `--exclude-term` when needed; never force-retarget an existing pack directory.

Write a durable artifact only when the user asks for one or the task requires a repository change.
