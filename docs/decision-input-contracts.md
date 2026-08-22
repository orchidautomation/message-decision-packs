# Decision Input Contracts

A Decision Input Contract is a versioned pack declaration of the data questions
a job must answer before deterministic MDP work can proceed.

It closes the gap between “normalize a row” and “what data should the upstream
system try to find?” The contract—not a generic people finder and not the
normalization prompt—owns that answer.

## Ownership

| Layer | Owns |
|---|---|
| MDP pack | questions, requirement class, applicability, value contracts, signal kinds and roles, source policy, provenance, confidence, freshness, sensitivity, conflict policy, status behavior, decision effects |
| MDP CLI | contract validation, job-specific compilation, JSON Schemas, fit, routing, brief context, gaps, optional output checks |
| Customer or host | source access, collection attempts, provider credentials, paid normalization, copy generation, sequencing |

The deterministic CLI makes no network or model calls.

## Compile A Job Contract

```bash
mdp --json requirements --dir PACK_ROOT --job JOB_ID
```

Start with `mdp --json capabilities`, but use the selected command's `--help`
and `requirements --job` output as the exact installed contract. The result is
`mdp.requirements.v1` for scalar-only jobs or `mdp.requirements.v2` for a job
whose bound Decision Input Contract declares signal projections. It is the
handoff to a collector and normalization host. It includes:

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

## New Generated GTM Packs

Fresh `mdp init --template gtm` output declares
`gtm.prospect-context@1.0.0` and binds it transitively through the shared
`prospect` input contract. The binding covers all three prospect-driven
canonical jobs:

- `prospect-fit-or-brief`
- `outbound-copy-brief`
- `outbound-copy-review`

The minimum contract asks for reviewed person and company identity, persona,
segment, a why-now trigger, and contact policy. Trigger and contact-policy
decisions may contain repeated sourced observations, so the starter declares
explicit signal projections and compiles the signal-aware v2 artifact matrix.
The generated `examples/decision-input-scenarios.json` fixture records the
attempted-complete, insufficient, disqualified, human-review, malformed, and
provider-error outcomes without performing collection or provider execution.

A generated pack must not be presented as governed or self-standing until
`requirements --job` reports `available: true` with the expected contract
ID/version, requirements digest, schemas, and runtime version for every
prospect-driven canonical job. Prompt prose, normalized field names,
`signals`, and `lead_input_requirements` never imply a Decision Input Contract.

This is a greenfield authoring rule, not a silent migration. A genuinely legacy
pack with no direct or transitive binding stays structurally compatible under
ordinary non-strict validation, reports the Decision Input contract as
unavailable, and remains legacy/unassessed rather than governed.

## Public Contract-Version Matrix

`data.contract_version_matrix` is authoritative for the selected job. The
public matrix is:

| Artifact | Scalar v1 | Signal-aware v2 |
|---|---|---|
| Requirements | `mdp.requirements.v1` | `mdp.requirements.v2` |
| Source binding | `mdp.source-binding.v1` | `mdp.source-binding.v2` |
| Source-attempt request | `mdp.source-attempt-request.v1` | `mdp.source-attempt-request.v2` |
| Collected attempt results | `mdp.collected-attempt-results.v1` | `mdp.collected-attempt-results.v2` |
| Normalized output | `mdp.normalized-decision-input.v1` | `mdp.normalized-decision-input.v2` |
| Post-validation signal receipt | not applicable | `mdp.signal-projection-decision-receipt.v1` |

The v2 normalized-output SHA-256 appears only in the post-validation receipt;
it cannot appear inside the output bytes being hashed. MDP rejects:

- v1 requirements with any v2 lineage artifact;
- v2 requirements with any v1 binding, request, results, or normalized output;
- a v1 source-binding hash inside a v2 request; and
- different source-binding hashes across the v2 request, results, and
  normalized output.

V1 and v2 are distinct job-execution paths, not fields to combine. Existing
scalar-only packs and prospect JSON stay valid. Their signal strings remain
readable as `legacy` or `unassessed` context but cannot satisfy a v2 role.

## First-Class Signal Projections

A Decision Input Contract may add `signal_projections` beside scalar
attributes. Each projection declares a stable ID qualified by its contract ID,
a profile-defined `kind`, zero or more engine-owned roles, contributing
attribute IDs, cardinality, a conservative conflict policy, and decision
effects. The closed roles are `fit`, `why-now`, `person-resolution`, and
`disqualifier`.

Roles are explicit authority. MDP never infers them from a signal title, ID,
free text, provider field name, source label, or arbitrary prospect attribute.
Changing prose without changing the declared roles and receipts cannot change
qualification. Structured repeated observations exist only in
`mdp.normalized-decision-input.v2`; putting structured observations in a
legacy prospect `signals` array does not upgrade them.

