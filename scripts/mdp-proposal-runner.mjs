#!/usr/bin/env node
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { basename, dirname, extname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const RUNNER_CONTRACT = 'mdp.proposal-runner.v0'
const RESULT_CONTRACT = 'mdp.proposal-runner-result.v0'
const TOOLS_CONTRACT = 'mdp.proposal-runner-tools.v0'
const SOURCE_AUDIT_CONTRACT = 'mdp.source-audit.v0'
const REQUEST_CONTRACT = 'mdp.native-normalize-request.v0'
const PROMPT_OUTPUT_CONTRACT = 'mdp.prompt-output.v0'
const DEFAULT_PROMPT_ID = 'normalize-opportunity'
const DEFAULT_SOURCE_KIND = 'private-scratch-opportunity'
const DEFAULT_MAX_SOURCE_BYTES = 12000
const MAX_CONTEXT_CHARS = 20000
const MAX_SNIPPET_CHARS = 500
const TEXT_EXTENSIONS = new Set(['.txt', '.md', '.markdown', '.csv', '.json', '.yaml', '.yml'])

const scriptDir = dirname(fileURLToPath(import.meta.url))
const bundleRoot = resolve(scriptDir, '..')

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
  validate-prompt-output and run-receipt. It is not a hosted MCP server and does
  not parse PDFs, read .env files, create API keys, submit proposals, or approve
  compliance.

Options:
  --pack PATH              Proposal MDP pack root. Required.
  --workdir PATH           Empty/customer-controlled run directory. Required.
  --source PATH            Text/Markdown/CSV/JSON/YAML source file. Repeatable.
  --source-audit PATH      Existing mdp.source-audit.v0 JSON to preserve.
  --source-id ID           .mdp/sources.yaml source id for generated source-audit refs.
  --source-kind KIND       Prompt source_kind. Defaults to ${DEFAULT_SOURCE_KIND}.
  --model MODEL            Model id. Required for real native calls; defaults to gpt-test for dry/mock.
  --mock-response PATH     Offline provider response fixture for native runner tests.
  --dry-run                Validate request shape only; no model output, receipt, fit, or route.
  --mdp-bin PATH           mdp executable path. Defaults to source cargo run when available, else mdp.
  --native-runner PATH     Native runner script. Defaults to adjacent mdp-native-normalize-openai.mjs.
  --prompt-id ID           Prompt id. Currently only normalize-opportunity is supported.
  --allow-existing         Allow writing into an existing non-empty workdir without deleting it.
  --skip-review            Skip fit/route review-support probes after receipt.
  --require-audit-grade    Exit nonzero unless run-receipt returns decision audit-grade.
  --max-source-bytes N     Per-source bounded text bytes to include in prompt payload.
`.trim()

const fail = (message, code = 1) => {
  console.error(message)
  process.exit(code)
}

const parseArgs = (argv) => {
  const command = argv[0] || 'help'
  const args = {
    command,
    pack: null,
    workdir: null,
    sources: [],
    sourceAudit: null,
    sourceId: null,
    sourceKind: DEFAULT_SOURCE_KIND,
    model: null,
    mockResponse: null,
    dryRun: false,
    mdpBin: null,
    nativeRunner: null,
    promptId: DEFAULT_PROMPT_ID,
    allowExisting: false,
    skipReview: false,
    requireAuditGrade: false,
    maxSourceBytes: DEFAULT_MAX_SOURCE_BYTES,
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
      case '--workdir':
        args.workdir = next(index, flag)
        index += 1
        break
      case '--source':
        args.sources.push(next(index, flag))
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
      case '--dry-run':
        args.dryRun = true
        break
      case '--allow-existing':
        args.allowExisting = true
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

const toolEnvelope = () => ({
  contract: TOOLS_CONTRACT,
  runner_contract: RUNNER_CONTRACT,
  note: 'These are host-neutral local runner steps that a future MCP server can wrap. They are not currently a hosted MCP implementation.',
  tools: [
    {
      name: 'mdp_intake_sources',
      mode: 'local-files',
      boundary: 'customer-controlled workdir',
      purpose: 'Stage supplied text/csv/markdown/json/yaml files and preserve or create mdp.source-audit.v0 refs.',
    },
    {
      name: 'mdp_normalize_opportunity',
      mode: 'native-api',
      boundary: 'fresh/stateless model request with declared prompt inputs only',
      purpose: 'Build mdp.native-normalize-request.v0 and call the optional BYOK native runner.',
    },
    {
      name: 'mdp_validate_normalization',
      mode: 'cli',
      boundary: 'deterministic local validation',
      purpose: 'Run mdp validate-prompt-output --source-audit and retain artifact hashes.',
    },
    {
      name: 'mdp_run_receipt',
      mode: 'cli',
      boundary: 'deterministic local receipt gate',
      purpose: 'Run mdp run-receipt --require-runner-audit to bind prompt output, validation, source audit, and runner audit.',
    },
    {
      name: 'mdp_review_proposal',
      mode: 'cli',
      boundary: 'review support only',
      purpose: 'Optionally run fit/route probes after the receipt; does not write, certify, approve, or submit proposals.',
    },
  ],
})

const sha256Buffer = (bytes) => createHash('sha256').update(bytes).digest('hex')
const sha256File = (path) => sha256Buffer(readFileSync(path))

const readJson = (path) => {
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch (error) {
    fail(`${path} must contain valid JSON: ${error.message}`)
  }
}

const maybeReadJson = (path) => {
  if (!existsSync(path)) return null
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch {
    return null
  }
}

const writeJson = (path, value) => {
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`)
}

