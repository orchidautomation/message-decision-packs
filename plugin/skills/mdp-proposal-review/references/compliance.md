# Compliance Review

Read this only for `compliance-review`.

Review supplied requirements and answers for coverage; never certify compliance or infer that missing requirements are satisfied.

Return:

- `review_status`: `ready-for-human-review`, `needs-more-info`, or `blocked`
- `review_owner`, `scope_reviewed`, and `source_notes`
- `requirements`: objects containing ID, requirement, source, obligation, supplied answer, coverage status, evidence, gap, risk, and owner/question
- `unsupported_or_risky_claims`
- `missing_requirements_or_sources`
- `human_review_required` and `next_questions`
- claim/proof/readable verification results when run

Use coverage values such as supported, partial, missing, unsupported, out-of-scope, or needs-human-review. Never claim or certify regulatory, security, privacy, accessibility, CMMC, NIST, or CUI status. Attribute supplied source statements as unverified inputs, surface gaps or contradictions, and require the responsible human reviewer.
