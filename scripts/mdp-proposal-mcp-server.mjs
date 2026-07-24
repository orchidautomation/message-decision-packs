#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { existsSync, lstatSync, readFileSync, realpathSync, statSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const MCP_PROTOCOL_VERSION = '2025-06-18'
const SERVER_NAME = 'message-decision-packs-proposal'
const MAX_OUTPUT_CHARS = 80_000
const MAX_CHILD_BUFFER_BYTES = 1_000_000
const DEFAULT_TIMEOUT_MS = 120_000
const MAX_TIMEOUT_MS = 300_000
const CHILD_ENV_KEYS = [
  'PATH',
  'HOME',
  'USER',
  'LOGNAME',
  'SHELL',
  'TMPDIR',
  'TMP',
  'TEMP',
  'LANG',
  'LC_ALL',
  'LC_CTYPE',
  'CARGO_HOME',
  'CARGO_TARGET_DIR',
  'RUSTUP_HOME',
  'SSL_CERT_FILE',
  'SSL_CERT_DIR',
  'NODE_EXTRA_CA_CERTS',
  'OPENAI_API_KEY',
]
const SECRET_ENV_KEYS = /(?:KEY|TOKEN|SECRET|PASSWORD|CREDENTIAL|AUTH)/i
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

const childEnvironment = () =>
  Object.fromEntries(
    CHILD_ENV_KEYS.filter((key) => typeof process.env[key] === 'string').map((key) => [key, process.env[key]]),
  )

