# MDP-179 Implementation Plan Review

Date: 2026-08-03
Artifact: `docs/orchid/plans/2026-08-03-001-feat-unified-clean-context-runtime-plan.md`
Document type: `unified-plan`

## Review Scope

Six review lenses evaluated the implementation-ready plan: coherence, feasibility, security, product fit, scope control, and adversarial breakage. Reviewers inspected the Rust CLI, proposal/native/MCP scripts, GTM Decision Input and deterministic commands, packaging and release tests, and the sibling MDP Cloud repository.

## Decisions Strengthened

- The Rust CLI is the sole canonical authority for snapshots, canonical hashes, assurance, terminal states, and verification.
- The native/BYOK reference HTTP transport is Rust-owned so MDP can observe the exact serialized provider-request body. Provider-to-model transformation remains unknown.
- External headless and customer drivers receive bounded canonical input on stdin and remain attested unless a verifier-configured authenticated observer supplies independent evidence.
- Authority JSON rejects duplicate members, unsafe numeric values, and configured structural size limits before typed deserialization.
- Snapshot staging uses descriptor-relative no-follow reads, hard-link and identity checks, sealed driver identity, post-run rehashing, and explicit assurance caps where OS immutability is not proved.
- Provider endpoint policy covers HTTPS scheme/host/port allowlists, redirects, proxies, userinfo, resolved targets, and secret attachment after request-body hashing.
- Deterministic GTM uses a frozen runtime context and a reviewed legacy characterization corpus before reason-code mapping.
- Replay protection separates pure integrity verification from host-owned atomic compare-and-consume. The local ledger is conformance-only and exposes crash, rollback, replacement, cloning, and durability limits.
- The original conversation receives a CLI-rendered canonical authority block that ambient commentary cannot alter.
- MDP releases after offline conformance instead of waiting on a billable provider proof. The real native/customer proof runs against installed release assets and triggers a patch-and-reproof cycle if it finds a defect.
- Proposal JavaScript remains the compatibility input compiler and readiness presenter until conformance proves a remaining function is authoritative.
- Normative host schemas, fixtures, protocol, replay semantics, and assurance mapping ship with conformance; post-release host work adds extended tutorials.
- GTM returns verified qualification and bounded context. Downstream campaign drafting remains host-owned and may use a separate generic declared run rather than a new MDP campaign product surface.

## Residual Boundaries

- An unsandboxed same-user external driver may read ambient host files. Its filesystem and exact-request dimensions remain attested or unknown.
- Provider aliases, hidden instructions, provider-side transformation, caching, storage, and policy are not MDP-observed unless the provider supplies verifiable metadata.
- Unsigned local receipts prove internal artifact integrity, not signer identity or non-repudiation.
- Release checksums distributed with the installer are not an independent authenticated supply-chain root.
- Production replay durability, scheduling, retries, batching, and incident response remain host-owned.

## Result

The plan is implementation-ready after the applied findings. It preserves the local-first product boundary, supports current plugin and table/job workflows, and does not authorize generalized hosted execution, portable container management, campaign orchestration, or an unqualified audit-grade claim.
