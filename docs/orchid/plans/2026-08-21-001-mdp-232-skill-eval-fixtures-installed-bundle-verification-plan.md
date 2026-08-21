---
title: "MDP-232: Restore skill eval fixtures and installed-bundle verification"
type: feat
date: 2026-08-21
execution: code
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: linear-mdp-232
linear_issues:
  - MDP-232
  - MDP-239
---

# Restore Skill Eval Fixtures and Installed-Bundle Verification

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Make eval resources a first-class, package-verifiable part of the five public MDP skills and prove that the installed host bundles preserve the same eval inventory as source. |
| Authority | `plugin/skills/` is the authored skill surface; `plugin/skill-evals/` remains the one canonical shared corpus; each skill owns an `evals/index.json` manifest that names its exact shared cases and contract. The CLI's `mdp.skills.v1` inventory remains the routing authority. |
| Compatibility | Keep the current five public skill IDs and aggregate corpus semantics. Do not restore the retired MDP-60/67 skill directories, duplicate fixture bodies, or make eval files part of CLI route eligibility. Existing deterministic checks stay offline and synthetic. |
| Distribution | Pluxx copies the shared corpus to `skill-evals/` in every generated host bundle, while recursive skill copying carries each `skills/<id>/evals/index.json`. Source, generated, and installed trees are checked for exact content, executable bits, and inventory parity. |
| Host boundary | Deterministic CI validates fixture shape and CLI routes. Optional host observations provide with-skill/baseline/previous-version comparisons plus bounded timing and token metadata; they remain ignored scratch and never include transcripts or private data. |
| Stop condition | All five public skills have valid owned manifests; trigger/output fixtures cover positive, negative, near-miss, typo, indirect-intent, and profile-crossing behavior; objective assertions, comparison metadata, and usage metrics validate; generated and installed bundles contain the same eval inventory; `skills-ref validate` and repository validation pass. |

## Product Contract

### Problem

MDP 0.1.73 ships five public skills with focused references but no per-skill `evals/` inventory. The maintained corpus in `plugin/skill-evals/` is useful for deterministic routing and output checks, but the current distribution contract explicitly treats it as a maintainer-only catalog and does not copy it into generated host bundles. `validate-skill-packaging.py` and `release-install-smoke.sh` therefore prove skill prose fidelity without proving that an installed artifact contains the eval resources needed to reproduce the contract.

This leaves three gaps:

1. A skill maintainer cannot inspect one skill and discover exactly which trigger/output cases, modes, assertion categories, and safety boundaries it owns.
2. The corpus does not explicitly label typo and indirect-intent variants, so coverage can appear complete while missing the trigger shapes that cause real routing errors.
3. A generated or installed bundle can omit the shared corpus (or drift from it) while its five `SKILL.md` files still look correct.

### Required behavior

- Every current public skill (`mdp`, `mdp-gtm-brief`, `mdp-pack-builder`, `mdp-pack-review`, `mdp-proposal-review`) has `evals/index.json` with a stable schema, exact trigger/output case IDs, required modes and assertion categories, and a reference to the shared corpus path.
- The five manifests form a complete, non-overlapping ownership partition of owned trigger and output cases. A trigger whose `expected_skill_id` is `null` remains a corpus-level negative/null route and is not assigned to a skill manifest.
- Trigger fixtures explicitly represent direct positive behavior, negative/out-of-scope or unsafe behavior, near misses, typo queries, indirect intent, adjacent-skill collisions, and both directions of profile crossing in each validation split. Existing collision and null-route requirements remain in force.
- Output fixtures retain structured, objective, required assertions. Assertions cover routing/CLI gates, content/evidence correctness, boundaries, safety, handoff, and human review as declared by each skill manifest. A prose-only or ungradable expected output is invalid.
- Host result imports support paired `with-skill`, `without-skill`/baseline, and `previous-version` runs. Results carry only case IDs, grades, source/release identifiers, elapsed milliseconds, and token counts; reports contain aggregates and confusion matrices, not raw prompts, transcripts, contact data, or credentials.
- Packaging validation fails closed if any skill manifest, referenced case, shared corpus file, generated host copy, or installed host copy is missing, stale, duplicated, symlinked, or content-different.
- Installed smoke enumerates the same five skills, per-skill manifests, root corpus files, and host-to-host parity. It invokes the installed-bundle eval harness against the installed CLI and installed resources.

