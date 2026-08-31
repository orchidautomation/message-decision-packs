# Pluxx Distribution Evaluation

This document is the evidence worksheet for deciding whether MDP should
**keep**, **further narrow**, or **switch away from** its current Pluxx
distribution model. It does not record a final verdict yet.

## Current proof state

Continue the measured Pluxx evaluation without selecting **keep**, **further
narrow**, or **switch** yet. MDP remains authored once, while Pluxx emits:

1. a narrow Agent Plugins v1.0.0 portable skills package for conformant
   clients; and
2. native Claude Code, Cursor, Codex, and OpenCode bundles where proven hooks,
   scripts, assets, commands, MCP wiring, installation, or verification add
   value.

Pluxx `0.1.42` and MDP `0.1.101` are public. MDP `0.1.101` was released from
`9168539388555394e049a1202933032701104db2` by
[run 33412887825](https://github.com/orchidautomation/message-decision-packs/actions/runs/33412887825).
Its checksums and release manifest validate all five distributions, the public
installer matches its release asset, and an isolated installation reports
`mdp 0.1.101`. Those facts prove the released artifacts and safe isolated
placement, not real-client discovery.

## Observed release evidence

| Question | Current observation | Evidence tier | Remaining gap |
| --- | --- | --- | --- |
| Is there one maintained source? | Yes. MDP authors skills under `plugin/skills/` and distribution intent in `pluxx.config.ts`; generated manifests are outputs. | Exact public source commit | Measure recurring maintenance cost |
| Is the portable floor narrow? | Yes. The public archive has exactly five immediate-child skills; no portable MCP, hooks, scripts, assets, commands, agents, or native manifests. | Public archive, manifest, and checksum | Real-client discovery |
| Are native outputs preserved? | Yes. The public release contains distinct Claude Code, Cursor, Codex, and OpenCode archives, each bound by the release checksum set and manifest. | Public release artifacts | Real-host behavior at each claimed tier |
| Is portable installation safe? | Yes at the isolated tier. The released installer requires an explicit absolute destination, rejects native overlaps and unknown ownership, updates only a recognized portable tree, and passed isolated installation. | Public installer and isolated install proof | Repeat on each selected real client |
| Does Cursor discover the portable package? | Not proven here. Cursor documents a local plugin path, but no real Cursor binary was available. | Documentation plus unobserved host | Run clean real-host discovery |
| Does Codex discover a root Agent Plugins package? | Not proven. No documented root local-import path was established, so MDP does not guess one. | Explicit gap | Obtain first-party path and clean proof, or keep native only |
| Does portable MCP ship? | No. MDP has not declared a portable `mcp.json`. | Artifact contract | Revisit only through a separately reviewed declaration |
| Do portable hooks ship? | No. Hook behavior remains native and host-specific. | Artifact contract | Do not claim without a documented extension and installed proof |

## Evidence still required for the decision

Before the final decision:

- clean-install native Claude Code, Cursor, Codex, and OpenCode outputs at the
  proof tier each public claim requires;
- prove all five portable skills are discovered in a real Cursor consumer;
- prove Codex portable discovery only if a current first-party local-import
  contract exists; otherwise record Codex as native-only;
- measure unchanged activation overhead and confirm native hook value remains
  compact and idempotent; and
- record recurring maintenance work separately for portable core, native
  overlays, installers, verification, and beta-host churn.

## Decision rubric

### Keep

Choose **keep** when the portable core removes redundant packaging work and
Pluxx's remaining native compilation, install, and verification paths prevent
more host-specific work than they create.

### Further narrow

Choose **further narrow** when Pluxx remains useful, but one or more native
targets or enhancements lack consumed value or repeatable installed proof.
Possible narrowing must be target- and capability-specific; format conformance
alone is not a support promise.

### Switch

Choose **switch** only when public installed evidence shows the portable core
supplies nearly all consumed value and Pluxx's remaining native/install/
verification layer causes more recurring work than it prevents. Archive
duplication alone is not enough.

## Closeout record

Release facts are complete; the measured decision fields intentionally remain
open until MDP-218 records its verdict:

| Field | Required value |
| --- | --- |
| Public Pluxx package | `@orchid-labs/pluxx@0.1.42`; integrity `sha512-Mw63WOao0GXFVcqNw3w4Axs1+5nQhb+wtNWJWwOy8SYwuKvlF3r4G+NSjgGd+ZEoqfS1V1gKm3nXsNPjbOtKaw==`; release commit `0f6621a39c02aa69ad3363ad22ada429175779b7` |
| Public MDP release | `v0.1.101`; source `9168539388555394e049a1202933032701104db2`; release-manifest SHA-256 `52319eb97b9a2503b5116bd4b0791d8353f81596b6d10217bb811697181dcfc8` |
| Portable consumers | Host/version, documented import contract, five-skill discovery result |
| Native consumers | Host/version, installed capability and degradation result |
| Overhead | Install/update effort, hook latency/noise, recurring target work |
| Decision | Pending MDP-218 measurement: `keep`, `further narrow`, or `switch` |
| Rationale | Pending measured user value and maintenance evidence, with explicit gaps |

Until MDP-218 completes the remaining measurements, the state is **released and
under evaluation**, not a final keep/narrow/switch conclusion.
