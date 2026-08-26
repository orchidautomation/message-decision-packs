import assert from 'node:assert/strict'
import { chmodSync, cpSync, existsSync, linkSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { consentBinding, consumeProviderConsent } from './lib/mcp-provider-consent.mjs'
import { createPathPolicy } from './lib/mcp-path-policy.mjs'
import { tmpdir } from 'node:os'
import { dirname, join, relative, resolve } from 'node:path'
import { spawn } from 'node:child_process'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const server = join(dirname(fileURLToPath(import.meta.url)), 'mdp-run-mcp-server.mjs')
const repoRoot = resolve(dirname(server), '..')
const realCli = join(repoRoot, 'cli', 'target', 'debug', 'mdp')

const fixtureCli = (root) => {
  const path = join(root, 'fake-mdp.mjs')
  writeFileSync(
    path,
    `#!/usr/bin/env node
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { spawn } from 'node:child_process'
const args = process.argv.slice(2)
if (args.includes('verify-run')) {
  const receiptPath = args[args.indexOf('--receipt') + 1]
  const receipt = JSON.parse(readFileSync(receiptPath, 'utf8'))
  const data = { contract: 'mdp.run-verification.v1', valid: receipt.valid !== false, checks: [] }
  process.stdout.write(JSON.stringify({ ok: true, command: 'verify-run', data }))
  if (!data.valid) process.exit(1)
  process.exit(0)
}
if (args.includes('run-preflight')) {
  const transportTimeout = Number(args[args.indexOf('--transport-timeout-ms') + 1] || 60000)
  const runtimeTimeout = 60000
  process.stdout.write(JSON.stringify({ ok: true, command: 'run-preflight', data: {
    contract: 'mdp.run-preflight.v1',
    execution_id: 'exec-fixture',
    mode: 'deterministic',
    recommended_timeout_ms: 60000,
    runtime_configured_ms: 60000,
    transport_configured_ms: transportTimeout,
    provider_configured_ms: 60000,
    finalization_reserve_ms: 250,
    effective_limit_ms: Math.min(runtimeTimeout, transportTimeout - 250),
    warnings: transportTimeout > runtimeTimeout
      ? ['outer-timeout-cannot-extend-inner']
      : transportTimeout - 250 < runtimeTimeout
        ? ['outer-timeout-truncates-runtime']
        : [],
    staging: 'not-started',
    provider: 'not-started',
  }}))
  process.exit(0)
}
const requestPath = args[args.indexOf('--request') + 1]
const outputDir = args[args.indexOf('--out-dir') + 1]
if (existsSync(outputDir + '.pause-before-read')) {
  writeFileSync(outputDir + '.ready', '')
  while (!existsSync(outputDir + '.continue')) {
    await new Promise((resolveWait) => setTimeout(resolveWait, 5))
  }
}
const request = JSON.parse(readFileSync(requestPath, 'utf8'))
writeFileSync(outputDir + '.invocation.json', JSON.stringify({
  args,
  request,
  secret_seen: Boolean(process.env.MDP_MCP_SECRET_MARKER),
  credential_canary_seen: [
    'test-key-must-not-be-printed',
    'must-not-cross-after-mutation',
  ].includes(process.env.OPENAI_API_KEY),
  env_keys: Object.keys(process.env).sort(),
}))
if (request.test_mode === 'fail') {
  process.stderr.write('PRIVATE-SOURCE-BODY /private/customer/path\\n')
  process.exit(7)
}
if (request.test_mode === 'invalid-json') {
  process.stdout.write('not json')
  process.exit(0)
}
if (request.test_mode === 'hang') {
  process.on('SIGTERM', () => {})
  setInterval(() => {}, 1000)
}
if (request.test_mode === 'overflow') {
  process.on('SIGTERM', () => {})
  process.stdout.write('x'.repeat(1_100_000))
  setInterval(() => {}, 1000)
}
if (request.test_mode === 'descendant') {
  const childCode = "process.on('SIGTERM',()=>{});setTimeout(()=>require('node:fs').writeFileSync(" + JSON.stringify(request.marker_path) + ",'survived'),700);setInterval(()=>{},1000)"
  spawn(process.execPath, ['-e', childCode], { stdio: 'ignore' })
  process.on('SIGTERM', () => process.exit(0))
  setInterval(() => {}, 1000)
}
const blocked = request.test_mode === 'no-draft'
const unavailable = request.test_mode === 'unavailable'
const data = {
  contract: request.test_mode === 'wrong-contract' ? 'wrong.run-contract' : 'mdp.run-execution.v1',
  valid: request.test_mode === 'wrong-contract' ? 'yes' : !blocked && !unavailable,
  execution_id: 'exec-fixture',
  terminal_state: blocked ? 'no-draft:decision-invalid' : unavailable ? 'no-draft:runner-failed' : 'success',
  run_dir: outputDir,
  bundle_sha256: 'a'.repeat(64),
  receipt_sha256: 'b'.repeat(64),
  authority: {
    authority_level: unavailable ? 'unavailable' : 'authoritative',
    disposition: unavailable ? 'undetermined' : blocked ? 'block' : 'allow',
    terminal: unavailable ? 'authority-unavailable' : blocked ? 'no-draft' : 'success',
    governed_generation: blocked || unavailable ? 'absent' : 'available',
    obligations: [{ id: 'fixture-decision', result: unavailable ? 'unknown' : blocked ? 'fail' : 'pass' }],
    reason_codes: blocked ? ['fixture-no-draft'] : unavailable ? ['fixture-unavailable'] : [],
  },
  authority_block: {
    terminal_state: blocked ? 'no-draft:decision-invalid' : unavailable ? 'no-draft:runner-failed' : 'success',
    decision: blocked
      ? { decision: 'no-draft', reason_codes: ['fixture-no-draft'] }
      : unavailable ? null : { decision: 'ready', reason_codes: [] },
    assurance: { 'declared-input-isolation': { level: 'unknown' } },
    limitations: ['fixture limitation'],
    diagnostics: blocked
      ? [{
        stage: 'generative-preflight',
        gate: 'routed-context-schema',
        code: 'wrong-contract',
        input: 'routed_context',
        field: '/contract',
        expected: { kind: 'contract', value: 'mdp.routed-context.v1' },
        observed: { kind: 'contract', value: 'missing' },
      }]
      : [],
  },
}
process.stdout.write(JSON.stringify({ ok: true, command: 'run', data }))
if (blocked || unavailable) process.exit(1)
if (request.test_mode === 'nonzero-success') process.exit(1)
if (request.test_mode === 'wrong-contract') process.exit(0)
`,
  )
  chmodSync(path, 0o755)
  return path
}

const pathValues = (value, result = []) => {
  if (typeof value === 'string' && value.startsWith('/')) result.push(value)
  else if (value && typeof value === 'object') Object.values(value).forEach((item) => pathValues(item, result))
  return result
}

const testRoot = (messages) => {
  const candidates = pathValues(messages).map((value) => {
    try { return existsSync(value) && !lstatSync(value).isDirectory() ? dirname(value) : value } catch { return dirname(value) }
  }).filter((value) => value.startsWith('/'))
  let common = candidates[0] || repoRoot
  for (const candidate of candidates.slice(1)) {
    while (relative(common, candidate).startsWith('..')) {
      const parent = dirname(common)
      if (parent === common) return repoRoot
      common = parent
    }
  }
  return common
}

const rpc = (cli, messages, extraEnv = {}) =>
  new Promise((resolvePromise, rejectPromise) => {
    const roots = testRoot(messages)
    const child = spawn(process.execPath, [server], {
      env: { ...process.env, MDP_BIN: cli, MDP_MCP_SECRET_MARKER: 'must-not-cross-boundary', ...Object.fromEntries(['PACK', 'INPUT', 'APPROVAL', 'WORK', 'OUTPUT', 'CONSENT'].map((role) => [`MDP_MCP_${role}_ROOTS`, roots])), ...extraEnv },
      stdio: ['pipe', 'pipe', 'pipe'],
    })
    let stdout = ''
    let stderr = ''
    child.stdout.setEncoding('utf8')
    child.stderr.setEncoding('utf8')
    child.stdout.on('data', (chunk) => {
      stdout += chunk
    })
    child.stderr.on('data', (chunk) => {
      stderr += chunk
    })
    child.on('error', rejectPromise)
    child.on('close', (status) => {
      if (status !== 0) return rejectPromise(new Error(`server exited ${status}: ${stderr}`))
      resolvePromise(
        stdout
          .trim()
          .split('\n')
          .filter(Boolean)
          .map((line) => JSON.parse(line)),
      )
    })
    for (const message of messages) {
      child.stdin.write(typeof message === 'string' ? `${message}\n` : `${JSON.stringify(message)}\n`)
    }
    child.stdin.end()
  })

const toolCall = (id, name, args = {}) => ({
  jsonrpc: '2.0',
  id,
  method: 'tools/call',
  params: { name, arguments: args },
})

const consentFixture = (root, id, overrides = {}) => {
  const value = { contract: 'mdp.mcp-provider-consent.v1', provider: 'openai', purpose: 'mdp.run', request_sha256: 'a'.repeat(64), source_sha256s: [], output_root: realpathSync(root), expires_at: new Date(Date.now() + 60_000).toISOString(), nonce: `${id}-nonce`, ...overrides }
  value.binding_sha256 = consentBinding({ provider: value.provider, purpose: value.purpose, requestSha256: value.request_sha256, sourceSha256s: value.source_sha256s, outputRoot: value.output_root, expiresAt: value.expires_at, nonce: value.nonce })
  writeFileSync(join(root, `${id}.json`), JSON.stringify(value))
  return value
}

test('freezes consent records, rejects mismatch/expiry, and consumes each nonce once', () => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-mcp-consent-'))
  try {
    const policy = createPathPolicy({ MDP_MCP_CONSENT_ROOTS: root }, ['consent'])
    const record = consentFixture(root, 'valid')
    const accepted = consumeProviderConsent({ policy, consentId: 'valid', provider: 'openai', purpose: 'mdp.run', requestSha256: record.request_sha256, outputRoot: record.output_root })
    assert.equal(accepted.nonce, record.nonce)
    assert.throws(() => consumeProviderConsent({ policy, consentId: 'valid', provider: 'openai', purpose: 'mdp.run', requestSha256: record.request_sha256, outputRoot: record.output_root }), /already been consumed/)
    const expired = consentFixture(root, 'expired', { expires_at: new Date(Date.now() - 1_000).toISOString() })
    assert.throws(() => consumeProviderConsent({ policy, consentId: 'expired', provider: 'openai', purpose: 'mdp.run', requestSha256: expired.request_sha256, outputRoot: expired.output_root }), /expired/)
    const mismatch = consentFixture(root, 'mismatch')
    assert.throws(() => consumeProviderConsent({ policy, consentId: 'mismatch', provider: 'openai', purpose: 'mdp.run', requestSha256: 'b'.repeat(64), outputRoot: mismatch.output_root }), /does not match/)
    const ordered = consentFixture(root, 'ordered', { source_sha256s: ['a'.repeat(64), 'b'.repeat(64)] })
    assert.throws(() => consumeProviderConsent({ policy, consentId: 'ordered', provider: 'openai', purpose: 'mdp.run', requestSha256: ordered.request_sha256, sourceSha256s: ['b'.repeat(64), 'a'.repeat(64)], outputRoot: ordered.output_root }), /does not match/)
  } finally { rmSync(root, { recursive: true, force: true }) }
})

