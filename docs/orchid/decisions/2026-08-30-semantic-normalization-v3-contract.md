# MDP Semantic Normalization v3 Contract

**Date:** 2026-08-30  
**Issue:** MDP-285  
**Status:** Approved product direction; implementation authority is the pinned MDP-285 plan  
**Decision type:** Additive public contract, pack-authoring contract, and runtime authority boundary

## Decision

MDP will add `mdp.normalized-decision-input.v3` as the first public normalized
decision-input contract whose canonical decision payload is profile neutral.
New v3 producers use `normalized_input`; `normalized_prospect`,
`normalized_opportunity`, `prospect-normalization`, and
`lead_input_requirements` remain compatibility surfaces for existing v0-v2
readers and packs.

The lifecycle is:

```text
bounded source-addressable evidence
  -> pack-compiled collection and classification specification
  -> model-owned semantic classification output
  -> host-owned v3 envelope and deterministic projection
  -> validation
  -> deterministic decision and routing
  -> minimal routed context
  -> governed generation
  -> receipts and verification
```

The model classifies evidence. It does not decide fit, pursuit, route,
readiness, draft eligibility, or receipt authority.

## Grounded current behavior

Repository inspection on `origin/main` at
`ecb9ad017942162ff62b1ca9fc1e7bd3416ab86c` confirms:

- `cli/src/models.rs::DecisionInputContract` owns normalization metadata,
  source classes, attributes, and signal projections. Its attributes do not
  distinguish directly observed facts from model-classified values.
- `cli/src/commands/requirements.rs::compile_contract` compiles every DIC
  attribute into one attempted-complete source contract.
- `cli/src/commands/requirements.rs::normalized_envelope_schema_v1` and its v2
  extension require `normalized_prospect`, model-echoed lineage hashes,
  `outcome`, and `draft_allowed`.
- `cli/src/commands/requirements.rs::validate_normalized_decision_input_with_projection`
  requires an `observed` attempt to equal its `normalized_prospect`
  projection, and forbids meaningful projection for a non-observed attempt.
  It therefore cannot honestly derive persona or segment from title,
  responsibilities, and company evidence.
- `cli/src/starter.rs::starter_decision_input_normalization_prompt` tells the
  model not to infer persona, segment, or trigger and asks it to echo hashes,
  attempts, outcome, and draft state.
- `cli/src/decision_input.rs::DecisionInput` is already a private neutral
  `fields`/`signals`/`attributes` representation. Proposal ingress still
  requires `normalized_prospect`; `normalized_opportunity` is an exact alias.
- `cli/src/run_runtime.rs::host_wrap_governed_output` already proves the
  correct trust pattern for governed generation: the provider sees a semantic
  schema, host-owned fields are rejected from model output, and the host
  constructs the sealed artifact. That mechanism is currently restricted to
  `governed-artifact`.
- `cli/src/run_runtime.rs::project_output_schema_for_openai` already projects
  canonical schemas into the supported provider schema subset before the
  request is sealed.
- `cli/src/profile_conformance.rs` contains a real cross-profile executable
  gate, but it does not yet prove model-derived classifications or a neutral
  v3 public wire.

The v3 change extends these seams. It does not create a second evidence ledger,
model runner, evaluator, or receipt system.

## Authored pack contract

### Attribute processing

Every v3 Decision Input attribute declares exactly one processing mode:

- `observed`: the host supplies an attempted-complete value and provenance.
  The value is projected deterministically after value-contract validation.
- `model-classified`: the attribute cites one canonical classification
  taxonomy and one or more observed contributor attributes. It is excluded
  from source-attempt requests and is populated only from a validated model
  classification.

There is no third `deterministic` input mode. Fit, pursuit, route, readiness,
draft permission, and other decisions are outputs of the deterministic engine,
not normalized input attributes.

For legacy DICs that omit `processing`, the reader treats the attribute as
`observed`. New v3-producing packs must write the field explicitly.

### Canonical classification taxonomies

Packs may define a top-level `classification_taxonomies` array. Each entry is
versioned and contains:

```yaml
- id: buyer-persona
  version: "1"
  output_attribute: persona
  contributor_attribute_ids: [person_title, responsibilities]
  source_classes: [public_web, customer_system, reviewed_internal]
  minimum_evidence:
    observed_contributors: 1
  basis_max_chars: 500
  ambiguity_policy: human-review
  no_match_policy: gap
  conflict_policy: human-review
  values:
    - value: GTM Systems Owner
      definition: Owns or builds technical systems used by go-to-market teams.
      positive_indicators:
        - owns GTM engineering or revenue automation
        - integrates CRM, enrichment, sequencing, or data systems
      exclusions:
        - uses sales tools without owning the system
        - purely quota-carrying seller
```

