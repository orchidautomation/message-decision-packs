#!/usr/bin/env node
import { spawn } from 'node:child_process'
import { existsSync, lstatSync, readFileSync, realpathSync, statSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createPathPolicy } from './lib/mcp-path-policy.mjs'
import { consumeProviderConsent } from './lib/mcp-provider-consent.mjs'
import {
  MAX_TIMEOUT_MS as DEADLINE_MAX_TIMEOUT_MS,
  MIN_TIMEOUT_MS,
  RECOMMENDED_TIMEOUT_MS,
  validateTransportTimeout,
} from './lib/deadline-policy.mjs'

const MCP_PROTOCOL_VERSION = '2025-06-18'
const SERVER_NAME = 'message-decision-packs-proposal'
const MAX_OUTPUT_CHARS = 80_000
const MAX_CHILD_BUFFER_BYTES = 1_000_000
const MAX_JSON_RPC_LINE_BYTES = 1_000_000
const MAX_SOURCE_COUNT = 16
const MAX_SOURCE_BYTES = 100_000
const MAX_SOURCE_FILE_BYTES = 5_000_000
const MAX_TOTAL_SOURCE_BYTES = 20_000_000
const DEFAULT_TIMEOUT_MS = RECOMMENDED_TIMEOUT_MS
const MAX_TIMEOUT_MS = DEADLINE_MAX_TIMEOUT_MS
const TERMINATION_GRACE_MS = 250
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
let pathPolicy = null
try { pathPolicy = createPathPolicy(process.env, ['pack', 'input', 'approval', 'work', 'consent']) } catch (error) { pathPolicy = { startupError: error } }
const requirePolicy = () => {
  if (pathPolicy?.startupError) throw pathPolicy.startupError
  return pathPolicy
}

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

const terminateProcessTree = (child, signal) => {
  if (!child.pid) return
  try {
    if (process.platform === 'win32') child.kill(signal)
    else process.kill(-child.pid, signal)
  } catch (error) {
    if (error?.code !== 'ESRCH') {
      try {
        child.kill(signal)
      } catch {
        // The child already exited.
      }
    }
  }
}

const runNode = (script, args, timeoutMs = DEFAULT_TIMEOUT_MS, includeProviderCredential = false) => {
  const environment = childEnvironment()
  if (!includeProviderCredential) delete environment.OPENAI_API_KEY
  return new Promise((resolveResult) => {
    const stdoutChunks = []
    const stderrChunks = []
    let stdoutBytes = 0
    let stderrBytes = 0
    let timedOut = false
    let overflowed = false
    let spawnError = null
    let finished = false
    let killTimer = null

    const child = spawn(process.execPath, [script, ...args], {
      cwd: bundleRoot,
      detached: process.platform !== 'win32',
      env: environment,
      stdio: ['ignore', 'pipe', 'pipe'],
    })

    const collect = (chunks, chunk, stream) => {
      const nextBytes = stream === 'stdout' ? stdoutBytes + chunk.length : stderrBytes + chunk.length
      if (nextBytes > MAX_CHILD_BUFFER_BYTES) {
        overflowed = true
        terminateProcessTree(child, 'SIGTERM')
        killTimer ||= setTimeout(() => terminateProcessTree(child, 'SIGKILL'), TERMINATION_GRACE_MS)
        return
      }
      chunks.push(chunk)
      if (stream === 'stdout') stdoutBytes = nextBytes
      else stderrBytes = nextBytes
    }
    child.stdout.on('data', (chunk) => collect(stdoutChunks, chunk, 'stdout'))
    child.stderr.on('data', (chunk) => collect(stderrChunks, chunk, 'stderr'))
    child.on('error', (error) => {
      spawnError = error
    })

    const timeout = setTimeout(() => {
      timedOut = true
      terminateProcessTree(child, 'SIGTERM')
      killTimer = setTimeout(() => terminateProcessTree(child, 'SIGKILL'), TERMINATION_GRACE_MS)
    }, timeoutMs)

    child.on('close', (code, signal) => {
      if (finished) return
      finished = true
      clearTimeout(timeout)
      if (killTimer) clearTimeout(killTimer)
      const stdout = redact(Buffer.concat(stdoutChunks).toString('utf8'), environment)
      const rawStderr = Buffer.concat(stderrChunks).toString('utf8')
      const errorText = spawnError && !timedOut ? spawnError.message : ''
      const stderr = redact(`${rawStderr}${errorText}`, environment)
      resolveResult({
        status: timedOut ? 124 : overflowed || spawnError ? 1 : (code ?? 1),
        stdout,
        stderr: timedOut
          ? `proposal runner timed out after ${timeoutMs}ms`
          : overflowed
            ? `proposal runner exceeded ${MAX_CHILD_BUFFER_BYTES} bytes of buffered output`
            : stderr,
        timedOut,
        signal: timedOut ? 'SIGTERM' : signal,
        environmentKeys: Object.keys(environment).sort(),
      })
    })
  })
}