test('denies a generative request without consent before any provider spawn', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-mcp-no-consent-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const request = join(root, 'request.json')
  writeFileSync(request, JSON.stringify({ contract: 'mdp.run-request.v1', mode: 'generative' }))
  const [reply] = await rpc(fixtureCli(root), [toolCall(1, 'mdp_run', { request_path: request, output_dir: join(root, 'run') })], { OPENAI_API_KEY: 'consent-test-key', MDP_ALLOW_NATIVE_MODEL_CALLS: '1' })
  assert.equal(reply.error.code, -32602)
  assert.match(reply.error.message, /consent/)
  assert.equal(existsSync(join(root, 'run.invocation.json')), false)
})

const waitForFile = async (path) => {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (existsSync(path)) return
    await new Promise((resolveWait) => setTimeout(resolveWait, 5))
  }
  throw new Error(`fixture did not create ${path}`)
}

test('lists preparation, run, and verification tools and identifies MCP as transport only', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const replies = await rpc(fixtureCli(root), [
    { jsonrpc: '2.0', id: 1, method: 'initialize', params: {} },
    { jsonrpc: '2.0', id: 2, method: 'tools/list' },
    toolCall(3, 'mdp_run_tools'),
  ])
  assert.equal(replies[0].result.serverInfo.name, 'message-decision-packs-runner')
  assert.deepEqual(replies[1].result.tools.map((tool) => tool.name), ['mdp_run_tools', 'mdp_prepare_run', 'mdp_run', 'mdp_verify_run'])
  assert.deepEqual(replies[2].result.structuredContent.mcp_authority, [])
  assert.match(replies[2].result.structuredContent.guardrails.join(' '), /does not prove fresh context/)
})

