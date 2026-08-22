#!/usr/bin/env node

import { createHash } from 'node:crypto'
import {
  closeSync,
  constants,
  existsSync,
  fstatSync,
  lstatSync,
  mkdtempSync,
  openSync,
  readFileSync,
  readSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, dirname, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'

import { superviseProcess } from './lib/process-supervisor.mjs'
import {
  MAX_TIMEOUT_MS,
  MIN_TIMEOUT_MS,
  RECOMMENDED_TIMEOUT_MS,
  validateTransportTimeout,
} from './lib/deadline-policy.mjs'

const MCP_PROTOCOL_VERSION = '2025-06-18'
const SERVER_NAME = 'message-decision-packs-runner'
const MAX_JSON_RPC_LINE_BYTES = 1_000_000
const MAX_REQUEST_FILE_BYTES = 1_048_576
const MAX_CHILD_BUFFER_BYTES = 1_000_000
const DEFAULT_TIMEOUT_MS = RECOMMENDED_TIMEOUT_MS
const JSON_RPC_PARSE_ERROR = -32700
const JSON_RPC_INVALID_REQUEST = -32600
const JSON_RPC_METHOD_NOT_FOUND = -32601
const JSON_RPC_INVALID_PARAMS = -32602
const CHILD_ENV_KEYS = [
  'PATH',
  'TMPDIR',
  'TMP',
  'TEMP',
  'LANG',
  'LC_ALL',
  'LC_CTYPE',
  'SSL_CERT_FILE',
  'SSL_CERT_DIR',
  'NODE_EXTRA_CA_CERTS',
]
const NATIVE_MODEL_ENV_KEYS = ['OPENAI_API_KEY', 'MDP_ALLOW_NATIVE_MODEL_CALLS']

const scriptDir = dirname(fileURLToPath(import.meta.url))
const bundleRoot = resolve(scriptDir, '..')

const readVersion = () => {
  for (const path of [
    join(bundleRoot, 'plugin.json'),
    join(bundleRoot, '.codex-plugin', 'plugin.json'),
    join(bundleRoot, 'plugin', '.codex-plugin', 'plugin.json'),
  ]) {
    if (!existsSync(path)) continue
    try {
      const value = JSON.parse(readFileSync(path, 'utf8'))
      if (typeof value.version === 'string' && value.version.trim()) return value.version
    } catch {
      // A malformed optional version file must not change run authority.
    }
  }
  return '0.0.0-local'
}

const serverVersion = readVersion()
const writeMessage = (message) => process.stdout.write(`${JSON.stringify(message)}\n`)
const response = (id, result) => ({ jsonrpc: '2.0', id, result })
const errorResponse = (id, code, message, data) => ({
  jsonrpc: '2.0',
  id: id ?? null,
  error: data === undefined ? { code, message } : { code, message, data },
})
const toolResult = (structuredContent, isError = false) => ({
  content: [{ type: 'text', text: JSON.stringify(structuredContent, null, 2) }],
  structuredContent,
  isError,
})

const asObject = (value, label) => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`)
  }
  return value
}

const assertOnly = (value, allowed) => {
  const unsupported = Object.keys(value).filter((key) => !allowed.has(key))
  if (unsupported.length > 0) {
    throw new Error(`unsupported argument(s): ${unsupported.sort().join(', ')}; pass file paths only`)
  }
}

const requiredString = (value, key) => {
  if (typeof value[key] !== 'string' || value[key].trim() === '') {
    throw new Error(`${key} must be a non-empty string`)
  }
  if (value[key].includes('\0')) throw new Error(`${key} must not contain NUL bytes`)
  return value[key]
}

const canonicalExistingFile = (value, label) => {
  const requested = resolve(value)
  if (!existsSync(requested)) throw new Error(`${label} does not exist`)
  if (lstatSync(requested).isSymbolicLink()) throw new Error(`${label} must not be a symlink`)
  const canonical = realpathSync(requested)
  const stats = statSync(canonical)
  if (!stats.isFile()) throw new Error(`${label} must be a regular file`)
  if (stats.size > MAX_REQUEST_FILE_BYTES) {
    throw new Error(`${label} exceeds ${MAX_REQUEST_FILE_BYTES} bytes`)
  }
  return canonical
}

const sameFile = (left, right) => left.dev === right.dev && left.ino === right.ino
const singleLink = (stats) => stats.nlink === 1n
const stableOpenedFile = (before, after) =>
  sameFile(before, after) &&
  before.size === after.size &&
  before.mtimeNs === after.mtimeNs &&
  before.ctimeNs === after.ctimeNs

const freezeRequestFile = (value) => {
  const requested = resolve(value)
  let descriptor
  let privateDir
  try {
    const before = lstatSync(requested, { bigint: true })
    if (before.isSymbolicLink()) throw new Error('request_path must not be a symlink')
    if (!before.isFile()) throw new Error('request_path must be a regular file')
    if (!singleLink(before)) throw new Error('request_path must have exactly one hard link')
    if (before.size > BigInt(MAX_REQUEST_FILE_BYTES)) {
      throw new Error(`request_path exceeds ${MAX_REQUEST_FILE_BYTES} bytes`)
    }

    descriptor = openSync(requested, constants.O_RDONLY | (constants.O_NOFOLLOW || 0))
    const opened = fstatSync(descriptor, { bigint: true })
    if (!opened.isFile() || !singleLink(opened) || !sameFile(before, opened)) {
      throw new Error('request_path changed while it was being opened')
    }
    if (opened.size > BigInt(MAX_REQUEST_FILE_BYTES)) {
      throw new Error(`request_path exceeds ${MAX_REQUEST_FILE_BYTES} bytes`)
    }

    const bytes = Buffer.alloc(Number(opened.size))
    let offset = 0
    while (offset < bytes.length) {
      const count = readSync(descriptor, bytes, offset, bytes.length - offset, offset)
      if (count === 0) break
      offset += count
    }
    const extra = Buffer.alloc(1)
    const hasExtraByte = readSync(descriptor, extra, 0, 1, offset) !== 0
    const after = fstatSync(descriptor, { bigint: true })
    if (offset !== bytes.length || hasExtraByte || !singleLink(after) || !stableOpenedFile(opened, after)) {
      throw new Error('request_path changed while it was being read')
    }

    let parsed = null
    try {
      parsed = JSON.parse(bytes.toString('utf8'))
    } catch {
      // The CLI remains the authority for rejecting malformed run requests.
    }
    privateDir = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-request-'))
    const frozenPath = join(privateDir, 'request.json')
    writeFileSync(frozenPath, bytes, { flag: 'wx', mode: 0o400 })
    return {
      path: frozenPath,
      privateDir,
      sha256: createHash('sha256').update(bytes).digest('hex'),
      executionId: typeof parsed?.execution_id === 'string' ? parsed.execution_id : null,
      packDir: typeof parsed?.pack_dir === 'string' ? parsed.pack_dir : null,
      usesNativeModel: parsed?.contract === 'mdp.run-request.v1' && parsed?.mode === 'generative',
    }
  } catch (error) {
    if (privateDir) rmSync(privateDir, { recursive: true, force: true })
    if (error?.code === 'ENOENT') throw new Error('request_path does not exist')
    if (error?.code === 'ELOOP') throw new Error('request_path must not be a symlink')
    throw error
  } finally {
    if (descriptor !== undefined) closeSync(descriptor)
  }
}

const canonicalExistingDir = (value, label) => {
  const requested = resolve(value)
  if (!existsSync(requested)) throw new Error(`${label} does not exist`)
  if (lstatSync(requested).isSymbolicLink()) throw new Error(`${label} must not be a symlink`)
  const canonical = realpathSync(requested)
  if (!statSync(canonical).isDirectory()) throw new Error(`${label} must be a directory`)
  return canonical
}

const canonicalNewOutputDir = (value) => {
  const requested = resolve(value)
  if (existsSync(requested)) throw new Error('output_dir must not already exist')
  const leaf = basename(requested)
  if (!leaf || leaf === '.' || leaf === '..') throw new Error('output_dir must name a new directory')
  const requestedParent = dirname(requested)
  if (!existsSync(requestedParent)) throw new Error('output_dir parent does not exist')
  const parent = realpathSync(requestedParent)
  if (!statSync(parent).isDirectory()) throw new Error('output_dir parent must be a directory')
  return join(parent, leaf)
}

const assertOutputOutsidePack = (packDir, outputDir) => {
  if (typeof packDir !== 'string' || packDir.trim() === '') return
  const requestedPack = resolve(packDir)
  if (!existsSync(requestedPack)) return
  const pack = realpathSync(requestedPack)
  if (!statSync(pack).isDirectory()) return
  const output = resolve(outputDir)
  const pathFromPack = relative(pack, output)
  const insidePack = pathFromPack === '' ||
    (!pathFromPack.startsWith(`..${sep}`) && pathFromPack !== '..' && !pathFromPack.startsWith(sep))
  if (insidePack) throw new Error('output_dir must be outside the active pack')
}

const childEnvironment = (includeNativeModel = false) =>
  Object.fromEntries(
    [...CHILD_ENV_KEYS, ...(includeNativeModel ? NATIVE_MODEL_ENV_KEYS : [])]
      .filter((key) => typeof process.env[key] === 'string')
      .map((key) => [key, process.env[key]]),
  )

const invokeCli = (args, cwd, timeoutMs, recovery = null, includeNativeModel = false) =>
  superviseProcess({
    command: [process.env.MDP_BIN || 'mdp'],
    args,
    cwd,
    environment: childEnvironment(includeNativeModel),
    timeoutMs,
    maxOutputBytes: MAX_CHILD_BUFFER_BYTES,
    recovery,
  })

const tools = [
  {
    name: 'mdp_run_tools',
    title: 'Inspect the MDP clean-run boundary',
    description:
      'Describe the local file-oriented clean-run adapter. MCP is transport only; the mdp CLI remains the sole authority for execution, hashes, assurance, validation, and terminal state.',
    inputSchema: { type: 'object', additionalProperties: false, properties: {} },
  },
  {
    name: 'mdp_run',
    title: 'Run an explicit MDP request',
    description:
      'Spawn mdp run with one explicit run-request file and a new output directory. Raw chat, source bodies, inline requests, and assurance overrides are not accepted.',
    inputSchema: {
      type: 'object',
      additionalProperties: false,
      required: ['request_path', 'output_dir'],
      properties: {
        request_path: {
          type: 'string',
          description: 'Existing regular, single-link, non-symlink mdp.run-request.v1 JSON file.',
        },
        output_dir: {
          type: 'string',
          description: 'New run directory whose existing non-symlink parent is controlled by the operator.',
        },
        timeout_ms: {
          type: 'integer',
          minimum: MIN_TIMEOUT_MS,
          maximum: MAX_TIMEOUT_MS,
          description: `Transport guard in milliseconds. The canonical Rust recommendation is ${DEFAULT_TIMEOUT_MS}ms.`,
        },
      },
    },
  },
  {
    name: 'mdp_verify_run',
    title: 'Verify an MDP run receipt',
    description:
      'Run the read-only mdp verify-run command for explicit bundle and receipt files. Returns the canonical CLI verification result without adding MCP assurance.',
    inputSchema: {
      type: 'object',
      additionalProperties: false,
      required: ['bundle_path', 'receipt_path'],
      properties: {
        bundle_path: { type: 'string', description: 'Existing regular, non-symlink mdp.run-bundle.v1 file.' },
        receipt_path: { type: 'string', description: 'Existing regular, non-symlink mdp.run-receipt.v1 file.' },
        artifact_root: { type: 'string', description: 'Optional existing, non-symlink artifact directory.' },
        timeout_ms: { type: 'integer', minimum: MIN_TIMEOUT_MS, maximum: MAX_TIMEOUT_MS },
      },
    },
  },
]

const callRunTools = (args) => {
  const parsed = asObject(args || {}, 'arguments')
  assertOnly(parsed, new Set())
  return toolResult({
    contract: 'mdp.run-mcp-tools.v1',
    transport: 'local-stdio',
    tools: ['mdp_run_tools', 'mdp_run', 'mdp_verify_run'],
    cli_authority: ['run request parsing', 'pack and input staging', 'execution', 'terminal state', 'assurance', 'artifact hashes', 'validation', 'receipt'],
    mcp_authority: [],
    guardrails: [
      'Only explicit local request_path and output_dir arguments cross this MCP boundary.',
      'The adapter freezes one bounded read of request_path in a private read-only copy before classifying or spawning the CLI.',
      'The adapter starts a separate CLI process with bounded time, output, stdin, and environment.',
      'Only a parsed mdp.run-request.v1 with mode=generative may inherit OPENAI_API_KEY and MDP_ALLOW_NATIVE_MODEL_CALLS from the server startup environment; tool arguments cannot supply or enable either.',
      'MCP transport does not prove fresh context, isolation, freshness, replay safety, or audit grade.',
      'The returned run result and authority block are copied from mdp --json run without modification.',
    ],
  })
}

const callRun = async (args) => {
  const parsed = asObject(args || {}, 'arguments')
  assertOnly(parsed, new Set(['request_path', 'output_dir', 'timeout_ms']))
  const requestPath = requiredString(parsed, 'request_path')
  const outputDir = canonicalNewOutputDir(requiredString(parsed, 'output_dir'))
  const timeoutMs = parsed.timeout_ms ?? DEFAULT_TIMEOUT_MS
  validateTransportTimeout(timeoutMs)
  const frozenRequest = freezeRequestFile(requestPath)

  let invocation
  try {
    assertOutputOutsidePack(frozenRequest.packDir, outputDir)
    invocation = await invokeCli(
      ['--json', 'run', '--request', frozenRequest.path, '--out-dir', outputDir, '--transport-timeout-ms', String(timeoutMs)],
      dirname(outputDir),
      timeoutMs,
      {
        outputDir,
        executionId: frozenRequest.executionId,
        requestSha256: frozenRequest.sha256,
      },
      frozenRequest.usesNativeModel,
    )
  } finally {
    rmSync(frozenRequest.privateDir, { recursive: true, force: true })
  }
  if (invocation.timedOut || invocation.overflowed || invocation.spawnFailed) {
    const code = invocation.timedOut
      ? 'cli-timeout'
      : invocation.overflowed
        ? 'cli-output-limit'
        : 'cli-unavailable'
    return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code, deadline: invocation.deadline }, true)
  }

  let envelope
  try {
    envelope = JSON.parse(invocation.stdout)
  } catch {
    return toolResult(
      {
        ok: false,
        contract: 'mdp.run-mcp-error.v1',
        code: invocation.status === 0 ? 'invalid-cli-output' : 'cli-run-failed',
        deadline: invocation.deadline,
      },
      true,
    )
  }
  if (
    envelope?.ok !== true ||
    envelope?.command !== 'run' ||
    !envelope.data ||
    typeof envelope.data !== 'object' ||
    Array.isArray(envelope.data) ||
    envelope.data.contract !== 'mdp.run-execution.v1' ||
    !envelope.data.authority_block ||
    typeof envelope.data.authority_block !== 'object' ||
    Array.isArray(envelope.data.authority_block) ||
    envelope.data.authority_block.terminal_state !== envelope.data.terminal_state
  ) {
    return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: 'invalid-cli-contract', deadline: invocation.deadline }, true)
  }

  const success =
    invocation.status === 0 &&
    envelope.data.valid === true &&
    envelope.data.terminal_state === 'success' &&
    envelope.data.authority?.authority_level === 'authoritative' &&
    envelope.data.authority?.disposition === 'allow' &&
    envelope.data.authority?.terminal === 'success' &&
    ['available', 'not-applicable'].includes(envelope.data.authority?.governed_generation)
  const completedNoDraft =
    invocation.status === 0 &&
    envelope.data.valid === false &&
    envelope.data.terminal_state === 'success' &&
    envelope.data.authority?.authority_level === 'authoritative' &&
    envelope.data.authority?.disposition === 'block' &&
    envelope.data.authority?.terminal === 'no-draft' &&
    envelope.data.authority?.governed_generation === 'absent' &&
    envelope.data.authority_block?.decision?.decision === 'no-draft'
  const failedNoDraft =
    invocation.status !== 0 &&
    envelope.data.valid === false &&
    typeof envelope.data.terminal_state === 'string' &&
    envelope.data.terminal_state.startsWith('no-draft:') &&
    ((envelope.data.authority?.authority_level === 'authoritative' &&
      envelope.data.authority?.disposition === 'block' &&
      envelope.data.authority?.terminal === 'no-draft' &&
      envelope.data.authority?.governed_generation === 'absent') ||
      (envelope.data.authority?.authority_level === 'unavailable' &&
        envelope.data.authority?.disposition === 'undetermined' &&
        envelope.data.authority?.terminal === 'authority-unavailable' &&
        envelope.data.authority?.governed_generation === 'absent'))
  if (!success && !completedNoDraft && !failedNoDraft) {
    return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: 'invalid-cli-contract', deadline: invocation.deadline }, true)
  }

  // A canonical no-draft result is decision data, not an MCP transport error.
  // Do not wrap, reinterpret, or promote assurance: the exact CLI data object
  // is the MCP tool result.
  return toolResult(envelope.data)
}

const callVerifyRun = async (args) => {
  const parsed = asObject(args || {}, 'arguments')
  assertOnly(parsed, new Set(['bundle_path', 'receipt_path', 'artifact_root', 'timeout_ms']))
  const bundlePath = canonicalExistingFile(requiredString(parsed, 'bundle_path'), 'bundle_path')
  const receiptPath = canonicalExistingFile(requiredString(parsed, 'receipt_path'), 'receipt_path')
  const artifactRoot = parsed.artifact_root === undefined
    ? null
    : canonicalExistingDir(requiredString(parsed, 'artifact_root'), 'artifact_root')
  const timeoutMs = parsed.timeout_ms ?? DEFAULT_TIMEOUT_MS
  validateTransportTimeout(timeoutMs)
  const cliArgs = ['--json', 'verify-run', '--bundle', bundlePath, '--receipt', receiptPath]
  if (artifactRoot) cliArgs.push('--artifact-root', artifactRoot)
  const invocation = await invokeCli(cliArgs, dirname(bundlePath), timeoutMs)
  if (invocation.timedOut || invocation.overflowed || invocation.spawnFailed) {
    const code = invocation.timedOut ? 'cli-timeout' : invocation.overflowed ? 'cli-output-limit' : 'cli-unavailable'
    return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code, deadline: invocation.deadline }, true)
  }
  let envelope
  try {
    envelope = JSON.parse(invocation.stdout)
  } catch {
    return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: 'invalid-cli-output', deadline: invocation.deadline }, true)
  }
  if (
    envelope?.ok !== true ||
    envelope?.command !== 'verify-run' ||
    !envelope.data ||
    typeof envelope.data !== 'object' ||
    Array.isArray(envelope.data) ||
    envelope.data.contract !== 'mdp.run-verification.v1' ||
    typeof envelope.data.valid !== 'boolean' ||
    (invocation.status === 0) !== envelope.data.valid
  ) {
    return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: 'invalid-cli-contract', deadline: invocation.deadline }, true)
  }
  // An invalid verification is a canonical integrity result, not an MCP
  // transport failure. Preserve it exactly so the caller can fail closed on
  // `valid: false` without losing the CLI's issue list.
  return toolResult(envelope.data)
}

const handleToolCall = async (params) => {
  const call = asObject(params || {}, 'params')
  if (typeof call.name !== 'string' || call.name.trim() === '') {
    throw new Error('params.name must be a non-empty string')
  }
  switch (call.name) {
    case 'mdp_run_tools':
      return callRunTools(call.arguments || {})
    case 'mdp_run':
      return await callRun(call.arguments || {})
    case 'mdp_verify_run':
      return await callVerifyRun(call.arguments || {})
    default:
      throw Object.assign(new Error(`unknown tool: ${call.name}`), { code: JSON_RPC_METHOD_NOT_FOUND })
  }
}

const handleRequest = async (message) => {
  if (!message || typeof message !== 'object' || Array.isArray(message)) {
    return errorResponse(null, JSON_RPC_INVALID_REQUEST, 'invalid JSON-RPC message')
  }
  if (message.jsonrpc !== '2.0') {
    return errorResponse(message.id, JSON_RPC_INVALID_REQUEST, 'jsonrpc must be 2.0')
  }
  const notification = !('id' in message)
  if (typeof message.method !== 'string' || !message.method.trim()) {
    return notification ? null : errorResponse(message.id, JSON_RPC_INVALID_REQUEST, 'method must be a non-empty string')
  }

  try {
    switch (message.method) {
      case 'initialize':
        return notification
          ? null
          : response(message.id, {
              protocolVersion: message.params?.protocolVersion || MCP_PROTOCOL_VERSION,
              capabilities: { tools: { listChanged: false } },
              serverInfo: { name: SERVER_NAME, version: serverVersion },
              instructions:
                'Use mdp_run with an already-written run-request file and a new output directory. Use mdp_verify_run to independently check the resulting bundle and receipt. The surrounding agent is control plane only; only the CLI result and receipt have decision authority.',
            })
      case 'notifications/initialized':
        return null
      case 'ping':
        return notification ? null : response(message.id, {})
      case 'tools/list':
        return notification ? null : response(message.id, { tools })
      case 'tools/call':
        return notification ? null : response(message.id, await handleToolCall(message.params))
      default:
        return notification
          ? null
          : errorResponse(message.id, JSON_RPC_METHOD_NOT_FOUND, `method not found: ${message.method}`)
    }
  } catch (error) {
    return notification
      ? null
      : errorResponse(
          message.id,
          Number.isInteger(error.code) ? error.code : JSON_RPC_INVALID_PARAMS,
          error.message || 'invalid parameters',
        )
  }
}

const handleLine = async (line) => {
  if (Buffer.byteLength(line, 'utf8') > MAX_JSON_RPC_LINE_BYTES) {
    writeMessage(errorResponse(null, JSON_RPC_INVALID_REQUEST, `JSON-RPC message exceeds ${MAX_JSON_RPC_LINE_BYTES} bytes`))
    return
  }
  if (!line.trim()) return
  let message
  try {
    message = JSON.parse(line)
  } catch {
    writeMessage(errorResponse(null, JSON_RPC_PARSE_ERROR, 'parse error'))
    return
  }
  if (Array.isArray(message)) {
    const replies = []
    for (const item of message) {
      const reply = await handleRequest(item)
      if (reply) replies.push(reply)
    }
    if (replies.length) writeMessage(replies)
    return
  }
  const reply = await handleRequest(message)
  if (reply) writeMessage(reply)
}

let buffer = ''
let discardingOversizedLine = false
let queue = Promise.resolve()
const enqueue = (line) => {
  queue = queue.then(() => handleLine(line))
}
process.stdin.setEncoding('utf8')
process.stdin.resume()
process.stdin.on('data', (chunk) => {
  if (discardingOversizedLine) {
    const newline = chunk.indexOf('\n')
    if (newline < 0) return
    discardingOversizedLine = false
    chunk = chunk.slice(newline + 1)
  }
  buffer += chunk
  let newline
  while ((newline = buffer.indexOf('\n')) >= 0) {
    enqueue(buffer.slice(0, newline))
    buffer = buffer.slice(newline + 1)
  }
  if (Buffer.byteLength(buffer, 'utf8') > MAX_JSON_RPC_LINE_BYTES) {
    writeMessage(errorResponse(null, JSON_RPC_INVALID_REQUEST, `JSON-RPC message exceeds ${MAX_JSON_RPC_LINE_BYTES} bytes`))
    buffer = ''
    discardingOversizedLine = true
  }
})
process.stdin.on('end', () => {
  if (buffer.trim() && !discardingOversizedLine) enqueue(buffer)
})

process.on('uncaughtException', () => {
  process.stderr.write('mdp run MCP server fatal error\n')
  process.exit(1)
})
