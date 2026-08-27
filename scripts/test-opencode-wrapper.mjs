#!/usr/bin/env node

import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { createHash } from 'node:crypto'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { spawnSync } from 'node:child_process'

const assert = (condition, message) => {
  if (!condition) throw new Error(message)
}

const run = (command, args, options = {}) => {
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    env: options.environment,
    encoding: 'utf8',
  })
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(' ')} failed (${result.status})\n${result.stdout}${result.stderr}`,
    )
  }
  return result
}

const sha256 = (filepath) =>
  createHash('sha256').update(readFileSync(filepath)).digest('hex')

const parseChecksums = (filepath) =>
  new Map(
    readFileSync(filepath, 'utf8')
      .trim()
      .split('\n')
      .map((line) => {
        const separator = line.indexOf('  ')
        assert(separator > 0, `Invalid checksum record in ${filepath}: ${line}`)
        return [line.slice(separator + 2), line.slice(0, separator)]
      }),
  )

const pluxxBin = process.argv[2]
assert(pluxxBin && existsSync(pluxxBin), 'Pass the exact Pluxx executable path as the first argument.')

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const sourceVersion = JSON.parse(
  readFileSync(join(root, 'plugin/.codex-plugin/plugin.json'), 'utf8'),
).version
const releaseWorkflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
const publishCommands = releaseWorkflow.match(/pluxx publish --github-release/g) ?? []
assert(
  publishCommands.length === 1 && !releaseWorkflow.includes('pluxx publish --github-release --dry-run'),
  'Release workflow must publish once so generated manifest entries are not duplicated.',
)
assert(
  releaseWorkflow.includes('npm pack @orchid-labs/pluxx@0.1.40') &&
    releaseWorkflow.includes('npm install -g "$pluxx_tarball_path"') &&
    releaseWorkflow.includes(
      'sha512-Q+jPmsq/vzApk9nWJE6o5UqV2Scddsf7rl5tsehX/n8A/D8QXGN9NQgAXgdOAKI6cw/DyDhDmnUnv92NsvfS5g==',
    ),
  'Release workflow must hash and install the same exact Pluxx 0.1.40 tarball.',
)
assert(
  releaseWorkflow.includes('npm pack @openai/codex@0.148.0') &&
    releaseWorkflow.includes(
      'sha512-bh5kH9+BMrFaHGmLeoSansPdfRksvr4UXzjQInns/KRO7r8VJ+6AAW+SqUsE8XcG3+OW/mI4EEy8Gpo9UDXGvQ==',
    ) &&
    releaseWorkflow.includes('test "$(codex --version)" = "codex-cli 0.148.0"'),
  'Release workflow must hash, install, and identify the exact native Codex CLI used by smoke.',
)
const releaseSequence = [
  'pluxx publish --github-release --allow-dirty --version "$version"',
  'gh release download "v$version" --dir release-assets',
  'cp scripts/install.sh release-assets/install.sh',
  'scripts/finalize-release-assets.sh release-assets',
  'gh release upload "v$version"',
]
const releaseSequenceIndexes = releaseSequence.map((command) => releaseWorkflow.indexOf(command))
assert(
  releaseSequenceIndexes.every((index) => index >= 0) &&
    releaseSequenceIndexes.every((index, position) =>
      position === 0 ? true : releaseSequenceIndexes[position - 1] < index,
    ) &&
    releaseWorkflow.includes('release-assets/SHA256SUMS.txt') &&
    releaseWorkflow.includes('release-assets/install-codex.sh') &&
    releaseWorkflow.includes('release-assets/release-manifest.json'),
  `Release workflow must publish, download, stage, finalize, and upload in order; got ${releaseSequenceIndexes.join(', ')}.`,
)
assert(
  releaseWorkflow.includes('MDP_RELEASE_REQUIRE_STAGED_PARITY=1') &&
    releaseWorkflow.includes('MDP_RELEASE_INSTALLER="release-assets/install.sh"') &&
    releaseWorkflow.includes('scripts/release-install-smoke.sh "$version"'),
  'Release workflow must compare and smoke-test the exact staged release artifacts.',
)
const releaseInstallSmoke = readFileSync(join(root, 'scripts/release-install-smoke.sh'), 'utf8')
assert(
  releaseInstallSmoke.includes('MDP_RELEASE_INSTALL_ARGS:---agents -y') &&
    releaseInstallSmoke.includes('mdp-proposal-runner.mjs') &&
    releaseInstallSmoke.includes('mdp-native-model-openai.mjs') &&
    releaseInstallSmoke.includes('mdp-native-normalize-openai.mjs') &&
    releaseInstallSmoke.includes('driver-request-v2') &&
    releaseInstallSmoke.includes('driver-result-v2') &&
    releaseInstallSmoke.includes('mdp-run-mcp-server.mjs') &&
    releaseInstallSmoke.includes('MCP path: mdp_run_tools -> mdp_prepare_run -> mdp_run -> mdp_verify_run.') &&
    releaseInstallSmoke.includes('The canonical MCP is local stdio transport only, not a hosted or remote MCP service') &&
    releaseInstallSmoke.includes('Hooks report readiness only; the CLI receipt is the blocking gate.'),
  'Release install smoke must exercise the documented --agents installer path and installed runner guardrails.',
)
const tempRoot = mkdtempSync(join(tmpdir(), 'mdp-opencode-wrapper-'))
const remoteReleaseRoot = join(tempRoot, 'remote-release')
const releaseRoot = join(tempRoot, 'release')
const fakeBin = join(tempRoot, 'bin')
mkdirSync(remoteReleaseRoot, { recursive: true })
mkdirSync(releaseRoot, { recursive: true })
mkdirSync(fakeBin, { recursive: true })

const fakeGhPath = join(fakeBin, 'gh')
writeFileSync(
  fakeGhPath,
  `#!/usr/bin/env node
