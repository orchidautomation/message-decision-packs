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
  --force-cli           Reinstall the CLI even when the selected version is present.
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
  MDP_FORCE_CLI_UPDATE  Set to 1 to force a CLI repair/reinstall.
  MDP_AGENT_PLUGINS_INSTALL_DIR
                        Explicit client-managed import directory for the portable
                        core. Required for --agent-plugins. When set, --agents
                        installs it in addition to the four native bundles, but
                        it must not overlap any native install tree. Use a
                        documented native path such as Cursor's local import only
                        with portable-only --agent-plugins mode.
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
force_cli="${MDP_FORCE_CLI_UPDATE:-0}"
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
    --force-cli)
      force_cli=1
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
  fi
elif [ "${#targets[@]}" -eq 0 ]; then
  targets=(codex)
fi

validate_portable_destination() {
  local install_dir="${MDP_AGENT_PLUGINS_INSTALL_DIR:-}"
  local target native_root
  local native_roots=()

  if [ -z "$install_dir" ]; then
    echo "MDP_AGENT_PLUGINS_INSTALL_DIR is required for --agent-plugins." >&2
    echo "The portable archive is client-managed; MDP will not guess an undocumented Codex path." >&2
    echo 'A documented native import path may be used only in portable-only --agent-plugins mode.' >&2
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

  for target in "${targets[@]}"; do
    case "$target" in
      claude-code)
        native_root="${PLUXX_CLAUDE_MARKETPLACE_DIR:-$HOME/.claude/plugins/data/${PLUXX_CLAUDE_MARKETPLACE_NAME:-message-decision-packs-releases}}/plugins/message-decision-packs"
        native_roots+=("claude-code=$native_root")
        ;;
      cursor)
        native_roots+=("cursor=${PLUXX_CURSOR_INSTALL_DIR:-$HOME/.cursor/plugins/local/message-decision-packs}")
        ;;
      codex)
        native_roots+=("codex=${PLUXX_CODEX_INSTALL_DIR:-$HOME/.codex/plugins/message-decision-packs}")
        ;;
      opencode)
        native_roots+=("opencode=${PLUXX_OPENCODE_INSTALL_DIR:-${PLUXX_OPENCODE_PLUGIN_ROOT_DIR:-$HOME/.config/opencode/plugins}/message-decision-packs}")
        ;;
    esac
  done

  need_cmd node
  node - "$install_dir" "${native_roots[@]}" <<'NODE'
const { existsSync, lstatSync, readFileSync, readdirSync, realpathSync } = require('fs')
const { dirname, join, relative, resolve, sep } = require('path')
const [portableInput, ...nativeInputs] = process.argv.slice(2)

const canonical = (input) => {
  let cursor = resolve(input)
  const suffix = []
  while (!existsSync(cursor)) {
    const parent = dirname(cursor)
    if (parent === cursor) break
    suffix.unshift(cursor.slice(parent.length + (parent.endsWith(sep) ? 0 : 1)))
    cursor = parent
  }
  const base = existsSync(cursor) ? realpathSync(cursor) : cursor
  return suffix.reduce((path, part) => join(path, part), base)
}
const overlaps = (left, right) => {
  if (left === right) return true
  const leftToRight = relative(left, right)
  const rightToLeft = relative(right, left)
  return (leftToRight !== '' && !leftToRight.startsWith(`..${sep}`) && leftToRight !== '..') ||
    (rightToLeft !== '' && !rightToLeft.startsWith(`..${sep}`) && rightToLeft !== '..')
}

