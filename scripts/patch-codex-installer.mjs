#!/usr/bin/env node

import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

const installerPath = resolve(process.argv[2] ?? '')
if (!process.argv[2]) {
  console.error('Usage: patch-codex-installer.mjs PATH_TO_INSTALL_CODEX_SH')
  process.exit(2)
}

const marker = '# MDP_NATIVE_CODEX_REGISTRATION_V1'
const source = readFileSync(installerPath, 'utf8')
if (source.includes(marker)) {
  console.error(`Codex installer is already patched: ${installerPath}`)
  process.exit(1)
}
const finalizeCall = '\npluxx_finalize_install_transaction\n'
const finalizeIndex = source.lastIndexOf(finalizeCall)
if (finalizeIndex < 0) {
  console.error(`Codex installer is missing the expected Pluxx transaction: ${installerPath}`)
  process.exit(1)
}

const registration = String.raw`

${marker}
if ! command -v codex >/dev/null 2>&1; then
  echo "Codex CLI is required to register $PLUGIN_NAME with the native plugin manager." >&2
  exit 1
fi

export CODEX_CONFIG_PATH CODEX_HOME_DIR INSTALL_DIR MARKETPLACE_PATH PLUGIN_NAME MARKETPLACE_NAME
marketplace_root="$(node <<'NODE'
const path = require('path')
const codexConfigPath = path.resolve(process.env.CODEX_CONFIG_PATH ?? '')
const codexHome = path.resolve(process.env.CODEX_HOME_DIR ?? '')
const installDir = path.resolve(process.env.INSTALL_DIR ?? '')
const marketplacePath = path.resolve(process.env.MARKETPLACE_PATH ?? '')
const pluginName = process.env.PLUGIN_NAME ?? ''
const marketplaceName = process.env.MARKETPLACE_NAME ?? ''
const safeIdentifier = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/
if (!safeIdentifier.test(pluginName) || !safeIdentifier.test(marketplaceName)) {
  console.error('Refusing unsafe Codex plugin or marketplace identifier.')
  process.exit(1)
}
const marketplaceRoot = path.dirname(path.dirname(path.dirname(marketplacePath)))
const expectedMarketplacePath = path.join(
  marketplaceRoot,
  '.agents',
  'plugins',
  'marketplace.json',
)
const expectedInstallDir = path.join(
  marketplaceRoot,
  '.codex',
  'plugins',
  pluginName,
)
if (
  codexConfigPath !== path.join(codexHome, 'config.toml') ||
  marketplacePath !== expectedMarketplacePath ||
  installDir !== expectedInstallDir
) {
  console.error(
    'Codex native registration requires matching native config, marketplace, and plugin paths.',
  )
  process.exit(1)
}
process.stdout.write(marketplaceRoot)
NODE
)"
marketplace_add_json="$(codex plugin marketplace add "$marketplace_root" --json)"
export MDP_CODEX_MARKETPLACE_ADD_JSON="$marketplace_add_json"
export MDP_CODEX_MARKETPLACE_ROOT="$marketplace_root"
registration_paths="$(node <<'NODE'
const fs = require('fs')
const path = require('path')
let payload
try {
  payload = JSON.parse(process.env.MDP_CODEX_MARKETPLACE_ADD_JSON ?? '')
} catch {
  console.error('Codex marketplace registration returned invalid JSON.')
  process.exit(1)
}
let installedRoot = ''
let expectedRoot = ''
try {
  installedRoot = fs.realpathSync(payload.installedRoot ?? '')
  expectedRoot = fs.realpathSync(process.env.MDP_CODEX_MARKETPLACE_ROOT ?? '')
} catch {}
if (installedRoot !== expectedRoot) {
  console.error('Codex marketplace registration did not bind the expected local root.')
  process.exit(1)
}
const marketplaceName = payload.marketplaceName ?? ''
const safeIdentifier = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/
if (!safeIdentifier.test(marketplaceName)) {
  console.error('Codex marketplace registration returned an unsafe marketplace identifier.')
  process.exit(1)
}
const pluginName = process.env.PLUGIN_NAME ?? ''
const cacheRoot = path.resolve(process.env.CODEX_HOME_DIR, 'plugins', 'cache')
const target = path.resolve(cacheRoot, marketplaceName, pluginName)
if (!target.startsWith(cacheRoot + path.sep)) {
  console.error('Refusing a native Codex cache path outside the cache root.')
  process.exit(1)
}
process.stdout.write(marketplaceName + '\t' + target)
NODE
)"
unset MDP_CODEX_MARKETPLACE_ADD_JSON MDP_CODEX_MARKETPLACE_ROOT
IFS=$'\t' read -r MARKETPLACE_NAME native_cache_path <<< "$registration_paths"
plugin_selector="$PLUGIN_NAME@$MARKETPLACE_NAME"
pluxx_tx_backup_owned_path "$native_cache_path"
if ! codex plugin add "$plugin_selector" --json >/dev/null; then
  echo "Failed to register $plugin_selector with the native Codex plugin manager." >&2
  exit 1
fi

plugin_list_json="$(codex plugin list --json)"
export MDP_CODEX_PLUGIN_SELECTOR="$plugin_selector"
export MDP_CODEX_PLUGIN_LIST_JSON="$plugin_list_json"
export MDP_CODEX_PLUGIN_INSTALL_DIR="$INSTALL_DIR"
node <<'NODE'
const fs = require('fs')
const path = require('path')
const selector = process.env.MDP_CODEX_PLUGIN_SELECTOR
const installDir = fs.realpathSync(process.env.MDP_CODEX_PLUGIN_INSTALL_DIR)
const manifest = JSON.parse(
  fs.readFileSync(path.join(installDir, '.codex-plugin/plugin.json'), 'utf8'),
)
let payload
try {
  payload = JSON.parse(process.env.MDP_CODEX_PLUGIN_LIST_JSON ?? '')
} catch {
  console.error('Codex plugin registration verification returned invalid JSON.')
  process.exit(1)
}
const installed = Array.isArray(payload.installed) ? payload.installed : []
const plugin = installed.find((candidate) => candidate?.pluginId === selector)
let sourcePath = ''
try {
  sourcePath = fs.realpathSync(plugin?.source?.path ?? '')
} catch {}
if (
  !plugin ||
  plugin.installed !== true ||
  plugin.enabled !== true ||
  plugin.version !== manifest.version ||
  sourcePath !== installDir
) {
  console.error(
    'Codex plugin registration did not report ' + selector + ' as installed and enabled.',
  )
  process.exit(1)
}
NODE
unset MDP_CODEX_PLUGIN_SELECTOR MDP_CODEX_PLUGIN_LIST_JSON MDP_CODEX_PLUGIN_INSTALL_DIR
echo "Registered $plugin_selector with the native Codex plugin manager."
`

const patched = `${source.slice(0, finalizeIndex)}${registration}${source.slice(finalizeIndex)}`
writeFileSync(installerPath, patched, { mode: 0o755 })
