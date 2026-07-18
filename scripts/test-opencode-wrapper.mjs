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

const pluxxBin = process.argv[2]
assert(pluxxBin && existsSync(pluxxBin), 'Pass the exact Pluxx executable path as the first argument.')

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const tempRoot = mkdtempSync(join(tmpdir(), 'mdp-opencode-wrapper-'))
const releaseRoot = join(tempRoot, 'release')
const fakeBin = join(tempRoot, 'bin')
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
    PLUXX_TEST_RELEASE_ROOT: releaseRoot,
  }
  const publish = run(
    pluxxBin,
    ['publish', '--github-release', '--allow-dirty', '--json'],
    { cwd: root, environment: publishEnvironment },
  )
  const publishResult = JSON.parse(publish.stdout)
  assert(publishResult.ok, 'Pluxx must report a verified generated GitHub release asset set.')

  const manifest = JSON.parse(readFileSync(join(releaseRoot, 'release-manifest.json'), 'utf8'))
  const builtPlatforms = [...new Set(manifest.assets.archives.map((archive) => archive.platform))].sort()
  assert(
    JSON.stringify(builtPlatforms) === JSON.stringify(['claude-code', 'codex', 'cursor', 'opencode']),
    `Release manifest must include every supported host bundle; got ${builtPlatforms.join(', ')}.`,
  )

  const packageManifest = JSON.parse(readFileSync(join(root, 'dist/opencode/package.json'), 'utf8'))
  assert(manifest.plugin.version === packageManifest.version, 'Release and OpenCode package versions must match.')

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
