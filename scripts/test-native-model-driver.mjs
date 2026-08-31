#!/usr/bin/env node
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  DRIVER_REQUEST_CONTRACT,
  DRIVER_RESULT_CONTRACT,
  PROVIDER_REQUEST_SCHEMA_ID,
  SCHEMA_PROJECTION_CONTRACT,
  buildProviderRequestBody,
  buildModelParametersProjection,
  executeNativeModelRequest,
  modelParametersProjectionSha256,
  projectOutputSchemaForOpenAI,
  sha256CanonicalJson,
  validateNativeModelRequest,
} from './mdp-native-model-openai.mjs'

const sha256 = (value) => createHash('sha256').update(value).digest('hex')
const schema = {
  type: 'object',
  additionalProperties: false,
  required: ['contract', 'prompt_id'],
  properties: {
    contract: { type: 'string', const: 'mdp.prompt-output.v0' },
    prompt_id: { type: 'string', const: 'generate-outbound-copy-v1' },
  },
}
const request = {
  contract: DRIVER_REQUEST_CONTRACT,
  execution_id: 'exec-native-model-001',
  provider: 'openai',
  model: 'gpt-test',
  prompt_id: 'generate-outbound-copy-v1',
  declared_inputs_only: true,
  input: [{ role: 'user', content: '{"task":"synthetic"}' }],
  output_schema: schema,
  output_schema_sha256: sha256CanonicalJson(schema),
  max_output_tokens: 800,
  timeout_ms: 25_000,
}

validateNativeModelRequest(request)
assert.deepEqual(buildProviderRequestBody(request), {
  model: 'gpt-test',
  input: [{ role: 'user', content: '{"task":"synthetic"}' }],
  text: {
    format: {
      type: 'json_schema',
      name: 'mdp_generate_outbound_copy_v1',
      strict: true,
      schema,
    },
  },
  store: false,
  tool_choice: 'none',
  max_output_tokens: 800,
})

const parametersProjection = buildModelParametersProjection(request)
assert.deepEqual(parametersProjection, {
  contract: 'mdp.model-parameters.v1',
  provider: 'openai',
  requested_model: 'gpt-test',
  authorized_endpoint: 'https://api.openai.com/v1/responses',
  declared_timeout_ms: 25_000,
  max_output_tokens: 800,
  structured_output_mode: 'json-schema-strict',
  schema_name: 'mdp_generate_outbound_copy_v1',
  provider_output_schema_sha256: sha256CanonicalJson(schema),
  input_framing: 'one-fresh-user-message:declared-inputs-only',
  visible_input_sha256: sha256CanonicalJson(request.input),
  store: false,
  tool_choice: 'none',
  continuation_policy: 'none',
  tools_policy: 'none',
  reasoning: null,
  metadata: null,
})
assert.match(modelParametersProjectionSha256(request), /^[0-9a-f]{64}$/)

for (const [field, mutate] of [
  ['model', (value) => `${value}-changed`],
  ['timeout_ms', (value) => value + 1],
  ['max_output_tokens', (value) => value + 1],
  ['schema_name', () => 'changed_schema'],
  ['input', () => [{ role: 'user', content: 'synthetic changed input' }]],
  ['reasoning', () => ({ effort: 'low' })],
  ['metadata', () => ({ synthetic: 'changed' })],
]) {
  const changed = { ...request, [field]: mutate(request[field]) }
  assert.notEqual(
    modelParametersProjectionSha256(changed),
    modelParametersProjectionSha256(request),
    `projection must change when ${field} changes`,
  )
}

const secretProjection = JSON.stringify(buildModelParametersProjection({
  ...request,
  reasoning: { effort: 'low' },
  metadata: { sentinel: 'api-key-private-input-sentinel' },
}))
assert(!secretProjection.includes('api-key-private-input-sentinel'))
assert(!secretProjection.includes('OPENAI_API_KEY'))

const conditionalSchema = {
  type: 'object',
  properties: {
    status: { enum: ['ready', 'gap'] },
    message: { type: 'string' },
  },
  required: ['status'],
  additionalProperties: false,
  allOf: [{
    if: { properties: { status: { const: 'ready' } } },
    then: { required: ['message'] },
    else: { not: { required: ['message'] } },
  }],
}
assert.deepEqual(projectOutputSchemaForOpenAI(conditionalSchema), {
  type: 'object',
  properties: {
    status: { type: 'string', enum: ['ready', 'gap'] },
    message: { type: 'string' },
  },
  required: ['message', 'status'],
  additionalProperties: false,
})

