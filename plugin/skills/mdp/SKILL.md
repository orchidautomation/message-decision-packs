---
name: mdp
description: Use when the user names MDP, Message Decision Packs, `.mdp/`, or the `mdp` CLI, asks about MDP commands, or needs a mixed MDP workflow coordinated. Do not use for generic GTM, copy, prospecting, or proposal work without an MDP objective.
---

# MDP

Coordinate explicit MDP work and use the CLI as the source of truth.

## Start Here

1. Find the intended pack root. Pass `--dir` explicitly; do not assume the current directory.
2. Inspect the installed contract before reading pack YAML:

```bash
mdp --json skills
mdp --json skills --dir <pack-root>
```

3. Treat `packaged_skill_ids` as released inventory, `eligibility` as pack policy, and `host_discovery.status: unobserved` as literal. Never claim MDP hid or exposed a host-discovered skill.
4. Use JSON output for decisions. Use `--summary` only for a concise human status.

For audit-grade proposal/document normalization, require `mdp run-receipt` after prompt-output validation. A skill running inside the current conversation cannot by itself guarantee model context isolation; the host runner must report `--isolation isolated`, `--declared-inputs-only`, and for production proposal pilots a valid `--runner-audit ... --require-runner-audit` from a native/headless runner. Prefer the host-neutral local proposal runner when available: `scripts/mdp-proposal-runner.mjs` in source checkouts or `${PLUGIN_ROOT}/scripts/mdp-proposal-runner.mjs` in installed Pluxx bundles. Its `tools` command prints MCP-shaped local steps, but it is not a hosted MCP server. MDP also ships the lower-level optional BYOK OpenAI reference runner at `scripts/mdp-native-normalize-openai.mjs` or `${PLUGIN_ROOT}/scripts/mdp-native-normalize-openai.mjs`; dry-run/mock validation and normal MDP install/use do not need an API key, but a real native model call requires the operator's secure `OPENAI_API_KEY`. Demo, fixture, mock, or synthetic runner audits are blocked from audit-grade and may only be used for walkthroughs/tests. Activation hooks may report whether that key is present, but they are advisory only; the blocking gate remains `mdp run-receipt --require-runner-audit`.

If the command is missing, run `command -v mdp` and `mdp --version`. Report the missing runtime and point to the documented installer; do not emulate CLI validation in prose.

## Route The User Job

- Create or improve `.mdp/` from approved material: use `$mdp-pack-builder`.
- Audit, harden, validate, or test the pack itself: use `$mdp-pack-review`.
- Check GTM fit, produce pre-draft context, or review supplied outbound copy against a GTM pack: use `$mdp-gtm-brief`.
- Review supplied pursuit material against a proposal pack: use `$mdp-proposal-review`.
- Explain commands, inspect contracts, or coordinate a request spanning those jobs: stay here and hand off each bounded phase.

Select one primary skill for each job. That skill owns its prerequisites and internal mode.

## Resolve Job-Bound Modes

Natural-language intent selects a canonical job ID; the CLI only validates it. For a profile-sensitive request, run:

```bash
mdp --json skills --dir <pack-root> --job <job-id>
```

Proceed only when `data.recommendation` names the expected skill and `pack_ready` is true. Unknown and profile-crossing job IDs do not have fallbacks.

Closed v1 pairs:

- `mdp-gtm-brief`: `prospect-fit-or-brief`, `outbound-copy-brief`, `outbound-copy-review`
- `mdp-proposal-review`: `bid-no-bid-review`, `compliance-review`, `proof-review`, `red-team-review`

## Core Operator Loop

Run only the commands the job requires:

```bash
mdp --json doctor --dir <pack-root>
mdp --json validate --dir <pack-root>
mdp --json explain --dir <pack-root>
mdp --json gaps --dir <pack-root>
mdp --json eval --dir <pack-root>
```

Use `--strict` on `validate` or `eval` for a blocking quality gate. Use `mdp <command> --help` rather than guessing flags.

Read [references/cli-operator.md](references/cli-operator.md) for command selection or artifact-write rules. Read [references/mental-model.md](references/mental-model.md) when explaining product boundaries, pack primitives, or responsibility splits.

## Boundaries

- MDP stores and validates decision context. It is not a CRM, sequencer, enrichment provider, scraper, BI tool, proposal writer, or generic automation system.
- Do not enrich prospects, send outreach, mutate CRM records, scrape gated sources, submit proposals, or approve compliance through this skill.
- Preserve missing or unsupported information as gaps. Never smooth a failed CLI decision into a plausible answer.
- When a GTM manifest declares `target`, keep all external positioning on that exact company, product, or project. The target name alone does not prove product claims or fit.
- Prefer user-approved local sources. Keep restricted material out of public artifacts and committed fixtures.

## Closeout

Report the pack root, selected skill or job ID, commands run, validation state, durable artifacts written, unresolved gaps, and any installed-versus-source uncertainty.