test('returns canonical valid and invalid read-only verification data', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const bundle = join(root, 'run-bundle.json')
  const validReceipt = join(root, 'valid-receipt.json')
  const invalidReceipt = join(root, 'invalid-receipt.json')
  const artifacts = join(root, 'artifacts')
  writeFileSync(bundle, '{}')
  writeFileSync(validReceipt, JSON.stringify({ valid: true }))
  writeFileSync(invalidReceipt, JSON.stringify({ valid: false }))
  mkdirSync(artifacts)
  const replies = await rpc(fixtureCli(root), [
    toolCall(1, 'mdp_verify_run', { bundle_path: bundle, receipt_path: validReceipt, artifact_root: artifacts }),
    toolCall(2, 'mdp_verify_run', { bundle_path: bundle, receipt_path: invalidReceipt }),
  ])
  assert.equal(replies[0].result.isError, false)
  assert.equal(replies[0].result.structuredContent.valid, true)
  assert.equal(replies[1].result.isError, false)
  assert.equal(replies[1].result.structuredContent.valid, false)
})

test('notifications/cancelled aborts a hanging clean run with sanitized cancellation data', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-cancel-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const request = join(root, 'run-request.json')
  const output = join(root, 'cancelled-run')
  writeFileSync(request, JSON.stringify({ test_mode: 'hang' }))
  const child = spawn(process.execPath, [server], {
    env: { ...process.env, MDP_BIN: fixtureCli(root), ...Object.fromEntries(['PACK', 'INPUT', 'APPROVAL', 'WORK', 'OUTPUT', 'CONSENT'].map((role) => [`MDP_MCP_${role}_ROOTS`, root])) },
    stdio: ['pipe', 'pipe', 'pipe'],
  })
  const replies = []
  let outputText = ''
  child.stdout.setEncoding('utf8')
  child.stdout.on('data', (chunk) => {
    outputText += chunk
    let newline
    while ((newline = outputText.indexOf('\n')) >= 0) {
      const line = outputText.slice(0, newline)
      outputText = outputText.slice(newline + 1)
      if (line.trim()) replies.push(JSON.parse(line))
    }
  })
  child.stdin.write(`${JSON.stringify(toolCall(1, 'mdp_run', { request_path: request, output_dir: output }))}\n`)
  const cancelTimer = setTimeout(() => {
    child.stdin.write(`${JSON.stringify({
      jsonrpc: '2.0',
      method: 'notifications/cancelled',
      params: { requestId: 1 },
    })}\n`)
  }, 500)
  await new Promise((resolvePromise, rejectPromise) => {
    const deadline = setTimeout(() => rejectPromise(new Error('cancellation response timed out')), 5000)
    const poll = setInterval(() => {
      if (replies.some((reply) => reply.id === 1)) {
        clearTimeout(deadline)
        clearInterval(poll)
        resolvePromise()
      }
    }, 10)
    child.once('error', rejectPromise)
  })
  clearTimeout(cancelTimer)
  const reply = replies.find((item) => item.id === 1)
  assert.equal(reply.result.isError, true)
  assert.equal(reply.result.structuredContent.code, 'cli-cancelled')
  assert.equal(reply.result.structuredContent.deadline.outcome, 'cancelled')
  assert.equal(reply.result.structuredContent.deadline.phase, 'cancellation')
  assert.equal(JSON.stringify(reply).includes(root), false)
  child.kill('SIGKILL')
})

