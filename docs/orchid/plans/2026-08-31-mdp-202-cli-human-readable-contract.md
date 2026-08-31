# MDP-202: Human-readable, contract-consistent CLI

## Goal Capsule

Make the existing local/offline `mdp` CLI understandable on first contact while
preserving its current machine contract. A human should be able to run `mdp`,
identify the active pack with `mdp status`, understand the result of common
commands, and copy the next safe command. An agent should continue to receive
exactly one JSON value on stdout for every `--json` path.

This plan is the implementation authority for MDP-202. It supersedes the stale
v0.1.62-era plan from PR #160. Current `main` already implements the global JSON
stdout invariant, presentation-conflict matrix, actionable diagnostics, parse
error classification, and doctor/validation exit behavior. Do not rewrite or
version those contracts again.

## Product Contract

- MDP remains local/offline. `status` performs no network, auth, update, or host
  discovery and never adds `login` or `whoami` language.
- Existing flat command names and existing successful JSON data contracts remain
  compatible. `mdp --version` remains unchanged; MDP-3 owns a future `version`
  command and MDP-4 owns upgrade visibility.
- Human output is a projection, never a second authority. Full structured detail
  remains available with `--json`; deliberate Markdown/YAML/Mermaid artifacts
  keep their explicit human modes.
- `--json` continues to produce one parseable envelope on stdout with empty
  stderr. Existing output-mode conflicts remain conflicts rather than silently
  changing the meaning of `--readable` or `--format`.
- Validation-style commands continue to use nonzero process status when their
  domain gate fails. `status` is observational and exits zero for missing or
  unhealthy local context while reporting the state and next action.

## Current Baseline

At source commit `ea077b796ca0a9fce39b45d1708007115e9430ab`:

- bare `mdp` exits 2 and prints the entire advanced command inventory as an
  error;
- `mdp --json` also exits 2 instead of returning an observational result;
- `status` is not registered;
- `doctor`, `validate`, and `check` have intentional human summaries, but the
  fallback renderer still pretty-prints raw JSON for many ordinary commands;
- human `--summary` prints JSON and can retain unavailable/null fields;
- Clap/capabilities parity and JSON stdout purity already have substantial test
  coverage in `cli/tests/json_stdout_contract.rs` and must be extended, not
  replaced.

## Implementation Units

### 1. Status and first-contact path

Add `Commands::Status { --dir }` and a read-only `mdp.status.v1` projection in a
focused command module. Reuse `doctor()` and the existing manifest/profile/target
facts; do not create a parallel validator. The projection must include:

- CLI version;
- `mode: "local-offline"` and `auth_required: false`;
- requested pack root and whether `.mdp/manifest.yaml` was observed;
- pack ID/name, profile ID, and target identity only when available;
- a closed health state (`ready`, `needs-input`, `blocked`, or `invalid`), first
  blocker/diagnostic when present, and one exact safe next command.

Make `Cli.command` optional. With no command, human mode prints a concise
outcome-first quickstart covering the two journeys:

1. Author: initialize/select a pack, inspect status, validate.
2. Use: select an exact job, check readiness, then request the relevant result.

Bare `mdp --json` and `mdp --json --summary` must dispatch to the same status
projection for `--dir .`; bare human `mdp` prints the quickstart. All three exit
zero and write nothing.

### 2. Intentional human projections

Keep existing special renderers and explicit rich artifacts. Replace the generic
pretty-JSON human fallback with a stable concise renderer derived from the
existing command summary projection:

- print `command: <state>` first, choosing the first meaningful available state
  from `status`, `decision`, `valid`, `available`, or artifact disposition;
- print a bounded set of non-null scalar/count facts in stable order;
- print the first blocker/issue when present;
- end with the exact domain-provided next command/action when one exists;
- never dump nested entry bodies, prompts, context, receipts, or full artifacts.

Human `--summary` must use the same concise text renderer, not serialized JSON.
Recursively omit null object fields from summary projections while preserving
false, zero, empty arrays, and array positions. JSON summary envelopes remain
compatible apart from removal of meaningless null object members.

Rich outputs (`render-brief`, readable brief/proof, trace Mermaid, sample YAML)
remain deliberate artifact paths and continue to obey the existing presentation
conflict matrix.

### 3. Discoverable help and truthful capabilities

Give root help an outcome-first description, `after_help` quickstart, and visual
grouping through Clap help headings without changing the flat parser surface.
At minimum distinguish Start, Inspect, Decide, Produce/Verify, and Advanced.
Keep advanced commands discoverable without letting them dominate the first
screen.

