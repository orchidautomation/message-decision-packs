export const RUNNER_CONTRACT = 'mdp.proposal-runner.v0'
export const RESULT_CONTRACT = 'mdp.proposal-runner-result.v0'
export const TOOLS_CONTRACT = 'mdp.proposal-runner-tools.v0'
export const SOURCE_INTAKE_CONTRACT = 'mdp.source-intake.v0'
export const SOURCE_AUDIT_CONTRACT = 'mdp.source-audit.v0'
export const WORKDIR_CONTRACT = 'mdp.proposal-workdir.v0'
export const RUN_MANIFEST_CONTRACT = 'mdp.proposal-run-manifest.v0'
export const REQUEST_CONTRACT = 'mdp.native-normalize-request.v0'
export const PROMPT_OUTPUT_CONTRACT = 'mdp.prompt-output.v0'
export const DEFAULT_PROMPT_ID = 'normalize-opportunity'
export const DEFAULT_SOURCE_KIND = 'private-scratch-opportunity'
export const DEFAULT_MAX_SOURCE_BYTES = 12000
export const MAX_CONTEXT_CHARS = 20000
export const MAX_SNIPPET_CHARS = 500
export const SAFE_SOURCE_ID = /^[a-z0-9][a-z0-9._-]{0,127}$/
export const PRIVACY_CLASSES = new Set([
  'synthetic-public',
  'sanitized-public',
  'private-customer',
  'restricted-local',
])
export const TEXT_EXTENSIONS = new Set([
  '.txt',
  '.md',
  '.markdown',
  '.csv',
  '.json',
  '.yaml',
  '.yml',
])

export const toolEnvelope = () => ({
  contract: TOOLS_CONTRACT,
  runner_contract: RUNNER_CONTRACT,
  note: 'These are host-neutral local runner steps exposed by the bundled local stdio MCP wrapper. This is not a hosted or remote MCP implementation.',
  tools: [
    {
      name: 'mdp_intake_sources',
      mode: 'local-files',
      boundary: 'customer-controlled workdir',
      purpose:
        'Stage supplied text/csv/markdown/json/yaml files and preserve or create mdp.source-audit.v0 refs.',
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
      purpose:
        'Run mdp run-receipt --require-runner-audit to bind prompt output, validation, source audit, and runner audit.',
    },
    {
      name: 'mdp_review_proposal',
      mode: 'cli',
      boundary: 'review support only',
      purpose:
        'Optionally run fit/route probes after the receipt; does not write, certify, approve, or submit proposals.',
    },
  ],
})

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
            description:
              'Short source-backed explanation of what was missing or why it could not be extracted.',
          },
        },
      },
    ],
  },
})

export const promptOutputSchema = () => {
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
          source_safety: {
            enum: [
              'synthetic',
              'sanitized',
              'private-scratch',
              'public-source',
              'user-approved-local',
            ],
          },
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