const parseRunnerJson = (stdout) => {
  try {
    return JSON.parse(stdout)
  } catch (error) {
    throw new Error(`runner stdout was not valid JSON: ${error.message}`)
  }
}

const isRecord = (value) => value !== null && typeof value === 'object' && !Array.isArray(value)

const validSourceAuthority = (authority, valid) => {
  if (!isRecord(authority) || !Array.isArray(authority.obligations) || !Array.isArray(authority.reason_codes)) return false
  if (authority.authority_level === 'authoritative' && authority.disposition === 'allow') {
    return valid === true && authority.terminal === 'success' &&
      ['available', 'not-applicable'].includes(authority.governed_generation) &&
      authority.obligations.every((gate) => ['pass', 'not-applicable'].includes(gate?.result))
  }
  if (authority.authority_level === 'authoritative' && authority.disposition === 'block') {
    return valid === false && authority.terminal === 'no-draft' && authority.governed_generation === 'absent' &&
      authority.obligations.some((gate) => gate?.result === 'fail')
  }
  return authority.authority_level === 'unavailable' && authority.disposition === 'undetermined' &&
    authority.terminal === 'authority-unavailable' && authority.governed_generation === 'absent'
}

const validProposalRunnerEnvelope = (parsed, cleanRunV1) => {
  if (!isRecord(parsed)) return false
  const baseKeys = [
    'contract', 'runner_contract', 'mode', 'ok', 'audit_grade_eligible', 'decision',
    'runner_assurance', 'run_id', 'run_manifest', 'readiness_report', 'workdir',
    'artifacts', 'steps', 'caveats',
  ]
  const v1Keys = ['authority_contract', 'terminal_state', 'canonical_run', 'canonical_authority']
  const allowed = new Set(cleanRunV1 ? [...baseKeys, ...v1Keys] : baseKeys)
  if (Object.keys(parsed).some((key) => !allowed.has(key)) || [...allowed].some((key) => !(key in parsed))) return false
  if (parsed.contract !== (cleanRunV1 ? 'mdp.proposal-runner-result.v1' : 'mdp.proposal-runner-result.v0')) return false
  if (!['dry-run', 'mock', 'native'].includes(parsed.mode) || typeof parsed.ok !== 'boolean' ||
      typeof parsed.audit_grade_eligible !== 'boolean' ||
      !['not-run', 'audit-grade', 'advisory', 'blocked'].includes(parsed.decision) ||
      !isRecord(parsed.artifacts) || !Array.isArray(parsed.steps) ||
      !Array.isArray(parsed.caveats) || parsed.caveats.length === 0) return false
  if (!cleanRunV1) return true

  const run = parsed.canonical_run
  const authorityBlock = parsed.canonical_authority
  return parsed.authority_contract === 'mdp.run-execution.v1' &&
    parsed.runner_assurance === 'see-canonical-authority' &&
    isRecord(run) && run.contract === 'mdp.run-execution.v1' &&
    run.terminal_state === parsed.terminal_state &&
    validSourceAuthority(run.authority, run.valid) &&
    isRecord(authorityBlock) && authorityBlock.contract === 'mdp.canonical-authority-block.v1' &&
    authorityBlock.terminal_state === parsed.terminal_state &&
    JSON.stringify(authorityBlock) === JSON.stringify(run.authority_block)
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
      maxItems: MAX_SOURCE_COUNT,
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
    clean_run_v1: {
      type: 'boolean',
      description: 'Finalize the generated output with canonical Rust mdp run v1. Requires pack_release_id and cannot be combined with dry_run.',
    },
    pack_release_id: {
      type: 'string',
      description: 'Immutable pack release identifier handed unchanged to canonical mdp run v1.',
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
      maximum: MAX_SOURCE_BYTES,
      description: 'Per-source bounded text bytes to include in the prompt payload.',
    },
    timeout_ms: {
      type: 'integer',
      minimum: MIN_TIMEOUT_MS,
      maximum: MAX_TIMEOUT_MS,
      description: `Child-process deadline in milliseconds. Defaults to ${DEFAULT_TIMEOUT_MS}.`,
    },
    consent_id: {
      type: 'string',
      description: 'Out-of-band one-shot consent record id required for real native runs.',
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
    'inner_timeout_ms',
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
    authority_contract: { type: ['string', 'null'] },
    terminal_state: { type: ['string', 'null'] },
    canonical_authority: { type: ['object', 'null'] },
    timed_out: { type: 'boolean' },
    termination_signal: { type: ['string', 'null'] },
    timeout_ms: { type: 'integer', minimum: MIN_TIMEOUT_MS, maximum: MAX_TIMEOUT_MS },
    inner_timeout_ms: { const: RECOMMENDED_TIMEOUT_MS },
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
      'Run the local proposal runner from explicit local file paths only. It stages supplied sources, builds a declared-input-only native request, optionally invokes the native runner, and can hand an existing output to canonical Rust mdp run v1 for deterministic validation and receipt authority. The v1 handoff does not claim the Rust runtime performed upstream model inference.',
    inputSchema: proposalRunSchema,
    outputSchema: proposalRunOutputSchema,
  },
]

