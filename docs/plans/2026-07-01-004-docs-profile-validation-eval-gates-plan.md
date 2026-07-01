---
title: MDP-40 Profile Validation and Eval Gates - Plan
type: docs
date: 2026-07-01
topic: mdp-profile-validation-eval-gates
execution: knowledge-work
linear_project: MDP: Domain Profile Foundation
linear_issues:
  - MDP-36
  - MDP-39
  - MDP-40
  - MDP-50
  - MDP-41
origin:
  - docs/plans/2026-07-01-001-docs-domain-profile-foundation-plan.md
  - docs/plans/2026-07-01-002-docs-card-extensibility-primitive-map-plan.md
  - docs/plans/2026-07-01-003-docs-account-context-icp-normalization-plan.md
---

# MDP-40 Profile Validation and Eval Gates - Plan

## Goal Capsule

| Field | Decision |
|---|---|
| Objective | Define profile-aware validation and eval gates before profile metadata becomes executable behavior. |
| Product authority | This artifact resolves the validation/eval policy for MDP-40 using the accepted MDP-39 and MDP-50 plans. |
| Core decision | Split structural validity from profile activation readiness. Warnings should not make ordinary validation invalid unless `--strict` or activation mode is used. |
| Eval stance | Profiles need minimum eval categories before activation: proceed, insufficient-context, refusal/disqualification/escalation, unsafe-output or unsupported-claim, and job-routing. |
| Account-context stance | MDP-50 adds required GTM account-context cases for profile activation: account context present, account context missing, account-only no-draft, and prompt-output validation. |
| Stop condition | Do not implement profile validation, eval fixture schema changes, or template metadata from this planning branch. |

---

## Product Contract

### Summary

A profile-aware pack should not become trusted just because its files parse.
It needs two separate checks:

```text
Structural validation:
  Are the files parseable, supported, and internally consistent?

Profile activation readiness:
  Does the profile cover its required primitives, jobs, input contracts, prompts, gaps, and evals well enough for bounded agent work?
```

Current `mdp validate` already catches many structural problems, but its JSON contract treats any issue as invalid because `valid` is derived from `issues.is_empty()`.
That makes warning-first profile validation impossible unless MDP separates error count from warning count.

The right MDP-40 policy is:

```text
Errors fail ordinary validation.
Warnings remain visible but do not fail ordinary validation.
--strict promotes warnings to failures.
Activation mode treats missing required profile coverage as blocking.
```

### Current Behavior Verified

Current repo behavior:

| Surface | Current fact | Implication |
|---|---|---|
| `cli/src/models.rs` | `Manifest` has no profile metadata fields yet; `CardKind` remains fixed. | Profile-aware validation needs optional structs before template metadata can validate cleanly. |
| `cli/src/commands/health.rs` | `validate_pack` returns `valid: issues.is_empty()`. Warnings and errors both make `valid: false`. Unknown manifest fields warn as unsupported. | Warning-first profile validation needs severity-aware validity. |
| `cli/src/app.rs` | `--strict` exists for `validate`, `eval`, `validate-prompt-output`, and `check-claims`; strict mode can promote warnings to invalid after command output is built. | Strict plumbing exists, but non-strict validity needs to stop failing on warnings. |
| `cli/src/commands/evals.rs` | Eval fixtures support `route`, `fit`, `brief`, and `check-claims` plus basic expected output assertions. | Profile eval gates need fixture metadata and likely a prompt-output validation command path. |
| `cli/src/commands/schemas.rs` | Manifest and eval schemas do not expose profile metadata, primitive maps, jobs, input contracts, or eval categories. | Schema output must change with implementation. |
| `plugin/assets/templates/basic/.mdp/evals/*.yaml` | Existing basic evals already cover route, fit, brief no-draft, disqualification, and claim checks, but do not label profile coverage categories. | Existing fixtures can be reused once categorized. |

### Validation Modes

MDP should support three related modes.

