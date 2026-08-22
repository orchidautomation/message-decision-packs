#!/usr/bin/env node
import { createHash } from 'node:crypto'
import { existsSync, readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

export const DRIVER_REQUEST_CONTRACT = 'mdp.native-model-subprocess-request.v1'
export const DRIVER_RESULT_CONTRACT = 'mdp.native-model-subprocess-result.v1'
export const SCHEMA_PROJECTION_CONTRACT = 'mdp.native-model-schema-projection.v1'
export const PROVIDER_REQUEST_SCHEMA_ID = 'openai.responses.json-schema-request.v1'
export const MODEL_PARAMETERS_PROJECTION_CONTRACT = 'mdp.model-parameters.v1'

const OFFICIAL_RESPONSES_ENDPOINT = 'https://api.openai.com/v1/responses'
const MAX_REQUEST_BYTES = 2 * 1024 * 1024
const MAX_PROVIDER_RESPONSE_BYTES = 4 * 1024 * 1024
const MAX_OUTPUT_BYTES = 1024 * 1024
const DEFAULT_REQUEST_TIMEOUT_MS = 60_000
const MAX_REQUEST_TIMEOUT_MS = 60_000
const SAFE_ID = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/
const SHA256 = /^[0-9a-f]{64}$/

class DriverFault extends Error {
  constructor(code, terminalState = 'no-draft:runner-failed') {
    super(code)
    this.code = code
    this.terminalState = terminalState
  }
}

const fault = (code, terminalState) => {
  throw new DriverFault(code, terminalState)
}

const requireObject = (value, label) => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new TypeError(`${label} must be an object`)
}

const requireSafeId = (value, label) => {
  if (typeof value !== 'string' || !SAFE_ID.test(value)) throw new TypeError(`${label} must be a safe identifier`)
}

const rejectUnknown = (value, allowed, label) => {
  const unknown = Object.keys(value).filter((key) => !allowed.has(key)).sort()
  if (unknown.length > 0) throw new TypeError(`${label} contains unsupported fields: ${unknown.join(', ')}`)
}

const stableJsonValue = (value) => {
  if (Array.isArray(value)) return value.map(stableJsonValue)
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, stableJsonValue(value[key])]))
  }
  return value
}

export const canonicalJsonBytes = (value) => JSON.stringify(stableJsonValue(value))
export const sha256Bytes = (value) => createHash('sha256').update(value).digest('hex')
export const sha256CanonicalJson = (value) => sha256Bytes(canonicalJsonBytes(value))
export const sha256CanonicalJsonForDomain = (domain, value) => sha256Bytes(`${domain}\u0000${canonicalJsonBytes(value)}`)

const OPENAI_SCHEMA_KEYS = new Set([
  '$defs', '$ref', 'type', 'properties', 'required', 'additionalProperties', 'items',
  'enum', 'const', 'description', 'title', 'anyOf', 'minimum', 'maximum',
  'exclusiveMinimum', 'exclusiveMaximum', 'multipleOf', 'minLength', 'maxLength',
  'pattern', 'minItems', 'maxItems',
])

const mergeProjectedSchemas = (left, right) => {
  if (Object.keys(left).length === 0) return right
  if (Object.keys(right).length === 0) return left
  if (left.type === 'object' && right.type === 'object') {
    const properties = { ...(left.properties || {}), ...(right.properties || {}) }
    return {
      ...left,
      ...right,
      type: 'object',
      properties,
      required: [...new Set([...Object.keys(properties), ...(left.required || []), ...(right.required || [])])].sort(),
      additionalProperties: false,
    }
  }
  return { anyOf: [left, right] }
}

