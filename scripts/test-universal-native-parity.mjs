#!/usr/bin/env node

import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  buildProviderRequestBody,
  buildModelParametersProjection,
  canonicalJsonBytes,
  DRIVER_REQUEST_CONTRACT,
  DRIVER_RESULT_CONTRACT,
  projectOutputSchemaForOpenAI,
  PROVIDER_REQUEST_SCHEMA_ID,
  sha256Bytes,
  sha256CanonicalJson,
} from './mdp-native-model-openai.mjs'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const runtimeVersion = process.env.MDP_RUNTIME_VERSION
  || (existsSync(join(repoRoot, 'cli', 'Cargo.toml'))
    ? readFileSync(join(repoRoot, 'cli', 'Cargo.toml'), 'utf8').match(/^version = "([^"]+)"/m)?.[1]
    : undefined)
if (!runtimeVersion) throw new Error('unable to read CLI runtime version')
const mdp = process.env.MDP_BIN || join(repoRoot, 'cli', 'target', 'debug', 'mdp')
const driver = join(repoRoot, 'scripts', 'mdp-native-model-openai.mjs')
const legacyDriver = join(repoRoot, 'scripts', 'mdp-native-normalize-openai.mjs')
const scratch = mkdtempSync(join(tmpdir(), 'mdp-universal-native-parity-'))