### Non-goals and boundaries

- No model/provider calls, network calls, browser use, CRM mutation, email, scheduling, or automatic host activation.
- No expansion of the public skill inventory and no changes to `mdp.skills.v1`, `PROFILE_JOBS`, or route eligibility.
- No copying of obsolete `mdp-lfg`, `mdp-proposal-pack-builder`, or other retired skill trees from historical MDP-60/67 commits.
- No checked-in host results, private transcripts, raw model output, contact values, API tokens, or unsanitized screenshots.
- No Blocks branding, Linear status/delegation changes, MDP-239 mutation, or release labeling as part of this plan.

## Current Baseline and Dependencies

- Repository: `orchidautomation/message-decision-packs`; base branch `main`; implementation must use one task branch/worktree and a PR as source of truth.
- Planning base observed for this artifact: `2cba9919483b5a7ba46efed53e3b5502b2abf477` (`MDP-225: Prepare v0.1.73 release`).
- Public source inventory is exactly the five directories under `plugin/skills/`; their `SKILL.md`, `agents/openai.yaml`, and `references/` files are currently the only authored skill resources.
- Current corpus files are `plugin/skill-evals/coverage.json`, `trigger-cases.json`, and `output-cases.json`. The harness is `scripts/skill-eval-harness.py`; mutation coverage is `scripts/test_skill_eval_harness.py`.
- `pluxx.config.ts` currently declares `skills`, `scripts`, and `assets`, but not a shared-corpus passthrough. CI and release pin `@orchid-labs/pluxx@0.1.40`; verify its passthrough schema and generated destination before implementation. If that pinned release cannot carry a root passthrough, stop and choose a source-controlled packaged-assets location rather than silently shipping an incomplete bundle.
- `scripts/validate-skill-packaging.py` compares `plugin/skills` to generated host skill roots; `scripts/release-install-smoke.sh` compares `scripts`, `skills`, and `assets` and currently calls the harness only with an installed skill root.
- MDP-239 is the execution index. MDP-232 is a Phase 0 child alongside MDP-226/229/231/233; it is not permission to implement those children or to alter the index issue.

## Technical Decisions

### TD1. Keep one canonical shared corpus and add per-skill indexes

Retain fixture bodies and the collision ledger under `plugin/skill-evals/`. Add the following tracked files:

```text
plugin/skills/mdp/evals/index.json
plugin/skills/mdp-gtm-brief/evals/index.json
plugin/skills/mdp-pack-builder/evals/index.json
plugin/skills/mdp-pack-review/evals/index.json
plugin/skills/mdp-proposal-review/evals/index.json
```

Each index uses `mdp.skill-eval-index.v1` and contains `skill_id`, `corpus_root: "skill-evals"`, corpus model names, explicit `trigger_case_ids`, explicit `output_case_ids`, supported modes, required assertion categories, required trigger shapes/types, and the comparison modes it supports. The harness verifies that every listed trigger is owned by that skill, every output has that skill's `skill_id`, every required mode/shape is present in both splits, and all owned cases are listed exactly once. The indexes are references, not duplicate fixture bodies.

This preserves the existing aggregate collision matrix and avoids reviving obsolete skill names while making ownership discoverable from each installed skill.

### TD2. Ship the shared corpus as a Pluxx passthrough

Add the pinned-Pluxx-supported equivalent of:

```ts
passthrough: ['./plugin/skill-evals/'],
```

The generated destination must be the bundle-root `skill-evals/`, alongside `skills/`, `scripts/`, and `assets/`. The implementation must confirm the exact `@orchid-labs/pluxx@0.1.40` API and output path with `pluxx doctor`, `pluxx test`, and a generated bundle before committing. Do not make a second authored `skills/` tree or copy corpus files into each skill.

