import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { chmodSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { cleanupOwnedTempWorkspace, cleanupStaleOwnedTempWorkspaces, createOwnedTempWorkspace, resolveDescriptorDirectoryPath, TEMP_WORKSPACE_MARKER } from './lib/temp-workspace.mjs'

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
  const diagnostics = []
  assert.equal(cleanupOwnedTempWorkspace(first, {
    purpose: 'validation',
    secureHelperDiagnostics: diagnostics,
  }), true, JSON.stringify(diagnostics))
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

test('wrapper preserves GNU Make jobserver descriptors', async (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-jobserver-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const observedPath = join(base, 'observed')
  const environment = {
    ...process.env,
    MAKEFLAGS: '--jobserver-auth=3,4 -j',
    TMPDIR: base,
    OBSERVED_PATH: observedPath,
  }
  delete environment.NODE_TEST_CONTEXT
  const script = "const fs=require('fs'); fs.fstatSync(3); fs.fstatSync(4); fs.writeFileSync(process.env.OBSERVED_PATH, process.env.MDP_TEMP_ROOT)"
  const child = spawn(
    process.execPath,
    [wrapper, '--purpose', 'validation', '--', process.execPath, '-e', script],
    { env: environment, stdio: ['ignore', 'ignore', 'pipe', 'pipe', 'pipe'] },
  )
  let stderr = ''
  child.stderr.setEncoding('utf8')
  child.stderr.on('data', (chunk) => { stderr += chunk })
  const code = await new Promise((resolve) => child.on('exit', resolve))
  assert.equal(code, 0, stderr)
  assert.equal(existsSync(readFileSync(observedPath, 'utf8')), false)
})

test('wrapper supports a temporary base path containing spaces', async (t) => {
  const parent = mkdtempSync(join(tmpdir(), 'mdp-temp-spaces-'))
  const base = join(parent, 'temporary workspace base')
  mkdirSync(base)
  t.after(() => rmSync(parent, { recursive: true, force: true }))
  const observedPath = join(parent, 'observed')
  const result = await runWrapper(base, observedPath)
  assert.equal(result.code, 0, result.stderr)
  const observed = readFileSync(observedPath, 'utf8')
  assert.ok(observed.startsWith(`${realpathSync(base)}/mdp-owned-validation-`))
  assert.equal(existsSync(observed), false)
})

test('macOS descriptor resolver verifies an opened directory duplicate', () => {
  const closed = []
  const identity = { dev: 7, ino: 11 }
  const resolved = resolveDescriptorDirectoryPath({
    fd: 9,
    identity,
    platform: 'darwin',
    stat: () => ({ dev: 1, ino: 2 }),
    open: (path) => {
      assert.equal(path, '/dev/fd/9')
      return 13
    },
    fstat: (fd) => {
      assert.equal(fd, 13)
      return identity
    },
    close: (fd) => closed.push(fd),
  })
  assert.equal(resolved, '/dev/fd/9')
  assert.deepEqual(closed, [13])
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

test('root replacement during cleanup is preserved without unrelated deletion', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-root-swap-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const root = createOwnedTempWorkspace({ purpose: 'validation', baseDir: base })
  writeFileSync(join(root, 'owned'), 'owned bytes')
  const displaced = join(base, 'displaced-owned-root')
  const replacement = join(base, 'replacement')
  mkdirSync(replacement, { mode: 0o700 })
  writeFileSync(join(replacement, 'keep'), 'replacement bytes')

  const removed = cleanupOwnedTempWorkspace(root, {
    purpose: 'validation',
    beforeQuarantine: () => {
      renameSync(root, displaced)
      renameSync(replacement, root)
    },
  })

  assert.equal(removed, false)
  assert.equal(readFileSync(join(root, 'keep'), 'utf8'), 'replacement bytes')
  assert.equal(readFileSync(join(displaced, 'owned'), 'utf8'), 'owned bytes')
  assert.equal(
    readdirSync(base).some((entry) => entry.startsWith('.mdp-owned-temp-quarantine-')),
    false,
  )
})

test('inspection-window root replacement cannot reuse the pinned marker proof', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-inspection-swap-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const root = createOwnedTempWorkspace({ purpose: 'validation', baseDir: base })
  const original = `${root}-original`
  writeFileSync(join(root, 'owned'), 'owned bytes')

  assert.equal(cleanupOwnedTempWorkspace(root, {
    purpose: 'validation',
    afterMarkerRead: () => {
      renameSync(root, original)
      mkdirSync(root, { mode: 0o700 })
      writeFileSync(join(root, 'keep'), 'replacement bytes')
    },
  }), false)
  assert.equal(readFileSync(join(root, 'keep'), 'utf8'), 'replacement bytes')
  assert.equal(readFileSync(join(original, 'owned'), 'utf8'), 'owned bytes')
})

test('quarantine destination collision preserves both unrelated and owned bytes', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-quarantine-collision-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const root = createOwnedTempWorkspace({ purpose: 'validation', baseDir: base })
  writeFileSync(join(root, 'owned'), 'owned bytes')
  let collision

  const removed = cleanupOwnedTempWorkspace(root, {
    purpose: 'validation',
    beforeQuarantine: ({ quarantine }) => {
      collision = quarantine
      mkdirSync(quarantine, { mode: 0o700 })
      writeFileSync(join(quarantine, 'keep'), 'unrelated quarantine bytes')
    },
  })

  assert.equal(removed, false)
  assert.equal(readFileSync(join(root, 'owned'), 'utf8'), 'owned bytes')
  assert.equal(
    readFileSync(join(base, basename(collision), 'keep'), 'utf8'),
    'unrelated quarantine bytes',
  )
})

