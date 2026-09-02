# MDP-4 — Aligned CLI And Agent Bundle Upgrade Plan

**Date:** 2026-09-02  
**Issue:** MDP-4  
**Repository:** `orchidautomation/message-decision-packs`  
**Base branch:** `main`  
**Planning baseline:** `e7006907ca39379f72b3676812857d71ecbb1edb` (`0.1.111`)  
**Risk:** Elevated — the command downloads and executes the fixed public installer and can replace local CLI/plugin files  
**Consumer:** One plan-pinned Orchid Work implementation lane authorized by Brandon's request to start MDP-4

## 1. Context And Current Behavior

The released installer is already the aligned update authority. `scripts/install.sh`
resolves one release, installs the CLI once, invokes the Pluxx native installer for
detected agent hosts, validates its `pluxx.install-results.v1` result, records
installed/updated/unchanged/skipped/failed target states, retains checksum
verification, and returns nonzero when any selected target fails. Its public fixed
front door is `https://mdp.orchidlabs.dev/install.sh`; `--agents -y` is the current
aggregate update contract.

The installed Rust CLI has no upgrade subcommand. Operators must recover the raw
installer command from `README.md` or `docs/getting-started.md`.
`scripts/check-update.sh` is repository-oriented, depends on nearby plugin paths,
and is not shipped as an installed CLI surface. `cli/src/commands/health.rs::doctor`
reports the running package version but no aligned-update next action.

Confirmed current contracts:

- `cli/src/cli.rs::Commands` is the Clap authority; `grouped_root_help()` derives
  grouped human help from it.
- `cli/src/app.rs::run` gates presentation before command work and routes output
  through `cli/src/output.rs`.
- Every global `--json` invocation must write exactly one JSON value to stdout and
  nothing to stderr. The first release will therefore support JSON for
  `upgrade --check` and explicitly reject mutating JSON upgrade execution before
  network access.
- `cli/src/commands/capabilities.rs` derives syntax from Clap and maintains legacy
  semantic command metadata.
- The current release version is synchronized across `cli/Cargo.toml`,
  `cli/Cargo.lock`, `pluxx.config.ts`, and `plugin/.codex-plugin/plugin.json`.
- MDP-202's outcome-first human/JSON conventions are on `main`. MDP-306's installer
  code is also on `main` even though its Linear lifecycle record is not yet closed;
  MDP-4 will reuse that code without changing its target semantics.

## 2. Objective

Add an explicit, auditable `mdp upgrade` adapter over the released installer:

1. `mdp upgrade` confirms interactively before any network access;
2. `mdp upgrade -y` downloads the fixed HTTPS installer and executes it as
   `bash INSTALLER --agents -y`;
3. `mdp upgrade --version VERSION` maps only to the installer's documented
   `MDP_VERSION` control;
4. `mdp upgrade --check` reports the running CLI version and reachable target
   release without mutation or false-current claims;
5. help, capabilities, doctor, docs, and authored operator guidance agree.

The Rust CLI owns intent, confirmation, safe download/process handling, and the
check output contract. The installer remains the only authority for release asset
resolution, checksums, CLI replacement, host detection, containment, target states,
and aggregate success/failure.

## 3. Scope

### In scope

- Add `Commands::Upgrade { yes, check, version }` with `-y`, `--check`, and
  `--version VERSION`.
- Add a focused `cli/src/commands/upgrade.rs` module.
- Require an interactive stdin terminal and an affirmative answer unless `-y` is
  present; a non-terminal invocation without `-y` fails with the exact next action
  `mdp upgrade -y` before downloader execution.
- Download only `https://mdp.orchidlabs.dev/install.sh`, visibly print that origin,
  and execute a regular temporary file with `bash` and `--agents -y`.
- Inherit documented installer environment controls. A CLI `--version` value
  overrides inherited `MDP_VERSION` for the child only. Display any effective
  version/repository/release-base/install-directory overrides before mutation.
- Preserve the child exit status semantically: zero is success; any nonzero or
  signal is a nonzero MDP failure; never print a success footer after failure.