### TD3. Extend the deterministic corpus contract without changing CLI authority

Keep the existing `mdp.skill-eval-coverage.v1`, `mdp.skill-trigger-corpus.v1`, and `mdp.skill-output-corpus.v1` model names unless implementation evidence shows a breaking schema change. Add closed fields rather than changing `mdp.skills.v1`:

- `coverage.json.trigger_requirements.required_query_shapes_per_skill_split`: `direct`, `typo`, and `indirect-intent`.
- `trigger-cases.json` `query_shape`: one of `direct`, `typo`, or `indirect-intent`; existing `case_type` continues to express positive, near-miss, out-of-scope, unsafe, adjacent, or profile-crossing semantics.
- `coverage.json` per-skill `eval_index` metadata: required manifest path and allowed modes/categories.
- Output-case required assertions continue to be the objective grade surface; no free-form rubric replaces them.

If a field/model is genuinely incompatible with existing consumers, introduce a versioned v2 model and make the harness read the new version only for the release gate while documenting the migration; do not silently accept missing MDP-232 fields.

### TD4. Make host comparison and usage metrics additive and privacy-safe

Extend `mdp.skill-host-results.v1` with an optional, validated `recording` object for host-run artifacts:

```json
{
  "comparison_mode": "with-skill|baseline|previous-version",
  "comparison_id": "synthetic-pair-id",
  "source_revision": "public-ref-or-sha",
  "elapsed_ms": 1234,
  "input_tokens": 100,
  "output_tokens": 200
}
```

When `--results` is supplied for the new host gate, require non-negative integer metrics and a non-empty comparison ID; do not commit the result file. Add `--baseline-results` and `--previous-results` scratch inputs to the harness, compare only aggregate accuracy/assertion metrics, and report deltas. Existing deterministic runs with no host result remain valid and never claim to observe a model. If an existing consumer requires strict v1 shape, write a new `mdp.skill-host-results.v2` adapter and preserve a clear v1 compatibility read path rather than changing a released external schema in place.

## Implementation Units

### U1. Author per-skill eval indexes and fixture-shape coverage

**Files and symbols**

- `plugin/skills/mdp/evals/index.json`
- `plugin/skills/mdp-gtm-brief/evals/index.json`
- `plugin/skills/mdp-pack-builder/evals/index.json`
- `plugin/skills/mdp-pack-review/evals/index.json`
- `plugin/skills/mdp-proposal-review/evals/index.json`
- `plugin/skill-evals/coverage.json` skill rows and `trigger_requirements`
- `plugin/skill-evals/trigger-cases.json` cases and collision ledger
- `plugin/skill-evals/output-cases.json` cases/assertions only where coverage gaps are found

**Work**

1. Enumerate the exact current five-skill corpus ownership. Generate explicit case-ID lists in each index and confirm that null routes are not silently assigned.
2. Audit all current trigger cases. Add or annotate synthetic `query_shape` values and add split-safe typo and indirect-intent examples where a skill/split lacks them. Keep scenario families in one split, preserve ordered collision pairs, and retain profile-crossing cases in both splits.
3. Add only missing output cases/assertions needed to cover every declared mode in train and validation. Assertions must say what an evaluator can observe: route/gate status, selected authority, evidence/claim handling, safety refusal, handoff, or human-review boundary.
4. Record no real customer, prospect, proposal, email, or model transcript data. Keep raw host runs under ignored scratch paths.

**Acceptance**

- Every public skill has an index; indexes are a complete disjoint partition of owned trigger/output IDs.
- Each skill/split has direct, typo, and indirect-intent triggers plus the existing owned/mode/collision minimums.
- Negative, near-miss, unsafe/out-of-scope, and profile-crossing semantics remain explicit and cannot be satisfied by positive-only cases.

### U2. Extend the eval harness and mutation tests

**Files and symbols**

- `scripts/skill-eval-harness.py`: constants `CASE_TYPES`/new `QUERY_SHAPES`; `load_json`; new `validate_skill_eval_indexes`; `validate_coverage`; `validate_triggers`; `validate_outputs`; `validate_observed_results`; new comparison/recording validator; `main` argparse.
- `scripts/test_skill_eval_harness.py`: fixture loader, `valid_host_results`, and mutation tests.

