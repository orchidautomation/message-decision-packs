# MDP-263: Linear and public GitHub boundary

## Outcome

Update the repository operating guidance so agents treat Linear as the private
authoritative control plane and project only public-safe GitHub delivery
evidence back into Linear.

## Ownership

- Edit: `AGENTS.md`.
- Do not edit any other product, plugin, CLI, workflow, or documentation file.
- Do not touch PR #208 or branch `codex/ci-1-cli-path-filter`.

## Required guidance

- Create or recover the private Linear work item before delivering a public PR.
- Treat synchronization as one-way from public GitHub into Linear: record the
  PR, check, and merge evidence in Linear.
- Never create a public GitHub Issue merely to unblock PR creation or linking.
- Never copy private Linear descriptions, comments, roadmap, customer, or
  business context into public GitHub surfaces.
- Expose only the minimum public-safe Linear reference required for
  traceability.

## Validation

- Confirm the final product diff changes only `AGENTS.md`.
- Run `git diff --check`.
- Assert that each required boundary appears in `AGENTS.md`.
- Stop at an unmerged PR created through Orchid's validated PR wrapper.

