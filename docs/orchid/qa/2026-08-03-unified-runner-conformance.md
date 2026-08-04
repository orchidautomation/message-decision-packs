---
title: Unified Runner Offline Conformance Evidence
date: 2026-08-03
status: passing-offline
linear: MDP-183
commit: pending
---

# Unified Runner Offline Conformance Evidence

## Result

The shared local v1 runtime passed its offline cross-profile authority and
adversarial suite before release. This evidence covers deterministic proposal
output validation and GTM qualification. It does not prove a real provider
call, customer sandbox, hosted tenancy, or production replay durability.

Commands run from the source worktree:

```bash
cargo test --manifest-path cli/Cargo.toml
node scripts/test-run-v1-golden.mjs
node scripts/test-run-conformance.mjs
node scripts/test-run-mcp-server.mjs
```

Observed focused results at the time this record was created:

- Rust runtime and contract tests: 344 of 344 passing, including proposal success,
  proposal no-draft, deterministic GTM qualification, pack mutation, input
  mutation, symlink refusal, hard-link refusal, and self-verification before
  publication.
- Language-neutral canonical JSON vectors: 8 accepted and 14 rejected.
- Cross-profile adversarial conformance: 20 of 20 passing: 14 proposal
  direct-CLI cases, 4 GTM direct-CLI cases, and real proposal and GTM
  unified-MCP-to-real-CLI cases. GTM covers both qualified and disqualified
  complete synthetic branches.
- Stdio MCP and proposal runtime behavior: 21 of 21 passing, including
  timeout, output overflow, contradictory exit status, descendant escalation,
  canonical no-draft, read-only verification, strict recovery-claim validation,
  and cleanup after interrupting the real CLI during private staging.
- Installed-artifact smoke: proposal and GTM `mdp run`, `verify-run`, and a
  real installed `mdp_run` MCP invocation passed without source-tree runtime
  dependencies.

The final PR and release closeout must replace `commit: pending` with the merged
commit or link this record to the immutable PR/release evidence.

## Attack Matrix

| Attack or false claim | Result | Evidence class | Remaining limitation |
| --- | --- | --- | --- |
| Same-conversation or ambient authority added to the request | Rejected as unknown fields; adjacent file/environment sentinels absent from published authority | MDP-observed parser and artifact evidence | Sentinel silence does not prove OS containment for an external model driver |
| Symlink, hard link, path escape, special or reused output path | Refused before stable publication | MDP-observed local staging policy and verifier evidence | Descriptor-relative no-follow reads across every platform remain a later hardening boundary |
| Pack or declared-input mutation during execution | `no-draft:audit-incomplete`; no output or decision authority | Verifier-recomputed pre/post snapshots | Same-user transient replacement outside the observed windows is not claimed impossible |
| Malformed, duplicate-member, oversized or ambiguous authority JSON | Sanitized `no-draft:preflight-refused` | MDP-observed bounded parser behavior | No receipt exists when preflight cannot form an immutable bundle |
| Invalid deterministic output | Verifiable `no-draft:output-invalid`; no output, decision, or compiled-context authority | MDP-observed validation and verifier evidence | Upstream authorship and source truth remain separate |
| Artifact, decision, audit or receipt mutation | Independent verification fails | Verifier-recomputed | Local receipt integrity is not signer identity or non-repudiation |
| Driver asserts `enforced` or `verified` | Verifier rejects driver-attested elevation | Verifier-recomputed assurance rule | An independent host observer may supply stronger evidence in a future adapter |
| Exact replay, duplicate, cross-job, prior-version mismatch | Distinct local ledger outcomes | MDP local reference implementation | Filesystem rollback, clone and snapshot restore require host-owned monotonic storage |
| MCP transport is presented as isolation | Adapter reports no MCP assurance and returns CLI authority unchanged | Transport behavior test | Host-selected executable identity is not authenticated by MCP |
| No-draft CLI exits nonzero | MCP preserves canonical no-draft authority instead of replacing it with a transport error | Behavioral parity test | Timeout before canonical output remains a sanitized MCP transport failure |
| Forced termination leaves staged private bytes | Adapter closes the process group, validates the exact bounded recovery claim and owned transaction, then removes only those paths | Real CLI staging-interruption test plus malformed and hard-linked claim tests | Same-user replacement races cannot be eliminated without lower-level directory-handle cleanup primitives |

## Cross-Profile Authority

Proposal and GTM use the same Rust functions for portable pack snapshots,
declared-input staging, policy hashing, assurance construction, terminal-state
mapping, receipt sealing, artifact publication, and `verify-run`. Profile logic
is limited to deterministic validation, decision construction, and compiled
context. GTM does not own a second runner or receipt implementation.

The proposal JavaScript runner remains a compatibility input compiler and
legacy v0 surface. Its v1 route invokes `mdp run` and labels its own projected
result advisory; only the CLI authority block and v1 run artifacts are
authoritative. The profile-neutral MCP server is likewise transport only.

## No-Draft Publication Contract

Every post-bundle failure publishes a receipt and audit only after internal
verification. A no-draft receipt contains no output, deterministic decision,
or compiled-context authority. A failure before a bundle can be formed returns
a sanitized preflight refusal with no hashes, run directory, or receipt and is
explicitly labeled non-verifiable.

## Deferred Proof

MDP-184 remains the separately human-gated installed-release proof for real
native/BYOK and customer-controlled generative calls. It must request explicit
action-time approval, use synthetic inputs, record exact model-visible request
hashes without credentials, preserve unknown provider properties, and keep raw
proof state in a private temporary directory with cleanup evidence.

MDP Cloud remains a bounded synthetic gateway. Passing this local suite does
not satisfy hosted authentication, tenancy, retention, signing, durable replay,
reliability, incident response, or production-readiness gates.
