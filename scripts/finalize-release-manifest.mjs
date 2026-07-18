#!/usr/bin/env node

import { readFileSync, writeFileSync } from 'node:fs'

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
writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)
