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

## Version Caveats

Codex and Claude Code hook APIs are host-specific and may change. Check the current host docs before publishing concrete config snippets:

- Codex hooks: <https://developers.openai.com/codex/hooks>
- Claude Code hooks: <https://docs.anthropic.com/en/docs/claude-code/hooks>

This repo should keep hook guidance as an operating contract unless a tested host-specific config is added and validated through the release path.
