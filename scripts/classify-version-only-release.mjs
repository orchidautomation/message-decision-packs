#!/usr/bin/env node

import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

export const RELEASE_VERSION_PATHS = [
  'cli/Cargo.lock',
  'cli/Cargo.toml',
  'plugin/.codex-plugin/plugin.json',
  'pluxx.config.ts',
]

function singleMatch(text, pattern, label) {
  const matches = [...text.matchAll(pattern)]
  if (matches.length !== 1) throw new Error(`${label} must contain exactly one version authority`)
  return matches[0][1]
}

function versions(files) {
  return {
    cargoLock: singleMatch(
      files['cli/Cargo.lock'],
      /\[\[package\]\]\nname = "mdp"\nversion = "([^"]+)"/gu,
      'cli/Cargo.lock',
    ),
    cargoToml: singleMatch(files['cli/Cargo.toml'], /^version = "([^"]+)"$/gmu, 'cli/Cargo.toml'),
    plugin: singleMatch(
      files['plugin/.codex-plugin/plugin.json'],
      /^  "version": "([^"]+)",$/gmu,
      'plugin/.codex-plugin/plugin.json',
    ),
    pluxx: singleMatch(files['pluxx.config.ts'], /^  version: '([^']+)',$/gmu, 'pluxx.config.ts'),
  }
}

function replaceExactlyOnce(text, from, to, label) {
  const first = text.indexOf(from)
  if (first === -1 || text.indexOf(from, first + from.length) !== -1) {
    throw new Error(`${label} version token must occur exactly once`)
  }
  return `${text.slice(0, first)}${to}${text.slice(first + from.length)}`
}

function isPatchIncrement(before, after) {
  const semver = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/u
  const oldMatch = before.match(semver)
  const newMatch = after.match(semver)
  return Boolean(
    oldMatch &&
    newMatch &&
    oldMatch[1] === newMatch[1] &&
    oldMatch[2] === newMatch[2] &&
    Number(newMatch[3]) === Number(oldMatch[3]) + 1,
  )
}

export function isVersionOnlyRelease({ changedPaths, beforeFiles, afterFiles }) {
  try {
    const normalizedPaths = [...new Set(changedPaths)].sort()
    if (JSON.stringify(normalizedPaths) !== JSON.stringify(RELEASE_VERSION_PATHS)) return false

    const beforeVersions = versions(beforeFiles)
    const afterVersions = versions(afterFiles)
    const oldValues = new Set(Object.values(beforeVersions))
    const newValues = new Set(Object.values(afterVersions))
    if (oldValues.size !== 1 || newValues.size !== 1) return false
    const [oldVersion] = oldValues
    const [newVersion] = newValues
    if (!isPatchIncrement(oldVersion, newVersion)) return false

    const expected = {
      'cli/Cargo.toml': replaceExactlyOnce(
        beforeFiles['cli/Cargo.toml'],
        `version = "${oldVersion}"`,
        `version = "${newVersion}"`,
        'cli/Cargo.toml',
      ),
      'cli/Cargo.lock': replaceExactlyOnce(
        beforeFiles['cli/Cargo.lock'],
        `[[package]]\nname = "mdp"\nversion = "${oldVersion}"`,
        `[[package]]\nname = "mdp"\nversion = "${newVersion}"`,
        'cli/Cargo.lock',
      ),
      'plugin/.codex-plugin/plugin.json': replaceExactlyOnce(
        beforeFiles['plugin/.codex-plugin/plugin.json'],
        `  "version": "${oldVersion}",`,
        `  "version": "${newVersion}",`,
        'plugin/.codex-plugin/plugin.json',
      ),
      'pluxx.config.ts': replaceExactlyOnce(
        beforeFiles['pluxx.config.ts'],
        `  version: '${oldVersion}',`,
        `  version: '${newVersion}',`,
        'pluxx.config.ts',
      ),
    }
    return RELEASE_VERSION_PATHS.every((path) => expected[path] === afterFiles[path])
  } catch {
    return false
  }
}

function assertSha(value, label) {
  if (!/^[0-9a-f]{40}$/u.test(value ?? '')) throw new Error(`${label} must be a full commit SHA`)
}

function git(root, args) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8' })
}

export function classifyGitDiff({ root = process.cwd(), before, after }) {
  assertSha(before, 'before')
  assertSha(after, 'after')
  const changedPaths = git(root, [
    'diff', '--name-only', '--diff-filter=ACDMRTUXB', '--no-renames', before, after,
  ]).split(/\r?\n/u).filter(Boolean)
  const readFiles = (ref) => Object.fromEntries(
    RELEASE_VERSION_PATHS.map((path) => [path, git(root, ['show', `${ref}:${path}`])]),
  )
  return isVersionOnlyRelease({
    changedPaths,
    beforeFiles: readFiles(before),
    afterFiles: readFiles(after),
  }) ? 'release-only' : 'other'
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [, , before, after] = process.argv
  try {
    process.stdout.write(`${classifyGitDiff({ before, after })}\n`)
  } catch (error) {
    process.stderr.write(`${error.message}\n`)
    process.exitCode = 2
  }
}
