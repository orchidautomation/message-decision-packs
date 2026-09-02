# Repository Streamlining Audit

Date: 2026-09-02
Linear: MDP-341

## Result

The repository-local MDP pack and the Eve/Vercel application were obsolete
product surfaces rather than current fixtures. They have been removed. The
remaining top-level `examples/` directories are all synthetic contract or
conformance fixtures used by tests, documentation, or release validation.

The Vercel project under `deploy/mdp-installer/` is still active infrastructure
for `mdp.orchidlabs.dev`; only its Eve-specific routes were stale.

## Folder-by-folder disposition

| Surface | Disposition | Reason |
|---|---|---|
| `.codex/`, `.entire/` | Keep | Current, optional Entire/Codex checkpoint configuration. Hooks fail open when Entire is unavailable. |
| `.github/` | Keep, simplified | Current CI and release workflows. The dedicated Eve job and stale Eve path filter were removed. |
| `.mdp/` | Remove | Old pack for this repository. MDP-for-MDP now owns this decision context. |
| `assets/` | Keep | Authored templates, conformance corpus, and brand assets. Exact parity with `plugin/assets/` is an enforced packaging contract. |
| `cli/` | Keep | Product implementation, schemas, tests, and the locked Rust dependency graph. |
| `deploy/mdp-installer/` | Keep, simplified | Active redirect-only host for installers and agent briefings. Eve application routes were removed. |
| `docs/` | Keep | Current public product and maintainer documentation. |
| `docs/orchid/` | Keep for now | Public-safe contributor history required by the repository's current Orchid Relay workflow; it is not canonical product documentation. |
| `examples/ai-sdr-eve-vercel/` | Remove | Obsolete hosted-agent application, duplicated pack contents, Node dependency graph, and Vercel-specific runtime code. |
| Other `examples/` directories | Keep | Shared synthetic fixtures for decision-input, cold-model, trace, route-budget, and run-conformance validation. |
| `plugin/` | Keep | Canonical authored skill source plus package assets and eval contracts. |
| `scripts/` | Keep | Current validation, installer, release, MCP, native-runner, and compatibility tooling. No script was removed solely because a static reference scan found no caller. |
| Root docs/config | Keep, simplified | README and LLM briefings now describe stable contracts instead of old point-release migrations or the removed Eve application. |

## Retained compatibility, not dead code

The CLI and scripts still contain explicit `legacy` and `v0` readers, adapters,
schemas, and tests. Those paths are implemented compatibility contracts and
cannot be classified as dead from naming alone. Removing them requires a
separate compatibility decision, consumer evidence, a versioned deprecation
plan, and targeted CLI changes.

Likewise, files with few repository-local references can still be release
assets or operator entry points. `check-update.sh`, `doctor-mdp.sh`, installer
helpers, and packaged scripts therefore remain until package manifests and
release smoke tests prove they are no longer shipped.

## Follow-up candidates

These are maintenance candidates, but were not safe to delete as part of this
cleanup:

1. **Contributor-history volume:** `docs/orchid/` contains the majority of the
   documentation files. Consider moving completed plans, QA notes, and reviews
   to an external history repository or generating a curated archive after the
   Orchid Relay retention contract changes.
2. **Explicitly historical public docs:** `docs/what-this-repo-is.md` and
   `docs/new-codex-user-journey.md` identify themselves as historical and are
   not in the current docs index. The old Eve/Flue decision and Vercel-first
   scout plan under `docs/orchid/` are also superseded runtime history. Move or
   remove these records after choosing the contributor-history retention
   boundary.
3. **Dated behavioral report:**
   `plugin/skill-evals/reports/mdp-262-codex-2026-08-27.json` is a single model
   snapshot with documented limitations. Decide whether published eval reports
   belong in release evidence, contributor history, or outside the shipped
   plugin passthrough.
4. **Fixture naming:** the remaining top-level `examples/` are tests, not
   runnable examples. A future mechanical move to `fixtures/` would make that
   boundary obvious but touches Rust, JavaScript, docs, CI filters, and release
   smoke paths and should land independently.
5. **Large CLI modules:** several Rust modules are thousands of lines. They are
   active code rather than deprecated code; split them only with behavior-
   preserving tests, not as repository housekeeping. A clean build currently
   reports 19 unused/dead-code warnings in the binary (plus three helper
   warnings in the template-inventory test target). Audit those symbols in a
   focused CLI issue before deleting them; several may be test-only seams.

## Maintenance rule

New runnable integrations should live in their owning runtime repository.
This repository should contain only the MDP standard, CLI, canonical plugin,
authored templates, contract fixtures, required distribution infrastructure,
and current documentation.
