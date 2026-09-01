#!/usr/bin/env node

import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const workflowPath = join(root, '.github/workflows/release.yml')
const ciWorkflowPath = join(root, '.github/workflows/ci.yml')
const stepName = 'Install and smoke-test the published release'
const stagedSourceBinding = 'MDP_RELEASE_SOURCE_PARITY_BIN="$GITHUB_WORKSPACE/release-assets/mdp-x86_64-unknown-linux-gnu"'
const stagedParityFlag = 'MDP_RELEASE_REQUIRE_STAGED_PARITY=1 \\'
const requiredCacheAction = 'uses: Swatinem/rust-cache@v2'
const expectedCacheSharedKey = 'release-${{ matrix.os }}-${{ matrix.target }}'
const smokeCommand = 'scripts/release-install-smoke.sh "$version"'
const codexPackCommand = 'npm pack @openai/codex@0.148.0'
const codexVersionCommand = 'test "$(codex --version)" = "codex-cli 0.148.0"'
const assetParityCommand = '/usr/bin/env -i /usr/bin/diff -qr "${{ github.workspace }}/plugin/assets" "${{ github.workspace }}/assets"'
const neutralFixturePath = 'cli/tests/fixtures/profile-conformance/'
const neutralFixtureId = 'neutral'
const requiredPrefix = [
  'set -euo pipefail',
  'version="${{ steps.version.outputs.version }}"',
  'chmod +x release-assets/mdp-x86_64-unknown-linux-gnu',
]

