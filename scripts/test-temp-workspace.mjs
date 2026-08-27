import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { chmodSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { cleanupOwnedTempWorkspace, cleanupStaleOwnedTempWorkspaces, createOwnedTempWorkspace, TEMP_WORKSPACE_MARKER } from './lib/temp-workspace.mjs'

const wrapper = fileURLToPath(new URL('./with-temp-workspace.mjs', import.meta.url))

const runWrapper = (base, observedPath, exitCode = 0, delayMs = 0) => {
  const environment = { ...process.env, TMPDIR: base, OBSERVED_PATH: observedPath }
  delete environment.NODE_TEST_CONTEXT
  const script = `require('fs').writeFileSync(process.env.OBSERVED_PATH, process.env.MDP_TEMP_ROOT); setTimeout(() => process.exit(${exitCode}), ${delayMs})`
  const child = spawn(process.execPath, [wrapper, '--purpose', 'validation', '--', process.execPath, '-e', script], { env: environment })
  let stderr = ''
  child.stderr.setEncoding('utf8')
  child.stderr.on('data', (chunk) => { stderr += chunk })
  return new Promise((resolve) => child.on('exit', (code) => resolve({ code, stderr })))
}

const waitForPath = async (path) => {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (existsSync(path)) return
    await new Promise((resolve) => setTimeout(resolve, 10))
  }
  throw new Error(`timed out waiting for ${path}`)
}

test('creates unpredictable private roots and cleans handled success and failure', async (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-test-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const first = createOwnedTempWorkspace({ purpose: 'validation', baseDir: base })
  const second = createOwnedTempWorkspace({ purpose: 'validation', baseDir: base })
  assert.notEqual(first, second)
  assert.equal(lstatSync(first).mode & 0o777, 0o700)
  assert.equal(lstatSync(join(first, TEMP_WORKSPACE_MARKER)).mode & 0o777, 0o600)
  assert.equal(cleanupOwnedTempWorkspace(first, { purpose: 'validation' }), true)
  assert.equal(cleanupOwnedTempWorkspace(second, { purpose: 'validation' }), true)

  for (const exitCode of [0, 7]) {
    const observedPath = join(base, `observed-${exitCode}`)
    const result = await runWrapper(base, observedPath, exitCode)
    assert.equal(result.code, exitCode, result.stderr)
    const observed = readFileSync(observedPath, 'utf8')
    assert.equal(existsSync(observed), false)
  }
})

test('concurrent validation wrappers use distinct roots and clean both', async (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-concurrent-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const observations = [join(base, 'first'), join(base, 'second')]
  const results = await Promise.all(observations.map((path) => runWrapper(base, path, 0, 100)))
  assert.deepEqual(results.map(({ code }) => code), [0, 0])
  const roots = observations.map((path) => readFileSync(path, 'utf8'))
  assert.notEqual(roots[0], roots[1])
  for (const root of roots) assert.equal(existsSync(root), false)
})

test('handled interruption forwards the signal and cleans the owned root', async (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-signal-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const observedPath = join(base, 'observed')
  const environment = { ...process.env, TMPDIR: base, OBSERVED_PATH: observedPath }
  delete environment.NODE_TEST_CONTEXT
  const script = "require('fs').writeFileSync(process.env.OBSERVED_PATH, process.env.MDP_TEMP_ROOT); setInterval(() => {}, 1000)"
  const child = spawn(process.execPath, [wrapper, '--purpose', 'validation', '--', process.execPath, '-e', script], { env: environment })
  await waitForPath(observedPath)
  const ownedRoot = readFileSync(observedPath, 'utf8')
  child.kill('SIGTERM')
  const result = await new Promise((resolve) => child.on('exit', (code, signal) => resolve({ code, signal })))
  assert.deepEqual(result, { code: null, signal: 'SIGTERM' })
  assert.equal(existsSync(ownedRoot), false)
})

test('stale cleanup requires marker, owner-safe modes, age, and a dead pid', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-stale-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const now = Date.now()
  const stale = createOwnedTempWorkspace({ purpose: 'run-mcp-freeze', baseDir: base, nowMs: now - 120_000, pid: 999_999_999 })
  const live = createOwnedTempWorkspace({ purpose: 'run-mcp-freeze', baseDir: base, nowMs: now - 120_000, pid: process.pid })
  const young = createOwnedTempWorkspace({ purpose: 'run-mcp-freeze', baseDir: base, nowMs: now - 1_000, pid: 999_999_999 })
  const badMode = createOwnedTempWorkspace({ purpose: 'run-mcp-freeze', baseDir: base, nowMs: now - 120_000, pid: 999_999_999 })
  chmodSync(badMode, 0o755)
  const badMarkerMode = createOwnedTempWorkspace({ purpose: 'run-mcp-freeze', baseDir: base, nowMs: now - 120_000, pid: 999_999_999 })
  chmodSync(join(badMarkerMode, TEMP_WORKSPACE_MARKER), 0o644)
  const stalePrepare = createOwnedTempWorkspace({ purpose: 'run-mcp-prepare', baseDir: base, nowMs: now - 120_000, pid: 999_999_999 })
  const unrelated = join(base, 'mdp-owned-run-mcp-freeze-unrelated')
  mkdirSync(unrelated, { mode: 0o700 })
  writeFileSync(join(unrelated, 'keep'), 'unrelated')
  const symlink = join(base, 'mdp-owned-run-mcp-freeze-link')
  symlinkSync(unrelated, symlink)

  const removed = cleanupStaleOwnedTempWorkspaces({ purpose: 'run-mcp-freeze', baseDir: base, minAgeMs: 60_000, nowMs: now })
  assert.deepEqual(removed, [stale])
  assert.deepEqual(
    cleanupStaleOwnedTempWorkspaces({ purpose: 'run-mcp-prepare', baseDir: base, minAgeMs: 60_000, nowMs: now }),
    [stalePrepare],
  )
  assert.equal(existsSync(stale), false)
  assert.equal(existsSync(stalePrepare), false)
  for (const path of [live, young, badMode, badMarkerMode, unrelated, symlink]) assert.equal(existsSync(path), true)
})

test('tampered marker and unrelated temp data are never removed', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-tamper-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const root = createOwnedTempWorkspace({ purpose: 'validation', baseDir: base, nowMs: 0, pid: 999_999_999 })
  const marker = join(root, TEMP_WORKSPACE_MARKER)
  const payload = JSON.parse(readFileSync(marker, 'utf8'))
  payload.basename = 'different-directory'
  writeFileSync(marker, `${JSON.stringify(payload)}\n`, { mode: 0o600 })
  const unrelated = join(base, 'ordinary-user-data')
  mkdirSync(unrelated)
  writeFileSync(join(unrelated, 'keep'), 'yes')
  cleanupStaleOwnedTempWorkspaces({ purpose: 'validation', baseDir: base, minAgeMs: 60_000, nowMs: 120_000 })
  assert.equal(existsSync(root), true)
  assert.equal(existsSync(unrelated), true)
})
