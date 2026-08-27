import { closeSync, constants, fstatSync, openSync, statSync } from 'node:fs'

export const identityBoundDirectoryCandidates = (platform, pid, fd) => {
  if (platform === 'linux') return [`/proc/self/fd/${fd}`, `/proc/${pid}/fd/${fd}`]
  return []
}

export const resolveIdentityBoundDirectory = ({
  platform = process.platform,
  pid = process.pid,
  fd,
  identity,
}) => {
  const matchesIdentity = (current) => current.isDirectory() &&
    current.dev === BigInt(identity.dev) &&
    current.ino === BigInt(identity.ino)
  const attempts = []
  for (const candidate of identityBoundDirectoryCandidates(platform, pid, fd)) {
    try {
      const current = statSync(candidate, { bigint: true })
      if (matchesIdentity(current)) return candidate
      attempts.push('stat-identity-mismatch')
    } catch (error) {
      attempts.push(`stat-${error?.code || 'failed'}`)
      // Some descriptor filesystems do not project the target identity via stat.
    }
    let duplicate
    try {
      duplicate = openSync(candidate, constants.O_RDONLY | (constants.O_DIRECTORY || 0))
      if (matchesIdentity(fstatSync(duplicate, { bigint: true }))) return candidate
      attempts.push('open-identity-mismatch')
    } catch (error) {
      attempts.push(`open-${error?.code || 'failed'}`)
      // Try the next kernel-owned descriptor namespace, if one exists.
    } finally {
      if (duplicate !== undefined) closeSync(duplicate)
    }
  }
  throw Object.assign(
    new Error(`identity-bound output publication is unavailable on ${platform} (${attempts.join(', ') || 'no-descriptor-namespace'})`),
    { code: 'mcp-output-denied' },
  )
}
