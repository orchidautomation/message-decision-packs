---
title: MDP-43 Pack Migration Command and Contract Drift Plan
type: docs
date: 2026-07-02
topic: mdp-migrate-pack-contract-drift
execution: knowledge-work
linear_project: MDP: Domain Profile Foundation
linear_issues:
  - MDP-36
  - MDP-38
  - MDP-39
  - MDP-43
  - MDP-52
origin: MDP-43
---

# MDP-43 Pack Migration Command and Contract Drift Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Design `mdp migrate` as the deterministic local command for pack contract upgrades and same-format drift repair. |
| Product authority | MDP-43 owns pack migration UX and JSON contracts; MDP-38 and MDP-39 own the current profile and primitive-map compatibility decisions. |
| First slice | Ship a preview-first migration framework now, before `mdp.v1`, because installed `mdp.v0` packs can already drift from newer CLI/template validation contracts. |
| Safety posture | `mdp migrate --dir .` and `mdp migrate --dir . --dry-run` are read-only plan commands. File writes require explicit `--apply`. |
| Migration boundary | Migrations may patch deterministic pack contract fields, prompt examples, manifest profile metadata, and validation-compatible references. They must not rewrite messaging strategy, invent cards, replace custom content, enrich data, or call the network. |
| Current format stance | Keep `format: mdp.v0` for the accepted profile/primitive-map path. Same-format contract migrations are first-class and do not imply a pack-format bump. |

---

## Problem Frame

The original migration question was whether MDP needs something like `mdp --migrate` when moving from one pack version to another. The accepted direction is a subcommand, `mdp migrate`, scoped to pack contract migration rather than CLI/plugin self-update.

The 2026-07-02 Trellis/GTM drift investigation makes the first real use case sharper: an existing customized pack can still be `format: mdp.v0` and preserve the right GTM/account-qualification logic, while falling behind newer CLI/template contracts such as:

- missing `profile.agent_surface` metadata, causing `mdp --json agent-surface --dir .` to report legacy routing;
- stale prompt-output examples that no longer satisfy current prompt-output validators;
- evidence or provenance references that were valid when authored but are now incomplete under stricter local validation.

This is not a full `mdp.v0 -> mdp.v1` migration. It is same-format contract drift. The migration framework should handle both classes with one registry and one JSON contract.

## Version Taxonomy

| Version surface | Owner | Migration behavior |
|---|---|---|
| CLI binary version, for example `mdp 0.1.26` | Release/install process | Out of scope for `mdp migrate`; belongs to installer and future `mdp upgrade` surfaces. |
| Pack content `version` in `.mdp/manifest.yaml` | Pack owner | Do not silently rewrite; it describes user/team content revision, not CLI compatibility. |
| Pack `format`, currently `mdp.v0` | MDP format contract | Use for breaking shape changes that require ordered format migrations. |
| Same-format contract baseline | MDP CLI/template validators | Use for deterministic drift repairs inside the same `format`, such as profile metadata and prompt-output fixture compatibility. |
| Template fingerprint | Starter/template registry | Advisory detection input only; never overwrite custom cards or strategy with starter defaults. |

The first implementation should introduce a migration registry with two migration families:

- `format`: ordered pack-format upgrades such as future `mdp.v0 -> mdp.v1`.
- `contract`: same-format patch sets such as `mdp.v0/gtm-agent-surface-v0` or `mdp.v0/prompt-output-example-v0`.

## Command UX

Prefer a subcommand and keep it local/offline:

```bash
mdp migrate --dir .
mdp migrate --dir . --dry-run
mdp migrate --dir . --to mdp.v1
mdp migrate --dir . --apply
mdp --json migrate --dir .
mdp --json migrate --dir . --apply
```

Behavior:

- `mdp migrate --dir .` is a read-only migration plan. It exits successfully for `up_to_date` and `migration_available`.
- `--dry-run` is accepted as an explicit alias for the default preview behavior, so scripts and agents can communicate intent.
- `--apply` is required for writes.
- `--to <format>` selects a target format; when omitted, target the latest supported format plus applicable same-format contract patches.
- `--format-only` may be added later if users need to suppress same-format contract patches, but the first slice should not add it unless implementation pressure proves it necessary.
- `--json` emits the same `mdp.migrate.v0` envelope regardless of dry-run or apply mode.

Human output should lead with status, source/target, planned operations, files touched, warnings, and next validation commands. JSON output is the source of truth for agents.

## JSON Contract

`mdp.migrate.v0` should preserve the existing CLI envelope:

