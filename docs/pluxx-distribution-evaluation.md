# Pluxx Distribution Evaluation

> Historical evaluation retained for provenance. Its pre-release validation results are superseded by the current [Distribution](distribution.md) contract and release closeout process.

> The migrated root skill tree and scaffold metadata described below were removed in July 2026. `plugin/skills/` is now the only authored skill source, and generated bundles are verified against it.

Date: 2026-06-24

## Decision

Pluxx is a strong candidate for the Message Decision Packs plugin distribution path.

Use it for:

- one canonical plugin source
- native plugin bundles for Codex, Claude Code, Cursor, and OpenCode
- local install testing
- GitHub Release packaging for plugin archives, installers, manifest, and checksums

Do not treat Pluxx as the complete CLI distribution answer yet. The MDP Rust CLI is still its own product artifact. Pluxx can ship plugin files, scripts, and assets, but the current migrated bundle does not include compiled `mdp` binaries or install them onto `PATH`.

## Decision: skills.sh As A Long-Tail Skill Installer

Date: 2026-06-24

Use `skills.sh` as a documented compatibility path for skill-aware agents that are not first-class Pluxx release targets.

MDP should keep Pluxx as the primary release packaging path because Pluxx builds the host plugin bundles and release installers used by the one-command install. The public installer remains the default user path because it can install both the `mdp` CLI and the agent/plugin files:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

