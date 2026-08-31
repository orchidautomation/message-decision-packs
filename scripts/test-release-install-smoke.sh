#!/usr/bin/env bash
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
find "$ROOT/scripts" -type d -name __pycache__ -prune -exec rm -rf {} +

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

mdp_bin="$ROOT/cli/target/debug/mdp"
if [ ! -x "$mdp_bin" ]; then
  cargo build --manifest-path "$ROOT/cli/Cargo.toml" >/dev/null
fi

fake_installer="$TMP_DIR/install.sh"
portable_fixture_source="$TMP_DIR/portable-source"
mkdir -p "$portable_fixture_source"
cp -R "$ROOT/plugin/skills" "$portable_fixture_source/skills"
cat > "$portable_fixture_source/plugin.json" <<'JSON'
{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "message-decision-packs",
  "version": "0.1.101",
  "description": "Release smoke fixture.",
  "author": { "name": "Orchid Labs" },
  "license": "Elastic-2.0"
}
JSON
cat > "$fake_installer" <<SH
#!/usr/bin/env bash
set -euo pipefail
expected_home="\${EXPECTED_INSTALL_HOME:?}"
if [ "\$HOME" != "\$expected_home" ]; then
  echo "release smoke did not isolate HOME: \$HOME" >&2
  exit 1
fi
if [ "\$CODEX_HOME" != "\$expected_home/.codex" ]; then
  echo "release smoke did not isolate CODEX_HOME: \$CODEX_HOME" >&2
  exit 1
fi
if [ "\$PLUXX_CODEX_CONFIG_PATH" != "\$expected_home/.codex/config.toml" ]; then
  echo "release smoke did not isolate PLUXX_CODEX_CONFIG_PATH: \$PLUXX_CODEX_CONFIG_PATH" >&2
  exit 1
fi
if [ "\$PLUXX_CODEX_INSTALL_DIR" != "\$expected_home/.codex/plugins/message-decision-packs" ]; then
  echo "release smoke did not isolate PLUXX_CODEX_INSTALL_DIR: \$PLUXX_CODEX_INSTALL_DIR" >&2
  exit 1
fi
if [ "\$PLUXX_CLAUDE_MARKETPLACE_DIR" != "\$expected_home/.claude/plugins/data/message-decision-packs-releases" ]; then
  echo "release smoke did not isolate PLUXX_CLAUDE_MARKETPLACE_DIR: \$PLUXX_CLAUDE_MARKETPLACE_DIR" >&2
  exit 1
fi
if ! command -v claude >/dev/null 2>&1; then
  echo "release smoke did not provide the isolated Claude CLI prerequisite" >&2
  exit 1
fi
if [ "\$PLUXX_INSTALL_LOCK_ROOT" != "\$expected_home/.pluxx/install-locks" ]; then
  echo "release smoke did not isolate PLUXX_INSTALL_LOCK_ROOT: \$PLUXX_INSTALL_LOCK_ROOT" >&2
  exit 1
fi
if [ "\$PLUXX_RUNTIME_STORE_ROOT" != "\$expected_home/.pluxx/runtimes" ]; then
  echo "release smoke did not isolate PLUXX_RUNTIME_STORE_ROOT: \$PLUXX_RUNTIME_STORE_ROOT" >&2
  exit 1
fi
if [ "\$MDP_AGENT_PLUGINS_INSTALL_DIR" != "\$expected_home/compatible-client-fixtures/cursor/message-decision-packs" ]; then
  echo "release smoke did not isolate the portable compatible-client fixture root" >&2
  exit 1
fi
plugin_root="\$PLUXX_CODEX_INSTALL_DIR"
mkdir -p "\$MDP_INSTALL_DIR" "\$(dirname "\$PLUXX_CODEX_CONFIG_PATH")" "\$(dirname "\$plugin_root")"
cp "$ROOT/cli/target/debug/mdp" "\$MDP_INSTALL_DIR/mdp"
chmod +x "\$MDP_INSTALL_DIR/mdp"
printf '[features]\\nhooks = true\\n' > "\$PLUXX_CODEX_CONFIG_PATH"
claude_root="\$PLUXX_CLAUDE_MARKETPLACE_DIR/plugins/message-decision-packs"
for plugin_root in \
  "\$claude_root" \
  "\$PLUXX_CODEX_INSTALL_DIR" \
  "\$PLUXX_CURSOR_INSTALL_DIR" \
  "\$PLUXX_OPENCODE_INSTALL_DIR"; do
  rm -rf "\$plugin_root"
  mkdir -p "\$plugin_root/assets"
  cp -R "$ROOT/scripts" "\$plugin_root/scripts"
  cp -R "$ROOT/plugin/skills" "\$plugin_root/skills"
  cp -R "$ROOT/plugin/skill-evals" "\$plugin_root/skill-evals"
  rm -rf "\$plugin_root/assets"
  cp -R "$ROOT/plugin/assets" "\$plugin_root/assets"