test('passes only file paths to a bounded CLI child and returns its authority unchanged', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const cli = fixtureCli(root)
  const request = join(root, 'run-request.json')
  const output = join(root, 'new-run')
  writeFileSync(request, '{}')

  const [reply] = await rpc(cli, [toolCall(1, 'mdp_run', { request_path: request, output_dir: output })])
  const result = reply.result.structuredContent
  assert.equal(reply.result.isError, false)
  assert.equal(result.execution_id, 'exec-fixture')
  assert.deepEqual(result.authority_block.assurance, {
    'declared-input-isolation': { level: 'unknown' },
  })
  assert.equal('mcp_assurance' in result, false)

  const invocation = JSON.parse(readFileSync(`${output}.invocation.json`, 'utf8'))
  assert.deepEqual(invocation.args.slice(0, 3), ['--json', 'run', '--request'])
  assert.notEqual(invocation.args[3], request)
  assert.equal(existsSync(invocation.args[3]), false)
  assert.deepEqual(invocation.args.slice(4), [
    '--out-dir',
    join(realpathSync(root), 'new-run'),
    '--transport-timeout-ms',
    '60000',
  ])
  assert.equal(invocation.secret_seen, false)
  assert.equal(invocation.env_keys.includes('MDP_MCP_SECRET_MARKER'), false)
})

