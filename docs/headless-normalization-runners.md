# Headless Normalization Runners

MDP is intentionally not the model runner. The plugin teaches the agent what to do, and the CLI validates/hash-gates the artifacts it receives. For production proposal or document review, the model call that turns messy source material into `mdp.prompt-output.v0` should run in a headless/stateless boundary, not inside the operator's current chat context.

This lets the user keep a one-thread workshop UX while the implementation keeps two separate planes:

```text
Control plane:  ChatGPT/Codex/Claude/Copilot conversation, status, questions, final explanation
Evidence plane: local source files -> source audit -> prompt package -> model output -> validation -> run receipt -> fit/proof checks
```

## Required Artifact Chain

A production proposal normalization runner should create or preserve these local artifacts:

1. `mdp.source-audit.v0` — bounded extraction ledger for the supplied PDF/doc/text material.
2. Prompt package — the selected `.mdp/prompts/normalize-opportunity.yaml` plus only its declared inputs (`raw_opportunity`, `existing_pack_context`, optional `runtime_context`, `source_kind`, and source-audit references as needed).
3. `mdp.prompt-output.v0` — strict JSON model output.
4. `mdp.validate-prompt-output.v0` result — from `mdp --json validate-prompt-output --source-audit`.
5. `mdp.runner-audit.v0` — host-owned evidence that the model call was isolated/headless/stateless.
6. `mdp.run-receipt.v0` — CLI-owned hash gate tying the artifacts together.

Then run:

```bash
mdp --json validate-prompt-output \
  --dir <pack-root> \
  --prompt-id normalize-opportunity \
  --file <normalize-opportunity-output.json> \
  --source-audit <source-audit.json> \
  > <validate-prompt-output-result.json>

mdp --json run-receipt \
  --dir <pack-root> \
  --workflow proposal-review \
  --isolation isolated \
  --declared-inputs-only \
  --prompt-id normalize-opportunity \
  --prompt-output <normalize-opportunity-output.json> \
  --validation <validate-prompt-output-result.json> \
  --source-audit <source-audit.json> \
  --runner-audit <runner-audit.json> \
  --require-runner-audit \
  --out <run-receipt.json>
```

Without `--require-runner-audit`, `run-receipt` can still record a host assertion. For paid pilots, use `--require-runner-audit` so the flow blocks when the headless boundary is missing or malformed.

## Runner Audit Contract

Get the schema with:

```bash
mdp --json schema runner-audit
```

Minimal Claude Code headless audit:

```json
{
  "contract": "mdp.runner-audit.v0",
  "runner": "claude-print",
  "model": "sonnet",
  "isolated_invocation": true,
  "conversation_resume": false,
  "declared_inputs_only": true,
  "output_schema_used": true,
  "bare": true,
  "session_persistence": false,
  "tools_disabled": true,
  "tool_invocations_observed": 0
}
```

Minimal Codex headless audit:

```json
{
  "contract": "mdp.runner-audit.v0",
  "runner": "codex-exec",
  "model": "gpt-5.4-codex",
  "isolated_invocation": true,
  "conversation_resume": false,
  "declared_inputs_only": true,
  "output_schema_used": true,
  "ephemeral": true,
  "session_persistence": false,
  "sterile_workdir": true,
  "prompt_input_audited": true,
  "config_discovery_disabled": true,
  "instructions_discovery_disabled": true,
  "sandbox": "read-only",
  "tool_invocations_observed": 0
}
```


Minimal Cursor headless audit:

```json
{
  "contract": "mdp.runner-audit.v0",
  "runner": "cursor-print",
  "model": "configured-by-host",
  "isolated_invocation": true,
  "conversation_resume": false,
  "declared_inputs_only": true,
  "output_schema_used": true,
  "session_persistence": false,
  "force_enabled": false,
  "sterile_workdir": true,
  "prompt_input_audited": true,
  "tools_disabled": true,
  "tool_invocations_observed": 0
}
```

Minimal OpenCode headless audit:

```json
{
  "contract": "mdp.runner-audit.v0",
  "runner": "opencode-run",
  "model": "configured-provider/model",
  "isolated_invocation": true,
  "conversation_resume": false,
  "declared_inputs_only": true,
  "output_schema_used": true,
  "session_persistence": false,
  "pure": true,
  "default_plugins_disabled": true,
  "claude_code_discovery_disabled": true,
  "sterile_workdir": true,
  "tools_disabled": true,
  "tool_invocations_observed": 0
}
```

Minimal native API audit:

```json
{
  "contract": "mdp.runner-audit.v0",
  "runner": "native-api",
  "model": "configured-by-host",
  "isolated_invocation": true,
  "conversation_resume": false,
  "declared_inputs_only": true,
  "output_schema_used": true,
  "stateless_request": true,
  "prior_messages_included": false,
  "tools_disabled": true,
  "tool_invocations_observed": 0
}
```

## Claude Code Headless Recipe

Official Claude Code docs define non-interactive mode as `claude -p` and recommend `--bare` for scripted calls because it skips auto-discovery of hooks, skills, plugins, MCP servers, auto memory, and `CLAUDE.md`. They also support JSON output plus `--json-schema`, `--no-session-persistence`, and tool restriction flags.

For MDP normalization, the safe shape is:

```bash
claude --bare -p \
  --no-session-persistence \
  --disallowedTools "*" \
  --max-turns 1 \
  --output-format json \
  --json-schema '<prompt-output-json-schema>' \
  < <prompt-package.txt>
```

Runner requirements:

- Do not use `--continue` or `--resume`.
- Do not load MDP plugin skills into the normalization model call; the prompt package already contains the needed contract.
- Extract `.structured_output` as the prompt output artifact.
- Record `runner: "claude-print"`, `bare: true`, `session_persistence: false`, `tools_disabled: true`, and `tool_invocations_observed: 0` in `mdp.runner-audit.v0`.
- Keep piped payloads under the documented stdin cap, or chunk/stage source text before building the prompt package.

## Codex Headless Recipe

Official Codex docs define non-interactive mode as `codex exec`. The docs call out `--ephemeral`, default read-only sandboxing, `--ignore-user-config`, JSONL events, `--output-schema`, `--output-last-message`, and `codex exec resume`. Codex also loads `AGENTS.md` from `CODEX_HOME` and the project path at startup, so a normal repo invocation is not enough isolation for proposal normalization.

For MDP normalization, prefer a sterile run directory and an isolated Codex home:

```bash
CODEX_HOME=<empty-codex-home-with-only-intended-auth> \
CODEX_API_KEY=<single-invocation-secret-if-needed> \
  codex exec \
    --ephemeral \
    --ignore-user-config \
    --ignore-rules \
    --cd <sterile-run-dir> \
    --skip-git-repo-check \
    --sandbox read-only \
    --json \
    --output-schema <prompt-output-json-schema> \
    --output-last-message <normalize-opportunity-output.json> \
    - < <prompt-package.txt> \
    > <codex-events.jsonl>
```

Runner requirements:

- Do not use `codex exec resume` or any previous session ID.
- Do not run from the proposal repo, MDP repo, or a directory with `AGENTS.md`; use a sterile directory containing only runner-owned scratch.
- Use a temporary/minimal `CODEX_HOME` so global `AGENTS.md` and normal config do not enter the model context. `--ignore-user-config` prevents config loading, but it does not by itself prove instruction discovery is sterile.
- Use `codex debug prompt-input` or equivalent host inspection to confirm the model-visible prompt input list contains only the intended prompt package.
- Parse the JSONL event log and require zero command executions, file changes, MCP calls, web searches, or other tool events during normalization.
- Record `runner: "codex-exec"`, `ephemeral: true`, `sterile_workdir: true`, `prompt_input_audited: true`, `config_discovery_disabled: true`, `instructions_discovery_disabled: true`, `sandbox: "read-only"`, and `tool_invocations_observed: 0` in `mdp.runner-audit.v0`.


## Cursor Headless Recipe

Cursor CLI documents print mode (`cursor-agent -p` / `--print`) for non-interactive scripts and `--output-format text|json|stream-json` for machine-readable output. It also documents that print mode has access to tools, including write and shell, and that `--force` allows direct changes without confirmation.