| Mode | User action | What fails | Intended use |
|---|---|---|---|
| Structural default | `mdp validate --dir .` | Errors only. Warnings are visible but `valid` can remain true. | Normal authoring, current packs, backwards-compatible checks. |
| Strict validation | `mdp validate --strict --dir .` and `mdp eval --strict --dir .` | Errors and warnings. | CI, release, review, and agent handoff when warnings should block. |
| Profile activation | Future explicit activation gate or strict profile check. | Errors plus missing required primitive/job/input/eval coverage. | Enabling profile-aware bounded agent work. |

Do not make existing packs without profile metadata fail or warn in the first implementation slice.
Profile checks should run only when profile metadata is present or when a caller explicitly asks for profile compliance.

### Severity Policy

| Condition | Default severity | Strict behavior | Activation behavior |
|---|---|---|---|
| Malformed profile field type | Error | Error | Blocks activation |
| Unknown primitive ID | Error | Error | Blocks activation |
| `primitive_map` references missing card, prompt, input contract, job, or eval path | Error | Error | Blocks activation |
| Declared required primitive has no mapped source | Warning | Fails strict | Blocks activation |
| Job requires a primitive with no mapped source | Warning | Fails strict | Blocks that job |
| Profile declares an input contract but prompt/schema is missing | Error | Error | Blocks activation |
| Minimum profile eval category is missing | Warning | Fails strict | Blocks activation |
| Eval fixture exists but fails | Error | Error | Blocks activation |
| GTM profile claims account context but maps no prompt/input/card/gap source | Warning | Fails strict | Blocks jobs that require account context |
| Existing pack has no profile metadata | No issue | No issue | Not profile-activated |

### JSON Contract Shape

Future validation output should preserve the existing `issues` array but add counts and activation details so agents do not infer validity from list length.

Recommended shape:

```json
{
  "valid": true,
  "error_count": 0,
  "warning_count": 2,
  "issues": [
    {
      "code": "profile_required_primitive_unmapped",
      "severity": "warning",
      "path": ".mdp/manifest.yaml#/required_primitives/3",
      "message": "required primitive needs-requirements has no mapped cards, prompts, input contracts, jobs, or evals",
      "strict": "fails",
      "activation": "blocks"
    }
  ],
  "profile": {
    "id": "gtm",
    "present": true,
    "activation_ready": false,
    "missing_required_primitives": ["needs-requirements"],
    "eval_categories": {
      "proceed": "present",
      "insufficient-context": "present",
      "refusal": "missing",
      "unsafe-output": "present",
      "job-routing": "present"
    }
  }
}
```

`valid` should mean "no errors in the current mode."
`activation_ready` should mean "profile coverage and required eval gates are satisfied."

### Minimum Eval Categories

A profile should not activate until it has fixtures for these categories:

| Category | Purpose | Existing GTM examples |
|---|---|---|
| `proceed` | Proves a known-good input can pass. | `fit-good.yaml`. |
| `insufficient-context` | Proves missing context causes no-draft or insufficient-context behavior. | `fit-insufficient-context.yaml`, `brief-insufficient-context.yaml`. |
| `refusal` | Proves disqualification, refusal, or escalation behavior. | `fit-disqualified.yaml`. |
| `unsafe-output` | Proves unsupported claims or output constraints fail. | `claim-check-unsupported.yaml`, `claim-check-output-rule.yaml`. |
| `job-routing` | Proves named work modes route the expected cards and exclude unrelated entries. | `linkedin-copy-route.yaml`, `email-initial-route.yaml`, `call-prep-route.yaml`. |

For GTM account-context activation, add these MDP-50-derived cases:

| Category | Purpose |
|---|---|
| `account-context-present` | Proves account/company context routes before fit gates for company-profile questions. |
| `account-context-missing` | Proves missing company proof is surfaced as a gap instead of invented. |
| `account-only-no-draft` | Proves account-only input with no person name/title stays insufficient-context and cannot produce a draft. |
| `prompt-output-validation` | Proves invalid or invented account/person prompt output is rejected before fit/brief. |

