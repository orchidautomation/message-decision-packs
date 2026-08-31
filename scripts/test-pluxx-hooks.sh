#!/usr/bin/env bash
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$ROOT"

cleanup_artifact_root=0
if [ -n "${MDP_TEMP_ROOT:-}" ]; then
  artifact_root="$MDP_TEMP_ROOT"
else
  artifact_root="$(mktemp -d)"
  cleanup_artifact_root=1
fi
lint_json="$artifact_root/mdp-pluxx-lint.json"
build_json="$artifact_root/mdp-pluxx-build.json"
packaging_json="$artifact_root/mdp-skill-packaging.json"
export MDP_PLUXX_LINT_JSON="$lint_json"
workspace_fixture=""
plugin_fixture=""
proposal_fixture=""
root_fallback_fixture=""
codex_manifest_fixture=""
codex_launch_fixture=""
codex_pack_fixture=""
cleanup() {
  for fixture in "${workspace_fixture:-}" "${plugin_fixture:-}" "${proposal_fixture:-}" "${root_fallback_fixture:-}" "${codex_manifest_fixture:-}" "${codex_launch_fixture:-}" "${codex_pack_fixture:-}"; do
    if [ -n "$fixture" ]; then
      rm -rf "$fixture"
    fi
  done
  if [ "$cleanup_artifact_root" = "1" ]; then
    rm -rf "$artifact_root"
  fi
}
trap cleanup EXIT

PLUXX_VERSION="${PLUXX_VERSION:-0.1.42}"
if command -v pluxx >/dev/null 2>&1 && [ "$(pluxx --version)" = "$PLUXX_VERSION" ]; then
  PLUXX_CMD=(pluxx)
elif command -v npx >/dev/null 2>&1; then
  PLUXX_CMD=(npx --yes --package "@orchid-labs/pluxx@$PLUXX_VERSION" pluxx)
else
  echo "Skipping Pluxx hook fixture validation; missing pluxx and npx on PATH."
  exit 0
fi

"${PLUXX_CMD[@]}" lint --json >"$lint_json"
find "$ROOT/scripts" -type d -name __pycache__ -prune -exec rm -rf {} +
find "$ROOT/dist" -type d -name __pycache__ -prune -exec rm -rf {} + 2>/dev/null || true
"${PLUXX_CMD[@]}" build --json >"$build_json"
if find "$ROOT/dist" -type d -name __pycache__ | grep -q .; then
  echo "Generated Pluxx bundles must not include Python __pycache__ directories." >&2
  find "$ROOT/dist" -type d -name __pycache__ >&2
  exit 1
fi
python3 scripts/validate-skill-packaging.py --require-bundles >"$packaging_json"

workspace_fixture="$(mktemp -d "$artifact_root/workspace.XXXXXX")"
plugin_fixture="$(mktemp -d "$artifact_root/plugin.XXXXXX")"
proposal_fixture="$(mktemp -d "$artifact_root/proposal.XXXXXX")"
mkdir -p "$workspace_fixture/.mdp" "$plugin_fixture/.mdp" "$proposal_fixture/.mdp/prompts"
printf 'name: hook-workspace-fixture\nversion: 0.1.0\n' >"$workspace_fixture/.mdp/manifest.yaml"
printf 'name: plugin-root-should-not-activate\nversion: 0.1.0\n' >"$plugin_fixture/.mdp/manifest.yaml"
printf 'name: proposal-hook-fixture\nversion: 0.1.0\nprofile: proposal\n' >"$proposal_fixture/.mdp/manifest.yaml"
printf 'id: normalize-opportunity\n' >"$proposal_fixture/.mdp/prompts/normalize-opportunity.yaml"

activation_output="$(
  cd "$plugin_fixture"
  PLUGIN_ROOT="$plugin_fixture" PLUXX_HOOK_WORKSPACE_ROOT="$workspace_fixture" bash "$ROOT/scripts/mdp-activate.sh"
)"
if ! printf '%s\n' "$activation_output" | grep -F "detected in $workspace_fixture" >/dev/null; then
  echo "MDP activation must use PLUXX_HOOK_WORKSPACE_ROOT when hook cwd is the plugin root." >&2
  exit 1
