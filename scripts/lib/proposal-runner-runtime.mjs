import { spawnSync } from 'node:child_process'
import { createHash, randomUUID } from 'node:crypto'
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  renameSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { dirname, join } from 'node:path'

import { MAX_CONTEXT_CHARS } from './proposal-runner-contracts.mjs'

export class RunnerError extends Error {
  constructor(message, code = 1) {
    super(message)
    this.exitCode = code
  }
}

export const fail = (message, code = 1) => {
  throw new RunnerError(message, code)
}

export const sha256Buffer = (bytes) => createHash('sha256').update(bytes).digest('hex')

export const sha256File = (path) => sha256Buffer(readFileSync(path))

export const readJson = (path) => {
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch (error) {
    fail(`${path} must contain valid JSON: ${error.message}`)
  }
}

export const maybeReadJson = (path) => {
  if (!existsSync(path)) return null
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch {
    return null
  }
}

export const writeJson = (path, value) => {
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`)
}

export const writeJsonAtomic = (path, value) => {
  mkdirSync(dirname(path), { recursive: true, mode: 0o700 })
  const temporary = `${path}.tmp-${process.pid}-${randomUUID()}`
  writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 })
  renameSync(temporary, path)
}

export const writeText = (path, value) => {
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, value)
}

export const readTextExcerpt = (path, maxChars = MAX_CONTEXT_CHARS) => {
  if (!existsSync(path)) return null
  const raw = readFileSync(path, 'utf8')
  return {
    path,
    sha256: sha256File(path),
    char_count: [...raw].length,
    truncated: [...raw].length > maxChars,
    text: [...raw].slice(0, maxChars).join(''),
  }
}

export const assertFile = (path, label) => {
  if (!existsSync(path)) fail(`${label} not found: ${path}`)
  if (lstatSync(path).isSymbolicLink()) fail(`${label} must not be a symlink: ${path}`)
  if (!statSync(path).isFile()) fail(`${label} must be a file: ${path}`)
}

export const resolveMdpCommand = (mdpBin, bundleRoot) => {
  const fromArg = mdpBin || process.env.MDP_BIN
  if (fromArg) {
    if (/\s/.test(fromArg)) {
      fail(
        'MDP_BIN/--mdp-bin must be an executable path without spaces; use a wrapper script for multi-argument commands.',
      )
    }
    return [fromArg]
  }

  const cargoManifest = join(bundleRoot, 'cli', 'Cargo.toml')
  if (existsSync(cargoManifest)) {
    return ['cargo', 'run', '--quiet', '--manifest-path', cargoManifest, '--']
  }
  return ['mdp']
}

export const nonProviderEnvironment = (source = process.env) =>
  Object.fromEntries(
    Object.entries(source).filter(
      ([key]) => !/(?:KEY|TOKEN|SECRET|PASSWORD|CREDENTIAL|AUTH)/i.test(key),
    ),
  )

export const runProcess = ({
  command,
  args,
  stdoutPath,
  stderrPath,
  allowNonZero = false,
  environment = nonProviderEnvironment(),
}) => {
  const result = spawnSync(command[0], [...command.slice(1), ...args], {
    encoding: 'utf8',
    env: environment,
    maxBuffer: 20 * 1024 * 1024,
  })
  if (stdoutPath) writeText(stdoutPath, result.stdout || '')
  if (stderrPath) writeText(stderrPath, result.stderr || '')
  const status = result.status ?? 1
  if (result.error) {
    fail(`Failed to run ${command[0]}: ${result.error.message}`)
  }
  if (status !== 0 && !allowNonZero) {
    fail(
      `Command failed (${status}): ${[...command, ...args].join(' ')}\n${result.stderr || result.stdout}`,
    )
  }
  return {
    status,
    stdout: result.stdout || '',
    stderr: result.stderr || '',
  }
}
