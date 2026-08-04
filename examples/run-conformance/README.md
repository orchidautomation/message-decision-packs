# Synthetic MDP v1 Host Conformance Envelopes

These fixtures show the exact public driver and runner-audit contract shapes for one synthetic success and one synthetic no-draft run. Validate them with the schemas exported by the same installed `mdp` release that will execute the run.

They are synthetic examples, not evidence of a provider call, a maintained Clay/Codex/Claude integration, MDP Cloud production readiness, or audit certification. The repeated digest values are explicit fixture authorities; do not reuse them in a real receipt.

## Files

- `run-requests/proposal-validate-existing-output.json`: executable source-tree
  example for deterministic proposal output validation.
- `run-requests/gtm-qualify.json`: executable source-tree example for one
  deterministic GTM qualification job.

- `synthetic-success/driver-request.json`: bounded request for an ephemeral customer worker.
- `synthetic-success/driver-result.json`: success result with output and audit authorities.
- `synthetic-success/runner-audit.json`: host-observed evidence with honest unknowns and limitations.
- `synthetic-no-draft/driver-request.json`: bounded request for one table row.
- `synthetic-no-draft/driver-result.json`: fail-closed result with `output: null`.
- `synthetic-no-draft/runner-audit.json`: incomplete-boundary evidence that cannot authorize a draft.

The example envelope is deliberately limited to the stdin/stdout driver boundary. The CLI remains responsible for constructing `mdp.run-bundle.v1`, deriving the final `mdp.run-receipt.v1`, and returning `mdp.run-verification.v1`.

Run the request examples from the repository root and choose new output
directories each time:

```bash
cargo run --manifest-path cli/Cargo.toml -- --json run \
  --request examples/run-conformance/run-requests/proposal-validate-existing-output.json \
  --out-dir /tmp/mdp-proposal-clean-run

cargo run --manifest-path cli/Cargo.toml -- --json run \
  --request examples/run-conformance/run-requests/gtm-qualify.json \
  --out-dir /tmp/mdp-gtm-clean-run
```

The checked-in release IDs identify source-tree examples, not published pack
releases. A real operator must replace them with the immutable release identity
used by that host and retain the MDP-observed portable digest from the bundle.

## Conformance expectations

For the success fixture, a host must preserve the driver-returned evidence without upgrading `host-attested` or `driver-attested` claims. Final artifact integrity and validation can become verified only after the CLI recomputes them.

For the no-draft fixture, a host must reject any side-channel or partial draft and preserve `output: null`. Changing the state to success, omitting the audit, or adding undeclared fields is a conformance failure.

See [Host Conformance](../../docs/host-conformance.md) for replay semantics, platform boundaries, credentials, retention, retries, and downstream-action ownership.

## Run the offline adversarial suite

Build the CLI from the same source revision, then run the black-box suite:

```bash
cargo build --manifest-path cli/Cargo.toml
node scripts/test-run-conformance.mjs
```

Set `MDP_BIN` to test an installed release instead. The suite creates private
scratch under the operating-system temporary directory, mutates only copies,
and removes the scratch unless `--keep` is passed. It covers malformed and
undeclared authority, filesystem indirection, no-draft publication, tampering,
assurance non-elevation, and local replay behavior. See the
[adversarial coverage notes](adversarial/README.md) for the exact boundary and
known non-guarantees.
