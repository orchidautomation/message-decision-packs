# MDP-279 — Shared cross-profile conformance gate

Status: `READY_TO_PIN`

## 1. Context and current behavior

The primitive-neutral architecture now has one closed ten-primitive vocabulary,
two registered profiles (`gtm` and `proposal`), two profile-neutral decision
input adapters, and one data-first template registry. Repository inspection on
cumulative head `89e8c4a9962f6fd06fbd3f28b5285730b252e4c8` confirms:

- `cli/src/primitives.rs` owns the exact ordered ten `PrimitiveId` values.
- `cli/src/skill_catalog.rs` owns the private closed profile/job/adapter/template
  registry, while `cli/src/template_registry.rs` owns the two shipped template
  descriptors and their embedded authored inventories.
- `cli/src/commands/health.rs::validate_pack`,
  `cli/src/commands/evals.rs::eval_pack`,
  `cli/src/commands/requirements.rs::requirements`, and
  `cli/src/routing.rs::route_budget_preflight` expose reusable crate-private
  validation seams.
- `cli/src/artifact_hash.rs::pack_content_sha256` deterministically binds the
  authored `.mdp` tree while excluding only generated top-level `briefs` and
  `traces` directories.
- `plugin/assets/templates/basic` and `plugin/assets/templates/proposal` both
  declare and map the exact ten primitives, but the repository does not have a
  single named suite that subjects both profiles to the same contract.
- `Makefile` aggregates repository validation through an isolated temporary
  workspace; `.github/workflows/ci.yml` runs the Rust suite but does not expose
  profile conformance as a separately named failure boundary.
- release binaries are built from the non-test Rust target, while Pluxx packages
  `plugin/` assets. A fixture under `cli/tests/fixtures` plus a module compiled
  only under `cfg(test)` is outside both shipped surfaces.

Existing validation, eval, authority monotonicity, clean-run, replay, native,
MCP, asset parity, and packaging checks remain compatibility authority. The
neutral subject permitted by the project decision is test evidence only, not a
runtime profile.

## 2. Objective, scope, and decisions

### Objective

Add one deterministic, test-only conformance harness and one named validation
target that run the same primitive, registry, input, routing, budget, eval,
receipt/trace, and authored-tree checks for both shipped profiles, then prove
the shared contract also accepts a vocabulary-neutral synthetic subject without
registering or packaging a third profile.

### In scope

- A private `cfg(test)` Rust conformance module with one parameterized runner.
- One synthetic neutral fixture under `cli/tests/fixtures/profile-conformance/`.
- Real-profile subjects derived from the existing profile and template
  registries plus the canonical authored template roots.
- Profile-specific, deterministic failure labels and adversarial mutation tests.
- A named `make validate-profile-conformance` target included in `make validate`
  and an explicit CI step.
- Exact checks that the neutral fixture is absent from runtime registries,
  template/capability/skill inventories, embedded template roots, and release
  asset sources.
- Focused compatibility digests for the two canonical authored `.mdp` trees.

### Out of scope

- A public `mdp conformance` command, JSON contract, schema, supported neutral
  profile, template, adapter, skill, capability, installer option, or runtime
  discovery mechanism.
- Changing the ten primitives, existing profiles/jobs/templates, manifest or
  template bytes, eval categories, routing semantics, budgets, receipts, traces,
  clean-run/replay contracts, packaging layout, or version.
- Replacing existing specialized authority, run, native, proposal, MCP, or
  installed-parity suites.
- Merge, release, deployment, installation, or local host-bundle refresh.

### Decisions and assumptions

1. Add `#[cfg(test)] mod profile_conformance;` in `cli/src/main.rs`. The harness
   is a unit-test module so it can use crate-private authorities without widening
   the production API. The one operator entry point is
   `make validate-profile-conformance`; no production CLI surface is added.
2. Add a single JSON fixture at
   `cli/tests/fixtures/profile-conformance/neutral-profile.json`, loaded only by
   `include_str!` inside the `cfg(test)` module. It models a synthetic
   record-triage workflow with all ten primitive IDs and neutral card/input/eval
   terms; it must not use GTM or proposal nouns.
