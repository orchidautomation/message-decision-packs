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

write_agents_installer() {
  local path="$FIXTURE_DIR/install-agents.sh"
  cat > "$path" <<SH
#!/usr/bin/env bash
set -euo pipefail
echo "agents args:\$*" >> "$LOG_FILE"
mode=explicit
selected=()
for arg in "\$@"; do
  case "\$arg" in
    --agents) mode=aggregate ;;
    --claude-code|--cursor|--codex|--opencode) selected+=("\${arg#--}") ;;
  esac
done
if [ "\$mode" = aggregate ]; then selected=(claude-code cursor codex opencode); fi
items=""
failed=0
for target in "\${selected[@]}"; do
  state=installed; reason=""; error=""; action=""
  if [ "\$mode" = aggregate ]; then
    case " \${MDP_TEST_DETECTED_HOSTS:-cursor opencode} " in *" \$target "*) ;; *) state=skipped; reason=host-not-detected;; esac
  fi
  if [ "\${MDP_TEST_FAIL_TARGET:-}" = "\$target" ]; then state=failed; error="fixture failure"; action="repair fixture"; failed=1; fi
  items="\$items\$target|\$state|\$reason|\$error|\$action
"
done
PLUXX_TEST_ITEMS="\$items" PLUXX_TEST_MODE="\$mode" node <<'NODE'
const results = process.env.PLUXX_TEST_ITEMS.trim().split(/\n/).filter(Boolean).map((line) => {
  const [target,state,reason,error,action] = line.split('|');
  return {target,state,...(reason?{reason}:{}),...(error?{error,action}:{})}
})
console.log(JSON.stringify({schema:'pluxx.install-results.v1',plugin:{name:'message-decision-packs',version:'0.1.101'},selectionMode:process.env.PLUXX_TEST_MODE,plan:results.map(({target})=>({target,selected:true})),results}))
NODE
exit "\$failed"
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
write_agents_installer

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
const skills = ['mdp', 'mdp-pack-apply', 'mdp-pack-builder', 'mdp-pack-review'].sort()
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
for asset in install-cli.sh install-agents.sh; do
  if command -v sha256sum >/dev/null 2>&1; then asset_sha="$(sha256sum "$FIXTURE_DIR/$asset" | awk '{print $1}')"
  else asset_sha="$(shasum -a 256 "$FIXTURE_DIR/$asset" | awk '{print $1}')"; fi
  printf '%s  %s\n' "$asset_sha" "$asset" >> "$FIXTURE_DIR/SHA256SUMS.txt"
done

BASE_URL="file://$FIXTURE_DIR"
TEST_PATH="/usr/bin:/bin"

: > "$LOG_FILE"
PATH="$TEST_PATH" "$ROOT/scripts/install.sh" --cli -y --base-url "$BASE_URL"
assert_log "cli args: skip:0"

: > "$LOG_FILE"
PATH="$TEST_PATH" "$ROOT/scripts/install.sh" --cli-only -y --base-url "$BASE_URL"
assert_log "cli args: skip:0"

: > "$LOG_FILE"
PATH="$TEST_PATH" "$ROOT/scripts/install.sh" --agents -y --base-url "$BASE_URL" \
  >"$TMP_DIR/agents.stdout" 2>"$TMP_DIR/agents.stderr"
assert_log "$(printf 'cli args: skip:0\nagents args:--json --quiet --base-url %s --version 0.1.101 --yes --agents' "$BASE_URL")"
if grep -F 'MDP_AGENT_PLUGINS_INSTALL_DIR' "$TMP_DIR/agents.stderr" >/dev/null; then
  echo "Ordinary --agents output unexpectedly warned about portable routing." >&2
  exit 1
fi
grep -F 'claude-code: skipped (host-not-detected)' "$TMP_DIR/agents.stdout" >/dev/null

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
assert_log "$(printf 'cli args: skip:0\nagents args:--json --quiet --base-url %s --version 0.1.101 --yes --agents' "$BASE_URL")"
for skill in mdp mdp-pack-apply mdp-pack-builder mdp-pack-review; do
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

PATH="$TEST_PATH" MDP_AGENT_PLUGINS_INSTALL_DIR="$portable_install" \
  "$ROOT/scripts/install.sh" --agent-plugins -y --base-url "$BASE_URL" >"$TMP_DIR/portable-repeat.stdout"
grep -F 'agent-plugins: unchanged' "$TMP_DIR/portable-repeat.stdout" >/dev/null

