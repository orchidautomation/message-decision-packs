import { constants, closeSync, fstatSync, lstatSync, mkdirSync, openSync, readSync, realpathSync, statSync } from 'node:fs'
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

export const createPathPolicy = (env = process.env) => {
  const roots = parseApprovedRoots(env)
  const select = (role, candidate) => {
    if (!roots[role]) fail('mcp-root-invalid', `unknown root role ${role}`)
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
    const opened = statSync(selected.path)
    if ((kind === 'file' && !opened.isFile()) || (kind === 'directory' && !opened.isDirectory())) {
      fail('mcp-path-denied', `${role} path has the wrong type`)
    }
    return selected
  }
  const newOutput = (role, candidate) => {
    const requested = resolve(candidate)
    const leaf = basename(requested)
    if (!leaf || leaf === '.' || leaf === '..') fail('mcp-output-denied', 'output must name a new leaf')
    try { lstatSync(requested); fail('mcp-output-denied', 'output already exists') } catch (error) { if (error?.code !== 'ENOENT') throw error }
    const parent = existing(role, dirname(requested), 'directory')
    const output = resolve(parent.path, leaf)
    if (!within(parent.path, output)) fail('mcp-output-denied', 'output escaped approved parent')
    return { path: output, root: parent.root, alias: role, parent: parent.path }
  }
  const freeze = (role, candidate, maxBytes = 1_048_576) => {
    const selected = existing(role, candidate, 'file')
    let fd
    try {
      fd = openSync(candidate, constants.O_RDONLY | (constants.O_NOFOLLOW || 0))
      const before = fstatSync(fd, { bigint: true })
      if (before.size > BigInt(maxBytes) || before.nlink !== 1n) fail('mcp-file-denied', `${role} file is not immutable`) 
      const bytes = Buffer.alloc(Number(before.size)); let offset = 0
      while (offset < bytes.length) { const count = readSync(fd, bytes, offset, bytes.length - offset, offset); if (!count) break; offset += count }
      const after = fstatSync(fd, { bigint: true })
      if (offset !== bytes.length || before.dev !== after.dev || before.ino !== after.ino || before.size !== after.size || before.mtimeNs !== after.mtimeNs || before.ctimeNs !== after.ctimeNs) fail('mcp-file-denied', `${role} file changed while being read`)
      return { ...selected, bytes, sha256: createHash('sha256').update(bytes).digest('hex') }
    } catch (error) { if (error?.code === 'ELOOP') fail('mcp-file-denied', `${role} path must not be a symlink`); throw error }
    finally { if (fd !== undefined) closeSync(fd) }
  }
  return Object.freeze({ roots, existing, freeze, newOutput, select })
}

export const boundedDenial = (error) => ({ code: error?.code || 'mcp-path-denied', message: String(error?.message || 'request denied').slice(0, 256) })