- Stream or faithfully relay installer output so its per-target evidence remains
  the source of installed/updated/unchanged/skipped/failed reporting.
- Read back the exact expected install destination (`MDP_INSTALL_DIR/mdp`, otherwise
  `$HOME/.local/bin/mdp`) after success, when safely discoverable, rather than
  trusting whichever binary happens to win `PATH`.
- Emit restart/reload guidance once after successful installer completion. The
  installer's host-specific reload line remains authoritative; the CLI footer is a
  concise general reminder and must not claim a host was updated.
- Implement `mdp.upgrade-check.v1` for `--check`, including running version,
  requested/effective target, availability, status (`current`, `update-available`,
  or `unavailable`), fixed installer origin, `next_command`, and an explicit
  unassessed bundle-drift field when host-native drift cannot be observed.
- Add deterministic subprocess tests with fake `curl`, `bash`, `HOME`, and
  `MDP_INSTALL_DIR`; no test may access the network or active installation.
- Update release version metadata from `0.1.111` to `0.1.112` if that remains the
  next available version after refreshing `main` immediately before implementation.
- Update public docs and the authored `plugin/skills/mdp` operator runtime. Retain
  the raw installer as bootstrap/repair fallback.

### Out of scope

- Reimplementing installer resolution, checksum, rollback, host detection,
  containment, plugin placement, or native/portable target semantics in Rust.
- Single-host selectors in the first CLI release.
- Silent/background checks or upgrades during ordinary MDP commands.
- Mutating execution under global `--json`; only `--json upgrade --check` is
  supported initially.
- Parsing human installer output into a second Rust target-result authority.
- Changing `scripts/install.sh` unless implementation proves a narrowly required,
  backward-compatible invocation seam. No such change is currently expected.
- Restarting hosts, publishing a release, installing into Brandon's home, merging,
  deploying, or mutating production.
- MDP format migration or richer `mdp version` work owned by MDP-3.

## 4. Decisions And Assumptions

### D1. Use existing system tools, not a Rust HTTP/TLS stack

Invoke `curl` with fail-fast, silent-with-errors, redirect-following, HTTPS-only,
TLS-constrained arguments and invoke `bash` by name. Missing tools produce actionable
nonzero errors. This matches the bootstrap contract and avoids introducing another
TLS/release client.

### D2. Use an owned temporary file

Create a unique directory below `std::env::temp_dir()` with restrictive permissions
on Unix, create the installer file with create-new semantics, write downloader bytes,
sync/close it, execute it, and remove the directory best-effort. Never execute a
partial download, symlink, or caller-selected path.

### D3. Confirmation precedes all network work

Print running version, aligned-update scope, fixed source, target mode, and effective
overrides first. If `-y` is absent, require terminal stdin and accept only an explicit
yes response. Decline exits without mutation. EOF/non-terminal returns an actionable
error naming `mdp upgrade -y`.

### D4. Reject mutating JSON execution explicitly

`mdp --json upgrade --check` returns one normal MDP envelope. Any global `--json`
upgrade without `--check` returns one stable error envelope before confirmation,
download, temp-file creation, or child execution. Capturing and normalizing the
installer's mixed human output is deferred because doing so would create a competing
result interpretation.

### D5. Check uses explicit version or GitHub release metadata

With `--version`, the check target is that normalized version and needs no network.
Otherwise it asks the GitHub latest-release endpoint derived from the documented
`MDP_GITHUB_REPO` value using fakeable `curl`. A failed request, malformed response,
or missing tag produces `unavailable`, never `current`. The check does not claim
native bundle currency because installed host-bundle discovery is not a stable CLI
contract.

### D6. Keep installer evidence authoritative

The Rust adapter does not transform individual target outcomes. It relays the
installer's output and uses only the process status for overall success/failure.
Post-install CLI version is an observation at the expected install path and is
reported as observed or unavailable, never inferred from the requested target.

## 5. Affected Files And Symbols