const writeText = (path, value) => {
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, value)
}

const readTextExcerpt = (path, maxChars = MAX_CONTEXT_CHARS) => {
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

const assertFile = (path, label) => {
  if (!existsSync(path)) fail(`${label} not found: ${path}`)
  if (!statSync(path).isFile()) fail(`${label} must be a file: ${path}`)
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

const prepareWorkdir = (workdir, allowExisting) => {
  const resolved = resolve(workdir)
  if (existsSync(resolved)) {
    const entries = readdirSync(resolved)
    if (entries.length > 0 && !allowExisting) {
      fail(`Workdir already exists and is not empty: ${resolved}\nPass --allow-existing only when this is an intended customer-controlled scratch directory.`)
    }
  }
  mkdirSync(resolved, { recursive: true })
  mkdirSync(join(resolved, 'artifacts'), { recursive: true })
  mkdirSync(join(resolved, 'sources'), { recursive: true })
  return resolved
}

const stageSources = (sources, workdir, maxSourceBytes) => {
  const staged = []
  const sourcesDir = join(workdir, 'sources')
  sources.forEach((source, index) => {
    const absolute = resolve(source)
    assertFile(absolute, 'source')
    validateTextSource(absolute)
    const bytes = readFileSync(absolute)
    const stagedName = `${String(index + 1).padStart(2, '0')}-${safeBasename(absolute)}`
    const stagedPath = join(sourcesDir, stagedName)
    copyFileSync(absolute, stagedPath)
    const excerptBytes = bytes.subarray(0, maxSourceBytes)
    const text = excerptBytes.toString('utf8')
    staged.push({
      index,
      original_path: absolute,
      filename: basename(absolute),
      staged_path: relative(workdir, stagedPath),
      sha256: sha256Buffer(bytes),
      byte_count: bytes.length,
      truncated: bytes.length > maxSourceBytes,
      text,
    })
  })
  return staged
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

const resolveMdpCommand = (mdpBin) => {
  const fromArg = mdpBin || process.env.MDP_BIN
  if (fromArg) {
    if (/\s/.test(fromArg)) {
      fail('MDP_BIN/--mdp-bin must be an executable path without spaces; use a wrapper script for multi-argument commands.')
    }
    return [fromArg]
  }

  const cargoManifest = join(bundleRoot, 'cli', 'Cargo.toml')
  if (existsSync(cargoManifest)) {
    return ['cargo', 'run', '--quiet', '--manifest-path', cargoManifest, '--']
  }
  return ['mdp']
}

const runProcess = ({ command, args, stdoutPath, stderrPath, allowNonZero = false }) => {
  const result = spawnSync(command[0], [...command.slice(1), ...args], {
    encoding: 'utf8',
    env: process.env,
    maxBuffer: 20 * 1024 * 1024,
  })
  if (stdoutPath) writeText(stdoutPath, result.stdout || '')
  if (stderrPath) writeText(stderrPath, result.stderr || '')
  const status = result.status ?? 1
  if (result.error) {
    fail(`Failed to run ${command[0]}: ${result.error.message}`)
  }
  if (status !== 0 && !allowNonZero) {
    fail(`Command failed (${status}): ${[...command, ...args].join(' ')}\n${result.stderr || result.stdout}`)
  }
  return {
    status,
    stdout: result.stdout || '',
    stderr: result.stderr || '',
  }
}

const missingRequiredTraceSchema = () => ({
  type: 'array',
  items: {
    anyOf: [
      { type: 'string' },
      {
        type: 'object',
        additionalProperties: false,
        required: ['field', 'path', 'reason', 'source_evidence'],
        properties: {
          field: { type: 'string' },
          path: { type: 'string' },
          reason: {
            type: 'string',
            description:
              'Why the field is absent, such as not_available_in_source, not_extractable_from_source, not_extractable_without_person, or invalid_out_of_contract.',
          },
          source_evidence: {
            type: 'string',
            description: 'Short source-backed explanation of what was missing or why it could not be extracted.',
          },
        },
      },
    ],
  },
})

const promptOutputSchema = () => {
  const normalizedEntity = {
    type: 'object',
    additionalProperties: false,
    required: [
      'name',
      'title',
      'company',
      'company_domain',
      'source_kind',
      'synthetic',
      'background',
      'trigger',
      'persona',
      'segment',
      'attributes',
      'signals',
    ],
    properties: {
      name: { type: 'string' },
      title: { type: 'string' },
      company: { type: 'string' },
      company_domain: { type: 'string' },
      source_kind: {
        enum: [
          'user-provided-opportunity',
          'private-scratch-opportunity',
          'public-source',
          'sanitized-example',
          'synthetic-example',
        ],
      },
      synthetic: { type: 'boolean' },
      background: { type: 'string' },
      trigger: { type: 'string' },
      persona: { type: 'string' },
      segment: { enum: ['municipal-modernization', 'public-services-review'] },
      attributes: {
        type: 'object',
        additionalProperties: false,
        required: ['source_safety'],
        properties: {
          source_safety: { enum: ['synthetic', 'sanitized', 'private-scratch', 'public-source', 'user-approved-local'] },
        },
      },
      signals: {
        type: 'array',
        items: {
          type: 'object',
          additionalProperties: false,
          required: ['id', 'title', 'source', 'confidence', 'freshness', 'state_as'],
          properties: {
            id: { type: 'string' },
            title: { type: 'string' },
            source: { type: 'string' },
            confidence: { enum: ['high', 'medium', 'low', 'unknown'] },
            freshness: { type: 'string' },
            state_as: { enum: ['observed', 'supplied', 'hypothesis', 'gap', 'unknown'] },
          },
        },
      },
    },
  }

  return {
    type: 'object',
    additionalProperties: false,
    required: [
      'contract',
      'prompt_id',
      'source_summary',
      'normalized_prospect',
      'normalization_trace',
      'card_patches',
      'gaps',
      'rejected_claims',
    ],
    properties: {
      contract: { enum: [PROMPT_OUTPUT_CONTRACT] },
      prompt_id: { enum: [DEFAULT_PROMPT_ID] },
      source_summary: {
        type: 'object',
        additionalProperties: false,
        required: [
          'company_domain',
          'company_name',
          'person_name',
          'person_title',
          'account_name',
          'inputs_used',
          'confidence',
        ],
        properties: {
          company_domain: { type: 'string' },
          company_name: { type: 'string' },
          person_name: { type: 'string' },
          person_title: { type: 'string' },
          account_name: { type: 'string' },
          inputs_used: {
            type: 'array',
            items: {
              enum: [
                'raw_opportunity',
                'existing_pack_context',
                'runtime_context',
                'source_audit',
                'source_kind',
              ],
            },
          },
          confidence: { enum: ['high', 'medium', 'low', 'unknown'] },
        },
      },
      normalized_prospect: normalizedEntity,
      normalization_trace: {
        type: 'object',
        additionalProperties: false,
        required: ['persona', 'fit_readiness', 'preserved_raw_fields', 'missing_required'],
        properties: {
          persona: {
            type: 'object',
            additionalProperties: false,
            required: ['source', 'matched_keywords', 'confidence', 'needs_review'],
            properties: {
              source: { type: 'string' },
              matched_keywords: { type: 'array', items: { type: 'string' } },
              confidence: { enum: ['high', 'medium', 'low', 'unknown'] },
              needs_review: { type: 'boolean' },
            },
          },
          fit_readiness: {
            type: 'object',
            additionalProperties: false,
            required: [
              'has_customer_or_agency',
              'has_due_date',
              'has_requirement_signal',
              'has_review_mode',
              'has_signal_source',
              'ready_for_mdp_fit',
            ],
            properties: {
              has_customer_or_agency: { type: 'boolean' },
              has_due_date: { type: 'boolean' },
              has_requirement_signal: { type: 'boolean' },
              has_review_mode: { type: 'boolean' },
              has_signal_source: { type: 'boolean' },
              ready_for_mdp_fit: { type: 'boolean' },
            },
          },
          preserved_raw_fields: { type: 'array', items: { type: 'string' } },
          missing_required: missingRequiredTraceSchema(),
        },
      },
      card_patches: {
        type: 'array',
        items: {
          type: 'object',
          additionalProperties: false,
          required: [],
          properties: {},
        },
      },
      gaps: { type: 'array', items: { type: 'string' } },
      rejected_claims: {
        type: 'array',
        items: {
          type: 'object',
          additionalProperties: false,
          required: ['claim', 'source', 'reason'],
          properties: {
            claim: { type: 'string' },
            source: { type: 'string' },
            reason: { type: 'string' },
          },
        },
      },
    },
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

const buildRequest = ({ args, packRoot, promptPath, sourceAudit, stagedSources }) => {
  const rawOpportunity =
    stagedSources.length > 0
      ? {
          source_shape: 'bounded-local-text-excerpts',
          sources: stagedSources.map((source) => ({
            ref: `raw_opportunity.sources[${source.index}]`,
            filename: source.filename,
            staged_path: source.staged_path,
            sha256: source.sha256,
            byte_count: source.byte_count,
            truncated: source.truncated,
            text: source.text,
          })),
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

const run = (args) => {
  if (!args.pack) fail(`Missing --pack\n\n${usage()}`)
  if (!args.workdir) fail(`Missing --workdir\n\n${usage()}`)
  if (args.promptId !== DEFAULT_PROMPT_ID) {
    fail(`This proposal runner currently supports only --prompt-id ${DEFAULT_PROMPT_ID}`)
  }
  const packRoot = resolve(args.pack)
  if (!existsSync(join(packRoot, '.mdp'))) fail(`Pack root must contain .mdp/: ${packRoot}`)
  const promptPath = join(packRoot, '.mdp', 'prompts', `${args.promptId}.yaml`)
  assertFile(promptPath, 'prompt contract')
  if (!args.sourceAudit && args.sources.length === 0) {
    fail('Pass at least one --source text file or a prebuilt --source-audit JSON.')
  }
  if (!args.dryRun && !args.mockResponse && !args.model) {
    fail('Real native runs require --model. Dry-run/mock modes default to gpt-test.')
  }
  if (!args.dryRun && !args.mockResponse && args.sources.length === 0) {
    fail('Real native runs require at least one --source text file so the model boundary receives approved source material, not only a source-audit summary.')
  }

  const nativeRunner = resolve(args.nativeRunner || join(scriptDir, 'mdp-native-normalize-openai.mjs'))
  assertFile(nativeRunner, 'native runner')
  if (args.sourceAudit) assertFile(resolve(args.sourceAudit), 'source audit')
  if (args.mockResponse) assertFile(resolve(args.mockResponse), 'mock response')

  const workdir = prepareWorkdir(args.workdir, args.allowExisting)
  const artifactsDir = join(workdir, 'artifacts')
  const paths = {
    sourceAudit: join(artifactsDir, 'source-audit.json'),
    request: join(artifactsDir, 'native-normalize-request.json'),
    nativeDryRun: join(artifactsDir, 'native-normalize-dry-run.json'),
    nativeResult: join(artifactsDir, 'native-normalize-result.json'),
    nativeStderr: join(artifactsDir, 'native-normalize.stderr'),
    promptOutput: join(artifactsDir, 'normalize-opportunity-output.json'),
    runnerAudit: join(artifactsDir, 'runner-audit.json'),
    validation: join(artifactsDir, 'normalize-opportunity-validation.json'),
    validationStderr: join(artifactsDir, 'normalize-opportunity-validation.stderr'),
    receipt: join(artifactsDir, 'run-receipt.json'),
    receiptStdout: join(artifactsDir, 'run-receipt.stdout.json'),
    receiptStderr: join(artifactsDir, 'run-receipt.stderr'),
    normalized: join(artifactsDir, 'normalized-opportunity.json'),
    fit: join(artifactsDir, 'fit-normalized-opportunity.json'),
    fitStderr: join(artifactsDir, 'fit-normalized-opportunity.stderr'),
    routeBidNoBid: join(artifactsDir, 'route-bid-no-bid-review.json'),
    routeBidNoBidStderr: join(artifactsDir, 'route-bid-no-bid-review.stderr'),
    result: join(artifactsDir, 'proposal-runner-result.json'),
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

  const request = buildRequest({ args, packRoot, promptPath, sourceAudit, stagedSources })
  writeJson(paths.request, request)

  const steps = [
    {
      name: 'mdp_intake_sources',
      status: 'ok',
      artifacts: { source_audit: paths.sourceAudit },
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

  const nativeArgs = ['--request', paths.request]
  if (args.dryRun) {
    const dryRun = runProcess({
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
      workdir,
      artifacts: paths,
      steps,
      caveats: [
        'Dry-run validates the native request shape only; it does not produce prompt-output, runner-audit, validation, receipt, or proposal review artifacts.',
      ],
    }
    writeJson(paths.result, result)
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
  const nativeResult = runProcess({
    command: ['node', nativeRunner],
    args: normalizeArgs,
    stdoutPath: paths.nativeResult,
    stderrPath: paths.nativeStderr,
  })
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

  const mdpCommand = resolveMdpCommand(args.mdpBin)
  const validation = runProcess({
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
  const validationData = parseCliData(paths.validation)
  steps.push({
    name: 'mdp_validate_normalization',
    status: validationData?.valid ? 'ok' : 'blocked',
    artifacts: { validation: paths.validation },
    exit_status: validation.status,
    issue_count: validationData?.issues?.length ?? null,
  })

  const receipt = runProcess({
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
      '--out',
      paths.receipt,
    ],
    stdoutPath: paths.receiptStdout,
    stderrPath: paths.receiptStderr,
    allowNonZero: true,
  })
  const receiptData = maybeReadJson(paths.receipt) || parseCliData(paths.receiptStdout)
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

  if (!args.skipReview) {
    const promptOutput = maybeReadJson(paths.promptOutput)
    if (promptOutput?.normalized_prospect) {
      writeJson(paths.normalized, promptOutput.normalized_prospect)
      const fit = runProcess({
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
      const route = runProcess({
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
  }

  const decision = receiptData?.decision ?? 'blocked'
  const runnerAssurance = receiptData?.runner?.assurance ?? 'unknown'
  const mode = args.mockResponse ? 'mock' : 'native'
  const result = {
    contract: RESULT_CONTRACT,
    runner_contract: RUNNER_CONTRACT,
    mode,
    ok: decision === 'audit-grade',
    audit_grade_eligible: mode === 'native' && decision === 'audit-grade',
    decision,
    runner_assurance: runnerAssurance,
    workdir,
    artifacts: paths,
    steps,
    caveats: [
      mode === 'mock'
        ? 'Mock mode is offline-only and must not be described as audit-grade model isolation.'
        : 'Native mode is audit-grade only when run-receipt returns decision audit-grade with stateless-api-verified or headless-verified assurance.',
      'This runner stages bounded local text and source-audit artifacts; it does not prove PDF/OCR quality, semantic truth beyond supplied artifacts, compliance status, legal approval, or proposal submission readiness.',
      'The current surface is a host-neutral local runner command set that a future MCP server can wrap; it is not itself a hosted MCP server.',
    ],
  }
  writeJson(paths.result, result)
  console.log(JSON.stringify(result, null, 2))
  if (args.requireAuditGrade && decision !== 'audit-grade') {
    process.exit(2)
  }
}

const main = () => {
  const args = parseArgs(process.argv.slice(2))
  if (args.command === 'help') {
    console.log(usage())
    return
  }
  if (args.command === 'tools') {
    console.log(JSON.stringify(toolEnvelope(), null, 2))
    return
  }
  run(args)
}

main()