fi
if ! printf '%s\n' "$activation_output" | grep -F "MCP path: mdp_run_tools -> mdp_prepare_run -> mdp_run -> mdp_verify_run." >/dev/null; then
  echo "MDP activation must expose the canonical MCP path for a basic/GTM pack." >&2
  printf '%s\n' "$activation_output" >&2
  exit 1
fi

source_activation_output="$(
  cd "$plugin_fixture"
  env -u PLUGIN_ROOT MDP_HOOK_DIR="$workspace_fixture" bash "$ROOT/scripts/mdp-activate.sh"
)"
if ! printf '%s\n' "$source_activation_output" | grep -F "available as node \"$ROOT/scripts/mdp-run-mcp-server.mjs\"" >/dev/null; then
  echo "Direct source activation must discover the canonical MCP without PLUGIN_ROOT." >&2
  printf '%s\n' "$source_activation_output" >&2
  exit 1
fi

plugin_root_output="$(
  cd "$plugin_fixture"
  PLUGIN_ROOT="$plugin_fixture" bash "$ROOT/scripts/mdp-activate.sh"
)"
if [ -n "$plugin_root_output" ]; then
  echo "MDP activation must not inspect .mdp relative to the installed plugin root." >&2
  exit 1
fi

proposal_output="$(
  cd "$plugin_fixture"
  PLUGIN_ROOT="$ROOT" PLUXX_HOOK_WORKSPACE_ROOT="$proposal_fixture" OPENAI_API_KEY= bash "$ROOT/scripts/mdp-activate.sh"
)"
if ! printf '%s\n' "$proposal_output" | grep -F "MDP clean-run readiness:" >/dev/null; then
  echo "MDP activation must print clean-run readiness for proposal packs." >&2
  printf '%s\n' "$proposal_output" >&2
  exit 1
fi
if ! printf '%s\n' "$proposal_output" | grep -F "Canonical local stdio MCP: available" >/dev/null; then
  echo "MDP activation must report canonical local stdio MCP availability for proposal packs." >&2
  printf '%s\n' "$proposal_output" >&2
  exit 1
fi
if ! printf '%s\n' "$proposal_output" | grep -F "Canonical native OpenAI driver: available for an operator-authorized BYOK model step." >/dev/null; then
  echo "MDP activation must report the canonical native driver." >&2
  printf '%s\n' "$proposal_output" >&2
  exit 1
fi
if ! printf '%s\n' "$proposal_output" | grep -F "The canonical MCP is local stdio transport only, not a hosted or remote MCP service." >/dev/null; then
  echo "MDP activation must avoid implying a hosted/remote MCP service exists." >&2
  printf '%s\n' "$proposal_output" >&2
  exit 1
fi
if ! printf '%s\n' "$proposal_output" | grep -F "MCP path: mdp_run_tools -> mdp_prepare_run -> mdp_run -> mdp_verify_run." >/dev/null; then
  echo "MDP activation must report the canonical four-tool path." >&2
  printf '%s\n' "$proposal_output" >&2
  exit 1
fi
if ! printf '%s\n' "$proposal_output" | grep -F "OPENAI_API_KEY: not detected; only required for an optional real native OpenAI runner call." >/dev/null; then
  echo "MDP activation must explain that missing OPENAI_API_KEY only affects optional native runs." >&2
  printf '%s\n' "$proposal_output" >&2
  exit 1
fi
if ! printf '%s\n' "$proposal_output" | grep -F "No OpenAI key is required for MDP install, validation, receipts, fit/review, dry-runs, mocks, or hardened headless runner audits." >/dev/null; then
  echo "MDP activation must preserve non-OpenAI audit runner guidance." >&2
  printf '%s\n' "$proposal_output" >&2
  exit 1
fi

key_output="$(
  cd "$plugin_fixture"
  PLUGIN_ROOT="$ROOT" PLUXX_HOOK_WORKSPACE_ROOT="$proposal_fixture" OPENAI_API_KEY="sk-test-do-not-print" bash "$ROOT/scripts/mdp-activate.sh"
)"
if ! printf '%s\n' "$key_output" | grep -F "OPENAI_API_KEY: detected for optional real native API normalization (value not printed)." >/dev/null; then
  echo "MDP activation must report key presence without printing the key." >&2
  printf '%s\n' "$key_output" >&2
  exit 1
