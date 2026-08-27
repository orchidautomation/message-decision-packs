#!/usr/bin/env node
import assert from 'node:assert/strict'
import { chmodSync, existsSync, linkSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import {
  DEFAULT_PROMPT_ID,
  PROMPT_OUTPUT_CONTRACT,
  RUNNER_CONTRACT,
  TOOLS_CONTRACT,
  promptOutputSchema,
  toolEnvelope,
} from './lib/proposal-runner-contracts.mjs'
import {
  RunnerError,
  nonProviderEnvironment,
  readJson,
  runProcess,
  sha256File,
  writeJsonAtomic,
} from './lib/proposal-runner-runtime.mjs'
import { cleanupMdpRecoveryClaim } from './lib/process-supervisor.mjs'

test('contract module exposes stable runner and prompt-output fixtures', () => {
  const tools = toolEnvelope()
  assert.equal(tools.contract, TOOLS_CONTRACT)
  assert.equal(tools.runner_contract, RUNNER_CONTRACT)
  assert.deepEqual(
    tools.tools.map((entry) => entry.name),
    [
      'mdp_intake_sources',
      'mdp_normalize_opportunity',
      'mdp_validate_normalization',
      'mdp_run_receipt',
      'mdp_clean_run_v1',
      'mdp_review_proposal',
    ],
  )

  const schema = promptOutputSchema()
  assert.deepEqual(schema.properties.contract.enum, [PROMPT_OUTPUT_CONTRACT])
  assert.deepEqual(schema.properties.prompt_id.enum, [DEFAULT_PROMPT_ID])
  assert.equal(schema.additionalProperties, false)
})

test('runtime module atomically writes readable JSON and stable hashes', () => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-runner-module-'))
  const path = join(root, 'artifact.json')
  writeJsonAtomic(path, { contract: 'fixture.v0', value: 1 })
  assert.deepEqual(readJson(path), { contract: 'fixture.v0', value: 1 })
  const firstHash = sha256File(path)
  writeFileSync(path, `${readFileSync(path, 'utf8').replace('1', '2')}`)
  assert.notEqual(sha256File(path), firstHash)
})

test('runtime environment strips likely provider secrets', () => {
  const environment = nonProviderEnvironment({
    PATH: '/usr/bin',
    OPENAI_API_KEY: 'not-a-real-secret',
    SESSION_TOKEN: 'not-a-real-token',
    SAFE_VALUE: 'visible',
  })
  assert.equal(environment.PATH, '/usr/bin')
  assert.equal(environment.SAFE_VALUE, 'visible')
  assert.equal(environment.OPENAI_API_KEY, undefined)
  assert.equal(environment.SESSION_TOKEN, undefined)
})

test('runtime errors preserve explicit exit codes', () => {
  const error = new RunnerError('fixture', 7)
  assert.equal(error.message, 'fixture')
  assert.equal(error.exitCode, 7)
})

test('runtime terminates a subprocess at its explicit deadline', async () => {
  await assert.rejects(
    runProcess({
        command: [process.execPath],
        args: ['-e', 'setInterval(() => {}, 1000)'],
        timeoutMs: 50,
      }),
    (error) => error instanceof RunnerError && /timed out after 50ms/.test(error.message),
  )
})