test('rejects output roots inside the request pack before spawning the CLI', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const cli = fixtureCli(root)
  const pack = join(root, 'pack')
  const nested = join(pack, 'nested')
  const alias = join(root, 'pack-alias')
  mkdirSync(nested, { recursive: true })
  symlinkSync(pack, alias)
  const request = join(root, 'run-request.json')
  writeFileSync(request, JSON.stringify({ contract: 'mdp.run-request.v1', pack_dir: pack }))
  const unsafe = [
    join(pack, 'direct-run'),
    join(pack, 'nested', 'nested-run'),
    join(pack, '..', 'pack', 'canonical-run'),
    join(alias, 'symlink-run'),
  ]
  const safe = join(root, 'external-scratch', 'safe-run')
  mkdirSync(dirname(safe), { recursive: true })
  const replies = await rpc(cli, [
    ...unsafe.map((output, index) => toolCall(index + 1, 'mdp_run', { request_path: request, output_dir: output })),
    toolCall(unsafe.length + 1, 'mdp_run', { request_path: request, output_dir: safe }),
  ])
  for (const [index, output] of unsafe.entries()) {
    assert.equal(replies[index].error.code, -32602)
    assert.match(replies[index].error.message, /outside the active pack|must not be a symlink/)
    assert.equal(existsSync(`${output}.invocation.json`), false)
  }
  assert.equal(replies[unsafe.length].result.isError, false)
  assert.equal(existsSync(`${safe}.invocation.json`), true)
})

test('forwards native model permission and credential only for generative requests', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const cli = fixtureCli(root)
  const deterministic = join(root, 'deterministic.json')
  const generative = join(root, 'generative.json')
  writeFileSync(deterministic, JSON.stringify({ contract: 'mdp.run-request.v1', mode: 'deterministic' }))
  writeFileSync(generative, JSON.stringify({ contract: 'mdp.run-request.v1', mode: 'generative' }))
  const requestSha256 = createHash('sha256').update(readFileSync(generative)).digest('hex')
  const expiresAt = new Date(Date.now() + 60_000).toISOString()
  const outputRoot = realpathSync(root)
  const bindingSha256 = consentBinding({ provider: 'openai', purpose: 'mdp.run', requestSha256, outputRoot, expiresAt, nonce: 'generative-nonce' })
  writeFileSync(join(root, 'gen-consent.json'), JSON.stringify({ contract: 'mdp.mcp-provider-consent.v1', provider: 'openai', purpose: 'mdp.run', request_sha256: requestSha256, source_sha256s: [], output_root: outputRoot, expires_at: expiresAt, nonce: 'generative-nonce', binding_sha256: bindingSha256 }))

  const replies = await rpc(
    cli,
    [
      toolCall(1, 'mdp_run', { request_path: deterministic, output_dir: join(root, 'deterministic-run') }),
      toolCall(2, 'mdp_run', { request_path: generative, output_dir: join(root, 'generative-run'), consent_id: 'gen-consent' }),
    ],
    { OPENAI_API_KEY: 'test-key-must-not-be-printed', MDP_ALLOW_NATIVE_MODEL_CALLS: '1' },
  )

  const deterministicInvocation = JSON.parse(readFileSync(join(root, 'deterministic-run.invocation.json'), 'utf8'))
  const generativeInvocation = JSON.parse(readFileSync(join(root, 'generative-run.invocation.json'), 'utf8'))
  assert.equal(deterministicInvocation.env_keys.includes('OPENAI_API_KEY'), false)
  assert.equal(deterministicInvocation.env_keys.includes('MDP_ALLOW_NATIVE_MODEL_CALLS'), false)
  assert.equal(generativeInvocation.env_keys.includes('OPENAI_API_KEY'), true)
  assert.equal(generativeInvocation.env_keys.includes('MDP_ALLOW_NATIVE_MODEL_CALLS'), true)
  assert.equal(deterministicInvocation.credential_canary_seen, false)
  assert.equal(generativeInvocation.credential_canary_seen, true)
  assert.equal(JSON.stringify(replies).includes('test-key-must-not-be-printed'), false)
})

