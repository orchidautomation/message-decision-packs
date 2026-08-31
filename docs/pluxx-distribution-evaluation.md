# Pluxx Distribution Evaluation

This document is the evidence worksheet for deciding whether MDP should
**keep**, **further narrow**, or **switch away from** its current Pluxx
distribution model. It does not record a final verdict yet.

## Current decision state

Keep Pluxx through the current proof cycle. MDP remains authored once, while
Pluxx emits:

1. a narrow Agent Plugins v1.0.0 portable skills package for conformant
   clients; and
2. native Claude Code, Cursor, Codex, and OpenCode bundles where proven hooks,
   scripts, assets, commands, MCP wiring, installation, or verification add
   value.

The prepared source targets Pluxx `0.1.42` and MDP `0.1.96`. Neither prepared
version is treated here as public or installed evidence. A local build or fake
home proves package shape and safety, not consumer discovery.

## Observed preparation evidence

| Question | Current observation | Evidence tier | Rebind needed |
| --- | --- | --- | --- |
| Is there one maintained source? | Yes. MDP authors skills under `plugin/skills/` and distribution intent in `pluxx.config.ts`; generated manifests are outputs. | Repository source | Confirm exact public release commit |
| Is the portable floor narrow? | Yes. Exactly five immediate-child skills; no portable MCP, hooks, scripts, assets, commands, agents, or native manifests. | Local artifact contract | Verify public archive and release manifest |
| Are native outputs preserved? | Yes in local build and regression proof for Claude Code, Cursor, Codex, and OpenCode. | Local generated/fixture proof | Clean-install each claimed public artifact |
| Is portable installation safe? | Prepared installer requires an explicit absolute destination, rejects native overlaps and unknown ownership, and updates only a recognized portable tree. | Local installer regression | Rerun from downloaded public installer/assets |
| Does Cursor discover the portable package? | Not proven here. Cursor documents a local plugin path, but no real Cursor binary was available. | Documentation plus unobserved host | Run clean real-host discovery |
| Does Codex discover a root Agent Plugins package? | Not proven. No documented root local-import path was established, so MDP does not guess one. | Explicit gap | Obtain first-party path and clean proof, or keep native only |
| Does portable MCP ship? | No. MDP has not declared a portable `mcp.json`. | Artifact contract | Revisit only through a separately reviewed declaration |
| Do portable hooks ship? | No. Hook behavior remains native and host-specific. | Artifact contract | Do not claim without a documented extension and installed proof |

## Evidence still required

Before the final decision:

- publish and verify the exact public `@orchid-labs/pluxx@0.1.42` npm artifact,
  including registry integrity;
- build and publish MDP `0.1.96` from trusted merged source;
- download and validate the CLI, native archives, portable archive, checksums,
  and release manifest;
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

Complete this table only after the evidence above exists:

| Field | Required value |
| --- | --- |
| Public Pluxx package | Version, registry integrity, source commit |
| Public MDP release | Version, tag, source commit, release-manifest digest |
| Portable consumers | Host/version, documented import contract, five-skill discovery result |
| Native consumers | Host/version, installed capability and degradation result |
| Overhead | Install/update effort, hook latency/noise, recurring target work |
| Decision | `keep`, `further narrow`, or `switch` |
| Rationale | Measured user value and maintenance evidence, with explicit gaps |

Until that table is complete, the state is **continue through proof**, not a
final keep/narrow/switch conclusion.
