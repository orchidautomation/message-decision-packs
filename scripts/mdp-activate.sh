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
    "${PLUGIN_ROOT:-}/scripts/mdp-native-normalize-openai.mjs" \
    "$SCRIPT_DIR/mdp-native-normalize-openai.mjs"; do
    if [ -n "$candidate" ] && [ -f "$candidate" ]; then
      return 0
    fi
  done
  return 1
}

proposal_runner_available() {
  local candidate
  for candidate in \
    "${PLUGIN_ROOT:-}/scripts/mdp-proposal-runner.mjs" \
    "$SCRIPT_DIR/mdp-proposal-runner.mjs"; do
    if [ -n "$candidate" ] && [ -f "$candidate" ]; then
      return 0
    fi
  done
  return 1
}

print_proposal_audit_readiness() {
  if [ ! -f "$TARGET_DIR/.mdp/prompts/normalize-opportunity.yaml" ]; then
    return 0
  fi

  echo
  echo "MDP proposal audit readiness:"
  if proposal_runner_available; then
    echo "  Local proposal runner: available in the plugin/source bundle."
    echo "  Inspect local runner steps with: node \"\${PLUGIN_ROOT}/scripts/mdp-proposal-runner.mjs\" tools"
  else
    echo "  Local proposal runner: not found in the plugin/source bundle."
  fi
  if native_runner_available; then
    echo "  Native OpenAI runner: available as the lower-level BYOK stateless API boundary."
  else
    echo "  Native OpenAI runner: not found in the plugin/source bundle."
  fi

  if [ -n "${OPENAI_API_KEY:-}" ]; then
    echo "  OPENAI_API_KEY: detected for optional real native API normalization (value not printed)."
  else
    echo "  OPENAI_API_KEY: not detected; only required for an optional real native OpenAI runner call."
  fi

  echo "  No OpenAI key is required for MDP install, validation, receipts, fit/review, dry-runs, mocks, or hardened headless runner audits."
  echo "  The local proposal runner is not a hosted MCP server; it is a local command surface for source staging, native/headless normalization, validation, receipts, and review probes."
  echo "  Audit-grade proposal reviews still need: mdp run-receipt --runner-audit ... --require-runner-audit."
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

print_proposal_audit_readiness

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
