# Adversarial Run-v1 Conformance Coverage

The executable cases are generated in a private temporary directory by
`scripts/test-run-conformance.mjs`. They are not checked-in receipts and must
not be presented as evidence of a provider call or production containment.

The black-box suite exercises the compiled or installed CLI and requires:

- a successful proposal validation transaction that independently verifies;
- rejection of unknown ambient fields, undeclared-input smuggling, logical
  path escape, symlink inputs, hard-link inputs on Unix, malformed JSON,
  duplicate JSON members, oversized authority JSON, and output-directory
  reuse;
- absence of adjacent-file and inherited-environment sentinels from every
  published authority artifact;
- an explicit `no-draft:output-invalid` receipt with no output, decision, or
  compiled-context authority;
- detection of output-artifact and receipt tampering;
- rejection of driver-attested `enforced` or `verified` assurance;
- replay classification for first consumption, permitted exact replay,
  duplicate, cross-job use, and prior-version mismatch;
- fail-closed replay behavior for record corruption, interrupted append, and
  a stale lock; and
- an explicit demonstration that a copied or rolled-back local ledger is not
  distinguishable from the original without a host-owned monotonic anchor.

The runtime's source-mutation check has a deterministic in-process race hook in
the Rust test `source_mutation_forces_audit_incomplete_and_no_output`. A
wall-clock mutation race against a subprocess is intentionally not used as the
conformance oracle because scheduler timing would make it flaky. Run all Rust
tests alongside this suite before release.

This suite proves contract behavior visible at the CLI boundary. It does not
prove that an operating system, coding-agent host, table platform, model
provider, proxy, backup system, or customer scheduler enforced controls that it
did not expose to the CLI.
