import { statSync } from 'node:fs'

export const identityBoundDirectoryCandidates = (platform, pid, fd) => {
  if (platform === 'linux') return [`/proc/self/fd/${fd}`, `/proc/${pid}/fd/${fd}`]
  if (platform === 'darwin') return [`/dev/fd/${fd}`]
  return []
}

export const resolveIdentityBoundDirectory = ({
  platform = process.platform,
  pid = process.pid,
  fd,
  identity,
}) => {
  for (const candidate of identityBoundDirectoryCandidates(platform, pid, fd)) {
    try {
      const current = statSync(candidate, { bigint: true })
      if (
        current.isDirectory() &&
        current.dev === BigInt(identity.dev) &&
        current.ino === BigInt(identity.ino)
      ) return candidate
    } catch {
      // Try the next kernel-owned descriptor namespace, if one exists.
    }
  }
  throw Object.assign(
    new Error(`identity-bound output publication is unavailable on ${platform}`),
    { code: 'mcp-output-denied' },
  )
}
