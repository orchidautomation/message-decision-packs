# Proof-Output Drafting

`mdp.proof-output.v0` is the machine source of truth for proof-carrying proposal review text. Use it after approved pack refs and source refs are available. Source audit and source acceptance happen before this step; this guide only helps turn known refs into a valid artifact that must still pass `mdp verify-output`.

The smallest safe workflow is:

```bash
mdp --json validate --dir PACK_ROOT
mdp --json route --entries --dir PACK_ROOT --persona "Proposal Lead" --job "compliance review"
mdp --json author-proof-output --dir PACK_ROOT --draft PACK_ROOT/examples/proof-output-drafts/compliance-row.draft.json --out /tmp/proof-output.json
mdp --json verify-output --dir PACK_ROOT --file /tmp/proof-output.json
```

`author-proof-output` compiles a smaller `mdp.proof-output-draft.v0` file into `mdp.proof-output.v0` by filling the loaded pack identity and joining `segments[].text` into `output.text`. It then runs the same verifier used by `mdp verify-output`, including the embedded full-text `check-claims` safety layer. It writes `--out` only when verification passes. It does not bypass `verify-output`, decide whether sources are approved, or create proposal prose from a blank page.

## Draft Shape

A draft omits brittle outer fields that the CLI can derive:

```json
{
  "contract": "mdp.proof-output-draft.v0",
  "route": {"persona": "Proposal Lead", "job": "compliance review"},
  "output": {"kind": "proposal-review-section", "format": "markdown"},
  "segments": []
}
```

Use `mdp --json schema proof-output-draft` for the complete draft schema and `mdp --json schema proof-output` for the verified artifact schema.

## Segment Kinds

| Segment kind | Use for | Binding rule |
| --- | --- | --- |
| `claim` | Material proof, capability, outcome, delivery, compliance-sensitive, customer-proof, or win-theme language. | Must bind to a real claims/evidence-proof card entry. When that entry has source evidence, include a matching `source` ref. |
| `requirement_status` | Requirement coverage, compliance-matrix status, response obligation, or evaluator-need status. | Must bind to a requirement or source-signal card entry. Use gaps when status is missing or only partial. |
| `template_text` | Output labels or boilerplate controlled by an output contract, such as `Requirement status:` or `Proof:`. | Must use a `renders` ref to an output-contract card entry such as `review-outputs:compliance-matrix`. |
| `gap` | Missing proof, missing source text, missing requirement clarity, weak evidence, or reviewer decision needed. | Must include `gap.code`, `gap.reason`, and a constraining card entry or source ref. Do not rewrite gaps into confident claims. |
| `connective` / `formatting` | Low-risk glue, whitespace, punctuation, or headings with no material facts. | Set `material: false`. Keep it short and free of claim, compliance, proof, customer, metric, or approval language. |

Every character in `output.text` must come from an ordered segment. Whitespace between material statements is either part of a material segment or its own `connective`/`formatting` segment.

## Proposal Template Examples

The proposal starter includes synthetic examples:

- `examples/proof-output-drafts/compliance-row.draft.json`: a draft containing template, requirement status, claim, connective, and gap segments.
- `examples/proof-output/compliance-row.json`: the compiled proof-output artifact; it should pass `mdp verify-output`.
- `examples/proof-output-drafts/missing-proof-gap.draft.json`: a missing-proof draft that keeps unsupported certification proof as an explicit gap.
- `examples/proof-output/missing-proof-gap.json`: the compiled missing-proof/gap artifact; it should pass `mdp verify-output` because the missing proof is not presented as a claim.

Run:

```bash
mdp --json author-proof-output --dir plugin/assets/templates/proposal --draft plugin/assets/templates/proposal/examples/proof-output-drafts/compliance-row.draft.json --out /tmp/compliance-row.json
mdp --json verify-output --dir plugin/assets/templates/proposal --file /tmp/compliance-row.json
mdp verify-output --readable --dir plugin/assets/templates/proposal --file /tmp/compliance-row.json
```

## Safety Boundaries

- Do not use this helper before the pack/source refs are reviewed and approved.
- Do not invent card IDs, source IDs, certifications, compliance status, customer outcomes, pricing, deadlines, past performance, or approvals.
- Do not use `connective` for material language. Reclassify material text as `claim`, `requirement_status`, `template_text`, or `gap`.
- Do not treat a source ID written by a model as proof until `mdp verify-output` resolves it.
- Keep `mdp check-claims` as the second safety layer. `verify-output` runs it against `output.text`; run `check-claims` again on any later adapted prose before reuse.
- Do not build or imply a proposal editor, proposal platform, automated submission workflow, approval workflow, or blank-page proposal generator.
