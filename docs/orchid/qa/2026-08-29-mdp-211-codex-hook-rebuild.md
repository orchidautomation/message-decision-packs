---
title: MDP-211 Codex hook rebuild with Pluxx 0.1.41
date: 2026-08-29
status: ready-for-human-merge
linear: MDP-211
depends_on: PLUXX-345
release: v0.1.95
compiler_pin: "@orchid-labs/pluxx@0.1.41"
compiler_sha512: sha512-m08Sr20N2SzohxySOSETpuQQlVVEFqyubreONy2KTWvzz4JHr4nPueXgOmYJeKC1Tmuij3Odqwk767hvhK+YcA==
---

# MDP-211 Codex hook rebuild with Pluxx 0.1.41

## Decision

The Message Decision Packs 0.1.95 source builds against the released Pluxx
0.1.41 compiler. Every generated Codex hook descriptor (`SessionStart`,
`UserPromptSubmit`, `PostToolUse`) resolves the installed plugin root via
`${PLUGIN_ROOT}` instead of the absent `${CODEX_PLUGIN_ROOT}` variable that
collapsed MDP 0.1.70 to root-level `/hooks/*` failures inside Codex Desktop.

## Why

[MDP-211](https://linear.app/orchid-automation/issue/MDP-211) reports that the
installed MDP 0.1.70 Codex bundle emitted one console-failure on every
`SessionStart` and one on every `UserPromptSubmit` invocation. The PLUXX
generator had been emitting `node "${CODEX_PLUGIN_ROOT}/hooks/..."`, which
expands to `/hooks/...` whenever Codex Desktop does not publish
`CODEX_PLUGIN_ROOT` to the hook shell. Both the user-supplied wrapper and the
canonical `scripts/mdp-activate.sh` were unreachable.

The cross-host generator regression is owned by
[PLUXX-345](https://linear.app/orchid-automation/issue/PLUXX-345). The fix in
Pluxx 0.1.41 swaps the unsupported `CODEX_PLUGIN_ROOT` reference for the
already-documented `${PLUGIN_ROOT}` contract and hardens the wrapper to fall
back to `import.meta.url`-derived bundle resolution when no plugin root is
exported. Imported integrity matches the readback receipt recorded in
[`docs/orchid/plans/2026-08-29-mdp-211-pluxx-0.1.41-codex-hook-release.md`](../../plans/2026-08-29-mdp-211-pluxx-0.1.41-codex-hook-release.md):
`sha512-m08Sr20N2SzohxySOSETpuQQlVVEFqyubreONy2KTWvzz4JHr4nPueXgOmYJeKC1Tmuij3Odqwk767hvhK+YcA==`.

## Source-side proof

- `bash scripts/test-pluxx-hooks.sh` regenerates bundles from
  `pluxx.config.ts` with Pinned `PLUXX_VERSION=0.1.41` and asserts:
  - Codex `hooks/hooks.json` references `${PLUGIN_ROOT}` and never
    `CODEX_PLUGIN_ROOT`.
  - The exact generated `SessionStart` and `UserPromptSubmit` descriptor
    strings execute against the staged installed bundle root with
    `CODEX_PLUGIN_ROOT` unset and exit 0 from an unrelated workspace.
  - The same descriptor reaches `scripts/mdp-activate.sh` from a pack
    workspace and surfaces `detected in <pack-root>` in its output.
  - Claude Code, Cursor, and OpenCode hook outputs remain green.
- `bash scripts/validate-version-sync.sh && bash scripts/test-version-sync.sh`
  confirm the bumped `cargo`, `plugin.json`, and `pluxx.config.ts` versions
  agree on `0.1.95`.
- `bash scripts/test-opencode-wrapper.mjs` requires the workflow assertion
  that locks the new Pluxx 0.1.41 SHA-512.

## Owned path diff

- `.github/workflows/ci.yml`, `.github/workflows/release.yml` — Pluxx
  0.1.40 → 0.1.41 pin with the new SHA-512.
- `scripts/test-pluxx-hooks.sh` — `PLUXX_VERSION` default advanced to 0.1.41
  and a manifest-bound Codex regression proof added.
- `scripts/test-opencode-wrapper.mjs` — workflow SHA-512 assertion updated
  to the 0.1.41 release digest.
- `cli/Cargo.toml`, `cli/Cargo.lock`, `plugin/.codex-plugin/plugin.json`,
  `pluxx.config.ts` — version surfaces stepped together to `0.1.95`.

## Forbidden path confirmation

- No edits to `orchidautomation/pluxx`. PLUXX-345 owns the generator fix.
- No change to `scripts/mdp-activate.sh`, `scripts/mdp-post-edit-validate.sh`,
  the Rust CLI, pack validation, proposal runtime, or messaging behavior.
- No CRM, outreach, email, or remote-Ledger mutation introduced or implied.
- No secrets, private packs, customer data, or local filesystem paths in
  tests or evidence.

## Post-merge boundary

This issue stops at merge. The release of v0.1.95 (immutable tag, MD
checksums, GitHub release asset upload) and installation into the active
Codex cache are owned by the release lane after Brandon's merge and remain
required for the `release.completed` lifecycle step.

## Remaining risks

- Code signing and SHA-512 integrity for the v0.1.95 release asset set
  are produced by the GitHub release workflow; until that workflow runs
  the release is mutable.
- Long-lived Codex Desktop threads retain stale plugin roots. Fresh-thread
  proof is the post-merge Codex Desktop verification boundary; this PR
  does not exercise a live Codex Desktop install.
