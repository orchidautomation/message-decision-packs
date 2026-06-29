# CLI Operator Hardening Scope

Date: 2026-06-29
Repo: `orchidautomation/message-decision-packs`
Team: MDP / Message Data Pack
Linear: MDP-2 parent, MDP-3 version command, MDP-4 upgrade guidance, MDP-5 agent-driving affordances

## Current State

- Active source commit: `f824f8f` on `origin/main`.
- Installed CLI: `/Users/brandonguerrero/.local/bin/mdp`.
- Installed CLI version: `mdp 0.1.7`.
- Latest release from `scripts/check-update.sh`: `v0.1.7`.
- Local CLI and plugin are current against the latest release.
- `mdp --version` works through Clap's generated global flag.
- `mdp version` fails with `unrecognized subcommand 'version'`.
- `mdp upgrade` fails with `unrecognized subcommand 'upgrade'`.
- Update guidance exists in `docs/distribution.md` and `scripts/check-update.sh`, but it is not exposed through the binary.

## Product Read

This gap is real and worth fixing. MDP is a local/offline decision contract layer, but the user experience around install health and updates is currently too repo-script-shaped. Users who install the release binary should be able to ask the binary what it is, whether it is current, and what the safe upgrade path is.

The upgrade path should stay explicit and auditable. The current docs intentionally say to rerun the installer instead of silently replacing the plugin/CLI during normal agent work. A first `mdp upgrade` should therefore either print the canonical installer command or run it only behind an explicit flag, rather than doing hidden background replacement by default.

## Recommended Scope

1. Add `mdp version`.
   - Human output should include CLI version, MDP format version, prompt contract version, default pack directory, and install path when discoverable.
   - `--json` output should use a stable contract, for example `mdp.version.v0`.
   - Keep `mdp --version` as the terse compatibility flag.

2. Add `mdp upgrade`.
   - Default behavior should be non-destructive: print current version, the canonical installer command, and pinned-version examples.
   - Add `--check` only if the implementation can query GitHub releases without making the core CLI brittle.
   - Add `--run` or `--yes` only if the command makes network and installer execution explicit before it mutates local tools.
   - Respect existing environment knobs: `MDP_VERSION`, `MDP_GITHUB_REPO`, `MDP_INSTALL_DIR`, `MDP_RELEASE_BASE_URL`, and `MDP_SKIP_CLI_UPDATE`.

3. Fold update visibility into `mdp doctor`.
   - `doctor` should mention whether the CLI can see an update-check script or upgrade command.
   - Avoid making `doctor` fail when offline or when GitHub cannot be reached.

4. Keep docs and agent instructions aligned.
   - Update `README.md`, `cli/USAGE.md`, and `docs/distribution.md`.
   - Update relevant MDP plugin skills if they mention install, doctor, validation, or release closeout.
   - Add tests for human and JSON output contracts.

5. Add agent-driving affordances.
   - Add `mdp capabilities` with stable machine-readable command metadata: command names, coarse argument requirements, output contracts, side-effect class, write behavior, `--json` support, `--out` support, and validation semantics.
   - Add selected `--dry-run` support for write commands such as `init`, `brief --out`, `emit-brief --out`, and `pack --out`.
   - Add or formalize `--strict` for warning-producing validation/checking flows so agents and CI can opt into fail-closed behavior.
   - Stabilize common JSON error codes such as `pack_not_found`, `invalid_manifest`, `missing_card`, `unsupported_claim`, `insufficient_context`, `write_conflict`, and `invalid_argument`.
   - Keep this as contract/driveability hardening, not a new orchestration layer.

## Out Of Scope

- Silent self-update during normal `mdp` commands.
- Package-manager distribution such as Homebrew or npm.
- Hosted MDP APIs, auth, enrichment, CRM writeback, sending, sequencing, scraping, or BI behavior.
- Changing pack schema behavior.
- First-slice implementation of `mdp next`; revisit only after capabilities, dry-run, strict mode, and stable error contracts exist.

## Definition Of Ready For Implementation

- Decide whether `mdp upgrade` should only print guidance in the first iteration, or whether an explicit `--run` path should be included immediately.
- Confirm whether network-based `--check` belongs inside the Rust CLI now or should remain delegated to `scripts/check-update.sh` until distribution matures.
- Split implementation if needed:
  - `version` command can ship independently.
  - `upgrade` command and `doctor` update visibility can ship as the next slice.
  - `capabilities`, selected `--dry-run`, strict gates, and stable error codes can ship as an agent-driving slice.

Linear breakdown:

- MDP-2: parent coordination issue.
- MDP-3: ready agent slice for `mdp version`.
- MDP-4: ready agent slice for non-destructive `mdp upgrade` guidance and update visibility.
- MDP-5: ready agent slice for `mdp capabilities`, selected `--dry-run`, strict mode, and stable JSON error codes.

## Validation

Run:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
make validate
```

Manual smoke checks:

```bash
mdp version
mdp --json version
mdp upgrade
mdp --json upgrade
mdp doctor
mdp --json doctor
```