const inferredPrimitiveSchema = {
  type: 'object',
  properties: {
    contract: { const: 'mdp.prompt-output.v0' },
    state: { enum: ['ready', 'gap'] },
    count: { enum: [1, 2] },
    ratio: { enum: [0.5, 1.5] },
    enabled: { const: true },
  },
}
assert.deepEqual(projectOutputSchemaForOpenAI(inferredPrimitiveSchema), {
  type: 'object',
  properties: {
    contract: { type: 'string', const: 'mdp.prompt-output.v0' },
    state: { type: 'string', enum: ['ready', 'gap'] },
    count: { type: 'integer', enum: [1, 2] },
    ratio: { type: 'number', enum: [0.5, 1.5] },
    enabled: { type: 'boolean', const: true },
  },
  required: ['contract', 'state', 'count', 'ratio', 'enabled'].sort(),
  additionalProperties: false,
})
assert.throws(
  () => projectOutputSchemaForOpenAI({ type: 'object', properties: { mixed: { enum: ['ready', 1] } } }),
  /enum schema values must share a provider-compatible type/,
)

const mockResponse = {
  id: 'resp_synthetic',
  model: 'gpt-test-2026-01-01',
  status: 'completed',
  output: [{
    type: 'message',
    content: [{
      type: 'output_text',
      text: '{"contract":"mdp.prompt-output.v0","prompt_id":"generate-outbound-copy-v1"}',
    }],
  }],
}
const mockResult = await executeNativeModelRequest(request, {
  mode: 'mock',
  mockResponse,
  environment: {
    OPENAI_API_KEY: 'must-not-be-needed-or-observed',
    OPENAI_BASE_URL: 'https://attacker.invalid/v1',
    HTTPS_PROXY: 'https://attacker.invalid',
  },
})
assert.equal(mockResult.contract, DRIVER_RESULT_CONTRACT)
assert.equal(mockResult.terminal_state, 'success')
assert.equal(mockResult.execution_id, request.execution_id)
assert.equal(mockResult.provider_request_schema_id, PROVIDER_REQUEST_SCHEMA_ID)
assert.equal(mockResult.provider_response_body_sha256, sha256(JSON.stringify(mockResponse)))
assert.equal(mockResult.provider_output_schema_sha256, sha256CanonicalJson(schema))
assert.equal(mockResult.output.media_type, 'application/json')
assert.equal(mockResult.output.encoding, 'utf-8')
assert.deepEqual(JSON.parse(mockResult.output.content), {
  contract: 'mdp.prompt-output.v0',
  prompt_id: 'generate-outbound-copy-v1',
})
assert.equal(mockResult.output.byte_count, Buffer.byteLength(mockResult.output.content))
assert.equal(mockResult.output.sha256, sha256(mockResult.output.content))
assert.deepEqual(mockResult.provider_observation, {
  provider: 'openai',
  response_id: 'resp_synthetic',
  model: 'gpt-test-2026-01-01',
})
assert.equal(mockResult.diagnostic_code, null)
assert.ok(!JSON.stringify(mockResult).includes('OPENAI_API_KEY'))
assert.ok(!JSON.stringify(mockResult).includes('must-not-be-needed-or-observed'))
assert.ok(!JSON.stringify(mockResult).includes('attacker.invalid'))

const exactProviderBytes = ` ${JSON.stringify(mockResponse)}\n`
const realSuccess = await executeNativeModelRequest(request, {
  mode: 'real',
  environment: {
    MDP_ALLOW_NATIVE_MODEL_CALLS: '1',
    OPENAI_API_KEY: 'sk-private-success-canary',
  },
  fetchImpl: async () => new Response(exactProviderBytes, {
    status: 200,
    headers: { 'content-type': 'application/json' },
  }),
})
assert.equal(realSuccess.terminal_state, 'success')
assert.equal(realSuccess.provider_response_body_sha256, sha256(exactProviderBytes))
assert.ok(!JSON.stringify(realSuccess).includes(exactProviderBytes))
assert.ok(!JSON.stringify(realSuccess).includes('sk-private-success-canary'))

for (const [environment, diagnosticCode] of [
  [{ OPENAI_API_KEY: 'sk-secret' }, 'native_model_calls_not_allowed'],
  [{ MDP_ALLOW_NATIVE_MODEL_CALLS: '1' }, 'openai_api_key_missing'],
]) {
  let fetchCalled = false
  const result = await executeNativeModelRequest(request, {
    mode: 'real',
    environment,
    fetchImpl: async () => {
      fetchCalled = true
      throw new Error('must not call')
    },
  })
  assert.equal(result.terminal_state, 'no-draft:policy-blocked')
  assert.equal(result.diagnostic_code, diagnosticCode)
  assert.equal(result.output, null)
  assert.equal(fetchCalled, false)
  assert.ok(!JSON.stringify(result).includes('sk-secret'))
}