fi
if printf '%s\n' "$key_output" | grep -F "sk-test-do-not-print" >/dev/null; then
  echo "MDP activation must never print OPENAI_API_KEY values." >&2
  exit 1
fi

if command -v cargo >/dev/null 2>&1 && command -v git >/dev/null 2>&1; then
  root_fallback_fixture="$(mktemp -d "$artifact_root/root-fallback.XXXXXX")"
  cp -R "$ROOT/plugin/assets/templates/basic/.mdp" "$root_fallback_fixture/.mdp"
  ln -s "$ROOT/cli" "$root_fallback_fixture/cli"
  ln -s "$ROOT/rust-toolchain.toml" "$root_fallback_fixture/rust-toolchain.toml"
  git -C "$root_fallback_fixture" init -q

  cargo_bin="$(dirname -- "$(command -v cargo)")"
  git_bin="$(dirname -- "$(command -v git)")"
  bash_bin="$(dirname -- "$(command -v bash)")"
  hook_path="$cargo_bin:$git_bin:$bash_bin:/usr/bin:/bin:/usr/sbin:/sbin"

  if PATH="$hook_path" command -v mdp >/dev/null 2>&1; then
    echo "Root-pack cargo fallback fixture path unexpectedly includes mdp." >&2
    exit 1
  fi

  root_fallback_output="$(
    PATH="$hook_path" PLUXX_HOOK_WORKSPACE_ROOT="$root_fallback_fixture" bash "$ROOT/scripts/mdp-post-edit-validate.sh"
  )"
  if ! printf '%s\n' "$root_fallback_output" | grep -F "MDP validation check: root pack validate" >/dev/null; then
    echo "Root-pack validation must fall back to the source CLI when mdp is absent from PATH." >&2
    printf '%s\n' "$root_fallback_output" >&2
    exit 1
  fi
fi

node <<'NODE'
const fs = require('fs')

function readJson(path) {
  return JSON.parse(fs.readFileSync(path, 'utf8'))
}

function assert(condition, message) {
  if (!condition) {
    console.error(message)
    process.exit(1)
  }
}

const startupEvent = 'Ses' + 'sionStart'
const claudeManifest = readJson('dist/claude-code/.claude-plugin/plugin.json')
const claudeHooks = readJson('dist/claude-code/hooks/hooks.json')
const codexManifest = readJson('dist/codex/.codex-plugin/plugin.json')
const codexHooks = readJson('dist/codex/hooks/hooks.json')
const codexCompanion = readJson('dist/codex/.codex/hooks.generated.json')
const lintResult = readJson(process.env.MDP_PLUXX_LINT_JSON)

const truncationIssues = lintResult.issues.filter((issue) => issue.code === 'skill-description-truncation')
assert(truncationIssues.length === 0, 'Pluxx lint must not truncate skill descriptions on supported hosts.')

assert(claudeManifest.hooks === undefined, 'Claude Code manifest must not duplicate the standard hooks file.')
assert(codexManifest.hooks === './hooks/hooks.json', 'Codex manifest must point at bundled hooks.')
assert(claudeHooks.hooks[startupEvent], 'Claude Code hooks must include startup activation.')
assert(claudeHooks.hooks.UserPromptSubmit, 'Claude Code hooks must include prompt activation.')
assert(claudeHooks.hooks.PostToolUse, 'Claude Code hooks must include post-tool validation.')
assert(codexHooks.hooks[startupEvent], 'Codex hooks must include startup activation.')
assert(codexHooks.hooks.UserPromptSubmit, 'Codex hooks must include prompt activation.')
assert(codexHooks.hooks.PostToolUse, 'Codex hooks must include post-tool validation.')
assert(codexHooks.hooks.PostToolUse[0]?.matcher === 'Edit|Write|apply_patch', 'Codex post-tool validation must be scoped to edit-capable tools.')
assert(codexCompanion.enforcedByPluginBundle === true, 'Codex hook companion must mark hooks as bundled.')
assert(codexCompanion.pluginBundleFeatureFlag === 'hooks', 'Codex hook companion must document the current feature flag.')