Required properties are ID, version, output attribute, contributor attributes,
source classes, minimum evidence, basis limit, three closed policies, and one
or more closed values. Each value requires a definition and may include
positive indicators and exclusions. Examples belong in eval fixtures rather
than becoming unbounded runtime prompt context.

One model-classified attribute references exactly one taxonomy. A taxonomy's
`output_attribute` must name that attribute, and its contributors must be
observed attributes in the same selected DIC set. Circular classification
dependencies are invalid. Taxonomy value domains must exactly equal the
attribute's existing value-contract enum so there is one allowed-value
authority.

The requirements compiler selects only taxonomies required by the current job
and computes a canonical SHA-256 over the selected taxonomy set. Prompt text,
skills, examples, and hosts may not maintain copied enum authorities.

## Collection boundary

The existing source binding, source-attempt request, and collected-attempt
results remain the lineage authority. For v3:

- source-attempt requests include only `observed` attributes;
- model-classified attributes are represented in the compiled classification
  specification, not as fake source attempts;
- contributor evidence must be observed, value-contract-valid, fresh enough,
  from an allowed source class, and bound to the supplied attempt/results
  hashes before it is eligible for classification;
- evidence content may be semantically messy, but each item remains bounded
  and source-addressable through the existing attempt/provenance records;
- the compiled collection specification describes required facts, evidence
  definitions, provenance, freshness, sensitivity, statuses, and output shape;
  it never names a retrieval provider or tool.

MDP does not attest that a source is true or that a host was authorized to
access it. Receipts prove artifact identity, declared lineage, and deterministic
validation only.

## Model-owned semantic payload

The provider receives a job-scoped schema for this semantic payload only:

```json
{
  "classifications": {
    "persona": {
      "status": "classified",
      "value": "GTM Systems Owner",
      "taxonomy_id": "buyer-persona",
      "taxonomy_version": "1",
      "derived_from": ["attempt-person-title", "attempt-responsibilities"],
      "basis": "The title and responsibilities assign ownership of GTM systems."
    }
  },
  "gaps": [],
  "rejected_claims": []
}
```

Allowed classification statuses are:

- `classified`: requires one allowed value, at least the taxonomy's minimum
  eligible evidence, and a bounded basis;
- `ambiguous`: forbids a selected value and requires two or more eligible
  evidence references or conflicting candidate support;
- `no-match`: forbids a selected value and records that eligible evidence did
  not satisfy any taxonomy value;
- `unsupported`: forbids a selected value and records that supplied evidence
  was ineligible or insufficient.

`derived_from` contains existing source-attempt IDs only. It is required and
non-empty for every status so the result records what the model evaluated.
`basis` is plain text, contains no control characters, is bounded by the
taxonomy (maximum 500 characters in v3), and is explanatory rather than
authoritative. Numeric model self-confidence is not accepted in the semantic
classification object.

The model never returns the v3 contract name, job identity, hashes, observed
attempts, normalized input projection, outcome, readiness, route, draft
permission, timestamps, or receipts. Presence of any host-owned top-level field
in provider output is an injection failure.

## Host-owned sealed v3 envelope

After validating provider semantics, the runtime constructs:

```json
{
  "contract": "mdp.normalized-decision-input.v3",
  "job_id": "prospect-fit-or-brief",
  "decision_input_contracts": ["gtm.prospect-context"],
  "normalization": [{
    "contract_id": "gtm.prospect-context",
    "prompt": "prompts/normalize-prospect.yaml",
    "prompt_version": "gtm-prospect-context.v3",
    "prompt_sha256": "..."
  }],
  "requirements_sha256": "...",
  "taxonomy_set_sha256": "...",
  "source_binding_sha256": "...",
  "source_attempt_request_sha256": "...",
  "collected_attempt_results_sha256": "...",
  "attributes": {},
  "classifications": {},
  "signal_observations": [],
  "normalized_input": {
    "fields": {},
    "signals": [],
    "attributes": {}
  },
  "gaps": [],
  "rejected_claims": []
}
```

