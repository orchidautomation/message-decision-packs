#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture_root="$(mktemp -d)"
python_bin="${PYTHON:-python3}"
trap 'rm -rf "$fixture_root"' EXIT

mkdir -p \
  "$fixture_root/cli" \
  "$fixture_root/plugin/.codex-plugin"
cp "$repo_root/cli/Cargo.toml" "$fixture_root/cli/Cargo.toml"
cp \
  "$repo_root/plugin/.codex-plugin/plugin.json" \
  "$fixture_root/plugin/.codex-plugin/plugin.json"
cp "$repo_root/pluxx.config.ts" "$fixture_root/pluxx.config.ts"

"$repo_root/scripts/validate-version-sync.sh" "$fixture_root" >/dev/null

"$python_bin" - "$fixture_root/plugin/.codex-plugin/plugin.json" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
manifest = json.loads(path.read_text())
manifest["version"] = "9.9.9"
path.write_text(json.dumps(manifest, indent=2) + "\n")
PY

if "$repo_root/scripts/validate-version-sync.sh" "$fixture_root" >/dev/null 2>&1; then
  echo "Expected a plugin-version mismatch to fail" >&2
  exit 1
fi

cp \
  "$repo_root/plugin/.codex-plugin/plugin.json" \
  "$fixture_root/plugin/.codex-plugin/plugin.json"
"$python_bin" - "$fixture_root/pluxx.config.ts" <<'PY'
import pathlib
import re
import sys

path = pathlib.Path(sys.argv[1])
content = path.read_text()
updated, count = re.subn(r"version: '[^']*'", "version: '9.9.9'", content, count=1)
if count != 1:
    raise SystemExit("expected exactly one Pluxx version field")
path.write_text(updated)
PY

if "$repo_root/scripts/validate-version-sync.sh" "$fixture_root" >/dev/null 2>&1; then
  echo "Expected a Pluxx-version mismatch to fail" >&2
  exit 1
fi

echo "Version sync regression tests passed"