const projectSchemaNode = (schema) => {
  if (typeof schema === 'boolean') {
    if (schema) return {}
    throw new TypeError('false JSON Schema cannot be projected for the provider')
  }
  requireObject(schema, 'schema node')
  let projected = {}
  for (const [key, value] of Object.entries(schema)) {
    if (!OPENAI_SCHEMA_KEYS.has(key)) continue
    if (key === 'properties' || key === '$defs') {
      requireObject(value, `schema.${key}`)
      projected[key] = Object.fromEntries(
        Object.entries(value).map(([name, child]) => [name, projectSchemaNode(child)]),
      )
    } else if (key === 'items') {
      projected.items = projectSchemaNode(value)
    } else if (key === 'anyOf') {
      if (!Array.isArray(value) || value.length === 0) throw new TypeError('schema.anyOf must be non-empty')
      projected.anyOf = value.map(projectSchemaNode)
    } else if (key !== 'required' && key !== 'additionalProperties') {
      projected[key] = value
    }
  }
  if (Array.isArray(schema.allOf)) {
    for (const branch of schema.allOf) projected = mergeProjectedSchemas(projected, projectSchemaNode(branch))
  }
  if (projected.type === 'object' || projected.properties) {
    const properties = projected.properties || {}
    projected.type = 'object'
    projected.properties = properties
    projected.required = Object.keys(properties).sort()
    projected.additionalProperties = false
  }
  return projected
}

export const projectOutputSchemaForOpenAI = (schema) => {
  const projected = projectSchemaNode(schema)
  if (Object.keys(projected).length === 0) {
    throw new TypeError('schema has no provider-compatible structural projection')
  }
  return projected
}

const validateInput = (input) => {
  if (typeof input === 'string') {
    if (input.trim() === '' || Buffer.byteLength(input) > MAX_REQUEST_BYTES) throw new TypeError('input must be a bounded non-empty string')
    return
  }
  if (!Array.isArray(input) || input.length !== 1) {
    throw new TypeError('input must contain exactly one fresh user message')
  }
  const message = input[0]
  requireObject(message, 'input[0]')
  rejectUnknown(message, new Set(['role', 'content']), 'input[0]')
  if (message.role !== 'user' || typeof message.content !== 'string' || message.content.trim() === '') {
    throw new TypeError('input[0] must be one non-empty user message')
  }
}

const validateMetadata = (metadata) => {
  requireObject(metadata, 'metadata')
  const entries = Object.entries(metadata)
  if (entries.length > 16) throw new TypeError('metadata has too many entries')
  for (const [key, value] of entries) {
    if (!SAFE_ID.test(key) || typeof value !== 'string' || value.length > 512) {
      throw new TypeError('metadata must contain bounded string entries with safe keys')
    }
  }
}

const validateReasoning = (reasoning) => {
  requireObject(reasoning, 'reasoning')
  rejectUnknown(reasoning, new Set(['effort', 'summary']), 'reasoning')
  if ('effort' in reasoning && !['none', 'minimal', 'low', 'medium', 'high', 'xhigh'].includes(reasoning.effort)) {
    throw new TypeError('reasoning.effort is unsupported')
  }
  if ('summary' in reasoning && !['auto', 'concise', 'detailed'].includes(reasoning.summary)) {
    throw new TypeError('reasoning.summary is unsupported')
  }
}

const validateJsonComplexity = (value, failureCode) => {
  const stack = [{ value, depth: 0 }]
  let nodes = 0
  while (stack.length > 0) {
    const current = stack.pop()
    nodes += 1
    if (nodes > 100_000 || current.depth > 64) fault(failureCode)
    if (Array.isArray(current.value)) {
      for (const child of current.value) stack.push({ value: child, depth: current.depth + 1 })
    } else if (current.value && typeof current.value === 'object') {
      for (const child of Object.values(current.value)) stack.push({ value: child, depth: current.depth + 1 })
    }
  }
}

const safeProviderIdentifier = (value) =>
  typeof value === 'string' && SAFE_ID.test(value) ? value : null

