#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Install Message Decision Packs release assets.

Usage:
  bash <(curl -fsSL https://mdp.orchidlabs.dev/install.sh) --agents -y

Options:
  --agents              Install agent plugin bundles for Claude Code, Cursor, Codex, and OpenCode.
  --claude-code         Install only the Claude Code plugin bundle.
  --cursor              Install only the Cursor plugin bundle.
  --codex               Install only the Codex plugin bundle.
  --opencode            Install only the OpenCode plugin bundle.
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
    --agents)
      agents=1
      shift
      ;;
    --claude-code|--cursor|--codex|--opencode)
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
  targets=(all)
elif [ "${#targets[@]}" -eq 0 ]; then
  targets=(codex)
fi

if [ "$yes" = "1" ]; then
  export PLUXX_CODEX_ENABLE_PLUGIN_HOOKS="${PLUXX_CODEX_ENABLE_PLUGIN_HOOKS:-1}"
fi

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

  if [ "$yes" = "1" ]; then
    installer_args+=(--yes)
  fi

  echo "Installing Message Decision Packs for $target..."
  curl -fsSL "$url" -o "$installer"
  bash "$installer" "${installer_args[@]}"
}

for target in "${targets[@]}"; do
  run_installer "$target"
done

echo "Message Decision Packs install complete."