3. Define one test-only `ConformanceSubject` abstraction and one
   `check_subject` runner. Real subjects populate it from parsed manifests,
   `PROFILE_DESCRIPTORS`, template descriptors, actual jobs, input contracts,
   eval output, route-budget output, and pack digests. The neutral subject is
   parsed directly from the fixture and deliberately has no runtime registry,
   adapter, template, or package identity.
4. Every finding is prefixed with the subject ID and stable check ID, for
   example `[gtm:primitive-coverage]` or `[proposal:route-budget]`. The aggregate
   test evaluates both shipped profiles before failing and reports their
   findings independently.
5. The common contract requires: exact ten primitive declarations and map keys;
   non-empty mappings; job/route/input-contract ownership; ready activation;
   passing strict pack validation and evals; required eval-category coverage;
   no route-budget overflow or authority loss; declared output/gap/eval
   mappings; and availability of receipt, trace, clean-run, and replay authority
   contracts. Profile-specific vocabulary and job counts are data, not branches
   in the runner.
6. Pin `pack_content_sha256` results for canonical basic and proposal `.mdp`
   trees in the test-only fixture or test constants. Digest updates are allowed
   only when an intentional authored-tree change lands with an explicit golden
   update; MDP-279 itself must not change either tree.
7. Packaging exclusion is structural and tested: the fixture path is under
   `cli/tests/fixtures`, the module is `cfg(test)`, production profile/template
   registries remain exactly `gtm, proposal`, packaged skill and capability
   inventories contain no neutral ID, and release sources remain production
   binaries plus `plugin/` assets. Do not add the fixture to `plugin/`, `assets/`,
   build-script embedded template roots, or release workflow inventories.

## 3. Acceptance mapping

| Acceptance criterion | Implementation | Proof |
| --- | --- | --- |
| One command/suite proves both shipped profiles satisfy the same core contract | Parameterize one `check_subject` runner over registry-derived GTM and proposal subjects; expose `make validate-profile-conformance`. | Named target runs one aggregate test and emits an independent result/finding prefix for each profile. |
| Neutral fixture fails if the shared core requires GTM/proposal nouns | Run the identical primitive/core portion against the neutral fixture and scan fixture-owned identifiers/values for a closed forbidden-noun list. | Positive neutral test plus mutations injecting a GTM noun and a proposal noun; both fail with `[neutral:vocabulary-isolation]`. |
| Fixture is never packaged or exposed as a supported third profile | Keep it in a test-only path/module and assert exact production profile/template/capability/skill inventories and release-source boundaries. | Registry/package-exclusion test, non-test release build, Pluxx packaging validation, and release workflow contract check. |
| Security/privacy/authority-monotonicity/clean-run/replay stay green | Do not modify those implementations; include their contract availability in the shared harness and run existing authoritative suites. | Full Rust plus authority conformance, run conformance/golden, native parity, proposal/MCP, public-artifact, installer, and repository validation checks. |
| CI reports profile-specific failures clearly | Use stable subject/check prefixes, collect all findings, and add a named CI step for the target. | Mutation tests assert exact prefixes; CI workflow contract test asserts the named step and command remain present. |

## 4. Affected files and ownership

### New test-only surfaces

- `cli/src/profile_conformance.rs`: test-only subject types, fixture parser,
  common checks, real-profile adapters, finding aggregation, golden digests, and
  positive/adversarial tests.
- `cli/tests/fixtures/profile-conformance/neutral-profile.json`: synthetic,
  public-safe neutral data containing the exact ten primitive IDs, one neutral
  job/input contract, route budget, eval categories, and core contract flags.

### Narrow existing changes

- `cli/src/main.rs`: register `profile_conformance` only under `cfg(test)`.
- `Makefile`: add `validate-profile-conformance` to `VALIDATION_TARGETS`, the
  aggregate `validate` dependencies, and the active-workspace recipes.