export const validateNativeModelRequest = (request) => {
  requireObject(request, 'request')
  rejectUnknown(request, new Set([
    'contract',
    'execution_id',
    'provider',
    'model',
    'prompt_id',
    'declared_inputs_only',
    'input',
    'output_schema',
    'output_schema_sha256',
    'schema_name',
    'max_output_tokens',
    'timeout_ms',
    'reasoning',
    'metadata',
  ]), 'request')
  if (request.contract !== DRIVER_REQUEST_CONTRACT) throw new TypeError(`request.contract must be ${DRIVER_REQUEST_CONTRACT}`)
  requireSafeId(request.execution_id, 'execution_id')
  if (request.provider !== 'openai') throw new TypeError('provider must be openai')
  requireSafeId(request.model, 'model')
  requireSafeId(request.prompt_id, 'prompt_id')
  if (request.declared_inputs_only !== true) throw new TypeError('declared_inputs_only must be true')
  validateInput(request.input)
  requireObject(request.output_schema, 'output_schema')
  if (!SHA256.test(request.output_schema_sha256 || '')) throw new TypeError('output_schema_sha256 must be sha256')
  if (sha256CanonicalJson(request.output_schema) !== request.output_schema_sha256) {
    throw new TypeError('output_schema_sha256 does not match output_schema')
  }
  if ('schema_name' in request) requireSafeId(request.schema_name, 'schema_name')
  if ('max_output_tokens' in request && (
    !Number.isSafeInteger(request.max_output_tokens) ||
    request.max_output_tokens < 1 ||
    request.max_output_tokens > 100_000
  )) throw new TypeError('max_output_tokens must be between 1 and 100000')
  if ('timeout_ms' in request && (
    !Number.isSafeInteger(request.timeout_ms) ||
    request.timeout_ms < 1 ||
    request.timeout_ms > MAX_REQUEST_TIMEOUT_MS
  )) throw new TypeError(`timeout_ms must be between 1 and ${MAX_REQUEST_TIMEOUT_MS}`)
  if ('reasoning' in request) validateReasoning(request.reasoning)
  if ('metadata' in request) validateMetadata(request.metadata)
  if (Buffer.byteLength(JSON.stringify(request)) > MAX_REQUEST_BYTES) throw new TypeError('request is too large')
  return request
}

const schemaName = (request) => {
  if (request.schema_name) return request.schema_name.replace(/[^A-Za-z0-9_]/g, '_').slice(0, 64)
  return `mdp_${request.prompt_id.replace(/[^A-Za-z0-9_]/g, '_')}`.slice(0, 64)
}

export const buildProviderRequestBody = (request) => {
  validateNativeModelRequest(request)
  const providerSchema = projectOutputSchemaForOpenAI(request.output_schema)
  const body = {
    model: request.model,
    input: request.input,
    text: {
      format: {
        type: 'json_schema',
        name: schemaName(request),
        strict: true,
        schema: providerSchema,
      },
    },
    store: false,
    tool_choice: 'none',
  }
  if (request.max_output_tokens) body.max_output_tokens = request.max_output_tokens
  if (request.reasoning) body.reasoning = request.reasoning
  if (request.metadata) body.metadata = request.metadata
  return body
}

// This is parity material only. Rust owns the runtime identity and assurance
// decision; this helper exposes the same bounded, secret-free projection for
// cross-language mutation tests.
export const buildModelParametersProjection = (request) => {
  validateNativeModelRequest(request)
  const providerSchema = projectOutputSchemaForOpenAI(request.output_schema)
  return {
    contract: MODEL_PARAMETERS_PROJECTION_CONTRACT,
    provider: request.provider,
    requested_model: request.model,
    authorized_endpoint: OFFICIAL_RESPONSES_ENDPOINT,
    declared_timeout_ms: request.timeout_ms || DEFAULT_REQUEST_TIMEOUT_MS,
    max_output_tokens: request.max_output_tokens || 1,
    structured_output_mode: 'json-schema-strict',
    schema_name: schemaName(request),
    provider_output_schema_sha256: sha256CanonicalJson(providerSchema),
    input_framing: 'one-fresh-user-message:declared-inputs-only',
    visible_input_sha256: sha256Bytes(
      typeof request.input === 'string' ? request.input : canonicalJsonBytes(request.input),
    ),
    store: false,
    tool_choice: 'none',
    continuation_policy: 'none',
    tools_policy: 'none',
    reasoning: request.reasoning ? sha256CanonicalJson(request.reasoning) : null,
    metadata: request.metadata ? sha256CanonicalJson(request.metadata) : null,
  }
}

