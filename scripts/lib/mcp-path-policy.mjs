import { constants, closeSync, fstatSync, lstatSync, openSync, readSync, realpathSync, statSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { basename, delimiter, dirname, isAbsolute, relative, resolve, sep } from 'node:path'

export const ROOT_ENV = Object.freeze({
  pack: 'MDP_MCP_PACK_ROOTS',
  input: 'MDP_MCP_INPUT_ROOTS',
  approval: 'MDP_MCP_APPROVAL_ROOTS',
  work: 'MDP_MCP_WORK_ROOTS',
  output: 'MDP_MCP_OUTPUT_ROOTS',
  consent: 'MDP_MCP_CONSENT_ROOTS',
})

const fail = (code, message) => { throw Object.assign(new Error(message), { code }) }
const within = (root, candidate) => {
  const path = relative(root, candidate)
  return path === '' || (!path.startsWith(`..${sep}`) && path !== '..' && !isAbsolute(path))
}

const entries = (value, envName) => {
  if (typeof value !== 'string' || value.trim() === '') fail('mcp-roots-not-configured', `${envName} is required`)
  const values = value.split(delimiter).map((item) => item.trim()).filter(Boolean)
  if (!values.length) fail('mcp-roots-not-configured', `${envName} is required`)
  const roots = values.map((item) => {
    let canonical
    try { canonical = realpathSync(item) } catch { fail('mcp-root-invalid', `${envName} contains an unavailable root`) }
    const stats = lstatSync(item)
    if (stats.isSymbolicLink() || !stats.isDirectory()) fail('mcp-root-invalid', `${envName} contains a non-directory root`)
    return canonical
  })
  if (new Set(roots).size !== roots.length) fail('mcp-root-invalid', `${envName} contains duplicate roots`)
  return roots
}

export const parseApprovedRoots = (env = process.env) => Object.freeze(Object.fromEntries(
  Object.entries(ROOT_ENV).map(([role, name]) => [role, entries(env[name], name)]),
))

export const createPathPolicy = (env = process.env, roles = Object.keys(ROOT_ENV)) => {
  const roots = Object.fromEntries(roles.map((role) => [role, null]))
  const requireRole = (role) => {
    if (!(role in roots)) fail('mcp-roots-not-configured', `${ROOT_ENV[role] || role} is required for this tool`)
    if (roots[role] === null) roots[role] = entries(env[ROOT_ENV[role]], ROOT_ENV[role])
  }
  const select = (role, candidate) => {
    requireRole(role)
    const requested = resolve(candidate)
    let canonical
    try { canonical = realpathSync(requested) } catch { fail('mcp-path-denied', `${role} path is unavailable`) }
    const root = roots[role].find((item) => within(item, canonical))
    if (!root) fail('mcp-path-denied', `${role} path is outside approved roots`)
    return { path: canonical, root, alias: role }
  }
  const existing = (role, candidate, kind = 'file') => {
    const selected = select(role, candidate)
    const stats = lstatSync(candidate)
    if (stats.isSymbolicLink()) fail('mcp-path-denied', `${role} path must not be a symlink`)
    let fd
    try {
      fd = openSync(candidate, constants.O_RDONLY | (constants.O_NOFOLLOW || 0))
      const opened = fstatSync(fd, { bigint: true })
      const selectedStats = lstatSync(selected.path, { bigint: true })
      if (opened.dev !== selectedStats.dev || opened.ino !== selectedStats.ino || opened.mode !== selectedStats.mode) fail('mcp-path-denied', `${role} path changed while being opened`)
      if ((kind === 'file' && !opened.isFile()) || (kind === 'directory' && !opened.isDirectory())) fail('mcp-path-denied', `${role} path has the wrong type`)
    } catch (error) { if (error?.code === 'ELOOP') fail('mcp-path-denied', `${role} path must not be a symlink`); throw error }
    finally { if (fd !== undefined) closeSync(fd) }
    const identity = statSync(selected.path, { bigint: true })
    return { ...selected, identity: { dev: identity.dev, ino: identity.ino } }
  }
  const newOutput = (role, candidate) => {
    const requested = resolve(candidate)
    const leaf = basename(requested)
    if (!leaf || leaf === '.' || leaf === '..') fail('mcp-output-denied', 'output must name a new leaf')
    try { lstatSync(requested); fail('mcp-output-denied', 'output must not already exist') } catch (error) { if (error?.code !== 'ENOENT') throw error }
    const parent = existing(role, dirname(requested), 'directory')
    const output = resolve(parent.path, leaf)
    if (!within(parent.path, output)) fail('mcp-output-denied', 'output escaped approved parent')
    return { path: output, root: parent.root, alias: role, parent: parent.path, parentIdentity: parent.identity }
  }
  const freeze = (role, candidate, maxBytes = 1_048_576) => {
    const selected = existing(role, candidate, 'file')
    let fd
    try {
      fd = openSync(candidate, constants.O_RDONLY | (constants.O_NOFOLLOW || 0))
      const before = fstatSync(fd, { bigint: true })
      if (before.dev !== BigInt(selected.identity.dev) || before.ino !== BigInt(selected.identity.ino)) fail('mcp-file-denied', `${role} file changed while being opened`)
      if (before.size > BigInt(maxBytes) || before.nlink !== 1n) fail('mcp-file-denied', `${role} file is not immutable`) 
      const bytes = Buffer.alloc(Number(before.size)); let offset = 0
      while (offset < bytes.length) { const count = readSync(fd, bytes, offset, bytes.length - offset, offset); if (!count) break; offset += count }
      const after = fstatSync(fd, { bigint: true })
      if (offset !== bytes.length || before.dev !== after.dev || before.ino !== after.ino || before.size !== after.size || before.mtimeNs !== after.mtimeNs || before.ctimeNs !== after.ctimeNs) fail('mcp-file-denied', `${role} file changed while being read`)
      return { ...selected, bytes, sha256: createHash('sha256').update(bytes).digest('hex') }
    } catch (error) { if (error?.code === 'ELOOP') fail('mcp-file-denied', `${role} path must not be a symlink`); throw error }
    finally { if (fd !== undefined) closeSync(fd) }
  }
  const finalCheck = (role, candidate, identity, kind = 'file') => {
    const current = existing(role, candidate, kind)
    if (identity && (current.path !== identity.path || current.root !== identity.root || current.identity.dev !== identity.identity.dev || current.identity.ino !== identity.identity.ino)) fail('mcp-path-denied', `${role} path changed before use`)
    return current
  }
  const root = (role) => { requireRole(role); return roots[role] }
  return Object.freeze({ roots, root, existing, freeze, newOutput, select, finalCheck })
}

export const boundedDenial = (error) => ({ code: error?.code || 'mcp-path-denied', message: String(error?.message || 'request denied').slice(0, 256) })
