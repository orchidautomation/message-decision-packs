# Product-foundation gap-classification fixtures

These reference fixtures document the three authoring cases the builder must
distinguish when classifying established authority versus genuine unresolved
insufficiency. They are documentation for the contract enforced by the
`skills_approved_boundary_entries_stay_ready_while_genuine_gap_blocks_job` and
`skills_pack_with_every_job_blocked_cannot_be_called_ready` CLI tests in
`cli/src/commands/skills.rs`, which build a synthetic initialized GTM pack and
mutate it.

- **Approved bounded policy** (e.g. approved terminology, case-led proof with a
  no-extrapolation rule) is authored as `entries` plus explicit `avoid`/`output`
  guardrails. The selected facet has no `gaps` and resolves `ready`.
- **Partial but usable authority** with explicit avoid/output rules is authored
  as `entries` in a guardrail facet. The facet has no `gaps` and resolves
  `ready`.
- **Genuine missing authority** (e.g. no approved source establishes
  portfolio-wide alternatives or outcomes) is authored as `gaps`. The selected
  required facet contains an explicit gap ref and resolves `blocked`, keeping
  the relevant job `pack_ready: false`.

The Rust CLI never infers gap meaning from prose and never auto-closes a gap
from keywords such as "approved" or "resolved"; this classification is owned by
the builder and reviewer skills. An explicit selected required gap remains a
veto at runtime.
