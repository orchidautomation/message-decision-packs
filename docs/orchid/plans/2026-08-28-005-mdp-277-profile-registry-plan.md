# MDP-277 — Declarative per-profile registry

Status: `READY_TO_PIN`

## 1. Context and authority

The approved architecture decision in
`docs/orchid/decisions/2026-08-28-primitive-core-profile-template-contract.md`
defines a closed declarative profile registry for `gtm` and `proposal`. Profiles
own jobs and domain bindings; the core and skills must not maintain competing
profile/job switch tables. The earlier skill-routing decision keeps exactly five
packaged skills and seven closed profile-scoped job-to-skill pairs.

Repository inspection on cumulative head
`2701f4c417410f1c6e9f6ae5c9b417aa98b6cf96` confirms:

- `cli/src/skill_catalog.rs` exposes five packaged skills, three bootstrap
  skills, and one flat seven-row `JOB_ROUTE_SPECS` table.
- `commands::health::validate_profile_jobs` combines manifest-shape checks with
  global route lookup and scans the flat table to distinguish wrong-profile
  from unknown jobs.
- `commands::skills` iterates and filters that same flat table, then repeats
  packaged-skill eligibility logic.
- `commands::schemas::canonical_job_skill_pairs` independently repeats all
  seven pairs in generated manifest and skills schemas.
- GTM and proposal starter construction owns its job payloads separately, but
  no descriptor binds a registered profile to its jobs, skill, input adapter,
  and template identity in one place.
- MDP-276 introduced the closed `gtm/prospect` and `proposal/opportunity`
  adapter selector. MDP-277 may name those adapter kinds declaratively but must
  not reopen their v0 wire behavior.

Existing JSON schemas, `mdp skills`, health diagnostics, starter manifests,
capabilities, model-step resolution, plugin inventory, and template bytes are
compatibility authority.

## 2. Objective and boundaries

### Objective

Replace the flat global job switch table with one private, immutable registry of
two `ProfileDescriptor` values. Each descriptor declares profile identity,
profile-owned job routes, packaged workflow skill, decision-input adapter, and
template association. All internal route lookup, validation, skills projection,
and schema pair generation consume this registry while public v0/v1 output is
byte/shape compatible.

### In scope

- A private closed registry for `gtm` and `proposal` only.
- Descriptor-owned job route arrays: three GTM jobs and four proposal jobs.
- Descriptor metadata for the existing input adapter and template association.
- Deterministic registry validation for duplicate profile IDs, duplicate jobs,
  cross-profile job reuse, unknown skills, ambiguous adapters, and duplicate
  template associations.
- Registry-backed route/profile lookup, health validation, skills projection,
  and generated schema `oneOf` pairs.
- Parity assertions for capabilities, requirements, model-step resolution,
  starter outputs, and plugin skill packaging where those surfaces consume or
  reflect registry-owned data.

### Out of scope

- Public registry configuration, runtime loading, dynamic libraries, arbitrary
  code hooks, or executable profile plugins.
- Adding Support, Recruiting, a neutral active profile, jobs, skills, template
  IDs, schema versions, or public JSON keys.
- MDP-278's unified template construction/publication pipeline. This issue may
  declare existing template association metadata but does not rewrite init.
- Changing the five packaged skill IDs, seven job IDs, job labels, prompts,
  input contracts, requirements, output bytes, diagnostics, or receipt hashes.
- Merge, release, deployment, installation, or production mutation.

### Decisions

1. `ProfileDescriptor` and `ProfileRegistry` are private Rust data. The registry
   is a compile-time closed slice, not a user-extensible API.
2. A descriptor contains `profile_id`, `jobs`, `input_adapter`, and
   `template_id`. Each `JobDescriptor` contains the existing `job_id` and
   `skill_id`; behavior/prompt closures do not live in the registry.
3. Bootstrap skills remain a global package concern. Profile descriptors own
   only profile-sensitive job bindings.
