---
name: mdp-pack-review
description: "Use when auditing, validating, hardening, testing, or diagnosing an existing Message Decision Pack itself: structure, evidence, jobs, routes, prompts, gaps, rules, or evals. Do not use to review a prospect, copy draft, proposal, or RFP."
---

# MDP Pack Review

Review an existing pack and produce evidence-backed findings. Do not silently repair it unless the user also asks for changes.

## Gate

Identify the exact pack root and inspect its policy state:

```bash
mdp --json capabilities
mdp --json skills --dir PACK_ROOT
mdp --json doctor --dir PACK_ROOT
```

This is a shared skill and remains eligible for invalid packs so it can diagnose them. Host discovery remains unobserved and host-managed.

## Deterministic Review

Run the narrow checks first, then strict gates:

```bash
mdp --json validate --dir PACK_ROOT
mdp --json gaps --dir PACK_ROOT
mdp --json eval --dir PACK_ROOT
mdp --json requirements --dir PACK_ROOT --job JOB_ID
mdp --json validate --strict --dir PACK_ROOT
mdp --json eval --strict --dir PACK_ROOT
```

Run `requirements` for each job that binds a decision-input contract. A legacy
job may report the contract as unavailable without becoming invalid.

For every available result, inspect `runtime_contract_version`, the public
`contract_version_matrix`, and compiled signal projections before reviewing
artifacts. Reject mixed v1/v2 chains. Structured repeated observations belong
only in the v2 normalized envelope. Confirm roles are explicit pack authority,
not inferred from titles, IDs, provider fields, attributes, or source prose.

Also run `skills --job` and `requirements --job` for every exact canonical job
that declares or should declare a product-foundation binding. Inspect the
CLI-resolved foundation before `.mdp/README.md`; the README is secondary
navigation and cannot supply authority.

For every job that claims self-standing generation or review, require exactly
one `model_task` binding to a matching, versioned `mdp.prompt.v1` prompt. Check
the compiled prompt ID/version/hash, declared input producers, selected
product foundation, exact governed-artifact schema, valid/refusal/gap fixtures,
and separate claim/proof verification for generated prose. Legacy unbound jobs
may remain valid only as `unassessed`; do not call them self-standing.

When a source binding is supplied, also run:

```bash
mdp --json validate-source-binding --dir PACK_ROOT \
  --job JOB_ID --file SOURCE_BINDING_JSON
```

Treat stale pack/requirements pins, missing or duplicate qualified attributes,
unknown attributes, requirement-class drift, incompatible source classes, or
non-fixed status translation as blocking integration findings. External field
keys may repeat. Review the integration release receipts, but do not claim that
schema validation proves source access, provider execution, or normalization.

For a v2 chain, validate the exact source binding, request, collected results,
prompt, and normalized envelope together. Then exercise `fit` or `brief` with
`--normalized-input` and the same lineage artifacts. Compare accepted/rejected
projection IDs, roles, authority classes, conflicts, and diagnostics across
JSON and human output. Detached `--prospect` input must remain legacy or
unassessed. `lineage-validated` may claim internal consistency only, never host
authenticity, authorization, non-repudiation, or observation truth.

Preview a portable compilation when needed:

```bash
mdp --json pack --dir PACK_ROOT --out PACK_JSON --dry-run
```

Read [references/structural-audit.md](references/structural-audit.md) for manifest, primitive, evidence, and content review. Read [references/routing-evals.md](references/routing-evals.md) for job binding, route, prompt, and eval review. Read [references/installed-template-qa.md](references/installed-template-qa.md) only when testing a released install or freshly initialized templates.

## Review Rules

- Treat CLI errors as findings, not prose to reinterpret away.
- Verify every agent-routable `jobs[]` entry has one canonical `skill_id` and a supported closed pair.
- Audit `profile.product_foundation.facets` as indexes over exact existing card
  entries and gap entries, never copied product prose. Reject an eleventh
  primitive, a new product `CardKind`, or a company-wiki registry/README.
- For each exact canonical job ID, compare `skills --job`, `requirements --job`,
  route/context/brief load order, and activation. Required and triggered
  conditional facets must agree; optional, excluded, unrelated-job, and false
  conditional content must not leak into selected context.
- Treat selected empty facets, explicit gaps, dangling refs, and explicit
  selected-facet conflicts as blocking. Do not infer prose conflicts or choose
  a precedence winner. Conditions may only compare `manifest_id`, `profile_id`,
  or `job_id` for exact equality.
- Require status semantics to remain exact: legacy/unbound is `unassessed`,
  complete selected authority is `ready`, and selected insufficiency is
  `blocked`. Foundation readiness only vetoes broader readiness; it never
  establishes sufficient-for-job or self-standing status, and explicit
  `needs-review`/`blocked` activation still vetoes.