Once a v2 envelope is lineage-validated, its accepted signal projections are
the canonical signal representation for fit and brief readiness. A pack may
retain legacy `lead_input_requirements.required_fields: signals` or
`required_signal_fields` declarations for compatibility; on the v2 path those
requirements are checked against eligible signal observations, not against a
duplicated `normalized_prospect.signals[]` array. If no eligible observations
exist, the requirements still fail closed. Scalar/v1 and explicitly supported
legacy paths continue to evaluate the prospect `signals[]` representation.

Every accepted observation remains separately inspectable and deterministically
ordered. Equal meaningful typed values may form one logical signal for
cardinality while retaining all observation receipts. Differing supported
values remain a conflict. The v2 conflict algebra is deliberately conservative:

- `require-agreement` routes disagreement to `human-review` and no-draft;
- `any-disqualifies` may resolve a conflict only by deterministically
  disqualifying when an eligible observation has the declared disqualifier
  role.

There is no newest, highest-confidence, last-write-wins, or other positive
winner-selection policy.

## Bind An External Source

An integration can map its own fields to the compiled requirements without
moving collection into MDP. First compile the job, then create the selected
integration-owned `mdp.source-binding.v1` or `mdp.source-binding.v2` document
outside the pack:

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

For v2, the binding also maps every compiled projection and records the adapter
profile/version, logical source identity, source class, transformation
identity, acquisition mode, and opaque or sanitized upstream references. Its
exact hash is repeated through the request, collected results, normalized
envelope, and post-validation receipt. The host must preserve those bytes.
MDP validates internal joins across contract, projection, contributor,
attempt, value, source class, locator, time, confidence, freshness, request,
results, prompt, and binding identities.

`lineage-validated` has a deliberately narrow meaning: the host-submitted
artifacts are internally consistent with the compiled policy. Confidence
measures anchoring strength, not truth probability, and hashes prove artifact
identity and linkage only. Without a separately trusted attestation mechanism,
MDP does not establish host authenticity, signer identity, authorization,
non-repudiation, or source truth.

The fixed value-to-attempt-status translation is:

| Integration observation | MDP attempt status |
|---|---|
| missing, null, empty, or whitespace-only | `not_found` |
| false or zero | `observed` |
| explicitly inapplicable | `not_applicable` |
| inaccessible or unmapped | `blocked` |
| runtime failure | `error` |

Both digests are deterministic. The pack digest covers authored regular files
under `.mdp` using portable relative paths and raw file bytes. Generated local
artifacts under `.mdp/briefs/` and `.mdp/traces/` are excluded so producing a
brief or trace does not stale an otherwise compatible source binding. The
requirements digest covers canonical JSON before its own digest field is
added. Symlinks fail closed.

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

`mdp.normalized-decision-input.v1` preserves scalar receipts beside the
provider-neutral `normalized_prospect`. Signal-aware jobs instead use
`mdp.normalized-decision-input.v2`, which also preserves repeated structured
observations and the source-binding hash. Each envelope records the normalization prompt
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
and the pack's declared output-path projections. This is the signal-aware v2
form; scalar v1 omits `--source-binding` and uses only v1 artifacts:

```bash
mdp --json validate-prompt-output --strict \
  --dir PACK_ROOT \
  --prompt prompts/normalize-prospect.yaml \
  --source-binding SOURCE_BINDING.json \
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

For signal-aware fit or brief, do not extract or edit `normalized_prospect`.
Pass the validated envelope and exact lineage artifacts again so MDP can issue
and consume the immutable projection receipt:

```bash
mdp --json fit --dir PACK_ROOT --job JOB_ID \
  --normalized-input NORMALIZED_INPUT.json \
  --prompt prompts/normalize-prospect.yaml \
  --source-binding SOURCE_BINDING.json \
  --source-attempt-request SOURCE_ATTEMPT_REQUEST.json \
  --collected-attempt-results COLLECTED_ATTEMPT_RESULTS.json

mdp --json brief --context --dir PACK_ROOT --job JOB_ID --channel linkedin \
  --normalized-input NORMALIZED_INPUT.json \
  --prompt prompts/normalize-prospect.yaml \
  --source-binding SOURCE_BINDING.json \
  --source-attempt-request SOURCE_ATTEMPT_REQUEST.json \
  --collected-attempt-results COLLECTED_ATTEMPT_RESULTS.json
