#!/usr/bin/env bash
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$ROOT"

PLUXX_VERSION="${PLUXX_VERSION:-0.1.36}"
if command -v pluxx >/dev/null 2>&1 && [ "$(pluxx --version)" = "$PLUXX_VERSION" ]; then
  PLUXX_CMD=(pluxx)
elif command -v npx >/dev/null 2>&1; then
  PLUXX_CMD=(npx --yes --package "@orchid-labs/pluxx@$PLUXX_VERSION" pluxx)
else
  echo "Skipping Pluxx hook fixture validation; missing pluxx and npx on PATH."
  exit 0
fi

"${PLUXX_CMD[@]}" lint --json >/tmp/mdp-pluxx-lint.json
find "$ROOT/scripts" -type d -name __pycache__ -prune -exec rm -rf {} +
find "$ROOT/dist" -type d -name __pycache__ -prune -exec rm -rf {} + 2>/dev/null || true
"${PLUXX_CMD[@]}" build --json >/tmp/mdp-pluxx-build.json
if find "$ROOT/dist" -type d -name __pycache__ | grep -q .; then
  echo "Generated Pluxx bundles must not include Python __pycache__ directories." >&2
  find "$ROOT/dist" -type d -name __pycache__ >&2
  exit 1
fi
python3 scripts/validate-skill-packaging.py --require-bundles >/tmp/mdp-skill-packaging.json

workspace_fixture="$(mktemp -d)"
plugin_fixture="$(mktemp -d)"
proposal_fixture="$(mktemp -d)"
trap 'rm -rf "$workspace_fixture" "$plugin_fixture" "$proposal_fixture"' EXIT
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
if ! printf '%s\n' "$proposal_output" | grep -F "MDP proposal audit readiness:" >/dev/null; then
  echo "MDP activation must print proposal audit readiness for proposal packs." >&2
  printf '%s\n' "$proposal_output" >&2
  exit 1
fi
if ! printf '%s\n' "$proposal_output" | grep -F "Local proposal runner: available in the plugin/source bundle." >/dev/null; then
  echo "MDP activation must report local proposal runner availability for proposal packs." >&2
  printf '%s\n' "$proposal_output" >&2
  exit 1
fi
if ! printf '%s\n' "$proposal_output" | grep -F "The local proposal runner is not a hosted MCP server" >/dev/null; then
  echo "MDP activation must avoid implying a hosted MCP server exists." >&2
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
  root_fallback_fixture="$(mktemp -d)"
  trap 'rm -rf "$workspace_fixture" "$plugin_fixture" "$proposal_fixture" "$root_fallback_fixture"' EXIT
  cp -R "$ROOT/plugin/assets/templates/basic/.mdp" "$root_fallback_fixture/.mdp"
  ln -s "$ROOT/cli" "$root_fallback_fixture/cli"
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
const lintResult = readJson('/tmp/mdp-pluxx-lint.json')

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

if [ "${PLUXX_CMD[0]}" = "pluxx" ]; then
  pluxx_bin="$(command -v pluxx)"
  node scripts/test-opencode-wrapper.mjs "$pluxx_bin"
else
  npx --yes --package "@orchid-labs/pluxx@$PLUXX_VERSION" -c '
    pluxx_bin="$(command -v pluxx)"
    node scripts/test-opencode-wrapper.mjs "$pluxx_bin"
  '
fi
