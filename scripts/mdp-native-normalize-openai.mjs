#!/usr/bin/env node
import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'

const RUNNER_CONTRACT = 'mdp.runner-audit.v0'
const REQUEST_CONTRACT = 'mdp.native-normalize-request.v0'
const DEFAULT_BASE_URL = 'https://api.openai.com/v1'

const usage = () => `
Usage:
  node scripts/mdp-native-normalize-openai.mjs --request REQUEST.json --out OUTPUT.json --runner-audit RUNNER_AUDIT.json
  node scripts/mdp-native-normalize-openai.mjs --request REQUEST.json --dry-run
  node scripts/mdp-native-normalize-openai.mjs --request REQUEST.json --mock-response RESPONSE.json --out OUTPUT.json --runner-audit RUNNER_AUDIT.json

Environment for real runs:
  OPENAI_API_KEY     Required only when neither --dry-run nor --mock-response is used.
  OPENAI_BASE_URL    Optional. Defaults to ${DEFAULT_BASE_URL}.

This script is an optional BYOK/native runner reference. It does not create API keys,
read .env files, parse PDFs, or mutate packs. The host must build REQUEST.json from
prompt-declared inputs only, then validate the output with mdp validate-prompt-output.
`.trim()

const fail = (message, code = 1) => {
  console.error(message)
  process.exit(code)
}

const parseArgs = (argv) => {
  const args = {
    request: null,
    out: null,
    runnerAudit: null,
    response: null,
    mockResponse: null,
    dryRun: false,
  }
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    const next = () => {
      index += 1
      if (index >= argv.length) fail(`Missing value for ${arg}`)
      return argv[index]
    }
    switch (arg) {
      case '--request':
        args.request = next()
        break
      case '--out':
        args.out = next()
        break
      case '--runner-audit':
        args.runnerAudit = next()
        break
      case '--response':
        args.response = next()
        break
      case '--mock-response':
        args.mockResponse = next()
        break
      case '--dry-run':
        args.dryRun = true
        break
      case '-h':
      case '--help':
        console.log(usage())
        process.exit(0)
        break
      default:
        fail(`Unknown argument: ${arg}\n\n${usage()}`)
    }
  }
  if (!args.request) fail(`Missing --request\n\n${usage()}`)
  if (!args.dryRun && (!args.out || !args.runnerAudit)) {
    fail('Real and mock runs require --out and --runner-audit')
  }
  return args
}

const readJson = (path) => {
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch (error) {
    fail(`${path} must contain valid JSON: ${error.message}`)
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
    if (input.trim() === '') fail('request.input must be a non-empty string')
    return
  }

  if (!Array.isArray(input)) fail('request.input must be a Responses API input string or single user message array')
  if (input.length !== 1) {
    fail('request.input array must contain exactly one user message; include all prompt guidance in that audited payload')
  }

  const [message] = input
  requireObject(message, 'request.input[0]')
  if (message.role !== 'user') fail('request.input[0].role must be user')
  if (!('content' in message)) fail('request.input[0].content is required')
  if ('id' in message || 'status' in message || 'type' in message) {
    fail('request.input[0] must be a plain user message without prior response metadata')
  }
}

const validateRequest = (request) => {
  requireObject(request, 'request')
  if (request.contract !== REQUEST_CONTRACT) fail(`request.contract must be ${REQUEST_CONTRACT}`)
  if (request.provider !== 'openai') fail('request.provider must be openai')
  requireString(request.model, 'request.model')
  requireString(request.prompt_id, 'request.prompt_id')
  if (request.declared_inputs_only !== true) fail('request.declared_inputs_only must be true')
  requireObject(request.prompt_output_schema, 'request.prompt_output_schema')
  validateInputPayload(request.input)
  if ('instructions' in request) fail('request.instructions is not allowed; include all model-visible prompt guidance in the audited request.input payload')
  if ('previous_response_id' in request) fail('request must not include previous_response_id')
  if ('conversation' in request) fail('request must not include conversation')
  if ('tools' in request && (!Array.isArray(request.tools) || request.tools.length > 0)) {
    fail('request.tools must be omitted or empty for native normalization')
  }
  if ('tool_choice' in request && request.tool_choice !== 'none') {
    fail('request.tool_choice must be omitted or set to none')
  }
}

