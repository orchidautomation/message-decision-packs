#!/usr/bin/env bash
set -euo pipefail

ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$ROOT"

if ! command -v pluxx >/dev/null 2>&1; then
  echo "Skipping Pluxx hook fixture validation; missing pluxx on PATH."
  exit 0
fi

pluxx lint --json >/tmp/mdp-pluxx-lint.json
pluxx build --json >/tmp/mdp-pluxx-build.json

workspace_fixture="$(mktemp -d)"
plugin_fixture="$(mktemp -d)"
trap 'rm -rf "$workspace_fixture" "$plugin_fixture"' EXIT
mkdir -p "$workspace_fixture/.mdp" "$plugin_fixture/.mdp"
printf 'name: hook-workspace-fixture\nversion: 0.1.0\n' >"$workspace_fixture/.mdp/manifest.yaml"
printf 'name: plugin-root-should-not-activate\nversion: 0.1.0\n' >"$plugin_fixture/.mdp/manifest.yaml"

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

console.log('Pluxx hook fixture validation passed.')
NODE