```json
{
  "ok": true,
  "command": "migrate",
  "data": {
    "contract": "mdp.migrate.v0",
    "status": "migration_available",
    "dry_run": true,
    "applied": false,
    "source": {
      "format": "mdp.v0",
      "content_version": "0.1.0",
      "profile_id": "gtm",
      "template_hint": "gtm",
      "legacy_profile": true
    },
    "target": {
      "format": "mdp.v0",
      "contract_baseline": "mdp.contract.gtm.v0"
    },
    "migration_plan": [
      {
        "id": "mdp.v0.gtm.agent-surface.v0",
        "kind": "contract",
        "status": "planned",
        "risk": "low",
        "description": "Add missing profile.agent_surface routing metadata without changing cards.",
        "files": [".mdp/manifest.yaml"],
        "operations": [
          {
            "op": "yaml_set_if_missing",
            "path": ".mdp/manifest.yaml#/profile/agent_surface",
            "preserves_existing": true
          }
        ]
      }
    ],
    "files_touched": [".mdp/manifest.yaml"],
    "warnings": [],
    "validation_commands": [
      "mdp --json validate --dir .",
      "mdp --json eval --dir ."
    ]
  }
}
```

Stable statuses:

| Status | Meaning |
|---|---|
| `up_to_date` | Current pack format and same-format contract checks require no migration. |
| `migration_available` | A deterministic migration path exists; the plan may be preview-only or ready for `--apply`. |
| `unsupported_format` | The CLI cannot read or migrate the declared format. |
| `blocked` | Migration cannot run safely because of invalid input, write conflicts, dirty files, ambiguous drift, or missing prerequisites. |

Stable operation fields:

- `id`: deterministic migration ID, namespaced by format/profile/contract.
- `kind`: `format` or `contract`.
- `status`: `planned`, `applied`, `skipped`, or `blocked`.
- `risk`: `low`, `medium`, or `high`, where high-risk migrations should refuse auto-apply until split into safer operations.
- `files`: repo-relative pack paths.
- `operations`: structured patch operations, not prose-only instructions.
- `preserves_existing`: required boolean for operations that touch user-owned files.

Stable error codes to add to capabilities:

- `unsupported_format`
- `migration_blocked`
- `migration_path_missing`
- `dirty_worktree`
- `backup_failed`

## Migration Registry

Add a CLI-owned registry with explicit ordered steps:

| Registry field | Requirement |
|---|---|
| `id` | Stable string, for example `mdp.v0.gtm.agent-surface.v0`. |
| `source_format` | Exact supported source format. |
| `target_format` | Exact target format; same-format patches keep source and target equal. |
| `kind` | `format` or `contract`. |
| `applies_when` | Deterministic predicate over manifest, prompts, cards, evals, and current validator diagnostics. |
| `plan` | Pure read-only function that returns operations and warnings. |
| `apply` | Optional write function. If absent, migration can report advisory steps but cannot be applied. |
| `idempotence` | Running twice must produce `up_to_date` or `skipped`, not duplicate fields. |

Registry behavior:

- Sort format migrations by source/target path.
- Apply same-format contract migrations after format target resolution.
- Fail with `migration_path_missing` when no ordered path exists.
- Refuse to plan or apply unknown future formats unless a read-only compatibility shim exists.
- Use current validators as detection input, but keep migration planning deterministic without scraping or AI.

## First Implementation Slice

Ship the framework now as a narrow CLI feature:

1. Add `migrate` command parsing in `cli/src/cli.rs` with `--dir`, `--to`, `--dry-run`, and `--apply`.
2. Add `cli/src/commands/migrate.rs` with read-only inspection, registry shape, and no-op/current-format behavior.
3. Support same-format drift detection for the first GTM profile case:
   - missing `profile.id: gtm` or `profile.agent_surface` when the pack otherwise matches the GTM starter family;
   - stale prompt-output examples that can be recognized and replaced without touching prompt instructions or user strategy;
   - unsupported future/unknown `format`.
4. Add write support only for low-risk, deterministic `yaml_set_if_missing` and prompt example replacement operations that preserve non-targeted fields.
5. Add `migrate` to `mdp capabilities` with `side_effects: writes-files`, `supports_dry_run: true`, `supports_json: true`, `supports_summary: true`, and no auth/network behavior.
6. Add `mdp migrate` references to `cli/USAGE.md`, `README.md`, `docs/distribution.md`, and `plugin/skills/mdp/SKILL.md`.

Do not wait for `mdp.v1`. A no-op-only command would be too weak after the Trellis drift case, but the first slice should still avoid broad pack rewriting.

## Safety Rules

Write guardrails:

- Preview is default. `--apply` is the only write mode.
- Refuse writes inside a git repo unless `git status --porcelain -- <pack-dir>` is clean for the pack files that would be touched.
- For non-git pack directories, create a deterministic backup under `.mdp/backups/migrations/<timestamp-or-run-id>/` before writing.
- Refuse unsafe overwrites when an operation would replace non-targeted custom content.
- Emit planned diffs or structured before/after summaries for each changed path.
- Never mutate pack content `version` unless a migration operation explicitly documents why and the user passes `--apply`.
- Never rewrite strategy cards, ICP content, claims, CTAs, personas, eval intent, or prospect examples from template defaults.

Privacy and product boundary:

- No auth, hosted API, scraping, enrichment, CRM writeback, sending, sequencing, or BI behavior.
- No AI-assisted rewriting.
- No private customer data, raw transcripts, tokens, or local auth/session paths in migration logs.

## Validate, Doctor, and Capabilities

`mdp validate`:

