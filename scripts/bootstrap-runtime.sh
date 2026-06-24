#!/usr/bin/env bash
set -euo pipefail

if command -v mdp >/dev/null 2>&1; then
  echo "mdp CLI already available: $(command -v mdp)"
  mdp --version || true
  exit 0
fi

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

need_cmd uname
need_cmd mktemp
need_cmd mkdir
need_cmd cp
need_cmd chmod
need_cmd curl
need_cmd find
need_cmd head

REPO="${MDP_GITHUB_REPO:-orchidautomation/message-decision-packs}"
VERSION="${MDP_VERSION:-latest}"
INSTALL_DIR="${MDP_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s):$(uname -m)" in
  Darwin:arm64)
    TARGET="aarch64-apple-darwin"
    ;;
  Darwin:x86_64)
    TARGET="x86_64-apple-darwin"
    ;;
  Linux:x86_64)
    TARGET="x86_64-unknown-linux-gnu"
    ;;
  Linux:aarch64|Linux:arm64)
    TARGET="aarch64-unknown-linux-gnu"
    ;;
  *)
    echo "Unsupported platform for automatic mdp install: $(uname -s) $(uname -m)" >&2
    echo "Install mdp manually from the Message Decision Packs release assets." >&2
    exit 1
    ;;
esac

ASSET="${MDP_ASSET:-mdp-$TARGET}"
TAG="$VERSION"
if [[ "$VERSION" != "latest" && "$VERSION" != v* ]]; then
  TAG="v$VERSION"
fi

if [[ -n "${MDP_DOWNLOAD_URL:-}" ]]; then
  RAW_URL="$MDP_DOWNLOAD_URL"
  ARCHIVE_URL=""
elif [[ "$VERSION" == "latest" ]]; then
  RAW_URL="https://github.com/$REPO/releases/latest/download/$ASSET"
  ARCHIVE_URL="https://github.com/$REPO/releases/latest/download/$ASSET.tar.gz"
else
  RAW_URL="https://github.com/$REPO/releases/download/$TAG/$ASSET"
  ARCHIVE_URL="https://github.com/$REPO/releases/download/$TAG/$ASSET.tar.gz"
fi

echo "Preparing mdp CLI for $TARGET from $REPO@$VERSION"

if [[ "${MDP_DRY_RUN:-0}" == "1" ]]; then
  echo "Dry run: would install $ASSET to $INSTALL_DIR/mdp"
  echo "Dry run: raw asset URL $RAW_URL"
  if [[ -n "$ARCHIVE_URL" ]]; then
    echo "Dry run: archive fallback URL $ARCHIVE_URL"
  fi
  exit 0
fi

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

download() {
  local url="$1"
  local output="$2"
  curl -fL "$url" -o "$output"
}

BINARY_PATH="$TMP_DIR/mdp"

if download "$RAW_URL" "$BINARY_PATH"; then
  chmod +x "$BINARY_PATH"
elif [[ -n "$ARCHIVE_URL" ]] && download "$ARCHIVE_URL" "$TMP_DIR/mdp.tar.gz"; then
  need_cmd tar
  tar -xzf "$TMP_DIR/mdp.tar.gz" -C "$TMP_DIR"
  if [[ -x "$TMP_DIR/mdp" ]]; then
    BINARY_PATH="$TMP_DIR/mdp"
  elif [[ -x "$TMP_DIR/$ASSET" ]]; then
    BINARY_PATH="$TMP_DIR/$ASSET"
  else
    FOUND="$(find "$TMP_DIR" -type f -name mdp -perm -111 | head -n 1 || true)"
    if [[ -z "$FOUND" ]]; then
      echo "Downloaded archive did not contain an executable mdp binary." >&2
      exit 1
    fi
    BINARY_PATH="$FOUND"
  fi
else
  echo "Could not download mdp release asset for $TARGET." >&2
  echo "Tried: $RAW_URL" >&2
  if [[ -n "$ARCHIVE_URL" ]]; then
    echo "Tried: $ARCHIVE_URL" >&2
  fi
  exit 1
fi

"$BINARY_PATH" --version >/dev/null

mkdir -p "$INSTALL_DIR"
cp "$BINARY_PATH" "$INSTALL_DIR/mdp"
chmod +x "$INSTALL_DIR/mdp"

echo "Installed mdp CLI to $INSTALL_DIR/mdp"

case ":$PATH:" in
  *":$INSTALL_DIR:"*)
    ;;
  *)
    echo "Add $INSTALL_DIR to PATH before using the MDP plugin." >&2
    ;;
esac