if PATH="$TEST_PATH" "$ROOT/scripts/install.sh" --agent-plugins -y --base-url "$BASE_URL" \
  >"$TMP_DIR/missing-portable-dir.stdout" 2>"$TMP_DIR/missing-portable-dir.stderr"; then
  echo "Portable installer unexpectedly accepted a missing explicit install directory." >&2
  exit 1
fi
grep -F "MDP_AGENT_PLUGINS_INSTALL_DIR is required" "$TMP_DIR/missing-portable-dir.stderr" >/dev/null

: > "$LOG_FILE"
PATH="$TEST_PATH" "$ROOT/scripts/install.sh" --claude-code -y --base-url "$BASE_URL"
assert_log "agents args:--json --quiet --base-url $BASE_URL --version 0.1.101 --yes --claude-code"

: > "$LOG_FILE"
PATH="$TEST_PATH" "$ROOT/scripts/install.sh" --codex -y --base-url "$BASE_URL"
assert_log "agents args:--json --quiet --base-url $BASE_URL --version 0.1.101 --yes --codex"

# An exact installed CLI version is a true no-op unless forced.
fake_path="$TMP_DIR/fake-path"
mkdir -p "$fake_path"
cat > "$fake_path/mdp" <<'EOF'
#!/usr/bin/env bash
echo 'mdp 0.1.101'
EOF
chmod +x "$fake_path/mdp"
: > "$LOG_FILE"
PATH="$fake_path:$TEST_PATH" "$ROOT/scripts/install.sh" --cli -y --base-url "$BASE_URL" >"$TMP_DIR/repeat.stdout"
test ! -s "$LOG_FILE"
grep -F 'cli: unchanged' "$TMP_DIR/repeat.stdout" >/dev/null
: > "$LOG_FILE"
PATH="$fake_path:$TEST_PATH" "$ROOT/scripts/install.sh" --cli --force-cli -y --base-url "$BASE_URL" >"$TMP_DIR/force.stdout"
assert_log "cli args: skip:0"
grep -F 'cli: updated' "$TMP_DIR/force.stdout" >/dev/null

# The standalone CLI bootstrap keeps skip-only and exact-version no-op behavior,
# but an explicit force repair must override the aggregate install's skip flag.
bootstrap_asset="$TMP_DIR/bootstrap-mdp"
cat > "$bootstrap_asset" <<'EOF'
#!/usr/bin/env bash
echo 'mdp 0.1.101'
EOF
chmod +x "$bootstrap_asset"
for scenario in skip-only same-version; do
  scenario_dir="$TMP_DIR/bootstrap-$scenario"
  scenario_skip=0
  if [ "$scenario" = skip-only ]; then scenario_skip=1; fi
  MDP_RESOLVED_VERSION=0.1.101 \
  MDP_DOWNLOAD_URL="file://$bootstrap_asset" \
  MDP_INSTALL_DIR="$scenario_dir" \
  MDP_SKIP_CLI_UPDATE="$scenario_skip" \
  MDP_FORCE_CLI_UPDATE=0 \
  PATH="$fake_path:$TEST_PATH" \
    "$ROOT/scripts/bootstrap-runtime.sh" >"$TMP_DIR/bootstrap-$scenario.stdout"
  test ! -e "$scenario_dir/mdp"
done
forced_bootstrap_dir="$TMP_DIR/bootstrap-forced"
MDP_RESOLVED_VERSION=0.1.101 \
MDP_DOWNLOAD_URL="file://$bootstrap_asset" \
MDP_INSTALL_DIR="$forced_bootstrap_dir" \
MDP_SKIP_CLI_UPDATE=1 \
MDP_FORCE_CLI_UPDATE=1 \
PATH="$fake_path:$TEST_PATH" \
  "$ROOT/scripts/bootstrap-runtime.sh" >"$TMP_DIR/bootstrap-forced.stdout"
test -x "$forced_bootstrap_dir/mdp"
grep -F "Installed mdp CLI to $forced_bootstrap_dir/mdp" "$TMP_DIR/bootstrap-forced.stdout" >/dev/null

# Explicit target failure stays strict and produces a truthful terminal summary.
: > "$LOG_FILE"
if PATH="$TEST_PATH" MDP_TEST_FAIL_TARGET=codex "$ROOT/scripts/install.sh" --codex -y --base-url "$BASE_URL" >"$TMP_DIR/failure.stdout" 2>"$TMP_DIR/failure.stderr"; then
  echo "Explicit native failure unexpectedly returned success." >&2
  exit 1
fi
grep -F 'codex: failed' "$TMP_DIR/failure.stdout" >/dev/null

echo "Installer fixture tests passed."
