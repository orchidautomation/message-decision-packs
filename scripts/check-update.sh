#!/usr/bin/env bash
set -euo pipefail

REPO="${MDP_GITHUB_REPO:-orchidautomation/message-decision-packs}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

json_escape() {
  python3 -c 'import json,sys; print(json.dumps(sys.stdin.read().strip()))'
}

version_from_plugin_json() {
  local file="$1"
  python3 - "$file" <<'PY'
import json
import sys

try:
    with open(sys.argv[1], "r", encoding="utf-8") as handle:
        print(json.load(handle).get("version", ""))
except Exception:
    print("")
PY
}

latest_release_tag() {
  curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" |
    python3 -c 'import json,sys; print(json.load(sys.stdin).get("tag_name", ""))'
}

normalize_version() {
  local value="$1"
  value="${value#v}"
  printf '%s' "$value"
}

need_cmd curl
need_cmd python3

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
plugin_version=""

for candidate in \
  "$script_dir/../.codex-plugin/plugin.json" \
  "$script_dir/../../.codex-plugin/plugin.json" \
  "$script_dir/../plugin/.codex-plugin/plugin.json"
do
  if [ -f "$candidate" ]; then
    plugin_version="$(version_from_plugin_json "$candidate")"
    if [ -n "$plugin_version" ]; then
      break
    fi
  fi
done

cli_version=""
if command -v mdp >/dev/null 2>&1; then
  cli_version="$(mdp --version 2>/dev/null | sed -E 's/.*([0-9]+\.[0-9]+\.[0-9]+).*/\1/' | head -n 1 || true)"
fi

latest_tag="$(latest_release_tag)"
latest_version="$(normalize_version "$latest_tag")"
cli_status="unknown"
plugin_status="unknown"

if [ -n "$cli_version" ] && [ -n "$latest_version" ] && [ "$cli_version" = "$latest_version" ]; then
  cli_status="current"
elif [ -n "$cli_version" ] && [ -n "$latest_version" ]; then
  cli_status="stale"
fi

if [ -n "$plugin_version" ] && [ -n "$latest_version" ] && [ "$plugin_version" = "$latest_version" ]; then
  plugin_status="current"
elif [ -n "$plugin_version" ] && [ -n "$latest_version" ]; then
  plugin_status="stale"
fi

full_install_command="bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y"
cli_only_install_command="bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --cli -y"

cat <<JSON
{
  "ok": true,
  "repo": $(printf '%s' "$REPO" | json_escape),
  "latest": {
    "tag": $(printf '%s' "$latest_tag" | json_escape),
    "version": $(printf '%s' "$latest_version" | json_escape)
  },
  "local": {
    "cli_version": $(printf '%s' "$cli_version" | json_escape),
    "plugin_version": $(printf '%s' "$plugin_version" | json_escape)
  },
  "status": {
    "cli": $(printf '%s' "$cli_status" | json_escape),
    "plugin": $(printf '%s' "$plugin_status" | json_escape)
  },
  "update_command": $(printf '%s' "$full_install_command" | json_escape),
  "full_install_command": $(printf '%s' "$full_install_command" | json_escape),
  "cli_only_install_command": $(printf '%s' "$cli_only_install_command" | json_escape)
}
JSON
