#!/usr/bin/env bash
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

mdp_bin="$ROOT/cli/target/debug/mdp"
if [ ! -x "$mdp_bin" ]; then
  cargo build --manifest-path "$ROOT/cli/Cargo.toml" >/dev/null
fi

fake_installer="$TMP_DIR/install.sh"
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
if [ "\$PLUXX_INSTALL_LOCK_ROOT" != "\$expected_home/.pluxx/install-locks" ]; then
  echo "release smoke did not isolate PLUXX_INSTALL_LOCK_ROOT: \$PLUXX_INSTALL_LOCK_ROOT" >&2
  exit 1
fi
if [ "\$PLUXX_RUNTIME_STORE_ROOT" != "\$expected_home/.pluxx/runtimes" ]; then
  echo "release smoke did not isolate PLUXX_RUNTIME_STORE_ROOT: \$PLUXX_RUNTIME_STORE_ROOT" >&2
  exit 1
fi
plugin_root="\$PLUXX_CODEX_INSTALL_DIR"
mkdir -p "\$MDP_INSTALL_DIR" "\$(dirname "\$PLUXX_CODEX_CONFIG_PATH")" "\$(dirname "\$plugin_root")"
cp "$ROOT/cli/target/debug/mdp" "\$MDP_INSTALL_DIR/mdp"
chmod +x "\$MDP_INSTALL_DIR/mdp"
printf '[features]\\nhooks = true\\n' > "\$PLUXX_CODEX_CONFIG_PATH"
rm -rf "\$plugin_root"
mkdir -p "\$plugin_root"
cp -R "$ROOT/scripts" "\$plugin_root/scripts"
cp -R "$ROOT/plugin/skills" "\$plugin_root/skills"
SH
chmod +x "$fake_installer"

install_home="$TMP_DIR/install-home"
MDP_RELEASE_INSTALLER="$fake_installer" \
MDP_RELEASE_INSTALL_HOME="$install_home" \
EXPECTED_INSTALL_HOME="$install_home" \
CODEX_HOME="$TMP_DIR/poison-codex-home" \
PLUXX_CODEX_CONFIG_PATH="$TMP_DIR/poison-codex-config.toml" \
PLUXX_CODEX_INSTALL_DIR="$TMP_DIR/poison-codex-plugin" \
PLUXX_INSTALL_LOCK_ROOT="$TMP_DIR/poison-locks" \
PLUXX_RUNTIME_STORE_ROOT="$TMP_DIR/poison-runtimes" \
  "$ROOT/scripts/release-install-smoke.sh" 0.0.0-local

echo "Release install smoke fixture passed."
