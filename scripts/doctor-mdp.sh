#!/usr/bin/env bash
set -euo pipefail

TARGET_DIR="${1:-.}"

if ! command -v mdp >/dev/null 2>&1; then
  printf '{"ok":false,"error":{"code":"mdp_missing","message":"mdp CLI is not installed on PATH","details":[]}}\n'
  exit 1
fi

mdp --json doctor --dir "$TARGET_DIR"
