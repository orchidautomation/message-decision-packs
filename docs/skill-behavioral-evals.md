# Behavioral skill evaluations

The deterministic skill harness validates corpus shape, routing contracts, and
installed-file parity. It does **not** prove observed agent behavior. Behavioral
trials are a separate, operator-run evidence plane.

`plugin/skill-evals/behavioral-suite.json` selects a bounded held-out suite from
the canonical shared corpus. It repeats positive, out-of-scope, near-miss,
collision, typo, profile-crossing, unsafe, and indirect-intent trigger cases.
Output trials cover communication, Author/Use separation, safety, and artifact
handoff, comparing the current skill with a no-skill baseline and a frozen
previous skill tree.

```bash
python3 scripts/run-skill-behavioral-evals.py materialize \
  --out /tmp/mdp-agent-skills-eval-views
git archive <previous-commit> plugin/skills | tar -x -C /tmp/mdp-previous
python3 scripts/run-skill-behavioral-evals.py run \
  --previous-skills /tmp/mdp-previous/plugin/skills \
  --codex-home /tmp/isolated-codex-home \
  --out .agent-artifacts/mdp-262-trials
python3 scripts/run-skill-behavioral-evals.py aggregate \
  --results .agent-artifacts/mdp-262-trials/manifest.json \
  --out /tmp/mdp-262-public-report.json
```

The materialized per-skill `evals.json` files are AgentSkills-compatible views,
not new authorities. They retain the shared case IDs and revision and must be
regenerated from `plugin/skill-evals/`; do not hand-edit or package them as
runtime instructions.

Use a fresh temporary `CODEX_HOME` containing authentication only. The runner
uses ephemeral, read-only, no-resume Codex invocations in a sterile trial
directory. Each private record includes the exact prompt, input file digests,
skill revision, host/model label, output, elapsed time, token usage when the
host reports it, assertion evidence, and result. Never commit the raw directory:
it can contain model output, prompts, local paths, or other host metadata.

## Blind human review

1. Copy output text for the three comparison modes into randomly labeled A/B/C
   rows without mode, timing, token, or pass metadata.
2. Have a reviewer who did not run the trials grade task usefulness,
   communication, Author/Use separation, safety, and artifact handoff against
   the canonical assertions.
3. Record the reviewer, date, rubric revision, per-row verdict, rationale, and
   the label-to-mode reveal in a private review record.
4. Publish only aggregate counts and sanitized rationale. Do not publish raw
   transcripts, authentication, private paths, or unreviewed model output.

A report must retain `static_validation_separate: true`, state its single-host
limitations, and must not be marketed as a general benchmark.
