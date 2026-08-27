import { randomBytes } from 'node:crypto'
import { chmodSync, closeSync, constants, fstatSync, lstatSync, mkdtempSync, openSync, readFileSync, readdirSync, realpathSync, renameSync, rmdirSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, dirname, join, resolve } from 'node:path'

export const TEMP_WORKSPACE_CONTRACT = 'mdp.owned-temp-workspace.v1'
export const TEMP_WORKSPACE_MARKER = '.mdp-owned-temp-workspace.json'
export const DEFAULT_STALE_AGE_MS = 24 * 60 * 60 * 1000
const MAX_MARKER_BYTES = 4_096

const currentUid = () => (typeof process.getuid === 'function' ? process.getuid() : null)
const mode = (stats) => stats.mode & 0o777
const sameIdentity = (left, right) => left.dev === right.dev && left.ino === right.ino
const descriptorDirectoryPath = (fd, identity) => {
  const candidates = process.platform === 'linux'
    ? [`/proc/self/fd/${fd}`, `/proc/${process.pid}/fd/${fd}`]
    : process.platform === 'darwin' ? [`/dev/fd/${fd}`] : []
  for (const candidate of candidates) {
    try {
      if (sameIdentity(statSync(candidate), identity)) return candidate
    } catch {
      // Try the next kernel-owned descriptor namespace.
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
    rmSync(root, { recursive: true, force: true })
    throw error
  }
}

export const inspectOwnedTempWorkspace = (root, { purpose } = {}) => {
  const requested = resolve(root)
  const rootStats = lstatSync(requested)
  if (!rootStats.isDirectory() || rootStats.isSymbolicLink() || mode(rootStats) !== 0o700) return null
  const uid = currentUid()
  if (uid !== null && rootStats.uid !== uid) return null
  const markerPath = join(requested, TEMP_WORKSPACE_MARKER)
  const markerStats = lstatSync(markerPath)
  if (!markerStats.isFile() || markerStats.isSymbolicLink() || mode(markerStats) !== 0o600 || markerStats.size > MAX_MARKER_BYTES) return null
  if (uid !== null && markerStats.uid !== uid) return null
  let marker
  try { marker = JSON.parse(readFileSync(markerPath, 'utf8')) } catch { return null }
  if (
    marker?.contract !== TEMP_WORKSPACE_CONTRACT ||
    marker.basename !== basename(requested) ||
    typeof marker.purpose !== 'string' ||
    (purpose !== undefined && marker.purpose !== purpose) ||
    !Number.isSafeInteger(marker.created_at_ms) || marker.created_at_ms < 0 ||
    !Number.isSafeInteger(marker.pid) || marker.pid < 1 ||
    marker.uid !== uid
  ) return null
  return { root: requested, marker, rootStats, markerStats }
}

export const cleanupOwnedTempWorkspace = (root, options = {}) => {
  const { beforeQuarantine, ...inspectionOptions } = options
  let inspected
  try { inspected = inspectOwnedTempWorkspace(root, inspectionOptions) } catch { return false }
  if (!inspected) return false
  const parent = dirname(inspected.root)
  const quarantineLeaf = `.mdp-owned-temp-quarantine-${randomBytes(16).toString('hex')}`
  let parentFd
  try {
    parentFd = openSync(parent, constants.O_RDONLY | (constants.O_DIRECTORY || 0) | (constants.O_NOFOLLOW || 0))
    const parentStats = fstatSync(parentFd)
    if (!parentStats.isDirectory()) return false
    const parentReference = descriptorDirectoryPath(parentFd, parentStats) || parent
    const ownedPath = join(parentReference, basename(inspected.root))
    const quarantine = join(parentReference, quarantineLeaf)
    try {
      lstatSync(quarantine)
      return false
    } catch (error) {
      if (error?.code !== 'ENOENT') return false
    }
    const finalStats = lstatSync(ownedPath)
    if (!sameIdentity(finalStats, inspected.rootStats)) return false
    if (typeof beforeQuarantine === 'function') beforeQuarantine()
    if (!sameIdentity(fstatSync(parentFd), parentStats)) return false
    renameSync(ownedPath, quarantine)
    const movedStats = lstatSync(quarantine)
    if (!sameIdentity(movedStats, inspected.rootStats)) {
      try {
        lstatSync(ownedPath)
      } catch (error) {
        if (error?.code === 'ENOENT') renameSync(quarantine, ownedPath)
      }
      return false
    }
    let quarantineFd
    try {
      quarantineFd = openSync(quarantine, constants.O_RDONLY | (constants.O_DIRECTORY || 0) | (constants.O_NOFOLLOW || 0))
      const quarantineStats = fstatSync(quarantineFd)
      if (!sameIdentity(quarantineStats, inspected.rootStats)) return false
      const quarantineReference = descriptorDirectoryPath(quarantineFd, quarantineStats)
      // Without a kernel-owned descriptor path, preserve the quarantine rather
      // than fall back to a replaceable pathname for recursive deletion.
      if (!quarantineReference) {
        try {
          lstatSync(ownedPath)
        } catch (error) {
          if (error?.code === 'ENOENT') renameSync(quarantine, ownedPath)
        }
        return false
      }
      for (const entry of readdirSync(quarantineReference)) {
        rmSync(join(quarantineReference, entry), { recursive: true, force: false })
      }
      const namedStats = lstatSync(quarantine)
      if (!sameIdentity(namedStats, inspected.rootStats)) return false
      rmdirSync(quarantine)
    } finally {
      if (quarantineFd !== undefined) closeSync(quarantineFd)
    }
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
