#!/usr/bin/env bash
set -euo pipefail

if command -v mdp >/dev/null 2>&1; then
  mdp --version >/dev/null
  exit 0
fi

cat >&2 <<'EOF'
Missing required command: mdp

The Message Decision Packs plugin needs the local mdp CLI on PATH.
Run the bundled runtime bootstrap script, or install the mdp release binary
and make sure its install directory is on PATH.
EOF

exit 1
