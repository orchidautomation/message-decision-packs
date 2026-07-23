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
plugin_root="\$HOME/.codex/plugins/message-decision-packs"
mkdir -p "\$MDP_INSTALL_DIR" "\$HOME/.codex/plugins"
cp "$ROOT/cli/target/debug/mdp" "\$MDP_INSTALL_DIR/mdp"
chmod +x "\$MDP_INSTALL_DIR/mdp"
rm -rf "\$plugin_root"
mkdir -p "\$plugin_root"
cp -R "$ROOT/scripts" "\$plugin_root/scripts"
cp -R "$ROOT/plugin/skills" "\$plugin_root/skills"
SH
chmod +x "$fake_installer"

MDP_RELEASE_INSTALLER="$fake_installer" "$ROOT/scripts/release-install-smoke.sh" 0.0.0-local

echo "Release install smoke fixture passed."
