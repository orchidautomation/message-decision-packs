#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const MCP_PROTOCOL_VERSION = '2025-06-18'
const SERVER_NAME = 'message-decision-packs-proposal'
const MAX_OUTPUT_CHARS = 80_000
const JSON_RPC_PARSE_ERROR = -32700
const JSON_RPC_INVALID_REQUEST = -32600
const JSON_RPC_METHOD_NOT_FOUND = -32601
const JSON_RPC_INVALID_PARAMS = -32602

const scriptDir = dirname(fileURLToPath(import.meta.url))
const bundleRoot = resolve(scriptDir, '..')
const runnerPath = join(scriptDir, 'mdp-proposal-runner.mjs')

const readVersion = () => {
  const candidates = [
    join(bundleRoot, 'plugin.json'),
    join(bundleRoot, '.codex-plugin', 'plugin.json'),
    join(bundleRoot, 'plugin', '.codex-plugin', 'plugin.json'),
  ]
  for (const pluginJson of candidates) {
    if (!existsSync(pluginJson)) continue
    try {
      const value = JSON.parse(readFileSync(pluginJson, 'utf8'))
      if (typeof value.version === 'string' && value.version.trim()) return value.version
    } catch {
      // fall through
    }
  }
  return '0.0.0-local'
}

const serverVersion = readVersion()

const compact = (value, limit = MAX_OUTPUT_CHARS) => {
  const text = typeof value === 'string' ? value : JSON.stringify(value, null, 2)
  if (text.length <= limit) return text
  return `${text.slice(0, limit)}\n... [truncated ${text.length - limit} chars]`
}

const writeMessage = (message) => {
  process.stdout.write(`${JSON.stringify(message)}\n`)
}

const response = (id, result) => ({ jsonrpc: '2.0', id, result })
const errorResponse = (id, code, message, data) => ({
  jsonrpc: '2.0',
  id: id ?? null,
  error: data === undefined ? { code, message } : { code, message, data },
})

