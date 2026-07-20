# MDP Mental Model

Read this when explaining what belongs in MDP or deciding which layer owns a behavior.

## Responsibility Split

- CLI: validation, closed job-to-skill routing, readiness, fit, routes, briefs, claim/output checks, gaps, and evals.
- Pack: approved decision context, one `skill_id` per agent-routable job, evidence, boundaries, output contracts, gaps, and fixtures.
- Prompt: extraction or normalization from supplied inputs into strict prompt-output JSON. `source_summary.inputs_used` names declared inputs only; source paths, snippets, page locators, URLs, and proof notes belong in evidence/provenance, `signals[].source`, or normalization trace.
- Manifest: allowed values, required fields/signals/attributes, profile job bindings, and pack-owned readiness policy.
- Skill: trigger boundary, workflow, mode selection, safety, and command orchestration.
- Agent host: skill discovery and loading.
- External systems: source collection, outreach, CRM, proposal submission, and other side effects.
- Downstream writer/reviewer: wording and human review only after routed context and CLI checks; it does not invent source facts, override validation, or revise the pack-owned policy.

## Universal Primitives

Use `actors`, `decision-criteria`, `source-signals`, `needs-requirements`, `evidence-proof`, `boundaries`, `output-contracts`, `routing-jobs`, `gaps`, and `evals` across profiles. Profile vocabulary maps to these primitives; it does not create a second core schema.

## Failure Discipline

- Fix invalid structure or stop.
- Record missing evidence as a gap.
- Reject unsupported job bindings; do not choose a nearby skill.
- State that host discovery is unobserved and host-managed.
- Keep side effects outside MDP.
