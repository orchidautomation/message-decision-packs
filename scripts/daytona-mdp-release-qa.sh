#!/usr/bin/env bash
set -euo pipefail

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

need_cmd daytona

REPO="${MDP_GITHUB_REPO:-orchidautomation/message-decision-packs}"
VERSION="${MDP_VERSION:-latest}"
SANDBOX_NAME="${DAYTONA_SANDBOX_NAME:-mdp-release-qa-$(date +%Y%m%d%H%M%S)}"
TARGET="${DAYTONA_TARGET:-us}"
CLASS="${DAYTONA_CLASS:-small}"
AUTO_STOP="${DAYTONA_AUTO_STOP:-15}"
AUTO_DELETE="${DAYTONA_AUTO_DELETE:-60}"

if [[ -n "${MDP_INSTALL_URL:-}" ]]; then
  INSTALL_URL="$MDP_INSTALL_URL"
elif [[ "$VERSION" == "latest" ]]; then
  INSTALL_URL="https://github.com/$REPO/releases/latest/download/install.sh"
else
  TAG="$VERSION"
  if [[ "$TAG" != v* ]]; then
    TAG="v$TAG"
  fi
  INSTALL_URL="https://github.com/$REPO/releases/download/$TAG/install.sh"
fi

REMOTE_QA_SCRIPT="$(cat <<'EOF'
set -euo pipefail

export PATH="$HOME/.local/bin:$PATH"

if command -v mdp >/dev/null 2>&1; then
  echo "Expected a net-new machine without mdp on PATH, but found: $(command -v mdp)" >&2
  exit 1
fi

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command in Daytona sandbox: $1" >&2
    exit 1
  fi
}

need_cmd bash
need_cmd curl
need_cmd tar
need_cmd node

curl -fsSL "$MDP_INSTALL_URL" -o /tmp/install-mdp.sh
bash /tmp/install-mdp.sh --agents -y

command -v mdp
mdp --version

WORKDIR="$(mktemp -d)"
mdp --json init --template gtm --name "Daytona QA Pack" --dir "$WORKDIR" --force
mdp --json validate --dir "$WORKDIR"
mdp --json eval --dir "$WORKDIR"
mdp --json fit --dir "$WORKDIR" --prospect "$WORKDIR/examples/clay-row.json"
mdp --json brief --dir "$WORKDIR" --prospect "$WORKDIR/examples/clay-row.json" --channel linkedin
mdp --json check-claims --dir "$WORKDIR" --text "MDP is a local offline CLI for modular message context."

test -f "$HOME/.agents/plugins/marketplace.json"
test -d "$HOME/.codex/plugins/message-decision-packs"
test -f "$HOME/.codex/plugins/message-decision-packs/.codex-plugin/plugin.json"

echo "Daytona MDP release QA passed."
EOF
)"
REMOTE_QA_B64="$(printf '%s' "$REMOTE_QA_SCRIPT" | base64 | tr -d '\n')"

if [[ "${DAYTONA_DRY_RUN:-0}" == "1" ]]; then
  cat <<EOF
Dry run: would create Daytona sandbox:
  name: $SANDBOX_NAME
  target: $TARGET
  class: $CLASS
  auto-stop: $AUTO_STOP
  auto-delete: $AUTO_DELETE

Dry run: would install from:
  $INSTALL_URL

Dry run: would execute net-new-machine QA:
$REMOTE_QA_SCRIPT
EOF
  exit 0
fi

echo "Creating Daytona sandbox: $SANDBOX_NAME"
daytona create \
  --name "$SANDBOX_NAME" \
  --target "$TARGET" \
  --class "$CLASS" \
  --auto-stop "$AUTO_STOP" \
  --auto-delete "$AUTO_DELETE"

echo "Running MDP release install QA in Daytona sandbox: $SANDBOX_NAME"
REMOTE_RUNNER="set -euo pipefail; printf '%s' '$REMOTE_QA_B64' | base64 -d > /tmp/mdp-release-qa.sh; chmod +x /tmp/mdp-release-qa.sh; export MDP_INSTALL_URL='$INSTALL_URL'; /tmp/mdp-release-qa.sh"
daytona exec "$SANDBOX_NAME" --timeout 900 -- "bash -lc '$REMOTE_RUNNER'"

echo
echo "Sandbox kept for inspection until Daytona auto-stop/auto-delete policy applies: $SANDBOX_NAME"
