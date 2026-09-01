# MDP Skill Evals

MDP has two separate eval surfaces:

- Pack evals under `.mdp/evals/` exercise CLI and pack behavior.
- The catalog corpus under `plugin/skill-evals/` evaluates the four public skill boundaries, modes, safety contracts, and CLI eligibility.

Skill evals are verification resources, not runtime instructions or CLI route authority. The shared corpus is shipped at the bundle root so an installed artifact can be checked without duplicating fixture bodies inside every skill directory.

## Corpus

```text
plugin/skill-evals/
├── coverage.json       # exact inventory, modes, risk, assertion requirements
├── trigger-cases.json  # catalog-level expected owner or intentional null route
└── output-cases.json   # mode/risk prompts and structured required assertions

plugin/skills/<skill-id>/evals/index.json  # exact ownership reference for that skill
```

Each of the four public skills owns one index. Indexes reference the shared
`skill-evals/` corpus by case ID; they do not copy prompts or expected outputs.
The indexes are a complete, disjoint partition of owned trigger and output
cases. Null routes remain corpus-level cases and are intentionally not assigned
to a skill. The three trigger query shapes (`direct`, `typo`, and
`indirect-intent`) are required in both train and validation for every skill.

Trigger cases include train/validation splits, distinct scenario families, an expected canonical skill ID or `null`, pack/profile context, near misses, unsafe requests, and profile-crossing cases. The top-level collision ledger names the exact corpus case and competing skill for every ordered pair in both splits; positive ownership alone does not count as collision evidence.

Output cases cover all 17 internal modes in both train and validation: 34
mode/split cells. Additional proposal bridge and adversarial cases require a
blocked result when audit-grade is requested without a callable runner/current
receipt, require explicit advisory acceptance before ambient pasted-text
review, keep chat-only facts out of approved evidence, and preserve OCR/source
mismatches as gaps. Assertions use
explicit categories such as CLI gate, evidence, boundary, safety, handoff, and
human review.

## Deterministic Gate

Canonical skill prose and path contracts are checked separately before the
behavior corpus. The contract validator enforces bounded frontmatter,
skill-local links, the single authored `plugin/skills/` source, safe load-time
instructions, current runner script names, and a small exact allowlist of
high-risk proposal refusal language:

```bash
make validate-skill-contracts
```

Its unit suite creates deliberately invalid temporary skills for every
proposal guardrail. Exact-string checks are reserved for those safety-critical
phrases; ordinary instructional prose remains free to evolve.

```bash
python3 scripts/skill-eval-harness.py \
  --plugin-skills plugin/skills \
  --corpus plugin/skill-evals \
  --mdp-bin cli/target/debug/mdp \
  --output /tmp/mdp-skill-evals
```

The gate requires:

- exact equality among the coverage manifest and canonical four-skill source inventory;
- no TODO scaffolds or missing descriptions;
- complete train/validation trigger and mode coverage;
- null-route and GTM/proposal profile-crossing cases;
- structured required output assertions for every mode;
- exact `mdp.skills.v1` inventory, shared bootstrap eligibility, three GTM routes, four proposal routes, no cross-profile fallback, and missing-pack diagnostics.
- every per-skill eval index, objective assertion category, and shared-corpus ownership partition.

Pass both `--installed-skills-root PATH` and `--installed-corpus PATH` to compare
an installed host tree recursively with the canonical catalog and shared
corpus, including content hashes and executable bits. The packaging validator
applies the same fidelity rule to all four generated bundles:

```bash
python3 -m unittest scripts/test_skill_eval_harness.py scripts/test_skill_packaging.py
python3 scripts/validate-skill-packaging.py --require-bundles
```

## Host-Observed Results

The deterministic harness cannot observe whether Codex, Claude Code, Cursor, or OpenCode actually loaded a skill. Keep that distinction honest.

Client-specific runners can import `mdp.skill-host-results.v1` with `--results FILE`. The file names the host, model, and recording time, then carries:

- `trigger_observations`: `case_id`, unique `trial_id`, and `selected_skill_id` or `null`;
- `output_observations`: `case_id`, unique `trial_id`, and a boolean grade for every required assertion ID.
- `recording`: comparison mode (`with-skill`, `baseline`, or `previous-version`),
  synthetic pair ID, public source revision, elapsed milliseconds, and input/output
  token counts.

Imported results must cover every trigger and output case. Ordinary misroutes, duplicates, missing cases, missing assertion grades, failed output assertions, and profile-crossing unsafe selections fail the configured host benchmark. The report preserves accuracy and the confusion matrix for diagnosis.

Run model-dependent activation trials multiple times and report trigger rates separately from deterministic CI. Do not make release validation flaky by pretending a fixture linter observes host behavior.

To compare a with-skill run with scratch-only baselines, pass matching pair IDs:

```bash
python3 scripts/skill-eval-harness.py \
  --results /tmp/mdp-evals/with-skill.json \
  --baseline-results /tmp/mdp-evals/baseline.json \
  --previous-results /tmp/mdp-evals/previous-version.json \
  --output /tmp/mdp-evals/report
```

Only aggregate accuracy, assertion accuracy, confusion matrices, timing, and
token metadata are reported. Prompts, transcripts, contact values, credentials,
and raw provider output are rejected and must remain under ignored scratch
storage such as `/tmp` or `.agent-artifacts/`.

## Iteration

1. Add a new realistic scenario family from actual confusion or workflow evidence.
2. Put related variants in only one split; do not leak validation prompts into skill instructions.
3. Run the deterministic gate and source-built CLI cases.
4. When a host runner is available, collect repeated observations and import them.
5. Tighten a description or mode reference only where results show a real failure.
6. Keep run output under temporary or ignored scratch and never commit restricted transcripts.
