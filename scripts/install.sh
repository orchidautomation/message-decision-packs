#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Install Message Decision Packs release assets.

Usage:
  bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y

Options:
  --cli, --cli-only     Install only the mdp CLI.
  --agents              Install the mdp CLI plus supported agent plugin bundles.
  --claude-code         Install only the Claude Code plugin bundle.
  --cursor              Install only the Cursor plugin bundle.
  --codex               Install only the Codex plugin bundle.
  --opencode            Install only the OpenCode plugin bundle.
  --agent-plugins       Install only the strict Agent Plugins portable core.
  -y, --yes             Noninteractive mode where supported by downstream installers.
  --repo OWNER/REPO     Override the GitHub repository.
  --version VERSION     Install a specific release version or tag.
  --base-url URL        Override the release asset base URL.
  -h, --help            Show this help.

Environment:
  MDP_GITHUB_REPO       Default repository. Defaults to orchidautomation/message-decision-packs.
  MDP_VERSION           Release version or tag. Defaults to latest.
  MDP_RELEASE_BASE_URL  Release asset base URL override.
  MDP_INSTALL_DIR       Directory where the mdp CLI should be installed by plugin bootstrap.
  MDP_SKIP_CLI_UPDATE   Set automatically after --agents installs the CLI once.
  MDP_AGENT_PLUGINS_INSTALL_DIR
                        Explicit client-managed import directory for the portable
                        core. Required for --agent-plugins. When set, --agents
                        installs it in addition to the four native bundles.
EOF
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

repo="${MDP_GITHUB_REPO:-orchidautomation/message-decision-packs}"
version="${MDP_VERSION:-latest}"
base_url="${MDP_RELEASE_BASE_URL:-}"
yes=0
agents=0
targets=()

while [ "$#" -gt 0 ]; do
  case "$1" in
    --cli|--cli-only)
      targets+=(cli)
      shift
      ;;
    --agents)
      agents=1
      shift
      ;;
    --claude-code|--cursor|--codex|--opencode|--agent-plugins)
      targets+=("${1#--}")
      shift
      ;;
    -y|--yes)
      yes=1
      shift
      ;;
    --repo)
      repo="$2"
      shift 2
      ;;
    --repo=*)
      repo="${1#*=}"
      shift
      ;;
    --version)
      version="$2"
      shift 2
      ;;
    --version=*)
      version="${1#*=}"
      shift
      ;;
    --base-url)
      base_url="$2"
      shift 2
      ;;
    --base-url=*)
      base_url="${1#*=}"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

need_cmd curl
need_cmd mktemp
need_cmd bash

if [ -z "$base_url" ]; then
  if [ "$version" = "latest" ]; then
    base_url="https://github.com/$repo/releases/latest/download"
  else
    tag="$version"
    case "$tag" in
      v*) ;;
      *) tag="v$tag" ;;
    esac
    base_url="https://github.com/$repo/releases/download/$tag"
  fi
fi

if [ "$agents" = "1" ]; then
  targets=(cli claude-code cursor codex opencode)
  if [ -n "${MDP_AGENT_PLUGINS_INSTALL_DIR:-}" ]; then
    targets+=(agent-plugins)
  else
    echo "Portable Agent Plugins import not auto-routed: set MDP_AGENT_PLUGINS_INSTALL_DIR to an explicit compatible-client path." >&2
    echo "Native Claude Code, Cursor, Codex, and OpenCode installation will continue unchanged." >&2
  fi
elif [ "${#targets[@]}" -eq 0 ]; then
  targets=(codex)
fi

if [ "$yes" = "1" ]; then
  export PLUXX_CODEX_ENABLE_PLUGIN_HOOKS="${PLUXX_CODEX_ENABLE_PLUGIN_HOOKS:-1}"
fi
export MDP_VERSION="$version"

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