test('rejects ambient or inline request arguments before spawning the CLI', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const request = join(root, 'run-request.json')
  writeFileSync(request, '{}')
  const [reply] = await rpc(fixtureCli(root), [
    toolCall(1, 'mdp_run', {
      request_path: request,
      output_dir: join(root, 'new-run'),
      request: { ambient_chat: 'do not allow' },
    }),
  ])
  assert.equal(reply.error.code, -32602)
  assert.match(reply.error.message, /pass file paths only/)
})

test('rejects request symlinks, hard links, existing output directories, and oversized request files', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const cli = fixtureCli(root)
  const request = join(root, 'request.json')
  const requestLink = join(root, 'request-link.json')
  const requestHardLink = join(root, 'request-hard-link.json')
  const cleanRequest = join(root, 'clean-request.json')
  writeFileSync(request, '{}')
  symlinkSync(request, requestLink)
  linkSync(request, requestHardLink)
  writeFileSync(cleanRequest, '{}')
  const existingOutput = join(root, 'existing')
  writeFileSync(existingOutput, 'not a directory')
  const oversized = join(root, 'oversized.json')
  writeFileSync(oversized, 'x'.repeat(1_048_577))

  const replies = await rpc(cli, [
    toolCall(1, 'mdp_run', { request_path: requestLink, output_dir: join(root, 'one') }),
    toolCall(2, 'mdp_run', { request_path: requestHardLink, output_dir: join(root, 'two') }),
    toolCall(3, 'mdp_run', { request_path: cleanRequest, output_dir: existingOutput }),
    toolCall(4, 'mdp_run', { request_path: oversized, output_dir: join(root, 'four') }),
  ])
  assert.match(replies[0].error.message, /must not be a symlink/)
  assert.match(replies[1].error.message, /exactly one hard link/)
  assert.match(replies[2].error.message, /must not already exist/)
  assert.match(replies[3].error.message, /exceeds 1048576 bytes/)
})

test('executes frozen request bytes when the public path is mutated or replaced after spawn', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-race-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const cli = fixtureCli(root)

  for (const attack of ['mutate', 'replace']) {
    const request = join(root, `${attack}.json`)
    const output = join(root, `${attack}-run`)
    const original = { contract: 'mdp.run-request.v1', mode: 'deterministic', marker: `${attack}-original` }
    writeFileSync(request, JSON.stringify(original))
    writeFileSync(`${output}.pause-before-read`, '')
    const pending = rpc(
      cli,
      [toolCall(1, 'mdp_run', { request_path: request, output_dir: output })],
      { OPENAI_API_KEY: 'must-not-cross-after-mutation', MDP_ALLOW_NATIVE_MODEL_CALLS: '1' },
    )
    await waitForFile(`${output}.ready`)
    if (attack === 'mutate') {
      writeFileSync(request, JSON.stringify({ ...original, mode: 'generative', marker: 'mutated' }))
    } else {
      const replacement = join(root, 'replacement.json')
      writeFileSync(replacement, JSON.stringify({ ...original, mode: 'generative', marker: 'replaced' }))
      renameSync(replacement, request)
    }
    writeFileSync(`${output}.continue`, '')
    const [reply] = await pending
    assert.equal(reply.result.isError, false)
    const invocation = JSON.parse(readFileSync(`${output}.invocation.json`, 'utf8'))
    assert.deepEqual(invocation.request, original)
    assert.equal(invocation.env_keys.includes('OPENAI_API_KEY'), false)
    assert.equal(invocation.env_keys.includes('MDP_ALLOW_NATIVE_MODEL_CALLS'), false)
    assert.equal(invocation.credential_canary_seen, false)
    assert.equal(JSON.stringify(reply).includes('must-not-cross-after-mutation'), false)
    assert.notEqual(invocation.args[3], request)
    assert.equal(existsSync(invocation.args[3]), false)
  }
})

