# Red-Team Review

Read this only for `red-team-review`.

Prioritize gaps, contradictions, unsupported claims, missing requirements, and boundary risk in supplied proposal material. Do not rewrite the proposal wholesale or present the result as final red-team authority.

Return:

- `review_status`: `ready-for-human-review`, `needs-more-info`, or `blocked`
- `review_owner`, `scope_reviewed`, and `source_notes`
- `priority_order`
- `gaps`: severity, issue type, issue, affected section, evidence, pack reference, confidence, owner/question, and next action
- `unsupported_or_risky_claims`
- `contradictions_or_boundary_risks`
- `missing_sources_or_requirements`
- `human_review_required`
- claim/proof verification results when run

Use blocker/high/medium/low/watch severity. Do not create automated scores without a supplied customer rubric. Never invent requirements, proof, certifications, customer results, evaluator priorities, scores, or past performance.