The host copies validated observed attempts into `attributes`, deterministically
projects observed values, inserts validated classified values at their authored
output paths, and deterministically constructs eligible signal observations.
The resulting `normalized_input` shape matches the existing private
`DecisionInput` boundary: bounded `fields`, `signals`, and `attributes`.
Profile-owned field names are allowed inside `fields`; the neutral core does
not require or interpret GTM/proposal vocabulary.

The sealed envelope deliberately omits model-authored `outcome` and
`draft_allowed`. Deterministic readiness/fit/decision/routing consumes the
validated attempt and classification statuses and produces its existing
versioned decision artifacts. An ambiguous, no-match, unsupported, missing,
stale, blocked, or conflicting state may be a valid sealed normalization
artifact while still deterministically preventing a ready route.

Compact receipts bind the requirements hash, selected taxonomy-set hash,
prompt hash, evidence artifact hashes, normalized output hash, and decision
artifacts. They do not copy raw evidence excerpts or model basis text unless an
existing explicit artifact contract requires it.

## Validation order

The runtime fails closed in this order:

1. Pack, job, DIC, taxonomy, and prompt resolution.
2. Canonical v3 schema compilation and provider-schema projection.
3. Source binding, request, results, provenance, freshness, sensitivity, and
   value-contract validation.
4. Provider call through the existing native driver.
5. Strict semantic JSON/schema validation.
6. Host-owned field injection check.
7. Taxonomy/value/status/basis validation.
8. `derived_from` existence, eligibility, contributor, source-class, freshness,
   minimum-evidence, and conflict validation.
9. Deterministic construction of the sealed v3 envelope.
10. Full canonical-envelope validation and hashing.
11. Deterministic decision/routing.
12. Governed generation and receipt verification when the route permits it.

Provider schema projection occurs before a request is sealed or sent. A schema
that cannot be represented by the configured provider profile returns a stable
policy-blocked diagnostic. The pilot pins the current supported OpenAI driver;
provider-neutral expansion may add projection profiles later without changing
the canonical v3 schema.

## Compatibility and migration

- v1/v2 schemas, readers, validators, hashes, and valid historical artifacts
  remain supported.
- New v3-producing prompts emit semantic fields only; the host seals the v3
  artifact.
- `normalized_prospect` and `normalized_opportunity` are not permitted in v3.
- A payload mixing `normalized_input` with either legacy alias fails closed.
- Proposal v0 alias equality remains unchanged for legacy artifacts.
- Legacy DIC attributes without `processing` read as `observed`; v3 pack
  validation requires explicit processing.
- There is no automatic semantic migration because MDP cannot infer a
  customer's taxonomy safely. Health diagnostics identify the exact attributes
  requiring author review. MDP-59 remains parked until repeated migration
  evidence justifies `migrate --dry-run`.
- MDP-26 is resolved by the generic v3 input: proposal terminology stays in
  the proposal profile, while pursuit is deterministic. No second core
  opportunity/pursuit object is introduced.

## Profile examples

### GTM

Observed `person_title`, `responsibilities`, and company evidence may classify
`persona` and `segment`; fit and why-now remain separate evidence roles. The
deterministic evaluator decides fit and route.

### Proposal

Observed RFP/opportunity facts may classify stage, category, or risk when the
proposal pack defines those taxonomies. Deterministic policy decides pursue,
review, or decline. v3 never requires `normalized_prospect`.

### Neutral test-only support fixture

Observed ticket impact and logs may classify issue category or severity inputs;
deterministic policy chooses priority/escalation. This proves the contract
without registering or shipping a support profile.

## Local and cloud disposition

The pilot normalizes raw evidence locally through the released CLI/MCP/runtime.
Future cloud evaluation may consume a sealed v3 envelope and compact lineage
through tenant-scoped opaque references. A future hosted raw-evidence
normalization endpoint requires a separate retention, redaction, deletion,
tenant-isolation, and authorization decision under MDP-154/187. This decision
does not authorize cloud implementation or raw evidence upload.

## Non-goals

No provider-specific collection adapter, new runner, CRM/browser integration,
cloud runtime, third production profile, numeric model-confidence gate, or
replacement evaluator/receipt system is authorized.

## Rollback

The implementation is additive. Before release, rollback is branch/PR revert.
After release, disable v3 production while retaining v1/v2 readers; no stored
legacy artifact requires rewriting. Pack releases that opt into v3 remain
versioned and can be rolled back to their preceding release rather than being
silently mutated.