For CLI-only users, use:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y
```

For agents outside the Pluxx-supported release set, document `skills.sh` as an optional fallback for installing MDP's `SKILL.md` files:

```bash
npx skills add https://github.com/orchidautomation/message-decision-packs --skill '*' --agent '*' -g -y
```

Use the `universal` target when the user's agent follows the shared `.agents/skills/` convention:

```bash
npx skills add https://github.com/orchidautomation/message-decision-packs --skill '*' --agent universal -g -y
```

This fallback installs skill instructions only. It does not replace the MDP installer because `skills.sh` does not install the Rust `mdp` CLI binary, release assets, or plugin runtime scripts. Users who install skills this way still need the CLI from the MDP release installer, usually `--cli`, or another supported binary install path.

This is intentionally a documentation decision, not a new product surface. Do not add another release system unless user demand shows that a fifth host needs a first-class Pluxx target or a dedicated generic skills install flag.

## What Was Tested

Starting point:

- Branch: `codex/evaluate-pluxx-distribution`
- Source plugin: `plugin/`
- Pluxx CLI: `0.1.22`

### Compatibility Refresh: Pluxx 0.1.36

Date: 2026-07-18

MDP CI and release builds now pin Pluxx `0.1.36`. The refresh from `0.1.22` through `0.1.36` was reviewed against MDP's maintained source, generated bundles, release manifests, and installed OpenCode wrapper.

Relevant results:

- Codex hook manifest generation, OpenCode plugin-root separation, and `PLUXX_HOOK_WORKSPACE_ROOT` propagation are covered by `scripts/test-pluxx-hooks.sh` and pass with `0.1.36`.
- The hook fixture now builds real release assets, installs the generated top-level OpenCode wrapper into an isolated location, launches it from a parent directory, selects a different MDP workspace, and proves activation preserves both the installed plugin root and selected workspace root.
- Pluxx's generic semantic rubric is retained as a warning gate (`warningThreshold: 60`, `failureThreshold: 40`) because MDP is a migrated/manual plugin with repo-owned skill evals in `plugin/skill-evals/`.
- Migrated scaffold hook metadata now uses the current `safe` mode values. MDP is a plugin-migrated project; the repo ships its own local stdio MCP script, but does not use Pluxx as the source of truth for generated MCP tools, so `pluxx sync` is not a supported maintenance path unless MDP later adopts a Pluxx-managed MCP source.
- New Claude Code description-length diagnostics identified five skills whose discovery descriptions exceeded the 250-character display limit; the root and `plugin/skills/` copies were shortened together.
- CI now runs the MDP-specific Pluxx hook fixture, rejects future skill-description truncation, and checks that eval fixtures with `description_under_test` stay aligned with their skill frontmatter.
- Codex cache secret scanning, stdio MCP runtime launcher registration, MCP workspace-root preservation, and custom-agent registration require no MDP-specific Pluxx config because this plugin declares no auto-registered MCP servers, install-time secrets, or custom agents. The local proposal MCP wrapper is packaged as a repo script.
- MDP keeps its top-level installer because it coordinates the Rust CLI and multiple Pluxx host bundles. Pluxx-generated host installers and release assets remain part of the release workflow.

Validation commands for this refresh:

```bash
pluxx doctor --json
pluxx lint --json
pluxx eval --json
pluxx test --json
bash scripts/test-pluxx-hooks.sh
```

Migration command:

```bash
pluxx migrate plugin
```

Generated source files:

```text
pluxx.config.ts
.pluxx/mcp.json
.pluxx/taxonomy.json
skills/
scripts/
assets/
```

Generated build output:

```text
dist/claude-code/
dist/cursor/
dist/codex/
dist/opencode/
```

## Validation Results

After fixing generated config issues, these passed:

```bash
pluxx doctor --json
pluxx lint --json
pluxx eval --json
pluxx test --target codex --json
pluxx build --json
pluxx test --json
pluxx install --dry-run --target codex --json
DAYTONA_DRY_RUN=1 MDP_VERSION=0.1.0 scripts/daytona-mdp-release-qa.sh
```

Results:

| Command | Result | Notes |
|---|---:|---|
| `pluxx doctor --json` | Pass | Initial migration had branding warnings; current polished config validates cleanly. |
| `pluxx lint --json` | Pass | The original Pluxx `0.1.22` evaluation passed with zero warnings. |
| `pluxx eval --json` | Pass | Scaffold file-level eval skipped because this is not MCP-tool-derived. |
| `pluxx test --target codex --json` | Pass | Builds Codex bundle and smoke-checks the plugin manifest. |
| `pluxx build --json` | Pass | Builds all configured targets. |
| `pluxx test --json` | Pass | Smoke-checks Claude Code, Cursor, Codex, and OpenCode bundles. |
| `pluxx install --dry-run --target codex --json` | Pass | Would install the generated Codex bundle through the local plugin marketplace. |
| `DAYTONA_DRY_RUN=1 MDP_VERSION=0.1.0 scripts/daytona-mdp-release-qa.sh` | Pass | Prints the fresh-sandbox release QA plan without creating a sandbox. |

GitHub Release dry run:

```bash
pluxx publish --dry-run --github-release --allow-dirty --version 0.1.0 --json
```

Result: planned release assets correctly in dry-run mode. No release was created.

Planned release assets:

```text
message-decision-packs-codex-v0.1.0.tar.gz
message-decision-packs-codex-latest.tar.gz
install-codex.sh
release-manifest.json
SHA256SUMS.txt
```

## Config Fixes Needed After Migration

The initial generated `pluxx.config.ts` did not pass lint.

Fixed:

- Reduced Codex default prompts from four to three because Codex supports at most three.
- Replaced unknown Codex capability `Workflow` with `Interactive`.

Under Pluxx `0.1.31`, validation passes without errors or skill-description truncation. The remaining lint output is expected host-translation guidance for Codex and OpenCode hooks.

## What Pluxx Solves

Pluxx gives MDP a clean plugin packaging layer:

- The repo can keep one canonical plugin source instead of hand-maintaining host-specific folders.
- Codex output preserves native plugin shape with plugin manifest, skills, scripts, and assets.
- Claude Code, Cursor, and OpenCode bundles can be generated from the same source.
- Maintainer-side validation becomes one command:

```bash
pluxx test
```

- Maintainer-side release planning becomes one command:

```bash
pluxx publish --github-release --version <version>
```

## What Pluxx Does Not Solve Yet

The current Pluxx migration does not package the `mdp` Rust CLI binary directly.

The generated Codex plugin still assumes:

```bash
mdp
```

exists on the user's `PATH`.

To close that install gap, the Pluxx source now includes:

```text
scripts/check-env.sh
scripts/bootstrap-runtime.sh
scripts/daytona-mdp-release-qa.sh
```

`check-env.sh` fails clearly when `mdp` is missing. `bootstrap-runtime.sh` installs the matching `mdp` binary from GitHub Releases when the CLI is absent. Pluxx-generated release installers already call `scripts/bootstrap-runtime.sh` after installing the plugin bundle, so the Codex release install path can bootstrap both plugin files and the CLI.

This still depends on publishing real `mdp-*` binary assets in the same release.

## Packaging Options

### Option 1: Plugin Via Pluxx, CLI Via GitHub Releases

This is the recommended near-term path.

Release shape:

```text
GitHub Release
- mdp-aarch64-apple-darwin
- mdp-x86_64-apple-darwin
- mdp-x86_64-unknown-linux-gnu
- checksums
- Pluxx-generated Codex plugin archive
- Pluxx-generated install-codex.sh
- release-manifest.json
```

User journey target:

```bash
curl -fsSL <release-url>/install-mdp.sh | sh
```

Then:

```bash
mdp --json doctor --dir .
```

Why this is best:

- Keeps the Rust CLI as a normal binary release.
- Keeps the plugin as a normal plugin release.
- Avoids hiding executable binaries inside agent instructions.
- Lets Codex skills detect missing CLI and give a precise fix.

### Option 2: Bundle CLI Binaries Inside The Plugin

Possible, but not recommended as the first release path.

This would require:

- adding platform-specific binaries under plugin assets
- selecting the right binary at install or runtime
- adding executable permission handling
- deciding whether skills call `mdp` from `PATH` or from the plugin cache
- avoiding checked-in binary churn

This makes install feel simpler only if the host supports safe executable setup. Otherwise it makes the plugin harder to audit and update.

### Option 3: npm Wrapper For CLI Plus Pluxx Plugin

Useful later if we want:

```bash
npm install -g @orchid-labs/mdp
```

The wrapper would download or invoke the right Rust binary. This is viable, but it adds Node as a distribution dependency for a Rust CLI that otherwise does not need it.

### Option 4: Pluxx As The Main Maintainer Release Tool

This is the best role for Pluxx now.

Maintainer flow:

```bash
pluxx test
pluxx publish --github-release --version <version>
```

This can generate plugin archives, installer scripts, release metadata, and checksums. Pair it with a Rust binary release job and the product gets a clean public release path without pretending the plugin is the CLI.

## Recommended Next Build

1. Keep `plugin/skills/` as the canonical skill source and generate every host bundle from it.
2. Add `/dist/` to `.gitignore` if Pluxx is adopted.
3. Add release automation that builds `mdp` binaries and then runs Pluxx publish.
4. Update install docs so the first user sees one primary command, backed by separate CLI and plugin artifacts.
5. Run the Daytona release QA harness against a real release tag:

```bash
MDP_VERSION=0.1.0 scripts/daytona-mdp-release-qa.sh
```

6. Add a plugin doctor path that checks `command -v mdp` and points to the release install command when missing.

## Readiness

Pluxx is locally credible for MDP plugin packaging now.

It becomes a complete one-command end-user install after the GitHub Release contains both the Pluxx installer assets and the matching `mdp-*` CLI binaries, and the Daytona QA harness passes against that release.

Full Daytona QA was attempted against the public latest-release installer URL on 2026-06-24 with a fresh sandbox:

```bash
DAYTONA_SANDBOX_NAME=mdp-release-qa-latest-20260624d scripts/daytona-mdp-release-qa.sh
```

Result: blocked at the expected first release gate. The sandbox was created successfully, had no preinstalled `mdp`, and failed because the latest-release installer asset did not exist yet. No plugin was installed. The sandbox was deleted after the check.

Rerun the same command after the first GitHub Release exists with `install.sh`, host installer assets such as `install-codex.sh`, and the matching `mdp-*` binary assets.
