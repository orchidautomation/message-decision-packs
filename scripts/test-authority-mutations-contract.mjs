#!/usr/bin/env node

import assert from 'node:assert/strict'
import { readFileSync, existsSync } from 'node:fs'
import { execFileSync, spawnSync } from 'node:child_process'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = dirname(dirname(fileURLToPath(import.meta.url)))
const workflow = readFileSync(join(root, '.github/workflows/authority-mutations.yml'), 'utf8')
const script = readFileSync(join(root, 'scripts/test-authority-mutations.sh'), 'utf8')
const ci = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8')

const complete = ['0/4', '1/4', '2/4', '3/4']
const smoke = ['from_run', 'permits_projection']
const smokeDescriptions = [
  'replace SourceAuthority::from_run -> Self with Default::default\\(\\)',
  'replace match guard decision_blocked with false in SourceAuthority::from_run',
  'replace SourceAuthority::permits_projection -> bool with false',
  'replace > with == in SourceAuthority::permits_projection',
]

function jobBlock(text, name) {
  const lines = text.split(/\r?\n/)
  const start = lines.findIndex((line) => line.trim() === `${name}:` && line.match(/^\s*/u)[0].length === 2)
  assert.notEqual(start, -1, `missing job: ${name}`)
  let end = lines.length
  for (let index = start + 1; index < lines.length; index += 1) {
    const indent = lines[index].match(/^\s*/u)[0].length
    if (lines[index].trim() && indent <= 2) { end = index; break }
  }
  return lines.slice(start, end).join('\n')
}

function classify(event, paths) {
  if (event !== 'pull_request') return 'full'
  return paths.some((path) =>
    path === '.github/workflows/authority-mutations.yml' ||
    path === 'scripts/test-authority-mutations.sh' ||
    path === 'scripts/test-authority-mutations-contract.mjs' ||
    path === 'cli/Cargo.toml' ||
    path === 'cli/Cargo.lock' ||
    path.startsWith('cli/src/authority/') ||
    path.startsWith('plugin/assets/authority-conformance/') ||
    path.startsWith('assets/authority-conformance/'),
  )
    ? 'smoke'
    : 'skip'
}
function aggregate(classification, contract, smokeResult, fullResult, classifier = 'success') {
  if (classifier !== 'success' || contract !== 'success') return false
  return (
    (classification === 'smoke' && smokeResult === 'success' && fullResult === 'skipped') ||
    (classification === 'skip' && smokeResult === 'skipped' && fullResult === 'skipped') ||
    (classification === 'full' && smokeResult === 'skipped' && fullResult === 'success')
  )
}

// Pure policy fixtures exercise the fail-closed routing contract.
assert.equal(classify('push', ['README.md']), 'full')
assert.equal(classify('schedule', []), 'full')
assert.equal(classify('workflow_dispatch', []), 'full')
assert.equal(classify('pull_request', ['cli/src/main.rs']), 'skip')
assert.equal(classify('pull_request', ['docs/getting-started.md']), 'skip')
assert.equal(classify('pull_request', ['cli/src/authority/mod.rs']), 'smoke')
assert.equal(classify('pull_request', ['scripts/test-authority-mutations.sh']), 'smoke')
assert.equal(classify('pull_request', ['cli/Cargo.toml']), 'smoke')
assert.equal(classify('pull_request', ['cli/Cargo.lock']), 'smoke')
assert.equal(classify('pull_request', ['plugin/assets/authority-conformance/corpus.json']), 'smoke')

for (const tuple of [
  ['smoke', 'success', 'success', 'skipped'],
  ['skip', 'success', 'skipped', 'skipped'],
  ['full', 'success', 'skipped', 'success'],
]) assert.equal(aggregate(...tuple), true)
for (const tuple of [
  ['smoke', 'success', 'failure', 'skipped'],
  ['skip', 'success', 'success', 'skipped'],
  ['full', 'success', 'skipped', 'failure'],
  ['smoke', 'failure', 'success', 'skipped'],
  ['skip', 'failure', 'skipped', 'skipped'],
  ['skip', 'success', 'skipped', 'skipped', 'failure'],
  ['unknown', 'success', 'skipped', 'skipped'],
]) assert.equal(aggregate(...tuple), false)

