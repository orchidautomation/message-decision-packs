# Installed Template QA

Use this when the objective is to test a released MDP install or a freshly initialized GTM/proposal template.

## Rules

- Test the installed `mdp` binary, not only `cargo run`.
- Use one isolated pack root per template.
- Pass explicit `--dir` to every command.
- Keep QA outputs under ignored scratch or `/tmp`; do not commit private test runs.
- Record version, binary path, commands, pass/fail, and filed issues.

## Commands

```bash
command -v mdp
mdp --version

mkdir -p /tmp/mdp-installed-template-qa/gtm
mdp --json init --template gtm --name "Synthetic GTM QA" --dir /tmp/mdp-installed-template-qa/gtm
mdp --json validate --strict --dir /tmp/mdp-installed-template-qa/gtm
mdp --json eval --strict --dir /tmp/mdp-installed-template-qa/gtm
mdp --json --summary route --entries --eval-fixture --dir /tmp/mdp-installed-template-qa/gtm --persona "GTM Engineering" --job "email outbound copy"
mdp --json gaps --dir /tmp/mdp-installed-template-qa/gtm

mkdir -p /tmp/mdp-installed-template-qa/proposal
mdp --json init --template proposal --dir /tmp/mdp-installed-template-qa/proposal
mdp --json validate --strict --dir /tmp/mdp-installed-template-qa/proposal
mdp --json eval --strict --dir /tmp/mdp-installed-template-qa/proposal
mdp --json --summary route --entries --eval-fixture --dir /tmp/mdp-installed-template-qa/proposal --persona "Proposal Lead" --job "bid no bid review"
mdp --json gaps --dir /tmp/mdp-installed-template-qa/proposal
```

Prompt-output validation is covered by strict template evals, but direct prompt-output files should be checked when investigating normalization bugs:

```bash
mdp --json validate-prompt-output --dir <pack-root> --prompt-id normalize-prospect-row --file <gtm-output.json>
mdp --json validate-prompt-output --dir <pack-root> --prompt-id normalize-opportunity --file <proposal-output.json>
```

## Closeout

Report:

- installed binary path and version
- exact pack roots
- validation/eval status
- route and prompt-output checks run
- confusing output or stale examples
- issues filed or linked
