# MDP-280 — Extension boundary and exact-commit integration gate

Status: `READY_TO_PIN`

## 1. Context and current behavior

The cumulative architecture through MDP-279 is present on
`0c11babe724a83a32b46a6c091edf3b30ead6dd3`. Repository and Linear inspection
confirm that the implementation now has:

- one typed, ordered ten-primitive core in `cli/src/primitives.rs`;
- a closed profile/job/skill/input-adapter registry for `gtm` and `proposal` in
  `cli/src/skill_catalog.rs`;
- a closed template registry for `gtm` and `proposal`, with build-time authored
  asset inventories and one transactional init pipeline, in
  `cli/src/template_registry.rs` and `cli/src/commands/init.rs`;
- profile-neutral internal decision-input and routing paths with explicit GTM
  and proposal compatibility adapters;
- one named, test-only cross-profile conformance gate and a neutral fixture
  that is excluded from production and distribution surfaces; and
- one cumulative delivery branch and PR, with no merge, release, deployment,
  installation, migration, or third-profile activation authorized.

The public docs already define primitives and profiles, but they do not yet
give one repository-grounded definition of template versus profile, identify
core/profile/template/skill/host ownership in one place, or state the exact
maintainer checklist for adding a reviewed template or profile. Large runtime
docs also preserve necessary GTM-shaped compatibility names without one concise
index explaining which names are compatibility surfaces rather than core
ontology.

## 2. Objective, scope, and decisions

### Objective

Publish one concise extension-boundary document, link it from the canonical
public vocabulary and operator/authoring surfaces, and prove the entire
MDP-273–280 cumulative change on one exact PR head.

### In scope

- Add `docs/extension-boundary.md` as the canonical maintainer-facing ownership
  and extension checklist.
- Add concise definitions of **template**, **skill**, and **host** beside the
  existing primitive/profile terms in `CONCEPTS.md`.
- Link and summarize the extension boundary in `README.md`, `docs/README.md`,
  `docs/pack-authoring.md`, and `cli/USAGE.md`.
- Add one direct reference from the authored `mdp-pack-builder` skill so a
  repository maintainer or pack author does not mistake ordinary pack editing
  for registration of a shipped template/profile.
- Keep compatibility guidance explicit for `normalized_prospect`, the
  deprecated route-budget `job` alias, the proposal v0 runner/MCP, and current
  GTM-shaped file/example names.
- Run focused public-doc/skill/package checks locally and use exact-head GitHub
  CI, including authority mutation shards, as the final cumulative regression
  and integration gate.
- Record public-safe MDP-280 execution evidence and update cumulative PR #236.

### Out of scope

- No new primitive, runtime profile, template, job, skill, CLI command, schema,
  manifest field, adapter, provider, host, or plugin API.
- No Support or Recruiting implementation, activation, example, roadmap claim,
  or packaging entry.
- No removal or semantic upgrade of a legacy/v0 compatibility field.
- No generated host-bundle edits; `plugin/skills/` remains the authored source
  and Pluxx remains the packager.
- No dependency/version bump, release-only change, merge, release, deployment,
  installation, migration, or external-system execution.

### Decisions

1. **A template is not a profile.** A template is an authored starter tree for
   one already registered profile. A profile owns vocabulary, jobs, adapter,
   mappings, eval categories, and one template association.
2. **A pack is not registration.** Editing or creating a `.mdp/` pack may use a
   shipped profile but cannot register a new runtime profile/template/skill.
3. **Extension is reviewed source work.** A new shipped template/profile is a
   repository change across closed registries, authored assets, skills where a
   job route requires one, conformance, packaging, docs, and exact-head CI.
4. **Hosts remain outside authority.** Hosts own connectors, model/provider
   execution, sequencing, credentials, and side effects; they do not extend the
   primitive vocabulary or silently promote compatibility evidence.
5. **Compatibility names remain labeled.** Existing GTM-shaped wire fields and
   v0 paths stay readable and tested, but are not the recommended neutral
   extension vocabulary.

## 3. Acceptance and evidence map