const portable = canonical(portableInput)
for (const value of nativeInputs) {
  const separator = value.indexOf('=')
  const host = value.slice(0, separator)
  const native = canonical(value.slice(separator + 1))
  if (overlaps(portable, native)) {
    console.error(`Refusing portable install: ${portable} overlaps the selected ${host} native install tree ${native}.`)
    console.error('Choose a distinct client-managed path, or run --agent-plugins by itself for a documented native import path.')
    process.exit(1)
  }
}
if (existsSync(portableInput)) {
  const stats = lstatSync(portableInput)
  if (stats.isSymbolicLink() || !stats.isDirectory()) {
    console.error(`Refusing portable install over a non-directory or symbolic-link destination: ${portable}`)
    process.exit(1)
  }
  const top = readdirSync(portableInput).sort()
  if (top.length > 0) {
    const ownedTop = JSON.stringify(top) === JSON.stringify(['plugin.json', 'skills'])
    let manifest
    try {
      manifest = ownedTop ? JSON.parse(readFileSync(join(portableInput, 'plugin.json'), 'utf8')) : undefined
    } catch {}
    const expectedSkills = ['mdp', 'mdp-pack-apply', 'mdp-pack-builder', 'mdp-pack-review'].sort()
    const skillsPath = join(portableInput, 'skills')
    const ownedSkills = existsSync(skillsPath) && !lstatSync(skillsPath).isSymbolicLink() &&
      lstatSync(skillsPath).isDirectory() &&
      JSON.stringify(readdirSync(skillsPath).sort()) === JSON.stringify(expectedSkills) &&
      expectedSkills.every((skill) => {
        const entry = join(skillsPath, skill)
        const skillFile = join(entry, 'SKILL.md')
        return existsSync(entry) && !lstatSync(entry).isSymbolicLink() && lstatSync(entry).isDirectory() &&
          existsSync(skillFile) && !lstatSync(skillFile).isSymbolicLink() && lstatSync(skillFile).isFile()
      })
    const ownedPortable =
      ownedTop &&
      ownedSkills &&
      manifest?.$schema === 'https://agent-plugins.org/schemas/1.0.0/plugin.schema.json' &&
      manifest?.name === 'message-decision-packs' &&
      manifest?.license === 'Elastic-2.0'
    if (!ownedPortable) {
      console.error(`Refusing portable install over an unknown or native nonempty destination: ${portable}`)
      console.error('Only an absent, empty, or previously owned MDP Agent Plugins portable directory may be replaced.')
      process.exit(1)
    }
  }
}
NODE
}

for target in "${targets[@]}"; do
  if [ "$target" = "agent-plugins" ]; then
    # Run before any native installer so an ambiguous destination cannot mutate
    # or replace a native tree before the portable install fails closed.
    validate_portable_destination
    break
  fi
done

if [ "$yes" = "1" ]; then
  export PLUXX_CODEX_ENABLE_PLUGIN_HOOKS="${PLUXX_CODEX_ENABLE_PLUGIN_HOOKS:-1}"
fi
export MDP_VERSION="$version"
export MDP_FORCE_CLI_UPDATE="$force_cli"

tmp_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

manifest="$tmp_dir/release-manifest.json"
checksums="$tmp_dir/SHA256SUMS.txt"
curl -fsSL "$base_url/release-manifest.json" -o "$manifest"
curl -fsSL "$base_url/SHA256SUMS.txt" -o "$checksums"
resolved_version="$(node - "$manifest" <<'NODE'
const { readFileSync } = require('fs')
const version = JSON.parse(readFileSync(process.argv[2], 'utf8'))?.plugin?.version
if (typeof version !== 'string' || !version) process.exit(1)
process.stdout.write(version)
NODE
)"
if [ "$version" != "latest" ] && [ "${version#v}" != "$resolved_version" ]; then
  echo "Release manifest version $resolved_version does not match requested version $version." >&2
  exit 1
fi
export MDP_RESOLVED_VERSION="$resolved_version"

summary_file="$tmp_dir/summary.tsv"
: > "$summary_file"
failed=0

verify_release_asset() {
  local filepath="$1" asset_name="$2" expected actual
  expected="$(awk -v name="$asset_name" '$2 == name { print $1 }' "$checksums")"
  if [ -z "$expected" ] || [ "$(printf '%s' "$expected" | wc -c | tr -d ' ')" -ne 64 ]; then
    echo "Release checksum inventory is missing a valid digest for $asset_name." >&2
    return 1
  fi
  if command -v sha256sum >/dev/null 2>&1; then actual="$(sha256sum "$filepath" | awk '{print $1}')"
  elif command -v shasum >/dev/null 2>&1; then actual="$(shasum -a 256 "$filepath" | awk '{print $1}')"
  else echo "Missing required checksum command: sha256sum or shasum" >&2; return 1
  fi
  if [ "$actual" != "$expected" ]; then
    echo "Release checksum mismatch for $asset_name." >&2
    return 1
  fi
}