4. Registry validation is executable and testable against injected descriptor
   slices. Production access first validates the canonical registry or relies
   on a once-initialized validated view; invalid registry state fails closed.
5. A job ID has exactly one profile owner. Reusing it across descriptors is an
   error even when the skill matches. Profile lookup never falls back by job.
6. Adapter values reuse the closed MDP-276 adapter kind. The registry does not
   parse payloads and does not move profile identity into the neutral core.
7. Template associations are declarative evidence for MDP-278. They remain
   `gtm -> gtm` and `proposal -> proposal`; init behavior is unchanged here.
8. Public schema generation may iterate registry routes, but emitted ordering
   must remain GTM's three routes followed by proposal's four routes.

## 3. Acceptance mapping

| Acceptance criterion | Implementation | Proof |
| --- | --- | --- |
| GTM owns three jobs and proposal owns four | Nest route descriptors under the two profile descriptors and remove the flat authoritative table. | Registry unit tests assert exact owners, counts, order, and pairs. |
| Existing skills, requirements, capabilities, schema, activation, and starter outputs remain compatible | Route lookup, skills projection, health validation, and schema pair generation read the registry without changing serializers. | Exact JSON/schema assertions, current command tests, strict starter validation/evals, and template parity. |
| Future profile work is localized | Centralize identity, routes, adapter, and template association behind a descriptor interface. | A synthetic registry unit test proves a well-formed descriptor is validated without edits to consumers; it is never activated or shipped. |
| No dynamic executable plugin model | Keep descriptors compile-time data with enum metadata only. | Source audit and tests reject unknown IDs/skills/adapters; no loading or callback surface exists. |
| Support/Recruiting are not activated | Canonical registry contains exactly `gtm` and `proposal`. | Exact registry inventory and public-output assertions. |

## 4. Implementation sequence and owned files

### Step A — Introduce the closed descriptor registry

Update `cli/src/skill_catalog.rs` (or rename it only if all callers remain
clear) to define:

- `ProfileDescriptor` with profile ID, adapter kind, template ID, and a static
  job slice;
- `JobRouteSpec` nested under its owning descriptor;
- `PROFILE_DESCRIPTORS` containing exactly GTM then proposal;
- `profile_descriptor(profile_id)`, `route_spec(profile_id, job_id)`,
  `job_owner(job_id)`, and ordered route iteration;
- `validate_registry(descriptors)` returning structured private errors for all
  duplicate/unknown/ambiguous cases.

Preserve `PACKAGED_SKILL_IDS` and `BOOTSTRAP_SKILL_IDS` as package inventory.
Do not expose a public registry contract or deserialize descriptors.

Focused tests:

- exact two-profile/seven-route inventory and stable order;
- duplicate profile ID;
- duplicate job within one profile;
- cross-profile job reuse;
- unknown packaged skill;
- missing/ambiguous adapter or duplicate template association;
- unknown profile, unknown job, and wrong-profile job fail closed.

### Step B — Migrate health and skills consumers

Update `cli/src/commands/health.rs` to use registry lookups for canonical pair,
wrong-profile owner, and unknown-route diagnostics. Preserve current diagnostic
codes, paths, severity, and messages.

Update `cli/src/commands/skills.rs` to iterate the selected descriptor's routes
instead of filtering a global table. Eligibility remains derived from manifest
bindings plus packaged inventory. Preserve route order and every public field.

Regression tests cover all seven positive routes, cross-profile rejection,
unknown jobs, wrong skills, malformed manifests, activation vetoes, and
bootstrap eligibility.

### Step C — Generate schema pairs from the same registry

Update `cli/src/commands/schemas.rs::canonical_job_skill_pairs` and canonical
skill enums to project from registry/package inventory rather than repeating
literal pairs. Keep JSON ordering and values identical.

Add an assertion that manifest job schema and skills route schema expose the
same ordered registry pairs and do not admit cross-profile or invented pairs.
Capabilities must continue embedding the exact same schemas and contract IDs.