const callProposalTools = async (args) => {
  const parsedArgs = asObject(args || {}, 'arguments')
  assertNoUnsupportedArgs(parsedArgs, new Set())
  const result = await runNode(runnerPath, ['tools'])
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

const callProposalRun = async (args) => {
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
    'clean_run_v1',
    'pack_release_id',
    'prompt_id',
    'reuse_workdir_id',
    'skip_review',
    'require_audit_grade',
    'max_source_bytes',
    'timeout_ms',
    'consent_id',
  ])
  assertNoUnsupportedArgs(parsedArgs, allowed)

  const packArg = optionalString(parsedArgs, 'pack')
  const workdirArg = optionalString(parsedArgs, 'workdir')
  if (!packArg) throw new Error('pack is required')
  if (!workdirArg) throw new Error('workdir is required')
  const policy = requirePolicy()
  const pack = canonicalPack(policy.existing('pack', packArg, 'directory').path)
  const workdir = existsSync(workdirArg)
    ? canonicalWorkdir(policy.existing('work', workdirArg, 'directory').path)
    : policy.newOutput('work', workdirArg).path

  const sourcePaths = optionalStringArray(parsedArgs, 'source_paths').map((path, index) =>
    policy.existing('input', path, 'file').path,
  )
  if (sourcePaths.length > MAX_SOURCE_COUNT) {
    throw new Error(`source_paths must contain at most ${MAX_SOURCE_COUNT} files`)
  }
  let totalSourceBytes = 0
  for (const [index, path] of sourcePaths.entries()) {
    const sourceBytes = statSync(path).size
    if (sourceBytes > MAX_SOURCE_FILE_BYTES) {
      throw new Error(`source_paths[${index}] exceeds the ${MAX_SOURCE_FILE_BYTES} byte file limit`)
    }
    totalSourceBytes += sourceBytes
  }
  if (totalSourceBytes > MAX_TOTAL_SOURCE_BYTES) {
    throw new Error(`source_paths exceed the ${MAX_TOTAL_SOURCE_BYTES} byte total limit`)
  }
  const sourceIntakeArg = optionalString(parsedArgs, 'source_intake_path')
  const sourceAuditArg = optionalString(parsedArgs, 'source_audit_path')
  const sourceIntakePath = sourceIntakeArg
    ? policy.existing('approval', sourceIntakeArg, 'file').path
    : null
  const sourceAuditPath = sourceAuditArg
    ? policy.existing('approval', sourceAuditArg, 'file').path
    : null
  const sourceId = optionalString(parsedArgs, 'source_id')
  const sourceKind = optionalString(parsedArgs, 'source_kind')
  const privacyClass = optionalString(parsedArgs, 'privacy_class')
  const model = optionalString(parsedArgs, 'model')
  const mockResponseArg = optionalString(parsedArgs, 'mock_response_path')
  const mockResponsePath = mockResponseArg
    ? policy.existing('input', mockResponseArg, 'file').path
    : null
  const promptId = optionalString(parsedArgs, 'prompt_id')
  const maxSourceBytes = optionalInteger(parsedArgs, 'max_source_bytes')
  if (maxSourceBytes !== null && (maxSourceBytes < 1000 || maxSourceBytes > MAX_SOURCE_BYTES)) {
    throw new Error(`max_source_bytes must be between 1000 and ${MAX_SOURCE_BYTES}`)
  }
  const dryRun = optionalBoolean(parsedArgs, 'dry_run')
  const cleanRunV1 = optionalBoolean(parsedArgs, 'clean_run_v1')
  const packReleaseId = optionalString(parsedArgs, 'pack_release_id')
  const reuseWorkdirId = optionalString(parsedArgs, 'reuse_workdir_id')
  const skipReview = optionalBoolean(parsedArgs, 'skip_review')
  const requireAuditGrade = optionalBoolean(parsedArgs, 'require_audit_grade')
  const consentId = optionalString(parsedArgs, 'consent_id')
  const timeoutMs = optionalInteger(parsedArgs, 'timeout_ms') ?? DEFAULT_TIMEOUT_MS
  validateTransportTimeout(timeoutMs)
  if (cleanRunV1 && !packReleaseId) {
    throw new Error('clean_run_v1 requires pack_release_id')
  }
  if (cleanRunV1 && dryRun) {
    throw new Error('clean_run_v1 cannot be combined with dry_run')
  }

  if (sourcePaths.length === 0) {
    throw new Error('Pass at least one source_paths file. Ambient chat/source text and audit-only runs are intentionally not accepted.')
  }
  if (!dryRun && !mockResponsePath) {
    if (!consentId) throw new Error('consent_id is required for real native runs')
    consumeProviderConsent({ policy, consentId, provider: 'openai', purpose: 'mdp.proposal-run', requestSha256: 'pending-runner-request', outputRoot: policy.roots.work[0] })
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
  if (cleanRunV1) runnerArgs.push('--clean-run-v1')
  if (packReleaseId) runnerArgs.push('--pack-release-id', packReleaseId)
  if (promptId) runnerArgs.push('--prompt-id', promptId)
  if (reuseWorkdirId) runnerArgs.push('--reuse-workdir-id', reuseWorkdirId)
  if (skipReview) runnerArgs.push('--skip-review')
  if (requireAuditGrade) runnerArgs.push('--require-audit-grade')
  if (consentId) runnerArgs.push('--consent-id', consentId)
  if (maxSourceBytes !== null) runnerArgs.push('--max-source-bytes', String(maxSourceBytes))
  runnerArgs.push('--timeout-ms', String(timeoutMs))

  const result = await runNode(runnerPath, runnerArgs, timeoutMs, !dryRun && !mockResponsePath)
  let parsed = null
  try {
    parsed = parseRunnerJson(result.stdout)
  } catch (error) {
    if (result.status === 0) throw error
  }

  const validRunnerEnvelope = validProposalRunnerEnvelope(parsed, cleanRunV1)
  const auditGradeRejected =
    requireAuditGrade &&
    (!validRunnerEnvelope || parsed.decision !== 'audit-grade' || parsed.audit_grade_eligible !== true)
  const ok = result.status === 0 && validRunnerEnvelope && parsed.ok !== false && !auditGradeRejected
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
    ...(cleanRunV1
      ? {
          authority_contract: parsed?.authority_contract ?? null,
          terminal_state: parsed?.terminal_state ?? null,
          canonical_authority: parsed?.canonical_authority ?? null,
        }
      : {}),
    timed_out: result.timedOut,
    termination_signal: result.signal,
    timeout_ms: timeoutMs,
    inner_timeout_ms: RECOMMENDED_TIMEOUT_MS,
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
      'For clean_run_v1, terminal state and canonical authority are returned unchanged from canonical Rust mdp run output; MCP does not calculate v1 hashes, assurance, terminal state, or receipt authority.',
      'Dry-run/mock/demo/fixture/synthetic evidence remains non-audit-grade.',
    ],
  }

  if (!validRunnerEnvelope) {
    return toolResult({
      isError: true,
      text: compact(JSON.stringify(structuredContent, null, 2)),
      structuredContent,
    })
  }

  // A well-formed runner envelope is a successful MCP transport even when
  // its canonical decision is blocked or audit-grade was refused. Preserve
  // the runner result and exit status as data; only malformed/missing output
  // is a transport error.
  return toolResult({ text: JSON.stringify(structuredContent, null, 2), structuredContent })
}

