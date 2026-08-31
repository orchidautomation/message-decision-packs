#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXPECTED_VERSION="27.1.0"
MAX_CANDIDATES=24
MAX_SMOKE_CANDIDATES=8
BUILD_TIMEOUT_SECONDS=120
# The unmutated CLI suite now runs close to three minutes on hosted runners.
# Keep enough headroom for normal runner variance so the mutation gate tests
# authority changes rather than intermittently timing out its baseline.
TEST_TIMEOUT_SECONDS=240
SELECTOR='(from_run|permits_projection)'
SMOKE_SELECTORS=(
  'replace SourceAuthority::from_run -> Self with Default::default\(\)'
  'replace match guard decision_blocked with false in SourceAuthority::from_run'
  'replace SourceAuthority::permits_projection -> bool with false'
  'replace > with == in SourceAuthority::permits_projection'
)
MUTATION_FILE='src/authority/mod.rs'

usage() {
  cat <<'USAGE'
Kill focused authority mutants using cargo-mutants.

Usage:
  scripts/test-authority-mutations.sh [SHARD]
  scripts/test-authority-mutations.sh --list [SHARD]
  scripts/test-authority-mutations.sh --smoke [--list]
  scripts/test-authority-mutations.sh --help

Supported shard topology: 0/4, 1/4, 2/4, 3/4.
Smoke mode selects the exact from_run and permits_projection mutation sets and
does not support sharding.

Environment:
  MDP_AUTHORITY_MUTATIONS_LIST_ONLY  When set to 1, only print the candidate
                                     list (full or for the requested shard)
                                     and exit 0. Used by workflow contracts.
USAGE
}

list_only="${MDP_AUTHORITY_MUTATIONS_LIST_ONLY:-0}"
smoke=0
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
    --smoke)
      smoke=1
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
if [ "$smoke" = "1" ] && [ -n "$shard" ]; then
  echo "--smoke does not support sharding" >&2
  exit 1
fi
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
list_file="$(mktemp)"
complete_list_file="$(mktemp)"
trap 'rm -f "$list_file" "$complete_list_file"' EXIT

if [ "$smoke" = "1" ]; then
  : >"$list_file"
  for smoke_selector in "${SMOKE_SELECTORS[@]}"; do
    selector="$smoke_selector"
    selector_file="$(mktemp)"
    cargo mutants --list --file "$MUTATION_FILE" --re "$selector" >"$selector_file"
    selector_count="$(grep -cve '^[[:space:]]*$' "$selector_file" || true)"
    if [ "$selector_count" -ne 1 ]; then
      echo "smoke selector produced ${selector_count} candidates, expected exactly one: ${smoke_selector}" >&2
      rm -f "$selector_file"
      exit 1
    fi
    cat "$selector_file" >>"$list_file"
    rm -f "$selector_file"
  done
  duplicate_count="$(awk 'NF { seen[$0]++ } END { for (candidate in seen) if (seen[candidate] > 1) count++ ; print count + 0 }' "$list_file")"
  if [ "$duplicate_count" -ne 0 ]; then
    echo "smoke selectors produced duplicate candidates" >&2
    exit 1
  fi
  cargo mutants --list --file "$MUTATION_FILE" --re "$SELECTOR" >"$complete_list_file"
  while IFS= read -r candidate; do
    [ -z "${candidate//[[:space:]]/}" ] && continue
    if ! grep -F -x -q -- "$candidate" "$complete_list_file"; then
      echo "smoke candidate is outside the complete candidate set: $candidate" >&2
      exit 1
    fi
  done <"$list_file"
else
  selector="$SELECTOR"
  cargo mutants --list --file "$MUTATION_FILE" --re "$selector" "${shard_args[@]}" >"$list_file"
fi
candidate_count="$(grep -cve '^[[:space:]]*$' "$list_file" || true)"
if [ "$candidate_count" -eq 0 ]; then
  echo "authority mutation selector produced no candidates" >&2
  exit 1
fi
if [ "$smoke" = "1" ] && [ "$candidate_count" -gt "$MAX_SMOKE_CANDIDATES" ]; then
  echo "smoke mutation selectors produced ${candidate_count} candidates; cap is ${MAX_SMOKE_CANDIDATES}" >&2
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
  "$candidate_count" "$actual_version" "${shard:-$([ "$smoke" = "1" ] && echo smoke || echo all)}"
# The CLI embeds repository-level plugin and script assets with include_str!, so
# cargo-mutants' crate-only scratch copy cannot compile the unmutated baseline.
# CI runs each shard in its own disposable checkout, making in-place mode safe
# while preserving parallel coverage across the isolated jobs.
if [ "$smoke" = "1" ]; then
  for smoke_selector in "${SMOKE_SELECTORS[@]}"; do
    cargo mutants \
      --file "$MUTATION_FILE" \
      --re "$smoke_selector" \
      --build-timeout "$BUILD_TIMEOUT_SECONDS" \
      --timeout "$TEST_TIMEOUT_SECONDS" \
      --in-place
  done
else
  cargo mutants \
    --file "$MUTATION_FILE" \
    --re "$selector" \
    --build-timeout "$BUILD_TIMEOUT_SECONDS" \
    --timeout "$TEST_TIMEOUT_SECONDS" \
    --in-place \
    "${shard_args[@]}"
fi