| Acceptance criterion | Implementation | Proof |
|---|---|---|
| Readers distinguish primitives, profiles, templates, skills, packs, and hosts | Canonical definitions and ownership table in `CONCEPTS.md` and `docs/extension-boundary.md` | Public-artifact lint, link/reference inspection, Sol review |
| Documented extension path matches executable behavior | Checklist names the exact primitive, profile, template, asset, skill/job, capability/help, conformance, packaging, and CI authorities | Compare docs to `primitives.rs`, `skill_catalog.rs`, `template_registry.rs`, capabilities, Make/CI, and Pluxx source boundary |
| Existing GTM/proposal paths remain documented and compatible | Preserve current examples and add a bounded compatibility table without changing commands/contracts | Full exact-head CLI CI, conformance, native/MCP, template/asset/version/install checks |
| No third profile or private strategy leaks into public artifacts | State only the generic reviewed-extension boundary; identify neutral fixture as non-shipping test evidence | Public-artifact lint, packaged skill/plugin checks, release workflow contract, diff review |
| One cumulative PR is review-ready | Push MDP-280 head into `codex/mdp-273-primitive-contracts`, validate exact head, update PR #236 through the requested GitHub connector | Remote-ref equality, GitHub check readback, verified PR metadata/readback, Orchid Verify/Review receipts |

## 4. Owned and forbidden surfaces

### Luna-owned implementation paths

- `CONCEPTS.md`
- `README.md`
- `docs/README.md`
- `docs/extension-boundary.md`
- `docs/pack-authoring.md`
- `cli/USAGE.md`
- `plugin/skills/mdp-pack-builder/SKILL.md`
- `plugin/skills/mdp-pack-builder/references/profile-template-extension.md`

### Sol-owned integration and evidence

- `docs/orchid/plans/2026-08-28-008-mdp-280-extension-boundary-integration-plan.md`
- `docs/orchid/qa/2026-08-28-mdp-280-execution-receipt.json`
- Orchid Work graph, dispatch, verification, review, PR-body validation, branch
  integration, GitHub/Linear readback, and final handoff

### Forbidden

- `cli/src/**`, `cli/tests/**`, `cli/build.rs`, `cli/Cargo.toml`,
  `cli/Cargo.lock`
- `plugin/assets/**`, `assets/**`, generated host bundles, schemas, examples,
  installers, release workflows, release manifests, and version files
- unrelated skills/docs, `AGENTS.md`, private Linear content, customer data,
  releases, deployments, installations, merges, and production systems

If the documented behavior cannot be stated truthfully without changing a
forbidden implementation surface, stop and escalate to Sol instead of widening
scope.

## 5. Ordered implementation sequence

### Step A — Add the canonical extension boundary

Create a concise public document with:

- definitions for core primitive, profile, template, pack, skill, and host;
- an ownership table naming the repository authority for each layer;
- separate checklists for adding a template to an existing profile and adding
  a new reviewed profile;
- the closed-registry rule and fail-closed expectations;
- compatibility names and their current status; and
- a non-goals/safety section that rejects third-profile implication, dynamic
  runtime plugins, provider calls, orchestration, and side-effect authority.

The checklist must match current code: ten fixed primitives; one profile
descriptor with unique jobs, packaged skills, adapter, and template ID; one
template descriptor with authored asset root, metadata, required directories,
examples, and bounded postprocess; build-time asset inventory; capabilities and
help derived from registries; common conformance; Pluxx packaging from
`plugin/`; exact-head CI.

### Step B — Align canonical navigation and terminology

Add `template`, `skill`, and `host` to `CONCEPTS.md` without duplicating the
full guide. Link the new guide from README documentation, the docs index,
pack-authoring guidance, and the CLI usage profile section. Preserve the two
product journeys and all current commands.

### Step C — Align authored skill guidance

Add one direct, one-hop reference from `mdp-pack-builder` explaining that pack
authoring operates inside an existing profile and that shipped profile/template
registration is maintainer work governed by the repository extension checklist.
The reference must remain self-contained because skill reference validation
forbids second-hop local Markdown dependencies.

### Step D — Validate the changed surfaces

Run formatting/diff checks and the existing public artifact, skill contract,
skill packaging, plugin, llms, template/asset/version, installer, and release
workflow contract checks that cover the changed docs and source bundle. Run the
focused cross-profile gate to prove the documented registries still match.

