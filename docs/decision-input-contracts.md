# Decision Input Contracts

A Decision Input Contract is a versioned pack declaration of the data questions
a job must answer before deterministic MDP work can proceed.

It closes the gap between “normalize a row” and “what data should the upstream
system try to find?” The contract—not a generic people finder and not the
normalization prompt—owns that answer.

## Ownership

| Layer | Owns |
|---|---|
| MDP pack | questions, requirement class, applicability, value contracts, source policy, provenance, confidence, freshness, sensitivity, status behavior, decision effects |
| MDP CLI | contract validation, job-specific compilation, JSON Schemas, fit, routing, brief context, gaps, optional output checks |
| Customer or host | source access, collection attempts, provider credentials, paid normalization, copy generation, sequencing |

The deterministic CLI makes no network or model calls.

## Compile A Job Contract

```bash
mdp --json requirements --dir PACK_ROOT --job JOB_ID
```

The `mdp.requirements.v1` result is the handoff to a collector and normalization
host. It includes:

- bound contract IDs and versions;
- normalization prompt path and version;
- exact attribute questions and normalized output paths;
- `required`, `optional`, `conditional`, and `hard-gate` semantics;
- source classes and whether public research is permitted;
- effective behavior for all attempt statuses;
- source-attempt and normalized-output JSON Schemas;
- a deterministic `validate-prompt-output` handoff that checks the exact schema
  and every observed attribute-to-`normalized_prospect` output-path projection;
- explicit no-draft outcomes and layer boundaries.

Legacy jobs without a binding return `available: false`; existing packs keep
their `lead_input_requirements` fit/readiness behavior.

## Attempted-Complete Semantics

Every declared attribute must receive an attempt result:

| Status | Meaning |
|---|---|
| `observed` | a permitted source supplied a contract-valid value |
| `not_found` | the permitted attempt completed but no value was found |
| `not_applicable` | a declared applicability rule did not apply |
| `blocked` | access or policy prevented the attempt |
| `error` | the provider or host failed during the attempt |

These states are not interchangeable. In particular, `blocked` and `error` do
not prove absence, and a missing hard gate does not imply a safe value.

Required and conditional attributes receive conservative compiler defaults.
Optional attributes preserve gaps without blocking ordinary missing or
not-applicable cases. Hard gates must explicitly map all five statuses and must
include the `no-draft` decision effect.

Applicability dependencies must form an acyclic graph. A circular contract
cannot prove which conditional attempt applies, so validation rejects it
instead of allowing every member of the cycle to report `not_applicable`.
Unknown keys anywhere inside a decision input contract are also errors; a typo
must not silently remove a freshness, confidence, provenance, or status rule.

## Evidence And Normalization

An `observed` attribute can require:

- an attempt ID;
- source class and locator;
- observation timestamp and optional excerpt;
- minimum confidence;
- maximum freshness age;
- a sensitivity class.

Non-observed statuses do not fabricate observation evidence. `not_found`,
`not_applicable`, and `blocked` may stand alone; `error` requires a non-blank
error detail. Optional attempt provenance may still identify the attempted
source without pretending a value was observed.

`mdp.normalized-decision-input.v1` preserves those receipts beside the
provider-neutral `normalized_prospect`. It records the normalization prompt
version and always requires:

```json
{
  "outcome": "ready",
  "draft_allowed": false
}
```

`ready` here means the data may enter deterministic MDP evaluation. It does not
authorize copy. Only a later ready fit/brief decision may emit compiled context
to a separate generator or sequencer.

Before fit, validate the normalized envelope against both the compiled schema
and the pack's declared output-path projections:

```bash
mdp --json validate-prompt-output --strict \
  --dir PACK_ROOT \
  --prompt prompts/normalize-prospect.yaml \
  --file NORMALIZED_INPUT.json
```

The blocked normalization outcomes are:

- `insufficient-context`
- `disqualified`
- `human-review`
- `malformed`
- `provider-error`

## Authoring And Review

The official `mdp-pack-builder` skill authors the contract before the
normalization prompt. The official `mdp-pack-review` skill compiles and audits
each bound job. MDP intentionally does not ship a public people-finder or
source-collection skill.

Use the synthetic
[Clay Audiences self-serve enterprise expansion example](../examples/clay-audiences-self-serve-enterprise-expansion/README.md)
for a complete fifteen-attribute pack, exact request/response shapes, a visual
data flow, and the six expected outcomes. The example implements no hosted API,
authentication, billing, Cloudflare resources, or production data access.
