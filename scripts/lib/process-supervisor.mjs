import { spawn } from 'node:child_process'
import {
  closeSync,
  constants,
  existsSync,
  fstatSync,
  lstatSync,
  openSync,
  readFileSync,
  realpathSync,
  rmSync,
  unlinkSync,
} from 'node:fs'
import { basename, dirname, join, resolve } from 'node:path'

const DEFAULT_GRACE_MS = 250
const MAX_RECOVERY_CLAIM_BYTES = 512
const EXECUTION_ID_PATTERN = /^[A-Za-z0-9_-]{1,128}$/

const sameFile = (left, right) => left.dev === right.dev && left.ino === right.ino
const ownedByCurrentUser = (stats) =>
  typeof process.getuid !== 'function' || stats.uid === process.getuid()
const singleLink = (stats) => typeof stats.nlink !== 'number' || stats.nlink === 1
const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

const readRecoveryClaim = (claimPath) => {
  let descriptor
  try {
    const before = lstatSync(claimPath)
    if (!before.isFile() || before.isSymbolicLink() || !singleLink(before) || !ownedByCurrentUser(before)) return null
    if (before.size <= 0 || before.size > MAX_RECOVERY_CLAIM_BYTES) return null
    descriptor = openSync(claimPath, constants.O_RDONLY | (constants.O_NOFOLLOW || 0))
    const opened = fstatSync(descriptor)
    if (!opened.isFile() || !singleLink(opened) || !ownedByCurrentUser(opened) || !sameFile(before, opened)) return null
    const bytes = readFileSync(descriptor)
    if (bytes.length <= 0 || bytes.length > MAX_RECOVERY_CLAIM_BYTES) return null
    return { value: JSON.parse(bytes.toString('utf8')), stats: opened }
  } catch {
    return null
  } finally {
    if (descriptor !== undefined) closeSync(descriptor)
  }
}

export const cleanupMdpRecoveryClaim = ({ outputDir, executionId }) => {
  if (!EXECUTION_ID_PATTERN.test(executionId || '')) return false
  const requestedOutput = resolve(outputDir)
  const outputLeaf = basename(requestedOutput)
  const requestedParent = dirname(requestedOutput)
  if (!outputLeaf || outputLeaf === '.' || outputLeaf === '..' || !existsSync(requestedParent)) return false

  let parent
  try {
    const parentStats = lstatSync(requestedParent)
    if (!parentStats.isDirectory() || parentStats.isSymbolicLink() || !ownedByCurrentUser(parentStats)) return false
    parent = realpathSync(requestedParent)
  } catch {
    return false
  }
  if (dirname(join(parent, outputLeaf)) !== parent) return false

  const claimPath = join(parent, `.${outputLeaf}.mdp-run.claim`)
  const claim = readRecoveryClaim(claimPath)
  if (!claim) return false
  const value = claim.value
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false
  if (Object.keys(value).sort().join(',') !== 'contract,execution_id,transaction_leaf') return false
  if (value.contract !== 'mdp.run-recovery-claim.v1' || value.execution_id !== executionId) return false
  if (!EXECUTION_ID_PATTERN.test(value.execution_id)) return false
  const transactionPattern = new RegExp(`^\\.${escapeRegExp(outputLeaf)}\\.tmp-[0-9a-f]{32}$`)
  if (
    typeof value.transaction_leaf !== 'string' ||
    basename(value.transaction_leaf) !== value.transaction_leaf ||
    value.transaction_leaf.includes('/') ||
    value.transaction_leaf.includes('\\') ||
    !transactionPattern.test(value.transaction_leaf)
  ) return false

  const transactionPath = join(parent, value.transaction_leaf)
  let transactionStats
  try {
    transactionStats = lstatSync(transactionPath)
    if (!transactionStats.isDirectory() || transactionStats.isSymbolicLink() || !ownedByCurrentUser(transactionStats)) return false
    const canonicalTransaction = realpathSync(transactionPath)
    if (dirname(canonicalTransaction) !== parent || basename(canonicalTransaction) !== value.transaction_leaf) return false
    const beforeRemoval = lstatSync(transactionPath)
    if (!sameFile(transactionStats, beforeRemoval) || !beforeRemoval.isDirectory() || beforeRemoval.isSymbolicLink()) return false
  } catch {
    return false
  }

  try {
    rmSync(transactionPath, { recursive: true, force: false })
    const claimBeforeUnlink = lstatSync(claimPath)
    if (!claimBeforeUnlink.isFile() || !singleLink(claimBeforeUnlink) || !sameFile(claim.stats, claimBeforeUnlink)) return false
    unlinkSync(claimPath)
    return true
  } catch {
    return false
  }
}

