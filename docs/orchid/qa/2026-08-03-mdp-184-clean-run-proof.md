---
title: MDP-184 Installed Clean-Run Proof
date: 2026-08-04
status: complete-with-explicit-no-draft-cells
linear: MDP-184
release: v0.1.59
release_commit: 245bc932f4c6786a13b8fe6e29d7d5009bc9106a
---

# MDP-184 Installed Clean-Run Proof

## Decision

The installed v0.1.59 runtime passed the customer-controlled proposal and GTM
cells at the **MDP-observed declared-input isolation** tier. Exact replays
produced the same bundle, decision, compiled-context, and receipt authority
hashes.

The native/BYOK cells did not make provider calls. The proposal request passed
offline request-shape validation, but execution remained policy-blocked because
the required action-time approval for an external, potentially billable call
was not granted. GTM was refused during preflight because the released v1
runtime has no canonical native generative driver. Neither native cell produced
a draft, output authority, runner audit, or receipt.

This is a complete four-cell proof record because every cell either passed or
ended in a precise, documented no-draft state. It is not evidence that MDP has a
verified provider integration or an end-to-end audit-grade generative runner.

## Four-Cell Matrix

| Profile | Boundary | Result | Authority / reason |
| --- | --- | --- | --- |
| Proposal | Native structured-output API / BYOK | `no-draft:policy-blocked` | Offline request shape passed. No provider call was authorized; no output or receipt exists. The remaining real-provider proof is MDP-149. |
| GTM | Native structured-output API / BYOK | `no-draft:preflight-refused` | Released v1 supports deterministic GTM qualification but no canonical native generative driver. No provider call, output, or receipt exists. |
| Proposal | Customer-controlled constrained subprocess | Pass at `observed` isolation tier | Installed v0.1.59 deterministically validated declared proposal output, internally verified the artifact chain, and published a valid receipt. |
| GTM | Customer-controlled constrained subprocess | Pass at `observed` isolation tier | Installed v0.1.59 deterministically qualified declared lead evidence, internally verified the artifact chain, and published a valid receipt. |

The two native dispositions are proof-matrix outcomes, not fabricated runtime
receipts. Pre-bundle and policy-blocked runs cannot honestly publish immutable
run authority.

## Installed Artifact And Environment

- Installed CLI: `mdp 0.1.59`.
- Release commit: `245bc932f4c6786a13b8fe6e29d7d5009bc9106a`.
- Release workflow: 30879133832, including three platform binaries, repository
  validation, bundle diagnostics, release assets, and installed-release smoke.
- Proof inputs: repository-owned synthetic proposal and GTM fixtures only.
- Scratch root: fresh operating-system temporary directory with mode `0700`.
- Runtime environment: the conformance harness supplies an empty environment
  allowlist to the child process; no provider credentials are required or
  forwarded for the customer-controlled deterministic cells.
- Tools and network: no model tools and no provider network call. The native
  proposal cell used `--dry-run`; the GTM native cell stopped before request
  construction.
- Retention: raw proof bundles stayed in private temporary scratch only. This
  document retains sanitized hashes and results; scratch was deleted after
  evidence extraction.

## Customer-Controlled Proposal Evidence

`mdp verify-run` returned `valid: true` with no issues for execution
`conformance-success`.

| Authority | SHA-256 |
| --- | --- |
| Portable pack digest | `a2b2b6bb12f4e3b9294b0b24236f0f28936820478e2fffddc7a51931b44f0dcb` |
| Declared prompt-output input | `dcdb1586f71691bf3450abf186351c8c9cb51d5ffa6f23ca9d76e0da39fd3cab` |
| Bundle authority | `79f96d7cbfa2db489a73088d22c31acb958dc4d1cded728ea40bee6e1f7e1155` |
| Decision (`valid-existing-output`) | `370a831670af41913c665cd1af4b042cd121d025d5e5d9b9ca5fc5f39a0ce576` |
| Compiled context | `6394aa7a22c51175106756e2078883cc9771eaa572e1f2b0bcfd7ba49135decd` |
| Receipt authority | `a105eba56d3a4aeec1f02eedd167ca7cfa853480b0c72549205572fecba37a51` |

The terminal reason was `validation-passed`. Repeating the exact request in a
new output directory reproduced every authority hash in the table.

## Customer-Controlled GTM Evidence

`mdp verify-run` returned `valid: true` with no issues for execution
`gtm-success`.