### Step D — Bind adapter and template metadata without broadening behavior

Use the descriptor's adapter metadata at existing ownership-selection call
sites where it removes another GTM/proposal branch without moving profile IDs
into `DecisionInput`. If doing so would change diagnostics or wire behavior,
keep the existing adapter selector and add a parity assertion instead.

Expose template association only through a private lookup/parity test against
the existing `AVAILABLE_TEMPLATES`/init selection. Do not refactor template
construction; that is MDP-278.

Audit `cli/src/model_steps.rs`, `commands::requirements`, `commands::health`,
`commands::skills`, `commands::schemas`, and `commands::capabilities` for
remaining duplicated job/profile pair tables. Manifest-owned per-job prompt,
primitive, and input-contract data is not duplication and remains in manifests.

### Step E — Compatibility and evidence

Run targeted registry, health, skills, schema, capabilities, requirements,
model-step, starter/init, and MDP-276 adapter tests while implementing. Then run
the full Rust suite, formatting, public-artifact lint, GTM/proposal starter
parity, strict validation/evals on freshly initialized packs, plugin skill
inventory validation, and `git diff --check`.

Record a public-safe exact-head receipt under `docs/orchid/qa/` only after the
implementation and review findings are resolved. Do not include `/tmp` paths,
private Linear prose, tokens, or host state in the committed artifact.

## 5. Compatibility invariants

- Exactly two active profile IDs: `gtm`, `proposal`.
- Exactly five packaged and three bootstrap skills, with current ordering.
- Exactly seven job/skill pairs, with current ordering and ownership.
- `mdp.profile.v0`, `mdp.skills.v1`, manifest schemas, capabilities schemas,
  CLI stdout fields, diagnostics, starter manifests, prompt files, and template
  bytes are unchanged.
- GTM uses the prospect adapter and GTM template; proposal uses the opportunity
  adapter and proposal template.
- Registry lookup requires explicit profile ownership and never infers a
  profile from a job, skill, card, prompt, template file, or payload field.
- Unknown, duplicate, or conflicting registry state fails closed before route
  projection.
- No dynamic profile execution, third shipped profile, new skill, or new job.

## 6. Verification commands

Run from repository root:

```bash
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo test --manifest-path cli/Cargo.toml skill_catalog
cargo test --manifest-path cli/Cargo.toml commands::skills
cargo test --manifest-path cli/Cargo.toml commands::health
cargo test --manifest-path cli/Cargo.toml commands::schemas
cargo test --manifest-path cli/Cargo.toml commands::capabilities
cargo test --manifest-path cli/Cargo.toml commands::requirements
cargo test --manifest-path cli/Cargo.toml model_steps
cargo test --manifest-path cli/Cargo.toml decision_input
cargo test --manifest-path cli/Cargo.toml
npm run test:starter-template-parity
npm run test:proposal-starter-template-parity
npm run test:public-artifact-unit
npm run lint:public-artifacts
git diff --check
```

Use the repository's current init/strict validate/eval commands to create fresh
GTM and proposal packs and require `ok=true`/`valid=true`. Run the existing
plugin skill inventory/semantic eval command named in `package.json`; do not
invent or reinstall host bundles.

## 7. Review and rollback

Review the exact pushed head with Orchid Verify and Orchid Review. Treat any
schema-order drift, diagnostic drift, profile inference, hidden fallback,
duplicate authority table, or template-byte change as blocking.

Rollback is commit-level reversal of the MDP-277 commits on the cumulative
branch. Because this is an internal authority refactor, rollback must restore
the previous flat catalog and consumers together; never partially leave schema
generation or health validation on a different source of truth. No data or
wire migration is required.

## 8. Delivery

Implement on `codex/mdp-277-profile-registry`, then integrate the exact verified
head into cumulative branch `codex/mdp-273-primitive-contracts`. Update only
cumulative PR #236 and private Linear evidence. Do not create another PR, add
`@codex review`, merge, release, deploy, or install.
