# Pluxx Distribution Evaluation

This document records MDP-218's evidence-based decision on whether MDP should
**keep**, **further narrow**, or **switch away from** its Pluxx distribution
model.

## Decision — further narrow, do not switch

Keep the one-source, five-target compiler and the strict skills-only portable
floor. Switching would recreate the release safety, ownership, manifest,
parity, and verification logic that Pluxx already centralizes. Narrow support
claims and defaults instead:

- maintain only one authored `plugin/skills/` tree and one `pluxx.config.ts`;
- keep Codex native-only unless first-party documentation establishes a
  generic root Agent Plugins import and real installed behavior proves it;
- keep other native overlays or enhancements only where repeatable real-host
  receipts show distinct value; do not call a native target supported before
  that proof exists;
- do not add beta hosts without an explicit acceptance gate; and
- do not describe archive format conformance or fixture placement as client
  discovery.

Real-client discovery and native-overlay evidence continue in PLUXX-289,
PLUXX-309, or MDP-261. They are follow-up proof, not blockers to this strategic
decision.

## Shipped proof

Pluxx `0.1.42` and MDP `0.1.101` are public. MDP `0.1.101` was released from
`9168539388555394e049a1202933032701104db2` by
[run 33412887825](https://github.com/orchidautomation/message-decision-packs/actions/runs/33412887825).
Its public release-manifest SHA-256 is
`52319eb97b9a2503b5116bd4b0791d8353f81596b6d10217bb811697181dcfc8`.
Checksums validate all five distributions, the public installer matches its
release asset, and an isolated installation reports `mdp 0.1.101`.

The Agent Plugins archive is a deliberately narrow floor:

- 77,084 compressed bytes, 231,142 raw bytes, and 57 files;
- exactly five immediate-child skills;
- `mcp_servers=[]`, with no hooks, scripts, assets, or native manifests; and
- archive-tree SHA-256
  `b374aa7412b80ea5325b0f6144365269e62130c5603169b6aa132747d9fda964`.

This proves public package shape and safe isolated placement. Release smoke
used compatible-client fixtures for portable Cursor- and Codex-labelled
destinations; it did not observe either client discover the portable package.
The actual Codex CLI proof applies to native registration only.

## Measured overhead and value

| Measure | Observation | Interpretation |
| --- | --- | --- |
| Portable size | 77,084 compressed bytes, 1.34% of the current native compressed total | The portable floor itself is small |
| Versioned archive growth | Four v0.1.95 native archives: 5,652,763 bytes; v0.1.101 portable plus four native archives: 5,819,853 bytes, +2.96% | Portable adoption did not create large archive overhead |
| Native-only growth | +1.59% compressed from v0.1.95 to v0.1.101 | Most release growth is not the portable floor |
| Native duplication | 99.228% of aggregate raw native bytes are common/repeated, versus 99.209% in v0.1.95 | Portable output did not reduce native archive duplication |
| Adoption delta | PR #255: +775/-28 across 14 files; installer 170→437 lines; release finalizer 97→171 lines | Most added work is installer, manifest, validation, safety, and proof logic |
| Release duration | v0.1.95: 8m53s; v0.1.101: 6m55s, about 22% faster | Added proof did not slow this release run |
| Activation overhead | p50 21.182ms, p95 21.947ms, n=50; repeated unchanged compact body stayed silent | Current activation path remains compact and idempotent |

The portable floor adds little artifact weight, but it does not remove the
native duplication or the need for host-specific proof. Pluxx still earns its
place by preventing each target from independently reimplementing the safety
and parity layer. The correct response is narrower claims, not a compiler
switch.

Reconsider switching only if real portable consumers supply nearly all
consumed value and measured Pluxx maintenance exceeds the cost of direct
packaging and equivalent safety/parity proof.

## Capability policy

| Consumer/path | Current policy | Evidence boundary |
| --- | --- | --- |
| Agent Plugins v1 package | Keep the exact five-skill portable floor | Public archive, manifest, checksum, and isolated install proof |
| Cursor portable import | Candidate only; do not claim support yet | Documented local path exists, but real Cursor discovery was not observed |
| Codex portable import | Do not claim; keep Codex native-only | No documented generic root import path and no real portable discovery |
| Other conformant clients | No support promise without an acceptance gate | Format conformance is not installed behavior |
| Claude Code native | Keep only while repeatable host evidence shows overlay value | Native proof remains separate from portable proof |
| Cursor native | Keep only while repeatable host evidence shows overlay value | Real-host evidence remains required |
| Codex native | Keep | Native CLI registration and installed bundle proof exist |
| OpenCode native | Keep only while repeatable host evidence shows overlay value | Explicit degradation remains part of the claim |

Portable MCP and portable hooks remain out of scope. Add either only through a
separately reviewed first-party extension contract and installed proof.

## Decision record

| Field | Recorded value |
| --- | --- |
| Public Pluxx package | `@orchid-labs/pluxx@0.1.42`; integrity `sha512-Mw63WOao0GXFVcqNw3w4Axs1+5nQhb+wtNWJWwOy8SYwuKvlF3r4G+NSjgGd+ZEoqfS1V1gKm3nXsNPjbOtKaw==`; release commit `0f6621a39c02aa69ad3363ad22ada429175779b7` |
| Public MDP release | `v0.1.101`; source `9168539388555394e049a1202933032701104db2`; release-manifest SHA-256 `52319eb97b9a2503b5116bd4b0791d8353f81596b6d10217bb811697181dcfc8` |
| Portable consumers | Public artifact and fixture install proven; no real Cursor or Codex portable discovery claimed |
| Native consumers | Four public native archives preserved; Codex native registration proven; other real-host tiers remain follow-up evidence |
| Overhead | Small portable archive; +2.96% total compressed archive growth; high native duplication; substantial one-time safety/proof code; no observed activation or release regression |
| Decision | `further narrow` — keep Pluxx; do not switch |
| Rationale | Pluxx centralizes valuable safety and parity logic, while unsupported portable-host and native-overlay claims should be narrowed until real-host evidence exists |