const terminateProcessGroup = (child, processGroupId, signal) => {
  if (!processGroupId) return
  try {
    if (process.platform === 'win32') child.kill(signal)
    else process.kill(-processGroupId, signal)
  } catch (error) {
    if (error?.code !== 'ESRCH') {
      try {
        child.kill(signal)
      } catch {
        // The process already exited.
      }
    }
  }
}

const groupIsClosed = (processGroupId) => {
  if (!processGroupId || process.platform === 'win32') return true
  try {
    process.kill(-processGroupId, 0)
    return false
  } catch (error) {
    return error?.code === 'ESRCH'
  }
}

const waitForClosedGroup = async (processGroupId, attempts = 40) => {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    if (groupIsClosed(processGroupId)) return true
    await new Promise((resolveWait) => setTimeout(resolveWait, 25))
  }
  return groupIsClosed(processGroupId)
}

export const superviseProcess = ({
  command,
  args = [],
  cwd,
  environment,
  timeoutMs,
  maxOutputBytes,
  terminationGraceMs = DEFAULT_GRACE_MS,
  recovery = null,
}) =>
  new Promise((resolveResult) => {
    const startedAt = performance.now()
    const stdoutChunks = []
    const stderrChunks = []
    let outputBytes = 0
    let overflowed = false
    let timedOut = false
    let spawnFailed = false
    let escalationPromise = null
    let finishRequested = false
    const child = spawn(command[0], [...command.slice(1), ...args], {
      cwd,
      detached: process.platform !== 'win32',
      env: environment,
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    const processGroupId = child.pid

    const escalate = () => {
      if (escalationPromise) return escalationPromise
      terminateProcessGroup(child, processGroupId, 'SIGTERM')
      escalationPromise = new Promise((resolveEscalation) => {
        setTimeout(async () => {
          terminateProcessGroup(child, processGroupId, 'SIGKILL')
          const processGroupClosed = await waitForClosedGroup(processGroupId)
          const recovered = processGroupClosed && recovery
            ? cleanupMdpRecoveryClaim(recovery)
            : false
          resolveEscalation({ processGroupClosed, recovered })
        }, terminationGraceMs)
      })
      return escalationPromise
    }

    const collect = (target) => (chunk) => {
      outputBytes += chunk.length
      if (outputBytes > maxOutputBytes) {
        overflowed = true
        escalate()
        return
      }
      target.push(chunk)
    }
    child.stdout.on('data', collect(stdoutChunks))
    child.stderr.on('data', collect(stderrChunks))
    child.on('error', () => {
      spawnFailed = true
    })
    const timeout = setTimeout(() => {
      timedOut = true
      escalate()
    }, timeoutMs)

    child.on('close', (code, signal) => {
      if (finishRequested) return
      finishRequested = true
      clearTimeout(timeout)
      const finish = (termination = { processGroupClosed: true, recovered: false }) =>
        resolveResult({
          status: timedOut ? 124 : overflowed || spawnFailed ? 1 : (code ?? 1),
          signal,
          stdout: Buffer.concat(stdoutChunks).toString('utf8'),
          stderr: Buffer.concat(stderrChunks).toString('utf8'),
          timedOut,
          overflowed,
          spawnFailed,
          ...termination,
          deadline: timedOut
            ? {
                contract: 'mdp.deadline-observation.v1',
                outcome: 'timed-out',
                phase: 'transport',
                elapsed_ms: Math.min(timeoutMs, Math.max(0, Math.round(performance.now() - startedAt))),
                configured_limit_ms: timeoutMs,
                effective_limit_ms: timeoutMs,
                transport_configured_ms: timeoutMs,
                runtime_configured_ms: timeoutMs,
                provider_configured_ms: timeoutMs,
                finalization_reserve_ms: 250,
                terminal_state: 'no-draft:runner-failed',
                warnings: [],
              }
            : null,
        })
      if (escalationPromise) escalationPromise.then(finish)
      else finish()
    })
  })
