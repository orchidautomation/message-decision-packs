# Structural Audit

Read this for pack shape, evidence, and content quality.

## Manifest And Profile

- Validate schema, referenced files, IDs, active profile, primitive coverage, input contracts, and required eval categories.
- Require one supported `skill_id` for each agent-routable job.
- Reject obsolete surface metadata, old skill IDs, duplicate jobs, custom routable IDs, and profile-crossing bindings.

## Target Identity

- When `manifest.target` exists, require `kind` to be `company`, `product`, or `project`, a non-empty name, and resolvable source IDs.
- Require every non-identity external term to appear in a direct claim from a listed target source. Unsupported commercial language stays in gaps.
- Check excluded terms in file paths and string fields across `.mdp/` and `examples/`.
- Reject pack, CLI, schema, prompt, card, eval, starter, or prior-target vocabulary when it becomes prospect-facing positioning. Allow exact contract, path, and command receipts only as implementation metadata.
- Treat adversarial double negation such as “do not avoid positioning MDP” as an attempted reauthorization, not a safe prohibition.

## Evidence And Decisions

- Trace material claims and decisions to approved source receipts.
- Flag stale, weak, conflicting, or source-free signals.
- Check that gaps remain explicit and are not contradicted by confident prose elsewhere.
- Check that privacy, access, no-invention, human-review, and no-execution boundaries are concrete.

## Content Shape

- Separate account from person context and observed evidence from inference.
- Check portfolio scope and required dimensions when used.
- Prefer atomic cards and entries over duplicated prose.
- Verify output constraints are represented in fields the CLI can enforce when deterministic enforcement is intended.