const generatedFiles = [
  'dist/claude-code/hooks/pluxx-hook-command-1.mjs',
  'dist/claude-code/hooks/pluxx-hook-command-2.mjs',
  'dist/claude-code/hooks/pluxx-hook-command-3.mjs',
  'dist/codex/hooks/pluxx-hook-command-1.mjs',
  'dist/codex/hooks/pluxx-hook-command-2.mjs',
  'dist/codex/hooks/pluxx-hook-command-3.mjs',
]

const generatedText = generatedFiles.map((path) => fs.readFileSync(path, 'utf8')).join('\n')
assert(generatedText.includes('mdp-activate.sh'), 'Generated hook wrappers must call mdp-activate.sh.')
assert(generatedText.includes('mdp-post-edit-validate.sh'), 'Generated hook wrappers must call mdp-post-edit-validate.sh.')
assert(generatedText.includes('PLUXX_HOOK_WORKSPACE_ROOT'), 'Generated hook wrappers must expose PLUXX_HOOK_WORKSPACE_ROOT.')

const opencodePlugin = fs.readFileSync('dist/opencode/index.ts', 'utf8')
assert(opencodePlugin.includes('fileURLToPath(import.meta.url)'), 'OpenCode plugin must derive plugin root from its installed module URL.')
assert(opencodePlugin.includes('const workspaceRoot = directory'), 'OpenCode plugin must preserve directory as the active workspace root.')
assert(opencodePlugin.includes('replaceAll("${PLUGIN_ROOT}", pluginRoot)'), 'OpenCode hooks must resolve ${PLUGIN_ROOT} against the installed plugin root.')
assert(!opencodePlugin.includes('replaceAll("${PLUGIN_ROOT}", directory)'), 'OpenCode hooks must not resolve ${PLUGIN_ROOT} against the active workspace directory.')
assert(opencodePlugin.includes('PLUXX_HOOK_WORKSPACE_ROOT: workspaceRoot'), 'OpenCode hooks must expose the active workspace root separately.')
assert(opencodePlugin.includes('PLUXX_PLUGIN_ROOT: pluginRoot'), 'OpenCode hooks must expose the installed plugin root separately.')

console.log('Pluxx hook fixture validation passed.')
NODE

# Stage the generated Codex bundle as the installed plugin root and execute
# the exact descriptor commands (no wrapper-by-absolute-path shortcut) from
# an unrelated workspace with CODEX_PLUGIN_ROOT unset. PLUXX-345's regression
# is that the prior generator embedded ${CODEX_PLUGIN_ROOT} in the descriptor,
# so the regression proof must be manifest-bound, not wrapper-bound.
codex_manifest_fixture="$(mktemp -d "$artifact_root/codex-installed.XXXXXX")"
codex_launch_fixture="$(mktemp -d "$artifact_root/codex-launched.XXXXXX")"
codex_pack_fixture="$(mktemp -d "$artifact_root/codex-pack-launched.XXXXXX")"
mkdir -p "$codex_pack_fixture/.mdp"
printf 'name: codex-hook-fixture\nversion: 0.1.0\n' >"$codex_pack_fixture/.mdp/manifest.yaml"
cp -R "$ROOT/dist/codex/." "$codex_manifest_fixture/"
export MDP_CODEX_INSTALLED_ROOT="$codex_manifest_fixture"
export MDP_CODEX_LAUNCH_ROOT="$codex_launch_fixture"
export MDP_CODEX_PACK_ROOT="$codex_pack_fixture"

export MDP_CODEX_INSTALLED_ROOT MDP_CODEX_LAUNCH_ROOT MDP_CODEX_PACK_ROOT
node <<'CODEX_MANIFEST_NODE'
const fs = require('fs')
const { spawnSync } = require('child_process')

const assert = (condition, message) => {
  if (!condition) {
    console.error(message)
    process.exit(1)
  }
}