const profiles = [
  {
    profile: 'gtm',
    pack: process.env.MDP_PARITY_GTM_PACK || join(repoRoot, 'plugin', 'assets', 'templates', 'basic'),
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
    pack: process.env.MDP_PARITY_PROPOSAL_PACK || join(repoRoot, 'plugin', 'assets', 'templates', 'proposal'),
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

const parseJsonResult = (result, label) => {
  assert.equal(result.stderr, '', `${label} wrote unexpected stderr`)
  assert.ok([0, 1, 2].includes(result.status), `${label} failed\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`)
  return JSON.parse(result.stdout)
}

const sha256File = (path) => sha256Bytes(readFileSync(path))
const authorityHash = (domain, value) => sha256Bytes(`${domain}\0${canonicalJsonBytes(value)}`)
const providerMaxOutputTokens = (maxOutputBytes) => Math.min(100000, Math.max(1, Math.floor(maxOutputBytes / 4)))
const nativeVisibleInput = (step, promptContent, invocationContent, inputs) => {
  let value = `<mdp-prompt id="${step.prompt_id}" version="${step.prompt_version}" canonical_sha256="${step.prompt_sha256}">\n`
  value += `${promptContent}\n</mdp-prompt>\n<mdp-invocation sha256="${sha256Bytes(invocationContent)}">\n`
  value += `${invocationContent}\n</mdp-invocation>\n<mdp-host-input name="prompt_receipt">\n`
  value += `${invocationContent}\n</mdp-host-input>\n<mdp-host-input name="invocation_receipt_sha256">\n`
  value += `${sha256Bytes(invocationContent)}\n</mdp-host-input>\n`
  for (const input of inputs) {
    value += `<mdp-declared-input name="${input.logical_name}" sha256="${sha256File(input.source_path)}">\n`
    value += `${readFileSync(input.source_path, 'utf8')}\n</mdp-declared-input>\n`
  }
  return value
}
const driverConfigurationProjection = (driverSourceSha256, nodeSha256) => ({
  contract: 'mdp.driver-configuration.v1',
  driver_id: 'mdp-native-openai',
  implementation: 'bundled:mdp-native-model-openai',
  runtime_version: runtimeVersion,
  bundled_source_sha256: driverSourceSha256,
  node_executable_sha256: nodeSha256,
  native_request_contract: 'mdp.native-model-subprocess-request.v1',
  native_result_contract: 'mdp.native-model-subprocess-result.v1',
  clear_env: true,
  allowlisted_environment_names: ['MDP_ALLOW_NATIVE_MODEL_CALLS', 'OPENAI_API_KEY'],
  filesystem_mode: 'private-staging',
  stdin_mode: 'bounded-json',
  stdout_mode: 'bounded-json-result',
  max_request_bytes: 2 * 1024 * 1024,
  max_response_bytes: 6 * 1024 * 1024 + 64 * 1024,
  timeout_enforced: true,
  authorized_endpoint: 'https://api.openai.com/v1/responses',
  redirect_policy: 'reject',
  proxy_policy: 'excluded',
  storage_policy: 'store-false',
  tool_policy: 'none',
})

try {
  const bindings = []
  const uniquePrompts = new Map()
  const publicPromptOutputSchema = expectJson(
    invoke(mdp, ['--json', 'schema', 'prompt-output']),
    'public prompt-output schema',
  ).data
  const resolvedOutputSchema = (step, normalizedOutputSchema) => {
    if (step.output_contract.schema) return step.output_contract.schema
    if (['mdp.normalized-decision-input.v1', 'mdp.normalized-decision-input.v2'].includes(step.output_contract.schema_ref)) {
      assert.ok(normalizedOutputSchema && typeof normalizedOutputSchema === 'object')
      assert.equal(
        normalizedOutputSchema.properties?.contract?.const,
        step.output_contract.schema_ref,
        `${step.prompt_id} requirements expose the wrong normalized output schema`,
      )
      return normalizedOutputSchema
    }
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
  const providerSchemaForStep = (step, normalizedOutputSchema) => {
    const canonical = structuredClone(resolvedOutputSchema(step, normalizedOutputSchema))
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
        bindings.push({
          profile: profile.profile,
          jobId,
          step,
          normalizedOutputSchema: envelope.data.normalized_output_schema,
        })

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

  let mcpParity = null
  for (const [index, binding] of bindings.entries()) {
    const { profile, jobId, step, normalizedOutputSchema } = binding
    const outputSchema = providerSchemaForStep(step, normalizedOutputSchema)
    const providerExample = Object.fromEntries(
      step.output_contract.required_top_level.map((field) => [field, step.output_contract.example[field]]),
    )
    if (step.output_contract.schema_ref?.startsWith('mdp.normalized-decision-input.')) {
      providerExample.job_id = jobId
    }
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
    // Bind every declared model-step to the exact authorities handed across the
    // subprocess boundary. These are the same digests consumed by driver v2
    // when it records the run receipt; a profile-neutral adapter parse alone is
    // not parity proof.
    const projectedProviderSchema = projectOutputSchemaForOpenAI(outputSchema)
    const providerBody = buildProviderRequestBody(request)
    assert.equal(request.output_schema_sha256, sha256CanonicalJson(outputSchema))
    assert.equal(result.provider_request_schema_id, PROVIDER_REQUEST_SCHEMA_ID)
    assert.equal(result.provider_request_body_sha256, sha256Bytes(JSON.stringify(providerBody)))
    assert.equal(result.provider_output_schema_sha256, sha256CanonicalJson(projectedProviderSchema))
    assert.equal(result.output.sha256, sha256Bytes(output))
    assert.equal(result.output.byte_count, Buffer.byteLength(output))
    assert.equal(result.provider_observation.provider, 'openai')
    assert.equal(result.provider_observation.response_id, `resp_parity_${index + 1}`)
    assert.equal(result.provider_observation.model, 'gpt-test-synthetic')

    // Legacy normalization can be validated from output bytes alone. Governed
    // Decision Input normalization additionally requires the exact binding,
    // attempt-request, and collected-results artifacts; canonical run tests
    // cover that lineage path. Here the job-compiled schema and authority hashes
    // are still checked for every binding without making a live provider call.
    if (step.phase === 'normalization' && step.output_contract.schema_ref === 'mdp.prompt-output.prospect-normalization.v0') {
      const outputPath = join(scratch, `canonical-output-${index + 1}.json`)
      writeFileSync(outputPath, `${canonicalJsonBytes(providerExample)}\n`)
      const validation = expectJson(
        invoke(mdp, [
          '--json', 'validate-prompt-output', '--dir', profile === 'gtm' ? profiles[0].pack : profiles[1].pack,
          '--prompt-id', step.prompt_id, '--file', outputPath,
        ]),
        `${profile}/${jobId}/${step.phase} canonical prompt-output validation`,
      )
      assert.equal(validation.data.valid, true, JSON.stringify(validation.data.issues))
      assert.equal(validation.data.artifacts.prompt_output.sha256, sha256Bytes(`${canonicalJsonBytes(providerExample)}\n`))
    }

    // Execute the public canonical run path offline for every binding. The
    // native subprocess is intentionally denied permission, but only after v2
    // has resolved and sealed the selected prompt, ordered inputs, schemas,
    // driver request/result, bundle, audit, and receipt authorities.
    const pack = profiles.find((candidate) => candidate.profile === profile).pack
    // Use a persona with explicit entry selectors; the router must not infer
    // PM from prose such as the substring inside PMM.
    const persona = profile === 'proposal' ? 'Proposal Lead' : 'PMM'
    const routeScopeArgs = profile === 'gtm' ? ['--scope', 'product=local-cli'] : []
    const runInputs = step.declared_inputs
      .filter((input) => input.required && !['prompt_receipt', 'invocation_receipt_sha256'].includes(input.name))
      .map((input, inputIndex) => {
        const inputPath = join(scratch, `run-${index + 1}-input-${inputIndex + 1}.json`)
        const routedContext = ['routed_context', 'routed-context'].includes(input.name)
        if (routedContext) {
          const emitted = expectJson(
            invoke(mdp, [
              '--json', 'emit-brief', '--dir', pack, '--persona', persona,
              '--job', jobId, ...routeScopeArgs, '--routed-context-out', inputPath,
            ]),
            `${profile}/${jobId}/${step.phase} emitted routed context`,
          )
          assert.equal(emitted.data.context.minimality.status, 'ready')
          const routed = JSON.parse(readFileSync(inputPath, 'utf8'))
          assert.equal(routed.contract, 'mdp.routed-context.v1')
          assert.equal(routed.job, jobId)
          assert.equal(sha256File(inputPath), sha256Bytes(canonicalJsonBytes(routed)))
        } else {
          writeFileSync(inputPath, `${JSON.stringify({})}\n`)
        }
        return {
          logical_name: input.name,
          source_path: inputPath,
          schema_id: routedContext
            ? 'mdp.routed-context.v1'
            : `mdp.synthetic-${input.name.replaceAll('_', '-')}.v1`,
          media_type: 'application/json',
          provenance_refs: [],
        }
      })
    const invocation = {
      contract: 'mdp.prompt-invocation.v1',
      inputs: runInputs.map((input) => ({ name: input.logical_name, sha256: sha256File(input.source_path) })),
      job_id: jobId,
      prompt: { id: step.prompt_id, sha256: step.prompt_sha256, version: step.prompt_version },
    }
    const invocationContent = `${JSON.stringify(invocation, null, 2)}\n`
    const visibleInput = nativeVisibleInput(
      step,
      readFileSync(join(pack, '.mdp', step.prompt_path), 'utf8'),
      invocationContent,
      runInputs,
    )
    const schemaName = `mdp_${step.step_id.replaceAll(':', '_').replaceAll('/', '_').replaceAll('-', '_')}`
    const identityRequest = {
      contract: DRIVER_REQUEST_CONTRACT,
      execution_id: `parity-${profile}-${index + 1}`,
      provider: 'openai',
      model: 'gpt-test',
      prompt_id: step.prompt_id,
      declared_inputs_only: true,
      input: visibleInput,
      output_schema: outputSchema,
      output_schema_sha256: sha256CanonicalJson(outputSchema),
      schema_name: schemaName,
      max_output_tokens: providerMaxOutputTokens(1048576),
      timeout_ms: 30000,
    }
    const driverSourceSha256 = sha256File(driver)
    const nodeSha256 = sha256File(process.execPath)
    const driverProjection = driverConfigurationProjection(driverSourceSha256, nodeSha256)
    const modelProjection = buildModelParametersProjection(identityRequest)
    const driverConfigurationSha256 = authorityHash('mdp.driver-configuration.v1', driverProjection)
    const modelParametersSha256 = authorityHash('mdp.model-parameters.v1', modelProjection)
    const runRequest = {
      contract: 'mdp.run-request.v1',
      execution_id: `parity-v2-${profile}-${index + 1}`,
      created_at: '2026-08-14T00:00:00Z',
      profile,
      operation: step.step_id,
      mode: 'generative',
      job_identity: { job_id: jobId, idempotency_key: `parity-v2-${index + 1}` },
      pack_dir: pack,
      pack_release_id: `parity-${profile}`,
      prompt: {
        logical_name: step.prompt_id,
        source_path: join(pack, '.mdp', step.prompt_path),
        schema_id: 'mdp.prompt.v1',
        media_type: 'application/yaml',
        provenance_refs: [],
      },
      inputs: runInputs,
      execution_policy: {
        environment_allowlist: ['OPENAI_API_KEY'],
        filesystem_mode: 'private-staging',
        tool_mode: 'none',
        network_mode: 'authorized-endpoints-only',
        authorized_endpoints: ['https://api.openai.com/v1/responses'],
        max_input_bytes: 131072,
        max_output_bytes: 1048576,
        timeout_ms: 30000,
        retention_policy: 'receipt-only',
      },
      driver: {
        driver_id: 'mdp-native-openai',
        implementation: 'bundled:mdp-native-model-openai',
        version: runtimeVersion,
        build_sha256: null,
        executable_sha256: sha256File(driver),
        image_digest: null,
        configuration_sha256: driverConfigurationSha256,
        dependency_lock_sha256: sha256File(process.execPath),
        identity_provenance: 'mdp-observed',
      },
      model: {
        provider: 'openai', requested_model: 'gpt-test', resolved_model: null,
        authorized_endpoint: 'https://api.openai.com/v1/responses', parameters_sha256: modelParametersSha256,
        session_behavior: 'not-applicable', cache_behavior: 'unknown', storage_behavior: 'declared',
      },
    }
    const runRequestPath = join(scratch, `run-${index + 1}.json`)
    const runDir = join(scratch, `run-${index + 1}`)
    writeFileSync(runRequestPath, `${JSON.stringify(runRequest)}\n`)
    const execution = parseJsonResult(
      invoke(mdp, ['--json', 'run', '--request', runRequestPath, '--out-dir', runDir]),
      `${profile}/${jobId}/${step.phase} canonical v2 offline run`,
    )
    if (!mcpParity && runInputs.some((input) => input.logical_name === 'routed_context')) {
      mcpParity = { execution, requestPath: runRequestPath, outputDir: join(scratch, 'mcp-parity-run') }
    }
    if (execution.data.terminal_state === 'no-draft:policy-blocked') {
      assert.equal(execution.data.run_dir, null)
      assert.equal(execution.data.bundle_sha256, null)
      assert.equal(execution.data.receipt_sha256, null)
      assert.equal(execution.data.authority_block.decision, null)
      assert.deepEqual(execution.data.authority_block.diagnostics, [{
        stage: 'run-preflight',
        gate: 'policy',
        code: 'internal-contract-mismatch',
        input: null,
        field: null,
        expected: { kind: 'binding', value: 'available' },
        observed: { kind: 'binding', value: 'unavailable' },
      }])
      assert.ok(!existsSync(runDir), `${profile}/${jobId}/${step.phase} published a blocked run directory`)
      assert.ok(!JSON.stringify(execution).includes('OPENAI_API_KEY'))
      continue
    }
    assert.ok(
      existsSync(join(runDir, 'run-bundle.json')),
      `${profile}/${jobId}/${step.phase} did not stage a run bundle: ${JSON.stringify(execution)}`,
    )
    const bundle = JSON.parse(readFileSync(join(runDir, 'run-bundle.json'), 'utf8'))
    const audit = JSON.parse(readFileSync(join(runDir, 'runner-audit.json'), 'utf8'))
    const receipt = JSON.parse(readFileSync(join(runDir, 'run-receipt.json'), 'utf8'))
    assert.equal(bundle.prompt.sha256, sha256File(runRequest.prompt.source_path))
    assert.match(step.prompt_sha256, /^[0-9a-f]{64}$/)
    assert.deepEqual(bundle.inputs.map((item) => item.logical_name), runInputs.map((item, i) => `declared/${String(i).padStart(3, '0')}-${item.logical_name}`))
    assert.deepEqual(bundle.inputs.map((item) => item.sha256), runInputs.map((item) => sha256File(item.source_path)))
    assert.equal(bundle.operation, step.step_id)
    assert.equal(bundle.driver.executable_sha256, sha256File(driver))
    assert.equal(bundle.model.requested_model, 'gpt-test')
    assert.match(audit.driver_request_sha256, /^[0-9a-f]{64}$/)
    assert.match(audit.driver_result_sha256, /^[0-9a-f]{64}$/)
    assert.match(audit.provider_request_body_sha256, /^[0-9a-f]{64}$/)
    assert.equal(audit.provider_request_schema_id, PROVIDER_REQUEST_SCHEMA_ID)
    assert.equal(audit.provider_response_body_sha256, null)
    const expectedDriverRequest = {
      contract: 'mdp.driver-request.v2', execution_id: runRequest.execution_id, profile, operation: step.step_id,
      job_identity: runRequest.job_identity, phase: step.phase, prompt_id: step.prompt_id,
      prompt_version: step.prompt_version, prompt_canonical_sha256: step.prompt_sha256,
      prompt: { authority: bundle.prompt, content_utf8: readFileSync(runRequest.prompt.source_path, 'utf8') },
      prompt_invocation: {
        authority: {
          logical_name: 'private/prompt-invocation.json', schema_id: 'mdp.prompt-invocation.v1',
          media_type: 'application/json', byte_count: Buffer.byteLength(invocationContent),
          sha256: sha256Bytes(invocationContent), provenance: 'mdp-observed',
          provenance_refs: [execution.data.bundle_sha256],
        },
        content_utf8: invocationContent,
      },
      inputs: bundle.inputs.map((authority, i) => ({
        authority, content_utf8: readFileSync(runInputs[i].source_path, 'utf8'),
      })),
      canonical_output_schema: resolvedOutputSchema(step, normalizedOutputSchema),
      canonical_output_schema_sha256: sha256CanonicalJson(resolvedOutputSchema(step, normalizedOutputSchema)),
      provider_output_schema: projectedProviderSchema,
      provider_output_schema_sha256: sha256CanonicalJson(projectedProviderSchema),
      provider_policy: {
        provider: 'openai', requested_model: 'gpt-test',
        authorized_endpoint: 'https://api.openai.com/v1/responses', timeout_ms: 1,
        max_output_bytes: 1048576,
      },
      execution_policy_sha256: bundle.execution_policy_sha256,
      request_sha256: '',
    }
    // Runtime subtracts elapsed staging time and a finalization reserve from
    // timeout_ms. Search only that single runtime-derived scalar; an exact hash
    // match proves every other v2 field above, including ordered inputs and both schemas.
    let matchedDriverTimeout = null
    for (let timeout = 1; timeout <= runRequest.execution_policy.timeout_ms; timeout += 1) {
      expectedDriverRequest.provider_policy.timeout_ms = timeout
      if (authorityHash('mdp.driver-request.v2', expectedDriverRequest) === audit.driver_request_sha256) {
        matchedDriverTimeout = timeout
        break
      }
    }
    assert.ok(matchedDriverTimeout !== null, `${profile}/${jobId}/${step.phase} v2 request authority mismatch`)
    const expectedDriverResult = {
      contract: 'mdp.driver-result.v2',
      execution_id: runRequest.execution_id,
      operation: step.step_id,
      terminal_state: 'no-draft:policy-blocked',
      output: null,
      provider_request_body_sha256: audit.provider_request_body_sha256,
      provider_request_schema_id: PROVIDER_REQUEST_SCHEMA_ID,
      provider_response_body_sha256: null,
      provider_output_schema_sha256: sha256CanonicalJson(projectedProviderSchema),
      provider_observation: null,
      diagnostic_code: 'native_model_calls_not_allowed',
      result_sha256: '',
    }
    assert.equal(audit.driver_result_sha256, authorityHash('mdp.driver-result.v2', expectedDriverResult))
    assert.equal(execution.data.bundle_sha256, authorityHash('mdp.run-bundle.v1', bundle))
    assert.equal(receipt.bundle_sha256, execution.data.bundle_sha256)
    assert.equal(receipt.runner_audit.sha256, sha256File(join(runDir, 'runner-audit.json')))
    assert.equal(receipt.receipt_sha256, execution.data.receipt_sha256)
    assert.equal(receipt.receipt_sha256, authorityHash('mdp.run-receipt.v1', { ...receipt, receipt_sha256: '' }))
    const verification = expectJson(invoke(mdp, [
      '--json', 'verify-run', '--bundle', join(runDir, 'run-bundle.json'),
      '--receipt', join(runDir, 'run-receipt.json'), '--artifact-root', runDir,
    ]), `${profile}/${jobId}/${step.phase} receipt verification`)
    assert.equal(verification.data.valid, true)
    assert.ok(!JSON.stringify(result).includes('OPENAI_API_KEY'))
  }

  assert.ok(mcpParity, 'a routed-context binding should provide the CLI/MCP parity request')
  const mcp = invoke(
    process.execPath,
    [join(repoRoot, 'scripts', 'mdp-run-mcp-server.mjs')],
    {
      env: { ...process.env, MDP_BIN: resolve(mdp) },
      input: [
        JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'initialize', params: {} }),
        JSON.stringify({
          jsonrpc: '2.0',
          id: 2,
          method: 'tools/call',
          params: {
            name: 'mdp_run',
            arguments: {
              request_path: mcpParity.requestPath,
              output_dir: mcpParity.outputDir,
              timeout_ms: 300000,
            },
          },
        }),
      ].join('\n') + '\n',
    },
  )
  assert.equal(mcp.status, 0, `stdio MCP parity failed\nstdout:\n${mcp.stdout}\nstderr:\n${mcp.stderr}`)
  const mcpReply = JSON.parse(mcp.stdout.trim().split('\n').at(-1))
  assert.equal(mcpReply.result.isError, false)
  const mcpExecution = mcpReply.result.structuredContent
  assert.equal(mcpExecution.terminal_state, mcpParity.execution.data.terminal_state)
  const comparableAuthority = ({ receipt_sha256, verification, ...authority }) => authority
  assert.deepEqual(comparableAuthority(mcpExecution.authority), comparableAuthority(mcpParity.execution.data.authority))
  assert.deepEqual(
    comparableAuthority(mcpExecution.authority_block),
    comparableAuthority(mcpParity.execution.data.authority_block),
  )

  for (const binding of [bindings.find(({ profile }) => profile === 'gtm'), bindings.find(({ profile }) => profile === 'proposal')]) {
    const { profile, jobId, step, normalizedOutputSchema } = binding
    const outputSchema = providerSchemaForStep(step, normalizedOutputSchema)
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
