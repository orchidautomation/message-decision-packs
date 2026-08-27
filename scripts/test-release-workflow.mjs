#!/usr/bin/env node

import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const workflowPath = join(root, '.github/workflows/release.yml')
const ciWorkflowPath = join(root, '.github/workflows/ci.yml')
const stepName = 'Install and smoke-test the published release'
const buildCommand = 'cargo build --manifest-path cli/Cargo.toml'
const smokeCommand = 'scripts/release-install-smoke.sh "$version"'
const requiredPrefix = [
  'set -euo pipefail',
  'version="${{ steps.version.outputs.version }}"',
  buildCommand,
]

function stepBlock(workflow, name) {
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
  return { lines, start, end, stepIndent }
}

function jobBlock(workflow, name) {
  const lines = workflow.split(/\r?\n/)
  const marker = `${name}:`
  const start = lines.findIndex(
    (line) => line.trim() === marker && line.match(/^\s*/u)[0].length === 2,
  )
  assert.notEqual(start, -1, `missing CI job: ${name}`)
  let end = lines.length
  for (let index = start + 1; index < lines.length; index += 1) {
    const trimmed = lines[index].trim()
    const indent = lines[index].match(/^\s*/u)[0].length
    if (trimmed && indent <= 2) {
      end = index
      break
    }
  }
  return lines.slice(start, end)
}

function assertDirectJobProperty(lines, property, expected) {
  const matches = lines.filter(
    (line) => line.match(/^\s*/u)[0].length === 4 && line.trim().startsWith(`${property}:`),
  )
  assert.deepEqual(matches.map((line) => line.trim()), [`${property}: ${expected}`])
}

function filterStepBlock(workflow) {
  const changes = jobBlock(workflow, 'changes')
  const start = changes.findIndex(
    (line) => line.match(/^\s*/u)[0].length === 6 && line.trim() === '- id: filter',
  )
  assert.notEqual(start, -1, 'changes job must contain the projected filter step')
  let end = changes.length
  for (let index = start + 1; index < changes.length; index += 1) {
    const trimmed = changes[index].trim()
    const indent = changes[index].match(/^\s*/u)[0].length
    if (trimmed.startsWith('- ') && indent === 6) {
      end = index
      break
    }
  }
  const step = changes.slice(start, end)
  assert.ok(
    step.some(
      (line) =>
        line.match(/^\s*/u)[0].length === 8 && line.trim() === 'uses: dorny/paths-filter@v4',
    ),
    'filter step must use dorny/paths-filter@v4',
  )
  assert.equal(
    step.find(
      (line) =>
        line.match(/^\s*/u)[0].length === 8 &&
        ['if:', 'continue-on-error:'].some((key) => line.trim().startsWith(key)),
    ),
    undefined,
    'projected paths-filter step must not be bypassable',
  )
  return step
}

function assertCliJobWiring(workflow) {
  const changes = jobBlock(workflow, 'changes')
  assert.equal(
    changes.find(
      (line) =>
        line.match(/^\s*/u)[0].length === 4 &&
        ['if:', 'continue-on-error:'].some((key) => line.trim().startsWith(key)),
    ),
    undefined,
    'changes job must not be bypassable',
  )
  const outputsIndex = changes.findIndex(
    (line) => line.match(/^\s*/u)[0].length === 4 && line.trim() === 'outputs:',
  )
  assert.notEqual(outputsIndex, -1, 'changes job must expose outputs')
  assert.equal(
    changes[outputsIndex + 1]?.trim(),
    'cli: ${{ steps.filter.outputs.cli }}',
    'changes.outputs.cli must project the paths-filter cli result',
  )

  const cli = jobBlock(workflow, 'cli')
  assertDirectJobProperty(cli, 'needs', 'changes')
  assertDirectJobProperty(cli, 'if', "needs.changes.outputs.cli == 'true'")
  assert.equal(
    cli.find(
      (line) =>
        line.match(/^\s*/u)[0].length === 4 && line.trim().startsWith('continue-on-error:'),
    ),
    undefined,
    'cli job must not ignore failures',
  )
  filterStepBlock(workflow)
}

