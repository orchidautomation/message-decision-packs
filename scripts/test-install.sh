#!/usr/bin/env bash
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

FIXTURE_DIR="$TMP_DIR/release"
LOG_FILE="$TMP_DIR/install.log"
mkdir -p "$FIXTURE_DIR"

write_installer() {
  local target="$1"
  local path="$FIXTURE_DIR/install-$target.sh"

  cat > "$path" <<SH
#!/usr/bin/env bash
set -euo pipefail
echo "$target args:\$* skip:\${MDP_SKIP_CLI_UPDATE:-0}" >> "$LOG_FILE"
SH
  chmod +x "$path"
}

assert_log() {
  local expected="$1"
  local actual
  actual="$(cat "$LOG_FILE")"
  if [[ "$actual" != "$expected" ]]; then
    echo "Unexpected installer dispatch log." >&2
    echo "Expected:" >&2
    printf '%s\n' "$expected" >&2
    echo "Actual:" >&2
    printf '%s\n' "$actual" >&2
    exit 1
  fi
}

for target in cli claude-code cursor codex opencode; do
  write_installer "$target"
done

BASE_URL="file://$FIXTURE_DIR"
TEST_PATH="/usr/bin:/bin"

: > "$LOG_FILE"
PATH="$TEST_PATH" "$ROOT/scripts/install.sh" --cli -y --base-url "$BASE_URL"
assert_log "cli args:--yes skip:0"

: > "$LOG_FILE"
PATH="$TEST_PATH" "$ROOT/scripts/install.sh" --cli-only -y --base-url "$BASE_URL"
assert_log "cli args:--yes skip:0"

: > "$LOG_FILE"
PATH="$TEST_PATH" "$ROOT/scripts/install.sh" --agents -y --base-url "$BASE_URL"
assert_log "$(cat <<'EOF'
cli args:--yes skip:0
cursor args:--yes skip:1
codex args:--yes skip:1
opencode args:--yes skip:1
EOF
)"

: > "$LOG_FILE"
PATH="$TEST_PATH" "$ROOT/scripts/install.sh" --claude-code -y --base-url "$BASE_URL"
assert_log "claude-code args:--yes skip:0"

echo "Installer fixture tests passed."