| File | Current responsibility | Intended change |
|---|---|---|
| `cli/src/cli.rs` | Clap surface and grouped root help | Add `Upgrade`, arguments/help, and place it in the `Inspect` group. |
| `cli/src/app.rs` | Runtime dispatch and JSON purity gate | Dispatch check versus execution; reject JSON execution before side effects; route check through `print_output`. |
| `cli/src/commands/upgrade.rs` (new) | N/A | Implement fixed-origin check/download/confirmation/child execution, temp containment, status propagation, and post-install version observation. |
| `cli/src/commands/mod.rs` | Command exports | Register the upgrade module and exports. |
| `cli/src/commands/capabilities.rs` | Semantic capability inventory | Advertise check/execution modes, contracts, side effects, args, and JSON limitation. |
| `cli/src/commands/health.rs::doctor` | Installation and pack health | Add running version plus `mdp upgrade --check` guidance without making doctor perform network access. |
| `cli/src/output.rs` | Human/summary rendering and stable envelopes | Add intentional human/summary output for upgrade check if the generic renderer is insufficient; preserve one-JSON behavior. |
| `cli/tests/upgrade.rs` (new) | N/A | Isolated process-level upgrade/check/confirmation/failure tests with fake tools and home. |
| `cli/tests/cli_contract.rs` | Public help and capabilities | Assert command grouping, argument conflicts, origin/scope wording, and capability annotations. |
| `cli/tests/json_stdout_contract.rs` | Global JSON invariant | Cover successful/unavailable check plus mutating JSON refusal with empty stderr and no fake-tool calls. |
| `cli/Cargo.toml`, `cli/Cargo.lock`, `pluxx.config.ts`, `plugin/.codex-plugin/plugin.json` | Release version metadata | Bump together to the next available release version. |
| `README.md` | Public first-contact install/update | Make `mdp upgrade` primary for installed users; retain raw installer bootstrap/repair. |
| `cli/USAGE.md` | Full CLI reference | Document check, interactive, non-interactive, version pin, environment controls, JSON boundary, and exit behavior. |
| `docs/getting-started.md` | Golden path and updates | Replace rediscovery flow with installed CLI command and retain recovery installer. |
| `docs/distribution.md` | Distribution/update runbook | Describe the CLI as an adapter over the canonical installer and preserve release authority. |
| `plugin/skills/mdp/references/operator-runtime.md` | Authored installed operator guidance | Prefer `mdp upgrade --check` / `mdp upgrade -y`; use raw installer only when CLI is missing/broken. |

`scripts/install.sh` and installer tests are forbidden unless a concrete missing seam
is demonstrated and escalated to the Sol orchestrator.

## 6. Ordered Implementation Steps

### Step 1 — Add the closed command and mode gate

1. Add `Upgrade` to `Commands` with `-y/--yes`, `--check`, and `--version`.
2. Mark `--check` and `--yes` as conflicting; allow `--version` in either mode.
3. Add the grouped help entry and detailed `after_help` examples.
4. In `app::run`, reject mutating JSON execution before calling the upgrade module.

**Why:** Syntax, confirmation intent, and JSON purity must be enforced before side
effects.  
**Acceptance:** recognized command; discoverable aligned-update wording; explicit
non-interactive and JSON safety.

### Step 2 — Implement the read-only check contract

1. Record `env!("CARGO_PKG_VERSION")` as running version.
2. Normalize a CLI/environment version pin; otherwise call fakeable `curl` for the
   latest GitHub release metadata and parse only `tag_name` from bounded JSON.
3. Return `current`, `update-available`, or `unavailable` and exact next command.
4. State bundle drift as `unassessed` unless a future stable installer artifact
   provides direct evidence.
5. Add human, summary, and one-envelope JSON render tests.

**Why:** Update visibility must be useful without mutation or false certainty.  
**Acceptance:** read-only check, honest network failure, running/latest versions,
machine-readable output, exact next action.

### Step 3 — Implement confirmation and fixed-origin download

1. Render a preflight describing running version, fixed source, `--agents` mode,
   effective version pin, and non-default inherited installer controls.
2. Without `-y`, fail non-terminal stdin with `mdp upgrade -y`; on a terminal,
   explicitly ask and accept/decline before network access.