export const modelParametersProjectionSha256 = (request) =>
  sha256CanonicalJsonForDomain(MODEL_PARAMETERS_PROJECTION_CONTRACT, buildModelParametersProjection(request))

const emptyResult = (
  request,
  terminalState,
  diagnosticCode,
  providerRequestBodySha256 = null,
  providerResponseBodySha256 = null,
  providerOutputSchemaSha256 = null,
) => ({
  contract: DRIVER_RESULT_CONTRACT,
  execution_id: typeof request?.execution_id === 'string' && SAFE_ID.test(request.execution_id)
    ? request.execution_id
    : 'invalid-request',
  terminal_state: terminalState,
  output: null,
  provider_request_body_sha256: providerRequestBodySha256,
  provider_request_schema_id: providerRequestBodySha256 === null ? null : PROVIDER_REQUEST_SCHEMA_ID,
  provider_response_body_sha256: providerResponseBodySha256,
  provider_output_schema_sha256: providerOutputSchemaSha256,
  provider_observation: null,
  diagnostic_code: diagnosticCode,
})

const extractOutputText = (response) => {
  if (response.status && response.status !== 'completed') fault('model_incomplete')
  if (typeof response.output_text === 'string' && response.output_text.trim()) return response.output_text.trim()
  const parts = []
  for (const item of response.output || []) {
    if (item?.type !== 'message') continue
    for (const content of item.content || []) {
      if (content?.type === 'refusal') fault('model_refusal')
      if (['output_text', 'text'].includes(content?.type) && typeof content.text === 'string') parts.push(content.text)
    }
  }
  const text = parts.join('').trim()
  if (!text) fault('provider_response_invalid')
  return text
}

const readBoundedResponse = async (response) => {
  let headerBytes = 0
  for (const [name, value] of response.headers?.entries?.() || []) {
    headerBytes += Buffer.byteLength(name) + Buffer.byteLength(value)
    if (headerBytes > 32 * 1024) fault('provider_response_headers_too_large')
  }
  const contentLength = Number(response.headers?.get?.('content-length'))
  if (Number.isFinite(contentLength) && contentLength > MAX_PROVIDER_RESPONSE_BYTES) fault('provider_response_too_large')
  if (!response.body?.getReader) {
    if (typeof response.arrayBuffer !== 'function') fault('provider_response_invalid')
    const bytes = Buffer.from(await response.arrayBuffer())
    if (bytes.length > MAX_PROVIDER_RESPONSE_BYTES) fault('provider_response_too_large')
    return { text: bytes.toString('utf8'), sha256: sha256Bytes(bytes) }
  }
  const reader = response.body.getReader()
  const chunks = []
  let size = 0
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    size += value.byteLength
    if (size > MAX_PROVIDER_RESPONSE_BYTES) {
      await reader.cancel().catch(() => {})
      fault('provider_response_too_large')
    }
    chunks.push(value)
  }
  const bytes = Buffer.concat(chunks.map((chunk) => Buffer.from(chunk)))
  return { text: bytes.toString('utf8'), sha256: sha256Bytes(bytes) }
}

