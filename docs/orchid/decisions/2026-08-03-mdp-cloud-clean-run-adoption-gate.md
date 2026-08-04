---
title: MDP Cloud Clean-Run Adoption Gate
date: 2026-08-03
status: accepted
decision: do-not-generalize-yet
linear: MDP-187
---

# MDP Cloud Clean-Run Adoption Gate

## Decision

MDP Cloud must not become a generalized production execution API in the first
clean-run release. The public Rust runtime and its v1 contracts are the
authority. The current Cloud gateway remains a bounded, single-tenant synthetic
demonstration and evaluation surface.

Cloud may later implement the same contracts as an adapter after this gate is
re-run against released local artifacts and real pilot evidence. It must not
fork hashing, assurance, terminal states, replay semantics, or profile policy.

## What Exists Today

The sibling Cloud repository already contains useful implementation evidence:

- an allowlisted synthetic `POST /v1/evaluations` route and output-check route;
- a Decision Lab whose browser has no decision authority;
- child-process lifecycle isolation, bounded timeouts, and sanitized telemetry;
- content-addressed synthetic decisions and HMAC receipts;
- runtime-epoch handling and in-process replay protection;
- a complete synthetic Clay reference integration and deterministic fixtures.

Those controls demonstrate a narrow product and deployment shape. They do not
establish arbitrary-input tenancy, production data handling, customer identity,
durable replay protection, generalized pack release resolution, provider
execution isolation, or v1 receipt conformance.

## Contract Mapping

| Cloud evidence | V1 disposition |
| --- | --- |
| Synthetic release manifests and request allowlist | Useful conformance fixtures; not a general run-request intake. |
| Evaluation child process | Candidate profile/host adapter; process creation alone is not declared-input isolation. |
| Deterministic Clay decision bundle | Candidate GTM profile mapping after it reproduces the public Rust decision and reason-code hashes. |
| HMAC decision receipt | Legacy authenticity evidence only; it cannot replace `mdp.run-receipt.v1` or prove truthful execution. |
| In-process idempotency/replay cache | Demo safety control; not durable host consumption authority. |
| Runtime epoch | Useful deployment-freshness input; not a substitute for an atomic durable replay ledger. |
| Decision Lab | Demonstration/presentation adapter; never decision authority. |

## Adoption Preconditions

All conditions are required before generalized hosted design or implementation:

1. A released and installed public CLI emits and verifies v1 receipts for both
   proposal and deterministic GTM.
2. The cross-profile adversarial conformance matrix passes with no unexplained
   Cloud-specific exception.
3. Cloud consumes the public schemas and golden fixtures without copying or
   weakening policy.
4. A bounded, synthetic Cloud adapter produces the same deterministic decision,
   reason codes, artifact hashes, no-draft states, and assurance limitations as
   the released local runtime.
5. A separate human-approved hosted threat model covers authentication,
   authorization, tenancy, retention/deletion, residency, secrets, abuse/rate
   limits, durable idempotency and replay, signing-key lifecycle, incident
   response, observability redaction, deployment provenance, rollback, and SLOs.
6. At least one customer-controlled pilot proves the contract and operating
   model without requiring MDP Cloud to own collection, enrichment, batching,
   drafting, outbound, CRM mutation, or proposal submission.
7. Product approval defines pricing, support, data-processing terms, kill
   switches, and the exact production claim language.

## Required Separation of Ownership

MDP Cloud may own hosted execution of the released contract, tenant policy
enforcement, hosted receipts, and hosted operational controls. Customers and
integration hosts continue to own source access, collection, credentials for
their systems, row/job orchestration, retries, downstream actions, and approval
to use their data. Model-funded normalization and generation remain a declared
driver/host responsibility unless a later separately approved product scope
changes that boundary.

## Rejection Conditions

The adoption gate fails if any proposal relies on an unqualified `audit-grade`
claim, treats a signature as execution proof, accepts arbitrary customer data
through the current synthetic route, retains in-memory replay as production
freshness authority, lets the browser or Clay template become decision
authority, or creates Cloud-only receipt/assurance rules.

## Next Review

Re-run this gate only after the local release, installed smoke tests, host
conformance kit, and bounded pilot proof exist. Until then, public language must
say “bounded synthetic gateway” or “Decision Lab,” never “generalized MDP
production API.”
