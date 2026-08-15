#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXPECTED_VERSION="27.1.0"
MAX_CANDIDATES=24

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
trap 'rm -f "$list_file"' EXIT
selector='(from_run|permits_projection)'
cargo mutants --list --file 'src/authority/mod.rs' --re "$selector" >"$list_file"
candidate_count="$(grep -cve '^[[:space:]]*$' "$list_file" || true)"
if [ "$candidate_count" -eq 0 ]; then
  echo "authority mutation selector produced no candidates" >&2
  exit 1
fi
if [ "$candidate_count" -gt "$MAX_CANDIDATES" ]; then
  echo "authority mutation selector produced ${candidate_count} candidates; cap is ${MAX_CANDIDATES}" >&2
  exit 1
fi

printf 'authority mutation candidates=%s version=%s\n' "$candidate_count" "$actual_version"
cargo mutants --file 'src/authority/mod.rs' --re "$selector" --timeout 40 --jobs 2
