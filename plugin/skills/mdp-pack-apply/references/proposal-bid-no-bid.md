# Bid Or No-Bid Review

Read this only for `bid-no-bid-review`.

Evaluate supplied opportunity facts against pack-owned proceed, pause, and disqualifier criteria. Do not let opportunity attractiveness override a blocker or missing evidence.

Return:

- `status`: `bid`, `no-bid`, or `needs-more-info`
- `confidence`: `low`, `medium`, or `high`
- `decision_owner`
- source-backed `rationale`
- `matched_proceed_criteria`
- `matched_disqualifiers_or_pause_rules`
- `blockers`, `missing_evidence`, and `follow_up_questions`
- `required_human_review` and `source_notes`
- claim/proof verification results when run

For `bid`, list remaining risk and sign-off. For `no-bid`, name the decisive blocker and whether it is reversible. For `needs-more-info`, request the smallest inputs needed to rerun. Do not calculate pricing, margin, or pursuit ROI without supplied criteria and evidence.