- `.github/workflows/ci.yml`: add one named `Validate cross-profile
  conformance` step running `make validate-profile-conformance` after Rust tests.
- `scripts/test-release-workflow.mjs`: assert the CI path filter, named step,
  exact make command, and release boundaries needed to keep test-only neutral
  data out of shipped plugin/release inventories.

`cli/src/primitives.rs`, `cli/src/skill_catalog.rs`,
`cli/src/template_registry.rs`, manifest model types, and the reusable command
functions may receive only narrow `pub(crate)` helpers or non-semantic accessors
if the harness cannot consume the current authority directly. Production
registries, `plugin/`, `assets/`, `cli/build.rs`, template assets, schemas,
installers, and release artifact lists are otherwise forbidden.

## 5. Ordered implementation sequence

### Step A — Model the test-only common contract

Create serializable/deserializable test-only subject and finding types. Represent
the exact ordered primitive IDs, primitive mappings, jobs, required primitive
sets, input-contract presence, route budgets, eval categories, output/gap/eval
mapping presence, core receipt/trace/clean-run/replay flags, authored digest,
and a `registered` boolean. Keep subject construction separate from validation
so mutations can alter one property at a time.

Implement `check_subject` with no `gtm`/`proposal` switch. It returns all stable
`[subject:check]` findings rather than stopping at the first failure. It checks
exact primitive coverage/order, complete non-empty mappings, unique owned jobs,
required-primitive subset, input contracts, safe nonzero budgets, required eval
coverage, ready/passing health, no route overflow or authority loss, required
output/gap/eval mappings, and core receipt/trace/clean-run/replay availability.

### Step B — Adapt both shipped profiles from existing authorities

For each profile descriptor, resolve its template descriptor and canonical asset
root, parse the real manifest through existing model/pack helpers, enumerate
declared jobs and input contracts, and call the existing health, eval,
requirements, route-budget, and pack-digest functions. Derive expected jobs and
adapter/template ownership from the registries instead of duplicating profile
tables in the harness.

Create a single aggregate test that always runs both subjects, records a compact
`gtm: PASS` / `proposal: PASS` diagnostic when invoked with `--nocapture`, and
panics only after combining all prefixed findings. Pin both authored digests
after computing them with `pack_content_sha256` at the plan-pinned source head.

### Step C — Add and isolate the neutral fixture

Author one JSON fixture using neutral terms such as record, source, policy,
review, outcome, and follow-up. It carries the exact same ten primitive IDs and
core properties but no runtime adapter/template/skill identity. Load it only in
the test module and run it through the same common checker with registration
checks disabled only where the fixture explicitly declares `registered: false`.

Apply a case-insensitive token scan to fixture-owned strings. Reject GTM/proposal
domain nouns including at minimum `prospect`, `lead`, `cta`, `hook`, `pain`,
`outbound`, `proposal`, `rfp`, `bid`, `compliance`, and `pursuit`. Do not scan
the shared primitive IDs themselves, because those are the architecture under
test rather than profile vocabulary.

### Step D — Prove failure behavior and packaging exclusion

Add table-driven mutations for missing/extra primitive, empty mapping, missing
input contract, cross-owned job, zero or lost budget, absent eval category,
failed activation/eval, missing receipt/trace/clean-run/replay authority,
changed pack digest, false runtime registration, and injected forbidden nouns.
Assert the exact subject/check prefix for each mutation.

Assert that production profile descriptors, template descriptors, supported
capability template IDs, and packaged skill routes remain the exact two shipped
profiles and existing skills; the neutral fixture path/ID appears in none of
them. Extend the release-workflow contract test to preserve the explicit named
CI gate and confirm release publication is limited to production CLI artifacts
and Pluxx-produced plugin assets, not `cli/tests`.

### Step E — Wire one unskippable validation surface

