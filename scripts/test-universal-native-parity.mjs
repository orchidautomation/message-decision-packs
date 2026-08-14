#!/usr/bin/env node

import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  DRIVER_REQUEST_CONTRACT,
  DRIVER_RESULT_CONTRACT,
  sha256CanonicalJson,
} from './mdp-native-model-openai.mjs'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const mdp = process.env.MDP_BIN || join(repoRoot, 'cli', 'target', 'debug', 'mdp')
const driver = join(repoRoot, 'scripts', 'mdp-native-model-openai.mjs')
const legacyDriver = join(repoRoot, 'scripts', 'mdp-native-normalize-openai.mjs')
const scratch = mkdtempSync(join(tmpdir(), 'mdp-universal-native-parity-'))

const profiles = [
  {
    profile: 'gtm',
    pack: join(repoRoot, 'plugin', 'assets', 'templates', 'basic'),
    jobs: {
      'prospect-fit-or-brief': [['normalization', 'normalize-prospect-row']],
      'outbound-copy-brief': [
        ['normalization', 'normalize-prospect-row'],
        ['generation', 'generate-outbound-copy-v1'],
      ],
      'outbound-copy-review': [
        ['normalization', 'normalize-prospect-row'],
        ['review', 'review-outbound-copy-v1'],
      ],
    },
  },
  {
    profile: 'proposal',
    pack: join(repoRoot, 'plugin', 'assets', 'templates', 'proposal'),
    jobs: {
      'bid-no-bid-review': [
        ['normalization', 'normalize-opportunity'],
        ['review', 'review-bid-no-bid-v1'],
      ],
      'compliance-review': [
        ['normalization', 'normalize-opportunity'],
        ['review', 'review-proposal-compliance-v1'],
      ],
      'proof-review': [
        ['normalization', 'normalize-opportunity'],
        ['review', 'review-proposal-proof-v1'],
      ],
      'red-team-review': [
        ['normalization', 'normalize-opportunity'],
        ['review', 'review-proposal-red-team-v1'],
      ],
    },
  },
]

const invoke = (command, args, options = {}) => spawnSync(command, args, {
  cwd: repoRoot,
  encoding: 'utf8',
  maxBuffer: 16 * 1024 * 1024,
  env: options.env || { PATH: process.env.PATH || '' },
  input: options.input,
})

const expectJson = (result, label) => {
  assert.equal(result.status, 0, `${label} failed\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`)
  assert.equal(result.stderr, '', `${label} wrote unexpected stderr`)
  return JSON.parse(result.stdout)
}

