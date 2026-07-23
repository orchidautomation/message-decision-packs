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
  releaseWorkflow.includes('npm pack @orchid-labs/pluxx@0.1.36') &&
    releaseWorkflow.includes('npm install -g "$pluxx_tarball_path"') &&
    releaseWorkflow.includes(
      'sha512-23lcYezDYXG0FM5Ys9Nrw7oBvp6pX5StkeWeoDcBm7HZ/V3z3djA9y3p3KVXAGzhkubIEUUaC1kD7evKqBB1DQ==',
    ),
  'Release workflow must hash and install the same exact Pluxx 0.1.36 tarball.',
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
    releaseWorkflow.includes('release-assets/release-manifest.json'),
  `Release workflow must publish, download, stage, finalize, and upload in order; got ${releaseSequenceIndexes.join(', ')}.`,
)
assert(
  releaseWorkflow.includes('MDP_RELEASE_INSTALLER="release-assets/install.sh" scripts/release-install-smoke.sh "$version"'),
  'Release workflow must smoke-test the staged release installer with the documented agents path.',
)
const releaseInstallSmoke = readFileSync(join(root, 'scripts/release-install-smoke.sh'), 'utf8')
assert(
  releaseInstallSmoke.includes('MDP_RELEASE_INSTALL_ARGS:---agents -y') &&
    releaseInstallSmoke.includes('mdp-proposal-runner.mjs') &&
    releaseInstallSmoke.includes('mdp-native-normalize-openai.mjs') &&
    releaseInstallSmoke.includes('The local proposal runner is not a hosted MCP server') &&
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
  const finalizedChecksums = parseChecksums(join(releaseRoot, 'SHA256SUMS.txt'))
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

  const wrapper = await import(`${pathToFileURL(wrapperPath).href}?proof=${Date.now()}`)
  const pluginFactory = Object.values(wrapper).find((value) => typeof value === 'function')
  assert(pluginFactory, 'Installed OpenCode wrapper must export a plugin factory.')

  let hookCommand = ''
  let hookOutput = ''
  const shell = (strings, ...values) => {
    assert(strings.length === 2 && strings[0] === 'bash -lc ', 'Hook must execute through bash -lc.')
    hookCommand = String(values[0])
    const result = run('bash', ['-lc', hookCommand], {
      cwd: launchRoot,
      environment: process.env,
    })
    hookOutput = result.stdout
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

  console.log(
    `Installed OpenCode wrapper proof passed: launch=${launchRoot} selected=${selectedWorkspace}`,
  )
} finally {
  rmSync(tempRoot, { recursive: true, force: true })
}

process.exit(0)
