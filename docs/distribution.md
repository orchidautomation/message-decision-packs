# Distribution Notes

The intended public shape is one repo containing both the CLI and the Codex plugin:

```text
message-decision-packs/
  cli/
  plugin/
  docs/
```

## Why One Repo

The CLI and plugin are tightly coupled:

- the CLI defines the pack schema, JSON contracts, validation, routing, entry routing, fit checks, claim checks, gaps, eval fixtures, and brief emission
- the plugin teaches agents how to author, inspect, and use those contracts
- examples, eval fixtures, and templates need to stay aligned with CLI behavior

Keeping them together avoids version drift while the standard is young.

## Local Use

Install the CLI:

```bash
make install-cli
```

Install the released CLI binary only:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y
```

Use the plugin source at `plugin/` when testing local Codex plugin installs.

## Future Distribution

Possible later channels:

- GitHub releases for CLI binaries
- Pluxx-generated plugin release archives and installers
- Homebrew formula for `mdp`
- npm wrapper only if agent workflows need Node distribution
- Codex/agent plugin marketplace entry for `plugin/`
- hosted MDP API that can serve validated packs and briefs

Do not treat hosted serving as part of the local MVP. The current implementation is offline and auth-free.

See [Pluxx Distribution Evaluation](pluxx-distribution-evaluation.md) for the current packaging recommendation and validation evidence.

## Release Installers

The public single-host install path uses the top-level installer plus a host flag:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --codex -y
```

The CLI plus supported agent-bundle installer mirrors Railway's agent installer shape:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

Use the CLI-only release installer when an agent/plugin bundle is not needed:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y
```

The top-level installer keeps the surfaces distinct:

- `--cli` / `--cli-only`: install only the `mdp` CLI.
- `--agents`: install the CLI once, then install supported host bundles for Claude Code, Cursor, Codex, and OpenCode. If Claude Code is not available, this path skips it with a warning.
- `--codex`, `--cursor`, `--claude-code`, `--opencode`: install one host bundle.

The tag-based release workflow installs Pluxx in CI, builds host plugin bundles, publishes Pluxx release assets, and uploads `mdp-*` CLI binaries plus `install.sh` and `install-cli.sh`. Host installer scripts install the plugin and use `scripts/bootstrap-runtime.sh` to prepare the local `mdp` CLI when it is missing.

## Updates

The public update path is to rerun the same installer:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

For CLI-only installs:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y
```

That keeps the update mechanism explicit and auditable. MDP changes can affect local CLI behavior, pack validation, routing, fit checks, claim checks, and agent skill instructions, so the plugin should not silently replace itself during normal agent work.

Use `scripts/check-update.sh` as a lightweight drift check:

```bash
scripts/check-update.sh
```

The script compares the local `mdp --version` and nearby plugin manifest version against the latest GitHub Release tag, then returns both the full install command and the CLI-only install command to run when either side is stale.

Host hooks may call this check at session start or plugin load time, but they should only notify. They should not auto-update by default. If a future host supports a trusted, user-approved update flow, the hook can offer the installer command as the next action.

Version policy:

- Release tags, `cli/Cargo.toml`, `pluxx.config.ts`, and plugin manifests should stay on the same semver.
- A user who pins `MDP_VERSION` should not be nudged to latest unless they ask for update checks against latest.
- `scripts/bootstrap-runtime.sh` should keep bootstrapping missing CLIs, not replacing an existing CLI unless the user reruns the installer or a future explicit `--force` update path is added.