test('missing or no-op secure helper fails closed without claiming cleanup', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-helper-fail-closed-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const previous = process.env.MDP_BIN
  t.after(() => {
    if (previous === undefined) delete process.env.MDP_BIN
    else process.env.MDP_BIN = previous
  })

  const missingRoot = createOwnedTempWorkspace({ purpose: 'validation', baseDir: base })
  writeFileSync(join(missingRoot, 'owned'), 'owned bytes')
  process.env.MDP_BIN = join(base, 'missing-mdp')
  assert.equal(cleanupOwnedTempWorkspace(missingRoot, { purpose: 'validation' }), false)
  assert.equal(readFileSync(join(missingRoot, 'owned'), 'utf8'), 'owned bytes')

  const noOp = join(base, 'no-op-mdp')
  writeFileSync(
    noOp,
    '#!/bin/sh\nprintf \'%s\\n\' \'{"ok":true,"command":"secure-install","data":{"contract":"mdp.secure-install.v1","status":"moved"}}\'\n',
    { mode: 0o700 },
  )
  process.env.MDP_BIN = noOp
  const noOpRoot = createOwnedTempWorkspace({ purpose: 'validation', baseDir: base })
  writeFileSync(join(noOpRoot, 'owned'), 'owned bytes')
  assert.equal(cleanupOwnedTempWorkspace(noOpRoot, { purpose: 'validation' }), false)
  assert.equal(readFileSync(join(noOpRoot, 'owned'), 'utf8'), 'owned bytes')
})

test('dedicated secure helper takes precedence over the operational CLI fixture', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-helper-precedence-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const previous = {
    mdp: process.env.MDP_BIN,
    secure: process.env.MDP_SECURE_INSTALL_BIN,
    marker: process.env.MDP_FAKE_SPAWN_MARKER,
  }
  t.after(() => {
    for (const [name, value] of [
      ['MDP_BIN', previous.mdp],
      ['MDP_SECURE_INSTALL_BIN', previous.secure],
      ['MDP_FAKE_SPAWN_MARKER', previous.marker],
    ]) {
      if (value === undefined) delete process.env[name]
      else process.env[name] = value
    }
  })
  const marker = join(base, 'fixture-spawned')
  const fixture = join(base, 'operational-fixture')
  writeFileSync(fixture, '#!/bin/sh\ntouch "$MDP_FAKE_SPAWN_MARKER"\nexit 9\n', { mode: 0o700 })
  process.env.MDP_BIN = fixture
  process.env.MDP_SECURE_INSTALL_BIN = fileURLToPath(new URL('../cli/target/debug/mdp', import.meta.url))
  process.env.MDP_FAKE_SPAWN_MARKER = marker
  const root = createOwnedTempWorkspace({ purpose: 'validation', baseDir: base })
  writeFileSync(join(root, 'owned'), 'owned bytes')

  assert.equal(cleanupOwnedTempWorkspace(root, { purpose: 'validation' }), true)
  assert.equal(existsSync(root), false)
  assert.equal(existsSync(marker), false)
})

