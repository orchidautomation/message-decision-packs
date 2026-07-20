# Proof-Output Drafting

Read this when a proposal review needs to produce or repair a `mdp.proof-output.v0` artifact.

Use the repo-level guide `docs/proof-output-drafting.md` as the complete contract reference. Inside a review session, keep this sequence:

1. Confirm source and pack refs are already available and approved for this local pack.
2. Build a `mdp.proof-output-draft.v0` file with `route`, `output.kind`, `output.format`, and ordered `segments`.
3. Compile it:

```bash
mdp --json author-proof-output --dir PACK_ROOT --draft PROOF_OUTPUT_DRAFT_JSON --out PROOF_OUTPUT_JSON
```

4. Re-run the verifier when the proof-output artifact moves or changes:

```bash
mdp --json verify-output --dir PACK_ROOT --file PROOF_OUTPUT_JSON
```

Segment rules:

- `claim`: material proof/capability/outcome/compliance/customer language; bind to claims/evidence-proof and source refs.
- `requirement_status`: requirement or compliance-matrix status; bind to requirement/source-signal refs.
- `template_text`: labels or output structure; bind with `role: renders` to an output-contract entry.
- `gap`: missing proof/source/requirement clarity or reviewer decision; include `gap.code`, `gap.reason`, and constraining refs.
- `connective` or `formatting`: low-risk glue only; set `material: false` and keep it non-material.

Do not invent proof, IDs, requirements, certifications, compliance status, past performance, approvals, or customer outcomes. Missing proof stays a gap. `verify-output` embeds `check-claims` as the second safety layer; run `check-claims` again on any later adapted prose before reuse.
