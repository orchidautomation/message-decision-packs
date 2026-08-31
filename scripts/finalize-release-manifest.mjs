#!/usr/bin/env node

import { createHash } from 'node:crypto'
import { existsSync, lstatSync, readFileSync, readdirSync, writeFileSync } from 'node:fs'
import { dirname, join, relative } from 'node:path'

const manifestPath = process.argv[2]
if (!manifestPath) {
  throw new Error('Pass the generated release-manifest.json path.')
}

const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
const archives = manifest?.assets?.archives
if (!Array.isArray(archives)) {
  throw new Error('Generated release manifest is missing assets.archives.')
}

const byPlatform = new Map()
for (const archive of archives) {
  const platform = archive?.platform
  if (typeof platform !== 'string' || platform.length === 0) {
    throw new Error('Generated release manifest contains an archive without a platform.')
  }
  const existing = byPlatform.get(platform)
  if (existing && JSON.stringify(existing) !== JSON.stringify(archive)) {
    throw new Error(`Generated release manifest contains conflicting ${platform} archives.`)
  }
  byPlatform.set(platform, archive)
}

const nativePlatforms = ['claude-code', 'cursor', 'codex', 'opencode']
const portablePlatform = 'agent-plugins'
const expectedPlatforms = [...nativePlatforms, portablePlatform]
const actualPlatforms = [...byPlatform.keys()].sort()
if (JSON.stringify(actualPlatforms) !== JSON.stringify([...expectedPlatforms].sort())) {
  throw new Error(
    `Generated release manifest platforms do not match MDP targets: ${actualPlatforms.join(', ')}.`,
  )
}

manifest.assets.archives = expectedPlatforms.map((platform) => byPlatform.get(platform))

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
const corpusPath = join(process.cwd(), 'plugin/assets/authority-conformance/corpus.json')
if (existsSync(corpusPath)) {
  const corpusBytes = readFileSync(corpusPath)
  const corpus = JSON.parse(corpusBytes)
  manifest.authority_conformance = {
    contract: corpus.contract,
    oracle: corpus.oracle,
    case_count: Array.isArray(corpus.cases) ? corpus.cases.length : 0,
    sha256: sha256(corpusBytes),
  }
}

const treeManifest = (root) => {
  const records = []
  const walk = (directory) => {
    for (const name of readdirSync(directory).sort()) {
      const path = join(directory, name)
      const stats = lstatSync(path)
      if (stats.isSymbolicLink()) {
        throw new Error(`Generated plugin tree contains a symbolic link: ${relative(root, path)}.`)
      }
      if (stats.isDirectory()) {
        walk(path)
      } else if (stats.isFile()) {
        records.push({
          path: relative(root, path).split('\\').join('/'),
          executable: (stats.mode & 0o111) !== 0,
          sha256: sha256(readFileSync(path)),
        })
      }
    }
  }
  walk(root)
  return { files: records, sha256: sha256(Buffer.from(`${JSON.stringify(records)}\n`)) }
}

const hostTrees = {}
for (const platform of nativePlatforms) {
  const root = join(process.cwd(), 'dist', platform)
  if (existsSync(root)) hostTrees[platform] = treeManifest(root)
}
if (Object.keys(hostTrees).length > 0) {
  if (Object.keys(hostTrees).length !== nativePlatforms.length) {
    throw new Error('Generated plugin tree inventory is incomplete.')
  }
  manifest.plugin_trees = hostTrees
}

const portableRoot = join(process.cwd(), 'dist', portablePlatform)
if (!existsSync(portableRoot)) {
  throw new Error('Generated Agent Plugins portable package is missing.')
}
{
  const expectedSkills = [
    'mdp',
    'mdp-gtm-brief',
    'mdp-pack-builder',
    'mdp-pack-review',
    'mdp-proposal-review',
  ]
  const topLevel = readdirSync(portableRoot).sort()
  const allowedTopLevel = ['plugin.json', 'skills']
  if (existsSync(join(portableRoot, 'mcp.json'))) allowedTopLevel.push('mcp.json')
  if (JSON.stringify(topLevel) !== JSON.stringify(allowedTopLevel.sort())) {
    throw new Error(
      `Agent Plugins package has native-only or unexpected top-level content: ${topLevel.join(', ')}.`,
    )
  }

  const pluginManifest = JSON.parse(readFileSync(join(portableRoot, 'plugin.json'), 'utf8'))
  if (
    pluginManifest?.$schema !== 'https://agent-plugins.org/schemas/1.0.0/plugin.schema.json' ||
    pluginManifest?.name !== 'message-decision-packs' ||
    pluginManifest?.version !== manifest?.plugin?.version
  ) {
    throw new Error('Agent Plugins plugin.json does not match the MDP release identity.')
  }

  const skillsRoot = join(portableRoot, 'skills')
  const skills = readdirSync(skillsRoot)
    .filter((name) => lstatSync(join(skillsRoot, name)).isDirectory())
    .sort()
  if (JSON.stringify(skills) !== JSON.stringify([...expectedSkills].sort())) {
    throw new Error(`Agent Plugins package skill inventory is not the five supported MDP skills: ${skills.join(', ')}.`)
  }
  for (const skill of skills) {
    if (!existsSync(join(skillsRoot, skill, 'SKILL.md'))) {
      throw new Error(`Agent Plugins package is missing skills/${skill}/SKILL.md.`)
    }
  }

  if (existsSync(join(portableRoot, 'mcp.json'))) {
    throw new Error('MDP does not declare portable MCP; generated agent-plugins/mcp.json is unexpected.')
  }

  manifest.portable_packages = {
    [portablePlatform]: {
      contract: 'mdp.agent-plugins-portable-package.v1',
      specification: '1.0.0',
      skills,
      mcp_servers: [],
      ...treeManifest(portableRoot),
    },
  }
}

const releaseRoot = dirname(manifestPath)
const cliAssets = readdirSync(releaseRoot)
  .filter((name) => name.startsWith('mdp-') && existsSync(join(releaseRoot, name)))
  .sort()
  .map((name) => ({ name, sha256: sha256(readFileSync(join(releaseRoot, name))) }))
if (cliAssets.length > 0) manifest.cli_artifacts = cliAssets

writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)
