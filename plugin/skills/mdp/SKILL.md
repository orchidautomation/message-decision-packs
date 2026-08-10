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

When the user asks why an existing fit, route, brief, normalization, or clean
run reached its result, prefer the bounded projection before opening full
source artifacts:

```bash
mdp --json trace --file <saved-cli-result-or-contracted-artifact.json>
mdp --json trace --bundle <run-bundle.json> --receipt <run-receipt.json> \
  --artifact-root <published-artifact-directory>
```

Treat `mdp.decision-trace.v1` as explanatory only. Distinguish its
`designed_graph` from its `observed_path`, keep the authority notice intact,
and follow artifact references only when deeper review is necessary. Never
infer missing steps, recover redacted prose, or upgrade a blocked/unavailable
trace. Use `--format mermaid` only as a display adapter over that same trace.

For a new authoritative execution, freeze the pack and declared inputs into an
`mdp.run-request.v1` file, then launch the shared runtime outside the authoring
conversation:

```bash
mdp --json run --request <run-request.json> --out-dir <new-run-directory>
mdp --json verify-run --bundle <new-run-directory>/run-bundle.json \
  --receipt <new-run-directory>/run-receipt.json \
  --artifact-root <new-run-directory>
```

For MCP-capable hosts, the profile-neutral adapter is
`scripts/mdp-run-mcp-server.mjs` or
`${PLUGIN_ROOT}/scripts/mdp-run-mcp-server.mjs`. It exposes `mdp_run_tools`,
`mdp_run`, and read-only `mdp_verify_run`; pass only existing authority-file
paths and, for execution, a new `output_dir`. The MCP server transports the
file-oriented CLI calls and returns canonical CLI data unchanged. It owns no
assurance dimension and must never accept ambient chat, inline evidence, or an
assurance override.

The current conversation is a control plane, never proof of fresh context. Do
not add chat facts, rewrite a decision, or repair a no-draft result after the
run. Present the verified CLI authority block intact and label all surrounding
explanation as outside receipt authority. A new agent task is only advisory
unless its runner evidence proves the relevant controls. Deterministic-only
runs must report inference dimensions as `not-applicable`, not “fresh.”

`mdp run-receipt` remains the legacy v0 proposal compatibility path. A v0
`audit-grade` label does not silently become v1 verified assurance. Prefer the
host-neutral local proposal runner when available: `scripts/mdp-proposal-runner.mjs` in source checkouts or `${PLUGIN_ROOT}/scripts/mdp-proposal-runner.mjs` in installed Pluxx bundles. For MCP-capable hosts, the bundled local stdio MCP wrapper is `scripts/mdp-proposal-mcp-server.mjs` or `${PLUGIN_ROOT}/scripts/mdp-proposal-mcp-server.mjs`; it exposes `mdp_proposal_tools` and file/path-only `mdp_proposal_run`. This is local stdio only, not a hosted or remote MCP service, and MCP transport alone is not isolation evidence. MDP also ships the lower-level optional BYOK OpenAI reference runner at `scripts/mdp-native-normalize-openai.mjs` or `${PLUGIN_ROOT}/scripts/mdp-native-normalize-openai.mjs`; dry-run/mock validation and normal MDP install/use do not need an API key, but a real native model call requires the operator's secure `OPENAI_API_KEY`. Demo, fixture, mock, or synthetic runner audits may only be used for walkthroughs/tests.

If the user asks whether proposal work is audit-grade, route the answer to
`$mdp-proposal-review` even when the request sounds like general MDP help. That
skill owns the source/runner/MCP decision tree. Do not answer from tool
availability or confidence: without a current audit-grade receipt, report
`advisory` or `blocked` and hand off the exact missing evidence step.