```

The detached `--prospect` form is compatible only when the selected job has no
direct or transitive Decision Input Contract binding. Select `--job`
explicitly whenever a pack declares multiple jobs. A governed job invoked with
detached input returns `mdp.job-ingress.v1` status `blocked`, diagnostic code
`governed_job_requires_normalized_input`, a non-success exit, and no fit or
draft authority. Its evidence remains `legacy` or `unassessed`; words such as
“strong fit,” “urgent,” or a non-empty source string cannot cross that boundary.
Missing lineage, ineligible observations, and unresolved conflicts remain
bounded diagnostics and no-draft states; agents must explain them rather than
drafting around them.

## Resource And Egress Safety

The compiled v2 schemas and validators enforce engine-owned limits before
untrusted host data can amplify work: input bytes, projections, observations,
contributors, identifiers, locators, strings, and diagnostic counts are all
bounded. Host-originated values cross output surfaces only through a
field-level allowlist with length and character rules. Control characters are
rejected, renderers escape display text, and locators are opaque display values
that MDP never dereferences. Detailed raw provider records stay host-owned and
outside public artifacts.

## Manual Legacy-To-V2 Adoption

Conversion is explicit and manual; MDP does not ship generalized migration or
collection automation.

1. Start from a valid legacy pack and preserve its v1 fixtures as compatibility
   proof.
2. In the canonical `.mdp/manifest.yaml`, add a `signal_projections` list to
   the relevant Decision Input Contract. Give each projection a stable ID,
   profile-owned kind, explicit closed roles, attribute contributors,
   cardinality, conflict policy, and decision effects. Change that contract's
   `normalization.normalized_schema_ref` to
   `mdp.normalized-decision-input.v2`.
3. Run `mdp --json validate --dir PACK_ROOT`, then
   `mdp --json requirements --dir PACK_ROOT --job JOB_ID`. Confirm
   `runtime_contract_version: v2`, inspect every projection, and use the exact
   emitted schemas and version matrix.
4. Have the integration owner create `mdp.source-binding.v2` outside `.mdp`
   and validate it with `validate-source-binding`. Do not add credentials,
   provider execution state, or raw records to the pack.
5. The host instantiates and preserves the exact v2 request, executes every
   compiled attempt, preserves the v2 collected-results ledger, invokes the
   bound normalization prompt, and returns a structured v2 envelope. MDP does
   not perform any of these collection/provider/model actions.
6. Validate the binding/output chain with `validate-prompt-output`, then use
   the `fit` or `brief --normalized-input` command above. Require
   `lineage-validated` contributions for explicit roles; retain
   legacy/unassessed context only as non-authoritative context.
7. Add synthetic fixtures for agreement, conflict, stale/weak evidence,
   forged or missing lineage, and misleading keyword-only legacy signals.
   Stop no-draft for every blocked or human-review result.

The repository's synthetic
[Clay Audiences example](../examples/clay-audiences-self-serve-enterprise-expansion/README.md)
is the validated reference. Its `prospect-fit-or-brief` job, v2 source-binding,
request, collected results, normalized response, and `fit --normalized-input`
command demonstrate the complete chain without live Clay access. “Clay” names
the synthetic adapter example; MDP is not a Clay integration, enrichment
provider, scraper, sequencer, CRM writer, or outreach tool.

## Deterministic synthetic chain preparation

For a validation-ready public fixture, use the additive offline
`rebind-synthetic-chain` command. It compiles one exact signal-aware v2 job,
creates the four lineage artifacts in dependency order, hashes the final
pretty-JSON-plus-newline bytes at every edge, and runs the existing
`validate-source-binding` and bound `validate-prompt-output` gates before any
destination write.

```bash
mdp --json rebind-synthetic-chain \
  --dir PACK_ROOT \
  --job CANONICAL_JOB_ID \
  --out-dir /tmp/mdp-synthetic-chain \
  --as-of 2026-01-01T00:00:00Z \
  --seed 0 \
  --apply
```

The safe operator path is:

```text
requirements -> rebind-synthetic-chain -> validate-source-binding -> validate-prompt-output -> fit -> brief -> routed-context -> clean-run preparation
```

The command is v2-only and does not collect sources, call providers or
models, normalize external records, edit pack files, or make synthetic
lineage authoritative. With `--input-dir`, all four conventional files must
be present and explicitly synthetic: every `source_class` must be
`synthetic_fixture`, locators must be opaque non-URLs, and
`normalized_prospect.synthetic` plus its synthetic `source_kind` are required.
Ambiguous, real, private, customer, provider, URL, or missing-marker inputs
are refused before destination planning. Dry-run is the default; changed
files require `--apply --force`, which creates a recoverable digest-keyed
backup. Exact replay reports `unchanged` and creates no backup.

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