const fs = require('node:fs')
const path = require('node:path')
const args = process.argv.slice(2)
const releaseRoot = process.env.PLUXX_TEST_RELEASE_ROOT
const assets = () => fs.existsSync(releaseRoot)
  ? fs.readdirSync(releaseRoot).sort().map((name) => ({ name }))
  : []

if (args[0] === 'auth' && args[1] === 'status') process.exit(0)

if (args[0] === 'release' && args[1] === 'view') {
  if (args.includes('tagName,assets')) {
    process.stdout.write(JSON.stringify({ tagName: args[2], assets: assets() }))
    process.exit(0)
  }
  process.stderr.write('release not found')
  process.exit(1)
}

if (args[0] === 'release' && args[1] === 'create') {
  const optionIndex = args.findIndex((value, index) => index > 2 && value.startsWith('--'))
  const files = args.slice(3, optionIndex === -1 ? args.length : optionIndex)
  for (const filepath of files) fs.copyFileSync(filepath, path.join(releaseRoot, path.basename(filepath)))
  process.stdout.write('captured generated release assets')
  process.exit(0)
}

if (args[0] === 'release' && args[1] === 'download') {
  const directory = args[args.indexOf('--dir') + 1]
  fs.mkdirSync(directory, { recursive: true })
  for (const asset of assets()) {
    fs.copyFileSync(path.join(releaseRoot, asset.name), path.join(directory, asset.name))
  }
  process.exit(0)
}

process.stderr.write('Unexpected gh invocation: ' + args.join(' '))
process.exit(1)
`,
)
chmodSync(fakeGhPath, 0o755)

const fakeCodexPath = join(fakeBin, 'codex')
writeFileSync(
  fakeCodexPath,
  `#!/usr/bin/env node