Integration support and per-run assurance are separate. Consult [canonical runner support matrix](https://github.com/orchidautomation/message-decision-packs/blob/main/docs/headless-normalization-runners.md#canonical-runner-support-matrix) and use only `verified`, `recipe-only`, `unsupported`, or `fixture/mock-only` for integration state. Never infer `verified` from a runner name, installed command, schema-valid audit, documented recipe, or MCP availability. Demo, fixture, mock, and synthetic evidence is always `fixture/mock-only`.

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

Inspect `data.recommendation.product_foundation` before opening pack prose. It
is the compact exact-job summary: `unassessed`, `ready`, or `blocked`, plus
selected/required facet IDs and diagnostics. Then use the same exact canonical
job ID with `requirements` to retrieve the complete resolved facets, exact
entry/gap refs, bounded entry content, and optional/excluded/untriggered IDs.
Never substitute a natural-language job approximation or use keyword routing
to infer product-foundation authority.

Treat `.mdp/README.md` as secondary navigation only, after CLI-resolved
foundation output. README prose cannot satisfy a facet, close a gap, resolve a
conflict, or override structured authority. Never invent missing product,
ICP, proof, certification, compliance, or outcome facts; preserve the CLI gap
or blocked diagnostic and ask for reviewed sources.

Foundation readiness only vetoes broader readiness. `ready` never promotes an
otherwise unready job and never establishes sufficient-for-job or self-standing
status. `unassessed` preserves legacy compatibility without claiming
sufficiency. Explicit profile activation `needs-review` or `blocked` still
prevents activation.

For a bound job, retrieve its attempted-complete collector and normalization handoff before sourcing or normalizing data:

```bash
mdp --json requirements --dir <pack-root> --job <job-id>
```

This command is read-only. It compiles the pack-owned questions, source policy, normalization identity, and request/response schemas; it does not collect sources or call a model. An existing job without a Decision Input Contract returns `available: false`. When that job declares `model_task`, inspect `data.model_task_available`, the exact prompt ID/version/hash, declared input producers, instructions, and output contract. Hand that package to the customer-selected host; MDP does not execute it.

When the user is connecting an external orchestrator, keep its mapping outside
the pack and validate it against the exact compiled release:

```bash
mdp --json schema source-binding
mdp --json validate-source-binding --dir <pack-root> \
  --job <job-id> --file <source-binding.json>
```

Require `data.valid: true` before integration activation. The command validates
portable pack/requirements pins, complete and unique qualified attribute
coverage, requirement classes, allowed source classes, release receipts, and
fixed status translation. It does not access the source system or run
normalization. A job with `available: false` cannot be source-bound.

When `requirements` returns `data.available: true`, validate the bound
normalization with the exact source-attempt request and exact host-collected
attempt-results ledger:

```bash
mdp --json validate-prompt-output --dir <pack-root> \
  --prompt <bound-prompt> \
  --source-attempt-request <source-attempt-request.json> \
  --collected-attempt-results <collected-attempt-results.json> \
  --file <normalized-output.json>
```

Do not extract or pass `normalized_prospect` to fit, routing, brief, or copy
work unless validation passes and the envelope's top-level `outcome` is exactly
`ready`. Every other normalized outcome remains no-draft.

For any selected prompt that is not the bound decision-input normalization
prompt, preserve the `mdp.prompt-output.v0` validation path without
`--source-attempt-request` or `--collected-attempt-results`, regardless of
job-wide `data.available`. Require
`normalization_trace.fit_readiness.ready_for_mdp_fit` only for a legacy
prospect-normalization prompt that declares `normalized_prospect` and that
readiness field. For extraction or card-patch prompts, successful contract
validation is the applicable machine gate; do not require an undeclared
normalization trace.

For `data.model_task.status: ready`, use only the compiled prompt package and
exact resolved product foundation. Validate the returned governed artifact
with its prompt ID and the exact host-created invocation receipt:

```bash
mdp --json validate-prompt-output --dir PACK_ROOT \
  --prompt-id PROMPT_ID \
  --invocation-receipt PROMPT_INVOCATION_JSON \
  --file OUTPUT_JSON
```

The receipt must use `mdp.prompt-invocation.v1` and bind the job, canonical
prompt ID/version/SHA-256, and per-declared-input SHA-256 values. A valid
artifact schema is not final claim approval;
generated prose must also pass the job's `check-claims` or `verify-output`
gate. A missing, blocked, or unassessed model task must never fall back to
instructions implied by this skill.

Closed v1 pairs:

- `mdp-gtm-brief`: `prospect-fit-or-brief`, `outbound-copy-brief`, `outbound-copy-review`
- `mdp-proposal-review`: `bid-no-bid-review`, `compliance-review`, `proof-review`, `red-team-review`

## Core Operator Loop

Run only the commands the job requires:

```bash
mdp --json doctor --dir <pack-root>
mdp --json validate --dir <pack-root>
mdp --json requirements --dir <pack-root> --job <job-id>
mdp --json explain --dir <pack-root>
mdp --json gaps --dir <pack-root>
mdp --json eval --dir <pack-root>
```

Use `--strict` on `validate` or `eval` for a blocking quality gate. Use `mdp <command> --help` rather than guessing flags.

Read [references/cli-operator.md](references/cli-operator.md) for command selection or artifact-write rules. Read [references/mental-model.md](references/mental-model.md) when explaining product boundaries, pack primitives, or responsibility splits.
After a validated fix yields a reusable engineering lesson, read
[references/compound-learning.md](references/compound-learning.md) before
capturing it.

## Boundaries

- MDP stores and validates decision context. It is not a CRM, sequencer, enrichment provider, scraper, BI tool, proposal writer, or generic automation system.
- Do not enrich prospects, send outreach, mutate CRM records, scrape gated sources, submit proposals, or approve compliance through this skill.
- Preserve missing or unsupported information as gaps. Never smooth a failed CLI decision into a plausible answer.
- Do not create an eleventh primitive, a new product `CardKind`, or a company
  wiki. Product-foundation facets only index existing structured authority.
- When a GTM manifest declares `target`, keep all external positioning on that exact company, product, or project. The target name alone does not prove product claims or fit.
- Prefer user-approved local sources. Keep restricted material out of public artifacts and committed fixtures.

## Closeout

Report the pack root, selected skill or job ID, commands run, validation state, durable artifacts written, unresolved gaps, and any installed-versus-source uncertainty.