**Work**

1. Load five `evals/index.json` files and validate schema, exact skill ID, shared-corpus paths, known case IDs, ownership, split/mode/category coverage, and disjointness. Validate source indexes and, when passed, installed indexes plus `--installed-corpus` recursively.
2. Add the closed query-shape validator and coverage assertions. Fail on missing/unknown shapes, malformed typo/indirect cases, duplicate case ownership, unknown modes, missing required assertions, or profile-crossing owner selection.
3. Keep `compare_skill_trees` as the per-skill content/executable/symlink check and add an equivalent shared-corpus tree comparison. The installed root must contain exactly the source corpus and indexes, not extra hidden fixture files.
4. Parse optional comparison recording metadata, require bounded non-negative metrics, reject raw transcript fields and duplicate pair/trial IDs, and add baseline/previous aggregate deltas without invoking a model.
5. Add mutation tests for missing index, wrong `skill_id`, unknown/duplicated case ID, wrong owner, missing query shape, installed shared-corpus drift/missing file/symlink, invalid recording metrics, mismatched comparison IDs, duplicate baseline trials, and failed required output assertions. Update valid synthetic results with the new metadata.

**Compatibility**

Keep existing function call contracts used by tests where practical; add optional parameters (`installed_corpus`, comparison paths) with safe defaults. Existing `--plugin-skills`, `--corpus`, `--mdp-bin`, `--installed-skills-root`, `--results`, and `--output` invocations must continue to work for deterministic source checks.

### U3. Make packaging validation prove eval resources

**Files and symbols**

- `scripts/validate-skill-packaging.py`: source/corpus constants, `relative_files`, `skill_inventory`, `compare_bundle`, new shared-corpus/index validation helpers, `main` argument handling and result model.
- `scripts/test_skill_packaging.py` (new focused mutation suite) or the existing packaging test surface if the repository keeps packaging tests consolidated.
- `pluxx.config.ts`: `definePlugin` configuration, adding the verified passthrough.

**Work**

1. Validate source indexes before inspecting bundles and include `evals/index.json` in each skill's canonical recursive tree.
2. With `--require-bundles`, compare `plugin/skill-evals` to `<dist>/<host>/skill-evals` for all four hosts, including hashes and executable bits; reject missing root corpus, extra files, symlinks, or per-skill index drift.
3. Keep generated Codex/OpenCode inventory checks tied to the same five skill IDs. Eval resources must not alter the inventory emitted by `mdp --json skills`.
4. Add focused mutation proof for omitted shared corpus, changed index, extra installed case, and wrong shared-corpus destination. Make `make validate-skill-packaging` run the focused unit test before the source check; the CI/release `--require-bundles` check remains the generated-bundle gate.

**Acceptance**

Packaging fails before release publication if source and generated host trees disagree, even when all `SKILL.md` files and CLI skill IDs still match.

### U4. Extend release/install and host parity smoke

**Files and symbols**

- `scripts/release-install-smoke.sh`: host skill loop around lines 125-153, staged manifest inventory around lines 156-197, final harness invocation around lines 504-508.
- `scripts/test-release-install-smoke.sh`: fake installer copy loop around lines 61-72 and manifest `trees` array around lines 90-108.
- `scripts/test-opencode-wrapper.mjs`: generated Codex/OpenCode installer assertions around lines 285-369 and finalized `plugin_trees` assertions around lines 236-241.
- `scripts/finalize-release-manifest.mjs`: verify the recursive plugin-tree walker continues to include the new root passthrough; change only if its tree allowlist excludes `skill-evals`.

**Work**