const manifest = JSON.parse(fs.readFileSync('dist/codex/hooks/hooks.json', 'utf8'))
const startName = 'Ses' + 'sionStart'
const startCommand = manifest.hooks[startName]?.[0]?.hooks?.[0]?.command
const promptCommand = manifest.hooks.UserPromptSubmit?.[0]?.hooks?.[0]?.command
assert(typeof startCommand === 'string', 'Codex generated manifest must include a SessionStart command.')
assert(typeof promptCommand === 'string', 'Codex generated manifest must include a UserPromptSubmit command.')
for (const [event, command] of [[startName, startCommand], ['UserPromptSubmit', promptCommand]]) {
  assert(
    !command.includes('CODEX_PLUGIN_ROOT'),
    `Codex ${event} must not depend on CODEX_PLUGIN_ROOT (got: ${command}).`,
  )
  assert(
    command.includes('${PLUGIN_ROOT}'),
    `Codex ${event} must resolve the installed plugin root via \${PLUGIN_ROOT} (got: ${command}).`,
  )
}

const installedRoot = process.env.MDP_CODEX_INSTALLED_ROOT
const launchRoot = process.env.MDP_CODEX_LAUNCH_ROOT
const packRoot = process.env.MDP_CODEX_PACK_ROOT
assert(installedRoot && launchRoot && packRoot, 'Expected installed, unrelated, and pack Codex workspaces to be staged.')

// The Codex host runs the exact descriptor string with $PLUGIN_ROOT
// expansion in its own shell. We mirror that with bash -c, supplying
// PLUGIN_ROOT as an environment variable and explicitly unsetting the
// legacy CODEX_PLUGIN_ROOT so the regression proof covers PLUXX-345.
const runDescriptor = (label, command, cwd, extraEnv) => {
  const env = { ...process.env, ...extraEnv, PLUGIN_ROOT: installedRoot }
  delete env.CODEX_PLUGIN_ROOT
  const result = spawnSync('bash', ['-c', command], {
    cwd,
    env,
    input: '',
    encoding: 'utf8',
  })
  if (result.status !== 0) {
    console.error(`Installed Codex ${label} exited ${result.status}; expected 0.`)
    console.error(`  command: ${command}`)
    console.error(`  cwd:     ${cwd}`)
    console.error(`  stdout:  ${result.stdout}`)
    console.error(`  stderr:  ${result.stderr}`)
    process.exit(1)
  }
  return result
}

// Manifest-bound regression proof for PLUXX-345: the exact SessionStart and
// UserPromptSubmit descriptor commands must launch from an unrelated
// workspace (no .mdp/manifest.yaml) when CODEX_PLUGIN_ROOT is unset.
runDescriptor('SessionStart', startCommand, launchRoot, {
  PLUGIN_ROOT: installedRoot,
})
runDescriptor('UserPromptSubmit', promptCommand, launchRoot, {
  PLUGIN_ROOT: installedRoot,
})

// The same descriptor must reach scripts/mdp-activate.sh when launched from
// a pack workspace, surfacing pack detection in the activation output.
const packResult = runDescriptor(
  'SessionStart in pack workspace',
  startCommand,
  packRoot,
  {
    PLUXX_HOOK_WORKSPACE_ROOT: packRoot,
  },
)
assert(
  packResult.stdout.includes(`detected in ${packRoot}`),
  `Installed Codex SessionStart must surface pack detection from the manifest command.
stdout=${packResult.stdout}`,
)

console.log('Installed Codex hook commands exit 0 with CODEX_PLUGIN_ROOT unset.')
CODEX_MANIFEST_NODE

# --------------------------------------------------------------------------
# MDP activation idempotence and bounded compactness (MDP-281).
# --------------------------------------------------------------------------

idempotence_cache_root="$(mktemp -d "$artifact_root/mdp-activation-cache.XXXXXX")"
idempotence_marker_root="$(mktemp -d "$artifact_root/mdp-activation-marker.XXXXXX")"
export MDP_ACTIVATION_CACHE_ROOT="$idempotence_cache_root"
export MDP_CODEX_BUNDLE_PATH=""

idempotence_workspace="$(mktemp -d "$artifact_root/mdp-281-workspace.XXXXXX")"
mkdir -p "$idempotence_workspace/.mdp"
cat > "$idempotence_workspace/.mdp/manifest.yaml" <<'WORKSPACE_YAML'
name: idempotence-pack
version: 0.1.0
WORKSPACE_YAML

