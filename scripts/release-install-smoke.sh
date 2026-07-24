#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Smoke-test a published MDP release install.

Usage:
  scripts/release-install-smoke.sh VERSION

Environment:
  MDP_RELEASE_INSTALLER      Installer script to run. Defaults to scripts/install.sh.
  MDP_RELEASE_INSTALL_HOME  Temporary HOME to use. Defaults to a new mktemp dir.
  MDP_RELEASE_INSTALL_ARGS  Installer args. Defaults to: --agents -y.
USAGE
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  usage
  exit 0
fi

version="${1:-${MDP_VERSION:-}}"
if [ -z "$version" ]; then
  echo "Release version is required." >&2
  usage >&2
  exit 1
fi

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
installer="${MDP_RELEASE_INSTALLER:-$ROOT/scripts/install.sh}"
if [ ! -f "$installer" ]; then
  echo "Installer not found: $installer" >&2
  exit 1
fi

cleanup_home=0
if [ -n "${MDP_RELEASE_INSTALL_HOME:-}" ]; then
  install_home="$MDP_RELEASE_INSTALL_HOME"
  mkdir -p "$install_home"
else
  install_home="$(mktemp -d)"
  cleanup_home=1
fi
cleanup() {
  if [ "$cleanup_home" = "1" ]; then
    rm -rf "$install_home"
  fi
}
trap cleanup EXIT

install_dir="$install_home/.local/bin"
codex_home="$install_home/.codex"
codex_plugin_root="$codex_home/plugins/message-decision-packs"
cursor_plugin_root="$install_home/.cursor/plugins/local/message-decision-packs"
opencode_plugin_root="$install_home/.config/opencode/plugins/message-decision-packs"
# shellcheck disable=SC2206
install_args=(${MDP_RELEASE_INSTALL_ARGS:---agents -y})

HOME="$install_home" \
CODEX_HOME="$codex_home" \
MDP_INSTALL_DIR="$install_dir" \
PLUXX_CODEX_CONFIG_PATH="$codex_home/config.toml" \
PLUXX_CODEX_INSTALL_DIR="$codex_plugin_root" \
PLUXX_CODEX_MARKETPLACE_PATH="$install_home/.agents/plugins/marketplace.json" \
PLUXX_CURSOR_INSTALL_DIR="$cursor_plugin_root" \
PLUXX_OPENCODE_PLUGIN_ROOT_DIR="$install_home/.config/opencode/plugins" \
PLUXX_OPENCODE_INSTALL_DIR="$opencode_plugin_root" \
PLUXX_OPENCODE_ENTRY_PATH="$install_home/.config/opencode/plugins/message-decision-packs.ts" \
PLUXX_OPENCODE_SKILLS_ROOT="$install_home/.config/opencode/skills" \
PLUXX_INSTALL_LOCK_ROOT="$install_home/.pluxx/install-locks" \
PLUXX_RUNTIME_STORE_ROOT="$install_home/.pluxx/runtimes" \
  bash "$installer" --version "$version" "${install_args[@]}"

mdp_bin="$install_dir/mdp"
if [ ! -x "$mdp_bin" ]; then
  echo "Installed mdp binary not found or not executable: $mdp_bin" >&2
  exit 1
fi
"$mdp_bin" --version

if [ ! -d "$codex_plugin_root" ]; then
  echo "Installed Codex plugin root not found: $codex_plugin_root" >&2
  exit 1
fi

for required in \
  "$codex_plugin_root/scripts/mdp-proposal-runner.mjs" \
  "$codex_plugin_root/scripts/mdp-proposal-mcp-server.mjs" \
  "$codex_plugin_root/scripts/mdp-native-normalize-openai.mjs" \
  "$codex_plugin_root/scripts/mdp-activate.sh" \
  "$codex_plugin_root/skills/mdp/SKILL.md" \
  "$codex_plugin_root/skills/mdp-proposal-review/SKILL.md"; do
  if [ ! -f "$required" ]; then
    echo "Installed plugin is missing required file: $required" >&2
    exit 1
  fi
done

if find "$codex_plugin_root" -type d -name __pycache__ | grep -q .; then
  echo "Installed plugin must not contain Python __pycache__ directories." >&2
  find "$codex_plugin_root" -type d -name __pycache__ >&2
  exit 1
fi

tools_json="$(node "$codex_plugin_root/scripts/mdp-proposal-runner.mjs" tools)"
for expected in \
  "mdp_run_receipt" \
  "bundled local stdio MCP wrapper" \
  "hosted or remote MCP"; do
  if ! printf '%s\n' "$tools_json" | grep -F "$expected" >/dev/null; then
    echo "Installed proposal runner tools output is missing MCP/local guardrail text: $expected" >&2
    printf '%s\n' "$tools_json" >&2
    exit 1
  fi
done

mcp_list_stdout="$(
  printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{"name":"release-install-smoke","version":"0.0.0"},"capabilities":{}}}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' |
    node "$codex_plugin_root/scripts/mdp-proposal-mcp-server.mjs"
)"
for expected in \
  "message-decision-packs-proposal" \
  "mdp_proposal_tools" \
  "mdp_proposal_run" \
  "Raw chat text is intentionally not accepted"; do
  if ! printf '%s\n' "$mcp_list_stdout" | grep -F "$expected" >/dev/null; then
    echo "Installed proposal MCP server list output missing expected text: $expected" >&2
    printf '%s\n' "$mcp_list_stdout" >&2
    exit 1
  fi
done

proposal_fixture="$(mktemp -d)"
trap 'rm -rf "$proposal_fixture"; cleanup' EXIT
"$mdp_bin" --json init --template proposal --dir "$proposal_fixture" >/tmp/mdp-release-install-init.json
"$mdp_bin" --json validate --dir "$proposal_fixture" >/tmp/mdp-release-install-validate.json

activation_output="$(
  HOME="$install_home" \
  CODEX_HOME="$codex_home" \
  PATH="$install_dir:$PATH" \
  PLUGIN_ROOT="$codex_plugin_root" \
  PLUXX_HOOK_WORKSPACE_ROOT="$proposal_fixture" \
  OPENAI_API_KEY= \
  bash "$codex_plugin_root/scripts/mdp-activate.sh"
)"
for expected in \
  "Local proposal runner: available in the plugin/source bundle." \
  "Native OpenAI runner: available as the lower-level BYOK stateless API boundary." \
  "OPENAI_API_KEY: not detected; only required for an optional real native OpenAI runner call." \
  "Local stdio MCP wrapper: available" \
  "MCP tools: mdp_proposal_tools and mdp_proposal_run" \
  "The bundled MCP is local stdio only, not a hosted or remote MCP service." \
  "MCP transport alone is not audit-grade; audit-grade proposal reviews still need: mdp run-receipt --runner-audit ... --require-runner-audit." \
  "Hooks report readiness only; the CLI receipt is the blocking gate."; do
  if ! printf '%s\n' "$activation_output" | grep -F "$expected" >/dev/null; then
    echo "Installed activation output missing expected guardrail: $expected" >&2
    printf '%s\n' "$activation_output" >&2
    exit 1
  fi
done

if [ -f "$ROOT/scripts/skill-eval-harness.py" ]; then
  python3 "$ROOT/scripts/skill-eval-harness.py" \
    --mdp-bin "$mdp_bin" \
    --installed-skills-root "$codex_plugin_root/skills" >/tmp/mdp-release-install-skill-eval.json
fi

echo "Release install smoke passed for $version at $install_home"
