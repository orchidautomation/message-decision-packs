# Agent Hook Guidance

MDP hooks should activate context and run validation feedback. They should not become hidden execution infrastructure.

Packaged default:

```text
Prompt starts in a workspace.
If .mdp/ exists, add model-visible MDP guidance.
If the pack has `normalize-opportunity`, report proposal audit-runner readiness, including whether the optional native OpenAI runner is available and whether `OPENAI_API_KEY` is present without printing its value.
If pack, prompt, schema, docs, or skill files change, run focused validation.
If validation fails, show the failure to the agent.
The agent edits files explicitly and reruns validation.
```

The Pluxx source config packages this behavior as bundled command hooks for supported targets. Codex and Claude Code receive native `hooks/hooks.json` files in the generated plugin bundle. Codex also receives `.codex/hooks.generated.json` as a debugging companion because runtime firing still depends on host flags, enabled plugin state, review, trust, and current host behavior.

Do not hook automatic full brief generation as the default. Briefs depend on the user's actual intent, prospect privacy, ignored scratch paths, and whether the fit gate passes. Agents should call `mdp fit`, `mdp brief --context`, and `mdp check-claims` deliberately.

## Codex

The generated Codex bundle includes `hooks/hooks.json` with command hooks for two visible behaviors:

- Startup or prompt activation: when the active workspace has `.mdp/manifest.yaml`, print MDP boundary guidance and the core commands the agent should run before meaningful pack work.
- Proposal audit readiness: when `.mdp/prompts/normalize-opportunity.yaml` exists, print a non-blocking reminder that `OPENAI_API_KEY` is required only for the optional native OpenAI model call; install, validation, run receipts, dry-runs, mocks, fit/review, and hardened headless runner audits do not need an OpenAI key.
- Post-tool validation: after tool use, detect changed pack, prompt, skill, docs, template, script, or CLI schema files and run the focused validation commands that match the edit.

Codex-compatible post-edit validation uses `postToolUse`, not `afterFileEdit`, because Pluxx maps `afterFileEdit` to an event Codex does not support today. The script self-gates to relevant edit paths. Hook scripts run from the installed plugin bundle, so Pluxx 0.1.25+ exposes the active project directory as `PLUXX_HOOK_WORKSPACE_ROOT` when the host provides a reliable workspace signal. MDP uses that value for `.mdp/manifest.yaml` checks and keeps conservative fallbacks for direct script tests or hosts that pass workspace data through common env vars or JSON hook payload fields.

Codex hook activation may require `[features].hooks = true`, an enabled plugin, review/trust, and a host version that supports plugin-bundled hooks. If hooks do not fire, inspect the generated `hooks/hooks.json` and `.codex/hooks.generated.json` files first.

