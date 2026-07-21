# Source Intake And Primitive Mapping

Read this when source material must become pack decisions.

## Source Rules

- Prefer user-approved local material and public primary sources.
- Treat model or collection output as untrusted until reviewed.
- Record source class, locator, snippet, date, confidence, and intended primitive.
- For proposal PDF/doc extraction, write a bounded `mdp.source-audit.v0` JSON ledger with refs, `.mdp/sources.yaml` source IDs, locators, and snippets; validate normalize-opportunity output with `--source-audit` before review.
- Exclude secrets, gated pages, contact databases, and unapproved exports.
- Preserve disagreement, staleness, and missing evidence as gaps.
- In prompt outputs, keep source inventory and source locators distinct: `source_summary.inputs_used` is exact declared input names only, while locators/snippets belong in evidence, provenance, `signals[].source`, and normalization trace.
- For targeted GTM packs, cite the target identity in `target.source_ids`. Treat additional `target.external_terms` as supported only when a listed source contains the term in a direct claim; otherwise keep the term as a gap.

## Workflow

1. State the decision the pack must support.
2. Resolve the external target identity and inventory approved sources, aliases, and excluded prior-target or starter terms.
3. Map evidence needs across `actors`, `decision-criteria`, `source-signals`, `needs-requirements`, `evidence-proof`, `boundaries`, `output-contracts`, `routing-jobs`, `gaps`, and `evals`.
4. Extract atomic facts with receipts. Do not blend multiple sources into an unsupported stronger claim.
5. Author reviewed entries and prompts; validate prompt output before accepting normalized values.
6. Keep missing evidence visible and assign an owner and resolution path when known.

## Profile Mapping

GTM actors include accounts, people, personas, and buying roles. Proposal actors include buyers, evaluators, proposal owners, solution owners, and executive reviewers.

GTM decision criteria include fit/disqualification and message readiness. Proposal decision criteria include bid/no-bid gates and review readiness. Both profiles use the same universal primitives.

## Stop Conditions

Stop and ask for direction when the only available source requires unauthorized access, confidentiality classification is unclear, or accepting a source would place restricted material into a public artifact.