### Eval Fixture Metadata

Current eval fixtures can keep their command/assertion structure.
Profile-aware eval gates need optional metadata, not a new fixture directory.

Recommended additive fixture fields:

```yaml
id: fit-good
command: fit
profile_eval:
  category: proceed
  primitives:
    - actors
    - decision-criteria
    - source-signals
  jobs: []
```

For prompt-output cases, add a new eval command:

```yaml
id: account-only-normalization-output
command: validate-prompt-output
profile_eval:
  category: prompt-output-validation
  primitives:
    - actors
    - source-signals
    - gaps
prompt_id: normalize-prospect-row
prompt_output:
  contract: mdp.prompt-output.v0
  prompt_id: normalize-prospect-row
  ...
expect_valid: false
```

This keeps `mdp eval` as the pack-level proof command while reusing the existing prompt-output validator.

### Primitive Coverage Rules

Profile validation should compute coverage from manifest-level `primitive_map`.

For each declared required primitive:

1. Validate that the primitive ID is known.
2. Validate all mapped references exist:
   - `cards` references match `cards[].id`.
   - `prompts` references match prompt IDs.
   - `input_contracts` references match declared input contract IDs.
   - `jobs` references match declared job IDs.
   - `evals` references resolve to existing `.mdp/evals/*.yaml` paths or fixture IDs, depending on the accepted schema.
3. If the primitive has no mapped source, warn in default mode and block activation.
4. For each job, validate that every `required_primitives` entry is known and has mapped coverage.

Coverage should not require every primitive to map to a card.
MDP-39 and MDP-50 both require mappings to support prompts, input contracts, evals, and jobs because account context can be input-contract-backed rather than card-backed.

### Account-Context Gate

MDP-50 makes account context the first GTM stress test.
Profile activation for the GTM profile should require:

- `input_contracts.prospect.normalizes` includes `account`, `person`, and `relationship`;
- `primitive_map.actors` includes `input_contracts: [prospect]`;
- `primitive_map.source-signals` includes account signal coverage through `signals`, `normalize-prospect-row`, or both;
- `primitive_map.gaps` includes account missing-proof or insufficient-context coverage;
- eval categories include `account-only-no-draft` and `prompt-output-validation` before account-aware jobs are strict.

This does not require a new `account-context` card ID in the first implementation slice.

---

## Planning Contract

### Key Decisions

- KTD1. `mdp validate` default mode should fail only on errors; warnings should be visible but not make `valid: false`.
- KTD2. `--strict` should continue to make warnings fail validation-style flows.
- KTD3. Profile activation readiness is a separate concept from file validity.
- KTD4. Existing packs with no profile metadata should remain valid and should not receive missing-profile warnings in the first slice.
- KTD5. `primitive_map` references are strict: unknown primitive IDs and missing mapped references are errors.
- KTD6. Missing required primitive coverage is warning-first in default validation and blocking in strict/activation.
- KTD7. Eval fixtures should get additive `profile_eval` metadata for categories and primitive coverage.
- KTD8. Add `validate-prompt-output` as an eval command so prompt contracts can be part of profile activation evidence.
- KTD9. Account-context gates come from MDP-50 and should be handled through input contracts, prompts, signals, gaps, and evals before any dedicated account-context card exists.

### Implementation Surface For Follow-Up Issues