install_cli() {
  local installed_version="" installer="$tmp_dir/install-cli.sh"
  if command -v mdp >/dev/null 2>&1; then
    installed_version="$(mdp --version 2>/dev/null | awk '{print $NF}' || true)"
  fi
  if [ "$installed_version" = "$resolved_version" ] && [ "$force_cli" != "1" ]; then
    printf 'cli\tunchanged\t\n' >> "$summary_file"
    return 0
  fi
  curl -fsSL "$base_url/install-cli.sh" -o "$installer" || return 1
  verify_release_asset "$installer" "install-cli.sh" || return 1
  if bash "$installer"; then
    if [ -n "$installed_version" ]; then printf 'cli\tupdated\t\n' >> "$summary_file"
    else printf 'cli\tinstalled\t\n' >> "$summary_file"
    fi
    export MDP_SKIP_CLI_UPDATE=1
  else
    printf 'cli\tfailed\tinstaller failed\n' >> "$summary_file"
    failed=1
  fi
}

install_native_targets() {
  local installer="$tmp_dir/install-agents.sh" result="$tmp_dir/native-results.json" status=0
  local args=(--json --quiet --base-url "$base_url" --version "$resolved_version")
  local target
  if [ "$yes" = "1" ]; then args+=(--yes); fi
  if [ "$agents" = "1" ]; then
    args+=(--agents)
  else
    for target in "${targets[@]}"; do
      case "$target" in claude-code|cursor|codex|opencode) args+=("--$target");; esac
    done
  fi
  curl -fsSL "$base_url/install-agents.sh" -o "$installer" || return 1
  verify_release_asset "$installer" "install-agents.sh" || return 1
  set +e
  bash "$installer" "${args[@]}" > "$result"
  status=$?
  set -e
  if ! node - "$result" "$summary_file" "$resolved_version" <<'NODE'
const { appendFileSync, readFileSync } = require('fs')
const fail = (message) => { console.error(message); process.exit(1) }
let value
try { value = JSON.parse(readFileSync(process.argv[2], 'utf8')) } catch { fail('Native installer returned invalid JSON.') }
const states = new Set(['installed', 'updated', 'unchanged', 'skipped', 'failed'])
const targets = new Set(['claude-code', 'cursor', 'codex', 'opencode'])
if (value?.schema !== 'pluxx.install-results.v1' || value?.plugin?.name !== 'message-decision-packs' ||
    value?.plugin?.version !== process.argv[4] ||
    !['aggregate', 'explicit'].includes(value?.selectionMode) || !Array.isArray(value?.results) ||
    !Array.isArray(value?.plan) || value.results.length !== value.plan.length) fail('Native installer result envelope failed validation.')
const seen = new Set()
for (const result of value.results) {
  if (!targets.has(result?.target) || seen.has(result.target) || !states.has(result?.state)) fail('Native installer returned an invalid target result.')
  if (result.state === 'skipped' && !result.reason) fail('Skipped native result is missing a reason.')
  if (result.state === 'failed' && (!result.error || !result.action)) fail('Failed native result is missing corrective detail.')
  seen.add(result.target)
  appendFileSync(process.argv[3], `${result.target}\t${result.state}\t${result.reason || result.error || ''}\n`)
}
NODE
  then
    echo "Could not consume the Pluxx install-results contract." >&2
    return 1
  fi
  native_failed=0
  grep -q $'\tfailed\t' "$summary_file" && native_failed=1
  if [ "$status" -ne 0 ] && [ "$native_failed" = "0" ]; then
    echo "Native installer exited nonzero without a failed terminal result." >&2
    return 1
  fi
  if [ "$status" -ne 0 ] || [ "$native_failed" = "1" ]; then failed=1; fi
}