function runBlock(workflow, name) {
  const { lines, start, end } = stepBlock(workflow, name)
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

function assertUnconditionalStep(workflow, name) {
  const { lines, start, end, stepIndent } = stepBlock(workflow, name)
  const control = lines
    .slice(start + 1, end)
    .find(
      (line) =>
        ['if:', 'continue-on-error:'].some((key) => line.trim().startsWith(key)) &&
        line.match(/^\s*/u)[0].length === stepIndent + 2,
    )
  assert.equal(control, undefined, `required CI step must not be bypassable: ${name}`)
}

function assertCliPathFilter(workflow, requiredGlob) {
  const lines = filterStepBlock(workflow)
  const filtersIndex = lines.findIndex((line) => line.trim() === 'filters: |')
  assert.notEqual(filtersIndex, -1, 'missing paths-filter filters block')
  const filtersIndent = lines[filtersIndex].match(/^\s*/u)[0].length
  const cliIndex = lines.findIndex(
    (line, index) =>
      index > filtersIndex &&
      line.trim() === 'cli:' &&
      line.match(/^\s*/u)[0].length > filtersIndent,
  )
  assert.notEqual(cliIndex, -1, 'missing cli paths-filter block')
  const cliIndent = lines[cliIndex].match(/^\s*/u)[0].length
  let end = lines.length
  for (let index = cliIndex + 1; index < lines.length; index += 1) {
    const trimmed = lines[index].trim()
    const indent = lines[index].match(/^\s*/u)[0].length
    if (trimmed && indent <= cliIndent) {
      end = index
      break
    }
  }
  const entries = lines.slice(cliIndex + 1, end).map((line) => line.trim())
  assert.ok(
    entries.includes(`- "${requiredGlob}"`),
    `cli paths-filter must include ${requiredGlob}`,
  )
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

function assertAssetParityCiContract(workflow) {
  assertCliJobWiring(workflow)
  assertCliPathFilter(workflow, 'plugin/assets/**')
  assertCliPathFilter(workflow, 'assets/**')
  const cliJob = jobBlock(workflow, 'cli').join('\n')
  assertUnconditionalStep(cliJob, 'Validate authored asset parity')
  const commands = runBlock(cliJob, 'Validate authored asset parity')
    .filter((line) => !line.startsWith('#'))
  assert.deepEqual(
    commands,
    ['make validate-asset-sync'],
    'required CI must execute authored asset parity unconditionally',
  )
}

const workflow = readFileSync(workflowPath, 'utf8')
assertReleaseSmokeContract(workflow)
const ciWorkflow = readFileSync(ciWorkflowPath, 'utf8')
assertAssetParityCiContract(ciWorkflow)

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


for (const [name, mutation] of [
  ['commented asset parity', ciWorkflow.replace('          make validate-asset-sync', '          # make validate-asset-sync')],
  ['echoed asset parity', ciWorkflow.replace('          make validate-asset-sync', '          echo make validate-asset-sync')],
  ['unreachable asset parity', ciWorkflow.replace('          make validate-asset-sync', '          if false; then\n            make validate-asset-sync\n          fi')],
  [
    'disabled asset parity step',
    ciWorkflow.replace(
      '      - name: Validate authored asset parity\n',
      '      - name: Validate authored asset parity\n        if: false\n',
    ),
  ],
  [
    'ignored asset parity failure',
    ciWorkflow.replace(
      '      - name: Validate authored asset parity\n',
      '      - name: Validate authored asset parity\n        continue-on-error: true\n',
    ),
  ],
  [
    'missing canonical asset filter',
    ciWorkflow.replace('              - "plugin/assets/**"\n', ''),
  ],
  [
    'missing packaged asset filter',
    ciWorkflow.replace('              - "assets/**"\n', ''),
  ],
  [
    'disconnected cli output',
    ciWorkflow.replace(
      '      cli: ${{ steps.filter.outputs.cli }}',
      '      cli: false',
    ),
  ],
  [
    'disabled cli job',
    ciWorkflow.replace(
      "    if: needs.changes.outputs.cli == 'true'",
      '    if: false',
    ),
  ],
  [
    'ignored cli job failure',
    ciWorkflow.replace(
      "    if: needs.changes.outputs.cli == 'true'\n",
      "    if: needs.changes.outputs.cli == 'true'\n    continue-on-error: true\n",
    ),
  ],
  [
    'cli job no longer needs changes',
    ciWorkflow.replace('    needs: changes', '    needs: pluxx'),
  ],
  [
    'renamed paths filter step',
    ciWorkflow.replace('      - id: filter', '      - id: filter2'),
  ],
  [
    'replaced paths filter action',
    ciWorkflow.replace('        uses: dorny/paths-filter@v4', '        uses: actions/checkout@v4'),
  ],
  [
    'disabled paths filter step',
    ciWorkflow.replace('      - id: filter\n', '      - id: filter\n        if: false\n'),
  ],
  [
    'disabled changes job',
    ciWorkflow.replace('  changes:\n    runs-on:', '  changes:\n    if: false\n    runs-on:'),
  ],
  [
    'parity step moved to disabled job',
    ciWorkflow
      .replace(
        '      - name: Validate authored asset parity\n        run: |\n          make validate-asset-sync\n',
        '',
      )
      .replace(
        '  mcp-macos:\n',
        '  dead-parity:\n    if: false\n    runs-on: ubuntu-latest\n    steps:\n      - name: Validate authored asset parity\n        run: |\n          make validate-asset-sync\n\n  mcp-macos:\n',
      ),
  ],
]) {
  assert.throws(() => assertAssetParityCiContract(mutation), undefined, name)
}

console.log('Release workflow and authored-asset CI contracts passed.')
