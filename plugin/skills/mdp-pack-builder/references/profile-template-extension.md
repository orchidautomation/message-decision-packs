# Profile and template extension boundary

This reference is intentionally self-contained. `mdp-pack-builder` authors
packs inside an existing registered profile; it must not register a new
profile, template, or skill as part of ordinary `.mdp/` editing.

- A **profile** owns vocabulary mappings, input contracts/adapter, jobs, eval
  categories, packaged skill routes, and one template association.
- A **template** is an authored starter tree for one registered profile. It
  owns starter metadata, required directories, examples, and bounded
  post-processing. A template is not a profile.
- A **skill** is authored agent guidance for an operator or profile job. The
  closed packaged skill registry determines routing.
- A **host** owns connectors, credentials, model/provider execution,
  sequencing, and external side effects. It does not add MDP primitives or
  promote compatibility evidence.

If the user asks for a new shipped profile or template, stop pack mutation and
hand the request to repository maintainers. Reviewed source work must update
the closed profile/template registries, authored assets, required skill/job
routes, capabilities/help parity, conformance, packaging, and exact-head CI.
Do not invent a third profile, dynamic plugin, provider call, orchestration, or
side-effect workflow. Existing compatibility names remain bounded: proposal
`normalized_prospect` is retained (with `normalized_opportunity` only as its
exact alias), route-budget `job` remains an alias of `job_id`, and proposal v0
runner/MCP paths remain compatibility-only.