install_agent_plugins() {
  local install_dir="${MDP_AGENT_PLUGINS_INSTALL_DIR:-}"
  local archive_name="message-decision-packs-agent-plugins-latest.tar.gz"
  local archive="$tmp_dir/$archive_name"
  local manifest="$tmp_dir/release-manifest.json"
  local checksums="$tmp_dir/SHA256SUMS.txt"
  local extracted="$tmp_dir/portable-extracted"
  local source_root="$extracted/agent-plugins"
  local expected actual existed=0
  [ -e "$install_dir" ] && existed=1

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
  plugin.name !== 'message-decision-packs' ||
  plugin.license !== 'Elastic-2.0'
) fail('Portable plugin.json does not match the MDP Agent Plugins contract.')
const expectedSkills = ['mdp', 'mdp-pack-apply', 'mdp-pack-builder', 'mdp-pack-review'].sort()
const skillsRoot = join(root, 'skills')
const skills = readdirSync(skillsRoot).filter((name) => lstatSync(join(skillsRoot, name)).isDirectory()).sort()
if (JSON.stringify(skills) !== JSON.stringify(expectedSkills)) {
  fail(`Portable package does not contain exactly the four supported MDP skills: ${skills.join(', ')}`)
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
if (manifest?.plugin?.license !== 'Elastic-2.0') fail('Release manifest does not retain the MDP Elastic-2.0 license.')
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

  if [ -d "$install_dir" ] && node - "$source_root" "$install_dir" <<'NODE'
const { createHash } = require('crypto')
const { lstatSync, readFileSync, readdirSync } = require('fs')
const { join, relative } = require('path')
const digest = (root) => {
  const records = []
  const walk = (directory) => {
    for (const name of readdirSync(directory).sort()) {
      const path = join(directory, name); const stats = lstatSync(path)
      if (stats.isSymbolicLink()) process.exit(1)
      if (stats.isDirectory()) walk(path)
      else if (stats.isFile()) records.push({
        path: relative(root, path).split('\\').join('/'),
        executable: (stats.mode & 0o111) !== 0,
        sha256: createHash('sha256').update(readFileSync(path)).digest('hex'),
      })
      else process.exit(1)
    }
  }
  walk(root)
  return createHash('sha256').update(`${JSON.stringify(records)}\n`).digest('hex')
}
process.exit(digest(process.argv[2]) === digest(process.argv[3]) ? 0 : 1)
NODE
  then
    printf 'unchanged\n' > "$tmp_dir/portable-state"
    return 0
  fi

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
  if [ "$existed" = "1" ]; then printf 'updated\n' > "$tmp_dir/portable-state"
  else printf 'installed\n' > "$tmp_dir/portable-state"
  fi
}

has_cli=0
has_native=0
has_portable=0
for target in "${targets[@]}"; do
  case "$target" in
    cli) has_cli=1 ;;
    claude-code|cursor|codex|opencode) has_native=1 ;;
    agent-plugins) has_portable=1 ;;
  esac
done

if [ "$has_cli" = "1" ]; then
  if ! install_cli; then
    printf 'cli\tfailed\tinstaller preparation failed\n' >> "$summary_file"
    failed=1
  fi
fi
if [ "$has_native" = "1" ]; then
  if ! install_native_targets; then
    echo "Native installer orchestration failed before returning a valid result envelope." >&2
    if [ "$agents" = "1" ]; then
      printf '%s\tfailed\tnative installer orchestration failed\n' claude-code cursor codex opencode >> "$summary_file"
    else
      for target in "${targets[@]}"; do
        case "$target" in
          claude-code|cursor|codex|opencode)
            printf '%s\tfailed\tnative installer orchestration failed\n' "$target" >> "$summary_file" ;;
        esac
      done
    fi
    failed=1
  fi
fi
if [ "$has_portable" = "1" ]; then
  set +e
  ( set -e; install_agent_plugins )
  portable_status=$?
  set -e
  if [ "$portable_status" -eq 0 ]; then
    portable_state="$(cat "$tmp_dir/portable-state")"
    printf 'agent-plugins\t%s\t\n' "$portable_state" >> "$summary_file"
  else
    printf 'agent-plugins\tfailed\tportable install failed\n' >> "$summary_file"
    failed=1
  fi
fi

echo "Message Decision Packs $resolved_version"
reload_hosts=()
while IFS=$'\t' read -r target state detail; do
  case "$state" in
    installed|updated) marker="✓" ;;
    unchanged) marker="=" ;;
    skipped) marker="-" ;;
    failed) marker="!" ;;
    *) marker="?" ;;
  esac
  if [ -n "$detail" ]; then printf '%s %s: %s (%s)\n' "$marker" "$target" "$state" "$detail"
  else printf '%s %s: %s\n' "$marker" "$target" "$state"
  fi
  case "$target:$state" in
    claude-code:installed|claude-code:updated|cursor:installed|cursor:updated|codex:installed|codex:updated|opencode:installed|opencode:updated)
      reload_hosts+=("$target") ;;
  esac
done < "$summary_file"
if [ "${#reload_hosts[@]}" -gt 0 ]; then
  reload_list="${reload_hosts[0]}"
  for target in "${reload_hosts[@]:1}"; do reload_list="$reload_list, $target"; done
  printf 'Reload or restart updated agent hosts: %s.\n' "$reload_list"
fi
[ "$failed" -eq 0 ]