function hasShellOverride(line) {
  if (hasMappingKey(line, 'shell')) {
    return true
  }
  if (/(?:^|[\s{,])shell\s*:/u.test(line.trim())) {
    return true
  }
  const quotedKeys = line.matchAll(
    /(?:^|[\s{,])((?:"(?:\\.|[^"\\])*")|(?:'(?:''|[^'])*'))\s*:/gu,
  )
  return [...quotedKeys].some((match) => hasMappingKey(`${match[1]}:`, 'shell'))
}

function hasMappingKey(line, key) {
  const trimmed = line.trim()
  if (trimmed.startsWith('?')) {
    // Explicit YAML keys admit quoted, escaped, commented, and block-scalar
    // forms. Required CI scopes do not need them, so reject all of them.
    return true
  }
  const terminator = '\\s*:'
  const doubleQuoted = trimmed.match(new RegExp(`^("(?:\\\\.|[^"\\\\])*")${terminator}`, 'u'))
  if (doubleQuoted) {
    try {
      return JSON.parse(doubleQuoted[1]) === key
    } catch {
      // YAML accepts additional escapes (for example \xNN and \UNNNNNNNN).
      // Fail closed on any double-quoted mapping key that JSON cannot decode.
      return doubleQuoted[1].includes('\\')
    }
  }
  const singleQuoted = trimmed.match(new RegExp(`^'((?:''|[^'])*)'${terminator}`, 'u'))
  if (singleQuoted) {
    return singleQuoted[1].replaceAll("''", "'") === key
  }
  const plain = trimmed.match(new RegExp(`^([^\\s:{},][^:]*)${terminator}`, 'u'))
  return plain?.[1].trim() === key
}

function hasBypassControl(line) {
  return hasMappingKey(line, 'if') || hasMappingKey(line, 'continue-on-error')
}

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
  assert.deepEqual(
    step
      .filter((line) => line.trim() && line.match(/^\s*/u)[0].length === 10)
      .map((line) => line.trim()),
    ['filters: |'],
    'paths-filter must use only the default-some filters input',
  )
  assert.equal(
    step.find(
      (line) =>
        line.match(/^\s*/u)[0].length === 8 &&
        hasBypassControl(line) || hasShellOverride(line),
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
        hasBypassControl(line) || hasShellOverride(line),
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
        line.match(/^\s*/u)[0].length === 4 && hasMappingKey(line, 'continue-on-error'),
    ),
    undefined,
    'cli job must not ignore failures',
  )
  assert.equal(
    cli.find(hasShellOverride),
    undefined,
    'cli job must not override run-command execution',
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
        (hasBypassControl(line) || hasShellOverride(line)) &&
        line.match(/^\s*/u)[0].length === stepIndent + 2,
    )
  assert.equal(control, undefined, `required CI step must not be bypassable: ${name}`)
}

function assertNoWorkflowShellOverride(workflow) {
  const lines = workflow.split(/\r?\n/)
  assert.equal(
    lines.find(hasShellOverride),
    undefined,
    'workflow must not override run-command execution',
  )
}

function assertNoWorkflowYamlIndirection(workflow) {
  const indirection = workflow
    .split(/\r?\n/)
    .find(
      (line) =>
        /(?:^|[\s{[,:])(?:&|\*)[A-Za-z0-9_-]+(?=\s|[,}\]:]|$)/u.test(line) ||
        /(?:^|[\s{[,:])!(?:!|<)?[A-Za-z0-9_:/.-]+>?/u.test(line) ||
        /(?:^|[\s{[,:])!(?=\s)/u.test(line),
    )
  assert.equal(
    indirection,
    undefined,
    'required CI workflow must not use YAML anchors, aliases, or tags',
  )
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


function findStepBlock(workflowLines, stepName, jobIndent) {
  const stepMarker = `- name: ${stepName}`
  for (let index = 0; index < workflowLines.length; index += 1) {
    const line = workflowLines[index]
    if (line.trim() !== stepMarker) continue
    if (line.match(/^\s*/u)[0].length !== jobIndent + 2) continue
    const stepIndent = line.match(/^\s*/u)[0].length
    let end = workflowLines.length
    for (let next = index + 1; next < workflowLines.length; next += 1) {
      const trimmed = workflowLines[next].trim()
      const indent = workflowLines[next].match(/^\s*/u)[0].length
      if (trimmed.startsWith('- ') && indent === stepIndent) {
        end = next
        break
      }
    }
    return { lines: workflowLines, start: index, end, stepIndent }
  }
  return null
}

function assertReleaseCacheContract(workflow) {
  const buildCli = jobBlock(workflow, 'build-cli')
  const buildCliText = buildCli.join('\n')
  assert.ok(
    buildCliText.includes(requiredCacheAction),
    'release build-cli job must restore a Rust cache via Swatinem/rust-cache@v2',
  )
  const workflowLines = workflow.split(/\r?\n/)
  const cacheStep = findStepBlock(workflowLines, 'Cache release build inputs', 4)
  assert.ok(cacheStep, 'release workflow must declare the "Cache release build inputs" step')
  const cacheBlock = cacheStep.lines.slice(cacheStep.start, cacheStep.end)
  const control = cacheBlock
    .slice(1)
    .find(
      (line) =>
        (hasBypassControl(line) || hasShellOverride(line)) &&
        line.match(/^\s*/u)[0].length === cacheStep.stepIndent + 2,
    )
  assert.equal(control, undefined, 'release cache step must not be bypassable')
  const withIndex = cacheBlock.findIndex((line) => line.trim() === 'with:')
  assert.ok(withIndex > 0, 'release cache step must declare a with: mapping')
  const withIndent = cacheBlock[withIndex].match(/^\s*/u)[0].length
  const inputs = cacheBlock.filter(
    (line) => line.match(/^\s*/u)[0].length === withIndent + 2,
  )
  const sharedKeyLine = inputs.find((line) => hasMappingKey(line, 'shared-key'))
  assert.ok(sharedKeyLine, 'release cache step must declare a shared-key input')
  assert.ok(
    sharedKeyLine.includes('${{ matrix.os }}') && sharedKeyLine.includes('${{ matrix.target }}'),
    'release cache shared-key must include ${{ matrix.os }} and ${{ matrix.target }} so incompatible OS/target caches cannot collide',
  )
  assert.equal(
    sharedKeyLine.trim(),
    `shared-key: ${expectedCacheSharedKey}`,
    'release cache shared-key must use the exact documented release cache key',
  )
  const workspacesLine = inputs.find((line) => hasMappingKey(line, 'workspaces'))
  assert.ok(workspacesLine, 'release cache step must declare a workspaces input')
  assert.ok(
    /workspaces:\s*\|/u.test(workspacesLine.trim()),
    'release cache step must use a block-scalar workspaces input that targets the cli workspace only',
  )
  const workspacesBody = cacheBlock
    .slice(cacheBlock.indexOf(workspacesLine) + 1)
    .filter(
      (line) =>
        line.match(/^\s*/u)[0].length > workspacesLine.match(/^\s*/u)[0].length,
    )
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith('#'))
  assert.ok(
    workspacesBody.some((line) => /^cli\s*->\s*cli\/target$/u.test(line)),
    'release cache workspaces must map cli -> cli/target and never cache mutated source or release assets',
  )
  assert.equal(
    workspacesBody.some((line) => /assets|release-assets|src/i.test(line)),
    false,
    'release cache workspaces must never include source files, assets, or release-assets',
  )
}

function assertPublishJobNoRebuild(workflow) {
  const publishJob = jobBlock(workflow, 'github-release')
  const publishText = publishJob.join('\n')
  assert.equal(
    /(^|\s)cargo\s+(build|test|run|check|install)\b/m.test(publishText),
    false,
    'github-release job must not recompile a debug CLI; use the staged release binary for source parity',
  )
  assert.equal(
    publishText.includes('/cli/target/debug/mdp'),
    false,
    'github-release job must not reference the local debug build path',
  )
}

function assertReleaseSmokeContract(workflow) {
  const commands = runBlock(workflow, stepName).filter((line) => !line.startsWith('#'))
  const smokeIndex = commands.indexOf(smokeCommand)
  assert.deepEqual(
    commands.slice(0, requiredPrefix.length),
    requiredPrefix,
    'release smoke step must declare strict mode, capture the resolved version, and chmod the staged Linux binary before the smoke runs',
  )
  assert.notEqual(smokeIndex, -1, 'release smoke step must execute the published smoke')
  // The line must be a real VAR= assignment, not an echo / comment / heredoc.
  // We also reject any binding that is inside an `if`/`else`/`case`/`while`
  // block, which would silently skip the staged source binding.
  const bindingLineRegex = /^\s*MDP_RELEASE_SOURCE_PARITY_BIN=["']?\$?GITHUB_WORKSPACE\/release-assets\/mdp-x86_64-unknown-linux-gnu["']?\s*(?:\\)?$/
  let bindingIndex = -1
  let conditionalDepth = 0
  for (let index = 0; index < commands.length; index += 1) {
    const line = commands[index]
    if (/^\s*if\s.*;\s*then\b/.test(line) || /^\s*(?:elif|else)\b/.test(line) || /^\s*while\b/.test(line) || /^\s*case\b/.test(line) || /^\s*until\b/.test(line)) {
      conditionalDepth += 1
      continue
    }
    if (/^\s*fi\b/.test(line)) {
      conditionalDepth = Math.max(0, conditionalDepth - 1)
      continue
    }
    if (conditionalDepth === 0 && bindingLineRegex.test(line)) {
      bindingIndex = index
      break
    }
  }
  assert.notEqual(
    bindingIndex,
    -1,
    'release smoke step must set MDP_RELEASE_SOURCE_PARITY_BIN to the exact staged Linux release binary at the top level (not inside a conditional)',
  )
  const debugLeak = commands.findIndex(
    (line) =>
      line.includes('MDP_RELEASE_SOURCE_PARITY_BIN=') &&
      (line.includes('cli/target/debug/mdp') || line.includes('cli/target/release/')),
  )
  assert.equal(
    debugLeak,
    -1,
    'release smoke must not bind the route-budget source binary to a freshly compiled debug or local release path',
  )
  const parityIndex = commands.findIndex(
    (line) => line.trim() === stagedParityFlag,
  )
  assert.notEqual(
    parityIndex,
    -1,
    'release smoke step must set MDP_RELEASE_REQUIRE_STAGED_PARITY=1 before the smoke runs',
  )
  assert.ok(
    bindingIndex < smokeIndex && parityIndex < smokeIndex,
    'staged source binding and staged parity flag must be set before the smoke command',
  )
  const codexPackIndex = workflow.indexOf(codexPackCommand)
  const codexVersionIndex = workflow.indexOf(codexVersionCommand)
  const smokeStepIndex = workflow.indexOf(`- name: ${stepName}`)
  assert.ok(codexPackIndex >= 0, 'release workflow must install the pinned native Codex package')
  assert.ok(codexVersionIndex >= 0, 'release workflow must verify the exact native Codex version')
  assert.ok(
    codexPackIndex < codexVersionIndex && codexVersionIndex < smokeStepIndex,
    'pinned native Codex setup must complete before release smoke',
  )
  assert.equal(workflow.includes(neutralFixturePath), false, 'release sources must exclude test fixtures')
  assert.equal(workflow.includes(neutralFixtureId), false, 'release sources must exclude neutral fixture IDs')
  assertReleaseCacheContract(workflow)
  assertPublishJobNoRebuild(workflow)
}

function assertAssetParityCiContract(workflow) {
  assertNoWorkflowShellOverride(workflow)
  assertNoWorkflowYamlIndirection(workflow)
  assertCliJobWiring(workflow)
  assertCliPathFilter(workflow, 'plugin/assets/**')
  assertCliPathFilter(workflow, 'assets/**')
  assertCliPathFilter(workflow, 'scripts/patch-codex-installer.mjs')
  assertCliPathFilter(workflow, 'scripts/test-opencode-wrapper.mjs')
  const cliJob = jobBlock(workflow, 'cli').join('\n')
  assertUnconditionalStep(cliJob, 'Validate authored asset parity')
  const commands = runBlock(cliJob, 'Validate authored asset parity')
    .filter((line) => !line.startsWith('#'))
  assert.deepEqual(
    commands,
    [assetParityCommand],
    'required CI must execute authored asset parity unconditionally',
  )
  assertUnconditionalStep(cliJob, 'Validate cross-profile conformance')
  assert.deepEqual(
    runBlock(cliJob, 'Validate cross-profile conformance').filter((line) => !line.startsWith('#')),
    ['make validate-profile-conformance'],
    'cross-profile conformance must use the named make gate',
  )
}

const workflow = readFileSync(workflowPath, 'utf8')
assertReleaseSmokeContract(workflow)
const ciWorkflow = readFileSync(ciWorkflowPath, 'utf8')
assertAssetParityCiContract(ciWorkflow)

for (const [name, mutation] of [
  ['missing staged source binding', workflow.replace(stagedSourceBinding, 'MDP_RELEASE_SOURCE_PARITY_BIN=cli/target/debug/mdp')],
  ['commented staged source binding', workflow.replace(stagedSourceBinding, `# ${stagedSourceBinding}`)],
  ['echoed staged source binding', workflow.replace(stagedSourceBinding, `echo ${stagedSourceBinding}`)],
  [
    'unreachable staged source binding',
    workflow.replace(
      `            ${stagedSourceBinding} \\`,
      `            if false; then\n              ${stagedSourceBinding} \\\n            fi`,
    ),
  ],
  [
    'late staged source binding',
    workflow
      .replace(`            ${stagedSourceBinding} \\\n`, '')
      .replace(`            ${smokeCommand}`, `            ${smokeCommand}\n            ${stagedSourceBinding} \\`),
  ],
  [
    'debug source binary in publish job',
    workflow.replace(
      `            ${stagedSourceBinding} \\`,
      `            MDP_RELEASE_SOURCE_PARITY_BIN="\\$GITHUB_WORKSPACE/cli/target/debug/mdp" \\`,
    ),
  ],
  [
    'cargo build reintroduced in publish job',
    workflow.replace(
      '      - name: Install and smoke-test the published release\n',
      '      - name: Build source debug CLI\n        run: cargo build --manifest-path cli/Cargo.toml\n      - name: Install and smoke-test the published release\n',
    ),
  ],
  [
    'shared cache key missing matrix target',
    workflow.replace(expectedCacheSharedKey, 'release-${{ matrix.os }}'),
  ],
  [
    'shared cache key missing matrix os',
    workflow.replace(expectedCacheSharedKey, 'release-${{ matrix.target }}'),
  ],
  [
    'release cache step bypassed',
    workflow.replace(
      '      - name: Cache release build inputs\n',
      '      - name: Cache release build inputs\n        if: false\n',
    ),
  ],
  [
    'release cache workspaces caches assets',
    workflow.replace('cli -> cli/target', 'cli -> cli/target\n            assets -> assets'),
  ],
  [
    'release cache workspaces caches source',
    workflow.replace('cli -> cli/target', 'cli -> cli/target\n            cli/src -> cli/src'),
  ],
  [
    'release cache step removed',
    workflow.replace(
      '      - name: Cache release build inputs\n',
      '      - name: Cache release build inputs REMOVED\n',
    ),
  ],
  [
    'release cache step uses unmaintained action',
    workflow.replace('uses: Swatinem/rust-cache@v2', 'uses: actions/cache@v4'),
  ],
  [
    'release cache step uses custom shell',
    workflow.replace(
      '      - name: Cache release build inputs\n',
      '      - name: Cache release build inputs\n        shell: /bin/true {0}\n',
    ),
  ],
  ['missing pinned Codex package', workflow.replace(codexPackCommand, 'npm pack @openai/codex@latest')],
  ['missing Codex version proof', workflow.replace(codexVersionCommand, 'codex --version')],
]) {
  assert.throws(() => assertReleaseSmokeContract(mutation), undefined, name)
}


for (const [name, mutation] of [
  [
    'missing Codex installer patch filter',
    ciWorkflow.replace(
      '              - "scripts/patch-codex-installer.mjs"',
      '              - "scripts/patch-codex-installer.mjs.disabled"',
    ),
  ],
  [
    'missing generated Codex installer proof filter',
    ciWorkflow.replace(
      '              - "scripts/test-opencode-wrapper.mjs"',
      '              - "scripts/test-opencode-wrapper.mjs.disabled"',
    ),
  ],
  ['commented asset parity', ciWorkflow.replace(`          ${assetParityCommand}`, `          # ${assetParityCommand}`)],
  ['echoed asset parity', ciWorkflow.replace(`          ${assetParityCommand}`, `          echo ${assetParityCommand}`)],
  ['unreachable asset parity', ciWorkflow.replace(`          ${assetParityCommand}`, `          if false; then\n            ${assetParityCommand}\n          fi`)],
  [
    'missing cross-profile conformance',
    ciWorkflow.replace('          make validate-profile-conformance', '          make validate-cli'),
  ],
  [
    'dry-run make substitution',
    ciWorkflow.replace(
      `      - name: Validate authored asset parity\n        run: |\n          ${assetParityCommand}`,
      '      - name: Validate authored asset parity\n        env: { MAKEFLAGS: "-n" }\n        run: |\n          make validate-asset-sync',
    ),
  ],
  [
    'relative comparison in alternate working directory',
    ciWorkflow.replace(
      `        run: |\n          ${assetParityCommand}`,
      '        working-directory: ${{ runner.temp }}\n        run: |\n          /usr/bin/env -i /usr/bin/diff -qr plugin/assets assets',
    ),
  ],
  [
    'disabled asset parity step',
    ciWorkflow.replace(
      '      - name: Validate authored asset parity\n',
      '      - name: Validate authored asset parity\n        if: false\n',
    ),
  ],
  [
    'quoted disabled asset parity step',
    ciWorkflow.replace(
      '      - name: Validate authored asset parity\n',
      '      - name: Validate authored asset parity\n        "if": false\n',
    ),
  ],
  [
    'escaped quoted disabled asset parity step',
    ciWorkflow.replace(
      '      - name: Validate authored asset parity\n',
      '      - name: Validate authored asset parity\n        "\\u0069f": false\n',
    ),
  ],
  [
    'yaml hex escaped disabled asset parity step',
    ciWorkflow.replace(
      `          ${assetParityCommand}\n`,
      `          ${assetParityCommand}\n        "\\x69f": false\n`,
    ),
  ],
  [
    'yaml long-unicode escaped disabled asset parity step',
    ciWorkflow.replace(
      `          ${assetParityCommand}\n`,
      `          ${assetParityCommand}\n        "\\U00000069f": false\n`,
    ),
  ],
  [
    'explicit mapping-key disabled asset parity step',
    ciWorkflow.replace(
      `          ${assetParityCommand}\n`,
      `          ${assetParityCommand}\n        ? "if"\n        : false\n`,
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
    'custom parity shell',
    ciWorkflow.replace(
      '      - name: Validate authored asset parity\n',
      '      - name: Validate authored asset parity\n        shell: /bin/true {0}\n',
    ),
  ],
  [
    'explicit mapping-key parity shell',
    ciWorkflow.replace(
      `          ${assetParityCommand}\n`,
      `          ${assetParityCommand}\n        ? "shell"\n        : "/bin/true {0}"\n`,
    ),
  ],
  [
    'commented explicit mapping-key parity shell',
    ciWorkflow.replace(
      `          ${assetParityCommand}\n`,
      `          ${assetParityCommand}\n        ? "shell" # explicit key comment\n        : "/bin/true {0}"\n`,
    ),
  ],
  [
    'colon-commented plain explicit parity shell',
    ciWorkflow.replace(
      `          ${assetParityCommand}\n`,
      `          ${assetParityCommand}\n        ? shell # comment: colon\n        : "/bin/true {0}"\n`,
    ),
  ],
  [
    'block-scalar explicit parity shell',
    ciWorkflow.replace(
      '        run: |\n',
      '        ? >-\n          shell\n        : "/bin/true {0}"\n        run: |\n',
    ),
  ],
  [
    'escaped flow-style workflow shell',
    ciWorkflow.replace(
      'jobs:\n',
      'defaults: { run: { "\\u0073hell": "/bin/true {0}" } }\n\njobs:\n',
    ),
  ],
  [
    'aliased workflow shell key',
    ciWorkflow.replace(
      'jobs:\n',
      'env:\n  SHELL_KEY: &shell_key shell\ndefaults: { run: { *shell_key: "/bin/true {0}" } }\n\njobs:\n',
    ),
  ],
  [
    'tagged disabled parity step',
    ciWorkflow.replace(
      '      - name: Validate authored asset parity\n',
      '      - name: Validate authored asset parity\n        !!str if: false\n',
    ),
  ],
  [
    'bare-tag disabled parity step',
    ciWorkflow.replace(
      '      - name: Validate authored asset parity\n',
      '      - name: Validate authored asset parity\n        ! if: false\n',
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
    'custom cli job shell',
    ciWorkflow.replace(
      '  cli:\n',
      '  cli:\n    defaults:\n      run:\n        shell: /bin/true {0}\n',
    ),
  ],
  [
    'flow-style cli job shell',
    ciWorkflow.replace(
      '  cli:\n',
      '  cli:\n    defaults: { run: { shell: "/bin/true {0}" } }\n',
    ),
  ],
  [
    'custom workflow shell',
    ciWorkflow.replace(
      'jobs:\n',
      'defaults:\n  run:\n    shell: /bin/true {0}\n\njobs:\n',
    ),
  ],
  [
    'flow-style workflow shell',
    ciWorkflow.replace(
      'jobs:\n',
      'defaults: { run: { "shell": "/bin/true {0}" } }\n\njobs:\n',
    ),
  ],
  [
    'post-jobs workflow shell',
    `${ciWorkflow}\ndefaults: { run: { "shell": "/bin/true {0}" } }\n`,
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
    'every-pattern paths filter',
    ciWorkflow.replace(
      '          filters: |\n',
      '          predicate-quantifier: every\n          filters: |\n',
    ),
  ],
  [
    'disabled paths filter step',
    ciWorkflow.replace('      - id: filter\n', '      - id: filter\n        if: false\n'),
  ],
  [
    'quoted disabled paths filter step',
    ciWorkflow.replace('      - id: filter\n', '      - id: filter\n        "if": false\n'),
  ],
  [
    'disabled changes job',
    ciWorkflow.replace('  changes:\n    runs-on:', '  changes:\n    if: false\n    runs-on:'),
  ],
  [
    'quoted disabled changes job',
    ciWorkflow.replace('  changes:\n    runs-on:', '  changes:\n    "if": false\n    runs-on:'),
  ],
  [
    'parity step moved to disabled job',
    ciWorkflow
      .replace(
        `      - name: Validate authored asset parity\n        run: |\n          ${assetParityCommand}\n`,
        '',
      )
      .replace(
        '  mcp-macos:\n',
        `  dead-parity:\n    if: false\n    runs-on: ubuntu-latest\n    steps:\n      - name: Validate authored asset parity\n        run: |\n          ${assetParityCommand}\n\n  mcp-macos:\n`,
      ),
  ],
]) {
  assert.throws(() => assertAssetParityCiContract(mutation), undefined, name)
}

console.log('Release workflow and authored-asset CI contracts passed.')
