import assert from 'node:assert/strict'
import { spawn, spawnSync } from 'node:child_process'
import { chmodSync, existsSync, linkSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, renameSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'
import { cleanupOwnedTempWorkspace, cleanupStaleOwnedTempWorkspaces, createOwnedTempWorkspace, resolveDescriptorDirectoryPath, TEMP_WORKSPACE_MARKER } from './lib/temp-workspace.mjs'

const wrapper = fileURLToPath(new URL('./with-temp-workspace.mjs', import.meta.url))
const repositoryRoot = fileURLToPath(new URL('..', import.meta.url))

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

test('creation normalizes private modes under a restrictive umask', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-umask-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const previous = process.umask(0o777)
  let root
  try {
    root = createOwnedTempWorkspace({ purpose: 'validation', baseDir: base })
  } finally {
    process.umask(previous)
  }
  assert.equal(lstatSync(root).mode & 0o777, 0o700)
  assert.equal(lstatSync(join(root, TEMP_WORKSPACE_MARKER)).mode & 0o777, 0o600)
  assert.equal(cleanupOwnedTempWorkspace(root, { purpose: 'validation' }), true)
})

test('wrapper refuses child success when owned-root cleanup is incomplete', async (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-wrapper-cleanup-refusal-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const helper = join(base, 'refuse-cleanup')
  writeFileSync(helper, '#!/bin/sh\nexit 73\n', { mode: 0o700 })
  const observedPath = join(base, 'observed')
  const environment = {
    ...process.env,
    TMPDIR: base,
    OBSERVED_PATH: observedPath,
    MDP_SECURE_INSTALL_BIN: helper,
  }
  delete environment.NODE_TEST_CONTEXT
  const script = "require('fs').writeFileSync(process.env.OBSERVED_PATH, process.env.MDP_TEMP_ROOT)"
  const child = spawn(
    process.execPath,
    [wrapper, '--purpose', 'validation', '--', process.execPath, '-e', script],
    { env: environment },
  )
  let stderr = ''
  child.stderr.setEncoding('utf8')
  child.stderr.on('data', (chunk) => { stderr += chunk })
  const code = await new Promise((resolve) => child.on('close', resolve))
  assert.equal(code, 74)
  assert.equal(stderr, 'temporary workspace cleanup incomplete\n')
  const ownedRoot = readFileSync(observedPath, 'utf8')
  assert.equal(existsSync(ownedRoot), true)
  assert.doesNotMatch(stderr, /mdp-owned|request|private/i)
})

