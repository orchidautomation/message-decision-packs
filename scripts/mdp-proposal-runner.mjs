#!/usr/bin/env node
import { randomUUID } from 'node:crypto'
import {
  chmodSync,
  closeSync,
  existsSync,
  lstatSync,
  mkdirSync,
  openSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs'
import { basename, dirname, extname, isAbsolute, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  DEFAULT_MAX_SOURCE_BYTES,
  DEFAULT_PROMPT_ID,
  DEFAULT_SOURCE_KIND,
  MAX_SNIPPET_CHARS,
  PRIVACY_CLASSES,
  PROMPT_OUTPUT_CONTRACT,
  REQUEST_CONTRACT,
  RESULT_CONTRACT,
  RESULT_CONTRACT_V1,
  RUN_REQUEST_CONTRACT_V1,
  RUNNER_CONTRACT,
  RUN_MANIFEST_CONTRACT,
  SAFE_SOURCE_ID,
  SOURCE_AUDIT_CONTRACT,
  SOURCE_INTAKE_CONTRACT,
  TEXT_EXTENSIONS,
  WORKDIR_CONTRACT,
  promptOutputSchema,
  toolEnvelope,
} from './lib/proposal-runner-contracts.mjs'
import {
  RunnerError,
  assertFile,
  fail,
  maybeReadJson,
  nonProviderEnvironment,
  readJson,
  readTextExcerpt,
  resolveMdpCommand,
  runProcess as runProcessWithDeadline,
  sha256Buffer,
  sha256File,
  writeJson,
  writeJsonAtomic,
  writeText,
} from './lib/proposal-runner-runtime.mjs'
import { buildProposalReadinessReport } from './lib/proposal-readiness-report.mjs'
import { RECOMMENDED_TIMEOUT_MS, validateTransportTimeout } from './lib/deadline-policy.mjs'

const scriptDir = dirname(fileURLToPath(import.meta.url))
const bundleRoot = resolve(scriptDir, '..')
let activeParentDeadline = null
const runProcess = (options) => runProcessWithDeadline({
  ...options,
  deadlineAt: activeParentDeadline,
})

const usage = () => `
Usage:
  node scripts/mdp-proposal-runner.mjs tools
  node scripts/mdp-proposal-runner.mjs run --pack PACK_ROOT --workdir RUN_DIR --source SOURCE_TEXT --source-id SOURCE_ID [--mock-response RESPONSE.json]
  node scripts/mdp-proposal-runner.mjs run --pack PACK_ROOT --workdir RUN_DIR --source-audit SOURCE_AUDIT.json --source SOURCE_TEXT [--mock-response RESPONSE.json]
  node scripts/mdp-proposal-runner.mjs run --pack PACK_ROOT --workdir RUN_DIR --source SOURCE_TEXT --source-id SOURCE_ID --dry-run

Purpose:
  Host-neutral local runner surface for proposal normalization. It stages local
  sources, writes or preserves source-audit evidence, builds a declared-input-only
  native normalization request, invokes the BYOK native runner, then runs
  validate-prompt-output and run-receipt. It can be wrapped by the bundled local
  stdio MCP server, but it is not a hosted/remote MCP service and does not parse
  PDFs, read .env files, create API keys, submit proposals, or approve compliance.

Options:
  --pack PATH              Proposal MDP pack root. Required.
  --clean-run-v1           Finalize the generated output through canonical Rust mdp run v1.
  --pack-release-id ID     Immutable pack release id. Required with --clean-run-v1.
  --workdir PATH           Empty/customer-controlled run directory. Required.
  --source PATH            Text/Markdown/CSV/JSON/YAML source file. Repeatable.
  --source-intake PATH     Existing approved mdp.source-intake.v0 JSON for a real native run.
  --source-audit PATH      Existing mdp.source-audit.v0 JSON to preserve.
  --source-id ID           .mdp/sources.yaml source id for generated source-audit refs.
  --source-kind KIND       Prompt source_kind. Defaults to ${DEFAULT_SOURCE_KIND}.
  --privacy-class CLASS    Ledger privacy class; defaults conservatively from source-kind.
  --model MODEL            Model id. Required for real native calls; defaults to gpt-test for dry/mock.
  --mock-response PATH     Offline provider response fixture for native runner tests.
  --dry-run                Validate request shape only; no model output, receipt, fit, or route.
  --mdp-bin PATH           mdp executable path. Defaults to source cargo run when available, else mdp.
  --native-runner PATH     Native runner script. Defaults to adjacent mdp-native-normalize-openai.mjs.
  --prompt-id ID           Prompt id. Currently only normalize-opportunity is supported.
  --reuse-workdir-id ID    Reuse a non-empty workdir only when its ownership manifest matches.
  --skip-review            Skip fit/route review-support probes after receipt.
  --require-audit-grade    Exit nonzero unless run-receipt returns decision audit-grade.
  --max-source-bytes N     Per-source bounded text bytes to include in prompt payload.
  --timeout-ms N            One parent deadline for all runner phases (default ${RECOMMENDED_TIMEOUT_MS}).
`.trim()

const parseArgs = (argv) => {
  const command = argv[0] || 'help'
  const args = {
    command,
    pack: null,
    cleanRunV1: false,
    packReleaseId: null,
    workdir: null,
    sources: [],
    sourceIntake: null,
    sourceAudit: null,
    sourceId: null,
    sourceKind: DEFAULT_SOURCE_KIND,
    privacyClass: null,
    model: null,
    mockResponse: null,
    dryRun: false,
    mdpBin: null,
    nativeRunner: null,
    promptId: DEFAULT_PROMPT_ID,
    reuseWorkdirId: null,
    skipReview: false,
    requireAuditGrade: false,
    maxSourceBytes: DEFAULT_MAX_SOURCE_BYTES,
    timeoutMs: RECOMMENDED_TIMEOUT_MS,
  }

  if (command === 'help' || command === '--help' || command === '-h') return args
  if (command !== 'run' && command !== 'tools') fail(`Unknown command: ${command}\n\n${usage()}`)

  const next = (index, flag) => {
    if (index + 1 >= argv.length) fail(`Missing value for ${flag}`)
    return argv[index + 1]
  }

  for (let index = 1; index < argv.length; index += 1) {
    const flag = argv[index]
    switch (flag) {
      case '--pack':
        args.pack = next(index, flag)
        index += 1
        break
      case '--clean-run-v1':
        args.cleanRunV1 = true
        break
      case '--pack-release-id':
        args.packReleaseId = next(index, flag)
        index += 1
        break
      case '--workdir':
        args.workdir = next(index, flag)
        index += 1
        break
      case '--source':
        args.sources.push(next(index, flag))
        index += 1
        break
      case '--source-intake':
        args.sourceIntake = next(index, flag)
        index += 1
        break
      case '--source-audit':
        args.sourceAudit = next(index, flag)
        index += 1
        break
      case '--source-id':
        args.sourceId = next(index, flag)
        index += 1
        break
      case '--source-kind':
        args.sourceKind = next(index, flag)
        index += 1
        break
      case '--privacy-class':
        args.privacyClass = next(index, flag)
        index += 1
        break
      case '--model':
        args.model = next(index, flag)
        index += 1
        break
      case '--mock-response':
        args.mockResponse = next(index, flag)
        index += 1
        break
      case '--mdp-bin':
        args.mdpBin = next(index, flag)
        index += 1
        break
      case '--native-runner':
        args.nativeRunner = next(index, flag)
        index += 1
        break
      case '--prompt-id':
        args.promptId = next(index, flag)
        index += 1
        break
      case '--max-source-bytes':
        args.maxSourceBytes = Number.parseInt(next(index, flag), 10)
        if (!Number.isFinite(args.maxSourceBytes) || args.maxSourceBytes < 1000) {
          fail('--max-source-bytes must be an integer >= 1000')
        }
        index += 1
        break
      case '--timeout-ms':
        args.timeoutMs = Number.parseInt(next(index, flag), 10)
        validateTransportTimeout(args.timeoutMs)
        index += 1
        break
      case '--dry-run':
        args.dryRun = true
        break
      case '--reuse-workdir-id':
        args.reuseWorkdirId = next(index, flag)
        index += 1
        break
      case '--skip-review':
        args.skipReview = true
        break
      case '--require-audit-grade':
        args.requireAuditGrade = true
        break
      case '--help':
      case '-h':
        console.log(usage())
        process.exit(0)
        break
      default:
        fail(`Unknown option: ${flag}\n\n${usage()}`)
    }
  }

  return args
}

const isWithin = (root, candidate) => {
  const path = relative(root, candidate)
  return path === '' || (!path.startsWith('..') && !isAbsolute(path))
}

const assertSafePackRoot = (value) => {
  const requested = resolve(value)
  if (!existsSync(requested)) fail(`Pack root not found: ${requested}`)
  if (lstatSync(requested).isSymbolicLink()) fail(`Pack root must not be a symlink: ${requested}`)
  const root = realpathSync(requested)
  if (!statSync(root).isDirectory()) fail(`Pack root must be a directory: ${root}`)
  const mdpRoot = join(root, '.mdp')
  if (!existsSync(mdpRoot) || lstatSync(mdpRoot).isSymbolicLink() || !statSync(mdpRoot).isDirectory()) {
    fail(`Pack root must contain a real .mdp directory: ${root}`)
  }
  const canonicalMdpRoot = realpathSync(mdpRoot)
  if (!isWithin(root, canonicalMdpRoot)) fail(`Pack .mdp directory escapes the pack root: ${mdpRoot}`)
  const visit = (directory) => {
    for (const name of readdirSync(directory)) {
      const path = join(directory, name)
      const stats = lstatSync(path)
      if (stats.isSymbolicLink()) fail(`Pack content must not be a symlink: ${path}`)
      const canonical = realpathSync(path)
      if (!isWithin(canonicalMdpRoot, canonical)) fail(`Pack content escapes .mdp/: ${path}`)
      if (stats.isDirectory()) visit(path)
      else if (!stats.isFile()) fail(`Pack content must be a regular file or directory: ${path}`)
    }
  }
  visit(canonicalMdpRoot)
  return root
}

const assertSafeSourceFile = (value) => {
  const absolute = resolve(value)
  if (!existsSync(absolute)) fail(`source not found: ${absolute}`)
  if (lstatSync(absolute).isSymbolicLink()) fail(`source must not be a symlink: ${absolute}`)
  const canonical = realpathSync(absolute)
  if (!statSync(canonical).isFile()) fail(`source must be a regular file: ${absolute}`)
  return canonical
}

const assertSafeSourceId = (sourceId) => {
  if (!SAFE_SOURCE_ID.test(sourceId || '')) {
    fail('source-id must be lowercase safe ID characters (a-z, 0-9, dot, underscore, hyphen) and at most 128 characters')
  }
}

const defaultPrivacyClass = (sourceKind) => {
  if (sourceKind === 'synthetic-example') return 'synthetic-public'
  if (sourceKind === 'sanitized-example') return 'sanitized-public'
  if (sourceKind === 'private-scratch-opportunity' || sourceKind === 'user-provided-opportunity') {
    return 'private-customer'
  }
  return 'restricted-local'
}

const sourceLedgerIds = (packRoot) => {
  const ledgerPath = join(packRoot, '.mdp', 'sources.yaml')
  assertFile(ledgerPath, 'pack source ledger')
  const ids = new Set()
  for (const line of readFileSync(ledgerPath, 'utf8').split(/\r?\n/)) {
    const match = line.match(/^\s*-\s+id:\s*['"]?([^'"\s#]+)['"]?\s*(?:#.*)?$/)
    if (match) ids.add(match[1])
  }
  return ids
}

const safeBasename = (value) =>
  basename(value)
    .replace(/[^A-Za-z0-9._-]/g, '-')
    .replace(/-+/g, '-')
    .slice(0, 120) || 'source.txt'

const firstSnippet = (text) => {
  const normalized = text.split(/\s+/).filter(Boolean).join(' ')
  return [...normalized].slice(0, MAX_SNIPPET_CHARS).join('')
}

const validateTextSource = (path) => {
  const extension = extname(path).toLowerCase()
  if (!TEXT_EXTENSIONS.has(extension)) {
    fail(`Unsupported source extension for ${path}. Use text, markdown, csv, json, yaml, or provide a prebuilt --source-audit.`)
  }
}

const prepareWorkdir = (workdir, reuseWorkdirId) => {
  const requested = resolve(workdir)
  if (existsSync(requested) && lstatSync(requested).isSymbolicLink()) {
    fail(`Workdir must not be a symlink: ${requested}`)
  }
  const existed = existsSync(requested)
  if (!existed) mkdirSync(requested, { recursive: true, mode: 0o700 })
  const resolved = realpathSync(requested)
  const stats = statSync(resolved)
  if (!stats.isDirectory()) fail(`Workdir must be a directory: ${resolved}`)
  if (typeof process.getuid === 'function' && stats.uid !== process.getuid()) {
    fail(`Workdir is not owned by the current user: ${resolved}`)
  }
  if ((stats.mode & 0o077) !== 0) {
    if (existed) fail(`Workdir permissions must not allow group/other access: ${resolved}`)
    chmodSync(resolved, 0o700)
  }

  const manifestPath = join(resolved, '.mdp-proposal-workdir.json')
  const entries = readdirSync(resolved)
  let manifest
  if (entries.length > 0) {
    if (!reuseWorkdirId) {
      fail(`Workdir already exists and is not empty: ${resolved}\nPass --reuse-workdir-id only with the matching ownership manifest.`)
    }
    if (!existsSync(manifestPath) || lstatSync(manifestPath).isSymbolicLink()) {
      fail(`Workdir reuse requires a regular ownership manifest: ${manifestPath}`)
    }
    manifest = readJson(manifestPath)
    if (
      manifest.contract !== WORKDIR_CONTRACT ||
      manifest.workdir_id !== reuseWorkdirId ||
      manifest.root !== resolved
    ) {
      fail('Workdir reuse manifest does not match the requested directory and --reuse-workdir-id')
    }
  } else {
    if (reuseWorkdirId) fail('--reuse-workdir-id cannot initialize a new workdir')
    manifest = {
      contract: WORKDIR_CONTRACT,
      workdir_id: randomUUID(),
      root: resolved,
      owner_uid: typeof process.getuid === 'function' ? process.getuid() : null,
      created_at: new Date().toISOString(),
    }
    writeJson(manifestPath, manifest)
  }
  return { root: resolved, manifest }
}

const validateManagedDirectory = (workdir, name) => {
  const path = join(workdir, name)
  if (!existsSync(path)) return
  const stats = lstatSync(path)
  if (stats.isSymbolicLink()) fail(`Managed proposal directory must not be a symlink: ${path}`)
  if (!stats.isDirectory()) fail(`Managed proposal path must be a directory: ${path}`)
  const canonical = realpathSync(path)
  if (!isWithin(workdir, canonical)) fail(`Managed proposal directory escapes the workdir: ${path}`)
}

const resetManagedDirectory = (workdir, name, clear) => {
  const path = join(workdir, name)
  validateManagedDirectory(workdir, name)
  if (clear && existsSync(path)) rmSync(path, { recursive: true, force: true })
  if (!existsSync(path)) mkdirSync(path, { mode: 0o700 })
  const canonical = realpathSync(path)
  if (!isWithin(workdir, canonical)) fail(`Managed proposal directory escapes the workdir: ${path}`)
  chmodSync(canonical, 0o700)
}

let activeRun = null

const collectRunArtifacts = (workdir) => {
  const files = []
  const visit = (root) => {
    if (!existsSync(root)) return
    for (const name of readdirSync(root).sort()) {
      const path = join(root, name)
      const stats = lstatSync(path)
      if (stats.isSymbolicLink()) fail(`Run artifact must not be a symlink: ${path}`)
      if (stats.isDirectory()) visit(path)
      else if (stats.isFile()) {
        files.push({
          path: relative(workdir, path),
          sha256: sha256File(path),
          byte_count: stats.size,
        })
      }
    }
  }
  visit(join(workdir, 'artifacts'))
  visit(join(workdir, 'sources'))
  return files
}

const startRunManifest = ({ workdir, ownership, reuseWorkdirId, args }) => {
  const manifestPath = join(workdir, '.mdp-proposal-run.json')
  const lockPath = join(workdir, '.mdp-proposal-run.lock')
  if (reuseWorkdirId) {
    const previous = maybeReadJson(manifestPath)
    if (
      !previous ||
      previous.contract !== RUN_MANIFEST_CONTRACT ||
      previous.owner?.workdir_id !== ownership.workdir_id ||
      !['completed', 'blocked'].includes(previous.status)
    ) {
      fail('Workdir reuse requires a matching terminal proposal run manifest; partial or unknown runs fail closed.')
    }
  } else if (existsSync(manifestPath) || existsSync(lockPath)) {
    fail('Workdir contains proposal run state but reuse was not explicitly authorized.')
  }

  let lockFd
  try {
    lockFd = openSync(lockPath, 'wx', 0o600)
    writeFileSync(lockFd, `${JSON.stringify({ contract: RUN_MANIFEST_CONTRACT, pid: process.pid })}\n`)
  } catch (error) {
    fail(`Proposal workdir is locked by another or interrupted run: ${error.code || error.message}`)
  } finally {
    if (lockFd !== undefined) closeSync(lockFd)
  }

  try {
    validateManagedDirectory(workdir, 'artifacts')
    validateManagedDirectory(workdir, 'sources')
    resetManagedDirectory(workdir, 'artifacts', Boolean(reuseWorkdirId))
    resetManagedDirectory(workdir, 'sources', Boolean(reuseWorkdirId))
  } catch (error) {
    unlinkSync(lockPath)
    throw error
  }

  const runId = randomUUID()
  const mode = args.dryRun ? 'dry-run' : args.mockResponse ? 'mock' : 'native'
  const manifest = {
    contract: RUN_MANIFEST_CONTRACT,
    run_id: runId,
    owner: {
      workdir_id: ownership.workdir_id,
      uid: typeof process.getuid === 'function' ? process.getuid() : null,
    },
    runner: {
      contract: RUNNER_CONTRACT,
      version: 'v0',
      pid: process.pid,
    },
    command: {
      mode,
      prompt_id: args.promptId,
      source_count: args.sources.length,
      reuse: Boolean(reuseWorkdirId),
    },
    started_at: new Date().toISOString(),
    ended_at: null,
    status: 'in-progress',
    decision: null,
    artifacts: [],
  }
  writeJsonAtomic(manifestPath, manifest)
  const readback = readJson(manifestPath)
  if (readback.run_id !== runId || readback.status !== 'in-progress') {
    fail('Proposal run manifest failed atomic start readback.')
  }
  activeRun = { workdir, manifestPath, lockPath, manifest }
  return activeRun
}

const finalizeRunManifest = ({ status, decision = 'blocked', error = null }) => {
  if (!activeRun) return null
  const terminal = {
    ...activeRun.manifest,
    ended_at: new Date().toISOString(),
    status,
    decision,
    artifacts: collectRunArtifacts(activeRun.workdir),
    ...(error ? { error } : {}),
  }
  writeJsonAtomic(activeRun.manifestPath, terminal)
  const readback = readJson(activeRun.manifestPath)
  if (
    readback.run_id !== terminal.run_id ||
    readback.status !== status ||
    readback.artifacts.length !== terminal.artifacts.length
  ) {
    fail('Proposal run manifest failed terminal readback.')
  }
  unlinkSync(activeRun.lockPath)
  activeRun = null
  return readback
}

const stageSources = (sources, workdir, maxSourceBytes) => {
  const staged = []
  const sourcesDir = join(workdir, 'sources')
  sources.forEach((source, index) => {
    const absolute = assertSafeSourceFile(source)
    validateTextSource(absolute)
    const bytes = readFileSync(absolute)
    const stagedName = `${String(index + 1).padStart(2, '0')}-${safeBasename(absolute)}`
    const stagedPath = join(sourcesDir, stagedName)
    writeFileSync(stagedPath, bytes, { flag: 'wx', mode: 0o600 })
    const sourceHash = sha256Buffer(bytes)
    const stagedHash = sha256File(stagedPath)
    if (stagedHash !== sourceHash) fail(`Staged source hash mismatch: ${absolute}`)
    const excerptBytes = bytes.subarray(0, maxSourceBytes)
    const text = excerptBytes.toString('utf8')
    staged.push({
      index,
      original_path: absolute,
      filename: basename(absolute),
      staged_path: relative(workdir, stagedPath),
      sha256: sourceHash,
      byte_count: bytes.length,
      truncated: bytes.length > maxSourceBytes,
      text,
    })
  })
  return staged
}

const normalizedIncludes = (text, snippet) => {
  const normalize = (value) => value.split(/\s+/).filter(Boolean).join(' ')
  return normalize(text).includes(normalize(snippet))
}

const sourceRefsByStagedSource = (sourceAudit, stagedSources) => {
  if (sourceAudit.contract !== SOURCE_AUDIT_CONTRACT || !Array.isArray(sourceAudit.refs)) {
    fail(`source audit must use ${SOURCE_AUDIT_CONTRACT} with a refs array`)
  }
  const bindings = new Map(stagedSources.map((source) => [source.staged_path, []]))
  const seenRefs = new Set()
  for (const ref of sourceAudit.refs) {
    if (!ref || typeof ref !== 'object' || typeof ref.ref !== 'string') {
      fail('source audit refs must be objects with a string ref')
    }
    if (seenRefs.has(ref.ref)) fail(`source audit contains duplicate ref: ${ref.ref}`)
    seenRefs.add(ref.ref)
    if (ref.ref === 'source_kind') continue
    const matches = stagedSources.filter((source) => {
      const snippetMatches =
        typeof ref.snippet === 'string' && ref.snippet.trim().length > 0 && normalizedIncludes(source.text, ref.snippet)
      const locatorNamesSource =
        typeof ref.locator === 'string' &&
        stagedSources.some((candidate) => ref.locator.includes(candidate.filename))
      const locatorMatches = typeof ref.locator === 'string' && ref.locator.includes(source.filename)
      return snippetMatches && (!locatorNamesSource || locatorMatches)
    })
    if (matches.length !== 1) {
      fail(`source audit ref ${ref.ref} must bind to exactly one staged source with matching snippet bytes`)
    }
    bindings.get(matches[0].staged_path).push(ref)
  }
  for (const source of stagedSources) {
    if (bindings.get(source.staged_path).length === 0) {
      fail(`source audit does not bind staged source: ${source.filename}`)
    }
  }
  return bindings
}

const buildSourceIntake = ({
  supplied,
  sourceAudit,
  stagedSources,
  sourceId,
  sourceKind,
  privacyClass,
}) => {
  const bindings = sourceRefsByStagedSource(sourceAudit, stagedSources)
  const sourceKindRefs = sourceAudit.refs.filter((ref) => ref?.ref === 'source_kind')
  if (
    sourceKindRefs.length !== 1 ||
    sourceKindRefs[0].snippet !== sourceKind ||
    (sourceId && sourceKindRefs[0].source_id !== sourceId)
  ) {
    fail('source audit must contain exactly one source_kind ref matching the selected source kind and source ID')
  }
  const now = new Date().toISOString()
  const entries = stagedSources.map((source) => {
    const refs = bindings.get(source.staged_path)
    const refSourceIds = [...new Set(refs.map((ref) => ref.source_id))]
    const resolvedSourceId = sourceId || (refSourceIds.length === 1 ? refSourceIds[0] : null)
    assertSafeSourceId(resolvedSourceId)
    if (refSourceIds.some((value) => value !== resolvedSourceId)) {
      fail(`source audit source_id mismatch for staged source: ${source.filename}`)
    }
    if (sourceKindRefs[0].source_id !== resolvedSourceId) {
      fail(`source audit source_kind source_id mismatch for staged source: ${source.filename}`)
    }
    return {
      candidate_id: `candidate-${String(source.index + 1).padStart(3, '0')}-${source.sha256.slice(0, 12)}`,
      state: 'candidate',
      approval_class: 'candidate',
      source_id: resolvedSourceId,
      source_kind: sourceKind,
      artifact: {
        path: source.staged_path,
        sha256: source.sha256,
        byte_count: source.byte_count,
        media_type: 'text/plain',
      },
      origin: {
        kind: 'operator-supplied-local-file',
        locator: source.original_path,
        importer: 'mdp-proposal-runner',
        importer_version: 'v0',
        imported_at: now,
        operator_supplied: true,
      },
      privacy_class: privacyClass,
      derivation: {
        parent_candidate_ids: [],
        method: 'bounded-text-staging',
      },
      truncated: source.truncated,
      warnings: source.truncated ? [`Source was bounded to ${source.text.length} decoded characters for the model request.`] : [],
      audit_refs: refs.map((ref) => ref.ref).sort(),
    }
  })

  if (!supplied) return { contract: SOURCE_INTAKE_CONTRACT, entries }
  if (supplied.contract !== SOURCE_INTAKE_CONTRACT || !Array.isArray(supplied.entries)) {
    fail(`source intake must use ${SOURCE_INTAKE_CONTRACT} with an entries array`)
  }
  if (supplied.entries.length !== entries.length) {
    fail('source intake entry count must match the staged source count')
  }
  const suppliedByPath = new Map(supplied.entries.map((entry) => [entry?.artifact?.path, entry]))
  return {
    contract: SOURCE_INTAKE_CONTRACT,
    entries: entries.map((expected) => {
      const entry = suppliedByPath.get(expected.artifact.path)
      if (!entry) fail(`source intake is missing staged artifact ${expected.artifact.path}`)
      if (
        entry.state !== 'approved' ||
        entry.approval_class !== 'operator-approved' ||
        entry.approval?.decision !== 'approved'
      ) {
        fail(`source intake entry ${entry.candidate_id || expected.artifact.path} is not operator-approved`)
      }
      for (const field of ['sha256', 'byte_count']) {
        if (entry.artifact?.[field] !== expected.artifact[field]) {
          fail(`source intake ${field} mismatch for ${expected.artifact.path}`)
        }
      }
      if (
        entry.approval.artifact_sha256 !== expected.artifact.sha256 ||
        entry.approval.purpose !== 'proposal-review' ||
        typeof entry.approval.operator !== 'string' ||
        entry.approval.operator.trim().length === 0 ||
        typeof entry.approval.decided_at !== 'string' ||
        !Number.isFinite(Date.parse(entry.approval.decided_at)) ||
        entry.source_id !== expected.source_id ||
        entry.source_kind !== expected.source_kind ||
        entry.privacy_class !== expected.privacy_class
      ) {
        fail(`source intake approval/source metadata mismatch for ${expected.artifact.path}`)
      }
      const actualRefs = [...(entry.audit_refs || [])].sort()
      if (JSON.stringify(actualRefs) !== JSON.stringify(expected.audit_refs)) {
        fail(`source intake audit_refs mismatch for ${expected.artifact.path}`)
      }
      return entry
    }),
  }
}

const generatedSourceAudit = ({ stagedSources: sources, sourceId, sourceKind }) => {
  if (!sourceId) {
    fail('Generating a source audit from --source requires --source-id matching an id in the pack .mdp/sources.yaml. Pass --source-audit to preserve a prebuilt ledger instead.')
  }
  if (sources.length === 0) fail('Generating a source audit requires at least one --source file')
  return {
    contract: SOURCE_AUDIT_CONTRACT,
    refs: [
      ...sources.map((source) => ({
        ref: `raw_opportunity.sources[${source.index}]`,
        source_id: sourceId,
        locator: `${source.staged_path}#bounded-text`,
        snippet: firstSnippet(source.text),
        confidence: 'operator-supplied',
      })),
      {
        ref: 'source_kind',
        source_id: sourceId,
        locator: 'operator-input#source-kind',
        snippet: sourceKind,
        confidence: 'operator-supplied',
      },
    ],
  }
}

const packContext = (packRoot, promptPath) => ({
  prompt_contract: readTextExcerpt(promptPath),
  manifest: readTextExcerpt(join(packRoot, '.mdp', 'manifest.yaml')),
  sources: readTextExcerpt(join(packRoot, '.mdp', 'sources.yaml')),
  constraints: [
    'Use only raw_opportunity, existing_pack_context, runtime_context, source_audit, and source_kind.',
    'Do not browse, enrich, scrape, call tools, submit proposals, certify compliance, invent proof, or infer missing deadlines.',
    'Cite source_audit refs for raw_opportunity/source_kind-backed facts.',
    'Return strict JSON only.',
    'Return normalized_prospect as the CLI-compatible normalized entity; do not include the optional normalized_opportunity readability alias in native strict-runner output.',
  ],
})

const buildRequest = ({ args, packRoot, promptPath, sourceAudit, sourceIntake, sourceIntakeSha256, stagedSources }) => {
  const intakeByPath = new Map(sourceIntake.entries.map((entry) => [entry.artifact.path, entry]))
  const rawOpportunity =
    stagedSources.length > 0
      ? {
          source_shape: 'bounded-local-text-excerpts',
          sources: stagedSources.map((source) => ({
            ref: `raw_opportunity.sources[${source.index}]`,
            filename: source.filename,
            staged_path: source.staged_path,
            candidate_id: intakeByPath.get(source.staged_path)?.candidate_id,
            sha256: source.sha256,
            byte_count: source.byte_count,
            truncated: source.truncated,
            text: source.text,
          })),
          source_intake: {
            contract: SOURCE_INTAKE_CONTRACT,
            sha256: sourceIntakeSha256,
            states: [...new Set(sourceIntake.entries.map((entry) => entry.state))].sort(),
          },
        }
      : {
          source_shape: 'source-audit-only',
          note: 'No raw source text was supplied to this runner. This mode is suitable for mock/dry-run only unless the supplied source_audit snippets contain the approved normalization payload.',
        }

  const existingPackContext = packContext(packRoot, promptPath)
  existingPackContext.runner_package = {
    prompt_id: args.promptId,
    task: 'Normalize supplied proposal material into mdp.prompt-output.v0 for prompt normalize-opportunity. Return strict JSON only.',
    safety_rules: [
      'Use only the declared payload fields in this JSON object.',
      'Do not use ambient chat context, hidden memory, browsing, tools, external systems, or prior messages.',
      'Do not invent RFP text, certifications, compliance status, past performance, pricing, named references, deadlines, evaluator criteria, or approvals.',
      'When evidence is missing, produce gaps or missing_required entries instead of smoothing uncertainty into a proceed decision.',
      'Return normalized_prospect only; normalized_opportunity is an optional downstream readability alias and is intentionally omitted from this native strict-runner schema.',
    ],
  }

  const payload = {
    raw_opportunity: rawOpportunity,
    existing_pack_context: existingPackContext,
    source_audit: sourceAudit,
    source_kind: args.sourceKind,
  }

  return {
    contract: REQUEST_CONTRACT,
    provider: 'openai',
    model: args.model || 'gpt-test',
    prompt_id: args.promptId,
    declared_inputs_only: true,
    input: [
      {
        role: 'user',
        content: JSON.stringify(payload),
      },
    ],
    prompt_output_schema: promptOutputSchema(),
  }
}

const parseCliData = (path) => {
  const value = maybeReadJson(path)
  if (!value) return null
  return value.data || value
}

const cleanRunInput = (logicalName, sourcePath, schemaId, provenanceRefs = []) => ({
  logical_name: logicalName,
  source_path: sourcePath,
  schema_id: schemaId,
  media_type: 'application/json',
  provenance_refs: provenanceRefs,
})

const buildCleanRunV1Request = ({ args, packRoot, runState, paths }) => ({
  contract: RUN_REQUEST_CONTRACT_V1,
  execution_id: runState.manifest.run_id,
  created_at: runState.manifest.started_at,
  profile: 'proposal',
  operation: 'validate-existing-output',
  mode: 'deterministic',
  job_identity: null,
  pack_dir: packRoot,
  pack_release_id: args.packReleaseId,
  prompt: null,
  inputs: [
    cleanRunInput('native-request', paths.request, REQUEST_CONTRACT),
    cleanRunInput('prompt-output', paths.promptOutput, PROMPT_OUTPUT_CONTRACT, ['native-request']),
    cleanRunInput('source-audit', paths.sourceAudit, SOURCE_AUDIT_CONTRACT),
    cleanRunInput('source-intake', paths.sourceIntake, SOURCE_INTAKE_CONTRACT, ['source-audit']),
    cleanRunInput('runner-audit', paths.runnerAudit, 'mdp.runner-audit.v0', [
      'native-request',
      'prompt-output',
    ]),
  ],
  execution_policy: {
    environment_allowlist: [],
    filesystem_mode: 'private-staging',
    tool_mode: 'none',
    network_mode: 'none',
    authorized_endpoints: [],
    max_input_bytes: 20_000_000,
    max_output_bytes: 1_048_576,
    // The selected host timeout is an outer guard. Canonical v1 keeps its
    // fixed 60s inner policy so the receipt can show which bound won.
    timeout_ms: RECOMMENDED_TIMEOUT_MS,
    retention_policy: 'customer-controlled-workdir',
  },
  driver: null,
  model: null,
})

const persistRunnerResult = ({
  paths,
  result,
  validation = null,
  receipt = null,
  runnerAudit = null,
  sourceIntake,
}) => {
  result.readiness_report = paths.readinessReport
  writeJson(paths.result, result)
  const report = buildProposalReadinessReport({
    result,
    validation,
    receipt,
    runnerAudit,
    sourceIntake,
    paths,
  })
  writeJson(paths.readinessReport, report)
  return report
}

const run = async (args) => {
  if (!args.pack) fail(`Missing --pack\n\n${usage()}`)
  if (!args.workdir) fail(`Missing --workdir\n\n${usage()}`)
  if (args.promptId !== DEFAULT_PROMPT_ID) {
    fail(`This proposal runner currently supports only --prompt-id ${DEFAULT_PROMPT_ID}`)
  }
  if (args.cleanRunV1 && !args.packReleaseId) {
    fail('--clean-run-v1 requires --pack-release-id naming the immutable pack release.')
  }
  if (args.cleanRunV1 && args.dryRun) {
    fail('--clean-run-v1 cannot be combined with --dry-run because no output exists to validate.')
  }
  const packRoot = assertSafePackRoot(args.pack)
  const promptPath = join(packRoot, '.mdp', 'prompts', `${args.promptId}.yaml`)
  assertFile(promptPath, 'prompt contract')
  if (args.sources.length === 0) fail('Pass at least one --source text file so source intake can bind exact staged bytes.')
  const privacyClass = args.privacyClass || defaultPrivacyClass(args.sourceKind)
  if (!PRIVACY_CLASSES.has(privacyClass)) {
    fail(`Unsupported privacy class: ${privacyClass}`)
  }
  if (!args.dryRun && !args.mockResponse && !args.model) {
    fail('Real native runs require --model. Dry-run/mock modes default to gpt-test.')
  }
  if (!args.dryRun && !args.mockResponse && !args.sourceIntake) {
    fail('Real native runs require --source-intake with operator-approved entries bound to the staged source bytes.')
  }
  activeParentDeadline = performance.now() + args.timeoutMs

  const nativeRunner = resolve(args.nativeRunner || join(scriptDir, 'mdp-native-normalize-openai.mjs'))
  assertFile(nativeRunner, 'native runner')
  if (args.sourceIntake) assertFile(resolve(args.sourceIntake), 'source intake')
  if (args.sourceAudit) assertFile(resolve(args.sourceAudit), 'source audit')
  if (args.mockResponse) assertFile(resolve(args.mockResponse), 'mock response')

  const preparedWorkdir = prepareWorkdir(args.workdir, args.reuseWorkdirId)
  const workdir = preparedWorkdir.root
  const runState = startRunManifest({
    workdir,
    ownership: preparedWorkdir.manifest,
    reuseWorkdirId: args.reuseWorkdirId,
    args,
  })
  const artifactsDir = join(workdir, 'artifacts')
  const paths = {
    runManifest: runState.manifestPath,
    sourceIntake: join(artifactsDir, 'source-intake.json'),
    sourceAudit: join(artifactsDir, 'source-audit.json'),
    request: join(artifactsDir, 'native-normalize-request.json'),
    nativeDryRun: join(artifactsDir, 'native-normalize-dry-run.json'),
    nativeResult: join(artifactsDir, 'native-normalize-result.json'),
    nativeStderr: join(artifactsDir, 'native-normalize.stderr'),
    packValidation: join(artifactsDir, 'pack-validation.json'),
    packValidationStderr: join(artifactsDir, 'pack-validation.stderr'),
    promptOutput: join(artifactsDir, 'normalize-opportunity-output.json'),
    runnerAudit: join(artifactsDir, 'runner-audit.json'),
    validation: join(artifactsDir, 'normalize-opportunity-validation.json'),
    validationStderr: join(artifactsDir, 'normalize-opportunity-validation.stderr'),
    receipt: join(artifactsDir, 'run-receipt.json'),
    receiptStdout: join(artifactsDir, 'run-receipt.stdout.json'),
    receiptStderr: join(artifactsDir, 'run-receipt.stderr'),
    cleanRunRequest: join(artifactsDir, 'run-request-v1.json'),
    cleanRunDir: join(artifactsDir, 'clean-run-v1'),
    cleanRunStdout: join(artifactsDir, 'clean-run-v1.stdout.json'),
    cleanRunStderr: join(artifactsDir, 'clean-run-v1.stderr'),
    normalized: join(artifactsDir, 'normalized-opportunity.json'),
    fit: join(artifactsDir, 'fit-normalized-opportunity.json'),
    fitStderr: join(artifactsDir, 'fit-normalized-opportunity.stderr'),
    routeBidNoBid: join(artifactsDir, 'route-bid-no-bid-review.json'),
    routeBidNoBidStderr: join(artifactsDir, 'route-bid-no-bid-review.stderr'),
    result: join(artifactsDir, 'proposal-runner-result.json'),
    readinessReport: join(artifactsDir, 'proposal-readiness-report.json'),
  }

  const stagedSources = stageSources(args.sources, workdir, args.maxSourceBytes)
  const sourceAudit = args.sourceAudit
    ? readJson(resolve(args.sourceAudit))
    : generatedSourceAudit({
        stagedSources,
        sourceId: args.sourceId,
        sourceKind: args.sourceKind,
      })
  if (sourceAudit.contract !== SOURCE_AUDIT_CONTRACT) {
    fail(`source audit contract must be ${SOURCE_AUDIT_CONTRACT}`)
  }
  writeJson(paths.sourceAudit, sourceAudit)

  const sourceIntake = buildSourceIntake({
    supplied: args.sourceIntake ? readJson(resolve(args.sourceIntake)) : null,
    sourceAudit,
    stagedSources,
    sourceId: args.sourceId,
    sourceKind: args.sourceKind,
    privacyClass,
  })
  const packSourceIds = sourceLedgerIds(packRoot)
  for (const entry of sourceIntake.entries) {
    if (!packSourceIds.has(entry.source_id)) {
      fail(`source intake source_id ${entry.source_id} does not exist in .mdp/sources.yaml`)
    }
  }
  writeJson(paths.sourceIntake, sourceIntake)
  const sourceIntakeSha256 = sha256File(paths.sourceIntake)

  const request = buildRequest({
    args,
    packRoot,
    promptPath,
    sourceAudit,
    sourceIntake,
    sourceIntakeSha256,
    stagedSources,
  })
  writeJson(paths.request, request)

  const steps = [
    {
      name: 'mdp_intake_sources',
      status: 'ok',
      artifacts: {
        source_intake: paths.sourceIntake,
        source_intake_sha256: sourceIntakeSha256,
        source_audit: paths.sourceAudit,
        workdir_manifest: join(workdir, '.mdp-proposal-workdir.json'),
      },
      staged_sources: stagedSources.map(({ filename, staged_path, sha256, byte_count, truncated }) => ({
        filename,
        staged_path,
        sha256,
        byte_count,
        truncated,
      })),
    },
    {
      name: 'mdp_normalize_opportunity_request',
      status: 'ok',
      artifacts: { request: paths.request },
      declared_inputs_only: true,
      prompt_id: args.promptId,
    },
  ]

  const mdpCommand = resolveMdpCommand(args.mdpBin, bundleRoot)
  const packValidation = await runProcess({
    command: mdpCommand,
    args: ['--json', 'validate', '--dir', packRoot, '--strict'],
    stdoutPath: paths.packValidation,
    stderrPath: paths.packValidationStderr,
    allowNonZero: true,
  })
  const packValidationData = parseCliData(paths.packValidation)
  steps.push({
    name: 'mdp_validate_pack',
    status: packValidation.status === 0 && packValidationData?.valid === true ? 'ok' : 'blocked',
    artifacts: { validation: paths.packValidation },
    exit_status: packValidation.status,
  })
  if (packValidation.status !== 0 || packValidationData?.valid !== true) {
    fail(`Pack validation failed before model invocation. See ${paths.packValidation} and ${paths.packValidationStderr}`)
  }

  const nativeArgs = ['--request', paths.request]
  if (args.dryRun) {
    const dryRun = await runProcess({
      command: ['node', nativeRunner],
      args: [...nativeArgs, '--dry-run'],
      stdoutPath: paths.nativeDryRun,
      stderrPath: paths.nativeStderr,
    })
    steps.push({
      name: 'mdp_normalize_opportunity',
      status: dryRun.status === 0 ? 'dry-run' : 'failed',
      artifacts: { dry_run: paths.nativeDryRun },
      exit_status: dryRun.status,
    })
    const result = {
      contract: RESULT_CONTRACT,
      runner_contract: RUNNER_CONTRACT,
      mode: 'dry-run',
      ok: dryRun.status === 0,
      audit_grade_eligible: false,
      decision: 'not-run',
      runner_assurance: 'not-run',
      run_id: runState.manifest.run_id,
      run_manifest: runState.manifestPath,
      workdir,
      artifacts: paths,
      steps,
      caveats: [
        'Dry-run validates the native request shape only; it does not produce prompt-output, runner-audit, validation, receipt, or proposal review artifacts.',
        'Generated source-intake entries remain candidate state; only an explicitly supplied operator-approved ledger may authorize a real native run.',
      ],
    }
    persistRunnerResult({ paths, result, sourceIntake })
    finalizeRunManifest({ status: 'completed', decision: result.decision })
    console.log(JSON.stringify(result, null, 2))
    return
  }

  const normalizeArgs = [
    ...nativeArgs,
    '--out',
    paths.promptOutput,
    '--runner-audit',
    paths.runnerAudit,
  ]
  if (args.mockResponse) normalizeArgs.push('--mock-response', resolve(args.mockResponse))
  const nativeResult = await runProcess({
    command: ['node', nativeRunner],
    args: normalizeArgs,
    stdoutPath: paths.nativeResult,
    stderrPath: paths.nativeStderr,
    environment: args.mockResponse ? nonProviderEnvironment() : process.env,
    allowNonZero: true,
  })
  if (nativeResult.status !== 0) {
    fail(`Native normalization failed before canonical clean-run finalization. See ${paths.nativeResult} and ${paths.nativeStderr}`)
  }
  steps.push({
    name: 'mdp_normalize_opportunity',
    status: nativeResult.status === 0 ? 'ok' : 'failed',
    artifacts: {
      prompt_output: paths.promptOutput,
      runner_audit: paths.runnerAudit,
      native_result: paths.nativeResult,
    },
    exit_status: nativeResult.status,
    mode: args.mockResponse ? 'mock' : 'native',
  })

  let validationData
  let receiptData
  let canonicalRun = null
  let reportPaths = paths
  if (args.cleanRunV1) {
    writeJson(
      paths.cleanRunRequest,
      buildCleanRunV1Request({ args, packRoot, runState, paths }),
    )
    const cleanRun = await runProcess({
      command: mdpCommand,
      args: [
        '--json',
        'run',
        '--request',
        paths.cleanRunRequest,
        '--out-dir',
        paths.cleanRunDir,
        '--transport-timeout-ms',
        String(args.timeoutMs),
      ],
      stdoutPath: paths.cleanRunStdout,
      stderrPath: paths.cleanRunStderr,
      allowNonZero: true,
      timeoutMs: args.timeoutMs,
      recovery: { outputDir: paths.cleanRunDir, executionId: runState.manifest.run_id },
    })
    canonicalRun = parseCliData(paths.cleanRunStdout)
    validationData = maybeReadJson(join(paths.cleanRunDir, 'artifacts', 'validation.json'))
    receiptData = maybeReadJson(join(paths.cleanRunDir, 'run-receipt.json'))
    steps.push({
      name: 'mdp_clean_run_v1',
      status: canonicalRun?.valid === true ? 'ok' : 'blocked',
      artifacts: {
        request: paths.cleanRunRequest,
        run_dir: paths.cleanRunDir,
        bundle: join(paths.cleanRunDir, 'run-bundle.json'),
        receipt: join(paths.cleanRunDir, 'run-receipt.json'),
      },
      exit_status: cleanRun.status,
      terminal_state: canonicalRun?.terminal_state ?? 'no-draft:runner-failed',
    })
    if (!canonicalRun || !receiptData) {
      fail(`Canonical mdp run did not return complete v1 authority. See ${paths.cleanRunStdout} and ${paths.cleanRunStderr}`)
    }
    reportPaths = {
      ...paths,
      validation: join(paths.cleanRunDir, 'artifacts', 'validation.json'),
      receipt: join(paths.cleanRunDir, 'run-receipt.json'),
    }
  } else {
    const validation = await runProcess({
      command: mdpCommand,
      args: [
        '--json',
        'validate-prompt-output',
        '--dir',
        packRoot,
        '--prompt-id',
        args.promptId,
        '--file',
        paths.promptOutput,
        '--source-audit',
        paths.sourceAudit,
      ],
      stdoutPath: paths.validation,
      stderrPath: paths.validationStderr,
      allowNonZero: true,
    })
    validationData = parseCliData(paths.validation)
    steps.push({
      name: 'mdp_validate_normalization',
      status: validationData?.valid ? 'ok' : 'blocked',
      artifacts: { validation: paths.validation },
      exit_status: validation.status,
      issue_count: validationData?.issues?.length ?? null,
    })

    const receipt = await runProcess({
      command: mdpCommand,
      args: [
        '--json',
        'run-receipt',
        '--dir',
        packRoot,
        '--workflow',
        'proposal-review',
        '--isolation',
        'isolated',
        '--declared-inputs-only',
        '--prompt-id',
        args.promptId,
        '--prompt-output',
        paths.promptOutput,
        '--validation',
        paths.validation,
        '--source-audit',
        paths.sourceAudit,
        '--runner-audit',
        paths.runnerAudit,
        '--require-runner-audit',
        '--artifact',
        `source-intake=${paths.sourceIntake}`,
        '--out',
        paths.receipt,
      ],
      stdoutPath: paths.receiptStdout,
      stderrPath: paths.receiptStderr,
      allowNonZero: true,
    })
    receiptData = maybeReadJson(paths.receipt) || parseCliData(paths.receiptStdout)
    steps.push({
      name: 'mdp_run_receipt',
      status: receiptData?.decision === 'audit-grade' ? 'ok' : 'blocked',
      artifacts: {
        receipt: paths.receipt,
        receipt_stdout: paths.receiptStdout,
      },
      exit_status: receipt.status,
      decision: receiptData?.decision ?? null,
      runner_assurance: receiptData?.runner?.assurance ?? null,
    })
  }

  const promptOutput = maybeReadJson(paths.promptOutput)
  if (validationData?.valid === true && promptOutput?.normalized_prospect) {
    writeJson(paths.normalized, promptOutput.normalized_prospect)
  }

  if (
    !args.skipReview &&
    validationData?.valid === true &&
    (args.cleanRunV1 ? canonicalRun?.valid === true : receiptData?.decision === 'audit-grade')
  ) {
    if (promptOutput?.normalized_prospect) {
      const fit = await runProcess({
        command: mdpCommand,
        args: ['--json', 'fit', '--dir', packRoot, '--prospect', paths.normalized],
        stdoutPath: paths.fit,
        stderrPath: paths.fitStderr,
        allowNonZero: true,
      })
      steps.push({
        name: 'mdp_review_proposal.fit',
        status: fit.status === 0 ? 'ok' : 'blocked',
        artifacts: { fit: paths.fit },
        exit_status: fit.status,
      })
      const route = await runProcess({
        command: mdpCommand,
        args: [
          '--json',
          '--summary',
          'route',
          '--entries',
          '--dir',
          packRoot,
          '--persona',
          'Proposal Lead',
          '--job',
          'bid no bid review',
        ],
        stdoutPath: paths.routeBidNoBid,
        stderrPath: paths.routeBidNoBidStderr,
        allowNonZero: true,
      })
      steps.push({
        name: 'mdp_review_proposal.route',
        status: route.status === 0 ? 'ok' : 'blocked',
        artifacts: { route_bid_no_bid: paths.routeBidNoBid },
        exit_status: route.status,
      })
    } else {
      steps.push({
        name: 'mdp_review_proposal',
        status: 'skipped',
        reason: 'prompt output did not include normalized_prospect',
      })
    }
  } else if (!args.skipReview) {
    steps.push({
      name: 'mdp_review_proposal',
      status: 'skipped',
      reason:
        validationData?.valid !== true
          ? 'prompt-output validation did not succeed'
          : args.cleanRunV1
            ? `canonical run terminal state ${canonicalRun?.terminal_state ?? 'missing'} is not acceptable for review probes`
            : `run receipt decision ${receiptData?.decision ?? 'missing'} is not acceptable for review probes`,
    })
  }

  const decision = args.cleanRunV1
    ? canonicalRun?.valid === true
      ? 'advisory'
      : 'blocked'
    : receiptData?.decision ?? 'blocked'
  const runnerAssurance = args.cleanRunV1
    ? 'see-canonical-authority'
    : receiptData?.runner?.assurance ?? 'unknown'
  const mode = args.mockResponse ? 'mock' : 'native'
  const result = {
    contract: args.cleanRunV1 ? RESULT_CONTRACT_V1 : RESULT_CONTRACT,
    runner_contract: RUNNER_CONTRACT,
    mode,
    ok: args.cleanRunV1 ? canonicalRun?.valid === true : decision === 'audit-grade',
    audit_grade_eligible: args.cleanRunV1 ? false : mode === 'native' && decision === 'audit-grade',
    decision,
    runner_assurance: runnerAssurance,
    run_id: runState.manifest.run_id,
    run_manifest: runState.manifestPath,
    workdir,
    artifacts: args.cleanRunV1 ? reportPaths : paths,
    steps,
    caveats: [
      ...(args.cleanRunV1
        ? [
            'Canonical v1 authority is the exact mdp run result and its hash-bound artifacts. The legacy decision field is an advisory compatibility projection only.',
            'This v1 compatibility route validates an existing generated output deterministically; the current Rust kernel does not claim to have performed or isolated the upstream model invocation.',
          ]
        : []),
      mode === 'mock'
        ? 'Mock mode is offline-only and must not be described as audit-grade model isolation.'
        : 'Native mode is audit-grade only when run-receipt returns decision audit-grade with stateless-api-verified or headless-verified assurance.',
      sourceIntake.entries.every((entry) => entry.state === 'approved')
        ? 'Source intake records explicit operator approval for the exact staged hashes; the receipt hashes this ledger as a source-intake artifact.'
        : 'Source intake remains candidate-only and does not authorize real client-source normalization.',
      'This runner stages bounded local text and source-audit artifacts; it does not prove PDF/OCR quality, semantic truth beyond supplied artifacts, compliance status, legal approval, or proposal submission readiness.',
      'The current surface is a host-neutral local runner command set also exposed by the bundled local stdio MCP wrapper; it is not a hosted or remote MCP service.',
    ],
    ...(args.cleanRunV1
      ? {
          authority_contract: canonicalRun.contract,
          terminal_state: canonicalRun.terminal_state,
          canonical_run: canonicalRun,
          canonical_authority: canonicalRun.authority_block,
        }
      : {}),
  }
  persistRunnerResult({
    paths: reportPaths,
    result,
    validation: validationData,
    receipt: receiptData,
    runnerAudit: maybeReadJson(paths.runnerAudit),
    sourceIntake,
  })
  finalizeRunManifest({ status: 'completed', decision })
  console.log(JSON.stringify(result, null, 2))
  if (args.requireAuditGrade && decision !== 'audit-grade') {
    process.exitCode = 2
  }
}

const main = async () => {
  try {
    const args = parseArgs(process.argv.slice(2))
    if (args.command === 'help') {
      console.log(usage())
      return
    }
    if (args.command === 'tools') {
      console.log(JSON.stringify(toolEnvelope(), null, 2))
      return
    }
    await run(args)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    if (activeRun) {
      try {
        finalizeRunManifest({
          status: 'blocked',
          decision: 'blocked',
          error: {
            code: 'runner-failed',
            message: message.split(/\r?\n/, 1)[0].slice(0, 500),
          },
        })
      } catch (manifestError) {
        console.error(`Failed to finalize blocked run manifest: ${manifestError.message}`)
      }
    }
    console.error(message)
    process.exitCode = error?.exitCode || 1
  }
}

await main()