# Default invocation with no --mode must remain full (backward compat).
default_output="$(
  PLUGIN_ROOT="$ROOT" PLUXX_HOOK_WORKSPACE_ROOT="$idempotence_workspace"     MDP_HOOK_SESSION_ID="default-session"     bash "$ROOT/scripts/mdp-activate.sh"
)"
if ! printf '%s\n' "$default_output" | grep -F "MDP activation: .mdp/manifest.yaml detected in $idempotence_workspace." >/dev/null; then
  echo "Default invocation must emit the full activation payload." >&2
  exit 1
fi

# --mode=compact first call emits one bounded refresh marker.
first_compact="$(
  PLUGIN_ROOT="$ROOT" PLUXX_HOOK_WORKSPACE_ROOT="$idempotence_workspace"     MDP_HOOK_SESSION_ID="session-compact-1"     bash "$ROOT/scripts/mdp-activate.sh" --mode=compact
)"
first_compact_len="${#first_compact}"
if [ "$first_compact_len" -lt 8 ] || [ "$first_compact_len" -gt 200 ]; then
  echo "First compact call refresh marker must be 8-200 chars; got $first_compact_len." >&2
  printf '%s\n' "$first_compact" >&2
  exit 1
fi
if ! printf '%s\n' "$first_compact" | grep -E '^MDP refresh: ' >/dev/null; then
  echo "First compact call must emit a refresh marker line." >&2
  printf '%s\n' "$first_compact" >&2
  exit 1
fi

# Subsequent compact calls for the same session and workspace are silent.
repeat_compact_lengths=()
for i in 1 2 3 4 5; do
  out="$(
    PLUGIN_ROOT="$ROOT" PLUXX_HOOK_WORKSPACE_ROOT="$idempotence_workspace"       MDP_HOOK_SESSION_ID="session-compact-1"       bash "$ROOT/scripts/mdp-activate.sh" --mode=compact
  )"
  repeat_compact_lengths+=("${#out}")
  if [ "${#out}" -ne 0 ]; then
    echo "Repeat compact call must be silent (got ${#out} chars)." >&2
    printf '%s\n' "$out" >&2
    exit 1
  fi
done

# Authority change in the workspace forces exactly one refresh marker.
cat > "$idempotence_workspace/.mdp/manifest.yaml" <<'WORKSPACE_YAML'
name: idempotence-pack
version: 0.1.0
description: changed authority
WORKSPACE_YAML
refreshed_compact="$(
  PLUGIN_ROOT="$ROOT" PLUXX_HOOK_WORKSPACE_ROOT="$idempotence_workspace"     MDP_HOOK_SESSION_ID="session-compact-1"     bash "$ROOT/scripts/mdp-activate.sh" --mode=compact
)"
if [ "${#refreshed_compact}" -lt 8 ] || [ "${#refreshed_compact}" -gt 200 ]; then
  echo "Authority-change compact refresh must be 8-200 chars; got ${#refreshed_compact}." >&2
  printf '%s\n' "$refreshed_compact" >&2
  exit 1
fi
if ! printf '%s\n' "$refreshed_compact" | grep -E '^MDP refresh: ' >/dev/null; then
  echo "Authority-change compact call must emit a refresh marker line." >&2
  exit 1
fi

# Out-of-order first event (compact before full) still results in one full
# activation, then compact returns to silence.
ordered_workspace="$(mktemp -d "$artifact_root/mdp-281-ordered.XXXXXX")"
mkdir -p "$ordered_workspace/.mdp"
cat > "$ordered_workspace/.mdp/manifest.yaml" <<'WORKSPACE_YAML'
name: ordered-events-pack
version: 0.1.0
WORKSPACE_YAML

# Compact first: session identity present → marker.
ordered_compact_first="$(
  PLUGIN_ROOT="$ROOT" PLUXX_HOOK_WORKSPACE_ROOT="$ordered_workspace"     MDP_HOOK_SESSION_ID="session-ordered"     bash "$ROOT/scripts/mdp-activate.sh" --mode=compact
)"
if [ "${#ordered_compact_first}" -lt 8 ] || [ "${#ordered_compact_first}" -gt 200 ]; then
  echo "Compact-first call must emit a refresh marker; got ${#ordered_compact_first}." >&2
  printf '%s\n' "$ordered_compact_first" >&2
  exit 1
