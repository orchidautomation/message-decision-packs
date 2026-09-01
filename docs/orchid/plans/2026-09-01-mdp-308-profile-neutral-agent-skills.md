# MDP-308: Profile-neutral agent skills and job disclosure

## Goal Capsule

Replace MDP's vertical public skill entrypoints with one generic apply workflow
while preserving the existing GTM and proposal profile/job contracts. A host
should discover four reusable skills, use the CLI to resolve an exact profile
job, load only that job's direct guidance, and execute or evaluate it without
expanding MCP authority.

This plan covers MDP-309 through MDP-312 and is implemented as one cumulative
feature branch and one PR. The source baseline is MDP 0.1.106 at merge commit
`b2a5c425ce51bc2e47f6881cd467e9b081b1b216`.

## Product Contract

- Public skills are exactly `mdp`, `mdp-pack-builder`, `mdp-pack-review`, and
  `mdp-pack-apply`.
- `mdp-gtm-brief` and `mdp-proposal-review` are removed in the next patch
  release; no compatibility aliases remain in packaged inventories.
- Every supported GTM and proposal job binds to `mdp-pack-apply`. Canonical job
  IDs, prompts, input/output contracts, safety behavior, and artifact formats
  remain unchanged.
- Progressive disclosure is three-stage: concise metadata, common workflow in
  `SKILL.md`, then shared/runtime and one selected profile reference.
- The CLI remains the decision authority. MCP remains the existing four-tool
  evaluation adapter and gains no capability.
- Native key detection and consent remain presence-only and secret-safe. The
  Agent Plugins archive remains portable/CLI-only.

## Implementation Units

### 1. Neutral routing contract (MDP-310)

Change the canonical packaged inventory and every canonical profile route to
`mdp-pack-apply`. Update schemas, human summaries, profile validation,
authorities, starter fixtures, template manifests, and contract tests so the
CLI reports four packaged skills and one neutral apply recommendation for every
supported job. Preserve `mdp.skills.v1` structure and fail-closed job/profile
selection.

### 2. Progressive skill source (MDP-311)

Create `plugin/skills/mdp-pack-apply` and consolidate the existing GTM and
proposal execution guidance behind direct profile-specific references. Keep its
entrypoint generic: resolve pack/profile/job first, then load only shared apply
guidance plus the selected profile reference. Include precise file ownership,
clean-context evaluation, CLI/MCP roles, hook/key consent, run artifact, resume,
and verification guidance.

Update `mdp` to route all Use and decide work to the neutral apply skill. Keep
builder and pack-review generic, but adjust exact inventory wording and handoff
links. Remove the two vertical authored directories. Preserve useful detailed
references by moving/renaming rather than weakening their safety contracts.

### 3. Packaging, eval corpus, and docs

Update Pluxx/package manifests, release-manifest validation, installation and
packaging tests, skill contract validators, trigger/output eval corpora, and
behavioral harnesses from the five-skill vertical catalog to the four-skill
neutral catalog. GTM/proposal cases remain distinct by profile/mode while their
expected skill ID becomes `mdp-pack-apply`.

Update README and affected docs to explain the four-skill surface, migration,
progressive disclosure, file behavior, narrow MCP role, and API-key/consent
boundary. Do not edit generated native bundles directly.

### 4. Release and installed-proof readiness (MDP-312)

Bump all synchronized version declarations from 0.1.106 to 0.1.107 in the
feature PR. Build native and Agent Plugins artifacts, inspect their skill
inventories, and exercise representative GTM and proposal prepare/run/verify
paths in clean temporary roots. The PR stops at Ready for Human; release and
active installation occur only after Brandon merges.

## Ownership and safety

This branch may change:

- `plugin/skills/`, `plugin/skill-evals/`, and affected `plugin/assets/templates/` manifests;
- `cli/` skill catalog, projections, schemas, fixtures, and tests;
- skill/packaging/eval scripts under `scripts/`;
- `pluxx.config.ts`, `plugin/.codex-plugin/plugin.json`, README, and affected docs;
- synchronized 0.1.107 version surfaces.

It must not broaden MCP tools, native provider permissions, pack primitives,
profile set, hosted behavior, release workflows, sending, enrichment, CRM, or
proposal submission. Generated host bundles are validation outputs, not authored
source. Existing unrelated worktrees and the dirty intake checkout are out of
scope.

Direct Sol implementation is authorized only through an Orchid
`sol-implementation-exception`: Brandon explicitly prohibited Orchid/hosted
delegation for this project after provider issues. Work remains isolated in the
MDP-308 worktree with acceptance-mapped validation.

## Acceptance Criteria

1. CLI/package inventories contain exactly the four neutral skills.
2. Every canonical GTM and proposal job recommends `mdp-pack-apply` and retains
   its existing readiness, prompt, input, output, and safety contract.
3. The apply entrypoint is profile-neutral and loads only the direct reference
   for the CLI-resolved profile/job.
4. Skills accurately describe pack/input immutability, run-owned artifacts,
   clean-context evaluation, MCP/CLI authority, hook key detection, and native
   consent.
5. Vertical skill directories and packaged aliases are absent; migration text
   is explicit.
6. Trigger, output, contract, behavior, and installed-parity tests exercise the
   neutral surface and both reference profiles.
7. MCP exposes exactly the existing four tools and no source/provider behavior
   changes.
8. Native and portable packages expose the same intended skill inventory while
   truthfully retaining their different hook/MCP capabilities.
9. Version 0.1.107 is synchronized in the feature PR.
10. One cumulative PR targets `main` and stops for Brandon-only merge.

## Verification Contract

Run from the project worktree:

```bash
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo test --manifest-path cli/Cargo.toml
python3 scripts/validate-skill-contracts.py
python3 scripts/test_skill_contracts.py
python3 scripts/test_skill_packaging.py
python3 scripts/test_skill_eval_harness.py
python3 scripts/test_skill_behavioral_evals.py
python3 scripts/validate-skill-packaging.py
bash scripts/test-pluxx-hooks.sh
make validate-version-sync
make validate
git diff --check
```

Capture bounded behavioral proof for:

```bash
cargo run --manifest-path cli/Cargo.toml -- --json skills --dir plugin/assets/templates/basic --job prospect-fit-or-brief
cargo run --manifest-path cli/Cargo.toml -- --json skills --dir plugin/assets/templates/proposal --job compliance-review
cargo run --manifest-path cli/Cargo.toml -- --json requirements --dir plugin/assets/templates/basic --job outbound-copy-brief
cargo run --manifest-path cli/Cargo.toml -- --json requirements --dir plugin/assets/templates/proposal --job proof-review
```

Build and inspect both release package shapes. Assert the native hook inventory,
presence-only key guidance, portable no-hook/no-MCP contract, exact four-skill
inventory, and unchanged MCP tool list. Any model-backed behavioral evaluation
must use synthetic inputs and the existing explicit native-call consent gate.

## Definition of Done

The final head has clean status, acceptance-mapped passing evidence, a focused
public-safe diff, synchronized version 0.1.107, and one PR linked to MDP-308,
MDP-310, MDP-311, and MDP-312. Linear remains truthful: MDP-309 may close when
this committed decision/plan is pinned; implementation children remain active
until their code and proof are delivered. No merge, release, or active install
is performed without Brandon's later explicit authorization.
