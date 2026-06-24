#!/usr/bin/env bash
set -euo pipefail

TARGET_DIR="${1:-/tmp/mdp-basic-demo}"

if ! command -v mdp >/dev/null 2>&1; then
  printf '{"ok":false,"error":{"code":"mdp_missing","message":"mdp CLI is not installed on PATH","details":[]}}\n'
  exit 1
fi

mdp --json init --template gtm --name "Example Message Pack" --dir "$TARGET_DIR" --force
mdp --json validate --dir "$TARGET_DIR"
mdp --json route --entries --dir "$TARGET_DIR" --persona "GTM Engineering" --job "linkedin outbound copy"
mdp --json fit --dir "$TARGET_DIR" --prospect "$TARGET_DIR/examples/clay-row.json"
mdp --json brief --dir "$TARGET_DIR" --prospect "$TARGET_DIR/examples/clay-row.json" --channel linkedin
mdp --json copy --dir "$TARGET_DIR" --prospect "$TARGET_DIR/examples/clay-row.json" --channel linkedin
mdp --json check-claims --dir "$TARGET_DIR" --text "MDP is a local offline CLI for modular message context."
mdp --json gaps --dir "$TARGET_DIR"
mdp --json eval --dir "$TARGET_DIR"