3. Resolve `curl` and `bash` only through `PATH`, then download the fixed HTTPS origin
   into the owned temporary file.
4. Fail on missing tool, downloader nonzero, empty download, temp safety error, or
   shell launch error without executing partial content.

**Why:** The CLI must be an auditable secure adapter, not a hidden pipe to shell.  
**Acceptance:** confirmation ordering; fixed visible endpoint; missing-tool and
download failures; isolated tests; no active-home writes.

### Step 4 — Execute the canonical installer and preserve outcomes

1. Run `bash TEMP_INSTALLER --agents -y`, inheriting documented installer variables
   and overriding only `MDP_VERSION` when the CLI option is present.
2. Relay child stdout/stderr without adding misleading success output.
3. Treat any child nonzero/signal as MDP failure and preserve the numeric nonzero
   exit where the application boundary permits; otherwise return a nonzero failure
   with the child status in the diagnostic.
4. On success only, inspect the expected install destination for `mdp --version`,
   report observed/unavailable, and emit one general restart/reload reminder.

**Why:** The installer owns aligned target logic and its evidence must survive intact.  
**Acceptance:** `--agents -y` forwarding; version/environment forwarding; idempotent
installer behavior; failure propagation; post-install observation; no false success.

### Step 5 — Align discovery and operator guidance

1. Add semantic capability metadata for read-only check and mutating execution.
2. Add doctor installation guidance without network calls or changing pack validity.
3. Update README, usage, getting-started, distribution, and authored operator-runtime
   guidance after the command contract is final.
4. Keep the raw public installer command as the missing/broken-CLI recovery path.
5. Bump synchronized release version metadata only after rechecking concurrent main
   and open release work.

**Why:** An installed product command is incomplete if help, agents, and recovery docs
disagree.  
**Acceptance:** capabilities/doctor/help/docs/skill parity; recovery remains possible;
release-intent version included.

## 7. Acceptance Mapping

| Acceptance criterion | Steps | Validation |
|---|---|---|
| Command is recognized and explains aligned CLI/native-bundle update | 1, 3, 5 | Root/subcommand help and human preflight subprocess tests |
| Interactive confirmation occurs before network/mutation | 1, 3 | Pseudo-terminal accepted/declined tests; fake curl invocation log |
| Non-TTY without `-y` fails with exact next command | 1, 3 | Piped-stdin subprocess test; assert zero downloader calls |
| `-y` invokes released installer as `--agents -y` | 3, 4 | Fake curl/bash argv capture |
| Version pin and documented environment controls survive | 3, 4 | Child environment capture for CLI and inherited values |
| Idempotent/no-op and target states remain installer-owned | 4 | Fake unchanged/install output relayed verbatim; no Rust reinterpretation |
| Download, tool, shell, checksum/target/partial failures are nonzero | 3, 4 | Missing/failing fake tools and installer exit matrix |
| Installer failure cannot print success | 4 | Footer absence and nonzero assertions |
| Post-install version is observed at exact destination | 4 | Fake install destination binary and PATH-mismatch test |
| Restart/reload guidance appears only after success | 4 | Success/failure output count assertions |
| Check is read-only and network uncertainty stays unavailable | 2 | File snapshot/fake HOME plus successful/malformed/failing release lookup |
| Human hierarchy and next actions match MDP-202 | 2, 3, 5 | `cli_contract` output-order assertions |
| JSON check is one envelope; JSON mutation is rejected | 1, 2 | `json_stdout_contract` cases with empty stderr/no side effects |
| Capabilities, doctor, docs, and authored skill agree | 5 | Capability assertions, doctor tests, docs/skill validation |
| Tests never touch real network/home | all | Every mutating test uses isolated fake PATH/HOME/install dir and call logs |

## 8. Tests And Validation

### Focused during implementation

```bash
cargo test --manifest-path cli/Cargo.toml --test upgrade
cargo test --manifest-path cli/Cargo.toml --test cli_contract upgrade
cargo test --manifest-path cli/Cargo.toml --test json_stdout_contract upgrade
cargo test --manifest-path cli/Cargo.toml commands::upgrade
```