Good focused commands:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json eval --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json validate-prompt-output --dir <pack> --prompt-id <prompt-id> --file <output.json>
```

Use `make validate` for release-impacting changes or before opening a PR that changes CLI behavior, plugin bundle behavior, skills, templates, install scripts, runtime assets, or public docs.

## Claude Code

The generated Claude Code bundle includes `hooks/hooks.json` with the same boundary:

- A prompt/session hook can add MDP activation guidance when `.mdp/` exists.
- A post-edit/tool hook can run focused validation after pack, prompt, skill, docs, or schema files change.
- Hook output should be visible to the agent and user, not silently applied as a rewrite.

If a Claude Code hook can mutate files, keep it limited to validation artifacts or ignored scratch that the project documents. Do not let it rewrite pack cards, prompts, skills, or prospects without normal diff review.

## Do Not Hook

- No automatic outbound sending, scheduling, CRM writeback, enrichment, scraping, or browser-session use.
- No hidden generation of full message briefs on every prompt.
- No automatic invention of missing `company_domain`, persona, trigger, signal source, fiscal year, or other readiness fields.
- No writes of raw private prospect rows, transcripts, browser data, tokens, or customer data into committed paths.

## Idempotent activation contract (MDP-281)

MDP activation hooks are idempotent and compact across supported native
hosts. The activation script is invoked once with `--mode=full` at session
start and once per prompt with `--mode=compact`. The behavior is:

| Event                   | Mode    | Behavior                                                                   |
| ----------------------- | ------- | -------------------------------------------------------------------------- |
| `sessionStart`          | `full`  | Emit the full boundary, readiness, capability/doctor summary.              |
| `beforeSubmitPrompt`    | `compact` | Empty body if workspace authority and session identity are unchanged.    |
| `beforeSubmitPrompt`    | `compact` | One bounded marker line (≤ 200 chars) on first event or after a change.   |

If a host cannot resolve a reliable session identity, the script degrades
to emitting the full activation body on every call. We never suppress
context across sessions without a reliable identity.

### Host evidence table

Installed behavioral proof or an explicit reliable-session-identity
degradation must be present for each supported native host. As of MDP-281:

| Host          | Session identity source           | Compact path evidence                                            |
| ------------- | --------------------------------- | ---------------------------------------------------------------- |
| Claude Code   | `CLAUDE_*_SESSION_ID` or hook payload | Same compact activation contract applies on the Claude bundle. |
| Cursor        | `CURSOR_SESSION_ID` or hook payload    | Same compact activation contract applies on the Cursor bundle. |
| Codex         | `CODEX_SESSION_ID` or hook payload     | `scripts/test-pluxx-hooks.sh` exercises Codex installed bundle plus installed idempotence proof in `release-install-smoke.sh`. |
| OpenCode      | **Degraded on Pluxx 0.1.42**: the native wrapper receives `input.sessionID` but does not project it into the hook environment or stdin | `scripts/test-opencode-wrapper.mjs` exercises the installed `chat.message` path and proves full activation repeats without writing suppression state. Direct installed-script coverage remains in `release-install-smoke.sh`; native compact suppression requires a future Pluxx wrapper release that propagates reliable session identity. |

If a host cannot supply a reliable session identity, that host is marked
**degraded** in the host evidence table above. Hosts marked degraded
fall back to the historical full-activation on every call rather than
sharing suppression state across sessions.

### Cache boundary

The activation state lives below
`${MDP_ACTIVATION_CACHE_ROOT:-${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/mdp-activation/}`,
keyed by the canonicalized workspace realpath. The cache records
contain only non-secret metadata (`schema-version`,
`workspace-id`, `fingerprint`, `session-hash`, `last-emitted-at`,
`full-count`, `reason`). Files are mode `0600`; the cache root is mode
`0700`. Persisted state never contains hook payloads, secrets,
`OPENAI_API_KEY` values, or absolute pack content.

### Fingerprint inventory

The fingerprint hashes the canonicalized workspace realpath, the
(installed or resolved) plugin root, the session hash, and the deterministic
list of files under `.mdp/`:

- `manifest.yaml`
- `prompts/*.yaml`
- `cards/*.yaml`
- `evals/index.json` (and any other declared eval fixtures)
- any other relative file under `.mdp/` that is not dot-prefixed or
  `__pycache__`

A change in any of those paths' size, mtime epoch, or path triggers exactly
one refresh marker before the script returns to compact behavior.

### Performance budget

The warm-unchanged `beforeSubmitPrompt` path is documented to stay well
below 25 ms p50 without invoking `mdp` or `node`. The bundled
`scripts/test-mdp-activation-benchmark.mjs` records iterations, p10/p25/p50/p75/p95,
node version, OS, and shell banner; the bench asserts against a safe
40 ms budget so regressions are caught before merge.

If the warm path ever fails this budget, do not weaken explicit `mdp
validate` or broaden the post-tool list. The fix is a tighter cache
key, a smaller inventory, or splitting the cache read into separate
scripts. Archive-size optimization is explicitly out of scope.

## Version Caveats

Codex and Claude Code hook APIs are host-specific and may change. Check the current host docs before publishing concrete config snippets:

- Codex hooks: <https://developers.openai.com/codex/hooks>
- Claude Code hooks: <https://docs.anthropic.com/en/docs/claude-code/hooks>

This repo should keep hook guidance as an operating contract unless a tested host-specific config is added and validated through the release path.
