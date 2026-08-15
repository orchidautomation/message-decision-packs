# Authority Conformance

MDP has one decision-authority owner: the Rust CLI. Renderers, summaries, traces, MCP servers, compatibility runners, skills, generated bundles, and installers are projections or transports. They may preserve or reduce authority; they may not increase it.

## Canonical dimensions

- **Authority level:** `unavailable < informational < authoritative`.
- **Disposition:** `undetermined`, `allow`, or `block`. A faithful projection of an authoritative result preserves `allow` or `block`.
- **Terminal:** authority unavailable, diagnostic complete, success, or no-draft. Existing public lifecycle fields remain compatibility metadata.
- **Governed generation:** not applicable, absent, or available. It is available only for an authoritative allow after every required machine gate passes.
- **Gate obligations:** each authority-bearing operation owns a closed obligation profile. Missing, malformed, unknown, unsupported, or unverifiable required authority is unavailable. An evaluated denial is an authoritative block.
- **Reasons:** source gate reasons remain canonical. Projection and transport diagnostics are namespaced additions and cannot replace source reasons.

## Stable failure semantics

A blocked or unavailable result cannot be overridden in place. New evidence requires a new CLI evaluation. A well-formed CLI decision envelope is data even when the CLI uses a nonzero decision-oriented exit. MCP `isError` is reserved for MCP-owned spawn, timeout, overflow, malformed-envelope, unsupported-contract, or argument failure.

A successful renderer remains informational. Renderer failure makes the projection unavailable; it does not create a replacement decision. Proposal `completed` and native-driver completion are lifecycle metadata and never grant governed success.

## Contributor contract

Every shipped path that receives, transforms, renders, transports, persists, or derives behavior from authority belongs in the supported-surface registry and the packaged corpus at `plugin/assets/authority-conformance/corpus.json`. The mirror under `assets/authority-conformance/` must remain byte-identical.

Run the focused gate with:

```bash
make validate-authority-conformance
```

The focused property suite uses 256 cases and transformation sequences of at most 64 edges. The mutation gate is limited to the authority kernel:

```bash
make validate-authority-mutations
```

The mutation job pins `cargo-mutants` 27.1.0, permits at most 24 candidates, uses two workers and a 40-second per-mutant timeout, and has a 12-minute CI job limit.

## Review and release proof

The MDP-210 PR requires a manual Cubic Ultrareview because `cubic.yaml` is read from the default branch and cannot govern the PR that first adds it. Trigger it with `@cubic-dev-ai ultrareview: focus on authority monotonicity and fail-closed gate completeness`.

When release is separately authorized, release CI records the staged CLI checksum, publishes the release, installs `--agents` into an isolated home, verifies all four generated plugin inventories and tree digests, and runs representative authority cases only through installed CLI/plugin paths. Source-only success cannot satisfy release completion.