test('validation wrapper executes nested CLI checks from a Cargo-configured build target', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-custom-cargo-target-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const target = join(base, 'target')
  const buildTarget = 'test-host-triple'
  const debug = join(target, buildTarget, 'debug')
  mkdirSync(debug, { recursive: true })
  const probe = join(base, 'invoked')
  const realCli = fileURLToPath(new URL('../cli/target/debug/mdp', import.meta.url))
  const customCli = join(debug, 'mdp')
  writeFileSync(customCli, `#!/bin/sh
printf invoked >>"${probe}"
exec "${realCli}" "$@"
`, { mode: 0o700 })
  const cargo = join(base, 'cargo')
  const cargoHome = join(base, 'cargo-home')
  mkdirSync(cargoHome)
  writeFileSync(join(cargoHome, 'config.toml'), `[build]\ntarget = "${buildTarget}"\n`)
  writeFileSync(cargo, `#!/bin/sh
grep -q 'target = "${buildTarget}"' "$CARGO_HOME/config.toml" || exit 64
if [ "$1" = build ]; then
  printf '%s\\n' '${JSON.stringify({ reason: 'compiler-artifact', target: { name: 'mdp', kind: ['bin'] }, executable: customCli })}'
fi
exit 0
`, { mode: 0o700 })
  const environment = {
    ...process.env,
    CARGO: cargo,
    CARGO_HOME: cargoHome,
    TMPDIR: base,
  }
  delete environment.MDP_BIN
  delete environment.MDP_SECURE_INSTALL_BIN
  delete environment.MDP_TEMP_WORKSPACE_ACTIVE
  delete environment.NODE_TEST_CONTEXT
  const result = spawnSync('make', [
    '--no-print-directory',
    '--warn-undefined-variables',
    'validate-cold-model-conformance',
  ], {
    cwd: repositoryRoot,
    encoding: 'utf8',
    env: environment,
    timeout: 120_000,
  })
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`)
  assert.equal(readFileSync(probe, 'utf8').includes('invoked'), true)
})

test('validation wrapper skips work for Make dry-run, question, and touch modes', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-make-dry-run-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const spawned = join(base, 'helper-spawned')
  const helper = join(base, 'must-not-spawn')
  writeFileSync(helper, `#!/bin/sh
touch "${spawned}"
exit 73
`, { mode: 0o700 })
  const environment = {
    ...process.env,
    TMPDIR: base,
    MDP_SECURE_INSTALL_BIN: helper,
  }
  delete environment.MDP_BIN
  delete environment.MDP_TEMP_WORKSPACE_ACTIVE
  delete environment.NODE_TEST_CONTEXT
  for (const [mode, expectedStatus] of [['-n', 0], ['-q', 1], ['-t', 0]]) {
    const result = spawnSync('make', [mode, 'validate-version-sync'], {
      cwd: repositoryRoot,
      encoding: 'utf8',
      env: environment,
    })
    assert.equal(result.status, expectedStatus, `${mode}\n${result.stdout}\n${result.stderr}`)
  }
  assert.equal(existsSync(spawned), false)
  assert.equal(readdirSync(base).some((name) => name.startsWith('mdp-owned-validation-')), false)
})

test('validation wrapper stops when the helper build produces no executable', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-make-helper-build-failure-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const environment = { ...process.env, TMPDIR: base }
  delete environment.MDP_BIN
  delete environment.MDP_SECURE_INSTALL_BIN
  delete environment.MDP_TEMP_WORKSPACE_ACTIVE
  delete environment.NODE_TEST_CONTEXT
  const result = spawnSync('make', ['CARGO=false', 'validate-llms'], {
    cwd: repositoryRoot,
    encoding: 'utf8',
    env: environment,
    timeout: 30_000,
  })
  assert.notEqual(result.status, 0, `${result.stdout}\n${result.stderr}`)
  assert.equal(readdirSync(base).some((name) => name.startsWith('mdp-owned-validation-')), false)
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

test('stale sweep recovers an owned root interrupted after quarantine publication', async (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-quarantine-recovery-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const realCli = fileURLToPath(new URL('../cli/target/debug/mdp', import.meta.url))
  const helper = join(base, 'kill-after-move')
  writeFileSync(helper, `#!/bin/sh
"${realCli}" "$@"
status=$?
case " $* " in
  *" --action move-directory "*) if [ "$status" -eq 0 ]; then kill -KILL "$PPID"; fi ;;
