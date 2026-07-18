#!/usr/bin/env bash
set -euo pipefail

release_assets="${1:-release-assets}"
if [ ! -d "$release_assets" ]; then
  echo "Release asset directory does not exist: $release_assets" >&2
  exit 1
fi

release_assets="$(cd "$release_assets" && pwd)"
for required in SHA256SUMS.txt install.sh release-manifest.json; do
  if [ ! -f "$release_assets/$required" ]; then
    echo "Missing release asset: $required" >&2
    exit 1
  fi
done

node "$(dirname "$0")/finalize-release-manifest.mjs" "$release_assets/release-manifest.json"

asset_list="$(mktemp "$release_assets/.checksum-assets.XXXXXX")"
plugin_checksums="$(mktemp "$release_assets/.SHA256SUMS.XXXXXX")"
cli_checksums="$(mktemp "$release_assets/.MDP_CLI_SHA256SUMS.XXXXXX")"
cleanup() {
  rm -f "$asset_list" "$plugin_checksums" "$cli_checksums"
}
trap cleanup EXIT

awk 'NF >= 2 { print $2 }' "$release_assets/SHA256SUMS.txt" > "$asset_list"
for required_checksum in install.sh release-manifest.json; do
  if ! grep -Fxq "$required_checksum" "$asset_list"; then
    echo "Release checksum inventory is missing: $required_checksum" >&2
    exit 1
  fi
done
(
  cd "$release_assets"
  while IFS= read -r asset; do
    case "$asset" in
      ""|-*|/*|*/*|*..*)
        echo "Unsafe release checksum asset path: $asset" >&2
        exit 1
        ;;
    esac
    if [ ! -f "$asset" ] || [ -L "$asset" ]; then
      echo "Missing checksummed release asset: $asset" >&2
      exit 1
    fi
    shasum -a 256 "$asset"
  done < "$asset_list" > "$plugin_checksums"

  set -- mdp-*
  if [ "$1" = 'mdp-*' ] || [ ! -f "$1" ]; then
    echo "No mdp CLI release assets found in $release_assets" >&2
    exit 1
  fi
  shasum -a 256 "$@" > "$cli_checksums"
)

mv "$plugin_checksums" "$release_assets/SHA256SUMS.txt"
mv "$cli_checksums" "$release_assets/MDP_CLI_SHA256SUMS.txt"
(
  cd "$release_assets"
  shasum -a 256 -c SHA256SUMS.txt
  shasum -a 256 -c MDP_CLI_SHA256SUMS.txt
)
