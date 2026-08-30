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
    compiled request, collected-results, and normalized-output schemas before
    authoring the prompt.
11. When repeated sourced observations affect the decision, declare
    `signal_projections` with stable IDs, profile-owned kinds, explicit closed
    roles, attribute contributors, bounded cardinality, conflict policy, and
    decision effects. Use v2 normalization; do not widen the v1 envelope.

## Semantic v3: Observe Facts, Classify Taxonomy Values

For a v3-producing contract, declare `processing` explicitly on every
attribute:

- `observed` means the host must collect a bounded, source-addressable fact.
  It belongs in the collection specification and source-attempt ledger.
- `model-classified` means the normalizer derives one closed value from
  observed contributor attempts. It never receives a fake source attempt.

Define classification meaning once in the manifest-level
`classification_taxonomies` array. Each taxonomy must pin an ID and version,
the classified output attribute, observed contributor attribute IDs, eligible
source classes, nested `minimum_evidence.observed_contributors`, a bounded
`basis_max_chars`, the closed ambiguity/no-match/conflict policies, and its
closed values. Each value requires a definition; add positive indicators and
exclusions when they materially distinguish nearby values. The classified
attribute's value enum must exactly equal the taxonomy values. Do not copy
definitions or allowed values into prompts, skills, or host code as a second
authority.

The job-scoped compiler emits two different artifacts:

- `collection_specification`: what observed evidence the host must supply,
  including provenance, freshness, sensitivity, statuses, and value shape;
- `classification_specification`: the exact selected taxonomy definitions,
  criteria, policies, evidence minimum, and basis bound, plus a canonical
  taxonomy-set hash.

These contracts specify **what** must be supplied and classified, never where
to retrieve it or which tool to use. Keep Monid, Deepline, Clay, browser,
customer-system, and other acquisition instructions in host orchestration,
not pack authority. A classification basis explains the mapping and cites
`derived_from` attempt IDs; it is not proof or model confidence.

The contract owns the questions. The prompt normalizes answers supplied by the
host. A collector may use customer-approved systems or permitted public
research, but that collection is not an MDP skill and does not run inside the
deterministic CLI.

## Legacy v1/v2 Minimal Shape

For a newly generated GTM pack, the minimum prospect contract must answer the
person/account identity, persona and segment, why-now trigger, and reviewed
contact-policy questions needed by the three canonical prospect jobs. Bind the
contract through their shared prospect `input_contracts[]` entry or bind it to
each job directly. If the decision consumes repeated sourced trigger or
contact-policy observations, declare explicit projections and use v2; a
scalar-only decision may remain v1. Do not present the pack as governed until
all three `requirements --job` calls are available and return the expected
contract ID/version, requirements digest, schemas, and runtime version.

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

Signal roles are limited to `fit`, `why-now`, `person-resolution`, and
`disqualifier`. They never come from signal titles, provider field names,
source prose, or legacy keywords. Use only `require-agreement` or
`any-disqualifies`; unresolved disagreement is human-review/no-draft and no
positive winner-selection policy is allowed.

## No-Draft Boundary

Normalization never drafts. Legacy v1/v2 normalized envelopes must set
`draft_allowed: false`; sealed v3 envelopes omit model-authored outcome and
draft fields entirely. Only a later deterministic ready decision may release
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
- one separately hashed collected-results ledger whose immutable statuses,
  evidence, confidence, freshness, and errors exactly bind the normalized
  attribute map while raw values may canonicalize into declared value
  contracts;
- one schema-valid normalized response;
- ready and insufficient-context behavior;
- an observed disqualifying hard-gate value;
- a blocked or ambiguous hard gate requiring human review;
- a malformed contract/payload rejection;
- a provider error preserved outside the decision engine.

Name or otherwise identify the six closed scenarios: `attempted-complete`,
`insufficient`, `disqualified`, `human-review`, `malformed`, and
`provider-error`. Every normalization scenario keeps `draft_allowed: false`;
`ready` permits deterministic evaluation, not copy generation.

Reject meaningful normalized prospect fields without a declared `output_path`;
only compiler-declared non-decision provenance/safety markers may remain
unbound.

Keep hosted APIs, provider credentials, auth, billing, live data access, model
calls, copy generation, CRM writes, and sequencing outside the pack.

For manual adoption, preserve v1 fixtures, change only the opted-in contract to
`mdp.normalized-decision-input.v2`, compile `requirements --job`, validate the
integration-owned v2 binding, request, collected results, and normalized
output, then run fit/brief through `--normalized-input`. Use the synthetic Clay
example as the reference. `lineage-validated` proves internal linkage only,
not host authenticity or observation truth.