esac
exit "$status"
`, { mode: 0o700 })
  const root = createOwnedTempWorkspace({
    purpose: 'validation',
    baseDir: base,
    nowMs: 0,
    pid: 999_999_999,
  })
  writeFileSync(join(root, 'private'), 'private bytes')
  const environment = { ...process.env }
  delete environment.NODE_TEST_CONTEXT
  const child = spawn(process.execPath, [
    '--input-type=module',
    '-e',
    `import { cleanupOwnedTempWorkspace } from ${JSON.stringify(new URL('./lib/temp-workspace.mjs', import.meta.url).href)}; cleanupOwnedTempWorkspace(${JSON.stringify(root)}, { purpose: 'validation', secureHelperPath: ${JSON.stringify(helper)} })`,
  ], { env: environment })
  const result = await new Promise((resolve) => child.on('exit', (code, signal) => resolve({ code, signal })))
  assert.equal(result.signal, 'SIGKILL')
  assert.equal(existsSync(root), false)
  const quarantine = readdirSync(base).find((name) => name.startsWith('.mdp-owned-temp-quarantine-'))
  assert.ok(quarantine)
  assert.equal(readFileSync(join(base, quarantine, 'private'), 'utf8'), 'private bytes')

  const removed = cleanupStaleOwnedTempWorkspaces({
    purpose: 'validation',
    baseDir: base,
    minAgeMs: 60_000,
    nowMs: Date.now() + 120_000,
    secureHelperPath: realCli,
  })
  assert.deepEqual(removed, [join(realpathSync(base), quarantine)])
  assert.equal(existsSync(join(base, quarantine)), false)
})

test('stale sweep accepts the recoverable marker rename left by native interruption', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-marker-recovery-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const root = createOwnedTempWorkspace({
    purpose: 'validation',
    baseDir: base,
    nowMs: 0,
    pid: 999_999_999,
  })
  writeFileSync(join(root, 'private'), 'private bytes')
  const quarantine = join(
    realpathSync(base),
    `.mdp-owned-temp-quarantine-${basename(root)}-${'2'.repeat(32)}`,
  )
  renameSync(root, quarantine)
  renameSync(
    join(quarantine, TEMP_WORKSPACE_MARKER),
    join(quarantine, `.mdp-owned-temp-workspace.json.quarantine-${'3'.repeat(32)}`),
  )

  assert.deepEqual(cleanupStaleOwnedTempWorkspaces({
    purpose: 'validation',
    baseDir: base,
    minAgeMs: 60_000,
    nowMs: Date.now() + 120_000,
  }), [quarantine])
  assert.equal(existsSync(quarantine), false)
})

test('stale sweep reclaims a SIGKILL terminal record bound to the exact quarantine inode', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-terminal-record-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const root = createOwnedTempWorkspace({
    purpose: 'validation', baseDir: base, nowMs: 0, pid: 999_999_999,
  })
  writeFileSync(join(root, 'private'), 'private bytes')
  const quarantineName = `.mdp-owned-temp-quarantine-${basename(root)}-${'4'.repeat(32)}`
  const quarantine = join(realpathSync(base), quarantineName)
  renameSync(root, quarantine)
  const identity = lstatSync(quarantine, { bigint: true })
  const recordName = `.mdp-owned-temp-recovery-${quarantineName}-${identity.dev.toString(16)}-${identity.ino.toString(16)}-${'5'.repeat(32)}`
  linkSync(join(quarantine, TEMP_WORKSPACE_MARKER), join(realpathSync(base), recordName))
  unlinkSync(join(quarantine, TEMP_WORKSPACE_MARKER))
  const ordinary = createOwnedTempWorkspace({
    purpose: 'validation', baseDir: base, nowMs: 0, pid: 999_999_999,
  })
  writeFileSync(join(ordinary, 'private'), 'ordinary private bytes')
  const unknownDirents = (path) => readdirSync(path, { withFileTypes: true }).map((entry) => ({
    name: entry.name,
    isFile: () => false,
    isDirectory: () => false,
    isSymbolicLink: () => false,
  }))

  assert.deepEqual(cleanupStaleOwnedTempWorkspaces({
    purpose: 'validation', baseDir: base, minAgeMs: 60_000,
    nowMs: Date.now() + 120_000,
    readDirectory: unknownDirents,
  }).sort(), [quarantine, ordinary].sort())
  assert.equal(existsSync(quarantine), false)
  assert.equal(existsSync(ordinary), false)
  assert.equal(existsSync(join(realpathSync(base), recordName)), false)
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

test('stale sweep skips young roots before spawning the secure inspector', (t) => {
  const base = mkdtempSync(join(tmpdir(), 'mdp-temp-young-prefilter-'))
  t.after(() => rmSync(base, { recursive: true, force: true }))
  const now = Date.now()
  const young = createOwnedTempWorkspace({
    purpose: 'run-mcp-freeze',
    baseDir: base,
    nowMs: now - 1_000,
    pid: 999_999_999,
  })
  const spawned = join(base, 'inspector-spawned')
  const helper = join(base, 'must-not-spawn')
  writeFileSync(helper, `#!/bin/sh\ntouch '${spawned}'\nexit 73\n`, { mode: 0o700 })

  assert.deepEqual(cleanupStaleOwnedTempWorkspaces({
    purpose: 'run-mcp-freeze',
    baseDir: base,
    minAgeMs: 60_000,
    nowMs: now,
    secureHelperPath: helper,
  }), [])
  assert.equal(existsSync(young), true)
  assert.equal(existsSync(spawned), false)
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