| Surface | Future change | Notes |
|---|---|---|
| `cli/src/models.rs` | Add optional manifest structs for `profile`, `required_primitives`, `primitive_map`, `input_contracts`, and `jobs`; add optional eval fixture metadata structs. | Keep fixed `CardKind`. |
| `cli/src/commands/health.rs` | Add profile validation, severity counts, warning-valid default behavior, and activation readiness summary. | Existing issue helper can remain, but validity cannot be `issues.is_empty()`. |
| `cli/src/app.rs` | Preserve strict behavior while aligning `valid` with severity-aware default semantics. | `apply_strict` already has useful plumbing. |
| `cli/src/commands/evals.rs` | Add `profile_eval` metadata parsing, category coverage summary, and optional `validate-prompt-output` fixture command. | Existing fixtures should still run unchanged. |
| `cli/src/commands/schemas.rs` | Extend manifest and eval schemas with profile metadata and profile eval metadata. | Do not expand the card kind enum. |
| `cli/src/starter.rs` | Add profile metadata and categorized eval fixtures only after validation support lands. | Avoid making older validators complain in the starter before support exists. |
| `plugin/assets/templates/basic/.mdp/manifest.yaml` | Add GTM profile metadata after the validator supports it. | No card rename or new account-context card in first slice. |
| `plugin/assets/templates/basic/.mdp/evals/*.yaml` | Add `profile_eval` categories to existing eval fixtures; add account-context/prompt-output fixtures from MDP-50. | Keep synthetic fixtures. |
| `plugin/skills/` | Update skills when validation/eval behavior changes. | Feature change hygiene requires matching agent-facing instructions. |

### Test Strategy For Future Implementation

- `validate_pack`: warnings do not make non-strict validation invalid.
- `validate_pack --strict`: warnings make validation invalid and populate strict warning details.
- Existing basic template without profile metadata validates unchanged.
- Profile metadata with known primitives and valid references validates with `valid: true`.
- Unknown primitive ID fails as an error.
- Missing mapped card/prompt/input/job/eval reference fails as an error.
- Declared required primitive with no mapped source is warning in default mode and invalid in strict mode.
- Job requiring an unmapped primitive warns in default mode and blocks that job in activation summary.
- Eval fixtures without `profile_eval` metadata still run.
- Eval fixtures with `profile_eval.category` populate the category summary.
- Missing minimum eval category warns in default mode and blocks activation.
- New `validate-prompt-output` eval command can assert prompt-output validator success/failure.
- GTM account-only no-draft fixture proves account-only input cannot generate a draft.

Run after implementation:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json validate --strict --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json eval --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json eval --strict --dir plugin/assets/templates/basic
make validate
```

Planning-only changes only require document review and diff hygiene.

### Sequencing

1. Implement severity-aware validation output before adding profile metadata to templates.
2. Add optional profile manifest structs and schema support while keeping existing packs valid.
3. Add `primitive_map`, job, input contract, and eval reference validation.
4. Add profile eval metadata and category summary.
5. Add `validate-prompt-output` eval fixture command.
6. Add GTM profile metadata to the basic template.
7. Categorize current basic evals and add MDP-50 account-context cases.
8. Update plugin skills and public docs in the same behavior-changing PR.
9. Let MDP-41 consume these gates in the profile-builder workflow.

---

## Sources

- MDP-37/MDP-38 foundation artifact: `docs/plans/2026-07-01-001-docs-domain-profile-foundation-plan.md`.
- MDP-39 accepted plan: `docs/plans/2026-07-01-002-docs-card-extensibility-primitive-map-plan.md`.
- MDP-50 accepted plan: `docs/plans/2026-07-01-003-docs-account-context-icp-normalization-plan.md`.
- Linear issue: MDP-40.
- Current validation and strict-mode surfaces: `cli/src/commands/health.rs`, `cli/src/app.rs`.
- Current eval surfaces: `cli/src/commands/evals.rs`, `cli/src/commands/schemas.rs`, `plugin/assets/templates/basic/.mdp/evals/*.yaml`.
- Current manifest/model surfaces: `cli/src/models.rs`, `plugin/assets/templates/basic/.mdp/manifest.yaml`.
- Prompt-output validation surfaces: `cli/src/commands/prompt_output.rs`, `plugin/assets/templates/basic/.mdp/prompts/normalize-prospect.yaml`.