done
portable_root="\${MDP_AGENT_PLUGINS_INSTALL_DIR:?}"
rm -rf "\$portable_root"
mkdir -p "\$(dirname "\$portable_root")"
cp -R "$portable_fixture_source" "\$portable_root"
SH
chmod +x "$fake_installer"
fake_codex="$TMP_DIR/codex"
cat > "$fake_codex" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
if [ "${1:-}" = "plugin" ] && [ "${2:-}" = "list" ] && [ "${3:-}" = "--json" ]; then
  cat <<'JSON'
{"installed":[{"pluginId":"message-decision-packs@message-decision-packs-local","installed":true,"enabled":true,"version":"0.0.0-local","source":{"source":"local","path":"REPLACE_CODEX_PLUGIN_ROOT"}}]}
JSON
  exit 0
fi
echo "unexpected fake Codex invocation" >&2
exit 1
EOF
chmod +x "$fake_codex"
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) staged_target="x86_64-unknown-linux-gnu" ;;
  Darwin-x86_64) staged_target="x86_64-apple-darwin" ;;
  Darwin-arm64) staged_target="aarch64-apple-darwin" ;;
  *) echo "unsupported release smoke fixture platform" >&2; exit 1 ;;
esac
staged_name="mdp-$staged_target"
cp "$mdp_bin" "$TMP_DIR/$staged_name"
staged_sha="$(shasum -a 256 "$TMP_DIR/$staged_name" | awk '{print $1}')"
node - "$ROOT" "$portable_fixture_source" "$TMP_DIR/release-manifest.json" "$staged_name" "$staged_sha" <<'NODE'
const { createHash } = require('node:crypto')
const { lstatSync, readFileSync, readdirSync, writeFileSync } = require('node:fs')
const { join, relative } = require('node:path')
const [root, portableRoot, output, stagedName, stagedSha] = process.argv.slice(2)
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const records = []
const trees = [
  ['scripts', join(root, 'scripts')],
  ['skills', join(root, 'plugin/skills')],
  ['assets', join(root, 'plugin/assets')],
  ['skill-evals', join(root, 'plugin/skill-evals')],
]
const walk = (prefix, treeRoot, directory) => {
  for (const name of readdirSync(directory).sort()) {
    const path = join(directory, name)
    const stats = lstatSync(path)
    if (stats.isDirectory()) walk(prefix, treeRoot, path)
    else if (stats.isFile()) records.push({
      path: `${prefix}/${relative(treeRoot, path).split('\\').join('/')}`,
      executable: (stats.mode & 0o111) !== 0,
      sha256: sha256(readFileSync(path)),
    })
  }
}
for (const [prefix, treeRoot] of trees) walk(prefix, treeRoot, treeRoot)
records.sort((left, right) => left.path.localeCompare(right.path))
const pluginTrees = Object.fromEntries(
  ['claude-code', 'codex', 'cursor', 'opencode'].map((platform) => [platform, { files: records }]),
)
const portableRecords = []
const walkPortable = (directory) => {
  for (const name of readdirSync(directory).sort()) {
    const path = join(directory, name)
    const stats = lstatSync(path)
    if (stats.isDirectory()) walkPortable(path)
    else if (stats.isFile()) portableRecords.push({
      path: relative(portableRoot, path).split('\\').join('/'),
      executable: (stats.mode & 0o111) !== 0,
      sha256: sha256(readFileSync(path)),
    })
  }
}
walkPortable(portableRoot)
writeFileSync(output, `${JSON.stringify({
  plugin: { version: '0.1.101', license: 'Elastic-2.0' },
  cli_artifacts: [{ name: stagedName, sha256: stagedSha }],
  plugin_trees: pluginTrees,
  portable_packages: { 'agent-plugins': {
    contract: 'mdp.agent-plugins-portable-package.v1',
    specification: '1.0.0',
    skills: ['mdp', 'mdp-gtm-brief', 'mdp-pack-builder', 'mdp-pack-review', 'mdp-proposal-review'],
    mcp_servers: [],
    files: portableRecords,
    sha256: sha256(Buffer.from(`${JSON.stringify(portableRecords)}\n`)),
  } },
})}\n`)
NODE

install_home="$TMP_DIR/install-home"
sed -i "s|REPLACE_CODEX_PLUGIN_ROOT|$install_home/.codex/plugins/message-decision-packs|" "$fake_codex"
MDP_RELEASE_REQUIRE_STAGED_PARITY=1 \
MDP_RELEASE_SOURCE_PARITY_BIN="$TMP_DIR/$staged_name" \
MDP_RELEASE_INSTALLER="$fake_installer" \
MDP_RELEASE_INSTALL_HOME="$install_home" \
MDP_CODEX_BIN="$fake_codex" \
EXPECTED_INSTALL_HOME="$install_home" \
CODEX_HOME="$TMP_DIR/poison-codex-home" \
PLUXX_CODEX_CONFIG_PATH="$TMP_DIR/poison-codex-config.toml" \
PLUXX_CODEX_INSTALL_DIR="$TMP_DIR/poison-codex-plugin" \
PLUXX_INSTALL_LOCK_ROOT="$TMP_DIR/poison-locks" \
PLUXX_RUNTIME_STORE_ROOT="$TMP_DIR/poison-runtimes" \
  "$ROOT/scripts/release-install-smoke.sh" 0.0.0-local

echo "Release install smoke fixture passed."
