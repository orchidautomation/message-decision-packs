# Installed Template QA

Read this only when testing a released MDP install or a freshly initialized template.

## Rules

- Test the installed binary, not only a source build.
- Record `command -v mdp` and `mdp --version`.
- Use isolated GTM and proposal roots under temporary storage.
- Pass explicit `--dir` to every command.
- Record commands, pass/fail, confusion, and installed-versus-release uncertainty.

## Minimum Matrix

For each template:

1. Initialize with the installed binary.
2. Run `skills`, `validate --strict`, `eval --strict`, and `gaps`.
3. Resolve every declared job ID with `skills --job`.
4. Test one representative card route.
5. Validate one good and one adversarial prompt output when the template includes normalization.
6. Exercise claim or proof-output checks relevant to the profile.

Confirm installed skill IDs equal the five CLI `packaged_skill_ids`. Packaging byte fidelity is validated separately by the repository packaging gate.