Add `validate-profile-conformance` to both wrapper and active-workspace Make
target lists. Its recipe runs only the conformance module with `--nocapture` so
profile labels are visible. Include it in aggregate `validate` even though
`validate-cli` also runs the Rust test suite; the duplicate focused invocation
is intentional named-gate evidence.

Add `Validate cross-profile conformance` to the CLI CI job after `Test CLI`.
Keep the existing `cli/**`, `Makefile`, and workflow path filters authoritative;
the new fixture automatically falls under `cli/**`.

## 6. Compatibility invariants

- The production primitive vocabulary remains exactly ten values in the current
  order; the harness consumes it and does not introduce an eleventh primitive.
- Runtime profile IDs remain exactly `gtm` and `proposal`; template IDs remain
  exactly `gtm` and `proposal`; packaged skill IDs and capability JSON do not
  change.
- No CLI command, option, output field, contract version, schema, manifest,
  template byte, budget, job, eval category, input adapter, receipt, or trace is
  added or changed.
- The neutral fixture is compiled only into tests and is absent from production
  binaries, plugin assets, Pluxx bundles, installers, and release manifests.
- Canonical `.mdp` digests for both shipped templates remain unchanged at the
  MDP-279 implementation head.
- Existing authority monotonicity, privacy boundaries, clean-run isolation,
  replay determinism, native parity, MCP behavior, and asset parity stay owned
  by their established suites.

## 7. Validation commands

Run focused checks during implementation, then exact-head proof from repository
root:

```bash
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo test --manifest-path cli/Cargo.toml profile_conformance -- --nocapture
make validate-profile-conformance
node scripts/test-release-workflow.mjs
cargo test --manifest-path cli/Cargo.toml
make validate-authority-conformance
make validate-run-v1-golden validate-run-conformance validate-cold-model-conformance
make validate-run-mcp validate-native-parity
make validate-proposal-runner validate-proposal-evidence-harness validate-proposal-mcp
make validate-template validate-asset-sync validate-version-sync
make validate-public-artifacts validate-skill-packaging validate-plugin
make validate-installers
make validate
git diff --check
```

Build the non-test release target with `cargo build --release --manifest-path
cli/Cargo.toml`, inspect its strings/inventory only through a deterministic test
or existing packaging verifier, and confirm that neither the neutral fixture ID
nor its fixture path appears in production profile/template/capability/plugin or
release inventories. Do not install the resulting binary or bundles.

## 8. Risks, rollout, and rollback

- **CI trust-boundary change:** limit workflow edits to one named make step;
  bind it in `scripts/test-release-workflow.mjs` so deletion or substitution
  fails locally.
- **False neutrality:** keep a closed noun denylist, scan every fixture-owned
  string, and include positive mutations from both shipped domains.
- **Harness merely restates constants:** derive real subjects from parsed
  manifests, registries, and actual command outputs; only expected pack digests
  and universal invariants are golden data.
- **Profile-specific logic leaks into core:** one runner has no profile-ID
  branches; subject adapters may collect profile data but cannot weaken checks.
- **Test fixture accidentally ships:** keep both module and fixture test-only,
  assert closed production inventories, and preserve the current production
  binary/Pluxx release sources.
- **Noisy or opaque CI:** aggregate findings so both profiles run and use stable
  subject/check prefixes in every failure.

Rollout is the existing cumulative PR #236 only. Rollback is commit-level
reversal of the MDP-279 harness, fixture, Make target, CI step, and matching
workflow-contract assertion together. No migration, feature flag, compatibility
shim, release, deployment, or installation is required.

## 9. Blockers and readiness verdict

MDP-279 has no remaining Linear blocker. The cumulative PR, exact parent head,
closed registries, reusable validation seams, packaging boundary, CI location,
test-only fixture location, acceptance criteria, and rollback are known. The
implementation may proceed on `codex/mdp-279-conformance-gate`, then integrate
its exact verified head into `codex/mdp-273-primitive-contracts` and update
cumulative PR #236. Do not create a second PR, add optional `@codex review`,
merge, release, deploy, or install.

Readiness: `READY_TO_PIN`.