const fs = require('node:fs')
const path = require('node:path')
const args = process.argv.slice(2)
const selector = 'message-decision-packs@message-decision-packs-local'
if (args[0] === 'plugin' && args[1] === 'add' && args[2] === selector && args[3] === '--json') {
  if (process.env.PLUXX_TEST_CODEX_FAILURE === 'add') {
    const cache = path.join(
      process.env.CODEX_HOME,
      'plugins/cache/message-decision-packs-local/message-decision-packs',
    )
    fs.mkdirSync(cache, { recursive: true })
    fs.writeFileSync(path.join(cache, 'partial'), 'partial native cache')
    fs.appendFileSync(process.env.PLUXX_CODEX_CONFIG_PATH, '\\npartial_native_registration = true\\n')
    process.exit(17)
  }
  process.stdout.write(JSON.stringify({ pluginId: selector }))
  process.exit(0)
}
if (args[0] === 'plugin' && args[1] === 'list' && args[2] === '--json') {
  process.stdout.write(JSON.stringify({
    installed: [{
      pluginId: selector,
      installed: true,
      enabled: true,
      version: process.env.PLUXX_TEST_PLUGIN_VERSION,
      source: { source: 'local', path: process.env.PLUXX_CODEX_INSTALL_DIR },
    }],
  }))
  process.exit(0)
}
process.stderr.write('unexpected fake Codex invocation: ' + args.join(' '))
process.exit(1)
`,
)
chmodSync(fakeCodexPath, 0o755)
const fakeMdpPath = join(fakeBin, 'mdp')
writeFileSync(
  fakeMdpPath,
  `#!/usr/bin/env bash