test('proposal runtime timeout closes descendants before returning', async (t) => {
  if (process.platform === 'win32') return t.skip('Unix process-group behavior')
  const root = mkdtempSync(join(tmpdir(), 'mdp-proposal-process-group-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const marker = join(root, 'descendant-survived')
  const childCode = [
    "const {spawn}=require('node:child_process')",
    `const code=${JSON.stringify(`process.on('SIGTERM',()=>{});setTimeout(()=>require('node:fs').writeFileSync(${JSON.stringify(marker)},'survived'),700);setInterval(()=>{},1000)`)}`,
    "spawn(process.execPath,['-e',code],{stdio:'ignore'})",
    "process.on('SIGTERM',()=>process.exit(0))",
    "setInterval(()=>{},1000)",
  ].join(';')
  await assert.rejects(
    runProcess({ command: [process.execPath], args: ['-e', childCode], timeoutMs: 100 }),
    (error) => error instanceof RunnerError && /timed out/.test(error.message),
  )
  await new Promise((resolveWait) => setTimeout(resolveWait, 800))
  assert.equal(existsSync(marker), false)
})

test('recovery removes only the exact transaction named by a bounded owned claim', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-recovery-module-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const output = join(root, 'clean-run')
  const transactionLeaf = '.clean-run.tmp-0123456789abcdef0123456789abcdef'
  const transaction = join(root, transactionLeaf)
  const claim = join(root, '.clean-run.mdp-run.claim')
  const unrelated = join(root, '.clean-run.tmp-ffffffffffffffffffffffffffffffff')
  mkdirSync(transaction)
  mkdirSync(unrelated)
  writeFileSync(join(transaction, 'private-source'), 'private bytes')
  writeFileSync(claim, `${JSON.stringify({
    contract: 'mdp.run-recovery-claim.v1',
    execution_id: 'run-1',
    transaction_leaf: transactionLeaf,
  })}\n`)
  assert.equal(cleanupMdpRecoveryClaim({ outputDir: output, executionId: 'run-1' }), true)
  assert.equal(existsSync(transaction), false)
  assert.equal(existsSync(claim), false)
  assert.equal(existsSync(unrelated), true)
})

test('supervisor recovery accepts the exact v2 claim emitted by the native CLI', (t) => {
  if (process.platform === 'win32' || typeof process.getuid !== 'function') return
  const root = mkdtempSync(join(tmpdir(), 'mdp-recovery-v2-module-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const output = join(root, 'clean-run')
  const transactionLeaf = '.clean-run.tmp-0123456789abcdef0123456789abcdef'
  const transaction = join(root, transactionLeaf)
  const claim = join(root, '.clean-run.mdp-run.claim')
  mkdirSync(transaction, { mode: 0o700 })
  chmodSync(transaction, 0o700)
  const transactionStats = statSync(transaction)
  writeFileSync(claim, `${JSON.stringify({
    contract: 'mdp.run-recovery-claim.v2',
    execution_id: 'run-v2',
    transaction_leaf: transactionLeaf,
    created_unix_seconds: Math.floor(Date.now() / 1000),
    owner_uid: process.getuid(),
    process_id: 4242,
    transaction_dev: transactionStats.dev,
    transaction_ino: transactionStats.ino,
  })}\n`, { mode: 0o600 })
  chmodSync(claim, 0o600)
  assert.equal(cleanupMdpRecoveryClaim({
    outputDir: output,
    executionId: 'run-v2',
    expectedProcessId: 4242,
  }), true)
  assert.equal(existsSync(transaction), false)
  assert.equal(existsSync(claim), false)
})

test('recovery refuses hard-linked or mismatched claims without deleting a transaction', (t) => {
  if (process.platform === 'win32') return
  const root = mkdtempSync(join(tmpdir(), 'mdp-recovery-module-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const output = join(root, 'clean-run')
  const transactionLeaf = '.clean-run.tmp-0123456789abcdef0123456789abcdef'
  const transaction = join(root, transactionLeaf)
  const claim = join(root, '.clean-run.mdp-run.claim')
  mkdirSync(transaction)
  writeFileSync(claim, `${JSON.stringify({
    contract: 'mdp.run-recovery-claim.v1',
    execution_id: 'different-run',
    transaction_leaf: transactionLeaf,
  })}\n`)
  linkSync(claim, `${claim}.hardlink`)
  assert.equal(cleanupMdpRecoveryClaim({ outputDir: output, executionId: 'run-1' }), false)
  assert.equal(existsSync(transaction), true)
  assert.equal(existsSync(claim), true)
})
