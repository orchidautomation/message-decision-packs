# MDP-281 — Idempotent, compact activation hooks

## Context and current behavior

`pluxx.config.ts` currently invokes `scripts/mdp-activate.sh` from both
`sessionStart` and `beforeSubmitPrompt`. The script resolves a workspace from
host environment variables or JSON hook input, detects `.mdp/manifest.yaml`,
prints the same full boundary/readiness guidance, and runs visible CLI
capabilities/doctor summaries every time. An unchanged prompt therefore repeats
roughly 2.3 KB of context and pays repeated shell/CLI startup cost.

`postToolUse` is already narrowly matched to `Edit|Write|apply_patch` and calls
`scripts/mdp-post-edit-validate.sh`; that constraint must remain. Current source,
generated-bundle, and installed-wrapper coverage lives primarily in
`scripts/test-pluxx-hooks.sh`, `scripts/test-opencode-wrapper.mjs`, and
`scripts/release-install-smoke.sh`.

PLUXX-345 and MDP-211 are complete, so Codex installed hooks now have a valid
plugin-root path. PLUXX-347 also confirms that activation remains a native
enhancement; this issue must not place hooks in an Agent Plugins extension.

## Objective

Deliver one authoritative full MDP activation per reliable host session and
workspace, followed by no output or a deterministic state-change message of at
most 200 characters while the workspace state is unchanged. Refresh once when
relevant pack/profile/workspace state changes. Measure warm unchanged script
overhead and target p50 below 25 ms without weakening visible boundaries,
doctor context, or edit-only validation.

## Scope

- Add explicit activation modes for the session-start and prompt hooks.
- Resolve a stable session/workspace identity from documented host environment
  or hook payload fields; record an explicit degradation when a host provides no
  reliable session identity.
- Persist only minimal, non-secret, session-scoped activation state outside the
  user's pack and installed plugin directories.
- Fingerprint the relevant `.mdp` authority needed to invalidate unchanged
  state without running the full CLI path on every prompt.
- Preserve one full activation payload, warnings, and CLI summaries for a fresh
  or changed state.
- Keep post-edit validation edit-only and quiet for non-edit tools.
- Add source, generated native-bundle, and installed Codex plus one other host
  fixture; measure cold and warm p50/p95 with environment details.

## Out of scope

- No Agent Plugins client-extension hooks.
- No hidden enrichment, scraping, model invocation, sending, CRM mutation,
  automatic brief generation, pack mutation, release, deployment, or install
  into Brandon's active homes.
- No weakening or removal of explicit `mdp validate` and no broad Pluxx hook
  compiler changes unless a proven host payload defect blocks MDP behavior.
- No archive-size optimization.

## Assumptions and decisions

- `sessionStart` requests full activation; `beforeSubmitPrompt` requests compact
  activation. The script remains safe if events arrive out of order: the first
  reliable event for a `(session, workspace)` pair emits the full payload.
- State lives below an isolated cache root such as
  `${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/mdp-activation/`, keyed by a hashed
  session/workspace identity. It must never contain hook payloads, secrets, CLI
  output, absolute pack content, or API-key values and must use restrictive
  permissions plus atomic replacement.
- Relevant change detection uses a deterministic fingerprint of workspace
  identity and pack/profile authority files under `.mdp/`, excluding transient
  ignored output. The implementation must document the exact inventory.
- If no reliable session identity exists, fail safe: retain correct full
  activation behavior and record that host as degraded instead of incorrectly
  suppressing context across sessions.

## Acceptance mapping

| Acceptance criterion | Implementation | Proof |
| --- | --- | --- |
| Fresh session gets required context once | Mode-aware script claims an atomic state record only after deciding to emit full activation | Source and installed fixtures invoke start/prompt sequences and count one full marker |
| Unchanged prompts do not repeat full payload; compact output <=200 chars | Fast fingerprint/state check exits silently or prints one bounded deterministic line | Byte-count and exact-output assertions across repeated prompt calls |
| State changes refresh once | Fingerprint includes declared pack/profile authority and atomically advances after full output | Mutate manifest/profile fixtures; next prompt refreshes once, following prompt is compact |
| Warm unchanged p50 target <25 ms | Fast path avoids CLI execution and scans only bounded metadata | Benchmark harness records iterations, host, shell, filesystem, p50 and p95 |
| Non-edit tools do not validate | Preserve matcher and wrapper tool filtering | Read/bash fixture produces zero validation calls |
| Edit/write/apply-patch still validate | Preserve `postToolUse` command and self-gating script | Existing and installed edit fixtures pass |
| Every native host proves or degrades | Add a host evidence table tied to reliable session identity/payload support | Claude Code, Cursor, Codex, and OpenCode rows have fixture IDs or explicit degradation |
| Installed proof covers Codex plus another host | Rebuild native bundles and exercise isolated installed artifacts | Artifact hash, host version, event sequence, output counts, and timing receipt are recorded |

## Affected files and symbols

- `pluxx.config.ts::hooks`: pass explicit full/compact modes while preserving
  the post-tool matcher and native targets.
- `scripts/mdp-activate.sh`: refactor payload reading, workspace/session
  resolution, fingerprinting, atomic state, fast exit, full rendering, and
  cleanup/error behavior.