assert.match(workflow, /branches:\s*\[main\]/u)
const pullRequestTrigger = workflow.match(/pull_request:\n([\s\S]*?)(?=\n\s{2}\w|$)/u)?.[1] ?? ''
assert.match(pullRequestTrigger, /branches:\s*\[main\]/u)
assert.doesNotMatch(pullRequestTrigger, /paths:/u, 'pull_request must reach the classifier for every path')
assert.match(workflow, /tags:\s*\["v\*"\]/u)
assert.match(workflow, /schedule:/u)
assert.match(workflow, /workflow_dispatch:/u)
assert.match(workflow, /actions\/github-script@v7/u)
assert.match(workflow, /github\.paginate\(github\.rest\.pulls\.listFiles/u)
assert.match(workflow, /classification.*\? 'smoke' : 'skip'/u)
assert.match(workflow, /core\.setFailed\(`unsupported event/u)
assert.match(workflow, /authority-mutation-classifier:/u)
assert.match(workflow, /authority-mutations-\$\{\{ github\.event\.pull_request\.number/u)
assert.match(workflow, /authority-mutation-smoke:/u)
assert.match(workflow, /bash scripts\/test-authority-mutations\.sh --smoke/u)
assert.match(workflow, /authority-mutation-shard:/u)
assert.deepEqual([...workflow.matchAll(/shard: \["([^"]+)", "([^"]+)", "([^"]+)", "([^"]+)"\]/gu)][0].slice(1), complete)
assert.match(workflow, /authority-mutations:\n    if: always\(\)/u)
assert.match(workflow, /needs: \[authority-mutation-classifier, authority-mutation-contract, authority-mutation-smoke, authority-mutation-shard\]/u)
assert.match(workflow, /smoke:success:skipped\|skip:skipped:skipped\|full:skipped:success/u)
assert.match(workflow, /test "\$CLASSIFIER_RESULT" = success/u)
assert.match(workflow, /test "\$CONTRACT_RESULT" = success/u)
assert.match(workflow, /cargo install cargo-mutants --version 27\.1\.0 --locked/u)
assert.match(workflow, /timeout-minutes: 5/u)
assert.match(workflow, /timeout-minutes: 40/u)
assert.match(workflow, /uses: Swatinem\/rust-cache@v2/u)
assert.match(workflow, /shared-key: authority-mutations-\$\{\{ runner\.os \}\}-\$\{\{ matrix\.shard \}\}/u)
assert.match(workflow, /workspaces:\s*\|\s*\n\s*cli -> cli\/target/u)
assert.doesNotMatch(workflow, /workspaces:[\s\S]{0,200}\b(?:src|source|assets|release-assets)\b/u)
assert.doesNotMatch(workflow, /continue-on-error/u)

assert.match(script, /set -euo pipefail/u)
assert.match(script, /EXPECTED_VERSION="27\.1\.0"/u)
assert.match(script, /MAX_CANDIDATES=24/u)
assert.match(script, /MAX_SMOKE_CANDIDATES=8/u)
assert.match(script, /BUILD_TIMEOUT_SECONDS=120/u)
assert.match(script, /TEST_TIMEOUT_SECONDS=240/u)
assert.match(script, /SELECTOR='\(from_run\|permits_projection\)'/u)
assert.match(script, /MUTATION_FILE='src\/authority\/mod\.rs'/u)
for (const description of smokeDescriptions) assert.ok(script.includes(`'${description}'`), `missing smoke selector: ${description}`)
assert.match(script, /SMOKE_SELECTOR='\(replace SourceAuthority::from_run/u)
assert.match(script, /expected exactly one/u)
assert.match(script, /--smoke/u)
assert.match(script, /does not support sharding/u)
assert.match(script, /awk 'NF \{ seen\[\$0\]\+\+ \}/u)
assert.match(script, /outside the complete candidate set/u)
assert.match(script, /--in-place/u)
const smokeExecution = script.slice(script.lastIndexOf('if [ "$smoke" = "1" ]; then'))
const smokeElse = smokeExecution.indexOf('\nelse\n')
assert.equal((smokeExecution.slice(0, smokeElse).match(/cargo mutants/g) || []).length, 1, 'smoke mode must execute cargo-mutants once for the union selector')
assert.match(script, /0\/4\|1\/4\|2\/4\|3\/4/u)
assert.match(workflow, /bash scripts\/test-authority-mutations\.sh "\$\{\{ matrix\.shard \}\}"/u)
assert.doesNotMatch(script, /0\/2|1\/2/u)
assert.doesNotMatch(script, /^\s*#\s*cargo\s+mutants/mu)
assert.match(ci, /node scripts\/test-authority-mutations-contract\.mjs/u)
assert.match(ci, /scripts\/test-authority-mutations\.sh/u)

if (existsSync(join(root, '.github/workflows/release.yml'))) {
  const release = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8')
  const buildCli = jobBlock(release, 'build-cli')
  assert.match(buildCli, /uses:\s*Swatinem\/rust-cache@v2/u)
  assert.match(buildCli, /shared-key:\s*release-\$\{\{\s*matrix\.os\s*\}\}-\$\{\{\s*matrix\.target\s*\}\}/u)
  assert.match(buildCli, /workspaces:\s*\|\s*\n\s*cli -> cli\/target/u)
  assert.doesNotMatch(buildCli, /workspaces:[\s\S]{0,200}\b(?:src|source|assets|release-assets)\b/u)
}

// When the pinned tool is available, prove that the complete four-shard
// topology is disjoint and exhaustive. The check is optional locally because
// CI installs the pinned binary in the workflow tool job.
const toolProbe = spawnSync('cargo-mutants', ['--help'], { stdio: 'ignore' })
if (toolProbe.error?.code === 'ENOENT') {
  console.log('cargo-mutants not installed locally; skipped list topology execution.')
} else {
  const list = (args) => execFileSync('bash', [join(root, 'scripts/test-authority-mutations.sh'), '--list', ...args], {
    cwd: root, env: { ...process.env, MDP_AUTHORITY_MUTATIONS_LIST_ONLY: '1' }, encoding: 'utf8',
  }).split(/\r?\n/).map((line) => line.trim()).filter(Boolean)
  const completeList = list([])
  const shardLists = complete.map((shard) => list([shard]))
  assert.equal(new Set(completeList).size, completeList.length, 'complete candidate list must not contain duplicates')
  assert.deepEqual(shardLists.flat().sort(), completeList.slice().sort(), 'complete shard union must equal unsharded candidates')
  assert.equal(new Set(shardLists.flat()).size, completeList.length, 'complete shards must be disjoint')
  const smokeList = list(['--smoke'])
  assert.equal(smokeList.length, 4, 'smoke list must contain exactly four candidates')
  const displayDescription = (description) => description.replaceAll('\\(', '(').replaceAll('\\)', ')')
  assert.deepEqual(smokeDescriptions.map((description) => smokeList.filter((candidate) => candidate.includes(displayDescription(description))).length), [1, 1, 1, 1], 'smoke list must contain one candidate for each declared description')
}

console.log('Authority mutation workflow and script contracts passed.')