const providerFailure = await executeNativeModelRequest(request, {
  mode: 'real',
  environment: {
    MDP_ALLOW_NATIVE_MODEL_CALLS: '1',
    OPENAI_API_KEY: 'sk-private-canary',
    OPENAI_BASE_URL: 'https://attacker.invalid/v1',
    HTTPS_PROXY: 'https://attacker.invalid',
  },
  fetchImpl: async (url, options) => {
    assert.equal(url, 'https://api.openai.com/v1/responses')
    assert.equal(options.redirect, 'error')
    assert.equal(options.headers.authorization, 'Bearer sk-private-canary')
    assert.equal(JSON.parse(options.body).store, false)
    assert.equal(JSON.parse(options.body).tool_choice, 'none')
    return new Response(JSON.stringify({
      error: { message: 'provider-private-canary sk-private-canary attacker.invalid' },
    }), { status: 429, headers: { 'content-type': 'application/json' } })
  },
})
assert.equal(providerFailure.terminal_state, 'no-draft:runner-failed')
assert.equal(providerFailure.diagnostic_code, 'provider_http_error')
assert.equal(providerFailure.output, null)
assert.match(providerFailure.provider_request_body_sha256, /^[0-9a-f]{64}$/)
assert.equal(providerFailure.provider_request_schema_id, PROVIDER_REQUEST_SCHEMA_ID)
assert.ok(!JSON.stringify(providerFailure).includes('provider-private-canary'))
assert.ok(!JSON.stringify(providerFailure).includes('sk-private-canary'))
assert.ok(!JSON.stringify(providerFailure).includes('attacker.invalid'))

for (const mutation of [
  { tools: [] },
  { conversation: 'conv_123' },
  { previous_response_id: 'resp_123' },
  { endpoint: 'https://attacker.invalid/v1/responses' },
  { input: [{ role: 'user', content: 'one' }, { role: 'assistant', content: 'two' }] },
  { output_schema_sha256: '0'.repeat(64) },
  { timeout_ms: 0 },
  { timeout_ms: 60_001 },
  { timeout_ms: '1000' },
]) {
  assert.throws(() => validateNativeModelRequest({ ...request, ...mutation }))
}

const refusal = await executeNativeModelRequest(request, {
  mode: 'mock',
  mockResponse: {
    id: 'resp_refusal',
    status: 'completed',
    output: [{ type: 'message', content: [{ type: 'refusal', refusal: 'private reason' }] }],
  },
})
assert.equal(refusal.terminal_state, 'no-draft:runner-failed')
assert.equal(refusal.diagnostic_code, 'model_refusal')
assert.equal(refusal.output, null)
assert.ok(!JSON.stringify(refusal).includes('private reason'))

const timeoutResult = await executeNativeModelRequest({ ...request, timeout_ms: 1 }, {
  mode: 'real',
  environment: {
    MDP_ALLOW_NATIVE_MODEL_CALLS: '1',
    OPENAI_API_KEY: 'sk-timeout-test',
  },
  fetchImpl: async (_url, options) => new Promise((_resolve, reject) => {
    options.signal.addEventListener('abort', () => {
      const error = new Error('private timeout detail')
      error.name = 'AbortError'
      reject(error)
    }, { once: true })
  }),
})
assert.equal(timeoutResult.terminal_state, 'no-draft:runner-failed')
assert.equal(timeoutResult.diagnostic_code, 'provider_timeout')
assert.equal(timeoutResult.output, null)
assert.ok(!JSON.stringify(timeoutResult).includes('private timeout detail'))
assert.ok(!JSON.stringify(timeoutResult).includes('sk-timeout-test'))

const driverPath = fileURLToPath(new URL('./mdp-native-model-openai.mjs', import.meta.url))
const subprocess = spawnSync(process.execPath, [driverPath, '--dry-run'], {
  input: JSON.stringify(request),
  encoding: 'utf8',
  env: {
    PATH: process.env.PATH,
    OPENAI_API_KEY: 'must-not-be-used',
    OPENAI_BASE_URL: 'https://attacker.invalid/v1',
  },
})
assert.equal(subprocess.status, 0)
assert.equal(subprocess.stderr, '')
const subprocessResult = JSON.parse(subprocess.stdout)
assert.equal(subprocessResult.contract, DRIVER_RESULT_CONTRACT)
assert.equal(subprocessResult.execution_id, request.execution_id)
assert.equal(subprocessResult.terminal_state, 'no-draft:policy-blocked')
assert.equal(subprocessResult.diagnostic_code, 'dry_run_complete')
assert.equal(subprocessResult.provider_output_schema_sha256, sha256CanonicalJson(schema))
assert.ok(!subprocess.stdout.includes('must-not-be-used'))
assert.ok(!subprocess.stdout.includes('attacker.invalid'))