const asObject = (value, label) => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`)
  }
  return value
}

const optionalString = (args, key) => {
  if (!(key in args) || args[key] === null || args[key] === undefined) return null
  if (typeof args[key] !== 'string' || args[key].trim() === '') throw new Error(`${key} must be a non-empty string`)
  return args[key]
}

const optionalBoolean = (args, key, defaultValue = false) => {
  if (!(key in args) || args[key] === null || args[key] === undefined) return defaultValue
  if (typeof args[key] !== 'boolean') throw new Error(`${key} must be a boolean`)
  return args[key]
}

const optionalInteger = (args, key) => {
  if (!(key in args) || args[key] === null || args[key] === undefined) return null
  if (!Number.isInteger(args[key])) throw new Error(`${key} must be an integer`)
  return args[key]
}

const optionalStringArray = (args, key) => {
  if (!(key in args) || args[key] === null || args[key] === undefined) return []
  if (!Array.isArray(args[key]) || args[key].some((item) => typeof item !== 'string' || item.trim() === '')) {
    throw new Error(`${key} must be an array of non-empty strings`)
  }
  return args[key]
}

const assertNoUnsupportedArgs = (args, allowed) => {
  const unsupported = Object.keys(args).filter((key) => !allowed.has(key))
  if (unsupported.length > 0) {
    throw new Error(`Unsupported arguments: ${unsupported.sort().join(', ')}. Use local file paths only; raw source_text/chat_context inputs are intentionally not accepted.`)
  }
}

const toolResult = ({ text, structuredContent, isError = false }) => ({
  content: [{ type: 'text', text }],
  structuredContent,
  isError,
})

const runNode = (script, args) => {
  const result = spawnSync('node', [script, ...args], {
    cwd: bundleRoot,
    encoding: 'utf8',
    env: process.env,
    maxBuffer: 30 * 1024 * 1024,
  })
  if (result.error) {
    return {
      status: 1,
      stdout: result.stdout || '',
      stderr: `${result.stderr || ''}${result.error.message}`,
    }
  }
  return {
    status: result.status ?? 1,
    stdout: result.stdout || '',
    stderr: result.stderr || '',
  }
}

const parseRunnerJson = (stdout) => {
  try {
    return JSON.parse(stdout)
  } catch (error) {
    throw new Error(`runner stdout was not valid JSON: ${error.message}`)
  }
}

const proposalToolsSchema = {
  type: 'object',
  additionalProperties: false,
  properties: {},
}

const proposalRunSchema = {
  type: 'object',
  additionalProperties: false,
  required: ['pack', 'workdir'],
  properties: {
    pack: {
      type: 'string',
      description: 'Local proposal MDP pack root containing .mdp/.',
    },
    workdir: {
      type: 'string',
      description: 'Customer-controlled local scratch directory for runner artifacts. Must be empty unless allow_existing is true.',
    },
    source_paths: {
      type: 'array',
      items: { type: 'string' },
      description: 'Local text/Markdown/CSV/JSON/YAML source files supplied by the operator. Raw chat text is intentionally not accepted.',
    },
    source_audit_path: {
      type: 'string',
      description: 'Existing mdp.source-audit.v0 JSON ledger to preserve. Optional when source_paths + source_id can generate one.',
    },
    source_id: {
      type: 'string',
      description: 'Source id from .mdp/sources.yaml used when generating source-audit refs from source_paths.',
    },
    source_kind: {
      type: 'string',
      enum: ['user-provided-opportunity', 'private-scratch-opportunity', 'public-source', 'sanitized-example', 'synthetic-example'],
      description: 'Prompt source_kind. Defaults to private-scratch-opportunity.',
    },
    model: {
      type: 'string',
      description: 'Model id for real native runs. Dry-run/mock modes default to gpt-test.',
    },
    mock_response_path: {
      type: 'string',
      description: 'Offline provider response fixture path. Mock mode is never audit-grade.',
    },
    dry_run: {
      type: 'boolean',
      description: 'Validate request shape only; no model output, receipt, fit, or route.',
    },
    mdp_bin: {
      type: 'string',
      description: 'Optional mdp executable path. Must be a path, not a shell command.',
    },
    native_runner: {
      type: 'string',
      description: 'Optional native runner script path. Defaults to bundled mdp-native-normalize-openai.mjs.',
    },
    prompt_id: {
      type: 'string',
      description: 'Prompt id. Current runner supports normalize-opportunity.',
    },
    allow_existing: {
      type: 'boolean',
      description: 'Allow writing into an existing non-empty workdir without deleting it.',
    },
    skip_review: {
      type: 'boolean',
      description: 'Skip fit/route review-support probes after receipt.',
    },
    require_audit_grade: {
      type: 'boolean',
      description: 'Return a tool error unless run-receipt returns decision audit-grade.',
    },
    max_source_bytes: {
      type: 'integer',
      minimum: 1000,
      description: 'Per-source bounded text bytes to include in the prompt payload.',
    },
  },
}

const tools = [
  {
    name: 'mdp_proposal_tools',
    title: 'Inspect MDP proposal runner boundaries',
    description:
      'Return the local proposal runner tool-boundary contract. This is read-only and helps the host understand source intake, normalization, validation, receipt, and review phases.',
    inputSchema: proposalToolsSchema,
  },
  {
    name: 'mdp_proposal_run',
    title: 'Run MDP proposal normalization pipeline',
    description:
      'Run the local proposal runner from explicit local file paths only. It stages supplied sources, builds a declared-input-only native request, optionally invokes the native runner, validates prompt output, creates a run receipt, and runs review probes. Dry-run/mock modes are never audit-grade; real audit-grade still requires valid runner-audit evidence and mdp run-receipt --require-runner-audit.',
    inputSchema: proposalRunSchema,
  },
]

const callProposalTools = (args) => {
  const parsedArgs = asObject(args || {}, 'arguments')
  assertNoUnsupportedArgs(parsedArgs, new Set())
  const result = runNode(runnerPath, ['tools'])
  if (result.status !== 0) {
    return toolResult({
      isError: true,
      text: compact(`proposal runner tools command failed (${result.status})\n${result.stderr || result.stdout}`),
      structuredContent: {
        ok: false,
        contract: 'mdp.proposal-mcp-error.v0',
        status: result.status,
        stderr: compact(result.stderr, 12_000),
        stdout: compact(result.stdout, 12_000),
      },
    })
  }
  const envelope = parseRunnerJson(result.stdout)
  const structuredContent = {
    ok: true,
    contract: 'mdp.proposal-mcp-tools.v0',
    mcp_transport: 'stdio',
    hosted_or_remote_mcp: false,
    server: { name: SERVER_NAME, version: serverVersion },
    runner_tools: envelope,
    guardrails: [
      'The local stdio MCP server is a wrapper around the local runner; it is not hosted or remote.',
      'The MCP API accepts local file paths, not raw chat/source text, so the runner boundary starts from explicit operator-supplied files.',
      'Audit-grade status still requires a real native/headless runner-audit and mdp run-receipt --require-runner-audit.',
      'Dry-run, mock, demo, fixture, or synthetic runner evidence must remain blocked/non-audit-grade.',
    ],
  }
  return toolResult({ text: JSON.stringify(structuredContent, null, 2), structuredContent })
}

const callProposalRun = (args) => {
  const parsedArgs = asObject(args || {}, 'arguments')
  const allowed = new Set([
    'pack',
    'workdir',
    'source_paths',
    'source_audit_path',
    'source_id',
    'source_kind',
    'model',
    'mock_response_path',
    'dry_run',
    'mdp_bin',
    'native_runner',
    'prompt_id',
    'allow_existing',
    'skip_review',
    'require_audit_grade',
    'max_source_bytes',
  ])
  assertNoUnsupportedArgs(parsedArgs, allowed)

  const pack = optionalString(parsedArgs, 'pack')
  const workdir = optionalString(parsedArgs, 'workdir')
  if (!pack) throw new Error('pack is required')
  if (!workdir) throw new Error('workdir is required')

  const sourcePaths = optionalStringArray(parsedArgs, 'source_paths')
  const sourceAuditPath = optionalString(parsedArgs, 'source_audit_path')
  const sourceId = optionalString(parsedArgs, 'source_id')
  const sourceKind = optionalString(parsedArgs, 'source_kind')
  const model = optionalString(parsedArgs, 'model')
  const mockResponsePath = optionalString(parsedArgs, 'mock_response_path')
  const mdpBin = optionalString(parsedArgs, 'mdp_bin')
  const nativeRunner = optionalString(parsedArgs, 'native_runner')
  const promptId = optionalString(parsedArgs, 'prompt_id')
  const maxSourceBytes = optionalInteger(parsedArgs, 'max_source_bytes')
  const dryRun = optionalBoolean(parsedArgs, 'dry_run')
  const allowExisting = optionalBoolean(parsedArgs, 'allow_existing')
  const skipReview = optionalBoolean(parsedArgs, 'skip_review')
  const requireAuditGrade = optionalBoolean(parsedArgs, 'require_audit_grade')

  if (!sourceAuditPath && sourcePaths.length === 0) {
    throw new Error('Pass source_paths and source_id, or pass source_audit_path. Ambient chat/source text is intentionally not accepted.')
  }

  const runnerArgs = ['run', '--pack', pack, '--workdir', workdir]
  for (const sourcePath of sourcePaths) runnerArgs.push('--source', sourcePath)
  if (sourceAuditPath) runnerArgs.push('--source-audit', sourceAuditPath)
  if (sourceId) runnerArgs.push('--source-id', sourceId)
  if (sourceKind) runnerArgs.push('--source-kind', sourceKind)
  if (model) runnerArgs.push('--model', model)
  if (mockResponsePath) runnerArgs.push('--mock-response', mockResponsePath)
  if (dryRun) runnerArgs.push('--dry-run')
  if (mdpBin) runnerArgs.push('--mdp-bin', mdpBin)
  if (nativeRunner) runnerArgs.push('--native-runner', nativeRunner)
  if (promptId) runnerArgs.push('--prompt-id', promptId)
  if (allowExisting) runnerArgs.push('--allow-existing')
  if (skipReview) runnerArgs.push('--skip-review')
  if (requireAuditGrade) runnerArgs.push('--require-audit-grade')
  if (maxSourceBytes !== null) runnerArgs.push('--max-source-bytes', String(maxSourceBytes))

  const result = runNode(runnerPath, runnerArgs)
  let parsed = null
  try {
    parsed = parseRunnerJson(result.stdout)
  } catch (error) {
    if (result.status === 0) throw error
  }

  const ok = result.status === 0 && parsed && parsed.ok !== false
  const structuredContent = {
    ok,
    contract: 'mdp.proposal-mcp-run-result.v0',
    mcp_transport: 'stdio',
    hosted_or_remote_mcp: false,
    runner_exit_status: result.status,
    runner_result: parsed,
    stderr: result.stderr ? compact(result.stderr, 12_000) : '',
    guardrails: [
      'This MCP tool passed only explicit local file/path arguments to the proposal runner.',
      'The model isolation claim comes from the runner-audit plus mdp run-receipt, not from MCP transport alone.',
      'Dry-run/mock/demo/fixture/synthetic evidence remains non-audit-grade.',
    ],
  }

  if (result.status !== 0) {
    return toolResult({
      isError: true,
      text: compact(JSON.stringify(structuredContent, null, 2)),
      structuredContent,
    })
  }

  return toolResult({ text: JSON.stringify(structuredContent, null, 2), structuredContent })
}

const handleToolCall = (params) => {
  const call = asObject(params || {}, 'params')
  if (typeof call.name !== 'string' || call.name.trim() === '') throw new Error('params.name must be a non-empty string')
  const args = call.arguments || {}
  switch (call.name) {
    case 'mdp_proposal_tools':
      return callProposalTools(args)
    case 'mdp_proposal_run':
      return callProposalRun(args)
    default:
      throw Object.assign(new Error(`Unknown tool: ${call.name}`), { code: JSON_RPC_METHOD_NOT_FOUND })
  }
}

const handleRequest = (message) => {
  if (!message || typeof message !== 'object' || Array.isArray(message)) {
    return errorResponse(null, JSON_RPC_INVALID_REQUEST, 'Invalid JSON-RPC message')
  }
  if (message.jsonrpc !== '2.0') {
    return errorResponse(message.id, JSON_RPC_INVALID_REQUEST, 'jsonrpc must be 2.0')
  }
  const isNotification = !('id' in message)
  const { id, method, params } = message
  if (typeof method !== 'string' || method.trim() === '') {
    if (isNotification) return null
    return errorResponse(id, JSON_RPC_INVALID_REQUEST, 'method must be a non-empty string')
  }

  try {
    switch (method) {
      case 'initialize': {
        if (isNotification) return null
        return response(id, {
          protocolVersion: params?.protocolVersion || MCP_PROTOCOL_VERSION,
          capabilities: {
            tools: { listChanged: false },
          },
          serverInfo: {
            name: SERVER_NAME,
            version: serverVersion,
          },
          instructions:
            'Use mdp_proposal_run only with explicit local file paths. Do not pass ambient chat/source text as proposal evidence. Audit-grade requires a real runner-audit and mdp run-receipt --require-runner-audit.',
        })
      }
      case 'notifications/initialized':
        return null
      case 'ping':
        if (isNotification) return null
        return response(id, {})
      case 'tools/list':
        if (isNotification) return null
        return response(id, { tools })
      case 'tools/call':
        if (isNotification) return null
        return response(id, handleToolCall(params))
      default:
        if (isNotification) return null
        return errorResponse(id, JSON_RPC_METHOD_NOT_FOUND, `Method not found: ${method}`)
    }
  } catch (error) {
    const code = Number.isInteger(error.code) ? error.code : JSON_RPC_INVALID_PARAMS
    if (isNotification) return null
    return errorResponse(id, code, error.message || 'Tool call failed')
  }
}

const handleLine = (line) => {
  const trimmed = line.trim()
  if (!trimmed) return
  let message
  try {
    message = JSON.parse(trimmed)
  } catch (error) {
    writeMessage(errorResponse(null, JSON_RPC_PARSE_ERROR, `Parse error: ${error.message}`))
    return
  }

  if (Array.isArray(message)) {
    const responses = []
    for (const item of message) {
      const itemResponse = handleRequest(item)
      if (itemResponse) responses.push(itemResponse)
    }
    if (responses.length > 0) writeMessage(responses)
    return
  }

  const messageResponse = handleRequest(message)
  if (messageResponse) writeMessage(messageResponse)
}

let buffer = ''
process.stdin.setEncoding('utf8')
process.stdin.resume()
process.stdin.on('data', (chunk) => {
  buffer += chunk
  let newlineIndex
  while ((newlineIndex = buffer.indexOf('\n')) >= 0) {
    const line = buffer.slice(0, newlineIndex)
    buffer = buffer.slice(newlineIndex + 1)
    handleLine(line)
  }
})

process.stdin.on('end', () => {
  const remaining = buffer.trim()
  if (remaining) handleLine(remaining)
})

process.on('uncaughtException', (error) => {
  process.stderr.write(`mdp proposal MCP server fatal error: ${error.stack || error.message}\n`)
  process.exit(1)
})