### Step E — Execute the cumulative exact-head gate

Push the issue branch, fast-forward the cumulative branch, and let the existing
GitHub CI execute the full Rust, native/MCP, Pluxx, Eve, authority-mutation, and
aggregate gates on one exact commit. Inspect the cumulative diff for
architecture, compatibility, public safety, security/data-correctness, and
rollback clarity. Repair only actionable in-scope findings, then reissue
head-bound verification/review receipts.

## 6. Compatibility invariants

- Production primitive IDs remain the exact current ten values and order.
- Runtime profile/template IDs remain exactly `gtm` and `proposal`; packaged
  skill IDs and capabilities remain unchanged.
- Existing GTM and proposal commands, JSON fields, assets, digests, route
  decisions, receipts, traces, adapters, evals, and examples do not change.
- `normalized_prospect` remains the compatibility field consumed by existing
  proposal integrations; `normalized_opportunity` remains only its exact alias.
- The route-budget v0 `job` alias remains equal to canonical `job_id`.
- Proposal v0 runner/MCP paths remain compatibility-only and cannot upgrade v1
  authority or assurance.
- The neutral fixture remains test-only and is never documented as a supported
  profile, template, skill, or product capability.
- Host/model/provider execution and all external side effects remain host-owned.

## 7. Validation commands

Run the focused changed-surface proof locally:

```bash
git diff --check
python3 scripts/lint-public-artifacts.py
python3 scripts/test_public_artifact_lint.py
python3 scripts/validate-skill-contracts.py
python3 scripts/test_skill_contracts.py
python3 scripts/validate-skill-packaging.py
make validate-plugin validate-llms
make validate-profile-conformance
make validate-template validate-asset-sync validate-version-sync
make validate-public-artifacts validate-installers
node scripts/test-release-workflow.mjs
```

Then use the required exact-head GitHub checks for the cumulative integration
commit, including `cli`, `mcp-macos`, `pluxx`, `eve-example`, changes,
authority-mutation tool, both authority-mutation shards, and the aggregate
authority-mutations job. Existing repository CI is the full
`make validate`/Rust/compatibility equivalent and is authoritative for the
final pushed head. Do not repeat the same unchanged full suite locally after
that head is green.

## 8. Risks, rollout, and rollback

- **Docs drift from private registries:** name exact code authorities and prove
  them through current conformance/CI rather than promising dynamic extension.
- **Template/profile ambiguity:** define both explicitly and keep ordinary pack
  editing separate from shipped registry changes.
- **Compatibility accidental deprecation:** state legacy fields as retained and
  bounded; do not recommend them for greenfield extension.
- **Private roadmap leakage:** describe only current `gtm`/`proposal` support
  and a generic reviewed checklist; do not name inactive future verticals as
  product plans.
- **Skill reference closure:** keep the new skill reference direct and
  self-contained; validate authored source and packaged plugin contracts.
- **Cumulative validation cost:** use focused local checks, then one exact-head
  GitHub matrix; any repair changes the head and requires fresh proof.

Rollout is documentation plus evidence in cumulative PR #236. Rollback is a
single commit-level reversal of the MDP-280 doc/skill-reference changes; the
underlying architecture commits remain independently revertible inside the
cumulative PR. No data migration or production rollback exists.

## 9. Blockers and readiness verdict

MDP-279's implementation and exact-head CI are complete on `0c11babe...`, so
the code dependency is satisfied even though private lifecycle projection is
stale. MDP-280's repository, branch, owned paths, acceptance criteria,
validation, cumulative PR, rollback, and human-only merge boundary are known.
The issue currently carries conflicting managed risk labels, so Linear
lifecycle transitions must fail closed until that metadata is repaired through
the canonical gateway; this does not authorize manual phase/delegate edits.

The implementation may proceed on `codex/mdp-280-integration-docs`, then
integrate its exact verified head into `codex/mdp-273-primitive-contracts` and
update PR #236 through the user-requested GitHub connector after the body passes
Orchid's deterministic validator. Do not create a second MDP PR, merge, release,
deploy, install, or activate another profile.

Readiness: `READY_TO_PIN`.