const projectionFailureRequest = {
  ...request,
  output_schema: { if: { type: 'string' }, then: { const: 'x' } },
  output_schema_sha256: sha256CanonicalJson({ if: { type: 'string' }, then: { const: 'x' } }),
}
const projectionFailure = await executeNativeModelRequest(projectionFailureRequest, { mode: 'mock', mockResponse })
assert.equal(projectionFailure.terminal_state, 'no-draft:preflight-refused')
assert.equal(projectionFailure.diagnostic_code, 'output_schema_projection_unsupported')
assert.equal(projectionFailure.output, null)
assert.equal(projectionFailure.provider_request_body_sha256, null)
assert.equal(projectionFailure.provider_request_schema_id, null)
assert.equal(projectionFailure.provider_output_schema_sha256, null)

const invalidRequestResult = await executeNativeModelRequest({ ...request, max_output_tokens: 0 }, {
  mode: 'mock',
  mockResponse,
})
assert.equal(invalidRequestResult.diagnostic_code, 'request_invalid')
assert.equal(invalidRequestResult.provider_request_body_sha256, null)
assert.equal(invalidRequestResult.provider_request_schema_id, null)

const conditionalRequest = {
  ...request,
  output_schema: conditionalSchema,
  output_schema_sha256: sha256CanonicalJson(conditionalSchema),
}
const conditionalResult = await executeNativeModelRequest(conditionalRequest, { mode: 'mock', mockResponse })
assert.equal(
  conditionalResult.provider_output_schema_sha256,
  sha256CanonicalJson(projectOutputSchemaForOpenAI(conditionalSchema)),
)
assert.notEqual(conditionalResult.provider_output_schema_sha256, conditionalRequest.output_schema_sha256)

const projectionDir = mkdtempSync(join(tmpdir(), 'mdp-schema-projection-'))
try {
  const schemaPath = join(projectionDir, 'canonical-schema.json')
  writeFileSync(schemaPath, JSON.stringify(conditionalSchema))
  const projectionProcess = spawnSync(process.execPath, [driverPath, '--project-schema', schemaPath], {
    encoding: 'utf8',
    env: { PATH: process.env.PATH },
  })
  assert.equal(projectionProcess.status, 0)
  assert.equal(projectionProcess.stderr, '')
  const projectionEnvelope = JSON.parse(projectionProcess.stdout)
  assert.equal(projectionEnvelope.contract, SCHEMA_PROJECTION_CONTRACT)
  assert.equal(projectionEnvelope.canonical_output_schema_sha256, sha256CanonicalJson(conditionalSchema))
  assert.deepEqual(projectionEnvelope.provider_output_schema, projectOutputSchemaForOpenAI(conditionalSchema))
  assert.equal(
    projectionEnvelope.provider_output_schema_sha256,
    sha256CanonicalJson(projectOutputSchemaForOpenAI(conditionalSchema)),
  )
} finally {
  rmSync(projectionDir, { recursive: true, force: true })
}

const legacyDriverPath = fileURLToPath(new URL('./mdp-native-normalize-openai.mjs', import.meta.url))
const legacyDir = mkdtempSync(join(tmpdir(), 'mdp-legacy-native-model-'))
try {
  for (const invalidOptional of [
    { max_output_tokens: 0 },
    { reasoning: null },
  ]) {
    const legacyRequestPath = join(legacyDir, `request-${Object.keys(invalidOptional)[0]}.json`)
    writeFileSync(legacyRequestPath, JSON.stringify({
      contract: 'mdp.native-normalize-request.v0',
      provider: 'openai',
      model: request.model,
      prompt_id: request.prompt_id,
      declared_inputs_only: true,
      input: request.input,
      prompt_output_schema: request.output_schema,
      ...invalidOptional,
    }))
    const legacyProcess = spawnSync(process.execPath, [legacyDriverPath, '--request', legacyRequestPath, '--dry-run'], {
      encoding: 'utf8',
      env: { PATH: process.env.PATH },
    })
    assert.notEqual(legacyProcess.status, 0)
    assert.match(legacyProcess.stderr, /Native model runner failed safely: internal_error/)
  }
} finally {
  rmSync(legacyDir, { recursive: true, force: true })
}

console.log(JSON.stringify({ ok: true, contract: 'mdp.native-model-driver-test.v1' }))