fi
ordered_compact_second="$(
  PLUGIN_ROOT="$ROOT" PLUXX_HOOK_WORKSPACE_ROOT="$ordered_workspace"     MDP_HOOK_SESSION_ID="session-ordered"     bash "$ROOT/scripts/mdp-activate.sh" --mode=compact
)"
if [ -n "$ordered_compact_second" ]; then
  echo "Compact-second call (after compact-first) must be silent." >&2
  printf '%s\n' "$ordered_compact_second" >&2
  exit 1
fi

# Missing session identity degrades to full output (never cross-session
# suppression).
degraded_workspace="$(mktemp -d "$artifact_root/mdp-281-degraded.XXXXXX")"
mkdir -p "$degraded_workspace/.mdp"
cat > "$degraded_workspace/.mdp/manifest.yaml" <<'WORKSPACE_YAML'
name: degraded-pack
version: 0.1.0
WORKSPACE_YAML
degraded_full="$(
  PLUGIN_ROOT="$ROOT" PLUXX_HOOK_WORKSPACE_ROOT="$degraded_workspace"     env -u MDP_HOOK_SESSION_ID -u CODEX_SESSION_ID -u CLAUDE_SESSION_ID       -u CLAUDE_CODE_SESSION_ID -u CURSOR_SESSION_ID -u OPENCODE_SESSION_ID     bash "$ROOT/scripts/mdp-activate.sh" --mode=compact
)"
if ! printf '%s\n' "$degraded_full" | grep -F "MDP activation:" >/dev/null; then
  echo "Compact mode without session identity must fall back to full activation." >&2
  printf '%s\n' "$degraded_full" >&2
  exit 1
fi
# The degraded path must NOT have written any cache file (cross-session
# suppression is forbidden).
cache_path="$idempotence_cache_root/$(printf 'workspace=%s\n' "$degraded_workspace" | sha256sum | cut -d' ' -f1).kv"
if [ -e "$cache_path" ]; then
  echo "Degraded session must not write a cache record." >&2
  exit 1
fi

# Secret non-disclosure: provide a key-like value and assert it never
# appears in stdout, stderr, or cache state.
secret_workspace="$(mktemp -d "$artifact_root/mdp-281-secret.XXXXXX")"
mkdir -p "$secret_workspace/.mdp"
cat > "$secret_workspace/.mdp/manifest.yaml" <<'WORKSPACE_YAML'
name: secret-pack
version: 0.1.0
WORKSPACE_YAML
secret_value="sk-test-do-not-print-12345"
# Drive through stdin payload + env to verify non-disclosure.
secret_payload="$(printf '{"sessionId":"secret-session","cwd":"%s"}' "$secret_workspace")"
secret_output="$(
  cd "$ROOT"
  PLUGIN_ROOT="$ROOT" MDP_HOOK_SESSION_ID="$secret_value"     OPENAI_API_KEY="$secret_value"     MDP_HOOK_DIR="$secret_workspace"     bash "$ROOT/scripts/mdp-activate.sh" --mode=full --workspace="$secret_workspace" --plugin-root="$ROOT"     <<< "$secret_payload"
)"
if printf '%s\n' "$secret_output" | grep -F "$secret_value" >/dev/null; then
  echo "MDP activation must never print OPENAI_API_KEY or session id values." >&2
  printf '%s\n' "$secret_output" >&2
  exit 1
fi
secret_cache="$idempotence_cache_root/$(printf 'workspace=%s\n' "$secret_workspace" | sha256sum | cut -d' ' -f1).kv"
if [ -f "$secret_cache" ] && grep -F "$secret_value" "$secret_cache" >/dev/null; then
  echo "MDP activation cache must never persist secret values." >&2
  cat "$secret_cache" >&2
  exit 1
fi

# Cache root permissions: created dirs must be 700 with 600 files.
cache_dir_perm="$(stat -c '%a' "$idempotence_cache_root")"
if [ "$cache_dir_perm" != "700" ]; then
  echo "MDP activation cache root must be mode 0700; got $cache_dir_perm." >&2
  exit 1