const buildResponsesBody = (request) => {
  const schemaName = request.schema_name || 'mdp_prompt_output'
  const body = {
    model: request.model,
    input: request.input,
    text: {
      format: {
        type: 'json_schema',
        name: schemaName,
        strict: true,
        schema: request.prompt_output_schema,
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

const extractOutputText = (response) => {
  if (typeof response.output_text === 'string' && response.output_text.trim()) return response.output_text

  const parts = []
  for (const item of response.output || []) {
    if (item?.type !== 'message') continue
    for (const content of item.content || []) {
      if (content?.type === 'refusal') {
        fail(`Model refusal: ${content.refusal || 'no refusal message supplied'}`)
      }
      if (content?.type === 'output_text' && typeof content.text === 'string') parts.push(content.text)
      if (content?.type === 'text' && typeof content.text === 'string') parts.push(content.text)
    }
  }

  const text = parts.join('').trim()
  if (!text) fail('OpenAI response did not include output_text content')
  return text
}

const parsePromptOutput = (response) => {
  const text = extractOutputText(response)
  try {
    return JSON.parse(text)
  } catch (error) {
    fail(`Model output was not parseable JSON: ${error.message}`)
  }
}

const callOpenAI = async (body) => {
  if (typeof fetch !== 'function') {
    fail('This runner requires Node.js 18+ for the built-in fetch API.')
  }
  const apiKey = process.env.OPENAI_API_KEY
  if (!apiKey) fail('OPENAI_API_KEY is required for a real native run. Use --dry-run or --mock-response for offline validation.')
  const baseUrl = (process.env.OPENAI_BASE_URL || DEFAULT_BASE_URL).replace(/\/+$/, '')
  const response = await fetch(`${baseUrl}/responses`, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${apiKey}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify(body),
  })
  const text = await response.text()
  let payload
  try {
    payload = JSON.parse(text)
  } catch {
    fail(`OpenAI API returned non-JSON response (${response.status}): ${text.slice(0, 500)}`)
  }
  if (!response.ok) {
    const message = payload?.error?.message || `OpenAI API request failed with status ${response.status}`
    fail(message)
  }
  return payload
}

const runnerAudit = ({ request, requestPath, promptOutputPath, response, mock }) => ({
  contract: RUNNER_CONTRACT,
  runner: 'native-api',
  model: request.model,
  isolated_invocation: !mock,
  conversation_resume: false,
  declared_inputs_only: true,
  output_schema_used: true,
  stateless_request: !mock,
  prior_messages_included: false,
  tools_disabled: true,
  tool_invocations_observed: 0,
  endpoint: '/v1/responses',
  store: false,
  prompt_id: request.prompt_id,
  prompt_output_sha256: sha256File(promptOutputPath),
  request_sha256: sha256File(requestPath),
  response_id: response?.id || null,
  mock_response: mock,
})

const main = async () => {
  const args = parseArgs(process.argv.slice(2))
  if (!existsSync(args.request)) fail(`Request file not found: ${args.request}`)
  const request = readJson(args.request)
  validateRequest(request)
  const body = buildResponsesBody(request)

  if (args.dryRun) {
    console.log(JSON.stringify({
      ok: true,
      contract: 'mdp.native-normalize-dry-run.v0',
      provider: 'openai',
      endpoint: '/v1/responses',
      model: request.model,
      prompt_id: request.prompt_id,
      declared_inputs_only: true,
      output_schema_used: true,
      store: false,
      tools_disabled: true,
      requires_api_key_for_real_run: true,
      request_sha256: sha256File(args.request),
      api_request_preview: {
        model: body.model,
        input_kind: Array.isArray(body.input) ? 'array' : 'string',
        text_format: body.text.format.type,
        schema_name: body.text.format.name,
        strict: body.text.format.strict,
        store: body.store,
        tool_choice: body.tool_choice,
      },
    }, null, 2))
    return
  }

  const response = args.mockResponse ? readJson(args.mockResponse) : await callOpenAI(body)
  if (args.response) writeJson(args.response, response)
  const promptOutput = parsePromptOutput(response)
  writeJson(args.out, promptOutput)
  writeJson(args.runnerAudit, runnerAudit({
    request,
    requestPath: args.request,
    promptOutputPath: args.out,
    response,
    mock: Boolean(args.mockResponse),
  }))
  console.log(JSON.stringify({
    ok: true,
    contract: 'mdp.native-normalize-result.v0',
    prompt_output: args.out,
    runner_audit: args.runnerAudit,
    response: args.response || null,
    audit_grade_eligible: !args.mockResponse,
  }, null, 2))
}

main().catch((error) => fail(error.stack || error.message))
