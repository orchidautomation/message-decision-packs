#!/usr/bin/env node
import { createHash } from 'node:crypto'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'

const usage = `Usage: node write-demo-runner-audit.mjs --prompt-output OUTPUT.json --out RUNNER_AUDIT.json`

const args = { promptOutput: null, out: null }
for (let i = 2; i < process.argv.length; i += 1) {
  const arg = process.argv[i]
  const next = () => {
    i += 1
    if (i >= process.argv.length) throw new Error(`Missing value for ${arg}`)
    return process.argv[i]
  }
  if (arg === '--prompt-output') args.promptOutput = next()
  else if (arg === '--out') args.out = next()
  else if (arg === '-h' || arg === '--help') {
    console.log(usage)
    process.exit(0)
  } else {
    throw new Error(`Unknown argument: ${arg}\n${usage}`)
  }
}

if (!args.promptOutput || !args.out) throw new Error(usage)

const sha256File = (path) => createHash('sha256').update(readFileSync(path)).digest('hex')

const audit = {
  contract: 'mdp.runner-audit.v0',
  runner: 'custom-headless',
  model: 'synthetic-mcp-fixture',
  isolated_invocation: true,
  conversation_resume: false,
  declared_inputs_only: true,
  output_schema_used: true,
  prompt_id: 'normalize-opportunity',
  prompt_output_sha256: sha256File(args.promptOutput),
  tool_invocations_observed: 0,
  session_persistence: false,
  tools_disabled: true,
  demo_fixture: true,
  notes: [
    'Synthetic video fixture that emulates the runner/MCP audit contract for public demo data.',
    'Do not use this hand-authored fixture as production model-isolation evidence; replace it with a native/headless runner or MCP-produced mdp.runner-audit.v0 artifact.'
  ]
}

mkdirSync(dirname(resolve(args.out)), { recursive: true })
writeFileSync(args.out, `${JSON.stringify(audit, null, 2)}\n`)
console.log(JSON.stringify({ ok: true, runner_audit: args.out, prompt_output_sha256: audit.prompt_output_sha256 }, null, 2))
