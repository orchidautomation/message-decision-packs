# Installed Template QA Recipes

Use this when testing a released installer, default GTM template, default proposal template, or fresh scratch workspace. This recipe tests the installed artifact, not whatever happens to be in the source checkout.

## Folder Rule

Never put two `.mdp` directories in one shared working folder. Use one isolated pack root per template.

```text
mdp-qa/
├── gtm/
│   └── .mdp/
└── proposal/
    └── .mdp/
```

Always pass `--dir`.

## Release Artifact Check

```bash
bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y
command -v mdp
mdp --version
```

Record the exact binary path and version in the QA notes.

## GTM Smoke

```bash
mkdir -p /tmp/mdp-template-qa/gtm
mdp --json init --template gtm --dir /tmp/mdp-template-qa/gtm
mdp --json validate --strict --dir /tmp/mdp-template-qa/gtm
mdp --json eval --strict --dir /tmp/mdp-template-qa/gtm
mdp --json --summary route --entries --eval-fixture --dir /tmp/mdp-template-qa/gtm --persona "GTM Engineering" --job "email outbound copy"
mdp --json gaps --dir /tmp/mdp-template-qa/gtm
mdp --json check-claims --dir /tmp/mdp-template-qa/gtm --text "We can auto-send every campaign and update the CRM for you."
```

Then validate at least one valid and one invalid prompt output:

```bash
mdp --json validate-prompt-output --dir /tmp/mdp-template-qa/gtm --prompt-id normalize-prospect-row --file <valid-output.json>
mdp --json validate-prompt-output --dir /tmp/mdp-template-qa/gtm --prompt-id normalize-prospect-row --file <invalid-output.json>
```

## Proposal Smoke

```bash
mkdir -p /tmp/mdp-template-qa/proposal
mdp --json init --template proposal --dir /tmp/mdp-template-qa/proposal
mdp --json validate --strict --dir /tmp/mdp-template-qa/proposal
mdp --json eval --strict --dir /tmp/mdp-template-qa/proposal
mdp --json --summary route --entries --eval-fixture --dir /tmp/mdp-template-qa/proposal --persona "Proposal Lead" --job "bid no bid review"
mdp --json --summary route --entries --eval-fixture --dir /tmp/mdp-template-qa/proposal --persona "Solution Owner" --job "compliance review"
mdp --json gaps --dir /tmp/mdp-template-qa/proposal
mdp --json check-claims --dir /tmp/mdp-template-qa/proposal --persona "Proposal Lead" --job "compliance review" --text "We are fully certified and already approved for this agency."
```

Then validate opportunity prompt outputs:

```bash
mdp --json validate-prompt-output --dir /tmp/mdp-template-qa/proposal --prompt-id normalize-opportunity --file <valid-output.json>
mdp --json validate-prompt-output --dir /tmp/mdp-template-qa/proposal --prompt-id normalize-opportunity --file <invalid-output.json>
```

## What Passed Means

- `validate --strict` proves the pack structure and profile activation gates are coherent.
- `eval --strict` proves the shipped template fixtures still pass.
- `validate-prompt-output` proves messy-source normalization cannot silently invent unsupported values.
- `route` proves the expected cards load for representative jobs.
- `gaps` proves missing information remains visible.
- `check-claims` proves risky generated claims are blocked or flagged before draft use.

## Closeout Template

```text
Installed mdp:
Template roots:
Commands run:
Passed:
Failed:
Files or issues to create:
Release/install uncertainty:
```
