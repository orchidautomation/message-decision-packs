# MDP Skill Evals

MDP has two separate eval surfaces:

- Pack evals under `.mdp/evals/` exercise CLI and pack behavior.
- The catalog corpus under `plugin/skill-evals/` evaluates the five public skill boundaries, modes, safety contracts, and CLI eligibility.

Skill evals are maintainer artifacts, not runtime instructions, so they do not ship inside every installed skill directory.

## Corpus

```text
plugin/skill-evals/
├── coverage.json       # exact inventory, modes, risk, assertion requirements
├── trigger-cases.json  # catalog-level expected owner or intentional null route
└── output-cases.json   # mode/risk prompts and structured required assertions
```

Trigger cases include train/validation splits, distinct scenario families, an expected canonical skill ID or `null`, pack/profile context, near misses, unsafe requests, and profile-crossing cases. The top-level collision ledger names the exact corpus case and competing skill for every ordered pair in both splits; positive ownership alone does not count as collision evidence.

Output cases cover all 17 internal modes in both train and validation: 34 mode/split cells. Assertions use explicit categories such as CLI gate, evidence, boundary, safety, handoff, and human review.

## Deterministic Gate

```bash
python3 scripts/skill-eval-harness.py \
  --plugin-skills plugin/skills \
  --corpus plugin/skill-evals \
  --mdp-bin cli/target/debug/mdp \
  --output /tmp/mdp-skill-evals
```

The gate requires:

- exact equality among the coverage manifest and canonical five-skill source inventory;
- no TODO scaffolds or missing descriptions;
- complete train/validation trigger and mode coverage;
- null-route and GTM/proposal profile-crossing cases;
- structured required output assertions for every mode;
- exact `mdp.skills.v1` inventory, shared bootstrap eligibility, three GTM routes, four proposal routes, no cross-profile fallback, and missing-pack diagnostics.

Pass `--installed-skills-root PATH` to compare an installed host tree recursively with the canonical catalog, including content hashes and executable bits. The packaging validator applies the same fidelity rule to all four generated bundles.

## Host-Observed Results

The deterministic harness cannot observe whether Codex, Claude Code, Cursor, or OpenCode actually loaded a skill. Keep that distinction honest.

Client-specific runners can import `mdp.skill-host-results.v1` with `--results FILE`. The file names the host, model, and recording time, then carries:

- `trigger_observations`: `case_id`, unique `trial_id`, and `selected_skill_id` or `null`;
- `output_observations`: `case_id`, unique `trial_id`, and a boolean grade for every required assertion ID.

Imported results must cover every trigger and output case. Ordinary misroutes, duplicates, missing cases, missing assertion grades, failed output assertions, and profile-crossing unsafe selections fail the configured host benchmark. The report preserves accuracy and the confusion matrix for diagnosis.

Run model-dependent activation trials multiple times and report trigger rates separately from deterministic CI. Do not make release validation flaky by pretending a fixture linter observes host behavior.

## Iteration

1. Add a new realistic scenario family from actual confusion or workflow evidence.
2. Put related variants in only one split; do not leak validation prompts into skill instructions.
3. Run the deterministic gate and source-built CLI cases.
4. When a host runner is available, collect repeated observations and import them.
5. Tighten a description or mode reference only where results show a real failure.
6. Keep run output under temporary or ignored scratch and never commit restricted transcripts.