const realProviderCall = async ({ providerBodyText, environment, fetchImpl, timeoutMs }) => {
  if (environment.MDP_ALLOW_NATIVE_MODEL_CALLS !== '1') {
    fault('native_model_calls_not_allowed', 'no-draft:policy-blocked')
  }
  const apiKey = environment.OPENAI_API_KEY
  if (typeof apiKey !== 'string' || apiKey.trim() === '') {
    fault('openai_api_key_missing', 'no-draft:policy-blocked')
  }
  if (typeof fetchImpl !== 'function') fault('runtime_fetch_unavailable')
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), timeoutMs)
  try {
    let response
    try {
      response = await fetchImpl(OFFICIAL_RESPONSES_ENDPOINT, {
        method: 'POST',
        headers: {
          authorization: `Bearer ${apiKey}`,
          'content-type': 'application/json',
        },
        body: providerBodyText,
        redirect: 'error',
        signal: controller.signal,
      })
    } catch (error) {
      if (error?.name === 'AbortError') fault('provider_timeout')
      fault('provider_transport_error')
    }
    const { text, sha256 } = await readBoundedResponse(response)
    if (!response.ok) fault('provider_http_error')
    try {
      return { response: JSON.parse(text), providerResponseBodySha256: sha256 }
    } catch {
      fault('provider_response_invalid')
    }
  } finally {
    clearTimeout(timeout)
  }
}

export const executeNativeModelRequest = async (request, options = {}) => {
  let providerRequestBodySha256 = null
  let providerResponseBodySha256 = null
  let providerOutputSchemaSha256 = null
  try {
    validateNativeModelRequest(request)
    let providerBody
    try {
      providerBody = buildProviderRequestBody(request)
    } catch {
      fault('output_schema_projection_unsupported', 'no-draft:preflight-refused')
    }
    const providerBodyText = JSON.stringify(providerBody)
    if (Buffer.byteLength(providerBodyText) > MAX_REQUEST_BYTES) fault('provider_request_too_large')
    providerRequestBodySha256 = sha256Bytes(providerBodyText)
    providerOutputSchemaSha256 = sha256CanonicalJson(providerBody.text.format.schema)

    let response
    if (options.mode === 'mock') {
      const rawMockResponse = JSON.stringify(options.mockResponse)
      const mockBytes = Buffer.byteLength(rawMockResponse)
      if (mockBytes > MAX_PROVIDER_RESPONSE_BYTES) fault('provider_response_too_large')
      providerResponseBodySha256 = sha256Bytes(rawMockResponse)
      response = options.mockResponse
    } else if (options.mode === 'dry-run') {
      return emptyResult(
        request,
        'no-draft:policy-blocked',
        'dry_run_complete',
        providerRequestBodySha256,
        providerResponseBodySha256,
        providerOutputSchemaSha256,
      )
    } else {
      const providerResult = await realProviderCall({
        providerBodyText,
        environment: options.environment || process.env,
        fetchImpl: options.fetchImpl || globalThis.fetch,
        timeoutMs: request.timeout_ms || DEFAULT_REQUEST_TIMEOUT_MS,
      })
      response = providerResult.response
      providerResponseBodySha256 = providerResult.providerResponseBodySha256
    }

    requireObject(response, 'provider response')
    validateJsonComplexity(response, 'provider_response_too_deep')
    const outputText = extractOutputText(response)
    if (Buffer.byteLength(outputText) > MAX_OUTPUT_BYTES) fault('model_output_too_large')
    let parsedOutput
    try {
      parsedOutput = JSON.parse(outputText)
    } catch {
      fault('model_output_invalid_json')
    }
    validateJsonComplexity(parsedOutput, 'model_output_too_deep')
    return {
      contract: DRIVER_RESULT_CONTRACT,
      execution_id: request.execution_id,
      terminal_state: 'success',
      output: {
        media_type: 'application/json',
        encoding: 'utf-8',
        content: outputText,
        byte_count: Buffer.byteLength(outputText),
        sha256: sha256Bytes(outputText),
      },
      provider_request_body_sha256: providerRequestBodySha256,
      provider_request_schema_id: PROVIDER_REQUEST_SCHEMA_ID,
      provider_response_body_sha256: providerResponseBodySha256,
      provider_output_schema_sha256: providerOutputSchemaSha256,
      provider_observation: {
        provider: 'openai',
        response_id: safeProviderIdentifier(response.id),
        model: safeProviderIdentifier(response.model),
      },
      diagnostic_code: null,
    }
  } catch (error) {
    if (error instanceof DriverFault) {
      return emptyResult(
        request,
        error.terminalState,
        error.code,
        providerRequestBodySha256,
        providerResponseBodySha256,
        providerOutputSchemaSha256,
      )
    }
    return emptyResult(
      request,
      'no-draft:preflight-refused',
      'request_invalid',
      providerRequestBodySha256,
      providerResponseBodySha256,
      providerOutputSchemaSha256,
    )
  }
}

