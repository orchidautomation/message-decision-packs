# CI CLI path filter plan

## Intent

Limit the `CI / cli` job to changes that own or exercise the Rust CLI while
leaving the rest of the CI workflow independent. This work is separate from
PR #208 and must not alter its branch, worktree, or history.

## Source

- Repository: `orchidautomation/message-decision-packs`
- Source ref: `origin/main`
- Source commit: `07773a5a2180c2ebcbebb59f30381a313e8d120a`
- Delivery branch: `codex/ci-1-cli-path-filter`

## Implementation

Edit only `.github/workflows/ci.yml`.

1. Add a lightweight path-classification job using a maintained path-filter
   action.
2. Export a `cli` boolean for these traced inputs:
   - the CI workflow itself;
   - `cli/**`, `rust-toolchain.toml`, and the root `Makefile`;
   - CLI-compiled or CLI-tested templates and authority corpora;
   - skill sources and eval corpora exercised by the CLI job;
   - exact example fixtures read by CLI tests;
   - exact scripts and script libraries invoked or embedded by CLI tests;
   - documentation surfaces read by the skill-packaging validation that runs
     in the CLI job.
3. Make only `cli` depend on the classifier and run when its output is true.
   Keep `pluxx` and `eve-example` independent so a skipped CLI job cannot
   suppress or block them.

## Acceptance criteria

- A CLI source, manifest, toolchain, shared Make target, traced fixture, or
  traced validation-script change selects the `cli` job.
- `.github/workflows/ci.yml` selects the `cli` job so filter changes validate
  themselves.
- An unrelated code or documentation path does not select the `cli` job.
- `pluxx` and `eve-example` retain no dependency on the classifier or `cli`.
- The workflow remains valid GitHub Actions YAML.
- No branch, PR, label, merge state, or worktree associated with PR #208 is
  modified.

## Focused validation

- Run `actionlint` against `.github/workflows/ci.yml` when available.
- Parse the YAML independently.
- Exercise representative positive and negative paths against the configured
  filter rules.
- Inspect the final job dependency graph and diff.
- Use Orchid verification, review, and validated PR-body closeout against the
  exact final commit. Stop at Ready for Human; do not merge or enable
  auto-merge.