const redact = (value, environment = childEnvironment()) => {
  let text = String(value || '')
  for (const [key, secret] of Object.entries(environment)) {
    if (!SECRET_ENV_KEYS.test(key) || !secret) continue
    text = text.split(secret).join(`[REDACTED:${key}]`)
  }
  return text
    .replace(/\bsk-[A-Za-z0-9_-]{8,}\b/g, '[REDACTED:API_KEY]')
    .replace(/\b(Bearer)\s+[A-Za-z0-9._~+/-]+=*/gi, '$1 [REDACTED]')
    .replace(/((?:KEY|TOKEN|SECRET|PASSWORD|CREDENTIAL|AUTH)[A-Z0-9_]*\s*[=:]\s*)[^\s,;]+/gi, '$1[REDACTED]')
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

const assertNoNul = (value, label) => {
  if (value.includes('\0')) throw new Error(`${label} must not contain NUL bytes`)
}

const canonicalExistingPath = (value, label, kind = 'file') => {
  assertNoNul(value, label)
  const requested = resolve(value)
  if (!existsSync(requested)) throw new Error(`${label} not found: ${requested}`)
  if (lstatSync(requested).isSymbolicLink()) throw new Error(`${label} must not be a symlink: ${requested}`)
  const canonical = realpathSync(requested)
  const stats = statSync(canonical)
  if (kind === 'file' && !stats.isFile()) throw new Error(`${label} must be a regular file: ${requested}`)
  if (kind === 'directory' && !stats.isDirectory()) throw new Error(`${label} must be a directory: ${requested}`)
  return canonical
}

const canonicalPack = (value) => {
  const pack = canonicalExistingPath(value, 'pack', 'directory')
  const mdp = join(pack, '.mdp')
  if (!existsSync(mdp) || lstatSync(mdp).isSymbolicLink() || !statSync(mdp).isDirectory()) {
    throw new Error(`pack must contain a real .mdp directory: ${pack}`)
  }
  return pack
}

const canonicalWorkdir = (value) => {
  assertNoNul(value, 'workdir')
  const requested = resolve(value)
  if (existsSync(requested)) return canonicalExistingPath(requested, 'workdir', 'directory')
  const parent = dirname(requested)
  canonicalExistingPath(parent, 'workdir parent', 'directory')
  return requested
}

const canonicalExecutable = (value, label) => {
  const path = canonicalExistingPath(value, label, 'file')
  if ((statSync(path).mode & 0o111) === 0) throw new Error(`${label} must be executable: ${path}`)
  return path
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

const runNode = (script, args, timeoutMs = DEFAULT_TIMEOUT_MS) => {
  const environment = childEnvironment()
  const result = spawnSync('node', [script, ...args], {
    cwd: bundleRoot,
    encoding: 'utf8',
    env: environment,
    maxBuffer: MAX_CHILD_BUFFER_BYTES,
    timeout: timeoutMs,
    killSignal: 'SIGTERM',
  })
  const timedOut = result.error?.code === 'ETIMEDOUT'
  const stdout = redact(result.stdout || '', environment)
  const stderr = redact(`${result.stderr || ''}${result.error && !timedOut ? result.error.message : ''}`, environment)
  if (result.error) {
    return {
      status: timedOut ? 124 : 1,
      stdout,
      stderr: timedOut ? `proposal runner timed out after ${timeoutMs}ms` : stderr,
      timedOut,
      signal: result.signal || (timedOut ? 'SIGTERM' : null),
      environmentKeys: Object.keys(environment).sort(),
    }
  }
  return {
    status: result.status ?? 1,
    stdout,
    stderr,
    timedOut: false,
    signal: result.signal || null,
    environmentKeys: Object.keys(environment).sort(),
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
      description: 'Customer-controlled local scratch directory for runner artifacts. Reuse requires the exact ownership/run manifests.',
    },
    source_paths: {
      type: 'array',
      items: { type: 'string' },
      description: 'Local text/Markdown/CSV/JSON/YAML source files supplied by the operator. Raw chat text is intentionally not accepted.',
    },
    source_intake_path: {
      type: 'string',
      description: 'Existing operator-approved mdp.source-intake.v0 ledger. Required for real native runs.',
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
    privacy_class: {
      type: 'string',
      enum: ['synthetic-public', 'sanitized-public', 'private-customer', 'restricted-local'],
      description: 'Source-intake privacy class. Defaults conservatively from source_kind.',
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
    reuse_workdir_id: {
      type: 'string',
      description: 'Reuse a non-empty workdir only when its local ownership manifest has this exact id.',
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
    timeout_ms: {
      type: 'integer',
      minimum: 100,
      maximum: MAX_TIMEOUT_MS,
      description: `Child-process deadline in milliseconds. Defaults to ${DEFAULT_TIMEOUT_MS}.`,
    },
  },
}

const proposalRunOutputSchema = {
  type: 'object',
  additionalProperties: false,
  required: [
    'ok',
    'contract',
    'mcp_transport',
    'hosted_or_remote_mcp',
    'runner_exit_status',
    'runner_result',
    'mode',
    'decision',
    'audit_grade_eligible',
    'runner_assurance',
    'timed_out',
    'termination_signal',
    'timeout_ms',
    'stdout',
    'stderr',
    'environment',
    'guardrails',
  ],
  properties: {
    ok: { type: 'boolean' },
    contract: { const: 'mdp.proposal-mcp-run-result.v0' },
    mcp_transport: { const: 'stdio' },
    hosted_or_remote_mcp: { const: false },
    runner_exit_status: { type: 'integer' },
    runner_result: { type: ['object', 'null'] },
    mode: { type: ['string', 'null'] },
    decision: { enum: ['not-run', 'audit-grade', 'advisory', 'blocked'] },
    audit_grade_eligible: { type: 'boolean' },
    runner_assurance: { type: 'string' },
    timed_out: { type: 'boolean' },
    termination_signal: { type: ['string', 'null'] },
    timeout_ms: { type: 'integer', minimum: 100, maximum: MAX_TIMEOUT_MS },
    stdout: { type: 'string' },
    stderr: { type: 'string' },
    environment: {
      type: 'object',
      additionalProperties: false,
      required: ['policy', 'keys', 'secret_values_reported'],
      properties: {
        policy: { const: 'allowlist' },
        keys: { type: 'array', items: { type: 'string' }, uniqueItems: true },
        secret_values_reported: { const: false },
      },
    },
    guardrails: { type: 'array', items: { type: 'string' }, minItems: 1 },
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
    outputSchema: proposalRunOutputSchema,
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
    'source_intake_path',
    'source_audit_path',
    'source_id',
    'source_kind',
    'privacy_class',
    'model',
    'mock_response_path',
    'dry_run',
    'mdp_bin',
    'native_runner',
    'prompt_id',
    'reuse_workdir_id',
    'skip_review',
    'require_audit_grade',
    'max_source_bytes',
    'timeout_ms',
  ])
  assertNoUnsupportedArgs(parsedArgs, allowed)

  const packArg = optionalString(parsedArgs, 'pack')
  const workdirArg = optionalString(parsedArgs, 'workdir')
  if (!packArg) throw new Error('pack is required')
  if (!workdirArg) throw new Error('workdir is required')
  const pack = canonicalPack(packArg)
  const workdir = canonicalWorkdir(workdirArg)

  const sourcePaths = optionalStringArray(parsedArgs, 'source_paths').map((path, index) =>
    canonicalExistingPath(path, `source_paths[${index}]`, 'file'),
  )
  const sourceIntakeArg = optionalString(parsedArgs, 'source_intake_path')
  const sourceAuditArg = optionalString(parsedArgs, 'source_audit_path')
  const sourceIntakePath = sourceIntakeArg
    ? canonicalExistingPath(sourceIntakeArg, 'source_intake_path', 'file')
    : null
  const sourceAuditPath = sourceAuditArg
    ? canonicalExistingPath(sourceAuditArg, 'source_audit_path', 'file')
    : null
  const sourceId = optionalString(parsedArgs, 'source_id')
  const sourceKind = optionalString(parsedArgs, 'source_kind')
  const privacyClass = optionalString(parsedArgs, 'privacy_class')
  const model = optionalString(parsedArgs, 'model')
  const mockResponseArg = optionalString(parsedArgs, 'mock_response_path')
  const mdpBinArg = optionalString(parsedArgs, 'mdp_bin')
  const nativeRunnerArg = optionalString(parsedArgs, 'native_runner')
  const mockResponsePath = mockResponseArg
    ? canonicalExistingPath(mockResponseArg, 'mock_response_path', 'file')
    : null
  const mdpBin = mdpBinArg ? canonicalExecutable(mdpBinArg, 'mdp_bin') : null
  const nativeRunner = nativeRunnerArg
    ? canonicalExistingPath(nativeRunnerArg, 'native_runner', 'file')
    : null
  const promptId = optionalString(parsedArgs, 'prompt_id')
  const maxSourceBytes = optionalInteger(parsedArgs, 'max_source_bytes')
  const dryRun = optionalBoolean(parsedArgs, 'dry_run')
  const reuseWorkdirId = optionalString(parsedArgs, 'reuse_workdir_id')
  const skipReview = optionalBoolean(parsedArgs, 'skip_review')
  const requireAuditGrade = optionalBoolean(parsedArgs, 'require_audit_grade')
  const timeoutMs = optionalInteger(parsedArgs, 'timeout_ms') ?? DEFAULT_TIMEOUT_MS
  if (timeoutMs < 100 || timeoutMs > MAX_TIMEOUT_MS) {
    throw new Error(`timeout_ms must be between 100 and ${MAX_TIMEOUT_MS}`)
  }

  if (sourcePaths.length === 0) {
    throw new Error('Pass at least one source_paths file. Ambient chat/source text and audit-only runs are intentionally not accepted.')
  }

  const runnerArgs = ['run', '--pack', pack, '--workdir', workdir]
  for (const sourcePath of sourcePaths) runnerArgs.push('--source', sourcePath)
  if (sourceIntakePath) runnerArgs.push('--source-intake', sourceIntakePath)
  if (sourceAuditPath) runnerArgs.push('--source-audit', sourceAuditPath)
  if (sourceId) runnerArgs.push('--source-id', sourceId)
  if (sourceKind) runnerArgs.push('--source-kind', sourceKind)
  if (privacyClass) runnerArgs.push('--privacy-class', privacyClass)
  if (model) runnerArgs.push('--model', model)
  if (mockResponsePath) runnerArgs.push('--mock-response', mockResponsePath)
  if (dryRun) runnerArgs.push('--dry-run')
  if (mdpBin) runnerArgs.push('--mdp-bin', mdpBin)
  if (nativeRunner) runnerArgs.push('--native-runner', nativeRunner)
  if (promptId) runnerArgs.push('--prompt-id', promptId)
  if (reuseWorkdirId) runnerArgs.push('--reuse-workdir-id', reuseWorkdirId)
  if (skipReview) runnerArgs.push('--skip-review')
  if (requireAuditGrade) runnerArgs.push('--require-audit-grade')
  if (maxSourceBytes !== null) runnerArgs.push('--max-source-bytes', String(maxSourceBytes))

  const result = runNode(runnerPath, runnerArgs, timeoutMs)
  let parsed = null
  try {
    parsed = parseRunnerJson(result.stdout)
  } catch (error) {
    if (result.status === 0) throw error
  }

  const auditGradeRejected =
    requireAuditGrade &&
    (!parsed || parsed.decision !== 'audit-grade' || parsed.audit_grade_eligible !== true)
  const ok = result.status === 0 && parsed && parsed.ok !== false && !auditGradeRejected
  const structuredContent = {
    ok,
    contract: 'mdp.proposal-mcp-run-result.v0',
    mcp_transport: 'stdio',
    hosted_or_remote_mcp: false,
    runner_exit_status: auditGradeRejected && result.status === 0 ? 2 : result.status,
    runner_result: parsed,
    mode: parsed?.mode ?? null,
    decision: parsed?.decision ?? 'blocked',
    audit_grade_eligible: parsed?.audit_grade_eligible === true,
    runner_assurance: parsed?.runner_assurance ?? 'unknown',
    timed_out: result.timedOut,
    termination_signal: result.signal,
    timeout_ms: timeoutMs,
    stdout: parsed ? '' : compact(result.stdout, 12_000),
    stderr: result.stderr ? compact(result.stderr, 12_000) : '',
    environment: {
      policy: 'allowlist',
      keys: result.environmentKeys,
      secret_values_reported: false,
    },
    guardrails: [
      'This MCP tool passed only explicit local file/path arguments to the proposal runner.',
      'All local paths were canonicalized and checked for expected file/directory type and final-component symlinks.',
      'The child received an explicit environment allowlist; secret values are never returned in MCP diagnostics.',
      `The runner was bounded by a ${timeoutMs}ms deadline and bounded stdout/stderr buffers.`,
      'The model isolation claim comes from the runner-audit plus mdp run-receipt, not from MCP transport alone.',
      'Dry-run/mock/demo/fixture/synthetic evidence remains non-audit-grade.',
    ],
  }

  if (result.status !== 0 || auditGradeRejected) {
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
    return errorResponse(id, code, redact(error.message || 'Tool call failed'))
  }
}

const handleLine = (line) => {
  const trimmed = line.trim()
  if (!trimmed) return
  let message
  try {
    message = JSON.parse(trimmed)
  } catch (error) {
    writeMessage(errorResponse(null, JSON_RPC_PARSE_ERROR, redact(`Parse error: ${error.message}`)))
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
  process.stderr.write(`mdp proposal MCP server fatal error: ${redact(error.stack || error.message)}\n`)
  process.exit(1)
})
