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

const expectedPlatforms = ['claude-code', 'cursor', 'codex', 'opencode']
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
for (const platform of expectedPlatforms) {
  const root = join(process.cwd(), 'dist', platform)
  if (existsSync(root)) hostTrees[platform] = treeManifest(root)
}
if (Object.keys(hostTrees).length > 0) {
  if (Object.keys(hostTrees).length !== expectedPlatforms.length) {
    throw new Error('Generated plugin tree inventory is incomplete.')
  }
  manifest.plugin_trees = hostTrees
}

const releaseRoot = dirname(manifestPath)
const cliAssets = readdirSync(releaseRoot)
  .filter((name) => name.startsWith('mdp-') && existsSync(join(releaseRoot, name)))
  .sort()
  .map((name) => ({ name, sha256: sha256(readFileSync(join(releaseRoot, name))) }))
if (cliAssets.length > 0) manifest.cli_artifacts = cliAssets

writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)
