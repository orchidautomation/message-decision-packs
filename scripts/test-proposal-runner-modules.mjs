#!/usr/bin/env node
import assert from 'node:assert/strict'
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
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
  sha256File,
  writeJsonAtomic,
} from './lib/proposal-runner-runtime.mjs'

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
