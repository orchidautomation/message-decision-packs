#!/usr/bin/env node

import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import {
  RELEASE_VERSION_PATHS,
  classifyGitDiff,
  isVersionOnlyRelease,
} from './classify-version-only-release.mjs'

function fixture(version) {
  return {
    'cli/Cargo.toml': `[package]\nname = "mdp"\nversion = "${version}"\nedition = "2024"\n`,
    'cli/Cargo.lock': `version = 4\n\n[[package]]\nname = "mdp"\nversion = "${version}"\ndependencies = [\n "anyhow",\n]\n`,
    'plugin/.codex-plugin/plugin.json': `{\n  "name": "message-decision-packs",\n  "version": "${version}",\n  "description": "fixture"\n}\n`,
    'pluxx.config.ts': `export default definePlugin({\n  name: 'message-decision-packs',\n  version: '${version}',\n})\n`,
  }
}

const beforeFiles = fixture('0.1.116')
const afterFiles = fixture('0.1.117')
const classify = (overrides = {}) => isVersionOnlyRelease({
  changedPaths: RELEASE_VERSION_PATHS,
  beforeFiles,
  afterFiles,
  ...overrides,
})

assert.equal(classify(), true)
assert.equal(classify({ changedPaths: RELEASE_VERSION_PATHS.slice(1) }), false)
assert.equal(classify({ changedPaths: [...RELEASE_VERSION_PATHS, 'README.md'] }), false)
assert.equal(classify({ changedPaths: [...RELEASE_VERSION_PATHS, 'cli/Cargo.toml'] }), true)
assert.equal(classify({ afterFiles: fixture('0.1.118') }), false)
assert.equal(classify({ afterFiles: fixture('1.0.0') }), false)

const dependencyEdit = fixture('0.1.117')
dependencyEdit['cli/Cargo.toml'] += 'serde = "2"\n'
assert.equal(classify({ afterFiles: dependencyEdit }), false)

const lockEdit = fixture('0.1.117')
lockEdit['cli/Cargo.lock'] = lockEdit['cli/Cargo.lock'].replace('"anyhow"', '"serde"')
assert.equal(classify({ afterFiles: lockEdit }), false)

const pluginEdit = fixture('0.1.117')
pluginEdit['plugin/.codex-plugin/plugin.json'] = pluginEdit['plugin/.codex-plugin/plugin.json']
  .replace('"fixture"', '"changed"')
assert.equal(classify({ afterFiles: pluginEdit }), false)

const mismatched = fixture('0.1.117')
mismatched['pluxx.config.ts'] = mismatched['pluxx.config.ts'].replace('0.1.117', '0.1.118')
assert.equal(classify({ afterFiles: mismatched }), false)

const repo = mkdtempSync(join(tmpdir(), 'mdp-release-classifier-'))
const git = (...args) => execFileSync('git', args, { cwd: repo, encoding: 'utf8' }).trim()
const writeFixture = (files) => {
  for (const [path, content] of Object.entries(files)) {
    mkdirSync(dirname(join(repo, path)), { recursive: true })
    writeFileSync(join(repo, path), content)
  }
}
try {
  git('init', '--initial-branch=main')
  git('config', 'user.name', 'MDP Release Classifier Test')
  git('config', 'user.email', 'mdp-release-classifier@example.invalid')
  writeFixture(beforeFiles)
  git('add', '.')
  git('commit', '-m', 'baseline')
  const beforeSha = git('rev-parse', 'HEAD')

  writeFixture(afterFiles)
  git('add', '.')
  git('commit', '-m', 'release bump')
  const releaseSha = git('rev-parse', 'HEAD')
  assert.equal(classifyGitDiff({ root: repo, before: beforeSha, after: releaseSha }), 'release-only')

  writeFileSync(join(repo, 'README.md'), 'extra semantic change\n')
  git('add', '.')
  git('commit', '-m', 'extra change')
  const extraSha = git('rev-parse', 'HEAD')
  assert.equal(classifyGitDiff({ root: repo, before: beforeSha, after: extraSha }), 'other')
  assert.throws(
    () => classifyGitDiff({ root: repo, before: 'not-a-sha', after: extraSha }),
    /before must be a full commit SHA/u,
  )
} finally {
  rmSync(repo, { recursive: true, force: true })
}

console.log('Version-only release classifier tests passed.')
