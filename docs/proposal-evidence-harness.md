# Deterministic Proposal Evidence Harness

`scripts/mdp-proposal-evidence-harness.mjs` exercises the production CLI
validation and receipt contracts with synthetic, local fixtures. It performs no
network or provider calls and does not read API keys.

Run it with:

```bash
make validate-proposal-evidence-harness
```

The harness writes a bounded report under
`/tmp/mdp-proposal-evidence-harness/` and covers:

| Case | Expected result | Boundary proved |
| --- | --- | --- |
| `clean-native-contract` | `audit-grade` / `stateless-api-verified` | A production-shaped, fully aligned synthetic contract chain is accepted. |
| `ambient-contamination` | `blocked` | Prior/ambient context and non-declared inputs invalidate runner assurance. |
| `mock-demo` | `blocked` | Fixture, mock, demo, and synthetic-model markers cannot become audit-grade. |
| `hash-mismatch` | `blocked` | Validation and runner-audit substitution are detected by exact hashes. |
| `prompt-injection` | `blocked` | Injected text without a matching audited snippet fails prompt-output validation. |
| `unsupported-proof` | `blocked` | An unaudited compliance assertion fails source validation and deterministic claim checking. |

## Critical Interpretation

The harness is fixture-only. Its positive case proves **contract acceptance**,
not that a provider call occurred. The fake runner writes a production-shaped
runner audit so the positive branch can be tested, but marks it
`harness_fixture: true` and `provider_call_observed: false`; the surrounding
report and audit notes also state that no provider call occurred. Never present
the positive harness receipt as production invocation evidence, verified runner
support, or client-source approval.

Production audit-grade claims still require machine-observed evidence from the
actual invocation, an operator-approved source-intake ledger, a terminal atomic
run manifest, and the canonical runner support-matrix posture.

The JSON report includes:

- `fixture_only: true`;
- `network_calls: 0`;
- `provider_calls: 0`;
- hashes for the schemas and each case artifact;
- exact validation/receipt decisions and issue codes;
- the explicit contract-only caveat.

All committed fixtures are synthetic/public-safe. Extend this harness rather
than adding customer documents, raw transcripts, private RFPs, or hand-waved
receipt assertions.
