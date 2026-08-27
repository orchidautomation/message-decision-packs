#!/usr/bin/env node

import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const workflowPath = join(root, '.github/workflows/release.yml')
const stepName = 'Install and smoke-test the published release'
const buildCommand = 'cargo build --manifest-path cli/Cargo.toml'
const smokeCommand = 'scripts/release-install-smoke.sh "$version"'
const requiredPrefix = [
  'set -euo pipefail',
  'version="${{ steps.version.outputs.version }}"',
  buildCommand,
]

function runBlock(workflow, name) {
  const lines = workflow.split(/\r?\n/)
  const marker = `- name: ${name}`
  const start = lines.findIndex((line) => line.trim() === marker)
  assert.notEqual(start, -1, `missing release workflow step: ${name}`)
  const stepIndent = lines[start].match(/^\s*/u)[0].length
  let end = lines.length
  for (let index = start + 1; index < lines.length; index += 1) {
    const trimmed = lines[index].trim()
    const indent = lines[index].match(/^\s*/u)[0].length
    if (trimmed.startsWith('- name:') && indent === stepIndent) {
      end = index
      break
    }
  }
  const runIndex = lines.findIndex(
    (line, index) => index > start && index < end && line.trim() === 'run: |',
  )
  assert.ok(runIndex > start && runIndex < end, `missing executable run block: ${name}`)
  const runIndent = lines[runIndex].match(/^\s*/u)[0].length
  return lines
    .slice(runIndex + 1, end)
    .filter((line) => line.trim() && line.match(/^\s*/u)[0].length > runIndent)
    .map((line) => line.trim())
}

function assertReleaseSmokeContract(workflow) {
  const commands = runBlock(workflow, stepName).filter((line) => !line.startsWith('#'))
  const buildIndex = commands.indexOf(buildCommand)
  const smokeIndex = commands.indexOf(smokeCommand)
  assert.deepEqual(
    commands.slice(0, requiredPrefix.length),
    requiredPrefix,
    'source CLI build must be an unconditional top-level command after strict setup',
  )
  assert.notEqual(buildIndex, -1, 'release smoke step must execute the exact source CLI build')
  assert.notEqual(smokeIndex, -1, 'release smoke step must execute the published smoke')
  assert.ok(buildIndex < smokeIndex, 'source CLI build must execute before published smoke')
}

const workflow = readFileSync(workflowPath, 'utf8')
assertReleaseSmokeContract(workflow)

for (const [name, mutation] of [
  ['commented build', workflow.replace(buildCommand, `# ${buildCommand}`)],
  ['echoed build', workflow.replace(buildCommand, `echo ${buildCommand}`)],
  [
    'unreachable build',
    workflow.replace(
      `          ${buildCommand}`,
      `          if false; then\n            ${buildCommand}\n          fi`,
    ),
  ],
  [
    'late build',
    workflow
      .replace(`          ${buildCommand}\n`, '')
      .replace(`            ${smokeCommand}`, `            ${smokeCommand}\n          ${buildCommand}`),
  ],
]) {
  assert.throws(() => assertReleaseSmokeContract(mutation), undefined, name)
}

console.log('Release workflow source-build contract passed.')