- Keep returning validation errors for structurally invalid packs.
- For stale-but-readable packs, add warnings with `migration_hint.command: "mdp migrate --dir . --dry-run"` once the migration planner can identify a deterministic path.
- Do not make ordinary validation depend on network access, template fetching, or AI.

`mdp doctor`:

- Report `migration_status` in `checks` when manifest can be read.
- For `migration_available`, show a concise setup hint: run `mdp migrate --dir . --dry-run`.
- For `unsupported_format`, explain that the installed CLI cannot migrate this pack.

`mdp capabilities`:

- Add `migrate` to command inventory.
- Add the new migration error codes.
- State that migration is local/offline and write-capable only with explicit apply.

## Tests and Fixtures

Implementation should add focused coverage around these fixtures:

| Scenario | Suggested fixture | Expected result |
|---|---|---|
| Current GTM starter | `plugin/assets/templates/basic` or generated temp pack | `up_to_date`, no writes. |
| Missing agent surface | copied GTM pack with `profile.agent_surface` removed | `migration_available`, manifest operation planned. |
| Trellis-like customized GTM drift | synthetic fixture preserving custom cards but missing newer metadata/examples | `migration_available`, only contract fields/examples touched. |
| Stale prompt example | prompt fixture with old `source_summary.inputs_used` shape | deterministic prompt example patch planned. |
| Unknown future format | manifest with `format: mdp.v99` | `unsupported_format`, no writes. |
| Dirty git pack | temp git repo with touched migration target file | `blocked` or `dirty_worktree` under `--apply`. |
| Non-git pack apply | temp pack outside git | backup path created before writes. |
| Capabilities | unit test in `cli/src/commands/capabilities.rs` | `migrate` command and migration codes present. |
| Doctor/validate hints | stale readable pack | warning/hint points to `mdp migrate --dir . --dry-run`. |

Suggested validation commands:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- migrate --dir plugin/assets/templates/basic --dry-run
cargo run --manifest-path cli/Cargo.toml -- --json migrate --dir plugin/assets/templates/basic --dry-run
cargo run --manifest-path cli/Cargo.toml -- --json capabilities
cargo run --manifest-path cli/Cargo.toml -- --json doctor --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
make validate
```

## Repo Surface

Likely implementation files:

- `cli/src/cli.rs`
- `cli/src/app.rs`
- `cli/src/commands/mod.rs`
- `cli/src/commands/migrate.rs`
- `cli/src/commands/capabilities.rs`
- `cli/src/commands/health.rs`
- `cli/src/output.rs`
- `cli/src/constants.rs`
- `cli/src/models.rs`
- `cli/src/pack_io.rs`
- `cli/src/starter.rs`
- `cli/USAGE.md`
- `README.md`
- `docs/distribution.md`
- `plugin/skills/mdp/SKILL.md`

If implementation needs broader YAML patching helpers, add a small local abstraction under `cli/src/commands/migrate.rs` first. Extract shared write helpers only after at least two command modules need them.

## Interaction With MDP-38, MDP-39, and MDP-52

MDP-38 and MDP-39 keep the first profile foundation additive: `profile`, `primitive_map`, input contracts, and jobs can remain optional under `mdp.v0`, while fixed `CardKind` stays the core loader family.

MDP-43 should not invent a separate profile/card-kind migration path. It should be the one migration registry for:

- future breaking pack-format changes, if custom kinds or renamed core fields are accepted later;
- same-format contract drift, including profile agent-surface metadata and prompt-output validator compatibility;
- any future primitive-map shape changes that can be patched deterministically without changing `format`.

MDP-52's `agent-surface` behavior supplies the first concrete stale-contract detector: a GTM-like pack that lacks `profile.agent_surface` can be valid `mdp.v0` but still need deterministic migration guidance.

## Follow-Up Implementation Issue Draft

Title: `Implement preview-first mdp migrate framework for same-format drift`

Route: `ce-work`

Scope:

- Add `mdp migrate` CLI parsing and `mdp.migrate.v0` output.
- Implement preview-first planning, explicit `--apply`, and current-format no-op behavior.
- Add the first same-format GTM contract migration for missing `profile.agent_surface` and one stale prompt-output example fixture if the stale shape can be identified from existing validators.
- Add capabilities, doctor, validate hints, docs, skill guidance, and focused tests.

Acceptance criteria:

- `mdp migrate --dir plugin/assets/templates/basic --dry-run` returns `up_to_date`.
- A stale GTM fixture returns `migration_available` with planned operations and validation commands.
- `--apply` refuses dirty git pack targets and preserves non-targeted user content.
- `mdp --json capabilities` advertises `migrate` and migration error codes.
- `mdp --json doctor` and `mdp --json validate` point stale readable packs to `mdp migrate --dir . --dry-run`.
- `cargo test --manifest-path cli/Cargo.toml` and `make validate` pass.

No-autofix exception: none expected for this slice if migrations are limited to local deterministic file edits and tests. Add `ai:autofix-enabled` on the implementation PR only if the PR does not include sensitive data, production infra changes, or broad destructive migration behavior.