1. Copy `plugin/skill-evals` into each fake installed plugin and include it in the staged fixture manifest. Assert each host has the three corpus JSON files plus all five skill indexes.
2. Add `skill-evals` to host-to-host `diff -qr`, executable inventory, and staged manifest prefix filters. Keep the existing scripts/skills/assets checks unchanged.
3. Call the harness with explicit source and installed paths, e.g. `--plugin-skills "$ROOT/plugin/skills" --corpus "$ROOT/plugin/skill-evals" --installed-skills-root "$codex_plugin_root/skills" --installed-corpus "$codex_plugin_root/skill-evals"`. This avoids depending on the caller's working directory.
4. Extend generated Codex/OpenCode proof to assert the root corpus and indexes survive installation. The check must remain deterministic and must not run a host model.

### U5. Align docs, Make targets, and CI/release gates

**Files and symbols**

- `docs/skill-evals.md`: corpus tree, deterministic gate, host-observed results, and iteration sections.
- `docs/distribution.md`: release artifact inventory and source/generated/installed distinction.
- `Makefile`: `.PHONY`, `validate`, `validate-skill-evals`, `validate-skill-packaging`, and a focused `validate-skill-ref` target if needed.
- `.github/workflows/ci.yml`: five-skill eval step and `skills-ref validate` step.
- `.github/workflows/release.yml`: generated bundle, install-smoke, and release validation ordering.

**Work**

1. Document the shared-corpus/per-skill-index contract, bundle path `skill-evals/`, exact source/installed parity proof, safe scratch location, comparison metrics, and the fact that eval resources do not grant route eligibility or external actions.
2. Keep `make validate-skill-evals` deterministic and offline. Add `--installed-corpus` only to installed smoke. Add `npx --yes skills-ref validate plugin/skills/<skill-id>` for each of the five skills in the focused validation target, using the repository's documented `skills-ref` invocation and failing closed if the tool is present but reports an error. Do not make a network-dependent command the only local validation path.
3. Run the focused target in CI alongside the existing Rust/CLI harness and run it through `make validate` before release. CI/release must run `pluxx doctor`, `pluxx lint`, `pluxx test`, packaging with `--require-bundles`, and installed smoke in that order.
4. Update release documentation so consumers know the shared corpus is shipped for verification but is not runtime skill instruction or CLI eligibility authority.

## Acceptance Mapping

| MDP-232 acceptance | Planned proof |
|---|---|
| Every public skill has focused eval inventory or exact shared reference | U1 five `evals/index.json` files; U2 index schema/ownership validator; U4 installed index checks. |
| Positive, negative, near-miss, typo, indirect-intent, profile-crossing triggers | U1 corpus audit and `query_shape`; U2 `QUERY_SHAPES`, case-type, collision, null-route, and crossing checks. |
| Objective MDP safety/correctness output assertions | U1 output assertion audit; U2 required assertion/category validation and failed-grade mutations. |
| With-skill vs baseline/previous comparison | TD4 and U2 comparison inputs/aggregate deltas; host records remain scratch. |
| Timing/token usage recorded | TD4 `recording` metrics; U2 non-negative integer validation and report-only aggregation. |
| Raw runs ignored; synthetic fixtures/aggregates only | U1 privacy boundary; `.gitignore`/scratch check during implementation; no results committed. |
| Packaging fails missing/stale eval resources | U3 source/bundle hash comparison and mutations; verified Pluxx passthrough in `pluxx.config.ts`. |
| Installed artifact enumerates source-equivalent inventory | U4 smoke, host parity, staged manifest, explicit installed harness paths. |
| `skills-ref validate` and repository validation pass | U5 Make/CI/release gates plus `make validate`; existing quick validator remains in place. |

## Validation and Evaluation Matrix

### Focused local checks

```bash
python3 -m unittest scripts/test_skill_eval_harness.py scripts/test_skill_packaging.py
python3 scripts/skill-eval-harness.py \
  --plugin-skills plugin/skills \
  --corpus plugin/skill-evals \
  --mdp-bin cli/target/debug/mdp \
  --output /tmp/mdp-skill-evals
python3 scripts/validate-skill-packaging.py
for skill in plugin/skills/*; do npx --yes skills-ref validate "$skill"; done
git diff --check
```

### Generated and installed checks