test('helper termination after mutation is reconciled from exact identities', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-helper-signal-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const helper = join(base, 'mutate-then-signal')
  writeFileSync(helper, `#!/usr/bin/env node
const fs = require('node:fs')
const path = require('node:path')
const args = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, index, all) => {
  if (value.startsWith('--') && all[index + 1] && !all[index + 1].startsWith('--')) pairs.push([value.slice(2), all[index + 1]])
  return pairs
}, []))
const parent = ${JSON.stringify(base)}
const target = path.join(parent, args.name)
if (args.action === 'inspect-owned-workspace') {
  const marker = JSON.parse(fs.readFileSync(target, 'utf8'))
  const marker_mtime_ms = fs.statSync(target).mtimeMs
  fs.writeSync(1, JSON.stringify({ ok: true, command: 'secure-install', data: { contract: 'mdp.secure-install.v1', status: 'inspected', marker, marker_mtime_ms } }) + '\\n')
  process.exit(0)
}
if (args.action === 'verify-directory') {
  let status = 'absent'
  try {
    const stats = fs.lstatSync(target, { bigint: true })
    status = String(stats.dev) === args['expected-file-dev'] && String(stats.ino) === args['expected-file-ino'] ? 'match' : 'mismatch'
  } catch (error) { if (error.code !== 'ENOENT') throw error }
  fs.writeSync(1, JSON.stringify({ ok: true, command: 'secure-install', data: { contract: 'mdp.secure-install.v1', status } }) + '\\n')
  process.exit(0)
}
if (args.action === 'move-directory') fs.renameSync(target, path.join(parent, args['to-name']))
else if (args.action === 'remove-directory-tree') fs.rmSync(target, { recursive: true })
else process.exit(2)
process.kill(process.pid, 'SIGTERM')
setTimeout(() => {}, 1_000)
`, { mode: 0o700 })
  chmodSync(helper, 0o700)
  const root = createOwnedTempWorkspace({ purpose: 'validation', baseDir: base })
  writeFileSync(join(root, 'owned'), 'owned bytes')

  const diagnostics = []
  assert.equal(cleanupOwnedTempWorkspace(root, {
    purpose: 'validation',
    secureHelperPath: helper,
    secureHelperDiagnostics: diagnostics,
  }), false)
  assert.equal(diagnostics.length > 0, true, JSON.stringify(diagnostics))
  if (existsSync(root)) assert.equal(readFileSync(join(root, 'owned'), 'utf8'), 'owned bytes')
  assert.equal(readdirSync(base).some((name) => name.includes('quarantine')), false)
})

test('restore destination collision preserves replacement and quarantined original', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-restore-collision-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const root = createOwnedTempWorkspace({ purpose: 'validation', baseDir: base })
  writeFileSync(join(root, 'owned'), 'owned bytes')
  let quarantine

  const removed = cleanupOwnedTempWorkspace(root, {
    purpose: 'validation',
    beforeRemove: () => false,
    beforeRestore: (paths) => {
      quarantine = paths.quarantine
      mkdirSync(paths.ownedPath, { mode: 0o700 })
      writeFileSync(join(paths.ownedPath, 'keep'), 'concurrent replacement bytes')
    },
  })

  assert.equal(removed, false)
  assert.equal(readFileSync(join(root, 'keep'), 'utf8'), 'concurrent replacement bytes')
  assert.equal(readFileSync(join(base, basename(quarantine), 'owned'), 'utf8'), 'owned bytes')
})
