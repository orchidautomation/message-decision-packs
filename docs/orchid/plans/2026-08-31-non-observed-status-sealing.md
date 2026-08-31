# MDP-307 — Preserve non-observed statuses in v3 host sealing

Status: `READY_TO_PIN`

## Context and current behavior

The released v0.1.103 native v3 normalization path can produce a valid
attempted-complete collection ledger and still terminate as
`no-draft:output-invalid` with
`decision-input-collected-attempt-results-mismatch`.

`cli/src/run_runtime.rs::host_wrap_v3_normalization_output` currently derives
`observed_attempts` by filtering `attempt_results` to `status == "observed"`.
That filtered set is correctly used to constrain classification evidence and
signal/value projection, but it is also incorrectly used as the source for the
host-owned sealed `attributes` map. A compiled attribute whose authoritative
ledger entry is `not_found`, errored, blocked, or unavailable is consequently
omitted from the sealed envelope.

`cli/src/commands/requirements.rs::validate_collected_attempt_results` then
iterates every compiled attribute and requires the sealed attribute's
`status`, `provenance`, `confidence`, `freshness`, and `error` fields to equal
the authoritative `collected-attempt-results.attributes` entry. The absent
sealed entry fails that deterministic comparison. This is a sealer/validator
contract inconsistency, not a provider-output or receipt-integrity failure.

The existing v3 host-wrap fixture in `cli/src/run_runtime.rs` includes one
observed collected attempt and one model-classified attribute. It does not
include an authoritative `attributes` map with a legitimate non-observed
entry, so the regression is not covered.

## Objective, scope, and assumptions

### Objective

Seal the authoritative status metadata for every compiled collected
non-model-classified attribute while keeping evidence eligibility and neutral
value projection restricted to observed attempts.

### Scope

- Update `host_wrap_v3_normalization_output` to read the authoritative
  `collected-attempt-results.attributes` object and copy the matching entry for
  every compiled non-model-classified attribute into the sealed `attributes`
  map, regardless of status.
- Retain the current observed-only set for classification-evidence validation,
  signal observations, and projection into `normalized_input.fields` or
  `normalized_input.attributes`.
- Extend the local v3 staged-input fixture and host-wrap regression tests with
  one observed attribute and one `not_found` attribute.
- Bump the patch release from 0.1.103 to 0.1.104 in the authored version
  surfaces required by this repository: `cli/Cargo.toml`, `cli/Cargo.lock`, and
  `pluxx.config.ts`.

### Out of scope

- Taxonomy, provider adapter, collection, CRM, sequencing, or outreach changes.
- Changes to deterministic validation semantics or accepted collection
  statuses.
- Downstream real-prospect data or rerunning the downstream proof before the
  corrected release is installed.
- Hand-built compatibility envelopes, detached prospect fallbacks, or any
  weakening of lineage checks.

### Assumptions

- `collected-attempt-results.attributes` is the authoritative aggregated
  status surface validated by the compiled v2 collection schema.
- `attempt_results` remains the authoritative evidence-attempt surface used to
  prove observed contributor eligibility.
- Release CI continues to own packaging and published-installer smoke testing.

## Acceptance mapping

| Acceptance criterion | Implementation | Validation |
|---|---|---|
| Valid `not_found` attributes no longer cause the mismatch | Copy each compiled collected attribute from the ledger `attributes` object into the sealed map | Host-wrap regression asserts the `not_found` entry is present and exact |
| Status metadata is preserved exactly | Clone the authoritative ledger attribute object without reconstructing its fields | Assert equality for status, provenance, confidence, freshness, and error-bearing fixture shape |
| Only observed values enter neutral projections | Keep `observed_attempts` lookup for `projected_value` and signals | Assert the `not_found` attribute is absent from `normalized_input` value projections |
| Non-observed evidence cannot classify | Keep `validate_v3_classification_evidence` bound to `observed_attempts` | Existing ineligible-evidence tests remain green; regression does not add the non-observed attempt to `derived_from` |
| No non-observed value is fabricated | Do not synthesize `value`; clone only the ledger status entry | Regression fixture has no value for `not_found` and sealed output adds none |
| Tampering still fails closed | Do not change `validate_collected_attempt_results` or sealed-envelope validation | Existing requirements/v3 adversarial tests remain green |
| v2/v3 compatibility remains green | Limit production change to host assembly of v3 collected attributes | Run the CLI test suite and formatting checks |
| Corrected version can be released | Apply the patch version bump in the three authored version surfaces | Version-consistency checks and release CI after merge |