const handleToolCall = async (params) => {
  const call = asObject(params || {}, 'params')
  if (typeof call.name !== 'string' || call.name.trim() === '') throw new Error('params.name must be a non-empty string')
  const args = call.arguments || {}
  switch (call.name) {
    case 'mdp_proposal_tools':
      return await callProposalTools(args)
    case 'mdp_proposal_run':
      return await callProposalRun(args)
    default:
      throw Object.assign(new Error(`Unknown tool: ${call.name}`), { code: JSON_RPC_METHOD_NOT_FOUND })
  }
}

const handleRequest = async (message) => {
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
            'Use mdp_proposal_run only with explicit local file paths. Do not pass ambient chat/source text as proposal evidence. clean_run_v1 delegates deterministic validation and receipt authority to canonical Rust mdp run but does not prove the Rust runtime performed upstream model inference.',
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
        return response(id, await handleToolCall(params))
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

const handleLine = async (line) => {
  if (Buffer.byteLength(line, 'utf8') > MAX_JSON_RPC_LINE_BYTES) {
    writeMessage(
      errorResponse(null, JSON_RPC_INVALID_REQUEST, `JSON-RPC message exceeds ${MAX_JSON_RPC_LINE_BYTES} bytes`),
    )
    return
  }
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
      const itemResponse = await handleRequest(item)
      if (itemResponse) responses.push(itemResponse)
    }
    if (responses.length > 0) writeMessage(responses)
    return
  }

  const messageResponse = await handleRequest(message)
  if (messageResponse) writeMessage(messageResponse)
}