- A narrowly scoped helper under `scripts/` only if shell portability and
  deterministic JSON hashing are clearer in Node; do not add a runtime service.
- `scripts/test-pluxx-hooks.sh`: source and generated-bundle sequences,
  byte-counts, invalidation, concurrency, permissions, missing-session
  degradation, and non-secret state assertions.
- `scripts/test-opencode-wrapper.mjs`: installed OpenCode event sequencing and
  unchanged prompt behavior without disturbing selected-workspace passthrough.
- `scripts/release-install-smoke.sh`: installed Codex plus second-host proof and
  activation output assertions.
- `scripts/mdp-post-edit-validate.sh`: no semantic change expected; touch only
  for a focused regression seam if necessary.
- `docs/agent-hook-guidance.md` and maintained proof/measurement documentation:
  record full/compact behavior, cache boundary, host degradation, and p50/p95.

## Ordered implementation steps

1. **Capture the event contract once.** Read stdin at most once, resolve
   workspace and session identity from a documented precedence order, and make
   full/compact mode explicit from `pluxx.config.ts`. Avoid consuming payload
   data separately for workspace and session parsing.
2. **Define bounded state and fingerprint contracts.** Normalize workspace
   realpath, hash identifiers before forming paths, inventory relevant `.mdp`
   authority files deterministically, exclude transient output, and specify
   cache schema/version. Reject unsafe cache roots, links, or ownership/mode
   drift.
3. **Implement atomic idempotence.** Use a per-key lock or atomic create/replace
   so concurrent start/prompt events cannot both emit full activation. Commit
   state only after the full decision is made; errors must prefer visible full
   context over false suppression.
4. **Split fast and full paths.** The unchanged compact path performs no `mdp`
   CLI calls. The full path preserves existing boundary text, readiness, secret
   redaction, capability summary, and doctor summary. Changed state emits one
   refresh and then returns to compact behavior.
5. **Preserve edit-only validation.** Keep the native `postToolUse` matcher and
   prove read/shell events stay quiet while edit/write/apply-patch events still
   reach scoped validation.
6. **Add adversarial fixtures.** Cover event-order reversal, concurrent calls,
   two workspaces in one session, two sessions in one workspace, manifest and
   profile changes, deleted/recreated pack, malformed payload, missing session
   identity, cache symlink/path attacks, stale schema, and secret non-disclosure.
7. **Build and test installed bundles.** Exercise Codex and OpenCode (or another
   native host with an existing deterministic fixture) from isolated homes and
   record host/artifact identities plus output counts.
8. **Measure and document.** Run enough cold/warm iterations to report p50/p95,
   environment, and threshold. Update hook guidance and Linear evidence. Open
   one MDP PR and stop.

## Tests and validation

Focused:

- `bash -n` for changed shell scripts.
- `scripts/test-pluxx-hooks.sh` with pinned Pluxx.
- `scripts/test-opencode-wrapper.mjs` through the repository wrapper.
- Focused release-install smoke paths for activation and post-edit behavior.
- Deterministic benchmark assertions for output size, call count, p50, and p95.

Broader:

- Existing MDP Pluxx, hook, install, OpenCode wrapper, and CI validation.
- Generated Codex descriptor remains runnable with `CODEX_PLUGIN_ROOT` unset.
- Existing native Claude Code, Cursor, Codex, and OpenCode bundle generation
  stays green.

Installed proof:

- Isolated Codex and at least one additional native host.
- Record exact source commit, bundle/archive hash, host version, event sequence,
  one full activation, unchanged compact behavior, refresh behavior, edit-only
  validation, and benchmark environment.

## Compatibility and migration

- Existing manual execution without a mode remains full and backward compatible.
- Generated native bundles change only their activation command arguments and
  bundled script/helper content.
- Cache schema is versioned; unknown/stale records are ignored and safely
  replaced. Cache cleanup is best effort and never blocks activation.
- Hosts without reliable session identity retain the current correct full
  behavior and are documented as degraded rather than sharing suppression state.

## Risks and safety boundaries

- **Cross-session suppression:** require reliable session identity; otherwise
  degrade to full output.
- **Missed authority change:** define and test the fingerprint inventory; prefer
  refresh on uncertainty.
- **Concurrent double output:** use atomic state/locking with adversarial tests.
- **Secret leakage:** persist hashes/metadata only and assert API-key values and
  hook payloads never enter state or output.
- **Pack mutation:** keep state outside the workspace and plugin installation.
- **Performance regression:** fast path must avoid CLI execution and report
  measured p50/p95, not estimates.

## Rollout, observability, and rollback

This lane stops at a validated PR. No release or active-host installation is
authorized. The observable contract is hook output count/size, CLI invocation
count, state-change refresh, edit validation count, and benchmark receipt.
Rollback is a revert to the previous two full activation calls; cache records
are non-authoritative and safe to ignore/remove.

## Blockers and readiness verdict

- PLUXX-345 and MDP-211 installed Codex root proof are complete.
- PLUXX-347 keeps this behavior on native bundles and introduces no blocker.
- The issue has no active blocked-by relation. Missing reliable session identity
  on an individual host is an explicit degradation path, not permission to
  suppress across sessions.

**Readiness: `READY_TO_PIN`.** Hosted Orchid may implement in the MDP repository,
open one PR, and stop before merge, release, deployment, or installation.