## Affected files and symbols

- `cli/src/run_runtime.rs`
  - `host_wrap_v3_normalization_output`: separate authoritative status sealing
    from observed-only projection.
  - `materialize_v3_staged_inputs`: make the fixture reflect the real v2
    collected-results shape with both `attributes` and `attempt_results`.
  - `v3_wrap_seals_a_semantic_payload_with_host_owned_fields` or one adjacent
    focused regression: assert non-observed status retention and exclusion from
    neutral value projection.
- `cli/Cargo.toml`: patch crate version.
- `cli/Cargo.lock`: synchronized package version.
- `pluxx.config.ts`: synchronized plugin/package version authority.

No generated host bundles are authored here; Pluxx owns them.

## Ordered implementation

1. Parse `collected_data["attributes"]` as an object and fail with the existing
   bounded collected-results-invalid diagnostic if the required authoritative
   shape is absent.
2. While iterating compiled attributes, clone the matching ledger attribute
   entry into sealed `attributes` for every non-model-classified attribute.
   Keep model-classified entries owned by `classifications` and do not copy
   them from collected status metadata.
3. Continue resolving `projected_value` exclusively from a matching observed
   attempt. This leaves neutral fields, neutral attributes, signals, and
   classification contributor evidence unchanged.
4. Expand the staged fixture with an aggregated observed entry and an
   aggregated `not_found` entry plus its corresponding attempted result. Add
   assertions proving both sealed status retention and observed-only value
   projection.
5. Bump the authored version surfaces to 0.1.104.
6. Run focused formatting and host-wrap tests, then the complete CLI test suite
   once on the final tree. Push one issue branch and open one PR; do not merge.

## Tests and validation

Focused checks:

```bash
cargo fmt --manifest-path cli/Cargo.toml --check
cargo test --manifest-path cli/Cargo.toml v3_wrap_seals_a_semantic_payload_with_host_owned_fields
cargo test --manifest-path cli/Cargo.toml v3_wrap_rejects_classification_from_non_contributor_evidence
```

Final repository checks:

```bash
git diff --check
cargo fmt --manifest-path cli/Cargo.toml --check
cargo test --manifest-path cli/Cargo.toml
```

Release CI must later provide the v0.1.104 packaging and published-installer
smoke evidence. The downstream pilot resumes only after that installed version
is verified and uses a freshly prepared run.

## Compatibility, rollout, and rollback

The wire contract does not change. The patch makes the host producer conform
to the validator's existing requirement that every compiled collected
attribute retain its authoritative status metadata. Observed-only
classification and projection behavior remains unchanged, and v0/v1/v2
compatibility readers are untouched.

Rollout is a patch release followed by installed-artifact verification. The
downstream pilot must prepare a fresh run rather than reuse the rejected v0.1.103
output. Rollback is a single commit revert; no migration, external mutation, or
data repair is required.

## Risks and safety boundaries

- Copying raw attempt rows instead of aggregated `attributes` entries would
  preserve the shape mismatch. The production code must use the ledger
  `attributes` object for the sealed status map.
- Using all attempts for value or classification projection would weaken the
  evidence contract. Those paths must remain observed-only.
- A valid receipt proves integrity, not semantic acceptance. No code or report
  may upgrade an `output-invalid` terminal state.
- Fixtures must remain synthetic and contain no real prospect evidence.

## Blockers and readiness verdict

No implementation blocker remains. Repository authority, issue scope,
affected symbols, test seams, version surfaces, rollout, and rollback are
resolved.

Readiness: `READY_TO_PIN`.
