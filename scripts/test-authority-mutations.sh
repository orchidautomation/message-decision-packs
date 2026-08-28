#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXPECTED_VERSION="27.1.0"
MAX_CANDIDATES=24
BUILD_TIMEOUT_SECONDS=120
TEST_TIMEOUT_SECONDS=180
SELECTOR='(from_run|permits_projection)'
MUTATION_FILE='src/authority/mod.rs'

usage() {
  cat <<'USAGE'
Kill focused authority mutants using cargo-mutants.

Usage:
  scripts/test-authority-mutations.sh [SHARD]
  scripts/test-authority-mutations.sh --list [SHARD]
  scripts/test-authority-mutations.sh --help

Supported shard topology: 0/4, 1/4, 2/4, 3/4.

Environment:
  MDP_AUTHORITY_MUTATIONS_LIST_ONLY  When set to 1, only print the candidate
                                     list (full or for the requested shard)
                                     and exit 0. Used by workflow contracts.
USAGE
}

list_only="${MDP_AUTHORITY_MUTATIONS_LIST_ONLY:-0}"
positional=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --help|-h)
      usage
      exit 0
      ;;
    --list)
      list_only=1
      shift
      ;;
    --list=*)
      list_only=1
      shift
      ;;
    --)
      shift
      while [ "$#" -gt 0 ]; do
        positional+=("$1")
        shift
      done
      ;;
    -*)
      echo "unsupported flag: $1" >&2
      usage >&2
      exit 1
      ;;
    *)
      positional+=("$1")
      shift
      ;;
  esac
done
set -- "${positional[@]:+${positional[@]}}"

shard="${1:-}"
case "$shard" in
  "") shard_args=() ;;
  0/4|1/4|2/4|3/4) shard_args=(--shard "$shard") ;;
  *)
    echo "unsupported authority mutation shard: ${shard:-<none>} (expected 0/4, 1/4, 2/4, or 3/4)" >&2
    exit 1
    ;;
esac

if ! command -v cargo-mutants >/dev/null 2>&1; then
  echo "cargo-mutants ${EXPECTED_VERSION} is required; install with: cargo install cargo-mutants --version ${EXPECTED_VERSION} --locked" >&2
  exit 1
fi
actual_version="$(cargo mutants --version 2>/dev/null | awk '{print $2}')"
if [ "$actual_version" != "$EXPECTED_VERSION" ]; then
  echo "cargo-mutants version mismatch: expected ${EXPECTED_VERSION}, got ${actual_version:-unknown}" >&2
  exit 1
fi

cd "$ROOT/cli"
selector="$SELECTOR"
list_file="$(mktemp)"
trap 'rm -f "$list_file"' EXIT
cargo mutants --list --file "$MUTATION_FILE" --re "$selector" "${shard_args[@]}" >"$list_file"
candidate_count="$(grep -cve '^[[:space:]]*$' "$list_file" || true)"
if [ "$candidate_count" -eq 0 ]; then
  echo "authority mutation selector produced no candidates" >&2
  exit 1
fi
if [ "$candidate_count" -gt "$MAX_CANDIDATES" ]; then
  echo "authority mutation selector produced ${candidate_count} candidates; cap is ${MAX_CANDIDATES}" >&2
  exit 1
fi

if [ "$list_only" = "1" ]; then
  cat "$list_file"
  exit 0
fi

printf 'authority mutation candidates=%s version=%s shard=%s\n' \
  "$candidate_count" "$actual_version" "${shard:-all}"
# The CLI embeds repository-level plugin and script assets with include_str!, so
# cargo-mutants' crate-only scratch copy cannot compile the unmutated baseline.
# CI runs each shard in its own disposable checkout, making in-place mode safe
# while preserving parallel coverage across the isolated jobs.
cargo mutants \
  --file "$MUTATION_FILE" \
  --re "$selector" \
  --build-timeout "$BUILD_TIMEOUT_SECONDS" \
  --timeout "$TEST_TIMEOUT_SECONDS" \
  --in-place \
  "${shard_args[@]}"
