#!/usr/bin/env node
import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'

import {
  DRIVER_REQUEST_CONTRACT,
  buildProviderRequestBody,
  executeNativeModelRequest,
  sha256CanonicalJson,
} from './mdp-native-model-openai.mjs'

const RUNNER_CONTRACT = 'mdp.runner-audit.v0'
const REQUEST_CONTRACT = 'mdp.native-normalize-request.v0'

const usage = () => `
Usage:
  node scripts/mdp-native-normalize-openai.mjs --request REQUEST.json --out OUTPUT.json --runner-audit RUNNER_AUDIT.json
  node scripts/mdp-native-normalize-openai.mjs --request REQUEST.json --dry-run
  node scripts/mdp-native-normalize-openai.mjs --request REQUEST.json --mock-response RESPONSE.json --out OUTPUT.json --runner-audit RUNNER_AUDIT.json

Environment for real runs:
  MDP_ALLOW_NATIVE_MODEL_CALLS=1  Explicit permission for a real provider call.
  OPENAI_API_KEY                  Credential for that call.

This legacy normalization entry point translates its v0 request into the universal,
profile-neutral ${DRIVER_REQUEST_CONTRACT} subprocess protocol. The provider endpoint
is fixed to the official OpenAI Responses endpoint. Raw provider envelopes are not retained.
`.trim()

const fail = (message, code = 1) => {
  console.error(message)
  process.exit(code)
}

const parseArgs = (argv) => {
  const args = { request: null, out: null, runnerAudit: null, mockResponse: null, dryRun: false }
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    const next = () => {
      index += 1
      if (index >= argv.length) fail(`Missing value for ${arg}`)
      return argv[index]
    }
    if (arg === '--request') args.request = next()
    else if (arg === '--out') args.out = next()
    else if (arg === '--runner-audit') args.runnerAudit = next()
    else if (arg === '--mock-response') args.mockResponse = next()
    else if (arg === '--response') fail('--response is no longer supported; raw provider envelopes are memory-only')
    else if (arg === '--dry-run') args.dryRun = true
    else if (arg === '--help' || arg === '-h') {
      console.log(usage())
      process.exit(0)
    } else fail(`Unknown argument: ${arg}\n\n${usage()}`)
  }
  if (!args.request) fail(`Missing --request\n\n${usage()}`)
  if (args.dryRun && args.mockResponse) fail('--dry-run and --mock-response are mutually exclusive')
  if (!args.dryRun && (!args.out || !args.runnerAudit)) fail('Real and mock runs require --out and --runner-audit')
  return args
}

const readJson = (path) => {
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch {
    fail(`${path} must contain valid JSON`)
  }
}