Use the actual test names introduced by implementation. Interactive confirmation
tests may use a Unix pseudo-terminal helper implemented with existing `libc`; do not
add a production dependency solely for testing.

### Repository regression

```bash
cargo test --manifest-path cli/Cargo.toml
bash -n scripts/install.sh
bash scripts/test-install.sh
bash scripts/test-release-install-smoke.sh
make validate-installers
make validate
```

If no installer source changes, run the installer suites as regression proof but do
not edit them to manufacture a seam.

### Manual source-tree smoke (non-mutating only)

```bash
cargo run --manifest-path cli/Cargo.toml -- upgrade --help
cargo run --manifest-path cli/Cargo.toml -- upgrade --check
cargo run --manifest-path cli/Cargo.toml -- --json upgrade --check
printf '' | cargo run --manifest-path cli/Cargo.toml -- upgrade
```

Do not run a real `mdp upgrade -y`, publish a release, or install into the active home
without separate authorization.

## 9. Compatibility And Migration

- Additive public command; all existing CLI syntax and artifact contracts remain.
- `mdp --version` remains unchanged. MDP-3 may later add richer version metadata.
- `mdp.doctor.v1` receives additive installation guidance fields only.
- `mdp.capabilities.v1` receives one command entry and explicit JSON-mode detail.
- Old installations still use the raw installer; docs retain that bootstrap path.
- The child installer receives the same documented environment controls it already
  supports. No new host path, release resolver, or target selector is invented.
- No pack/schema/data migration is required.

## 10. Risks And Safety Boundaries

| Risk | Mitigation |
|---|---|
| Download-and-execute supply-chain exposure | Fixed visible HTTPS origin; HTTPS-only curl; owned temp file; existing release checksum verification remains in installer |
| Confirmation after side effect | Mode gate and confirmation run before curl/temp execution; call-log tests prove ordering |
| Rust updater drifts from installer | Pass only `--agents -y` and `MDP_VERSION`; do not reproduce target/release/checksum logic |
| JSON corruption from child output | Reject mutating JSON mode before execution; support only check envelope |
| Partial installer result reported as success | Overall success depends solely on installer exit; never infer from requested version |
| Wrong binary used for post-install check | Resolve exact documented install destination, not ambient PATH |
| Temp-file substitution | Unique owned directory, create-new regular file, restrictive permissions, no caller path |
| Concurrent release version collision | Refresh main/open PRs immediately before bump; use next available version or stop for replan |
| Updated files mistaken for host reload | One explicit restart/reload reminder; no discovery claim |

No release publication, active-home installation, host restart, merge, deployment,
outreach, CRM mutation, credential access, or production mutation is authorized.

## 11. Rollout, Observability, And Rollback

- Deliver one feature PR from `codex/mdp-4-aligned-upgrade` to `main` and stop at
  Ready for Human.
- Observable proof is Clap help, `mdp.upgrade-check.v1`, capabilities/doctor output,
  fake subprocess logs, child exit propagation, docs/skill validation, and full CI.
- A merge completes the coding task but does not publish or install the release.
- Rollback is a normal revert of the additive command/module/tests/docs and synchronized
  version bump before release. No data or pack rollback is needed.
- After a separately authorized release, manual proof must run from an isolated or
  disposable environment before any active-home upgrade claim.

## 12. Blockers And Readiness Verdict

MDP-202 and MDP-6 are shipped. The MDP-306 installer implementation is present on
current `main` and MDP-4 has no `blockedBy` relation, so its stale Linear lifecycle
status is a closeout inconsistency rather than a code dependency blocker. Repository,
source origin, command behavior, JSON boundary, test seam, compatibility, rollback,
and ownership are resolved.

Immediately before implementation, refresh `origin/main` and open PRs. If another
change has consumed `0.1.112` or altered the installer invocation/result contract,
stop and repin rather than silently changing this plan.

**Readiness verdict: `READY_TO_PIN`**