fi
for kv in "$idempotence_cache_root"/*.kv; do
  [ -f "$kv" ] || continue
  file_perm="$(stat -c '%a' "$kv")"
  if [ "$file_perm" != "600" ]; then
    echo "MDP activation cache files must be mode 0600; got $file_perm on $kv." >&2
    exit 1
  fi
done

# Concurrency: two simultaneous compact invocations produce at most one
# full activation each (one marker each, no double output).
concurrent_workspace="$(mktemp -d "$artifact_root/mdp-281-concurrent.XXXXXX")"
mkdir -p "$concurrent_workspace/.mdp"
cat > "$concurrent_workspace/.mdp/manifest.yaml" <<'WORKSPACE_YAML'
name: concurrent-pack
version: 0.1.0
WORKSPACE_YAML
concurrent_a="$(mktemp)"
concurrent_b="$(mktemp)"
PLUGIN_ROOT="$ROOT" PLUXX_HOOK_WORKSPACE_ROOT="$concurrent_workspace"   MDP_HOOK_SESSION_ID="concurrent-session"   bash "$ROOT/scripts/mdp-activate.sh" --mode=compact >"$concurrent_a" 2>&1 &
pid_a=$!
PLUGIN_ROOT="$ROOT" PLUXX_HOOK_WORKSPACE_ROOT="$concurrent_workspace"   MDP_HOOK_SESSION_ID="concurrent-session"   bash "$ROOT/scripts/mdp-activate.sh" --mode=compact >"$concurrent_b" 2>&1 &
pid_b=$!
wait "$pid_a" || true
wait "$pid_b" || true
size_a="$(wc -c <"$concurrent_a")"
size_b="$(wc -c <"$concurrent_b")"
if [ "$size_a" -gt 200 ] || [ "$size_b" -gt 200 ]; then
  echo "Concurrent compact invocations must each stay <=200 chars; got $size_a + $size_b." >&2
  cat "$concurrent_a" "$concurrent_b" >&2
  exit 1
fi
rm -f "$concurrent_a" "$concurrent_b"

# Repeat unchanged compact-call p50 measurement (informational).
benchmark_file="$artifact_root/mdp-pluxx-activation-benchmark.json"
node "$ROOT/scripts/test-mdp-activation-benchmark.mjs" \
  --root "$ROOT" \
  --workspace "$idempotence_workspace" \
  --cache "$idempotence_cache_root" \
  --out "$benchmark_file" \
  --iterations 50
if [ -s "$benchmark_file" ]; then
  p50_value="$(node -e "const { p50_ms } = JSON.parse(require('node:fs').readFileSync(process.argv[1], 'utf8')); console.log(p50_ms)" "$benchmark_file")"
  if [ "$(printf '%.0f' "$p50_value")" -ge 40 ]; then
    echo "MDP-281 activation warm-unchanged benchmark p50=${p50_value}ms exceeds the 40ms safety budget." >&2
    cat "$benchmark_file" >&2
    exit 1
  fi
fi

echo "MDP activation idempotence + compact + safe-fallback tests passed."
echo "  full-mode payload preserved."
echo "  compact 1st call: refresh marker (8-200 chars)."
echo "  compact repeated: silent body."
echo "  authority refresh: marker; order-independent persistence."
echo "  degraded session: full fallback, no cache write."
echo "  secret non-disclosure: API key + session value never persisted."
echo "  cache permissions: 700 root + 600 files."
echo "  concurrent calls: each output <=200 chars."
echo "  benchmark p50 (warm unchanged): $(node -e "const { p50_ms, p95_ms } = JSON.parse(require('node:fs').readFileSync(process.argv[1], 'utf8')); console.log(p50_ms + 'ms (p95=' + p95_ms + 'ms)')" "$benchmark_file" 2>/dev/null || echo "unavailable")"

# --------------------------------------------------------------------------
# End idempotence fixtures.
# --------------------------------------------------------------------------

if ! command -v git >/dev/null 2>&1; then
  echo "Installed OpenCode wrapper proof requires git on PATH to exercise scoped edit detection." >&2
  exit 1
fi

if [ "${PLUXX_CMD[0]}" = "pluxx" ]; then
  pluxx_bin="$(command -v pluxx)"
  node scripts/test-opencode-wrapper.mjs "$pluxx_bin"
else
  npx --yes --package "@orchid-labs/pluxx@$PLUXX_VERSION" -c '
    pluxx_bin="$(command -v pluxx)"
    node scripts/test-opencode-wrapper.mjs "$pluxx_bin"
  '
fi
