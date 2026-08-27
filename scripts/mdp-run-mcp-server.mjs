#!/usr/bin/env node

import { createHash } from 'node:crypto'
import {
  closeSync,
  constants,
  copyFileSync,
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
import { createPathPolicy } from './lib/mcp-path-policy.mjs'
import { consumeProviderConsent } from './lib/mcp-provider-consent.mjs'
import { resolveIdentityBoundDirectory } from './lib/identity-bound-directory.mjs'

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
const providerCapabilityAvailable = () => process.env.MDP_ALLOW_NATIVE_MODEL_CALLS === '1' && typeof process.env.OPENAI_API_KEY === 'string' && process.env.OPENAI_API_KEY !== ''

const scriptDir = dirname(fileURLToPath(import.meta.url))
const bundleRoot = resolve(scriptDir, '..')
let pathPolicy = null
try { pathPolicy = createPathPolicy(process.env, ['pack', 'input', 'work', 'output', 'consent']) } catch (error) { pathPolicy = { startupError: error } }
const requirePolicy = () => {
  if (pathPolicy?.startupError) throw pathPolicy.startupError
  return pathPolicy
}

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

const validateDeadlinePlan = (plan, timeoutMs, executionId = null) => {
  if (!plan || typeof plan !== 'object' || Array.isArray(plan)) return false
  const required = [
    'contract', 'execution_id', 'mode', 'recommended_timeout_ms',
    'runtime_configured_ms', 'transport_configured_ms', 'provider_configured_ms',
    'finalization_reserve_ms', 'effective_limit_ms', 'warnings', 'staging', 'provider',
  ]
  const allowed = new Set(required)
  if (Object.keys(plan).some((key) => !allowed.has(key)) || required.some((key) => !(key in plan))) return false
  if (plan.contract !== 'mdp.run-preflight.v1' || typeof plan.execution_id !== 'string' || plan.execution_id.trim() === '' ||
      (executionId !== null && plan.execution_id !== executionId) ||
      !['deterministic', 'generative'].includes(plan.mode) || plan.recommended_timeout_ms !== RECOMMENDED_TIMEOUT_MS ||
      plan.transport_configured_ms !== timeoutMs || plan.provider_configured_ms !== RECOMMENDED_TIMEOUT_MS ||
      plan.finalization_reserve_ms !== 250 || plan.staging !== 'not-started' || plan.provider !== 'not-started' ||
      !Number.isSafeInteger(plan.runtime_configured_ms) || plan.runtime_configured_ms < 251 || plan.runtime_configured_ms > 60_000 ||
      !Number.isSafeInteger(plan.effective_limit_ms) || plan.effective_limit_ms < 1 || plan.effective_limit_ms > 60_000 ||
      !Array.isArray(plan.warnings) || plan.warnings.some((warning) => !['outer-timeout-cannot-extend-inner', 'outer-timeout-truncates-runtime'].includes(warning))) return false
  const expected = Math.min(plan.runtime_configured_ms, timeoutMs - 250)
  if (plan.effective_limit_ms !== expected) return false
  const expectedWarnings = timeoutMs > plan.runtime_configured_ms
    ? ['outer-timeout-cannot-extend-inner']
    : timeoutMs - 250 < plan.runtime_configured_ms
      ? ['outer-timeout-truncates-runtime']
      : []
  return JSON.stringify(plan.warnings) === JSON.stringify(expectedWarnings)
}

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
      parsed,
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

const canonicalOutputFile = (value, label) => {
  const requested = resolve(value)
  if (existsSync(requested) && lstatSync(requested).isSymbolicLink()) throw new Error(`${label} must not be a symlink`)
  const parent = dirname(requested)
  if (!existsSync(parent) || lstatSync(parent).isSymbolicLink()) throw new Error(`${label} parent must be an existing non-symlink directory`)
  if (!statSync(realpathSync(parent)).isDirectory()) throw new Error(`${label} parent must be a directory`)
  return requested
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
  const requestedOutput = resolve(outputDir)
  const lexical = relative(requestedPack, requestedOutput)
  if (lexical === '' || (!lexical.startsWith(`..${sep}`) && lexical !== '..' && !lexical.startsWith(sep))) {
    throw new Error('output_dir must be outside the active pack')
  }
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

const invokeCli = (args, cwd, timeoutMs, recovery = null, includeNativeModel = false, deadlineMetadata = null, signal = null) =>
  superviseProcess({
    command: [process.env.MDP_BIN || 'mdp'],
    args,
    cwd,
    environment: childEnvironment(includeNativeModel),
    timeoutMs,
    maxOutputBytes: MAX_CHILD_BUFFER_BYTES,
    recovery,
    deadlineMetadata,
    signal,
  })

const tools = [
  {
    name: 'mdp_run_tools',
    title: 'Inspect the MDP clean-run boundary',
    description:
      'Discover the canonical four-stage local MCP path and its artifacts: inspect, prepare, run, then verify. MCP is transport only; the mdp CLI remains the sole authority.',
    inputSchema: { type: 'object', additionalProperties: false, properties: {} },
  },
  {
    name: 'mdp_prepare_run',
    title: 'Prepare an MDP run request offline',
    description:
      'Compile and persist one sealed mdp.run-request.v1 under an approved work root from a pack, selected job/step, and declared local input paths. No provider is invoked. Next, pass that request path to mdp_run.',
    inputSchema: {
      type: 'object',
      additionalProperties: false,
      required: ['dir', 'job', 'model', 'out'],
      properties: {
        dir: { type: 'string', description: 'Existing pack directory.' },
        job: { type: 'string', description: 'Exact profile job id.' },
        operation: { type: 'string', description: 'Exact model:<job>/<phase> step id when selection is ambiguous.' },
        inputs: { type: 'array', items: { type: 'string' }, description: 'Declared logical_name=path mappings.' },
        model: { type: 'string', description: 'Requested model name.' },
        retention_policy: { type: 'string', enum: ['receipt-only', 'customer-controlled-workdir'] },
        created_at: { type: 'string', description: 'Optional RFC3339 UTC test clock.' },
        out: { type: 'string', description: 'Required new mdp.run-request.v1 path under an approved work root.' },
        manifest_out: { type: 'string', description: 'Optional new full compiler manifest path under an approved work root.' },
        full: { type: 'boolean' },
        timeout_ms: { type: 'integer', minimum: 100, maximum: MAX_TIMEOUT_MS },
      },
    },
  },
  {
    name: 'mdp_run',
    title: 'Run an explicit MDP request',
    description:
      'Pass one mdp.run-request.v1 file to mdp run and produce a run directory containing run-bundle.json, run-receipt.json, and declared artifacts. Next, pass the bundle and receipt to mdp_verify_run. Raw chat, source bodies, inline requests, and assurance overrides are not accepted.',
    inputSchema: {
      type: 'object',
      additionalProperties: false,
      required: ['request_path', 'output_dir'],
      properties: {
        request_path: {
          type: 'string',
          description: 'Existing regular, single-link, non-symlink mdp.run-request.v1 file under an approved work root.',
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
        consent_id: { type: 'string', description: 'Out-of-band one-shot consent record id for generative runs.' },
      },
    },
  },
  {
    name: 'mdp_verify_run',
    title: 'Verify an MDP run receipt',
    description:
      'Read an explicit run-bundle.json and run-receipt.json with mdp verify-run and return mdp.run-verification.v1. This is the terminal read-only stage and adds no MCP assurance.',
    inputSchema: {
      type: 'object',
      additionalProperties: false,
      required: ['bundle_path', 'receipt_path'],
      properties: {
        bundle_path: { type: 'string', description: 'Existing regular, non-symlink mdp.run-bundle.v1 file under an approved output root.' },
        receipt_path: { type: 'string', description: 'Existing regular, non-symlink mdp.run-receipt.v1 file under an approved output root.' },
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
    tools: ['mdp_run_tools', 'mdp_prepare_run', 'mdp_run', 'mdp_verify_run'],
    canonical_path: [
      {
        stage: 'inspect',
        tool: 'mdp_run_tools',
        input: 'no arguments',
        artifact: 'mdp.run-mcp-tools.v1 boundary inventory',
        next: 'mdp_prepare_run',
      },
      {
        stage: 'prepare',
        tool: 'mdp_prepare_run',
        input: 'pack directory, exact job/model step, and declared input paths',
        artifact: 'persisted mdp.run-request.v1 under an approved work root, plus optional compile manifest',
        next: 'mdp_run',
      },
      {
        stage: 'run',
        tool: 'mdp_run',
        input: 'mdp.run-request.v1 path and a new output directory',
        artifact: 'run-bundle.json, run-receipt.json, and declared run artifacts',
        next: 'mdp_verify_run',
      },
      {
        stage: 'verify',
        tool: 'mdp_verify_run',
        input: 'run-bundle.json and run-receipt.json paths',
        artifact: 'mdp.run-verification.v1',
        next: null,
      },
    ],
    cli_authority: ['run request parsing', 'pack and input staging', 'execution', 'terminal state', 'assurance', 'artifact hashes', 'validation', 'receipt'],
    mcp_authority: [],
    guardrails: [
      'Only explicit local paths and the closed prepare/run selectors declared by these tool schemas cross this MCP boundary; ambient chat and inline source bodies are rejected.',
      'The adapter freezes one bounded read of request_path in a private read-only copy before classifying or spawning the CLI.',
      'The adapter starts a separate CLI process with bounded time, output, stdin, and environment.',
      'Only a parsed mdp.run-request.v1 with mode=generative may inherit OPENAI_API_KEY and MDP_ALLOW_NATIVE_MODEL_CALLS from the server startup environment; tool arguments cannot supply or enable either.',
      'MCP transport does not prove fresh context, isolation, freshness, replay safety, or audit grade.',
      'The returned run result and authority block are copied from mdp --json run without modification.',
    ],
  })
}

const blockedPrepareRun = (code, message, nextCommand = 'mdp prepare-run --help') => toolResult({
  contract: 'mdp.run-request-compile.v1',
  status: 'blocked',
  diagnostics: [{
    code,
    contract: 'mdp.run-request-compile.v1',
    message: `${code}: ${message}`.slice(0, 512),
    next_command: nextCommand,
  }],
  next_command: nextCommand,
})

const pinOutputParent = (reservation) => {
  let fd
  try {
    fd = openSync(reservation.parent, constants.O_RDONLY | (constants.O_DIRECTORY || 0) | (constants.O_NOFOLLOW || 0))
    const opened = fstatSync(fd, { bigint: true })
    if (!opened.isDirectory() || opened.dev !== BigInt(reservation.parentIdentity.dev) || opened.ino !== BigInt(reservation.parentIdentity.ino)) {
      throw Object.assign(new Error('work output parent changed while being pinned'), { code: 'mcp-output-denied' })
    }
    if (process.platform === 'darwin') return { ...reservation, fd, securePath: null }
    const pinnedParent = resolveIdentityBoundDirectory({ fd, identity: reservation.parentIdentity })
    return { ...reservation, fd, securePath: join(pinnedParent, basename(reservation.path)) }
  } catch (error) {
    if (fd !== undefined) closeSync(fd)
    throw error
  }
}

const invokeSecureInstaller = (output, action) => {
  const args = [
    '--json', '__secure-install',
    '--action', action,
    '--name', basename(output.path),
    '--dir-fd', '3',
    '--expected-dev', output.parentIdentity.dev.toString(),
    '--expected-ino', output.parentIdentity.ino.toString(),
  ]
  if (action === 'install') args.push('--source', output.cliPath)
  else args.push(
    '--expected-file-dev', output.installedIdentity.dev.toString(),
    '--expected-file-ino', output.installedIdentity.ino.toString(),
  )
  return superviseProcess({
    command: [process.env.MDP_SECURE_INSTALL_BIN || process.env.MDP_BIN || 'mdp'],
    args,
    cwd: output.parent,
    environment: childEnvironment(false),
    timeoutMs: DEFAULT_TIMEOUT_MS,
    maxOutputBytes: MAX_CHILD_BUFFER_BYTES,
    inheritedFds: [output.fd],
  })
}

const publishPinnedOutput = async (output) => {
  if (output.securePath) {
    copyFileSync(output.cliPath, output.securePath, constants.COPYFILE_EXCL)
    const installed = lstatSync(output.securePath, { bigint: true })
    if (!installed.isFile() || installed.isSymbolicLink()) throw new Error('prepared output publication was not a regular file')
    return { dev: installed.dev, ino: installed.ino }
  }
  const invocation = await invokeSecureInstaller(output, 'install')
  let envelope
  try { envelope = JSON.parse(invocation.stdout) } catch { throw new Error('secure output installer returned invalid data') }
  if (invocation.status !== 0 || envelope?.ok !== true || envelope?.command !== 'secure-install' || envelope.data?.contract !== 'mdp.secure-install.v1' || envelope.data?.status !== 'installed') {
    throw new Error('secure output installer refused publication')
  }
  return { dev: BigInt(envelope.data.dev), ino: BigInt(envelope.data.ino) }
}

const removePinnedOutput = async (output) => {
  if (!output.installedIdentity) return
  if (!output.securePath) {
    try { await invokeSecureInstaller(output, 'remove') } catch { /* preserve unknown or concurrently replaced nodes */ }
    return
  }
  try {
    const current = lstatSync(output.securePath, { bigint: true })
    if (current.dev === output.installedIdentity.dev && current.ino === output.installedIdentity.ino) rmSync(output.securePath)
  } catch { /* preserve unknown or concurrently replaced nodes */ }
}

const normalizePreparedOutputPaths = (data, outputs) => {
  const normalized = { ...data }
  for (const key of ['request_path', 'manifest_path', 'next_command']) {
    if (typeof normalized[key] !== 'string') continue
    for (const output of outputs) {
      if (output.securePath) normalized[key] = normalized[key].replaceAll(output.securePath, output.path)
      if (output.cliPath) normalized[key] = normalized[key].replaceAll(output.cliPath, output.path)
    }
  }
  return normalized
}

const callPrepareRunValidated = async (args) => {
  const parsed = asObject(args || {}, 'arguments')
  assertOnly(parsed, new Set(['dir', 'job', 'operation', 'inputs', 'model', 'retention_policy', 'created_at', 'out', 'manifest_out', 'full', 'timeout_ms']))
  const policy = requirePolicy()
  const dir = policy.existing('pack', requiredString(parsed, 'dir'), 'directory').path
  const job = requiredString(parsed, 'job')
  const model = requiredString(parsed, 'model')
  if (parsed.operation !== undefined) requiredString(parsed, 'operation')
  if (parsed.created_at !== undefined) requiredString(parsed, 'created_at')
  if (parsed.retention_policy !== undefined && !['receipt-only', 'customer-controlled-workdir'].includes(parsed.retention_policy)) {
    throw new Error('retention_policy must be receipt-only or customer-controlled-workdir')
  }
  const inputs = parsed.inputs ?? []
  if (!Array.isArray(inputs) || inputs.length > 128 || inputs.some((value) => typeof value !== 'string')) throw new Error('inputs must be an array of logical_name=path strings')
  const frozenInputs = inputs.map((mapping) => {
    const separator = mapping.indexOf('=')
    if (separator <= 0) throw new Error('inputs must use logical_name=path')
    const name = mapping.slice(0, separator)
    const path = policy.freeze('input', mapping.slice(separator + 1)).path
    return `${name}=${path}`
  })
  const requestOutput = policy.newOutput('work', requiredString(parsed, 'out'))
  const out = requestOutput.path
  const manifestOutput = parsed.manifest_out === undefined ? null : policy.newOutput('work', requiredString(parsed, 'manifest_out'))
  const manifestOut = manifestOutput?.path ?? null
  if (manifestOut === out) throw new Error('out and manifest_out must name distinct files')
  const timeoutMs = parsed.timeout_ms ?? DEFAULT_TIMEOUT_MS
  if (!Number.isInteger(timeoutMs) || timeoutMs < 100 || timeoutMs > MAX_TIMEOUT_MS) throw new Error(`timeout_ms must be an integer between 100 and ${MAX_TIMEOUT_MS}`)
  const pinnedOutputs = []
  const privateDir = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-prepare-'))
  let published = false
  try {
    pinnedOutputs.push({ ...pinOutputParent(requestOutput), cliPath: join(privateDir, 'request.json'), installedIdentity: null })
    if (manifestOutput) pinnedOutputs.push({ ...pinOutputParent(manifestOutput), cliPath: join(privateDir, 'manifest.json'), installedIdentity: null })
    const requestParent = { path: requestOutput.parent, root: requestOutput.root, identity: requestOutput.parentIdentity }
    const manifestParent = manifestOutput && { path: manifestOutput.parent, root: manifestOutput.root, identity: manifestOutput.parentIdentity }
    policy.finalCheck('work', requestOutput.parent, requestParent, 'directory')
    if (manifestOutput) policy.finalCheck('work', manifestOutput.parent, manifestParent, 'directory')
    const cliArgs = ['--json', 'prepare-run', '--dir', dir, '--job', job, '--model', model]
    if (parsed.operation !== undefined) cliArgs.push('--operation', parsed.operation)
    for (const mapping of frozenInputs) cliArgs.push('--input', mapping)
    if (parsed.retention_policy !== undefined) cliArgs.push('--retention-policy', parsed.retention_policy)
    if (parsed.created_at !== undefined) cliArgs.push('--created-at', parsed.created_at)
    cliArgs.push('--out', pinnedOutputs[0].cliPath)
    if (manifestOut) cliArgs.push('--manifest-out', pinnedOutputs[1].cliPath)
    if (parsed.full === true) cliArgs.push('--full')
    const invocation = await invokeCli(cliArgs, dir, timeoutMs)
    if (invocation.timedOut || invocation.overflowed || invocation.spawnFailed) {
      return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: invocation.timedOut ? 'cli-timeout' : invocation.overflowed ? 'cli-output-limit' : 'cli-unavailable' }, true)
    }
    let envelope
    try { envelope = JSON.parse(invocation.stdout) } catch { return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: 'invalid-cli-output' }, true) }
    if (envelope?.ok === false && envelope?.command === 'prepare-run' && envelope.data?.contract === 'mdp.run-request-compile.v1') {
      return toolResult(normalizePreparedOutputPaths(envelope.data, pinnedOutputs))
    }
    if (envelope?.ok !== true || envelope?.command !== 'prepare-run' || !envelope.data || envelope.data.contract !== 'mdp.run-request-compile.v1' || envelope.data.status !== 'ready') {
      return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: invocation.status === 0 ? 'invalid-cli-contract' : 'prepare-run-refused' }, true)
    }
    policy.finalCheck('work', requestOutput.parent, requestParent, 'directory')
    if (manifestOutput) policy.finalCheck('work', manifestOutput.parent, manifestParent, 'directory')
    for (const output of pinnedOutputs) {
      const staged = lstatSync(output.cliPath)
      if (staged.isSymbolicLink() || !staged.isFile()) throw new Error('prepared output was not a regular file')
      output.installedIdentity = await publishPinnedOutput(output)
    }
    policy.finalCheck('work', requestOutput.parent, requestParent, 'directory')
    if (manifestOutput) policy.finalCheck('work', manifestOutput.parent, manifestParent, 'directory')
    policy.existing('work', out, 'file')
    if (manifestOut) policy.existing('work', manifestOut, 'file')
    policy.finalCheck('work', requestOutput.parent, requestParent, 'directory')
    if (manifestOutput) policy.finalCheck('work', manifestOutput.parent, manifestParent, 'directory')
    published = true
    return toolResult(normalizePreparedOutputPaths(envelope.data, pinnedOutputs))
  } finally {
    if (!published) for (const output of pinnedOutputs) await removePinnedOutput(output)
    for (const output of pinnedOutputs) closeSync(output.fd)
    rmSync(privateDir, { recursive: true, force: true })
  }
}

