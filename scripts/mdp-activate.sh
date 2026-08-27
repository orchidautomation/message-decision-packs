#!/usr/bin/env bash
set -euo pipefail

read_hook_payload() {
  local payload=""
  local line=""

  if [ -t 0 ]; then
    return 0
  fi

  if IFS= read -r -t 1 line || [ -n "$line" ]; then
    payload="$line"
    while IFS= read -r -t 1 line; do
      payload="$payload
$line"
    done
  fi

  printf '%s' "$payload"
}

workspace_from_payload() {
  local payload="$1"
  if [ -z "$payload" ] || ! command -v node >/dev/null 2>&1; then
    return 0
  fi

  node -e '
const fs = require("fs")
let data
try {
  data = JSON.parse(process.argv[1] || "")
} catch {
  process.exit(0)
}
const values = [
  data.cwd,
  data.workdir,
  data.workspace,
  data.workspaceRoot,
  data.projectRoot,
  data.project_dir,
  data.project && data.project.cwd,
  data.project && data.project.root,
  data.tool_input && data.tool_input.cwd,
  data.tool_input && data.tool_input.workdir,
]
for (const value of values) {
  if (typeof value === "string" && value && fs.existsSync(value)) {
    process.stdout.write(value)
    process.exit(0)
  }
}
' "$payload"
}

resolve_target_dir() {
  if [ -n "${MDP_HOOK_DIR:-}" ]; then
    printf '%s\n' "$MDP_HOOK_DIR"
    return 0
  fi

  local var value
  for var in PLUXX_HOOK_WORKSPACE_ROOT CODEX_WORKSPACE_ROOT CODEX_WORKDIR CODEX_CWD CLAUDE_PROJECT_DIR CLAUDE_CWD CURSOR_WORKSPACE_ROOT OPENCODE_WORKSPACE_ROOT WORKSPACE_ROOT PROJECT_ROOT; do
    value="${!var:-}"
    if [ -n "$value" ] && [ -d "$value" ]; then
      printf '%s\n' "$value"
      return 0
    fi
  done

  value="$(workspace_from_payload "$(read_hook_payload)")"
  if [ -n "$value" ] && [ -d "$value" ]; then
    printf '%s\n' "$value"
    return 0
  fi

  if [ -n "${PWD:-}" ] && [ -d "$PWD" ] && [ "${PLUGIN_ROOT:-}" != "$PWD" ]; then
    printf '%s\n' "$PWD"
  fi
}

TARGET_DIR="$(resolve_target_dir)"
if [ -z "$TARGET_DIR" ]; then
  exit 0
fi
MANIFEST="$TARGET_DIR/.mdp/manifest.yaml"

if [ ! -f "$MANIFEST" ]; then
  exit 0
fi

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

native_runner_available() {
  local candidate
  for candidate in \
    "${PLUGIN_ROOT:-}/scripts/mdp-native-model-openai.mjs" \
    "$SCRIPT_DIR/mdp-native-model-openai.mjs"; do
    if [ -n "$candidate" ] && [ -f "$candidate" ]; then
      return 0
    fi
  done
  return 1
}

run_mcp_path() {
  if [ -n "${PLUGIN_ROOT:-}" ] && [ -f "$PLUGIN_ROOT/scripts/mdp-run-mcp-server.mjs" ]; then
    printf '%s\n' "$PLUGIN_ROOT/scripts/mdp-run-mcp-server.mjs"
    return 0
  fi
  if [ -f "$SCRIPT_DIR/mdp-run-mcp-server.mjs" ]; then
    printf '%s\n' "$SCRIPT_DIR/mdp-run-mcp-server.mjs"
    return 0
  fi
  return 1
}

print_clean_run_readiness() {
  local run_mcp=""
  echo
  echo "MDP clean-run readiness:"
  if run_mcp="$(run_mcp_path)"; then
    echo "  Canonical local stdio MCP: available as node \"$run_mcp\"."
    echo "  MCP path: mdp_run_tools -> mdp_prepare_run -> mdp_run -> mdp_verify_run."
    echo "  Artifacts: boundary inventory -> mdp.run-request.v1 -> run bundle/receipt -> mdp.run-verification.v1."
  else
    echo "  Canonical local stdio MCP: not found in the plugin/source bundle."
  fi
  if native_runner_available; then
    echo "  Canonical native OpenAI driver: available for an operator-authorized BYOK model step."
  else
    echo "  Canonical native OpenAI driver: not found in the plugin/source bundle."
  fi

  if [ -n "${OPENAI_API_KEY:-}" ]; then
    echo "  OPENAI_API_KEY: detected for optional real native API normalization (value not printed)."
  else
    echo "  OPENAI_API_KEY: not detected; only required for an optional real native OpenAI runner call."
  fi

  echo "  No OpenAI key is required for MDP install, validation, receipts, fit/review, dry-runs, mocks, or hardened headless runner audits."
  echo "  The canonical MCP is local stdio transport only, not a hosted or remote MCP service."
  echo "  MCP adds no authority or isolation assurance; the CLI result, receipt, and verification remain authoritative."
  echo "  Hooks report readiness only; the CLI receipt is the blocking gate."
}

echo "MDP activation: .mdp/manifest.yaml detected in $TARGET_DIR."
echo "Use MDP as visible context and validation, not as hidden execution infrastructure."
echo "Read-only commands to run before meaningful pack work:"
echo "  mdp --json capabilities"
echo "  mdp --json doctor --dir \"$TARGET_DIR\""
echo "  mdp --json validate --dir \"$TARGET_DIR\""
echo "Deliberate commands for later use: mdp fit, mdp brief --context, mdp check-claims, mdp gaps, mdp eval."
echo "Do not enrich, scrape, send outreach, update a CRM, or auto-generate full briefs from hook activation."

print_clean_run_readiness

if ! command -v mdp >/dev/null 2>&1; then
  echo "MDP activation warning: mdp CLI is not installed on PATH."
  echo "Install with: bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y"
  exit 0
fi

run_visible() {
  local label="$1"
  shift
  local output

  echo
  echo "MDP activation check: $label"
  if output="$("$@" 2>&1)"; then
    printf '%s\n' "$output"
  else
    local status=$?
    printf '%s\n' "$output"
    echo "MDP activation warning: $label exited with status $status."
  fi
}

run_visible "capabilities summary" mdp --json --summary capabilities
run_visible "doctor summary" mdp --json --summary doctor --dir "$TARGET_DIR"