run_installer() {
  local target="$1"
  local installer="$tmp_dir/install-$target.sh"
  local url="$base_url/install-$target.sh"
  local installer_args=()

  if [ "$agents" = "1" ] && [ "$target" = "claude-code" ] && ! command -v claude >/dev/null 2>&1; then
    echo "Skipping Claude Code bundle because the claude CLI is not available on PATH." >&2
    echo "Run with --claude-code to require Claude Code installation and fail if prerequisites are missing." >&2
    return 0
  fi

  if [ "$agents" = "1" ] && [ "$target" = "codex" ] && ! command -v codex >/dev/null 2>&1; then
    echo "Skipping Codex bundle because the codex CLI is not available on PATH." >&2
    echo "Run with --codex to require Codex installation and fail if prerequisites are missing." >&2
    return 0
  fi

  if [ "$yes" = "1" ]; then
    installer_args+=(--yes)
  fi

  echo "Installing Message Decision Packs for $target..."
  curl -fsSL "$url" -o "$installer"
  bash "$installer" "${installer_args[@]}"

  if [ "$target" = "cli" ]; then
    export MDP_SKIP_CLI_UPDATE=1
  fi
}

install_agent_plugins() {
  local install_dir="${MDP_AGENT_PLUGINS_INSTALL_DIR:-}"
  local archive_name="message-decision-packs-agent-plugins-latest.tar.gz"
  local archive="$tmp_dir/$archive_name"
  local manifest="$tmp_dir/release-manifest.json"
  local checksums="$tmp_dir/SHA256SUMS.txt"
  local extracted="$tmp_dir/portable-extracted"
  local source_root="$extracted/agent-plugins"
  local expected actual

  if [ -z "$install_dir" ]; then
    echo "MDP_AGENT_PLUGINS_INSTALL_DIR is required for --agent-plugins." >&2
    echo "The portable archive is client-managed; MDP will not guess an undocumented Codex path." >&2
    echo 'For a documented Cursor local import, set it explicitly under $HOME/.cursor/plugins/local/.' >&2
    return 1
  fi
  case "$install_dir" in
    /*) ;;
    *) echo "MDP_AGENT_PLUGINS_INSTALL_DIR must be an absolute path: $install_dir" >&2; return 1 ;;
  esac
  if [ "$install_dir" = "/" ]; then
    echo "Refusing to install the portable package at filesystem root." >&2
    return 1
  fi

  need_cmd tar
  need_cmd node
  curl -fsSL "$base_url/$archive_name" -o "$archive"
  curl -fsSL "$base_url/release-manifest.json" -o "$manifest"
  curl -fsSL "$base_url/SHA256SUMS.txt" -o "$checksums"
  expected="$(awk -v name="$archive_name" '$2 == name { print $1 }' "$checksums")"
  if [ -z "$expected" ] || [ "$(printf '%s' "$expected" | wc -c | tr -d ' ')" -ne 64 ]; then
    echo "Release checksum inventory is missing a valid digest for $archive_name." >&2
    return 1
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$archive" | awk '{print $1}')"
  elif command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "$archive" | awk '{print $1}')"
  else
    echo "Missing required checksum command: sha256sum or shasum" >&2
    return 1
  fi
  if [ "$actual" != "$expected" ]; then
    echo "Portable archive checksum mismatch for $archive_name." >&2
    return 1
  fi

  mkdir -p "$extracted"
  if ! tar -tzf "$archive" | node -e '
    const entries = require("fs").readFileSync(0, "utf8").split(/\r?\n/).filter(Boolean)
    if (entries.length === 0 || entries.some((entry) =>
      entry.startsWith("/") ||
      !entry.startsWith("agent-plugins/") ||
      entry.split("/").some((part) => part === "..")
    )) process.exit(1)
  '; then
    echo "Portable archive contains an unsafe or unexpected path." >&2
    return 1
  fi
  tar -xzf "$archive" -C "$extracted"
  node - "$source_root" "$manifest" "$archive_name" <<'NODE'
const { createHash } = require('crypto')
const { existsSync, lstatSync, readFileSync, readdirSync } = require('fs')
const { join, relative } = require('path')
const [root, manifestPath, archiveName] = process.argv.slice(2)
const fail = (message) => { console.error(message); process.exit(1) }
if (!existsSync(root) || lstatSync(root).isSymbolicLink() || !lstatSync(root).isDirectory()) {
  fail('Portable archive is missing the agent-plugins package root.')
}
const top = readdirSync(root).sort()
if (JSON.stringify(top) !== JSON.stringify(['plugin.json', 'skills'])) {
  fail(`Portable package contains native-only or unexpected top-level entries: ${top.join(', ')}`)
}
const plugin = JSON.parse(readFileSync(join(root, 'plugin.json'), 'utf8'))
if (
  plugin.$schema !== 'https://agent-plugins.org/schemas/1.0.0/plugin.schema.json' ||
  plugin.name !== 'message-decision-packs'
) fail('Portable plugin.json does not match the MDP Agent Plugins contract.')
const expectedSkills = ['mdp', 'mdp-gtm-brief', 'mdp-pack-builder', 'mdp-pack-review', 'mdp-proposal-review'].sort()
const skillsRoot = join(root, 'skills')
const skills = readdirSync(skillsRoot).filter((name) => lstatSync(join(skillsRoot, name)).isDirectory()).sort()
if (JSON.stringify(skills) !== JSON.stringify(expectedSkills)) {
  fail(`Portable package does not contain exactly the five supported MDP skills: ${skills.join(', ')}`)
}
for (const skill of skills) {
  if (!existsSync(join(skillsRoot, skill, 'SKILL.md'))) fail(`Portable package is missing skills/${skill}/SKILL.md.`)
}
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const records = []
const walk = (directory) => {
  for (const name of readdirSync(directory).sort()) {
    const path = join(directory, name)
    const stats = lstatSync(path)
    if (stats.isSymbolicLink()) fail(`Portable package contains a symbolic link: ${relative(root, path)}`)
    if (stats.isDirectory()) walk(path)
    else if (stats.isFile()) records.push({
      path: relative(root, path).split('\\').join('/'),
      executable: (stats.mode & 0o111) !== 0,
      sha256: sha256(readFileSync(path)),
    })
    else fail(`Portable package contains a non-regular entry: ${relative(root, path)}`)
  }
}
walk(root)
const treeSha = sha256(Buffer.from(`${JSON.stringify(records)}\n`))
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
const archive = manifest?.assets?.archives?.find((entry) => entry.platform === 'agent-plugins')
const portable = manifest?.portable_packages?.['agent-plugins']
if (plugin.version !== manifest?.plugin?.version) fail('Portable plugin version differs from the release manifest.')
if (archive?.latestAsset !== archiveName) fail('Release manifest does not bind the requested Agent Plugins archive.')
if (
  portable?.contract !== 'mdp.agent-plugins-portable-package.v1' ||
  portable?.specification !== '1.0.0' ||
  JSON.stringify(portable.skills) !== JSON.stringify(expectedSkills) ||
  JSON.stringify(portable.mcp_servers) !== '[]' ||
  portable.sha256 !== treeSha ||
  JSON.stringify(portable.files) !== JSON.stringify(records)
) fail('Release manifest portable-package contract does not match the extracted artifact.')
NODE

  mkdir -p "$(dirname "$install_dir")"
  local staged="$install_dir.mdp-portable-staging.$$"
  local backup="$install_dir.mdp-portable-backup.$$"
  rm -rf "$staged" "$backup"
  mv "$source_root" "$staged"
  if [ -e "$install_dir" ]; then
    mv "$install_dir" "$backup"
  fi
  if ! mv "$staged" "$install_dir"; then
    if [ -e "$backup" ]; then mv "$backup" "$install_dir"; fi
    return 1
  fi
  rm -rf "$backup"
  echo "Installed Agent Plugins portable core to $install_dir"
  echo "Reload the compatible client and record real-client discovery separately from native hook proof."
}

for target in "${targets[@]}"; do
  if [ "$target" = "agent-plugins" ]; then
    install_agent_plugins
  else
    run_installer "$target"
  fi
done

echo "Message Decision Packs install complete."