const callPrepareRun = async (args) => {
  try {
    return await callPrepareRunValidated(args)
  } catch (error) {
    return blockedPrepareRun('mcp-arguments-invalid', error?.message || 'preparation refused')
  }
}

const callRun = async (args, signal = null) => {
  const parsed = asObject(args || {}, 'arguments')
  assertOnly(parsed, new Set(['request_path', 'output_dir', 'timeout_ms', 'consent_id']))
  const requestPath = requiredString(parsed, 'request_path')
  const policy = requirePolicy()
  const outputRequest = requiredString(parsed, 'output_dir')
  const timeoutMs = parsed.timeout_ms ?? DEFAULT_TIMEOUT_MS
  validateTransportTimeout(timeoutMs)
  const frozenRequest = freezeRequestFile(policy.existing('work', requestPath, 'file').path)
  const approvedPack = frozenRequest.packDir
    ? policy.existing('pack', frozenRequest.packDir, 'directory')
    : null
  const providerCapable = frozenRequest.usesNativeModel && providerCapabilityAvailable()
  const frozenInputs = Array.isArray(frozenRequest.parsed?.inputs)
    ? frozenRequest.parsed.inputs.map((mapping) => {
        const sourcePath = typeof mapping === 'string'
          ? mapping.slice(mapping.indexOf('=') + 1)
          : mapping && typeof mapping.source_path === 'string' ? mapping.source_path : null
        if (!sourcePath || (typeof mapping === 'string' && mapping.indexOf('=') <= 0)) throw new Error('request inputs must declare source paths')
        return policy.freeze('input', sourcePath)
      })
    : []

  let invocation
  let plan = null
  const finalCheckInputs = () => {
    if (approvedPack) policy.finalCheck('pack', approvedPack.path, approvedPack, 'directory')
    for (const input of frozenInputs) policy.finalCheck('input', input.path, input)
  }
  try {
    assertOutputOutsidePack(frozenRequest.packDir, outputRequest)
    const outputParent = policy.existing('output', dirname(resolve(outputRequest)), 'directory')
    if (providerCapable) {
      const consentId = requiredString(parsed, 'consent_id')
      consumeProviderConsent({
        policy,
        consentId,
        provider: 'openai',
        purpose: 'mdp.run',
        requestSha256: frozenRequest.sha256,
        sourceSha256s: frozenInputs.map((input) => input.sha256),
        outputRoot: outputParent.root,
      })
    }
    finalCheckInputs()
    policy.finalCheck('output', outputParent.path, outputParent, 'directory')
    const parentDeadline = performance.now() + timeoutMs
    const preflightBudget = Math.max(1, Math.ceil(parentDeadline - performance.now()))
    const preflight = await invokeCli(
      ['--json', 'run-preflight', '--request', frozenRequest.path, '--transport-timeout-ms', String(timeoutMs)],
      outputParent.path,
      preflightBudget,
      null,
      providerCapable,
      null,
      signal,
    )
    if (preflight.timedOut || preflight.cancelled || preflight.overflowed || preflight.spawnFailed) {
      return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: preflight.cancelled ? 'cli-cancelled' : preflight.timedOut ? 'cli-timeout' : 'cli-unavailable' }, true)
    }
    let preflightEnvelope
    try {
      preflightEnvelope = JSON.parse(preflight.stdout)
    } catch {
      return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: 'invalid-cli-output' }, true)
    }
    plan = preflightEnvelope?.data
    if (
      preflightEnvelope?.ok !== true ||
      preflightEnvelope?.command !== 'run-preflight' ||
      !plan ||
      plan.contract !== 'mdp.run-preflight.v1'
    ) {
      return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: 'run-preflight-refused' }, true)
    }
    if (!validateDeadlinePlan(plan, timeoutMs, frozenRequest.executionId)) {
      return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: 'run-preflight-malformed' }, true)
    }
    finalCheckInputs()
    policy.finalCheck('output', outputParent.path, outputParent, 'directory')
    const outputReservation = policy.newOutput('output', outputRequest)
    const outputDir = outputReservation.path
    const runBudget = Math.max(1, Math.ceil(parentDeadline - performance.now()))
    invocation = await invokeCli(
      ['--json', 'run', '--request', frozenRequest.path, '--out-dir', outputDir, '--transport-timeout-ms', String(timeoutMs)],
      dirname(outputDir),
      runBudget,
      {
        outputDir,
        executionId: frozenRequest.executionId,
        requestSha256: frozenRequest.sha256,
      },
      providerCapable,
      plan,
      signal,
    )
  } finally {
    rmSync(frozenRequest.privateDir, { recursive: true, force: true })
  }
  if (invocation.timedOut || invocation.cancelled || invocation.overflowed || invocation.spawnFailed) {
    const code = invocation.cancelled
      ? 'cli-cancelled'
      : invocation.timedOut
      ? 'cli-timeout'
      : invocation.overflowed
        ? 'cli-output-limit'
        : 'cli-unavailable'
    return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code, ...(invocation.deadline ? { deadline: invocation.deadline } : {}) }, true)
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
        ...(invocation.deadline ? { deadline: invocation.deadline } : {}),
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
    return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: 'invalid-cli-contract', ...(invocation.deadline ? { deadline: invocation.deadline } : {}) }, true)
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
    return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: 'invalid-cli-contract', ...(invocation.deadline ? { deadline: invocation.deadline } : {}) }, true)
  }

  // A canonical no-draft result is decision data, not an MCP transport error.
  // Do not wrap, reinterpret, or promote assurance: the exact CLI data object
  // is the MCP tool result.
  return toolResult({ ...envelope.data, deadline_plan: plan })
}