| Authority | SHA-256 |
| --- | --- |
| Portable pack digest | `a5c71e71e1e58611f4520049ca1f0adbe025e77dcca1735b9deb8b06d38f794e` |
| Normalized decision input | `3e6b9cf681bfc742819b2962552d937939c04c95d92881f4225d16666a8d1c10` |
| Source-attempt request | `b228884027f7754f5136bb9317d164fc8e4d702ea145891997f29c26913fa9df` |
| Collected attempt results | `4181b47b398318883538265ff9a3c0735f66ac401086757b9f5d14dd14ba67c7` |
| Bound prompt | `13a93cf24285f504e0770b57f0f8ba76866b2db3dfe9332fe12ad11b85c59264` |
| Bundle authority | `c5ea2ad94373c18543d9044b8756d02ce0e0684c46d0a7dd523a2603d5fdcb70` |
| Decision (`qualified`) | `fcb87b59bc5444c4fe4b96728873b64052d569daa5109b245a3c4a30278a2b41` |
| Compiled context | `78952ba21eb0bf2135135bd746839d12bac59a858ad8a36914bdf497b6bd8e71` |
| Receipt authority | `0e4e4088753b6ca36589c211df5eb64ffa333e0066e8872018d4a69425275957` |

The terminal reason was `ready`. Repeating the exact request in a new output
directory reproduced every authority hash in the table.

## Runner Assurance And Limits

Both successful runner audits use `mdp.runner-audit.v1` and record:

- declared-input isolation: `observed`, by MDP;
- exact-byte binding: `verified`, by MDP;
- mutation resistance: `verified`, by verifier recomputation;
- audit evidence: `observed`;
- stateless inference: `not-applicable`, because these cells are deterministic
  and perform no model inference.

These receipts prove that the released MDP runtime bound, evaluated, and
verified the declared bytes. They do not attest operating-system access outside
the private staging directory. Process, filesystem, network, credential, and
tenant containment remain host/operator responsibilities. Local hashes prove
artifact integrity, not signer identity, monotonic storage, non-repudiation, or
semantic truth.

## Native Proposal Dry-Run Evidence

The installed proposal runner built one plain user message with no
`instructions`, `previous_response_id`, conversation attachment, or tools. It
targeted the official `/v1/responses` path, set `store: false`, and requested a
strict structured JSON schema.

| Dry-run artifact | SHA-256 |
| --- | --- |
| Native request | `b6081066c29f257f40b253a392c2976c70551ce7016deae2f07d26d145e42d9e` |
| Source intake | `910a501da078be379237027b08e59dbf10cc076d467532590a97974bf1ba48d3c` |
| Source audit | `e7455d5e3782293aaef95aa16caa8410e2025f555a8a4107c9a8aa562d3eb58c` |
| Dry-run record | `eec554a3c2c69e0b7033bb94e5a74caa0072539557fa188a93e280a8408c428f` |
| Exact staged source | `2dfeb76d2529ec2935796cd1934eba841ea328abafe0df64161f220c7933bf84` |

The requested model alias was `gpt-5-mini`, but a dry run cannot verify model
availability, provider resolution, response metadata, storage behavior, or
actual model-visible bytes. The source intake also remains a candidate until a
real operator-approved source ledger is formed. Therefore this evidence does
not upgrade `native-api` from `recipe-only`.

## Repeatable Offline Checks

From a source checkout, with the installed release selected explicitly:

```bash
MDP_BIN=/path/to/installed/mdp node scripts/test-run-conformance.mjs

node /path/to/installed/plugin/scripts/mdp-proposal-runner.mjs run \
  --pack plugin/assets/templates/proposal \
  --workdir /private/temporary/proof \
  --source examples/proposal-flow-video/messy-sources/01-rfp-ocr.txt \
  --source-id synthetic-rfp-summary \
  --source-kind synthetic-example \
  --model <operator-selected-model> \
  --dry-run \
  --mdp-bin /path/to/installed/mdp
```

Do not remove `--dry-run` without explicit action-time authorization for the
provider call, reviewed synthetic inputs, approved credential and endpoint
handling, and an approved retention policy.

## Remaining Work

- MDP-149 remains open for one real, explicitly authorized native proposal
  invocation with synthetic inputs and a sanitized receipt chain.
- A separate follow-up owns a canonical generative driver and native GTM proof.
- MDP Cloud remains a bounded synthetic gateway. This proof does not authorize
  a generalized production execution API or transfer customer-owned sandbox,
  credentials, network policy, retention, or signing responsibilities to MDP.
