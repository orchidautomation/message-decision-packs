# MDP Mental Model

Read this when explaining what belongs in MDP or deciding which layer owns a behavior.

## Responsibility Split

- Product category: versioned decision context for agents. The compatibility
  phrase “decision/context layer” remains valid. “Decision graph” is only the
  JSON or Mermaid visualization of designed policy plus one observed path.
- CLI: validation, closed job-to-skill routing, readiness, fit, routes, briefs, claim/output checks, gaps, and evals.
- Decision trace: a bounded inspection projection of existing CLI or run
  authority. Its designed graph and observed path explain a result without
  executing policy or replacing the source artifact.
- Pack: approved decision context, one `skill_id` per agent-routable job, evidence, boundaries, output contracts, gaps, and fixtures.
- Prompt: a versioned pack-owned normalization, generation, or review contract over declared inputs. `mdp.prompt.v1` names each input producer, the procedure and evidence rules, and an exact structured output schema. The host executes it; MDP compiles and validates it. `source_summary.inputs_used` names declared inputs only; source paths, snippets, page locators, URLs, and proof notes belong in evidence/provenance, `signals[].source`, or normalization trace.
- Governed prompts may declare `mdp.governed-host-envelope.v1`: the model returns `selected_authority`, `artifact`, `gaps`, and `rejected_claims`, while MDP injects and validates deterministic prompt, job, context, receipt, and input-inventory fields.
- Manifest: allowed values, required fields/signals/attributes, profile job
  bindings, pack-owned signal projections and explicit roles, conservative
  conflict policy, and readiness policy.
- Product foundation: a profile/job index over exact existing card entries and
  gaps. It selects required and triggered conditional authority for one
  canonical job; it is not an eleventh primitive, card kind, or company wiki.
- `.mdp/README.md`: human orientation and secondary navigation. Its bytes
  participate in portable pack identity, but its prose has no decision or
  readiness authority.
- Skill: trigger boundary, workflow, mode selection, safety, and command orchestration.
- Agent host: skill discovery and loading.
- External systems: source collection, outreach, CRM, proposal submission, and other side effects.
- Downstream writer/reviewer: wording and human review only after routed context and CLI checks; it does not invent source facts, override validation, or revise the pack-owned policy.

## Universal Primitives

Use `actors`, `decision-criteria`, `source-signals`, `needs-requirements`, `evidence-proof`, `boundaries`, `output-contracts`, `routing-jobs`, `gaps`, and `evals` across profiles. Profile vocabulary maps to these primitives; it does not create a second core schema.

## Failure Discipline

- Fix invalid structure or stop.
- Record missing evidence as a gap.
- Inspect CLI-resolved product foundation before README prose. Preserve
  `unassessed` and `blocked` honestly; `ready` only removes this one veto and
  never establishes sufficient-for-job or self-standing status.
- Reject unsupported job bindings; do not choose a nearby skill.
- State that host discovery is unobserved and host-managed.
- Keep side effects outside MDP.
- Do not describe MDP as a graph database, agent runtime, orchestration
  framework, persistent memory layer, or proof that a source claim is true.
- Keep legacy signals readable but non-authoritative. A v2
  `lineage-validated` result means the host-submitted artifact chain is
  internally consistent; it does not authenticate the host or prove truth.