test('fails closed without returning CLI stderr, partial stdout, paths, or source bodies', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const cli = fixtureCli(root)
  const failedRequest = join(root, 'failed.json')
  const invalidRequest = join(root, 'invalid.json')
  writeFileSync(failedRequest, JSON.stringify({ test_mode: 'fail' }))
  writeFileSync(invalidRequest, JSON.stringify({ test_mode: 'invalid-json' }))

  const replies = await rpc(cli, [
    toolCall(1, 'mdp_run', { request_path: failedRequest, output_dir: join(root, 'failed-run') }),
    toolCall(2, 'mdp_run', { request_path: invalidRequest, output_dir: join(root, 'invalid-run') }),
  ])
  assert.deepEqual(replies[0].result.structuredContent, {
    ok: false,
    contract: 'mdp.run-mcp-error.v1',
    code: 'cli-run-failed',
  })
  assert.equal(JSON.stringify(replies[0]).includes('PRIVATE-SOURCE-BODY'), false)
  assert.equal(JSON.stringify(replies[0]).includes('/private/customer/path'), false)
  assert.equal(replies[1].result.structuredContent.code, 'invalid-cli-output')
})

test('returns a canonical no-draft result even when the CLI exits nonzero', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const request = join(root, 'no-draft.json')
  writeFileSync(request, JSON.stringify({ test_mode: 'no-draft' }))
  const [reply] = await rpc(fixtureCli(root), [
    toolCall(1, 'mdp_run', { request_path: request, output_dir: join(root, 'no-draft-run') }),
  ])
  assert.equal(reply.result.isError, false)
  assert.equal(reply.result.structuredContent.valid, false)
  assert.equal(reply.result.structuredContent.terminal_state, 'no-draft:decision-invalid')
  assert.equal(reply.result.structuredContent.authority_block.decision.decision, 'no-draft')
  assert.equal(reply.result.structuredContent.authority.disposition, 'block')
  assert.deepEqual(reply.result.structuredContent.authority_block.diagnostics, [{
    stage: 'generative-preflight',
    gate: 'routed-context-schema',
    code: 'wrong-contract',
    input: 'routed_context',
    field: '/contract',
    expected: { kind: 'contract', value: 'mdp.routed-context.v1' },
    observed: { kind: 'contract', value: 'missing' },
  }])
})

test('returns a canonical unavailable result as data when the CLI cannot establish authority', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const request = join(root, 'unavailable.json')
  writeFileSync(request, JSON.stringify({ test_mode: 'unavailable' }))
  const [reply] = await rpc(fixtureCli(root), [
    toolCall(1, 'mdp_run', { request_path: request, output_dir: join(root, 'unavailable-run') }),
  ])
  assert.equal(reply.result.isError, false)
  assert.equal(reply.result.structuredContent.valid, false)
  assert.equal(reply.result.structuredContent.authority.authority_level, 'unavailable')
  assert.equal(reply.result.structuredContent.authority.disposition, 'undetermined')
  assert.equal(reply.result.structuredContent.authority.terminal, 'authority-unavailable')
})

test('rejects contradictory child exit status and canonical terminal state', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const request = join(root, 'contradiction.json')
  writeFileSync(request, JSON.stringify({ test_mode: 'nonzero-success' }))
  const [reply] = await rpc(fixtureCli(root), [
    toolCall(1, 'mdp_run', { request_path: request, output_dir: join(root, 'run') }),
  ])
  assert.equal(reply.result.isError, true)
  assert.equal(reply.result.structuredContent.code, 'invalid-cli-contract')
})

test('rejects a wrong CLI contract and reports spawn failure without leaking paths', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const request = join(root, 'request.json')
  writeFileSync(request, JSON.stringify({ test_mode: 'wrong-contract' }))
  const [wrongContract] = await rpc(fixtureCli(root), [
    toolCall(1, 'mdp_run', { request_path: request, output_dir: join(root, 'wrong-run') }),
  ])
  const [spawnFailure] = await rpc(join(root, 'missing-mdp'), [
    toolCall(2, 'mdp_run', { request_path: request, output_dir: join(root, 'spawn-run') }),
  ])
  assert.equal(wrongContract.result.structuredContent.code, 'invalid-cli-contract')
  assert.deepEqual(spawnFailure.result.structuredContent, {
    ok: false,
    contract: 'mdp.run-mcp-error.v1',
    code: 'cli-unavailable',
  })
  assert.equal(JSON.stringify(spawnFailure).includes(root), false)
})