const callVerifyRun = async (args) => {
  const parsed = asObject(args || {}, 'arguments')
  assertOnly(parsed, new Set(['bundle_path', 'receipt_path', 'artifact_root', 'timeout_ms']))
  const policy = requirePolicy()
  const bundlePath = policy.existing('output', requiredString(parsed, 'bundle_path'), 'file').path
  const receiptPath = policy.existing('output', requiredString(parsed, 'receipt_path'), 'file').path
  const artifactRoot = parsed.artifact_root === undefined
    ? null
    : policy.existing('output', requiredString(parsed, 'artifact_root'), 'directory').path
  const timeoutMs = parsed.timeout_ms ?? DEFAULT_TIMEOUT_MS
  validateTransportTimeout(timeoutMs)
  const cliArgs = ['--json', 'verify-run', '--bundle', bundlePath, '--receipt', receiptPath]
  if (artifactRoot) cliArgs.push('--artifact-root', artifactRoot)
  const invocation = await invokeCli(cliArgs, dirname(bundlePath), timeoutMs)
  if (invocation.timedOut || invocation.overflowed || invocation.spawnFailed) {
    const code = invocation.timedOut ? 'cli-timeout' : invocation.overflowed ? 'cli-output-limit' : 'cli-unavailable'
    return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code, ...(invocation.deadline ? { deadline: invocation.deadline } : {}) }, true)
  }
  let envelope
  try {
    envelope = JSON.parse(invocation.stdout)
  } catch {
    return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: 'invalid-cli-output', ...(invocation.deadline ? { deadline: invocation.deadline } : {}) }, true)
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
    return toolResult({ ok: false, contract: 'mdp.run-mcp-error.v1', code: 'invalid-cli-contract', ...(invocation.deadline ? { deadline: invocation.deadline } : {}) }, true)
  }
  // An invalid verification is a canonical integrity result, not an MCP
  // transport failure. Preserve it exactly so the caller can fail closed on
  // `valid: false` without losing the CLI's issue list.
  return toolResult(envelope.data)
}

