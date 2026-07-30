#!/usr/bin/env bash
set -euo pipefail

repo_root="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
python_bin="${PYTHON:-python3}"

cli_version="$(
  awk -F'"' '/^version = / { print $2; exit }' "$repo_root/cli/Cargo.toml"
)"
plugin_version="$(
  "$python_bin" -c \
    'import json, sys; print(json.load(open(sys.argv[1]))["version"])' \
    "$repo_root/plugin/.codex-plugin/plugin.json"
)"
pluxx_version="$(
  sed -n "s/^[[:space:]]*version: '\([^']*\)'.*/\1/p" \
    "$repo_root/pluxx.config.ts"
)"

for surface in cli plugin pluxx; do
  version_variable="${surface}_version"
  if [[ -z "${!version_variable}" ]]; then
    printf 'Missing %s version\n' "$surface" >&2
    exit 1
  fi
done

if [[ "$cli_version" != "$plugin_version" ]]; then
  printf 'Version mismatch: CLI=%s plugin=%s\n' \
    "$cli_version" "$plugin_version" >&2
  exit 1
fi

if [[ "$cli_version" != "$pluxx_version" ]]; then
  printf 'Version mismatch: CLI=%s Pluxx=%s\n' \
    "$cli_version" "$pluxx_version" >&2
  exit 1
fi

printf 'Version sync OK: %s\n' "$cli_version"