Audit public arguments lacking help text and add plain descriptions/value names,
prioritizing every `--dir`, `--job`, `--persona`, `--file`, identifier, output,
and presentation option. `--job` help must say when it requires the exact
canonical `jobs[].id`. Examples must be copy-pasteable or explicitly mark
placeholders.

Register `status` in capabilities and expose its observational exit semantics.
Update capability/help parity tests. Do not add a second hand-maintained syntax
graph; Clap remains authoritative.

### 4. Executable contract coverage

Extend process-level tests (prefer `cli/tests/cli_contract.rs` plus focused
additions to `json_stdout_contract.rs`) to cover:

- no-argument human quickstart and no-argument JSON status;
- status for valid, missing, and malformed packs, including no writes;
- root help hierarchy and representative option help;
- default human output for representative inspect, decision, artifact, and
  validation commands without raw pretty-JSON fallback;
- human and JSON `--summary`, including recursive null omission;
- existing readable/format conflicts, parse/runtime failures, domain-invalid
  results, stdout/stderr, and exit codes;
- status/help presence in the generated capabilities graph and compatibility
  command metadata.

Fixtures must be synthetic and bounded. Tests should assert semantic lines and
fields rather than snapshotting the entire large command inventory.

### 5. Documentation, authored skill, and release version

Update only contract-affected surfaces:

- `README.md` and `docs/getting-started.md`: human first-contact path, explicit
  offline/no-auth posture, and agent JSON path;
- `cli/USAGE.md`: status/no-argument behavior, output/exit contract, canonical
  job-ID wording, and current target-aware init example;
- `plugin/skills/mdp/SKILL.md` and
  `plugin/skills/mdp/references/cli-operator.md`: status first for humans,
  capabilities first for agents, and no inference from human summaries;
- `cli/Cargo.toml`, `cli/Cargo.lock`, and the README release callout: bump the
  patch version from 0.1.103 to 0.1.104 because this PR has explicit release
  intent. Do not publish or install the release in this task.

Do not modify generated host bundles; `plugin/skills/` is the authored source.

## Ownership Boundaries

The implementation lane owns:

- `cli/src/cli.rs`, `cli/src/main.rs`, `cli/src/app.rs`, `cli/src/output.rs`;
- `cli/src/commands/status.rs`, `cli/src/commands/mod.rs`, and the minimal shared
  health/capabilities seams required by status;
- CLI subprocess/unit tests and synthetic fixtures;
- the five contract-affected docs/skill files named above;
- `cli/Cargo.toml` and `cli/Cargo.lock` for the patch version.

It must not edit pack schemas, templates, examples, native runner behavior,
provider/MCP code, release workflows, installer logic, generated plugin bundles,
other Orchid plans, or unrelated issue work. If the current code makes the
status projection or human fallback unsafe without crossing those boundaries,
stop and escalate rather than broadening scope.

## Acceptance Criteria

1. `mdp` exits 0 with a concise quickstart and runnable next commands.
2. `mdp status` is read-only, works in human/full JSON/summary JSON modes, and
   reports local/offline/auth/pack/profile/target/health context truthfully.
3. Bare `mdp --json` is one status envelope with empty stderr and exit 0.
4. Missing or malformed pack status remains observational (exit 0) while
   `doctor`/validation remain explicit failing gates.
5. Representative ordinary human commands no longer fall through to raw pretty
   JSON; explicit rich artifacts remain unchanged.
6. Human summaries are concise; JSON summaries omit null object fields without
   deleting meaningful false/zero/empty values.
7. Every tested JSON path remains exactly one envelope on stdout, including
   presentation conflicts and parse/runtime/domain failures.
8. Root/subcommand help explains the common workflow and canonical identifier
   semantics while preserving all flat commands.
9. Capabilities, docs, authored skill guidance, exit codes, stdout, and stderr
   agree with executable behavior.
10. Version 0.1.104 is committed with the feature; no release, install, merge,
    or production mutation occurs.

## Verification Contract

Run from the repository root:

```bash
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
make validate
git diff --check
```

Capture bounded manual proof for:

```bash
cargo run --manifest-path cli/Cargo.toml --
cargo run --manifest-path cli/Cargo.toml -- status --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json status --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json
cargo run --manifest-path cli/Cargo.toml -- --summary doctor --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json --summary doctor --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json verify-output --readable --dir plugin/assets/templates/proposal --file missing.json
```

The final head must have one passing acceptance-mapped verification receipt and
one Elevated review receipt bound to the same commit. Deliver one PR to `main`
and stop at Ready for Human; Brandon alone decides whether to merge.