- Verify target and proposal gaps remain explicit. Never invent product facts,
  ICP detail, proof, certifications, compliance status, RFP requirements,
  pricing, past performance, or approval to clear a finding.
- Check source receipts, freshness, confidence, approved claims/proof, avoid rules, output rules, and gaps for internal consistency.
- For each decision-input contract, verify that every attribute states an answerable question, requirement class, output path, value contract, decision effects, allowed source classes, provenance, confidence, freshness, sensitivity, and effective behavior for all five attempt statuses. Hard gates must map every status explicitly and include no-draft behavior.
- Verify that required and hard-gate output paths agree with `lead_input_requirements`, that conditional dependencies resolve, and that the compiled source request attempts every declared attribute.
- Verify normalization prompt identity/version, the normalized JSON envelope, explicit `draft_allowed: false`, and synthetic coverage for ready, insufficient-context, disqualified, human-review, malformed, and provider-error.
- Require conservative signal conflict behavior: agreement may coalesce while
  keeping every receipt; `require-agreement` stops human-review/no-draft and
  `any-disqualifies` may only disqualify. Reject positive winner selection.
- Check engine-owned signal resource limits and egress rules: bounded artifacts
  and diagnostics, safe field allowlists, control-character rejection,
  renderer escaping, opaque locator non-dereference, and no raw provider
  records.
- When `manifest.target` exists, verify target kind/name, source IDs, aliases, supported external terms, exclusions, and internal vocabulary boundaries. Treat target contamination as a high-severity wrong-product risk.
- Distinguish structural validity from commercial readiness or human approval.
- Sample representative routes and deterministic claim/output gates when the pack changed those decisions.
- Exercise generated surfaces such as sample leads, prompt output, JSON/readable briefs, run receipts, and eval payloads; required contracts and CLI receipts are implementation metadata, while their prospect-facing content must remain target-aware or neutral.
- For proposal/document normalization QA, require `mdp run-receipt` before calling the flow audit-grade; paid-pilot QA should include `--runner-audit ... --require-runner-audit`. Prefer the host-neutral local proposal runner when available (`scripts/mdp-proposal-runner.mjs` in the source repo or `${PLUGIN_ROOT}/scripts/mdp-proposal-runner.mjs` in an installed bundle) because it exercises source staging, native/headless request construction, validation, receipt, and review probes together. For MCP-capable hosts, the bundled local stdio MCP wrapper is `scripts/mdp-proposal-mcp-server.mjs` or `${PLUGIN_ROOT}/scripts/mdp-proposal-mcp-server.mjs`; it exposes `mdp_proposal_tools` and file/path-only `mdp_proposal_run`. It is not a hosted or remote MCP service, and MCP transport alone is not audit-grade. A lower-level native API runner audit (`scripts/mdp-native-normalize-openai.mjs`) or a hardened headless runner audit is also acceptable when it produces a valid `mdp.runner-audit.v0`. Ambient, unknown, dry-run/mock, demo/fixture runner-audit, synthetic runner model, missing runner-audit, or invalid runner-audit context isolation is an advisory/blocking finding even when prompt-output validation passes.
- When QA asks whether a live proposal review is audit-grade, do not answer from
  pack validity or MCP availability. Route the live evidence decision to
  `$mdp-proposal-review`; this skill reports whether the pack and its fixtures
  can support the path, not whether an invocation crossed it. For MCP fixture
  QA, assert the strict `mode`, `decision`, `audit_grade_eligible`,
  `runner_assurance`, timeout, and exit fields rather than parsing response
  prose.
- Keep evaluation output and temporary packs outside committed source paths.

Report integration support separately from the current receipt. Use only the canonical `verified`, `recipe-only`, `unsupported`, or `fixture/mock-only` state from [canonical runner support matrix](https://github.com/orchidautomation/message-decision-packs/blob/main/docs/headless-normalization-runners.md#canonical-runner-support-matrix); do not promote a runner based on its identifier, recipe, schema acceptance, MCP availability, or one accepted receipt.

## Findings

Report findings first, ordered by severity:

- High: invalid pack, unsafe boundary, unsupported proof, broken job binding, or route/eval behavior that can produce a wrong decision.
- Medium: ambiguous decision context, weak evidence, missing high-value fixture, or output rule that is not enforceable as written.
- Low: clarity, duplication, naming, or maintainability issue with limited behavior risk.

For each finding include the file or CLI path, evidence, impact, and smallest durable fix. If there are no findings, say so and list the commands and coverage limits.

## Boundaries

Pack review does not enrich prospects, review supplied copy/proposals as business artifacts, certify compliance, submit work, or mutate downstream systems. Route those user jobs to the appropriate specialized skill.
