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

## Bind An External Source

An integration can map its own fields to the compiled requirements without
moving collection into MDP. First compile the job, then create an
integration-owned `mdp.source-binding.v1` document outside the pack:

```bash
mdp --json requirements --dir PACK_ROOT --job JOB_ID
mdp --json schema source-binding
mdp --json validate-source-binding \
  --dir PACK_ROOT \
  --job JOB_ID \
  --file SOURCE_BINDING.json
```

The binding pins the exact pack ID/version/content digest, requirements digest,
job, Decision Input Contract ID/version receipts, binding release, and
normalization release. Every compiled attribute must appear exactly once using
the qualified `(decision_input_contract_id, attribute_id)` identity. The
validator rejects missing, duplicate, unknown, stale, requirement-class
mismatched, or source-class-incompatible entries. One external field key may
serve multiple requirements; field-key reuse is intentionally allowed.

The public contract keeps `system_of_record` and `acquisition_mode` open so the
same validator works for Clay, record grids, internal tools, and future
orchestrators. Provider-specific roles, credentials, legacy IDs, execution
state, and row results remain integration-owned.

The fixed value-to-attempt-status translation is:

| Integration observation | MDP attempt status |
|---|---|
| missing, null, empty, or whitespace-only | `not_found` |
| false or zero | `observed` |
| explicitly inapplicable | `not_applicable` |
| inaccessible or unmapped | `blocked` |
| runtime failure | `error` |

Both digests are deterministic. The pack digest covers regular files under
`.mdp` using portable relative paths and raw file bytes. The requirements
digest covers canonical JSON before its own digest field is added. Symlinks
fail closed.

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
Applicability dependencies must themselves be required or hard gates, so an
unresolved optional or conditional dependency cannot make a downstream
condition fail open.
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
version and the SHA-256 of the exact source-attempt request. The request carries
a trusted `as_of` timestamp; validation derives freshness from that anchor,
binds provenance attempt IDs to the matching requested attribute and source,
and requires the exact job-bound prompt. Every normalized envelope requires
`draft_allowed: false` and exactly one closed outcome. A ready envelope is:

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
  --source-attempt-request SOURCE_ATTEMPT_REQUEST.json \
  --collected-attempt-results COLLECTED_ATTEMPT_RESULTS.json \
  --file NORMALIZED_INPUT.json
```

Only `outcome: ready` may proceed to normalized prospect extraction. Every
other top-level outcome stops before fit, brief, or draft work. A non-observed
attribute must also leave its declared normalized prospect output absent or
neutral; it cannot smuggle an unverified value into deterministic evaluation.
The normalized attribute map must exactly equal the separately hashed
host-collected results ledger, and meaningful prospect fields without a
declared Decision Input `output_path` are rejected.

The blocked normalization outcomes are:

- `insufficient-context`
- `disqualified`
- `human-review`
- `malformed`
- `provider-error`

## Authoring And Review

The official `mdp-pack-builder` skill authors the contract before the
normalization prompt. The official `mdp-pack-review` skill compiles and audits
each bound job and any supplied source binding. The shared `mdp` skill routes
operators to the validator. MDP intentionally does not ship a public
people-finder, source-collection skill, or sixth integration skill.

Use the synthetic
[Clay Audiences self-serve enterprise expansion example](../examples/clay-audiences-self-serve-enterprise-expansion/README.md)
for a complete fifteen-attribute pack, exact request/response shapes, a visual
data flow, and the six expected outcomes. The example implements no hosted API,
authentication, billing, Cloudflare resources, or production data access.
