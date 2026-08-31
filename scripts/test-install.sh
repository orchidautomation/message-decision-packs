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

portable_source="$FIXTURE_DIR/agent-plugins"
mkdir -p "$portable_source"
cp -R "$ROOT/plugin/skills" "$portable_source/skills"
cat > "$portable_source/plugin.json" <<'JSON'
{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "message-decision-packs",
  "version": "0.1.101",
  "description": "Installer fixture.",
  "author": { "name": "Orchid Labs" },
  "license": "Elastic-2.0"
}
JSON
portable_archive="$FIXTURE_DIR/message-decision-packs-agent-plugins-latest.tar.gz"
tar -czf "$portable_archive" -C "$FIXTURE_DIR" agent-plugins
node - "$portable_source" "$FIXTURE_DIR/release-manifest.json" <<'NODE'
const { createHash } = require('crypto')
const { lstatSync, readFileSync, readdirSync, writeFileSync } = require('fs')
const { join, relative } = require('path')
const [root, output] = process.argv.slice(2)
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const records = []
const walk = (directory) => {
  for (const name of readdirSync(directory).sort()) {
    const path = join(directory, name)
    const stats = lstatSync(path)
    if (stats.isDirectory()) walk(path)
    else if (stats.isFile()) records.push({
      path: relative(root, path).split('\\').join('/'),
      executable: (stats.mode & 0o111) !== 0,
      sha256: sha256(readFileSync(path)),
    })
  }
}
walk(root)
const skills = ['mdp', 'mdp-gtm-brief', 'mdp-pack-builder', 'mdp-pack-review', 'mdp-proposal-review'].sort()
writeFileSync(output, `${JSON.stringify({
  plugin: { version: '0.1.101', license: 'Elastic-2.0' },
  assets: { archives: [{ platform: 'agent-plugins', latestAsset: 'message-decision-packs-agent-plugins-latest.tar.gz' }] },
  portable_packages: { 'agent-plugins': {
    contract: 'mdp.agent-plugins-portable-package.v1',
    specification: '1.0.0',
    skills,
    mcp_servers: [],
    files: records,
    sha256: sha256(Buffer.from(`${JSON.stringify(records)}\n`)),
  } },
})}\n`)
NODE
if command -v sha256sum >/dev/null 2>&1; then
  portable_sha="$(sha256sum "$portable_archive" | awk '{print $1}')"
else
  portable_sha="$(shasum -a 256 "$portable_archive" | awk '{print $1}')"
fi
printf '%s  %s\n' "$portable_sha" "$(basename "$portable_archive")" > "$FIXTURE_DIR/SHA256SUMS.txt"

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
opencode args:--yes skip:1
EOF
)"

portable_install="$TMP_DIR/portable-install"

# A combined native + portable run must reject exact and ancestor overlaps
# before invoking any native installer or touching the existing native tree.
native_cursor="$TMP_DIR/native-cursor/message-decision-packs"
mkdir -p "$native_cursor/hooks"
printf 'native-hook\n' > "$native_cursor/hooks/keep.txt"
for colliding_portable in "$native_cursor" "$(dirname "$native_cursor")"; do
  : > "$LOG_FILE"
  if PATH="$TEST_PATH" \
    PLUXX_CURSOR_INSTALL_DIR="$native_cursor" \
    MDP_AGENT_PLUGINS_INSTALL_DIR="$colliding_portable" \
      "$ROOT/scripts/install.sh" --agents -y --base-url "$BASE_URL" \
      >"$TMP_DIR/collision.stdout" 2>"$TMP_DIR/collision.stderr"; then
    echo "Portable installer unexpectedly accepted a native-tree collision: $colliding_portable" >&2
    exit 1
  fi
  grep -F "overlaps the selected cursor native install tree" "$TMP_DIR/collision.stderr" >/dev/null
  test ! -s "$LOG_FILE"
  test "$(cat "$native_cursor/hooks/keep.txt")" = "native-hook"
done

# Portable-only mode must still refuse an existing native or unknown tree.
: > "$LOG_FILE"
if PATH="$TEST_PATH" \
  MDP_AGENT_PLUGINS_INSTALL_DIR="$native_cursor" \
    "$ROOT/scripts/install.sh" --agent-plugins -y --base-url "$BASE_URL" \
    >"$TMP_DIR/native-ownership.stdout" 2>"$TMP_DIR/native-ownership.stderr"; then
  echo "Portable-only installer unexpectedly replaced an existing native tree." >&2
  exit 1
fi
grep -F "unknown or native nonempty destination" "$TMP_DIR/native-ownership.stderr" >/dev/null
test ! -s "$LOG_FILE"
test "$(cat "$native_cursor/hooks/keep.txt")" = "native-hook"
test "$(find "$(dirname "$native_cursor")" -maxdepth 1 -name 'message-decision-packs.mdp-portable-backup.*' -print | wc -l | tr -d ' ')" = "0"

: > "$LOG_FILE"
PATH="$TEST_PATH" \
MDP_AGENT_PLUGINS_INSTALL_DIR="$portable_install" \
  "$ROOT/scripts/install.sh" --agents -y --base-url "$BASE_URL"
assert_log "$(cat <<'EOF'
cli args:--yes skip:0
cursor args:--yes skip:1
opencode args:--yes skip:1
EOF
)"
for skill in mdp mdp-gtm-brief mdp-pack-builder mdp-pack-review mdp-proposal-review; do
  test -f "$portable_install/skills/$skill/SKILL.md"
done
test -f "$portable_install/plugin.json"
test ! -e "$portable_install/hooks"
test ! -e "$portable_install/scripts"
test ! -e "$portable_install/mcp.json"

# An existing strictly owned portable destination is safely replaceable.
printf 'stale portable payload\n' > "$portable_install/skills/mdp/SKILL.md"
: > "$LOG_FILE"
PATH="$TEST_PATH" \
MDP_AGENT_PLUGINS_INSTALL_DIR="$portable_install" \
  "$ROOT/scripts/install.sh" --agent-plugins -y --base-url "$BASE_URL"
if grep -F "stale portable payload" "$portable_install/skills/mdp/SKILL.md" >/dev/null; then
  echo "Portable update did not replace the previously owned payload." >&2
  exit 1
fi
test "$(find "$(dirname "$portable_install")" -maxdepth 1 -name 'portable-install.mdp-portable-backup.*' -print | wc -l | tr -d ' ')" = "0"

if PATH="$TEST_PATH" "$ROOT/scripts/install.sh" --agent-plugins -y --base-url "$BASE_URL" \
  >"$TMP_DIR/missing-portable-dir.stdout" 2>"$TMP_DIR/missing-portable-dir.stderr"; then
  echo "Portable installer unexpectedly accepted a missing explicit install directory." >&2
  exit 1
fi
grep -F "MDP_AGENT_PLUGINS_INSTALL_DIR is required" "$TMP_DIR/missing-portable-dir.stderr" >/dev/null

: > "$LOG_FILE"
PATH="$TEST_PATH" "$ROOT/scripts/install.sh" --claude-code -y --base-url "$BASE_URL"
assert_log "claude-code args:--yes skip:0"

: > "$LOG_FILE"
PATH="$TEST_PATH" "$ROOT/scripts/install.sh" --codex -y --base-url "$BASE_URL"
assert_log "codex args:--yes skip:0"

echo "Installer fixture tests passed."
