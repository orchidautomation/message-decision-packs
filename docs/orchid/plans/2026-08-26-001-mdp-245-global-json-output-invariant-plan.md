---
title: MDP-245 Global JSON Output Invariant - Implementation Plan
type: bug
date: 2026-08-26
topic: global-json-output-invariant
execution: orchid
artifact_contract: orchid-plan/v1
artifact_readiness: implementation-ready
linear_issues:
  - MDP-245
  - MDP-247
---

# MDP-245 Global JSON Output Invariant - Implementation Plan

## Context and current behavior

At planning base `5aaaf850b24b57622aca118da84cf02649380ab7`, `cli/src/cli.rs::Cli` defines global `--json` and `--summary`. Most commands use `cli/src/output.rs::print_output`, which writes a single success envelope, while `print_error` writes a single JSON error in JSON mode.

The invariant is bypassed by direct presentation branches in `cli/src/app.rs`. `Commands::VerifyOutput { readable: true }` prints Markdown even when global `--json` is present. Other human format requests—trace Mermaid, render-brief Markdown, sample-leads YAML, and brief readable—currently have inconsistent precedence or are silently ignored in JSON mode. `cli/src/main.rs` also lets Clap `--help` and `--version` display plain text when combined with `--json`.

`cli/src/commands/capabilities.rs::capabilities` declares command flags and hard-codes JSON support, but has no authoritative presentation compatibility matrix, stdout/stderr guarantee, conflict code, or help/version behavior.

## Objective, scope, and pinned policy

Guarantee that every invocation containing global `--json` writes exactly one parseable JSON value to stdout on success and failure.

Pinned policy:

- `--json --summary` is valid and produces the existing JSON summary envelope.
- `--json` combined with a command-level human-only presentation request (`--readable`, Mermaid, Markdown, or YAML) fails nonzero with one JSON error carrying stable code `output_mode_conflict`; stderr stays empty.
- `--json --help` and `--json --version` succeed and wrap the display text in a JSON envelope rather than emitting raw text.
- No presentation flag may be silently ignored.
- Human-only behavior without `--json` remains unchanged.

Out of scope: redesigning human copy, changing command decision semantics, implementing the broader generated-capabilities work in MDP-247, or changing non-presentation payload contracts.

## Acceptance mapping

| Acceptance criterion | Implementation | Validation |
|---|---|---|
| Every `--json` invocation writes one JSON value | Centralize early display handling and eliminate direct non-JSON stdout paths after JSON mode is selected. | Process-level table parses stdout with `serde_json::from_slice`, which rejects preludes and trailing text. |
| Conflicts are stable or enveloped | Apply the pinned conflict/display policy above with `output_mode_conflict`. | Matrix asserts exit status, code/envelope, and empty stderr. |
| No diagnostic prelude/trailing text | Route JSON success/errors through one emitter and keep Clap/raw rendering inside envelope data. | Include runtime errors, invalid args, help, version, and successful commands. |
| Every public presentation combination is covered | Define one typed presentation compatibility table shared by validation/capabilities and exhaust it in integration tests. | JSON × summary × readable/format-value table for all public presentation flags. |
| Capabilities describe conflicts exactly | Project the same table, stable code, envelope shape, stdout/stderr, and exit policy from capabilities. | Unit test equality/completeness against the table. |

## Affected files and symbols

- `cli/src/cli.rs`
  - `Cli`, command presentation fields, and format enums.
  - Add typed presentation intent/validation without relying on derive conflicts for value-dependent formats.
- `cli/src/app.rs`
  - `run`, trace/verify-output/render-brief/brief branches, and `print_sample_leads`.
  - Human direct rendering occurs only after the compatibility gate permits it.
- `cli/src/main.rs`
  - Raw `--json` detection and Clap `DisplayHelp`/`DisplayVersion` handling.
  - Wrap display text as a successful JSON display envelope.
- `cli/src/output.rs`
  - `print_output`, `print_error`, and `classify_error` or an equivalent typed error path.
  - Add stable `output_mode_conflict` handling and exactly-once serialization.