For MDP normalization, Cursor headless is usable only behind a wrapper that removes or externally denies tools. Do not treat plain `cursor-agent -p` as audit-grade by itself.

Safe shape:

```bash
cursor-agent -p \
  --output-format stream-json \
  --model <model> \
  "<normalization prompt package>" \
  > <cursor-events.jsonl>
```

Runner requirements:

- Do not use `--resume` or any previous chat ID.
- Do not use `--force` for normalization.
- Run from a sterile directory and pass the prompt package explicitly.
- Use host policy, container sandboxing, or a dedicated wrapper to deny write/shell/tool use.
- Parse stream JSON and require zero tool calls.
- Record `runner: "cursor-print"`, `force_enabled: false`, `sterile_workdir: true`, `prompt_input_audited: true`, `tools_disabled: true`, and `tool_invocations_observed: 0` in `mdp.runner-audit.v0`.

## OpenCode Headless Recipe

OpenCode documents `opencode run` as non-interactive mode, `opencode serve` as a headless API server, `--format json` for raw JSON events, resume/session flags that continue prior sessions, `--pure` to run without external plugins, and environment flags that disable default plugins and Claude Code prompt/skill discovery.

For MDP normalization, prefer one-shot `opencode run` rather than attaching to a long-lived server unless the server is dedicated to the isolated MDP runner.

Safe shape:

```bash
OPENCODE_DISABLE_DEFAULT_PLUGINS=true \
OPENCODE_DISABLE_CLAUDE_CODE=true \
OPENCODE_DISABLE_CLAUDE_CODE_PROMPT=true \
OPENCODE_DISABLE_CLAUDE_CODE_SKILLS=true \
  opencode --pure run \
    --dir <sterile-run-dir> \
    --format json \
    --model <provider/model> \
    --agent <no-tool-normalizer-agent> \
    "<normalization prompt package>" \
    > <opencode-events.jsonl>
```

Runner requirements:

- Do not use `--continue`, `--session`, `--fork`, or an attached shared server for audit-grade normalization.
- Use `--pure` and disable default plugin and Claude Code discovery env flags.
- Use a no-tool/no-permission agent configuration.
- Run from a sterile directory and pass the prompt package explicitly.
- Parse JSON events and require zero tool calls.
- Record `runner: "opencode-run"`, `pure: true`, `default_plugins_disabled: true`, `claude_code_discovery_disabled: true`, `sterile_workdir: true`, `tools_disabled: true`, and `tool_invocations_observed: 0` in `mdp.runner-audit.v0`.

## Recommended Architecture

For the product, the clean path is one installable MDP plugin bundle plus a runner/MCP layer that the plugin can call:

- Pluxx continues to package the authored skills, hooks, and assets from this repo for each host.
- The runner/MCP owns PDF/doc ingestion, prompt-package construction, native/headless model invocation, artifact persistence, and runner-audit emission.
- Pluxx may generate host-specific install/config shims that point Cursor, OpenCode, Claude, Codex, or Copilot toward the same runner contract, but Pluxx should not be the only place that defines or enforces audit-grade isolation.
- The `mdp` CLI owns deterministic validation, source-audit checks, fit/proof/routing checks, and run-receipt gating.
- The chat agent owns user guidance, questions, and final explanation, but it should not normalize proposal facts in its ambient conversation when audit-grade output is required.

Same-conversation normalization is still useful for drafting, debugging, and workshops. It should be labeled `advisory`, not `audit-grade`, unless the runner receipt includes a valid runner audit.

## Source Docs

- Codex non-interactive mode: <https://learn.chatgpt.com/docs/non-interactive-mode>
- Codex AGENTS.md discovery: <https://learn.chatgpt.com/docs/agent-configuration/agents-md>
- Claude Code headless/programmatic usage: <https://code.claude.com/docs/en/headless>
- Claude Code CLI flags: <https://code.claude.com/docs/en/cli-usage>
- Cursor headless CLI: <https://cursor.com/docs/cli/headless>
- Cursor CLI output formats: <https://docs.cursor.com/en/cli/reference/output-format>
- OpenCode CLI: <https://opencode.ai/docs/cli/>