echo "mdp 0.0.0-pluxx-installer-test"
`,
)
chmodSync(fakeMdpPath, 0o755)

try {
  const publishEnvironment = {
    ...process.env,
    PATH: `${fakeBin}:${process.env.PATH}`,
    PLUXX_TEST_RELEASE_ROOT: remoteReleaseRoot,
  }
  const publish = run(
    pluxxBin,
    ['publish', '--github-release', '--allow-dirty', '--json'],
    { cwd: root, environment: publishEnvironment },
  )
  const publishResult = JSON.parse(publish.stdout)
  assert(publishResult.ok, 'Pluxx must report a verified generated GitHub release asset set.')

  run(fakeGhPath, ['release', 'download', `v${sourceVersion}`, '--dir', releaseRoot], {
    cwd: root,
    environment: publishEnvironment,
  })

  const manifestPath = join(releaseRoot, 'release-manifest.json')
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
  const archivePlatforms = manifest.assets.archives.map((archive) => archive.platform)
  const builtPlatforms = [...new Set(archivePlatforms)].sort()
  assert(
    JSON.stringify(builtPlatforms) === JSON.stringify(['claude-code', 'codex', 'cursor', 'opencode']),
    `Release manifest must include every supported host bundle; got ${builtPlatforms.join(', ')}.`,
  )
  const packageManifest = JSON.parse(readFileSync(join(root, 'dist/opencode/package.json'), 'utf8'))
  assert(manifest.plugin.version === packageManifest.version, 'Release and OpenCode package versions must match.')

  manifest.assets.archives.push({ ...manifest.assets.archives[0] })
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)

  const generatedChecksums = parseChecksums(join(releaseRoot, 'SHA256SUMS.txt'))
  const generatedInstallChecksum = generatedChecksums.get('install.sh')
  const stagedInstallPath = join(releaseRoot, 'install.sh')
  const stagedInstall = `${readFileSync(join(root, 'scripts/install.sh'), 'utf8')}\n# checksum refresh fixture\n`
  writeFileSync(stagedInstallPath, stagedInstall)
  for (const target of [
    'aarch64-apple-darwin',
    'x86_64-apple-darwin',
    'x86_64-unknown-linux-gnu',
  ]) {
    writeFileSync(join(releaseRoot, `mdp-${target}`), `mdp ${target}\n`)
  }
  run('bash', [join(root, 'scripts/finalize-release-assets.sh'), releaseRoot], { cwd: root })
  const finalizedManifest = JSON.parse(
    readFileSync(join(releaseRoot, 'release-manifest.json'), 'utf8'),
  )
  const finalizedPlatforms = finalizedManifest.assets.archives.map((archive) => archive.platform)
  assert(
    JSON.stringify(finalizedPlatforms) ===
      JSON.stringify(['claude-code', 'cursor', 'codex', 'opencode']) &&
      new Set(finalizedPlatforms).size === finalizedPlatforms.length,
    `Finalized release manifest must list each host archive once; got ${finalizedPlatforms.join(', ')}.`,
  )
  assert(
    finalizedManifest.authority_conformance?.contract ===
      'mdp.authority-conformance-corpus.v1' &&
      finalizedManifest.authority_conformance?.oracle === 'hand-authored' &&
      finalizedManifest.authority_conformance?.case_count >= 10,
    'Finalized release manifest must bind the independent authority corpus.',
  )
  assert(
    JSON.stringify(Object.keys(finalizedManifest.plugin_trees || {}).sort()) ===
      JSON.stringify(['claude-code', 'codex', 'cursor', 'opencode']) &&
      Object.values(finalizedManifest.plugin_trees).every(
        (tree) => Array.isArray(tree.files) && tree.files.length > 0 && /^[a-f0-9]{64}$/.test(tree.sha256),
      ),
    'Finalized release manifest must bind complete generated plugin trees.',
  )
  assert(
    Object.values(finalizedManifest.plugin_trees).every((tree) =>
      tree.files.some((entry) => entry.path.startsWith('skill-evals/')),
    ),
    'Finalized release manifest must bind the shared skill-evals tree.',
  )
  assert(
    finalizedManifest.cli_artifacts?.length === 3 &&
      finalizedManifest.cli_artifacts.every((asset) => /^[a-f0-9]{64}$/.test(asset.sha256)),
    'Finalized release manifest must bind exact staged CLI artifacts.',
  )
  const finalizedChecksums = parseChecksums(join(releaseRoot, 'SHA256SUMS.txt'))
  const finalizedCodexInstaller = readFileSync(join(releaseRoot, 'install-codex.sh'), 'utf8')
  assert(
    finalizedCodexInstaller.includes('# MDP_NATIVE_CODEX_REGISTRATION_V1') &&
      finalizedCodexInstaller.includes('codex plugin add "$plugin_selector" --json') &&
      finalizedCodexInstaller.includes('plugin.installed !== true') &&
      finalizedCodexInstaller.includes('plugin.enabled !== true') &&
      finalizedCodexInstaller.includes('sourcePath !== installDir'),
    'Finalized Codex installer must register and verify the native plugin.',
  )
  assert(
    finalizedChecksums.get('install.sh') === sha256(stagedInstallPath) &&
      finalizedChecksums.get('install.sh') !== generatedInstallChecksum,
    'Finalized plugin checksums must independently match the replaced install.sh.',
  )
  const cliChecksums = parseChecksums(join(releaseRoot, 'MDP_CLI_SHA256SUMS.txt'))
  const expectedCliAssets = [
    'mdp-aarch64-apple-darwin',
    'mdp-x86_64-apple-darwin',
    'mdp-x86_64-unknown-linux-gnu',
  ]
  assert(
    JSON.stringify([...cliChecksums.keys()].sort()) === JSON.stringify(expectedCliAssets) &&
      [...cliChecksums].every(
        ([asset, digest]) => !asset.includes('/') && digest === sha256(join(releaseRoot, asset)),
      ),
    'Published CLI checksums must contain exact portable basenames and matching digests.',
  )

  const conflictingManifestPath = join(tempRoot, 'conflicting-release-manifest.json')
  const conflictingManifest = structuredClone(finalizedManifest)
  conflictingManifest.assets.archives.push({
    ...conflictingManifest.assets.archives[0],
    latestAsset: 'conflicting-latest.tar.gz',
  })
  writeFileSync(conflictingManifestPath, `${JSON.stringify(conflictingManifest, null, 2)}\n`)
  const conflictResult = spawnSync(
    process.execPath,
    [join(root, 'scripts/finalize-release-manifest.mjs'), conflictingManifestPath],
    { cwd: root, encoding: 'utf8' },
  )
  assert(
    conflictResult.status !== 0 && conflictResult.stderr.includes('conflicting claude-code archives'),
    'Manifest finalization must reject conflicting duplicate platform metadata.',
  )

  const codexHome = join(tempRoot, 'installed', 'codex-home')
  const codexConfigPath = join(codexHome, '.codex/config.toml')
  const codexPluginRoot = join(codexHome, '.codex/plugins/message-decision-packs')
  run('bash', [join(releaseRoot, 'install-codex.sh')], {
    cwd: root,
    environment: {
      ...process.env,
      PATH: `${fakeBin}:${process.env.PATH}`,
      HOME: codexHome,
      CODEX_HOME: join(codexHome, '.codex'),
      MDP_SKIP_CLI_UPDATE: '1',
      PLUXX_CODEX_BUNDLE_PATH: join(
        releaseRoot,
        'message-decision-packs-codex-latest.tar.gz',
      ),
      PLUXX_CODEX_CONFIG_PATH: codexConfigPath,
      PLUXX_CODEX_ENABLE_PLUGIN_HOOKS: '1',
      PLUXX_CODEX_INSTALL_DIR: codexPluginRoot,
      PLUXX_CODEX_MARKETPLACE_PATH: join(codexHome, '.agents/plugins/marketplace.json'),
      PLUXX_TEST_PLUGIN_VERSION: sourceVersion,
      PLUXX_INSTALL_LOCK_ROOT: join(codexHome, '.pluxx/install-locks'),
      PLUXX_RUNTIME_STORE_ROOT: join(codexHome, '.pluxx/runtimes'),
    },
  })

  assert(
    existsSync(join(codexPluginRoot, 'scripts/mdp-proposal-runner.mjs')),
    'Generated Codex installer must install the local proposal runner.',
  )
  assert(
    existsSync(join(codexPluginRoot, 'scripts/mdp-native-model-openai.mjs')),
    'Generated Codex installer must install the universal native model driver.',
  )
  assert(
    existsSync(join(codexPluginRoot, 'scripts/mdp-native-normalize-openai.mjs')),
    'Generated Codex installer must install the native normalization runner.',
  )
  assert(
    readFileSync(codexConfigPath, 'utf8').includes('hooks = true'),
    'Generated Codex installer must enable hooks in the isolated Codex config path.',
  )
  const codexMarketplacePath = join(codexHome, '.agents/plugins/marketplace.json')
  const nativeCachePath = join(
    codexHome,
    '.codex/plugins/cache/message-decision-packs-local/message-decision-packs',
  )
  mkdirSync(nativeCachePath, { recursive: true })
  writeFileSync(join(nativeCachePath, 'sentinel'), 'previous native cache')
  const beforeFailedRegistration = {
    config: readFileSync(codexConfigPath, 'utf8'),
    marketplace: readFileSync(codexMarketplacePath, 'utf8'),
    runner: sha256(join(codexPluginRoot, 'scripts/mdp-proposal-runner.mjs')),
  }
  const failedRegistration = spawnSync('bash', [join(releaseRoot, 'install-codex.sh')], {
    cwd: root,
    env: {
      ...process.env,
      PATH: `${fakeBin}:${process.env.PATH}`,
      HOME: codexHome,
      CODEX_HOME: join(codexHome, '.codex'),
      MDP_SKIP_CLI_UPDATE: '1',
      PLUXX_CODEX_BUNDLE_PATH: join(
        releaseRoot,
        'message-decision-packs-codex-latest.tar.gz',
      ),
      PLUXX_CODEX_CONFIG_PATH: codexConfigPath,
      PLUXX_CODEX_ENABLE_PLUGIN_HOOKS: '1',
      PLUXX_CODEX_INSTALL_DIR: codexPluginRoot,
      PLUXX_CODEX_MARKETPLACE_PATH: codexMarketplacePath,
      PLUXX_TEST_PLUGIN_VERSION: sourceVersion,
      PLUXX_TEST_CODEX_FAILURE: 'add',
      PLUXX_INSTALL_LOCK_ROOT: join(codexHome, '.pluxx/install-locks'),
      PLUXX_RUNTIME_STORE_ROOT: join(codexHome, '.pluxx/runtimes'),
    },
    encoding: 'utf8',
  })
  assert(failedRegistration.status !== 0, 'Native Codex registration failure must fail install.')
  assert(
    readFileSync(codexConfigPath, 'utf8') === beforeFailedRegistration.config,
    'Native Codex registration failure must restore the prior config.',
  )
  assert(
    readFileSync(codexMarketplacePath, 'utf8') === beforeFailedRegistration.marketplace,
    'Native Codex registration failure must restore the prior marketplace.',
  )
  assert(
    sha256(join(codexPluginRoot, 'scripts/mdp-proposal-runner.mjs')) ===
      beforeFailedRegistration.runner,
    'Native Codex registration failure must restore the prior plugin install.',
  )
  assert(
    readFileSync(join(nativeCachePath, 'sentinel'), 'utf8') === 'previous native cache',
    'Native Codex registration failure must restore the prior native cache.',
  )
  assert(!existsSync(join(nativeCachePath, 'partial')), 'Partial native cache must be removed.')
  const codexTools = run('node', [join(codexPluginRoot, 'scripts/mdp-proposal-runner.mjs'), 'tools'], {
    cwd: root,
  }).stdout
  assert(
    codexTools.includes('mdp_run_receipt') &&
      codexTools.includes('bundled local stdio MCP wrapper') &&
      codexTools.includes('hosted or remote MCP'),
    'Installed Codex runner tools must preserve receipt and local/hosted MCP guardrails.',
  )
  const codexPycache = spawnSync(
    'bash',
    ['-lc', `find "${codexPluginRoot}" -type d -name __pycache__ -print -quit`],
    { cwd: root, encoding: 'utf8' },
  )
  assert(codexPycache.status === 0, 'Generated Codex installed bundle pycache scan must run.')
  assert(
    codexPycache.stdout.trim() === '',
    'Generated Codex installed bundle must not contain Python __pycache__ directories.',
  )
  console.log(`Generated Codex installer proof passed: codexHome=${codexHome}`)

  const installRoot = join(tempRoot, 'installed', 'plugins')
  const installedPluginRoot = join(installRoot, 'message-decision-packs')
  const wrapperPath = join(installRoot, 'message-decision-packs.ts')
  const skillsRoot = join(tempRoot, 'installed', 'skills')
  mkdirSync(installRoot, { recursive: true })
  mkdirSync(skillsRoot, { recursive: true })

  run('bash', [join(releaseRoot, 'install-opencode.sh')], {
    cwd: root,
    environment: {
      ...process.env,
      PLUXX_OPENCODE_BUNDLE_PATH: join(
        releaseRoot,
        'message-decision-packs-opencode-latest.tar.gz',
      ),
      PLUXX_OPENCODE_PLUGIN_ROOT_DIR: installRoot,
      PLUXX_OPENCODE_INSTALL_DIR: installedPluginRoot,
      PLUXX_OPENCODE_ENTRY_PATH: wrapperPath,
      PLUXX_OPENCODE_SKILLS_ROOT: skillsRoot,
    },
  })

  assert(existsSync(wrapperPath), 'Generated installer must write the top-level OpenCode wrapper.')
  const resolvedPluginRoot = realpathSync(installedPluginRoot)

  const launchRoot = join(tempRoot, 'parent-launch')
  const selectedWorkspace = join(launchRoot, 'selected-workspace')
  mkdirSync(join(selectedWorkspace, '.mdp'), { recursive: true })
  copyFileSync(
    join(root, 'plugin/assets/templates/basic/.mdp/manifest.yaml'),
    join(selectedWorkspace, '.mdp/manifest.yaml'),
  )
  run('git', ['init', '-q'], { cwd: selectedWorkspace, environment: process.env })
  run('git', ['config', 'user.email', 'mdp-hook-proof@example.invalid'], {
    cwd: selectedWorkspace,
    environment: process.env,
  })
  run('git', ['config', 'user.name', 'MDP Hook Proof'], {
    cwd: selectedWorkspace,
    environment: process.env,
  })
  run('git', ['add', '.mdp/manifest.yaml'], {
    cwd: selectedWorkspace,
    environment: process.env,
  })
  run('git', ['commit', '-q', '-m', 'fixture baseline'], {
    cwd: selectedWorkspace,
    environment: process.env,
  })

  const wrapper = await import(`${pathToFileURL(wrapperPath).href}?proof=${Date.now()}`)
  const pluginFactory = Object.values(wrapper).find((value) => typeof value === 'function')
  assert(pluginFactory, 'Installed OpenCode wrapper must export a plugin factory.')

  let hookCommand = ''
  let hookOutput = ''
  const hookInvocations = []
  const shell = (strings, ...values) => {
    assert(strings.length === 2 && strings[0] === 'bash -lc ', 'Hook must execute through bash -lc.')
    hookCommand = String(values[0])
    const result = run('bash', ['-lc', hookCommand], {
      cwd: launchRoot,
      environment: {
        ...process.env,
        PATH: `${fakeBin}:${process.env.PATH}`,
      },
    })
    hookOutput = result.stdout
    hookInvocations.push({ command: hookCommand, output: hookOutput })
    return Promise.resolve(result)
  }
  const client = { app: { log: async () => undefined } }
  const hooks = await pluginFactory({
    project: launchRoot,
    directory: selectedWorkspace,
    client,
    $: shell,
  })
  await hooks.event({ event: { type: 'session.created' } })

  assert(
    hookOutput.includes(`detected in ${selectedWorkspace}`),
    'Installed wrapper activation must detect the selected MDP workspace, not the parent launch directory.',
  )
  assert(
    hookCommand.includes(`; export PLUGIN_ROOT='${resolvedPluginRoot}';`),
    `Installed wrapper must preserve the installed plugin root; command was: ${hookCommand}`,
  )
  assert(
    hookCommand.includes(`PLUXX_HOOK_WORKSPACE_ROOT='${selectedWorkspace}'`),
    'Installed wrapper must preserve the selected workspace root.',
  )
  assert(!existsSync(join(launchRoot, '.mdp')), 'Parent launch directory must remain a non-MDP workspace.')

  hookInvocations.length = 0
  await hooks['tool.execute.after']({ tool: 'read' }, {})
  await hooks['tool.execute.after']({ tool: 'bash' }, {})
  assert(
    hookInvocations.length === 0,
    'Read and shell tools must not invoke MDP post-edit validation.',
  )

  writeFileSync(join(selectedWorkspace, 'notes.txt'), 'irrelevant edit\n')
  await hooks['tool.execute.after']({ tool: 'edit' }, {})
  assert(
    hookInvocations.length === 1 &&
      !hookInvocations[0].output.includes('MDP post-edit validation: relevant changes detected.'),
    'An irrelevant edit must invoke the scoped hook once and exit without pack validation.',
  )

  writeFileSync(
    join(selectedWorkspace, '.mdp/manifest.yaml'),
    `${readFileSync(join(selectedWorkspace, '.mdp/manifest.yaml'), 'utf8')}\n# relevant edit\n`,
  )
  await hooks['tool.execute.after']({ tool: 'apply_patch' }, {})
  assert(
    hookInvocations.length === 2 &&
      hookInvocations[1].output.includes('MDP post-edit validation: relevant changes detected.'),
    'A relevant MDP edit must invoke post-edit validation exactly once.',
  )

  console.log(
    `Installed OpenCode wrapper proof passed: launch=${launchRoot} selected=${selectedWorkspace}`,
  )
} finally {
  rmSync(tempRoot, { recursive: true, force: true })
}

process.exit(0)
