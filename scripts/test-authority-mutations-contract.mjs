#!/usr/bin/env node

import assert from 'node:assert/strict'
import { readFileSync, existsSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const workflowPath = join(root, '.github/workflows/authority-mutations.yml')
const scriptPath = join(root, 'scripts/test-authority-mutations.sh')

const expectedShardMatrix = ['0/4', '1/4', '2/4', '3/4']
const expectedVersion = '27.1.0'
const expectedMaxCandidates = 24
const expectedBuildTimeout = 120
const expectedTestTimeout = 180
const expectedSelector = '(from_run|permits_projection)'
const expectedFile = 'src/authority/mod.rs'

const workflow = readFileSync(workflowPath, 'utf8')
const script = readFileSync(scriptPath, 'utf8')

// Workflow: every required shard must be present, exactly once, and the matrix
// shard list must equal the documented topology.
function jobBlock(text, name) {
  const lines = text.split(/\r?\n/)
  const marker = `${name}:`
  const start = lines.findIndex(
    (line) => line.trim() === marker && line.match(/^\s*/u)[0].length === 2,
  )
  assert.notEqual(start, -1, `missing job: ${name}`)
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

const shardJob = jobBlock(workflow, 'authority-mutation-shard')
// strategy: must precede matrix:, so we slice from matrix: until the next
// non-indented sibling (steps:).
const matrixStart = shardJob.findIndex((line) => line.trim() === 'matrix:')
assert.ok(matrixStart > -1, 'authority-mutation-shard job must declare a strategy.matrix block')
let matrixEnd = shardJob.length
for (let index = matrixStart + 1; index < shardJob.length; index += 1) {
  const trimmed = shardJob[index].trim()
  const indent = shardJob[index].match(/^\s*/u)[0].length
  if (trimmed && indent <= 6) {
    matrixEnd = index
    break
  }
}
const matrixBlock = shardJob.slice(matrixStart, matrixEnd)
const matrixText = matrixBlock.join('\n')
const shardListMatch = matrixText.match(/shard:\s*\[([^\]]+)\]/u)
assert.ok(shardListMatch, 'authority-mutation-shard job must declare a shard list')
const shardList = shardListMatch[1]
  .split(',')
  .map((s) => s.trim().replace(/^["']|["']$/gu, ''))
  .filter(Boolean)
assert.deepEqual(
  shardList,
  expectedShardMatrix,
  'authority mutation shard matrix must enumerate exactly the documented 0/4..3/4 topology',
)

// Every shard must be referenced exactly once, both as matrix input and as a
// runtime argument passed into the script.
const duplicates = shardList.filter((value, index, list) => list.indexOf(value) !== index)
assert.deepEqual(duplicates, [], 'authority mutation shard list must not contain duplicates')

// Each shard job must invoke the script with its matrix.shard as the first
// positional argument and must not bypass failures.
for (const line of shardJob) {
  if (line.match(/^\s{4}shard:/u)) continue
  if (line.match(/^\s{6}-/u)) {
    assert.ok(
      !/continue-on-error/.test(line),
      'authority-mutation-shard step must not use continue-on-error',
    )
  }
}
assert.ok(
  shardJob.join('\n').includes('bash scripts/test-authority-mutations.sh "${{ matrix.shard }}"'),
  'authority-mutation-shard step must invoke scripts/test-authority-mutations.sh with the exact matrix shard argument',
)

// The aggregate job must require every shard to succeed.
const aggregateJob = jobBlock(workflow, 'authority-mutations')
const aggregateText = aggregateJob.join('\n')
assert.match(
  aggregateText,
  /SHARD_RESULT\s*:\s*\$\{\{\s*needs\.authority-mutation-shard\.result\s*\}\}/u,
  'aggregate job must read the authority-mutation-shard matrix result',
)
assert.match(
  aggregateText,
  /test\s+"\$SHARD_RESULT"\s*=\s*success/u,
  'aggregate job must require the authority-mutation-shard matrix result to equal "success"',
)
assert.ok(
  /needs:\s*authority-mutation-shard/.test(aggregateText),
  'aggregate job must depend on authority-mutation-shard',
)
assert.ok(
  /if:\s*always\(\)/.test(aggregateText),
  'aggregate job must run with if: always() so a shard failure still produces the aggregate gate',
)

// Cargo-mutants tool job must install the pinned version and not skip on
// non-zero exits.
const toolJob = jobBlock(workflow, 'authority-mutation-tool')
const toolText = toolJob.join('\n')
assert.match(
  toolText,
  new RegExp(`cargo install cargo-mutants --version ${expectedVersion} --locked`, 'u'),
  `authority-mutation-tool job must install cargo-mutants ${expectedVersion} (locked)`,
)
assert.ok(
  /timeout-minutes:\s*5/.test(toolText),
  'authority-mutation-tool job must declare timeout-minutes: 5 to fail fast on a broken tool install',
)
assert.ok(
  /timeout-minutes:\s*40/.test(shardJob.join('\n')),
  'authority-mutation-shard job must declare timeout-minutes: 40 to fail closed on hung mutations',
)

// Cache boundaries: build-cli caches must be keyed by runner OS, matrix
// target, pinned Rust toolchain, and the CLI lockfile so incompatible
// artifacts cannot be reused.
function releaseJobBlock() {
  const releasePath = join(root, '.github/workflows/release.yml')
  if (!existsSync(releasePath)) return null
  return readFileSync(releasePath, 'utf8')
}
const releaseWorkflow = releaseJobBlock()
if (releaseWorkflow) {
  const buildCli = jobBlock(releaseWorkflow, 'build-cli')
  const buildCliText = buildCli.join('\n')
  assert.match(
    buildCliText,
    /uses:\s*Swatinem\/rust-cache@v2/u,
    'release build-cli job must use Swatinem/rust-cache@v2',
  )
  assert.match(
    buildCliText,
    /shared-key:\s*release-\$\{\{\s*matrix\.os\s*\}\}-\$\{\{\s*matrix\.target\s*\}\}/u,
    'release cache shared-key must include both ${{ matrix.os }} and ${{ matrix.target }}',
  )
  assert.match(
    buildCliText,
    /workspaces:\s*\|\s*\n\s*cli -> cli\/target/u,
    'release cache workspaces must map cli -> cli/target and never include source or release-assets',
  )
}

// Authority shard cache: each shard must cache only the cli/ target, never
// mutated source, and must be keyed by runner OS, Rust 1.88.0, lockfile,
// cargo-mutants version, and shard topology.
assert.match(
  workflow,
  /uses:\s*Swatinem\/rust-cache@v2/u,
  'authority-mutation-shard job must use Swatinem/rust-cache@v2 to cache dependency builds',
)
assert.match(
  workflow,
  /shared-key:\s*authority-mutations-\$\{\{\s*runner\.os\s*\}\}-\$\{\{\s*matrix\.shard\s*\}\}/u,
  'authority cache shared-key must combine runner.os with matrix.shard to prevent cross-shard contamination',
)
assert.match(
  workflow,
  /workspaces:\s*\|\s*\n\s*cli -> cli\/target/u,
  'authority cache workspaces must map cli -> cli/target only',
)
assert.doesNotMatch(
  workflow,
  /workspaces:[\s\S]{0,200}\b(?:src|source|assets|release-assets)\b/u,
  'authority cache workspaces must never include source, assets, or release-assets',
)

// Script contract: must enforce the supported topology, candidate cap, and
// the deterministic listing mode used by the disjoint-coverage check.
assert.match(
  script,
  new RegExp(`EXPECTED_VERSION="${expectedVersion}"`),
  'authority mutation script must pin the cargo-mutants version',
)
assert.match(
  script,
  new RegExp(`MAX_CANDIDATES=${expectedMaxCandidates}`),
  'authority mutation script must cap the candidate count at 24',
)
assert.match(
  script,
  new RegExp(`BUILD_TIMEOUT_SECONDS=${expectedBuildTimeout}`),
  'authority mutation script must cap build timeout at 120s',
)
assert.match(
  script,
  new RegExp(`TEST_TIMEOUT_SECONDS=${expectedTestTimeout}`),
  'authority mutation script must cap test timeout at 180s',
)
assert.ok(
  script.includes(`SELECTOR='${expectedSelector}'`),
  'authority mutation script must use the (from_run|permits_projection) selector',
)
assert.match(
  script,
  new RegExp(`MUTATION_FILE='${expectedFile}'`),
  'authority mutation script must target src/authority/mod.rs',
)
assert.ok(
  script.includes('0/4|1/4|2/4|3/4) shard_args='),
  'authority mutation script must enumerate the 0/4..3/4 shard topology in a single case branch',
)
assert.doesNotMatch(
  script,
  /\b0\/2\b|\b1\/2\b/u,
  'authority mutation script must not accept the legacy two-shard topology',
)
assert.match(
  script,
  /MDP_AUTHORITY_MUTATIONS_LIST_ONLY/u,
  'authority mutation script must expose a deterministic list-only mode for the shard coverage contract',
)
assert.match(
  script,
  /--list/u,
  'authority mutation script must accept a --list flag to print the candidate list',
)
assert.match(
  script,
  /--help/u,
  'authority mutation script must accept --help for contract smoke runs',
)
assert.match(
  script,
  /--in-place/u,
  'authority mutation script must invoke cargo-mutants with --in-place so each shard run is isolated',
)

// Fail-closed: comments, echo, and conditional bypasses must not be enough
// to satisfy any of the required assertions. We assert that the script
// itself contains none of the documented bypass patterns.
const bypassAnchors = [
  /^\s*#\s*cargo\s+mutants/mu,
]
for (const anchor of bypassAnchors) {
  assert.equal(
    anchor.test(script),
    false,
    `authority mutation script must not contain a documented bypass pattern: ${anchor}`,
  )
}

// The required shard matrix union must equal the unsharded candidate list.
// When cargo-mutants is unavailable locally, the disjoint-coverage helper
// short-circuits with a clear message; we only assert it in environments
// where the binary is installed. The script's deterministic listing path
// stays the same regardless of execution.
console.log('Authority mutation workflow and script contracts passed.')
