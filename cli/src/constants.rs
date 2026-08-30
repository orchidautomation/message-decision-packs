pub(crate) const FORMAT_NAME: &str = "Message Decision Pack";
pub(crate) const FORMAT_VERSION: &str = "mdp.v0";
pub(crate) const DEFAULT_DIR: &str = ".mdp";
pub(crate) const GENERATED_PACK_DIRECTORIES: &[&str] = &["briefs", "traces"];
pub(crate) const PROMPT_FORMAT_VERSION: &str = "mdp.prompt.v0";
pub(crate) const PROMPT_FORMAT_V1: &str = "mdp.prompt.v1";
pub(crate) const PROMPT_OUTPUT_CONTRACT: &str = "mdp.prompt-output.v0";
pub(crate) const GOVERNED_HOST_ENVELOPE_CONTRACT: &str = "mdp.governed-host-envelope.v1";
pub(crate) const PROMPT_OUTPUT_VALIDATION_CONTRACT: &str = "mdp.prompt-output-validation.v1";
pub(crate) const ROUTED_CONTEXT_CONTRACT: &str = "mdp.routed-context.v1";
pub(crate) const SOURCE_AUDIT_CONTRACT: &str = "mdp.source-audit.v0";
pub(crate) const SOURCE_INTAKE_CONTRACT: &str = "mdp.source-intake.v0";
pub(crate) const RUNNER_AUDIT_CONTRACT: &str = "mdp.runner-audit.v0";
pub(crate) const RUN_RECEIPT_CONTRACT: &str = "mdp.run-receipt.v0";
pub(crate) const NATIVE_NORMALIZE_REQUEST_CONTRACT: &str = "mdp.native-normalize-request.v0";
pub(crate) const PROPOSAL_RUNNER_RESULT_CONTRACT: &str = "mdp.proposal-runner-result.v0";
pub(crate) const PROPOSAL_READINESS_REPORT_CONTRACT: &str = "mdp.proposal-readiness-report.v0";
pub(crate) const PROPOSAL_MCP_RUN_RESULT_CONTRACT: &str = "mdp.proposal-mcp-run-result.v0";
pub(crate) const PROPOSAL_RUN_MANIFEST_CONTRACT: &str = "mdp.proposal-run-manifest.v0";
pub(crate) const REQUIREMENTS_CONTRACT: &str = "mdp.requirements.v1";
pub(crate) const REQUIREMENTS_CONTRACT_V2: &str = "mdp.requirements.v2";
pub(crate) const SOURCE_BINDING_CONTRACT: &str = "mdp.source-binding.v1";
pub(crate) const SOURCE_BINDING_CONTRACT_V2: &str = "mdp.source-binding.v2";
pub(crate) const SOURCE_BINDING_VALIDATION_CONTRACT: &str = "mdp.source-binding-validation.v1";
pub(crate) const COLLECTED_ATTEMPT_RESULTS_CONTRACT: &str = "mdp.collected-attempt-results.v1";
pub(crate) const COLLECTED_ATTEMPT_RESULTS_CONTRACT_V2: &str = "mdp.collected-attempt-results.v2";
pub(crate) const NORMALIZED_DECISION_INPUT_CONTRACT: &str = "mdp.normalized-decision-input.v1";
pub(crate) const NORMALIZED_DECISION_INPUT_CONTRACT_V2: &str = "mdp.normalized-decision-input.v2";
pub(crate) const SOURCE_ATTEMPT_REQUEST_CONTRACT_V2: &str = "mdp.source-attempt-request.v2";
pub(crate) const PROMPT_CARD_PATCH_SCHEMA_REF: &str = "mdp.prompt-output.card-patches.v0";
pub(crate) const PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF: &str =
    "mdp.prompt-output.prospect-normalization.v0";

// =============================================================================
// Semantic normalization v3 (MDP-287): profile-neutral host-owned contract.
// =============================================================================
// The v3 wire is the first normalized decision-input contract whose canonical
// payload is profile neutral. New v3 producers use `normalized_input`; the
// legacy `normalized_prospect`/`normalized_opportunity` aliases remain
// compatibility surfaces only. The model emits semantic-only fields; the host
// seals every host-owned field and rejects injection.
pub(crate) const NORMALIZED_DECISION_INPUT_CONTRACT_V3: &str = "mdp.normalized-decision-input.v3";
pub(crate) const NORMALIZED_SEMANTIC_PROVIDER_SCHEMA_REF_V3: &str =
    "mdp.normalization-semantic-provider.v3";
pub(crate) const CLASSIFICATION_TAXONOMY_CONTRACT_V3: &str = "mdp.classification-taxonomy.v3";
pub(crate) const TAXONOMY_SET_HASH_LABEL_V3: &str = "taxonomy_set_sha256";
pub(crate) const REQUIREMENTS_HASH_LABEL_V3: &str = "requirements_sha256";

// Output kinds supported by the v3 generalized host envelope.
pub(crate) const OUTPUT_KIND_GOVERNED_ARTIFACT: &str = "governed-artifact";
pub(crate) const OUTPUT_KIND_DECISION_INPUT_NORMALIZATION: &str = "decision-input-normalization";

// Bounded v3 limits. The contract fixes a 500-character basis cap and a bounded
// classification payload; runtime fails closed on overflow.
pub(crate) const V3_BASIS_MAX_CHARS_DEFAULT: usize = 500;
pub(crate) const V3_MAX_CLASSIFICATIONS_PER_ENVELOPE: usize = 32;
pub(crate) const V3_MAX_TAXONOMY_VALUES: usize = 32;
pub(crate) const V3_MAX_TAXONOMY_CONTRIBUTORS: usize = 16;
pub(crate) const V3_MAX_REJECTED_CLAIMS_PER_ENVELOPE: usize = 32;
pub(crate) const V3_MAX_GAPS_PER_ENVELOPE: usize = 32;
pub(crate) const V3_MAX_DERIVED_FROM_PER_CLASSIFICATION: usize = 16;
pub(crate) const V3_IDENTIFIER_MAX_LEN: usize = 64;
pub(crate) const V3_BASIS_MAX_CHARS_HARD_LIMIT: usize = 500;
pub(crate) const V3_OUTCOME_KIND: &str = "decision-input-normalization";

// Closed value enums used by the v3 semantic payload. Model output honoring
// these values is a precondition for host sealing.
pub(crate) const V3_CLASSIFICATION_STATUS_CLASSIFIED: &str = "classified";
pub(crate) const V3_CLASSIFICATION_STATUS_AMBIGUOUS: &str = "ambiguous";
pub(crate) const V3_CLASSIFICATION_STATUS_NO_MATCH: &str = "no-match";
pub(crate) const V3_CLASSIFICATION_STATUS_UNSUPPORTED: &str = "unsupported";
pub(crate) const V3_AMBIGUITY_POLICY_HUMAN_REVIEW: &str = "human-review";
pub(crate) const V3_NO_MATCH_POLICY_GAP: &str = "gap";
pub(crate) const V3_CONFLICT_POLICY_HUMAN_REVIEW: &str = "human-review";
