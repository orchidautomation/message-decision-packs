# MDP-283 — Faster release and authority-mutation workflows

Status: `READY_TO_PIN`

## 1. Context and current behavior

MDP publishes three native CLI artifacts from `.github/workflows/release.yml`:
Linux x86_64, Intel macOS x86_64, and Apple Silicon macOS arm64. The three
build jobs run in parallel, but the publish job cannot start until all three
finish. Release v0.1.94 run
[`33188192296`](https://github.com/orchidautomation/message-decision-packs/actions/runs/33188192296)
completed successfully in 13m 8s. Its critical path was the cold
`macos-15-intel` Rust build at 10m 20s, compared with 2m 23s on Linux and 2m
57s on Apple Silicon. None of the release build jobs restores a Rust/Cargo
cache.

After the matrix completes, `github-release` installs pinned Pluxx and Codex
packages, builds and tests all Pluxx bundles, publishes and re-downloads the
release assets, finalizes checksums and the manifest, and runs
`scripts/release-install-smoke.sh`. That smoke currently requires
`cargo build --manifest-path cli/Cargo.toml` solely to provide
`cli/target/debug/mdp` to the route-budget source-versus-installed parity test.
The same exact source commit already produced the staged Linux release binary,
which is downloaded into `release-assets/` and is compared byte-for-byte with
the installed binary earlier in the smoke.

`.github/workflows/authority-mutations.yml` runs a pinned `cargo-mutants`
binary and then two Ubuntu shards. `scripts/test-authority-mutations.sh`
selects at most 24 mutations in `cli/src/authority/mod.rs`; each shard compiles
and tests roughly half of those mutations in a disposable checkout. The main
run for release commit `0c27043da4c906f857a8438de2be4e88a09b83ec`,
[`33187539176`](https://github.com/orchidautomation/message-decision-packs/actions/runs/33187539176),
showed shard 0/2 taking 32m 29s while shard 1/2 was still executing after 33
minutes. The workflow has no Rust/Cargo cache and each shard has a 40-minute
timeout.

Confirmed constraints:

- `rust-toolchain.toml` pins Rust 1.88.0 with the minimal profile.
- `scripts/test-release-workflow.mjs` fails closed if the published-install
  smoke or its source-parity setup is bypassed.
- `scripts/release-install-smoke.sh` already supports staged CLI parity via
  `MDP_RELEASE_REQUIRE_STAGED_PARITY=1` and verifies released checksums,
  manifests, installed plugin trees, authority conformance, schemas, native
  parity, MCP behavior, and route-budget parity.
- Mutation selection is intentionally focused and capped; optimization may not
  omit candidates or treat a surviving mutant as success.

## 2. Objective, scope, assumptions, and exclusions

### Objective

Reduce the time Brandon waits for MDP release and authority-mutation proof while
preserving every release platform, artifact, checksum, installation check, and
focused mutation candidate.

### In scope

- Add target-safe Rust/Cargo caching to the release matrix and mutation jobs.
- Reuse the staged Linux release binary as the source binary for installed
  route-budget parity instead of compiling an additional debug CLI in the
  publish job.
- Expand the focused mutation matrix from two balanced shards to four when
  measured evidence shows that this is required to meet the wall-clock target.
- Add deterministic workflow-contract tests for cache isolation, staged source
  parity, shard coverage, timeouts, and required fail-closed steps.
- Record exact before/after GitHub Actions run and job timings in MDP-283 and
  the delivery PR.

### Out of scope

- Removing Linux, Intel macOS, or Apple Silicon release artifacts.
- Changing MDP runtime behavior, schemas, packs, templates, profiles, or public
  distribution URLs.
- Reducing the mutation selector, candidate cap, or failure semantics.
- Publishing a test tag or release; any future tag/release needs separate
  action-time authorization.
- Replacing GitHub Actions, Rust, `cargo-mutants`, Pluxx, or the installer.

### Assumptions to verify during implementation

- The staged `release-assets/mdp-x86_64-unknown-linux-gnu` binary is a valid
  source-parity executable on the Ubuntu publish runner after `chmod +x`.
- `cargo-mutants` 27.1.0 applies `--shard N/M` consistently to `--list` and
  execution, allowing a deterministic union/no-duplicate coverage check.
- GitHub-hosted cache restore/save overhead is lower than the cold dependency
  compilation it replaces. If measurement disproves this for any job, retain
  only the beneficial cache boundary and record the evidence.

## 3. Acceptance mapping

| Acceptance criterion | Implementation steps | Validation |
|---|---|---|
| Warm-cache release under 8 minutes, or a documented hosted-runner floor | A, B, E | Exact release-matrix timing from a separately authorized tagged run; PR CI proves workflow contracts without publishing |
| Intel macOS materially faster than the 10m 20s baseline | A, E | Compare exact Intel job start/end and build-step duration to run `33188192296` |
| Authority mutations under 20 minutes, or justified lower bound | C, D, E | Exact aggregate and shard timing from the implementation PR and main run |
| Cache cannot mix incompatible artifacts | A, C, D | Static contract tests require OS, target/shard, Rust toolchain, and `cli/Cargo.lock` boundaries; cold/warm runs remain green |
| Release checksums, manifest, and published install proof remain intact | B, D | `node scripts/test-release-workflow.mjs`; existing release CI steps remain unconditional; next authorized release is the final external proof |
| Mutation coverage and fail-closed behavior are unchanged | C, D | Full candidate list equals the disjoint union of shard lists; mutation aggregate requires all shards to succeed |
| Before/after evidence is durable | E | PR body and MDP-283 closeout include exact run URLs, commit, job durations, cache hit/miss result, and residual floor |

## 4. Affected files and responsibilities

### Implementation-owned paths

- `.github/workflows/release.yml`
  - Current: builds three release binaries, publishes assets, and runs the
    installed-release smoke.
  - Change: restore/save target-specific Rust caches; pass the staged Linux CLI
    to the smoke instead of compiling a new debug CLI; keep publish and smoke
    steps sequential and unconditional.
- `.github/workflows/authority-mutations.yml`
  - Current: installs one pinned mutation tool and runs two uncached shards.
  - Change: add dependency/target caching, use the measured shard count, keep
    the aggregate gate, and retain explicit timeouts.
- `scripts/release-install-smoke.sh`
  - Current: hard-codes `cli/target/debug/mdp` as the source route-budget
    parity binary.
  - Change: accept a bounded `MDP_RELEASE_SOURCE_PARITY_BIN` override, require
    an executable file, and default to the existing debug path for local
    compatibility.
- `scripts/test-release-workflow.mjs`
  - Current: requires an unconditional source debug build before smoke.
  - Change: require the exact staged-source binding and cache isolation while
    continuing to reject bypasses, alternate shells, `continue-on-error`,
    missing pinned tooling, or reordered publish/smoke proof.
- `scripts/test-authority-mutations.sh`
  - Current: accepts only `0/2` or `1/2` and runs the focused selector.
  - Change: accept only the explicit supported shard topology, preserve the
    total candidate cap, and expose deterministic listing needed by the
    coverage contract.
- `scripts/test-authority-mutations-contract.mjs` (new, if no equally small
  existing test seam is preferable)
  - Prove the workflow matrix, aggregate dependencies, timeouts, selector,
    candidate cap, and disjoint/exhaustive shard contract.
- `Makefile` and `.github/workflows/ci.yml`
  - Change only if needed to place the new workflow-contract test in the
    existing named validation gate and path filter.

### Forbidden paths and effects

- No changes under `cli/src/**`, `plugin/**`, `assets/**`, schemas, examples,
  public docs, version files, installers, or generated host bundles unless a
  discovered test-contract dependency makes a minimal change necessary and Sol
  approves it before widening ownership.
- No tag, GitHub release, deployment, installation on Brandon's machine,
  protected-branch push, merge, or external mutation beyond the linked Linear
  lifecycle evidence and one PR.

## 5. Ordered implementation plan

### Step A — Add target-safe release caching

Add a maintained Rust cache action after the pinned toolchain setup in each
release matrix job. Scope its key/input by runner OS, matrix target,
`rust-toolchain.toml`, and `cli/Cargo.lock`; point it only at the CLI workspace
and target directory needed for that target. Do not share compiled target
artifacts between Intel and ARM macOS. Keep the artifact copied from
`cli/target/<target>/release/mdp` and all upload names unchanged.

Why: the slow job is a cold Intel compile on the release critical path. Safe
dependency and target reuse attacks that bottleneck without changing the
artifact contract.

### Step B — Remove the redundant publish-job CLI build

Add `MDP_RELEASE_SOURCE_PARITY_BIN` to the documented environment contract in
`scripts/release-install-smoke.sh`. Resolve it to an absolute/existing
executable and otherwise retain `cli/target/debug/mdp` as the local default.
In the release workflow, pass
`release-assets/mdp-x86_64-unknown-linux-gnu` while retaining
`MDP_RELEASE_REQUIRE_STAGED_PARITY=1`.

Update `scripts/test-release-workflow.mjs` so it requires this exact staged
binding before `scripts/release-install-smoke.sh`, rejects a missing or
untrusted alternate path, and no longer requires the redundant `cargo build`.
The smoke must still compare the staged binary to the installed published
binary and run source-assets-versus-installed-assets route-budget parity.

Why: this reuses an exact-commit release artifact already downloaded into the
job while preserving stronger parity than a second debug build.

### Step C — Shorten authority mutations without weakening them

First add a cache boundary for the CLI workspace keyed by Ubuntu, Rust 1.88.0,
`cli/Cargo.lock`, the mutation-tool version, and shard topology. Do not cache
mutated source files or a workspace outside `cli/target`; each job remains a
fresh checkout and `--in-place` remains isolated.

Change the supported topology to four shards (`0/4` through `3/4`) if local
candidate listing and the prior 32+ minute two-shard baseline show a roughly
balanced split. Keep `fail-fast: false` for full diagnosis and keep the final
aggregate job requiring the entire matrix result to be `success`.

Why: caching removes repeated dependency work, while four isolated shards
reduce the wall-clock critical path without deleting mutations. Increased
runner usage is an explicit cost tradeoff in favor of Brandon's shipping time.

### Step D — Make optimization contracts fail closed

Extend the release workflow test and add the smallest authority workflow test
needed to prove:

- release cache isolation includes OS/target/toolchain/lockfile inputs;
- the publish job consumes the staged Linux binary for source parity;
- checksum, manifest, publish, and install-smoke steps remain required;
- the mutation matrix contains every supported shard exactly once;
- the union of shard candidates equals the unsharded focused candidate list
  with no duplicates;
- `MAX_CANDIDATES=24`, selector `(from_run|permits_projection)`, build/test
  timeouts, in-place isolation, and the aggregate success requirement remain;
- comments, echoes, conditional bypasses, alternate shells, and
  `continue-on-error` cannot satisfy required steps.

Wire these tests into existing CI and path filters only where necessary.

### Step E — Measure, deliver, and stop at human merge

Run focused local contract tests, push one MDP-283 implementation branch, and
open one validated PR. Use PR GitHub Actions to record cold/partial-cache
authority timings. If the authority target is missed, inspect per-shard
candidate counts and build/test timing before changing the selector; only
cache scope or shard balance may change without a new decision.

A PR cannot execute the tag-only release workflow. Therefore release contract
tests plus ordinary CI are the pre-merge proof; the next separately authorized
real release supplies the warm-cache release timing and published-install
proof. Record that deferred measurement truthfully rather than creating a test
release. Stop at Ready for Human; Brandon alone merges.

## 6. Tests and validation

Focused local checks:

```bash
git diff --check
node scripts/test-release-workflow.mjs
node scripts/test-authority-mutations-contract.mjs
bash scripts/test-authority-mutations.sh --help
```

If the implementation chooses a different test filename or exposes a dedicated
list-only flag, update the exact commands while preserving the proof. Do not
run the full mutation execution locally merely to duplicate GitHub Actions.

Repository regression checks:

```bash
make validate-version-sync
make validate-installers
```

Required GitHub evidence on the exact PR head:

- standard CI workflow contract tests green;
- authority mutation tool, every shard, and aggregate gate green;
- exact shard start/end times and candidate counts recorded;
- no skipped required job caused by a missing path-filter entry.

Required later release evidence, only after separate authorization:

- all three native artifacts build;
- Intel macOS duration compared with 10m 20s baseline;
- GitHub release assets, `SHA256SUMS.txt`, `MDP_CLI_SHA256SUMS.txt`, and
  `release-manifest.json` validate;
- published installer smoke and staged parity pass;
- total release duration compared with 13m 8s baseline.

## 7. Compatibility and migration behavior

- Release tag/version matching, artifact names, target triples, checksums,
  manifest schema, public release URLs, and installer arguments do not change.
- `MDP_RELEASE_SOURCE_PARITY_BIN` is additive. Local callers that omit it keep
  the current `cli/target/debug/mdp` behavior.
- `MDP_RELEASE_REQUIRE_STAGED_PARITY=1` retains its current meaning and remains
  mandatory in release CI.
- Mutation selection, maximum candidate count, cargo-mutants version, file
  target, regex selector, and fail-closed aggregate result do not change.
- No data migration or user-facing configuration change exists.
- Hosted execution uses Orchid with temporary compatibility label
  `delegate:codex` until the deployed Eve runtime accepts `delegate:agent`.

## 8. Risks, safety, rollout, observability, and rollback

- **Cache poisoning or incompatibility:** isolate by OS, target, toolchain,
  lockfile, and workflow purpose; cache build outputs only, never source,
  release assets, credentials, or mutation worktrees.
- **False speed from skipped proof:** contract tests must keep publish,
  checksum, install, parity, all shard, and aggregate gates unconditional.
- **Mutation imbalance:** list and record per-shard counts; adjust topology,
  not selector coverage.
- **Higher Actions consumption:** four mutation runners trade compute minutes
  for lower operator wait. Record total compute as well as wall-clock time.
- **Misleading release target:** PR CI cannot prove a tag-only warm-cache run.
  Mark the release-duration criterion pending until the next authorized release.
- **Third-party action risk:** use an established maintained cache action and
  follow the repository's existing action-version policy; do not introduce
  credentials or write permissions.

Rollout is one workflow-only PR. Observe cache hit/miss output and exact job
durations on its authority run, then observe the next approved release. Roll
back by reverting the PR; the prior workflows and local smoke default remain
available without data cleanup. If caching behaves incorrectly, disable only
the affected cache step while keeping all proof gates active.

## 9. Blockers and readiness verdict

The repository, exact baseline commit, affected workflows/scripts, acceptance
criteria, validation, compatibility behavior, and rollback are known. The
issue has no external dependency and no product decision is required. The
plan intentionally defers a real tag/release measurement because that action
requires separate authorization; this does not block implementation or PR
validation.

Implementation route: one Elevated, plan-pinned Orchid lane in one dedicated
worktree and branch, owning only the paths listed above, producing one validated
PR to `main`, with no autofix enrollment and Brandon-only merge.

Readiness: `READY_TO_PIN`.