test('bounds hung and overflowing children', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const cli = fixtureCli(root)
  const hang = join(root, 'hang.json')
  const overflow = join(root, 'overflow.json')
  writeFileSync(hang, JSON.stringify({ test_mode: 'hang' }))
  writeFileSync(overflow, JSON.stringify({ test_mode: 'overflow' }))
  const replies = await rpc(cli, [
    toolCall(1, 'mdp_run', { request_path: hang, output_dir: join(root, 'hang-run'), timeout_ms: 500 }),
    toolCall(2, 'mdp_run', { request_path: overflow, output_dir: join(root, 'overflow-run') }),
  ])
  assert.equal(replies[0].result.structuredContent.code, 'cli-timeout')
  assert.equal(replies[1].result.structuredContent.code, 'cli-output-limit')
})

test('keeps SIGKILL escalation alive after the child leader exits', async (t) => {
  if (process.platform === 'win32') return t.skip('Unix process-group behavior')
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const request = join(root, 'descendant.json')
  const marker = join(root, 'descendant-survived')
  writeFileSync(request, JSON.stringify({ test_mode: 'descendant', marker_path: marker }))
  const [reply] = await rpc(fixtureCli(root), [
    toolCall(1, 'mdp_run', { request_path: request, output_dir: join(root, 'run'), timeout_ms: 500 }),
  ])
  assert.equal(reply.result.structuredContent.code, 'cli-timeout')
  await new Promise((resolvePromise) => setTimeout(resolvePromise, 800))
  assert.equal(existsSync(marker), false)
})

test('interrupting the real CLI during staging removes its exact claim and private transaction', async (t) => {
  if (process.platform === 'win32') return t.skip('Unix process-group and ownership behavior')
  if (!existsSync(realCli)) return t.skip('compiled CLI is unavailable')
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-real-cleanup-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const pack = join(root, 'pack')
  cpSync(join(repoRoot, 'plugin', 'assets', 'templates', 'proposal'), pack, { recursive: true })
  const stagingFixture = join(pack, '.mdp', 'staging-fixture')
  mkdirSync(stagingFixture)
  for (let index = 0; index < 6_000; index += 1) {
    writeFileSync(join(stagingFixture, `${index.toString().padStart(4, '0')}.txt`), 'a')
  }
  const promptOutput = join(root, 'prompt-output.json')
  cpSync(join(repoRoot, 'examples', 'proposal-flow-video', 'fixtures', 'normalize-opportunity-output.json'), promptOutput)
  const requestPath = join(root, 'request.json')
  const outputDir = join(root, 'interrupted-run')
  writeFileSync(requestPath, `${JSON.stringify({
    contract: 'mdp.run-request.v1',
    execution_id: 'real-interrupted-staging',
    created_at: '2026-08-04T00:00:00Z',
    profile: 'proposal',
    operation: 'validate-existing-output',
    mode: 'deterministic',
    job_identity: null,
    pack_dir: pack,
    pack_release_id: 'real-interrupted-staging-v1',
    prompt: null,
    inputs: [{
      logical_name: 'prompt-output',
      source_path: promptOutput,
      schema_id: 'mdp.prompt-output.v0',
      media_type: 'application/json',
      provenance_refs: [],
    }],
    execution_policy: {
      environment_allowlist: [], filesystem_mode: 'private-staging', tool_mode: 'none',
      network_mode: 'none', authorized_endpoints: [], max_input_bytes: 1048576,
      max_output_bytes: 1048576, timeout_ms: 30000, retention_policy: 'receipt-only',
    },
    driver: null,
    model: null,
  })}\n`)
  const [reply] = await rpc(realCli, [
    toolCall(1, 'mdp_run', { request_path: requestPath, output_dir: outputDir, timeout_ms: 500 }),
  ])
  assert.equal(reply.result.structuredContent.code, 'cli-timeout')
  assert.equal(existsSync(outputDir), false)
  assert.equal(existsSync(join(root, '.interrupted-run.mdp-run.claim')), false)
  assert.deepEqual(
    readdirSync(root).filter((name) => name.startsWith('.interrupted-run.tmp-')),
    [],
  )
})

test('bounds JSON-RPC stdin and recovers at the next line', async (t) => {
  const root = mkdtempSync(join(tmpdir(), 'mdp-run-mcp-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const oversized = JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'ping', padding: 'x'.repeat(1_000_001) })
  const replies = await rpc(fixtureCli(root), [oversized, { jsonrpc: '2.0', id: 2, method: 'ping' }])
  assert.equal(replies[0].error.code, -32600)
  assert.match(replies[0].error.message, /exceeds 1000000 bytes/)
  assert.deepEqual(replies[1], { jsonrpc: '2.0', id: 2, result: {} })
})
