# MDP Skill Evals

MDP has two different eval surfaces:

- Pack evals under `.mdp/evals/` test the CLI, template, routing, prompt-output, proof-output, fit, gaps, and claim-check behavior.
- Skill evals under `plugin/skills/<skill>/evals/` test whether agent-facing skills trigger on the right user requests and produce useful outputs.

The skill eval files are intentionally simple JSON so they can be reused by Codex, Blocks, or another client-specific harness.

## Layout

```text
plugin/skills/<skill>/
└── evals/
    ├── trigger-queries.json
    └── output-evals.json
```

`trigger-queries.json` contains realistic user prompts:

- `should_trigger: true` when the skill should activate.
- `should_trigger: false` for near-misses.
- `split: train` or `split: validation` to avoid overfitting descriptions to one prompt list.

`output-evals.json` contains task prompts, expected output summaries, and objective assertions.

## Validate Fixtures

```bash
python3 scripts/skill-eval-harness.py --plugin-skills plugin/skills --output /tmp/mdp-skill-evals
```

The script validates committed fixture shape and writes `/tmp/mdp-skill-evals/benchmark.json`. It does not observe live Codex skill activation. Use client transcripts, verbose logs, or a client-specific runner to score actual trigger rates.

## Iteration Loop

1. Add or update eval cases from real confusing tasks.
2. Run the harness to catch malformed fixture files.
3. Run the tasks in the target agent client with and without the skill, or against a previous skill version.
4. Grade assertions with concrete evidence.
5. Tighten `SKILL.md` descriptions and progressive references only where evals show confusion.
6. Keep generated run output under ignored scratch or `/tmp`; do not commit private transcripts.