try {
  const bindings = []
  const uniquePrompts = new Map()
  const publicPromptOutputSchema = expectJson(
    invoke(mdp, ['--json', 'schema', 'prompt-output']),
    'public prompt-output schema',
  ).data
  const resolvedOutputSchema = (step) => {
    if (step.output_contract.schema) return step.output_contract.schema
    assert.equal(
      step.output_contract.schema_ref,
      'mdp.prompt-output.prospect-normalization.v0',
      `${step.prompt_id} uses an unsupported shipped output schema ref`,
    )
    return publicPromptOutputSchema
  }
  const schemaFromExample = (value) => {
    if (value === null) return { type: 'null' }
    if (Array.isArray(value)) return { type: 'array', items: value.length > 0 ? schemaFromExample(value[0]) : {} }
    if (typeof value === 'object') {
      return {
        type: 'object',
        properties: Object.fromEntries(Object.entries(value).map(([field, child]) => [field, schemaFromExample(child)])),
        required: Object.keys(value).sort(),
        additionalProperties: false,
      }
    }
    if (typeof value === 'boolean') return { type: 'boolean' }
    if (typeof value === 'number') return { type: Number.isInteger(value) ? 'integer' : 'number' }
    return { type: 'string' }
  }
  const schemaForExampleShape = (schema, example) => {
    if (Array.isArray(example) && schema?.type === 'array') {
      return example.length > 0 && schema.items
        ? { ...structuredClone(schema), items: schemaForExampleShape(schema.items, example[0]) }
        : structuredClone(schema)
    }
    if (!example || typeof example !== 'object' || Array.isArray(example)) return structuredClone(schema)
    const properties = schema?.properties
    if (!properties || Object.keys(example).some((field) => !(field in properties))) return schemaFromExample(example)
    return {
      ...structuredClone(schema),
      properties: Object.fromEntries(
        Object.entries(example).map(([field, child]) => [field, schemaForExampleShape(properties[field], child)]),
      ),
      required: Object.keys(example).sort(),
    }
  }
  const providerSchemaForStep = (step) => {
    const canonical = structuredClone(resolvedOutputSchema(step))
    assert.equal(canonical.type, 'object')
    assert.ok(canonical.properties && typeof canonical.properties === 'object')
    const required = step.output_contract.required_top_level
    assert.ok(Array.isArray(required) && required.length > 0)
    canonical.properties = Object.fromEntries(required.map((field) => {
      assert.ok(field in canonical.properties, `${step.prompt_id} requires ${field} outside its canonical schema`)
      return [field, canonical.properties[field]]
    }))
    canonical.required = required
    const requiredExample = Object.fromEntries(
      required.map((field) => {
        assert.ok(field in step.output_contract.example, `${step.prompt_id} example omits required ${field}`)
        return [field, step.output_contract.example[field]]
      }),
    )
    return step.output_contract.schema_ref
      ? schemaForExampleShape(canonical, requiredExample)
      : canonical
  }

  for (const profile of profiles) {
    for (const [jobId, expected] of Object.entries(profile.jobs)) {
      const envelope = expectJson(
        invoke(mdp, ['--json', 'requirements', '--dir', profile.pack, '--job', jobId]),
        `${profile.profile}/${jobId} requirements`,
      )
      assert.equal(envelope.ok, true)
      assert.equal(envelope.command, 'requirements')
      const resolution = envelope.data.model_steps
      assert.equal(resolution.contract, 'mdp.model-step-resolution.v1')
      assert.equal(resolution.status, 'ready')
      assert.equal(resolution.job_id, jobId)
      assert.deepEqual(
        resolution.steps.map((step) => [step.phase, step.prompt_id]),
        expected,
        `${profile.profile}/${jobId} resolved an unexpected model-step sequence`,
      )

      for (const step of resolution.steps) {
        assert.equal(step.contract, 'mdp.compiled-model-step.v1')
        assert.equal(step.step_id, `model:${jobId}/${step.phase}`)
        assert.match(step.prompt_sha256, /^[0-9a-f]{64}$/)
        assert.match(step.output_contract_sha256, /^[0-9a-f]{64}$/)
        assert.equal(step.output_contract.strict_json_only, true)
        assert.ok(step.output_contract.example && typeof step.output_contract.example === 'object')
        assert.ok(
          (step.output_contract.schema && typeof step.output_contract.schema === 'object') ||
            (typeof step.output_contract.schema_ref === 'string' && step.output_contract.schema_ref.length > 0),
          `${step.prompt_id} has neither an inline schema nor a schema_ref`,
        )
        bindings.push({ profile: profile.profile, jobId, step })

        const prior = uniquePrompts.get(step.prompt_id)
        const authority = {
          prompt_sha256: step.prompt_sha256,
          output_contract_sha256: step.output_contract_sha256,
        }
        if (prior) assert.deepEqual(authority, prior, `${step.prompt_id} changed across job bindings`)
        else uniquePrompts.set(step.prompt_id, authority)
      }
    }
  }

  assert.equal(Object.values(profiles[0].jobs).length + Object.values(profiles[1].jobs).length, 7)
  assert.equal(bindings.length, 13)
  assert.equal(uniquePrompts.size, 8)

  for (const [index, binding] of bindings.entries()) {
    const { profile, jobId, step } = binding
    const outputSchema = providerSchemaForStep(step)
    const providerExample = Object.fromEntries(
      step.output_contract.required_top_level.map((field) => [field, step.output_contract.example[field]]),
    )
    const output = JSON.stringify(providerExample)
    const request = {
      contract: DRIVER_REQUEST_CONTRACT,
      execution_id: `parity-${profile}-${index + 1}`,
      provider: 'openai',
      model: 'gpt-test',
      prompt_id: step.prompt_id,
      declared_inputs_only: true,
      input: [{
        role: 'user',
        content: JSON.stringify({
          contract: 'mdp.synthetic-model-input.v1',
          job_id: jobId,
          operation: step.step_id,
        }),
      }],
      output_schema: outputSchema,
      output_schema_sha256: sha256CanonicalJson(outputSchema),
      max_output_tokens: 4096,
      timeout_ms: 30_000,
    }
    const mockPath = join(scratch, `mock-${index + 1}.json`)
    writeFileSync(mockPath, JSON.stringify({
      id: `resp_parity_${index + 1}`,
      model: 'gpt-test-synthetic',
      status: 'completed',
      output: [{ type: 'message', content: [{ type: 'output_text', text: output }] }],
    }))
    const result = expectJson(
      invoke(process.execPath, [driver, '--mock-response', mockPath], { input: JSON.stringify(request) }),
      `${profile}/${jobId}/${step.phase} universal subprocess`,
    )
    assert.equal(result.contract, DRIVER_RESULT_CONTRACT)
    assert.equal(result.execution_id, request.execution_id)
    assert.equal(result.terminal_state, 'success')
    assert.deepEqual(JSON.parse(result.output.content), providerExample)
    assert.ok(!JSON.stringify(result).includes('OPENAI_API_KEY'))
  }

  for (const binding of [bindings.find(({ profile }) => profile === 'gtm'), bindings.find(({ profile }) => profile === 'proposal')]) {
    const { profile, jobId, step } = binding
    const outputSchema = providerSchemaForStep(step)
    const request = {
      contract: DRIVER_REQUEST_CONTRACT,
      execution_id: `parity-${profile}-dry-run`,
      provider: 'openai',
      model: 'gpt-test',
      prompt_id: step.prompt_id,
      declared_inputs_only: true,
      input: [{ role: 'user', content: JSON.stringify({ job_id: jobId, operation: step.step_id }) }],
      output_schema: outputSchema,
      output_schema_sha256: sha256CanonicalJson(outputSchema),
    }
    const result = expectJson(
      invoke(process.execPath, [driver, '--dry-run'], { input: JSON.stringify(request) }),
      `${profile} key-free no-draft`,
    )
    assert.equal(result.terminal_state, 'no-draft:policy-blocked')
    assert.equal(result.diagnostic_code, 'dry_run_complete')
    assert.equal(result.output, null)
  }

  const proposalNormalization = bindings.find(
    ({ profile, step }) => profile === 'proposal' && step.phase === 'normalization',
  ).step
  const legacyRequestPath = join(scratch, 'legacy-request.json')
  writeFileSync(legacyRequestPath, JSON.stringify({
    contract: 'mdp.native-normalize-request.v0',
    provider: 'openai',
    model: 'gpt-test',
    prompt_id: proposalNormalization.prompt_id,
    declared_inputs_only: true,
    input: [{ role: 'user', content: '{"raw_opportunity":"synthetic"}' }],
    prompt_output_schema: resolvedOutputSchema(proposalNormalization),
  }))
  const legacy = expectJson(
    invoke(process.execPath, [legacyDriver, '--request', legacyRequestPath, '--dry-run']),
    'legacy proposal normalization adapter',
  )
  assert.equal(legacy.contract, 'mdp.native-normalize-dry-run.v0')
  assert.equal(legacy.delegated_contract, DRIVER_REQUEST_CONTRACT)
  assert.equal(legacy.endpoint_policy, 'official-fixed')
  assert.equal(legacy.requires_api_key_for_real_run, true)
  assert.equal(legacy.requires_native_call_permission_for_real_run, true)

  process.stdout.write(`${JSON.stringify({
    ok: true,
    contract: 'mdp.universal-native-parity-test.v1',
    profiles: profiles.map(({ profile }) => profile),
    jobs: 7,
    model_step_bindings: bindings.length,
    unique_prompts: uniquePrompts.size,
    shell_adapter: 'profile-neutral-subprocess',
    mcp_adapter_proof: 'scripts/test-run-mcp-server.mjs',
    legacy_proposal_adapter: 'delegates',
    live_provider_calls: 0,
  })}\n`)
} finally {
  rmSync(scratch, { recursive: true, force: true })
}
