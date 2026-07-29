# Decision Input Contracts

Read this when a job must articulate the data it needs before normalization or a
deterministic decision.

## Author The Decision Before The Prompt

1. Name the decision and the allowed outcomes.
2. List the smallest attributes that can change readiness, fit,
   disqualification, routing, brief content, gaps, human review, or no-draft
   behavior.
3. Write one answerable data question for each attribute.
4. Classify it as `required`, `optional`, `conditional`, or `hard-gate`.
5. Define its normalized `output_path` and bounded value contract.
6. For conditional attributes, declare exact `applies_when` dependencies.
7. Declare allowed source classes, required provenance fields, confidence,
   freshness, and sensitivity.
8. For every hard gate, explicitly map all five attempt statuses:
   `observed`, `not_found`, `not_applicable`, `blocked`, and `error`.
9. Bind the contract to the relevant `input_contracts[]` or `jobs[]`.
10. Run `mdp --json requirements --dir PACK_ROOT --job JOB_ID` and inspect the
    compiled request and normalized-output schemas before authoring the prompt.

The contract owns the questions. The prompt normalizes answers supplied by the
host. A collector may use customer-approved systems or permitted public
research, but that collection is not an MDP skill and does not run inside the
deterministic CLI.

## Minimal Shape

```yaml
decision_input_contracts:
- id: example.expansion
  version: 1.0.0
  description: Inputs for one bounded expansion decision.
  normalization:
    prompt: prompts/normalize-prospect.yaml
    prompt_version: example-expansion.v1
    normalized_schema_ref: mdp.normalized-decision-input.v1
  source_classes:
  - user_provided
  - customer_system
  - reviewed_internal
  - public_web
  - synthetic_fixture
  attributes:
  - id: company_domain
    question: What is the reviewed canonical company domain?
    output_path: company_domain
    value:
      type: string
    requirement: required
    decision_effects:
    - readiness
    - fit
    - gaps
    - no-draft
    source_classes:
    - user_provided
    - customer_system
    - public_web
    provenance:
      required: true
      required_fields:
      - attempt_id
      - source_class
      - source_locator
      - observed_at
    confidence:
      required: true
      minimum: 90
    freshness:
      required: true
      max_age_days: 365
      allow_unknown: false
    sensitivity: public
```

Required and conditional attributes receive conservative default status
behavior from the compiler. Optional attributes remain visible without
blocking ordinary missing/not-applicable cases. Hard gates have no defaults:
authors must make every outcome explicit.

## No-Draft Boundary

Normalization never drafts. The normalized envelope must set
`draft_allowed: false`. Only a later deterministic ready decision may release
compiled context to a customer-funded generator or sequencer. These outcomes
always block copy:

- `insufficient-context`
- `disqualified`
- `human-review`
- `malformed`
- `provider-error`

Do not convert missing, blocked, errored, or inapplicable attempts into invented
safe values.

## Synthetic Acceptance Coverage

Commit only synthetic or explicitly sanitized fixtures. Show:

- one attempted-complete source request;
- one schema-valid normalized response;
- ready and insufficient-context behavior;
- an observed disqualifying hard-gate value;
- a blocked or ambiguous hard gate requiring human review;
- a malformed contract/payload rejection;
- a provider error preserved outside the decision engine.

Keep hosted APIs, provider credentials, auth, billing, live data access, model
calls, copy generation, CRM writes, and sequencing outside the pack.
