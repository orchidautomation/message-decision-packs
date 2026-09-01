# Proof Review

Read this only for `proof-review`.

Test proposed win themes and claims against approved proof, evaluator needs, and pack boundaries. Proposed themes are not approved claims.

Return:

- `review_status`: `ready-for-draft`, `needs-more-proof`, or `blocked`
- `review_owner`, `scope_reviewed`, and `source_notes`
- `themes`: theme, evaluator need, status, approved claims, supporting proof, missing proof, risk, next question, and recommended action
- `unsupported_or_risky_claims`, `missing_proof`, and `sme_questions`
- claim/proof/readable verification results when run

For `ready-for-draft`, give only supported claim language and its limits. For `needs-more-proof`, ask for the smallest proof or SME input. For `blocked`, name the unsupported claim, missing source, or boundary conflict. Never invent metrics, named references, certifications, customer outcomes, implementation history, evaluator priorities, or past performance.
