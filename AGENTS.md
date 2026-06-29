# AGENTS.md

## Project Truth

This repo contains Message Decision Packs (MDP): a local/offline standard, CLI, and Codex plugin for modular GTM messaging context.

MDP stores decision context and routing contracts. It is not execution infrastructure.

Do not describe MDP as:

- AI SDR
- CRM
- sequencer
- enrichment provider
- scraper
- BI tool
- generic automation system

## Repo Shape

- `cli/`: Rust `mdp` CLI.
- `plugin/`: Codex plugin and skills.
- `plugin/assets/templates/basic`: neutral example pack.
- `docs/`: distribution and design notes.

## Linear Coordination

- Default Linear team: `MDP`.
- Team overview: https://linear.app/orchid-automation/team/MDP/overview
- Issue prefix: `MDP`.

Use the MDP Linear team for this repo's bugs, feature work, planning, and closeout. When work is tied to a Linear issue, include the issue ID in branch names, PR titles/descriptions, and relevant comments, for example `MDP-123`.

This is a public-facing source-available repo. Keep public GitHub references limited to issue identifiers, public links, and sanitized summaries; do not expose private Linear discussion details, customer data, raw transcripts, tokens, or local-only paths in commits, PRs, or public docs.

## Feature Change Hygiene

When adding, changing, or removing MDP behavior, update the matching agent-facing skill instructions in `plugin/skills/` in the same change. The CLI contract, starter/template pack, docs, and skills should not drift apart.

## Release And Install Closeout

The documented installer uses GitHub release assets, not the current `main` branch. A merged PR is not locally shipped until a release containing the merge commit has been published and the installer smoke test passes.

For any merged PR that changes CLI behavior, plugin bundle behavior, skill instructions, starter/template packs, install scripts, or runtime assets, the agent should complete the release/install closeout without requiring Brandon to remember it:

1. Confirm the PR is merged to `main`, checks are green, and local `main` or the active release worktree is clean and current with `origin/main`.
2. Run the relevant validation again from the release commit, usually `make validate`.
3. Cut the next patch release from current `main` using the repo's release process and the next semver patch tag. This repo instruction is standing approval for routine patch releases after release-affecting merged PRs. Ask Brandon before a minor/major release, prerelease, release with known failing validation, or any release that includes unrelated risky changes.
4. Run the documented installer:

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
```

5. Smoke test the installed artifact, not just the source tree. Prefer `/Users/brandonguerrero/.local/bin/mdp` or `command -v mdp`, and verify the behavior that changed.
6. Close out with three explicit states: merged commit, released tag, and installed/smoke-tested status.

Docs-only changes that do not affect installer assets, CLI behavior, plugin behavior, skills, or templates can stop at merge unless Brandon asks for an immediate release.

## Validation

Prefer narrow checks first:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
make validate
```

If local Codex plugin validators are unavailable, say that plugin validation was skipped and still run the CLI/template checks.

## Safety

- Do not commit secrets, private customer data, browser sessions, tokens, raw transcripts, or local auth files.
- Do not publish packages, create public repos, or push public releases without explicit approval, except for the routine patch-release closeout described above after a release-affecting PR has already merged to `main`.
- Keep generated scratch under `.agent-artifacts/` and do not commit it.
