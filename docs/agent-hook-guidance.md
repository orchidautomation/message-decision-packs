# Agent Hook Guidance

MDP hooks should activate context and run validation feedback. They should not become hidden execution infrastructure.

Current default:

```text
Prompt starts in a workspace.
If .mdp/ exists, add model-visible MDP guidance.
If pack, prompt, schema, docs, or skill files change, run focused validation.
If validation fails, show the failure to the agent.
The agent edits files explicitly and reruns validation.
```

Do not hook automatic full brief generation as the default. Briefs depend on the user's actual intent, prospect privacy, ignored scratch paths, and whether the fit gate passes. Agents should call `mdp fit`, `mdp brief --context`, and `mdp check-claims` deliberately.

## Codex

Use Codex hooks, where available, for two visible behaviors:

- Prompt-time activation: when the workspace has `.mdp/manifest.yaml`, add a short instruction telling the model to run `mdp --json capabilities`, `mdp --json doctor --dir .`, and `mdp --json validate --dir .` before meaningful pack work.
- Pack-edit validation: after changes under `.mdp/`, `plugin/assets/templates/basic/.mdp/`, `plugin/skills/`, `docs/`, or the CLI schema/model files, run the focused validation commands that match the edit.

Good focused commands:

```bash
cargo test --manifest-path cli/Cargo.toml
cargo run --manifest-path cli/Cargo.toml -- --json validate --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json eval --dir plugin/assets/templates/basic
cargo run --manifest-path cli/Cargo.toml -- --json validate-prompt-output --dir <pack> --prompt-id <prompt-id> --file <output.json>
```

Use `make validate` for release-impacting changes or before opening a PR that changes CLI behavior, plugin bundle behavior, skills, templates, install scripts, runtime assets, or public docs.

## Claude Code

Use Claude Code hooks with the same boundary:

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