const usage = () => `
Usage:
  node scripts/mdp-native-model-openai.mjs [--request REQUEST.json] [--mock-response RESPONSE.json | --dry-run]
  node scripts/mdp-native-model-openai.mjs --project-schema SCHEMA.json

Without --request, exactly one bounded ${DRIVER_REQUEST_CONTRACT} object is read from stdin.
Exactly one ${DRIVER_RESULT_CONTRACT} object is written to stdout. Real calls require both
MDP_ALLOW_NATIVE_MODEL_CALLS=1 and OPENAI_API_KEY. The endpoint is fixed to OpenAI Responses.
`.trim()

const readStdinBounded = async () => {
  const chunks = []
  let size = 0
  for await (const chunk of process.stdin) {
    size += chunk.length
    if (size > MAX_REQUEST_BYTES) fault('request_too_large')
    chunks.push(chunk)
  }
  return Buffer.concat(chunks).toString('utf8')
}

const parseArgs = (argv) => {
  const args = { request: null, mockResponse: null, projectSchema: null, dryRun: false }
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    const next = () => {
      index += 1
      if (index >= argv.length) throw new TypeError(`Missing value for ${arg}`)
      return argv[index]
    }
    if (arg === '--request') args.request = next()
    else if (arg === '--mock-response') args.mockResponse = next()
    else if (arg === '--project-schema') args.projectSchema = next()
    else if (arg === '--dry-run') args.dryRun = true
    else if (arg === '--help' || arg === '-h') {
      console.log(usage())
      process.exit(0)
    } else throw new TypeError(`Unknown argument: ${arg}`)
  }
  if (args.mockResponse && args.dryRun) throw new TypeError('--mock-response and --dry-run are mutually exclusive')
  if (args.projectSchema && (args.request || args.mockResponse || args.dryRun)) {
    throw new TypeError('--project-schema cannot be combined with run arguments')
  }
  return args
}

const readJsonPath = (path, maxBytes) => {
  if (!existsSync(path)) throw new TypeError('request file not found')
  const bytes = readFileSync(path)
  if (bytes.length > maxBytes) fault('request_too_large')
  return JSON.parse(bytes.toString('utf8'))
}

const main = async () => {
  let request = null
  try {
    const args = parseArgs(process.argv.slice(2))
    if (args.projectSchema) {
      const canonicalSchema = readJsonPath(args.projectSchema, MAX_REQUEST_BYTES)
      const providerSchema = projectOutputSchemaForOpenAI(canonicalSchema)
      process.stdout.write(`${JSON.stringify({
        contract: SCHEMA_PROJECTION_CONTRACT,
        canonical_output_schema_sha256: sha256CanonicalJson(canonicalSchema),
        provider_output_schema: providerSchema,
        provider_output_schema_sha256: sha256CanonicalJson(providerSchema),
      })}\n`)
      return
    }
    const requestText = args.request ? null : await readStdinBounded()
    request = args.request
      ? readJsonPath(args.request, MAX_REQUEST_BYTES)
      : JSON.parse(requestText)
    const mockResponse = args.mockResponse
      ? readJsonPath(args.mockResponse, MAX_PROVIDER_RESPONSE_BYTES)
      : null
    const result = await executeNativeModelRequest(request, {
      mode: args.mockResponse ? 'mock' : args.dryRun ? 'dry-run' : 'real',
      mockResponse,
    })
    process.stdout.write(`${JSON.stringify(result)}\n`)
  } catch (error) {
    const diagnosticCode = error instanceof DriverFault ? error.code : 'request_invalid'
    process.stdout.write(`${JSON.stringify(emptyResult(request, 'no-draft:preflight-refused', diagnosticCode))}\n`)
  }
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  await main()
}
