# MDP Mental Model

Read this when explaining what belongs in MDP or deciding which layer owns a behavior.

## Responsibility Split

- CLI: validation, closed job-to-skill routing, readiness, fit, routes, briefs, claim/output checks, gaps, and evals.
- Pack: approved decision context, one `skill_id` per agent-routable job, evidence, boundaries, output contracts, gaps, and fixtures.
- Skill: trigger boundary, workflow, mode selection, safety, and command orchestration.
- Agent host: skill discovery and loading.
- External systems: source collection, outreach, CRM, proposal submission, and other side effects.

## Universal Primitives

Use `actors`, `decision-criteria`, `source-signals`, `needs-requirements`, `evidence-proof`, `boundaries`, `output-contracts`, `routing-jobs`, `gaps`, and `evals` across profiles. Profile vocabulary maps to these primitives; it does not create a second core schema.

## Failure Discipline

- Fix invalid structure or stop.
- Record missing evidence as a gap.
- Reject unsupported job bindings; do not choose a nearby skill.
- State that host discovery is unobserved and host-managed.
- Keep side effects outside MDP.