let buffer = ''
let discardingOversizedLine = false
let inputQueue = Promise.resolve()
const enqueueLine = (line) => {
  inputQueue = inputQueue.then(() => handleLine(line))
}
process.stdin.setEncoding('utf8')
process.stdin.resume()
process.stdin.on('data', (chunk) => {
  if (discardingOversizedLine) {
    const newlineIndex = chunk.indexOf('\n')
    if (newlineIndex < 0) return
    discardingOversizedLine = false
    chunk = chunk.slice(newlineIndex + 1)
  }
  buffer += chunk
  let newlineIndex
  while ((newlineIndex = buffer.indexOf('\n')) >= 0) {
    const line = buffer.slice(0, newlineIndex)
    buffer = buffer.slice(newlineIndex + 1)
    enqueueLine(line)
  }
  if (Buffer.byteLength(buffer, 'utf8') > MAX_JSON_RPC_LINE_BYTES) {
    writeMessage(
      errorResponse(null, JSON_RPC_INVALID_REQUEST, `JSON-RPC message exceeds ${MAX_JSON_RPC_LINE_BYTES} bytes`),
    )
    buffer = ''
    discardingOversizedLine = true
  }
})

process.stdin.on('end', () => {
  const remaining = buffer.trim()
  if (remaining && !discardingOversizedLine) enqueueLine(remaining)
})

process.on('uncaughtException', (error) => {
  process.stderr.write(`mdp proposal MCP server fatal error: ${redact(error.stack || error.message)}\n`)
  process.exit(1)
})