const handleToolCall = async (params, signal = null) => {
  const call = asObject(params || {}, 'params')
  if (typeof call.name !== 'string' || call.name.trim() === '') {
    throw new Error('params.name must be a non-empty string')
  }
  switch (call.name) {
    case 'mdp_run_tools':
      return callRunTools(call.arguments || {})
    case 'mdp_prepare_run':
      return await callPrepareRun(call.arguments || {})
    case 'mdp_run':
      return await callRun(call.arguments || {}, signal)
    case 'mdp_verify_run':
      return await callVerifyRun(call.arguments || {})
    default:
      throw Object.assign(new Error(`unknown tool: ${call.name}`), { code: JSON_RPC_METHOD_NOT_FOUND })
  }
}

const activeRequests = new Map()

const cancelActiveRequest = (requestId) => {
  const controller = activeRequests.get(String(requestId))
  if (controller) controller.abort()
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
                'Use the canonical path in order: mdp_run_tools, mdp_prepare_run, mdp_run, then mdp_verify_run. Each stage consumes explicit local paths and returns CLI-owned artifacts. The surrounding agent and MCP adapter are control plane only; only CLI results and receipts have decision authority.',
            })
      case 'notifications/initialized':
        return null
      case '$/cancelRequest': {
        cancelActiveRequest(message.params?.requestId)
        return null
      }
      case 'notifications/cancelled': {
        cancelActiveRequest(message.params?.requestId)
        return null
      }
      case 'ping':
        return notification ? null : response(message.id, {})
      case 'tools/list':
        return notification ? null : response(message.id, { tools })
      case 'tools/call': {
        if (notification) return null
        const controller = new AbortController()
        activeRequests.set(String(message.id), controller)
        try {
          return response(message.id, await handleToolCall(message.params, controller.signal))
        } finally {
          activeRequests.delete(String(message.id))
        }
      }
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
  let parsed
  try { parsed = JSON.parse(line) } catch { parsed = null }
  const cancellation = parsed && !Array.isArray(parsed) &&
    (parsed.method === '$/cancelRequest' || parsed.method === 'notifications/cancelled')
  if (cancellation) {
    void handleRequest(parsed)
    return
  }
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
