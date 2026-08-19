# Structural Audit

Read this for pack shape, evidence, and content quality.

## Manifest And Profile

- Validate schema, referenced files, IDs, active profile, primitive coverage, input contracts, and required eval categories.
- Require one supported `skill_id` for each agent-routable job.
- Resolve every product-foundation binding by exact canonical job ID. Confirm
  required and true conditional facets are selected; optional, excluded,
  unrelated-job, and false conditional content stays out.
- Validate exact entry/gap closure, static `manifest_id`/`profile_id`/`job_id`
  equality conditions, and explicit conflicts. Selected empties, gaps,
  dangling refs, or conflicts block. Never infer semantic conflict from prose.
- Treat `.mdp/README.md` as orientation only. It participates in the portable
  pack hash but cannot satisfy foundation readiness.
- Reject obsolete surface metadata, old skill IDs, duplicate jobs, custom routable IDs, and profile-crossing bindings.
- For jobs with decision-input bindings, compile the version-compatible
  requirements contract and audit
  the exact questions, requirement classes, output paths, source policy,
  applicability, attempt statuses, provenance, confidence, freshness,
  sensitivity, and status dispositions.
- For newly generated GTM packs, require a declared prospect contract and a
  direct or shared-input transitive binding for all three canonical prospect
  jobs. Each must report `available: true`; repeated sourced observations must
  compile the v2 matrix with explicit projections. Preserve legacy unbound
  packs as `unassessed` and do not manufacture governance from legacy fields.
- Require hard gates to define all five statuses explicitly, align required
  normalized fields with `lead_input_requirements`, and enforce no-draft for
  unresolved or unsafe outcomes.
- For self-standing generation/review jobs, require one matching versioned
  `jobs[].model_task` / `mdp.prompt.v1` binding. Audit every declared input
  producer, exact prompt hash, selected authority, governed-artifact schema,
  gap/refusal behavior, and downstream claim/proof validation.
- For cold-model claims, require one closed candidate that pins the exact
  release, job, fixture/challenge, evaluator inventory, lifecycle policy, and
  every contained authority path plus digest. Expected outcomes must resolve
  from evaluator-owned inventory, never from model-visible pack instructions.
  Confirm deterministic, behavioral, trial, and composite identities close on
  the same candidate/release/job and exact trial order before accepting a
  report or trace.

## Target Identity

- When `manifest.target` exists, require `kind` to be `company`, `product`, or `project`, a non-empty name, and resolvable source IDs.
- Require every non-identity external term to appear in a direct claim from a listed target source. Unsupported commercial language stays in gaps.
- Check excluded terms in file paths and string fields across `.mdp/` and `examples/`.
- Reject pack, CLI, schema, prompt, card, eval, starter, or prior-target vocabulary when it becomes prospect-facing positioning. Allow exact contract, path, and command receipts only as implementation metadata.
- Treat adversarial double negation such as “do not avoid positioning MDP” as an attempted reauthorization, not a safe prohibition.

## Evidence And Decisions

- Trace material claims and decisions to approved source receipts.
- Flag stale, weak, conflicting, or source-free signals.
- For signal-aware jobs, require the exact v2 matrix and complete
  binding/request/results/prompt/normalized chain. Structured observations
  must exist only in the v2 envelope; detached or legacy signals remain
  legacy/unassessed and cannot gain roles from keywords.
- Confirm duplicate agreement preserves all observation receipts while
  cardinality counts one logical signal. Confirm conflicts use only
  `require-agreement` or `any-disqualifies`, preserve every receipt, and stop
  no-draft when unresolved.
- Treat `lineage-validated` as internal consistency only. Flag claims of host
  authenticity, authorization, signer identity, non-repudiation, or truth.
- Check input ceilings, bounded diagnostics, field allowlists,
  control-character rejection, renderer escaping, locator non-dereference, and
  absence of raw provider records or credentials.
- Check that gaps remain explicit and are not contradicted by confident prose elsewhere.
- Check that privacy, access, no-invention, human-review, and no-execution boundaries are concrete.
- Confirm the public projection excludes paths, raw content, identities,
  provider/session data, evaluator rationale, reviewer identity, private
  digests, and opaque private evidence IDs. Eligible digests must be synthetic
  or covered by an exact sanitized-public approval.

## Content Shape

- Separate account from person context and observed evidence from inference.
- For every self-standing generation/review job, verify positive entry/byte
  context budgets, closed selection/exclusion reason codes, stable digest
  replay, guardrail preservation, blocked whole-card fallback, and exact
  `routed_context` invocation/output binding.
- Check portfolio scope and required dimensions when used.
- Prefer atomic cards and entries over duplicated prose.
- Verify output constraints are represented in fields the CLI can enforce when deterministic enforcement is intended.