```bash
pluxx doctor
pluxx lint
pluxx test
python3 scripts/validate-skill-packaging.py --require-bundles
bash scripts/test-release-install-smoke.sh
MDP_RELEASE_REQUIRE_STAGED_PARITY=1 scripts/release-install-smoke.sh <version>
```

The implementation must inspect generated trees before deleting temporary output and confirm that every host contains `skill-evals/coverage.json`, `trigger-cases.json`, `output-cases.json`, and the five per-skill indexes. Host-result comparison trials, if run, go under `.agent-artifacts/` or `/tmp`; only the deterministic report and sanitized aggregate needed for review may be attached, never committed.

### Full repository gate

```bash
make validate
eve info  # only if the repository's release handoff invokes Eve; record exact outcome
```

Run `make validate` from a clean implementation worktree after focused checks. If an environment lacks the skill validator, Pluxx package, Rust toolchain, or network for `skills-ref`, record the exact skipped/failed command and reproduce it in CI; do not weaken the contract or commit generated scratch.

## Risks, Dependencies, and Mitigations

| Risk/dependency | Mitigation and stop condition |
|---|---|
| Pinned Pluxx 0.1.40 does not support `passthrough` or uses another destination | Verify the package schema and generated output first. If unsupported, stop and choose a documented packaged-assets path that is already copied by Pluxx; never assume a root corpus shipped because source validation passed. |
| Corpus IDs are duplicated across manifests or drift when a case is edited | Make the harness derive ownership from both sides and require exact set equality/disjointness; mutation tests delete, duplicate, and reassign IDs. |
| New typo/indirect examples leak across train/validation | Keep scenario-family split checks and add explicit shape counts per skill/split; use synthetic variants only. |
| Host results expose private prompts or credentials | Schema accepts IDs/grades/metrics only; reject transcript-like fields, keep files ignored, and document redaction. |
| Adding eval directories changes Pluxx semantic scoring or host behavior | Preserve the five SKILL.md descriptions and CLI inventory; eval indexes are references. Run Pluxx lint/test and host install smoke on every generated bundle. |
| Release manifest omits a new root tree | Extend fixture and staged-prefix checks, then verify `finalize-release-manifest.mjs` hashes the generated root recursively. |
| `skills-ref` availability/network differs by runner | Keep the command as an explicit focused gate with a clear tool-unavailable diagnostic; preserve offline validators for local development and run the required command in CI. |
| MDP-239 ordering changes while this plan is implemented | Re-read MDP-239 before implementation, preserve its Backlog/planned state unless its owner changes it, and do not claim Phase 0 completion from this plan alone. |

## Compatibility and Rollback

- Existing `mdp --json skills`, skill IDs, route cases, generated host inventory files, and runtime hooks remain unchanged.
- Existing source corpus consumers can continue reading the three current JSON files. New required fields are validated by the updated harness; if a model version must change, use an explicit versioned adapter and document the old read path.
- A rollback is a single PR revert: remove the five indexes, passthrough, validator/harness extensions, and smoke assertions. The old source-only corpus gate and five-skill bundles remain recoverable. Do not delete or overwrite historical raw artifacts; temporary output is disposable and ignored.
- Before implementation handoff, compare the task branch against `origin/main`, retain unrelated dirty work in other worktrees, and use the pushed commit/PR—not an unpushed local tree—as the Linear handoff source of truth.

## Handoff Contract

Implementation should start only after the plan is reviewed against the exact MDP-232/MDP-239 issue text. The implementer must report:

- repository `orchidautomation/message-decision-packs`, base `main` and exact base SHA;
- one pushed source ref `codex/<descriptive-mdp-232-name>` and exact commit SHA;
- tracked plan path (this file);
- Pluxx pinned version and passthrough destination evidence;
- focused, packaging, generated-bundle, installed-smoke, `skills-ref`, and full validation results;
- any host-result comparison run IDs only as sanitized aggregate references;
- remaining dependency/blocker, especially whether MDP-239 still expects MDP-232 as Phase 0;
- explicit statement that no runtime behavior, external message, private data, Linear status/delegation, or Blocks branding was changed by the handoff.

