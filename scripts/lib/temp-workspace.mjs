import { randomBytes } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import { chmodSync, closeSync, constants, existsSync, fstatSync, lstatSync, mkdtempSync, openSync, readdirSync, realpathSync, statSync, utimesSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

export const TEMP_WORKSPACE_CONTRACT = 'mdp.owned-temp-workspace.v1'
export const TEMP_WORKSPACE_MARKER = '.mdp-owned-temp-workspace.json'
export const DEFAULT_STALE_AGE_MS = 24 * 60 * 60 * 1000

const currentUid = () => (typeof process.getuid === 'function' ? process.getuid() : null)
const mode = (stats) => stats.mode & 0o777
const sameIdentity = (left, right) => left.dev === right.dev && left.ino === right.ino
const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const secureHelper = (explicit) => {
  if (explicit) return explicit
  if (process.env.MDP_SECURE_INSTALL_BIN) return process.env.MDP_SECURE_INSTALL_BIN
  if (process.env.MDP_BIN) return process.env.MDP_BIN
  const developmentBinary = join(repositoryRoot, 'cli', 'target', 'debug', 'mdp')
  return existsSync(developmentBinary) ? developmentBinary : 'mdp'
}
const secureDirectoryAction = ({ action, parentFd, parentStats, name, toName, expected, helper }) => {
  const args = [
    '--json', '__secure-install', '--action', action,
    '--name', name,
    '--dir-fd', '3',
    '--expected-dev', String(parentStats.dev),
    '--expected-ino', String(parentStats.ino),
    '--expected-file-dev', String(expected.dev),
    '--expected-file-ino', String(expected.ino),
  ]
  if (toName) args.push('--to-name', toName)
  const result = spawnSync(secureHelper(helper), args, {
    stdio: ['ignore', 'pipe', 'pipe', parentFd],
    encoding: 'utf8',
    timeout: 10_000,
  })
  const failure = (reason) => ({
    ok: false,
    reason,
    status: result.status,
    signal: result.signal,
    error: result.error?.code ?? result.error?.message ?? null,
    stderr: String(result.stderr ?? '').slice(0, 2_048),
  })
  if (result.status !== 0 || result.error) return failure('process-failed')
  try {
    const envelope = JSON.parse(result.stdout)
    const expectedStatus = action === 'move-directory' ? 'moved'
      : action === 'verify-directory' ? envelope?.data?.status
        : action === 'inspect-owned-workspace' ? 'inspected'
        : 'removed'
    const ok = envelope?.ok === true &&
      envelope?.command === 'secure-install' &&
      envelope?.data?.contract === 'mdp.secure-install.v1' &&
      envelope?.data?.status === expectedStatus
    return ok ? { ok: true, status: envelope.data.status, data: envelope.data } : failure('invalid-envelope')
  } catch {
    return failure('invalid-json')
  }
}
export const resolveDescriptorDirectoryPath = ({
  fd,
  identity,
  platform = process.platform,
  pid = process.pid,
  stat = statSync,
  open = openSync,
  fstat = fstatSync,
  close = closeSync,
} = {}) => {
  const candidates = platform === 'linux'
    ? [`/proc/self/fd/${fd}`, `/proc/${pid}/fd/${fd}`]
    : platform === 'darwin' ? [`/dev/fd/${fd}`] : []
  for (const candidate of candidates) {
    try {
      if (sameIdentity(stat(candidate), identity)) return candidate
    } catch {
      // Some descriptor filesystems do not project the target identity via stat.
    }
    let duplicate
    try {
      duplicate = open(candidate, constants.O_RDONLY | (constants.O_DIRECTORY || 0))
      if (sameIdentity(fstat(duplicate), identity)) return candidate
    } catch {
      // Try the next kernel-owned descriptor namespace.
    } finally {
      if (duplicate !== undefined) close(duplicate)
    }
  }
  return null
}
const safePurpose = (purpose) => {
  if (typeof purpose !== 'string' || !/^[a-z][a-z0-9-]{0,47}$/.test(purpose)) {
    throw new Error('temp workspace purpose must use lowercase letters, digits, and hyphens')
  }
  return purpose
}

const processIsAlive = (pid) => {
  if (!Number.isSafeInteger(pid) || pid < 1) return false
  try {
    process.kill(pid, 0)
    return true
  } catch (error) {
    return error?.code === 'EPERM'
  }
}

export const createOwnedTempWorkspace = ({ purpose, baseDir = tmpdir(), nowMs = Date.now(), pid = process.pid } = {}) => {
  const normalizedPurpose = safePurpose(purpose)
  if (!Number.isSafeInteger(nowMs) || nowMs < 0) throw new Error('temp workspace creation time must be a non-negative integer')
  if (!Number.isSafeInteger(pid) || pid < 1) throw new Error('temp workspace pid must be a positive integer')
  const canonicalBase = realpathSync(resolve(baseDir))
  if (!statSync(canonicalBase).isDirectory()) throw new Error('temp workspace base must be a directory')
  const root = mkdtempSync(join(canonicalBase, `mdp-owned-${normalizedPurpose}-`))
  try {
    chmodSync(root, 0o700)
    const marker = {
      contract: TEMP_WORKSPACE_CONTRACT,
      purpose: normalizedPurpose,
      basename: basename(root),
      created_at_ms: nowMs,
      pid,
      uid: currentUid(),
    }
    const markerPath = join(root, TEMP_WORKSPACE_MARKER)
    writeFileSync(markerPath, `${JSON.stringify(marker)}\n`, {
      flag: 'wx',
      mode: 0o600,
    })
    const createdAt = new Date(nowMs)
    utimesSync(markerPath, createdAt, createdAt)
    utimesSync(root, createdAt, createdAt)
    return root
  } catch (error) {
    // Creation failures must not fall back to recursive pathname deletion.
    // If a complete ownership marker exists, the same identity-bound cleanup
    // protocol may remove it; otherwise preserve the empty/private residue for
    // conservative stale inspection.
    cleanupOwnedTempWorkspace(root, { purpose: normalizedPurpose })
    throw error
  }
}

export const inspectOwnedTempWorkspace = (root, { purpose, afterMarkerRead, secureHelperPath, secureHelperDiagnostics } = {}) => {
  const requested = resolve(root)
  let rootFd
  try {
    // Pin the candidate before consuming ownership proof. The native helper
    // opens and reads the marker with openat(O_NOFOLLOW) relative to this FD.
    rootFd = openSync(requested, constants.O_RDONLY | (constants.O_DIRECTORY || 0) | (constants.O_NOFOLLOW || 0))
    const rootStats = fstatSync(rootFd)
    const rootIdentity = fstatSync(rootFd, { bigint: true })
    if (!rootStats.isDirectory() || mode(rootStats) !== 0o700) return null
    const uid = currentUid()
    if (uid !== null && rootStats.uid !== uid) return null
    if (!sameIdentity(lstatSync(requested, { bigint: true }), rootIdentity)) return null
    const result = secureDirectoryAction({
      action: 'inspect-owned-workspace', parentFd: rootFd, parentStats: rootIdentity,
      name: TEMP_WORKSPACE_MARKER, expected: rootIdentity, helper: secureHelperPath,
    })
    if (!result.ok) {
      if (Array.isArray(secureHelperDiagnostics)) secureHelperDiagnostics.push({ action: 'inspect-owned-workspace', ...result })
      return null
    }
    const marker = result.data.marker
    const markerStats = { mtimeMs: result.data.marker_mtime_ms }
    if (typeof afterMarkerRead === 'function') afterMarkerRead({ requested })
    if (!sameIdentity(lstatSync(requested, { bigint: true }), rootIdentity)) return null
    if (
      marker?.contract !== TEMP_WORKSPACE_CONTRACT ||
      marker.basename !== basename(requested) ||
      typeof marker.purpose !== 'string' ||
      (purpose !== undefined && marker.purpose !== purpose) ||
      !Number.isSafeInteger(marker.created_at_ms) || marker.created_at_ms < 0 ||
      !Number.isSafeInteger(marker.pid) || marker.pid < 1 ||
      marker.uid !== uid
    ) return null
    return { root: requested, marker, rootStats, rootIdentity, markerStats }
  } finally {
    if (rootFd !== undefined) closeSync(rootFd)
  }
}

export const cleanupOwnedTempWorkspace = (root, options = {}) => {
  const {
    beforeQuarantine,
    beforeRestore,
    beforeRemove,
    secureHelperPath,
    secureHelperDiagnostics,
    ...inspectionOptions
  } = options
  let inspected
  try {
    inspected = inspectOwnedTempWorkspace(root, {
      ...inspectionOptions,
      secureHelperPath,
      secureHelperDiagnostics,
    })
  } catch { return false }
  if (!inspected) return false
  const parent = dirname(inspected.root)
  const quarantineLeaf = `.mdp-owned-temp-quarantine-${randomBytes(16).toString('hex')}`
  let parentFd
  try {
    parentFd = openSync(parent, constants.O_RDONLY | (constants.O_DIRECTORY || 0) | (constants.O_NOFOLLOW || 0))
    const parentStats = fstatSync(parentFd, { bigint: true })
    if (!parentStats.isDirectory()) return false
    const ownedName = basename(inspected.root)
    const ownedPath = join(parent, ownedName)
    const quarantine = join(parent, quarantineLeaf)
    const statusFor = (name) => {
      const result = secureDirectoryAction({
        action: 'verify-directory', parentFd, parentStats, name,
        expected: inspected.rootIdentity, helper: secureHelperPath,
      })
      if ((!result.ok || !['match', 'mismatch', 'absent'].includes(result.status)) && Array.isArray(secureHelperDiagnostics)) {
        secureHelperDiagnostics.push({ action: 'verify-directory', name, ...result })
      }
      return result
    }
    if (statusFor(ownedName).status !== 'match' || statusFor(quarantineLeaf).status !== 'absent') return false
    if (typeof beforeQuarantine === 'function') beforeQuarantine({ ownedPath, quarantine })
    if (!sameIdentity(fstatSync(parentFd, { bigint: true }), parentStats)) return false
    const moveResult = secureDirectoryAction({
      action: 'move-directory',
      parentFd,
      parentStats,
      name: ownedName,
      toName: quarantineLeaf,
      expected: inspected.rootIdentity,
      helper: secureHelperPath,
    })
    if (!moveResult.ok && Array.isArray(secureHelperDiagnostics)) secureHelperDiagnostics.push({ action: 'move-directory', ...moveResult })
    // A pending SIGTERM is delivered when the helper leaves its masked finite
    // transaction, so it can mutate safely yet exit before printing JSON.
    // Reconcile the identity-bound postcondition rather than trusting process
    // status alone.
    if (statusFor(ownedName).status !== 'absent' || statusFor(quarantineLeaf).status !== 'match') {
      if (typeof beforeRestore === 'function') beforeRestore({ ownedPath, quarantine })
      const restoreResult = secureDirectoryAction({
        action: 'move-directory',
        parentFd,
        parentStats,
        name: quarantineLeaf,
        toName: ownedName,
        expected: inspected.rootIdentity,
        helper: secureHelperPath,
      })
      if (!restoreResult.ok && Array.isArray(secureHelperDiagnostics)) secureHelperDiagnostics.push({ action: 'restore-directory', ...restoreResult })
      return false
    }
    if (typeof beforeRemove === 'function' && beforeRemove({ ownedPath, quarantine }) === false) {
      if (typeof beforeRestore === 'function') beforeRestore({ ownedPath, quarantine })
      const restoreResult = secureDirectoryAction({
        action: 'move-directory', parentFd, parentStats, name: quarantineLeaf,
        toName: ownedName, expected: inspected.rootIdentity, helper: secureHelperPath,
      })
      if (!restoreResult.ok && Array.isArray(secureHelperDiagnostics)) secureHelperDiagnostics.push({ action: 'restore-directory', ...restoreResult })
      return false
    }
    const removeResult = secureDirectoryAction({
      action: 'remove-directory-tree', parentFd, parentStats, name: quarantineLeaf,
      expected: inspected.rootIdentity, helper: secureHelperPath,
    })
    if (!removeResult.ok && Array.isArray(secureHelperDiagnostics)) secureHelperDiagnostics.push({ action: 'remove-directory-tree', ...removeResult })
    if (!removeResult.ok || statusFor(quarantineLeaf).status !== 'absent') return false
    return true
  } catch {
    return false
  } finally {
    if (parentFd !== undefined) closeSync(parentFd)
  }
}

export const cleanupStaleOwnedTempWorkspaces = ({
  purpose,
  baseDir = tmpdir(),
  minAgeMs = DEFAULT_STALE_AGE_MS,
  nowMs = Date.now(),
} = {}) => {
  const normalizedPurpose = safePurpose(purpose)
  if (!Number.isSafeInteger(minAgeMs) || minAgeMs < 60_000) {
    throw new Error('stale cleanup minimum age must be at least 60000ms')
  }
  const canonicalBase = realpathSync(resolve(baseDir))
  const prefix = `mdp-owned-${normalizedPurpose}-`
  const removed = []
  for (const entry of readdirSync(canonicalBase, { withFileTypes: true })) {
    if (!entry.name.startsWith(prefix) || !entry.isDirectory() || entry.isSymbolicLink()) continue
    const candidate = join(canonicalBase, entry.name)
    let inspected
    try { inspected = inspectOwnedTempWorkspace(candidate, { purpose: normalizedPurpose }) } catch { continue }
    const newestIdentityTime = inspected
      ? Math.max(inspected.marker.created_at_ms, inspected.rootStats.mtimeMs, inspected.markerStats.mtimeMs)
      : nowMs
    if (!inspected || nowMs - newestIdentityTime < minAgeMs || processIsAlive(inspected.marker.pid)) continue
    if (cleanupOwnedTempWorkspace(candidate, { purpose: normalizedPurpose })) removed.push(candidate)
  }
  return removed
}