- `cli/src/commands/capabilities.rs`
  - `global_options`, command metadata, stable error codes, and capabilities tests.
  - Generate presentation contract fields from the shared typed table.
- `cli/tests/json_stdout_contract.rs` (new)
  - Process-level exhaustive matrix using `CARGO_BIN_EXE_mdp`.

Forbidden without replanning: `cli/src/commands/init.rs`, `cli/src/pack_io.rs`, starter/template content, routing/decision semantics, and MDP-247-specific capability generation beyond this presentation contract.

## Ordered implementation steps

1. Inventory every public presentation selector: global `--summary`; trace `--format`; verify-output `--readable`; render-brief `--format`; sample-leads `--format`; brief `--readable`; plus help/version display actions.
2. Add a typed compatibility function/table that resolves the effective presentation from parsed CLI state. It must distinguish JSON-compatible summary from human-only format values and return a typed `output_mode_conflict` before command work or writes.
3. Invoke the gate once before dispatch. Remove raw Markdown/YAML/Mermaid paths from JSON mode and preserve them only for validated human mode.
4. Refactor JSON output/error helpers only as needed so every path serializes once, writes stdout once, and leaves stderr empty. Preserve the specialized `prepare-run` blocked envelope unless the shared table proves a compatibility break.
5. In `main.rs`, intercept Clap help/version when raw args contain `--json` and emit `{ok:true, command:"help|version", data:{text:...}}`; ordinary help/version remains plain text.
6. Expose the exact presentation contract from capabilities: selectors and allowed values, conflict disposition/code, help/version display envelopes, exactly-one-JSON stdout guarantee, stderr policy, exit behavior, and `--summary` compatibility.
7. Add a table-driven process integration test covering flags before/after subcommands where Clap permits them, successful and failing commands, the verified proof-output readable regression, all format values, help/version, and invalid arguments. Assert stdout parses as one JSON value and JSON mode emits no stderr.
8. Add focused parser, output-classification, and capabilities completeness tests. Do not duplicate the matrix in free-form metadata.

## Tests and validation

Focused commands:

```bash
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo test --manifest-path cli/Cargo.toml --test json_stdout_contract
cargo test --manifest-path cli/Cargo.toml cli::tests
cargo test --manifest-path cli/Cargo.toml commands::capabilities::tests
cargo test --manifest-path cli/Cargo.toml output::tests
```

Exact-head regression and lint:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo clippy --manifest-path cli/Cargo.toml --all-targets -- -D warnings
```

Manual synthetic proof:

```bash
cargo run --manifest-path cli/Cargo.toml -- --json capabilities | jq -e .
cargo run --manifest-path cli/Cargo.toml -- --json --help | jq -e .
```

Also run a representative conflicting readable invocation and assert nonzero exit, `output_mode_conflict`, parseable stdout, and empty stderr.

## Compatibility, migration, rollout, and rollback

- Previously tolerated ambiguous combinations now fail explicitly instead of silently choosing a presentation. The stable code and capabilities matrix are the compatibility notice.
- Human-only invocations and JSON payload contracts remain unchanged.
- MDP-245 owns the presentation matrix and metadata. MDP-247 must consume it after this issue lands rather than creating a parallel source.
- No data/schema migration is required. Delivery is one cumulative foundation PR; release remains separate.
- Rollback is a normal PR revert.

## Risks and safety boundaries

- Clap display errors are represented as errors internally even for successful help/version; preserve exit zero in the JSON wrapper.
- Do not write a conflict diagnostic after a command has already produced output.
- Do not emit JSON diagnostics to stderr or human diagnostics to stdout in non-JSON error mode.
- Capabilities and runtime must share one source; hand-maintained duplicate matrices will drift.
- Keep integration fixtures synthetic and bounded.

## Blockers and readiness verdict

The two issue-level alternatives are resolved by the pinned policy above. Repository paths, symbols, compatibility behavior, downstream ownership, and tests are identified. No dependency blocks execution.

**Verdict: `READY_TO_PIN`.**