const writeJson = (path, value) => {
  mkdirSync(dirname(resolve(path)), { recursive: true })
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`)
}

const sha256File = (path) => createHash('sha256').update(readFileSync(path)).digest('hex')

const requireObject = (value, label) => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) fail(`${label} must be an object`)
}

const requireString = (value, label) => {
  if (typeof value !== 'string' || value.trim() === '') fail(`${label} must be a non-empty string`)
}

const validateInputPayload = (input) => {
  if (typeof input === 'string') {
    requireString(input, 'request.input')
    return
  }
  if (!Array.isArray(input) || input.length !== 1) {
    fail('request.input must contain exactly one user message')
  }
  requireObject(input[0], 'request.input[0]')
  const unknown = Object.keys(input[0]).filter((key) => !['role', 'content'].includes(key)).sort()
  if (unknown.length > 0) fail(`request.input[0] contains unsupported fields: ${unknown.join(', ')}`)
  if (input[0].role !== 'user') fail('request.input[0].role must be user')
  requireString(input[0].content, 'request.input[0].content')
}

const validateLegacyRequest = (request) => {
  requireObject(request, 'request')
  const allowed = new Set([
    'contract', 'provider', 'model', 'prompt_id', 'declared_inputs_only', 'input',
    'prompt_output_schema', 'schema_name', 'max_output_tokens', 'reasoning', 'metadata',
    'tools', 'tool_choice',
  ])
  const unsupported = Object.keys(request).filter((key) => !allowed.has(key)).sort()
  if (unsupported.length > 0) fail(`request contains unsupported fields: ${unsupported.join(', ')}`)
  if (request.contract !== REQUEST_CONTRACT) fail(`request.contract must be ${REQUEST_CONTRACT}`)
  if (request.provider !== 'openai') fail('request.provider must be openai')
  requireString(request.model, 'request.model')
  requireString(request.prompt_id, 'request.prompt_id')
  if (request.declared_inputs_only !== true) fail('request.declared_inputs_only must be true')
  requireObject(request.prompt_output_schema, 'request.prompt_output_schema')
  validateInputPayload(request.input)
  if ('tools' in request && (!Array.isArray(request.tools) || request.tools.length > 0)) {
    fail('request.tools must be omitted or empty')
  }
  if ('tool_choice' in request && request.tool_choice !== 'none') {
    fail('request.tool_choice must be omitted or set to none')
  }
}

const translateRequest = (legacy, requestSha256) => {
  validateLegacyRequest(legacy)
  const translated = {
    contract: DRIVER_REQUEST_CONTRACT,
    execution_id: `legacy-${requestSha256.slice(0, 24)}`,
    provider: 'openai',
    model: legacy.model,
    prompt_id: legacy.prompt_id,
    declared_inputs_only: true,
    input: legacy.input,
    output_schema: legacy.prompt_output_schema,
    output_schema_sha256: sha256CanonicalJson(legacy.prompt_output_schema),
  }
  if (legacy.schema_name) translated.schema_name = legacy.schema_name
  if (legacy.max_output_tokens) translated.max_output_tokens = legacy.max_output_tokens
  if (legacy.reasoning) translated.reasoning = legacy.reasoning
  if (legacy.metadata) translated.metadata = legacy.metadata
  return translated
}

const main = async () => {
  const args = parseArgs(process.argv.slice(2))
  if (!existsSync(args.request)) fail(`Request file not found: ${args.request}`)
  const legacyRequest = readJson(args.request)
  const requestSha256 = sha256File(args.request)
  const request = translateRequest(legacyRequest, requestSha256)
  const providerBody = buildProviderRequestBody(request)

  if (args.dryRun) {
    console.log(JSON.stringify({
      ok: true,
      contract: 'mdp.native-normalize-dry-run.v0',
      delegated_contract: DRIVER_REQUEST_CONTRACT,
      provider: 'openai',
      endpoint: '/v1/responses',
      endpoint_policy: 'official-fixed',
      model: request.model,
      prompt_id: request.prompt_id,
      declared_inputs_only: true,
      output_schema_used: true,
      store: false,
      tools_disabled: true,
      requires_api_key_for_real_run: true,
      requires_native_call_permission_for_real_run: true,
      request_sha256: requestSha256,
      api_request_preview: {
        model: providerBody.model,
        input_kind: Array.isArray(providerBody.input) ? 'array' : 'string',
        text_format: providerBody.text.format.type,
        schema_name: providerBody.text.format.name,
        strict: providerBody.text.format.strict,
        store: providerBody.store,
        tool_choice: providerBody.tool_choice,
      },
    }, null, 2))
    return
  }

  const result = await executeNativeModelRequest(request, {
    mode: args.mockResponse ? 'mock' : 'real',
    mockResponse: args.mockResponse ? readJson(args.mockResponse) : null,
  })
  if (result.terminal_state !== 'success' || !result.output) {
    fail(`Native model call failed safely: ${result.diagnostic_code || 'runner_failed'}`)
  }

  const promptOutput = JSON.parse(result.output.content)
  writeJson(args.out, promptOutput)
  writeJson(args.runnerAudit, {
    contract: RUNNER_CONTRACT,
    runner: 'native-api',
    model: request.model,
    isolated_invocation: !args.mockResponse,
    conversation_resume: false,
    declared_inputs_only: true,
    output_schema_used: true,
    stateless_request: !args.mockResponse,
    prior_messages_included: false,
    tools_disabled: true,
    tool_invocations_observed: 0,
    endpoint: '/v1/responses',
    endpoint_policy: 'official-fixed',
    store: false,
    prompt_id: request.prompt_id,
    prompt_output_sha256: sha256File(args.out),
    request_sha256: requestSha256,
    response_id: result.provider_observation?.response_id || null,
    mock_response: Boolean(args.mockResponse),
  })
  console.log(JSON.stringify({
    ok: true,
    contract: 'mdp.native-normalize-result.v0',
    delegated_contract: DRIVER_REQUEST_CONTRACT,
    prompt_output: args.out,
    runner_audit: args.runnerAudit,
    response: null,
    audit_grade_eligible: !args.mockResponse,
  }, null, 2))
}

main().catch(() => fail('Native model runner failed safely: internal_error'))
