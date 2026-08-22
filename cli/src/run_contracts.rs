use crate::artifact_hash::PortableFileRecord;
use serde::{Deserialize, Serialize};

pub(crate) const RUN_REQUEST_V1: &str = "mdp.run-request.v1";
pub(crate) const RUN_BUNDLE_V1: &str = "mdp.run-bundle.v1";
pub(crate) const DRIVER_REQUEST_V1: &str = "mdp.driver-request.v1";
pub(crate) const DRIVER_RESULT_V1: &str = "mdp.driver-result.v1";
pub(crate) const DRIVER_REQUEST_V2: &str = "mdp.driver-request.v2";
pub(crate) const DRIVER_RESULT_V2: &str = "mdp.driver-result.v2";
pub(crate) const RUNNER_AUDIT_V1: &str = "mdp.runner-audit.v1";
pub(crate) const RUN_RECEIPT_V1: &str = "mdp.run-receipt.v1";
pub(crate) const RUN_VERIFICATION_V1: &str = "mdp.run-verification.v1";
pub(crate) const RUN_EXECUTION_V1: &str = "mdp.run-execution.v1";
pub(crate) const CANONICAL_AUTHORITY_BLOCK_V1: &str = "mdp.canonical-authority-block.v1";
pub(crate) const PROPOSAL_RUNNER_RESULT_V1: &str = "mdp.proposal-runner-result.v1";
pub(crate) const DRIVER_CONFIGURATION_PROJECTION_V1: &str = "mdp.driver-configuration.v1";
pub(crate) const MODEL_PARAMETERS_PROJECTION_V1: &str = "mdp.model-parameters.v1";
pub(crate) const OPENAI_PROVIDER_REQUEST_SCHEMA_ID: &str =
    "openai.responses.json-schema-request.v1";
pub(crate) const MDP_RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const PROVIDER_REQUEST_RELATION_V1: &str =
    "full-body-includes-model-parameters-and-input";
pub(crate) const PROVIDER_REQUEST_NOT_OBSERVED_V1: &str = "not-observed";
pub(crate) const DEADLINE_OBSERVATION_V1: &str = "mdp.deadline-observation.v1";

/// The deadline projection is deliberately closed: it contains only bounded
/// phase/outcome labels and numeric limits.  It is explanation evidence, not a
/// second decision or authority source.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DeadlinePhase {
    Preflight,
    Staging,
    Driver,
    Provider,
    Validation,
    Finalization,
    Cancellation,
    Transport,
    Cleanup,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DeadlineOutcome {
    TimedOut,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeadlineObservationV1 {
    pub(crate) contract: String,
    pub(crate) outcome: DeadlineOutcome,
    pub(crate) phase: DeadlinePhase,
    pub(crate) elapsed_ms: u64,
    pub(crate) configured_limit_ms: u64,
    pub(crate) effective_limit_ms: u64,
    pub(crate) transport_configured_ms: Option<u64>,
    pub(crate) runtime_configured_ms: u64,
    pub(crate) provider_configured_ms: u64,
    pub(crate) finalization_reserve_ms: u64,
    pub(crate) terminal_state: TerminalState,
    pub(crate) warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RunMode {
    Deterministic,
    Generative,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum TerminalState {
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "no-draft:preflight-refused")]
    NoDraftPreflightRefused,
    #[serde(rename = "no-draft:runner-failed")]
    NoDraftRunnerFailed,
    #[serde(rename = "no-draft:output-invalid")]
    NoDraftOutputInvalid,
    #[serde(rename = "no-draft:decision-invalid")]
    NoDraftDecisionInvalid,
    #[serde(rename = "no-draft:audit-incomplete")]
    NoDraftAuditIncomplete,
    #[serde(rename = "no-draft:policy-blocked")]
    NoDraftPolicyBlocked,
}

impl TerminalState {
    pub(crate) fn is_success(self) -> bool {
        self == Self::Success
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum EvidenceProvenance {
    CompilerDerived,
    MdpObserved,
    ProviderReturned,
    CustomerAttested,
    HostAttested,
    DriverAttested,
    VerifierRecomputed,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AssuranceEvidenceState {
    Declared,
    Observed,
    Enforced,
    Verified,
    Unknown,
    Redacted,
    Unsupported,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum GtmReasonCode {
    Ready,
    InsufficientContext,
    MissingRequiredSourceAttempt,
    InvalidSourceBinding,
    StaleDecisionInput,
    InvalidDecisionInput,
    Disqualified,
    ScopeMismatch,
    HardGateFailed,
    ValidationFailed,
    PolicyBlocked,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JobIdentity {
    pub(crate) job_id: String,
    pub(crate) idempotency_key: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocalArtifactInput {
    pub(crate) logical_name: String,
    pub(crate) source_path: String,
    pub(crate) schema_id: String,
    pub(crate) media_type: String,
    pub(crate) provenance_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArtifactAuthority {
    pub(crate) logical_name: String,
    pub(crate) schema_id: String,
    pub(crate) media_type: String,
    pub(crate) byte_count: u64,
    pub(crate) sha256: String,
    pub(crate) provenance: EvidenceProvenance,
    pub(crate) provenance_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PackAuthority {
    pub(crate) release_id: String,
    pub(crate) pack_id: String,
    pub(crate) version: String,
    pub(crate) profile_id: String,
    pub(crate) portable_digest: String,
    pub(crate) files: Vec<PortableFileRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExecutionPolicy {
    pub(crate) environment_allowlist: Vec<String>,
    pub(crate) filesystem_mode: String,
    pub(crate) tool_mode: String,
    pub(crate) network_mode: String,
    pub(crate) authorized_endpoints: Vec<String>,
    pub(crate) max_input_bytes: u64,
    pub(crate) max_output_bytes: u64,
    pub(crate) timeout_ms: u64,
    pub(crate) retention_policy: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DriverIdentity {
    pub(crate) driver_id: String,
    pub(crate) implementation: String,
    pub(crate) version: String,
    pub(crate) build_sha256: Option<String>,
    pub(crate) executable_sha256: Option<String>,
    pub(crate) image_digest: Option<String>,
    pub(crate) configuration_sha256: String,
    pub(crate) dependency_lock_sha256: Option<String>,
    pub(crate) identity_provenance: EvidenceProvenance,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelIdentity {
    pub(crate) provider: String,
    pub(crate) requested_model: String,
    pub(crate) resolved_model: Option<String>,
    pub(crate) authorized_endpoint: String,
    pub(crate) parameters_sha256: String,
    pub(crate) session_behavior: AssuranceEvidenceState,
    pub(crate) cache_behavior: AssuranceEvidenceState,
    pub(crate) storage_behavior: AssuranceEvidenceState,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DriverConfigurationProjectionV1 {
    pub(crate) contract: String,
    pub(crate) driver_id: String,
    pub(crate) implementation: String,
    pub(crate) runtime_version: String,
    pub(crate) bundled_source_sha256: String,
    pub(crate) node_executable_sha256: String,
    pub(crate) native_request_contract: String,
    pub(crate) native_result_contract: String,
    pub(crate) clear_env: bool,
    pub(crate) allowlisted_environment_names: Vec<String>,
    pub(crate) filesystem_mode: String,
    pub(crate) stdin_mode: String,
    pub(crate) stdout_mode: String,
    pub(crate) max_request_bytes: u64,
    pub(crate) max_response_bytes: u64,
    pub(crate) timeout_enforced: bool,
    pub(crate) authorized_endpoint: String,
    pub(crate) redirect_policy: String,
    pub(crate) proxy_policy: String,
    pub(crate) storage_policy: String,
    pub(crate) tool_policy: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelParametersProjectionV1 {
    pub(crate) contract: String,
    pub(crate) provider: String,
    pub(crate) requested_model: String,
    pub(crate) authorized_endpoint: String,
    pub(crate) declared_timeout_ms: u64,
    pub(crate) max_output_tokens: u64,
    pub(crate) structured_output_mode: String,
    pub(crate) schema_name: String,
    pub(crate) provider_output_schema_sha256: String,
    pub(crate) input_framing: String,
    pub(crate) visible_input_sha256: String,
    pub(crate) store: bool,
    pub(crate) tool_choice: String,
    pub(crate) continuation_policy: String,
    pub(crate) tools_policy: String,
    pub(crate) reasoning: Option<String>,
    pub(crate) metadata: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DriverConfigurationFactsV1 {
    pub(crate) driver_id: String,
    pub(crate) implementation: String,
    pub(crate) runtime_version: String,
    pub(crate) bundled_source_sha256: String,
    pub(crate) node_executable_sha256: String,
    pub(crate) native_request_contract: String,
    pub(crate) native_result_contract: String,
    pub(crate) clear_env: bool,
    pub(crate) allowlisted_environment_names: Vec<String>,
    pub(crate) filesystem_mode: String,
    pub(crate) stdin_mode: String,
    pub(crate) stdout_mode: String,
    pub(crate) max_request_bytes: u64,
    pub(crate) max_response_bytes: u64,
    pub(crate) timeout_enforced: bool,
    pub(crate) authorized_endpoint: String,
    pub(crate) redirect_policy: String,
    pub(crate) proxy_policy: String,
    pub(crate) storage_policy: String,
    pub(crate) tool_policy: String,
}

impl From<&DriverConfigurationProjectionV1> for DriverConfigurationFactsV1 {
    fn from(projection: &DriverConfigurationProjectionV1) -> Self {
        Self {
            driver_id: projection.driver_id.clone(),
            implementation: projection.implementation.clone(),
            runtime_version: projection.runtime_version.clone(),
            bundled_source_sha256: projection.bundled_source_sha256.clone(),
            node_executable_sha256: projection.node_executable_sha256.clone(),
            native_request_contract: projection.native_request_contract.clone(),
            native_result_contract: projection.native_result_contract.clone(),
            clear_env: projection.clear_env,
            allowlisted_environment_names: projection.allowlisted_environment_names.clone(),
            filesystem_mode: projection.filesystem_mode.clone(),
            stdin_mode: projection.stdin_mode.clone(),
            stdout_mode: projection.stdout_mode.clone(),
            max_request_bytes: projection.max_request_bytes,
            max_response_bytes: projection.max_response_bytes,
            timeout_enforced: projection.timeout_enforced,
            authorized_endpoint: projection.authorized_endpoint.clone(),
            redirect_policy: projection.redirect_policy.clone(),
            proxy_policy: projection.proxy_policy.clone(),
            storage_policy: projection.storage_policy.clone(),
            tool_policy: projection.tool_policy.clone(),
        }
    }
}

impl From<&DriverConfigurationFactsV1> for DriverConfigurationProjectionV1 {
    fn from(facts: &DriverConfigurationFactsV1) -> Self {
        Self {
            contract: DRIVER_CONFIGURATION_PROJECTION_V1.into(),
            driver_id: facts.driver_id.clone(),
            implementation: facts.implementation.clone(),
            runtime_version: facts.runtime_version.clone(),
            bundled_source_sha256: facts.bundled_source_sha256.clone(),
            node_executable_sha256: facts.node_executable_sha256.clone(),
            native_request_contract: facts.native_request_contract.clone(),
            native_result_contract: facts.native_result_contract.clone(),
            clear_env: facts.clear_env,
            allowlisted_environment_names: facts.allowlisted_environment_names.clone(),
            filesystem_mode: facts.filesystem_mode.clone(),
            stdin_mode: facts.stdin_mode.clone(),
            stdout_mode: facts.stdout_mode.clone(),
            max_request_bytes: facts.max_request_bytes,
            max_response_bytes: facts.max_response_bytes,
            timeout_enforced: facts.timeout_enforced,
            authorized_endpoint: facts.authorized_endpoint.clone(),
            redirect_policy: facts.redirect_policy.clone(),
            proxy_policy: facts.proxy_policy.clone(),
            storage_policy: facts.storage_policy.clone(),
            tool_policy: facts.tool_policy.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelParametersFactsV1 {
    pub(crate) provider: String,
    pub(crate) requested_model: String,
    pub(crate) authorized_endpoint: String,
    pub(crate) declared_timeout_ms: u64,
    pub(crate) max_output_tokens: u64,
    pub(crate) structured_output_mode: String,
    pub(crate) schema_name: String,
    pub(crate) provider_output_schema_sha256: String,
    pub(crate) input_framing: String,
    pub(crate) visible_input_sha256: String,
    pub(crate) store: bool,
    pub(crate) tool_choice: String,
    pub(crate) continuation_policy: String,
    pub(crate) tools_policy: String,
    pub(crate) reasoning: Option<String>,
    pub(crate) metadata: Option<String>,
}

impl ModelParametersFactsV1 {
    pub(crate) fn from_runtime_inputs(
        provider: String,
        requested_model: String,
        authorized_endpoint: String,
        declared_timeout_ms: u64,
        max_output_tokens: u64,
        schema_name: String,
        provider_output_schema_sha256: String,
        visible_input_sha256: String,
    ) -> Self {
        Self {
            provider,
            requested_model,
            authorized_endpoint,
            declared_timeout_ms,
            max_output_tokens,
            structured_output_mode: "json-schema-strict".into(),
            schema_name,
            provider_output_schema_sha256,
            input_framing: "one-fresh-user-message:declared-inputs-only".into(),
            visible_input_sha256,
            store: false,
            tool_choice: "none".into(),
            continuation_policy: "none".into(),
            tools_policy: "none".into(),
            reasoning: None,
            metadata: None,
        }
    }
}

impl From<&ModelParametersFactsV1> for ModelParametersProjectionV1 {
    fn from(facts: &ModelParametersFactsV1) -> Self {
        Self {
            contract: MODEL_PARAMETERS_PROJECTION_V1.into(),
            provider: facts.provider.clone(),
            requested_model: facts.requested_model.clone(),
            authorized_endpoint: facts.authorized_endpoint.clone(),
            declared_timeout_ms: facts.declared_timeout_ms,
            max_output_tokens: facts.max_output_tokens,
            structured_output_mode: facts.structured_output_mode.clone(),
            schema_name: facts.schema_name.clone(),
            provider_output_schema_sha256: facts.provider_output_schema_sha256.clone(),
            input_framing: facts.input_framing.clone(),
            visible_input_sha256: facts.visible_input_sha256.clone(),
            store: facts.store,
            tool_choice: facts.tool_choice.clone(),
            continuation_policy: facts.continuation_policy.clone(),
            tools_policy: facts.tools_policy.clone(),
            reasoning: facts.reasoning.clone(),
            metadata: facts.metadata.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderRequestObservationV1 {
    pub(crate) provider_request_body_sha256: Option<String>,
    pub(crate) provider_request_schema_id: Option<String>,
    pub(crate) relation: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IdentityObservationV1 {
    pub(crate) driver_declaration_sha256: String,
    pub(crate) driver_observed_sha256: String,
    pub(crate) driver_projection: DriverConfigurationProjectionV1,
    pub(crate) driver_facts: DriverConfigurationFactsV1,
    pub(crate) model_declaration_sha256: String,
    pub(crate) model_observed_sha256: String,
    pub(crate) model_projection: ModelParametersProjectionV1,
    pub(crate) provider_request: ProviderRequestObservationV1,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AssuranceDimension {
    pub(crate) dimension: String,
    pub(crate) state: AssuranceEvidenceState,
    pub(crate) provenance: EvidenceProvenance,
    pub(crate) evidence_refs: Vec<String>,
    pub(crate) limitations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunRequestV1 {
    pub(crate) contract: String,
    pub(crate) execution_id: String,
    pub(crate) created_at: String,
    pub(crate) profile: String,
    pub(crate) operation: String,
    pub(crate) mode: RunMode,
    pub(crate) job_identity: Option<JobIdentity>,
    pub(crate) pack_dir: String,
    pub(crate) pack_release_id: String,
    pub(crate) prompt: Option<LocalArtifactInput>,
    pub(crate) inputs: Vec<LocalArtifactInput>,
    pub(crate) execution_policy: ExecutionPolicy,
    pub(crate) driver: Option<DriverIdentity>,
    pub(crate) model: Option<ModelIdentity>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunBundleV1 {
    pub(crate) contract: String,
    pub(crate) execution_id: String,
    pub(crate) created_at: String,
    pub(crate) profile: String,
    pub(crate) operation: String,
    pub(crate) mode: RunMode,
    pub(crate) job_identity: Option<JobIdentity>,
    pub(crate) pack: PackAuthority,
    pub(crate) prompt: Option<ArtifactAuthority>,
    pub(crate) inputs: Vec<ArtifactAuthority>,
    pub(crate) execution_policy_sha256: String,
    pub(crate) driver: Option<DriverIdentity>,
    pub(crate) model: Option<ModelIdentity>,
    #[serde(default)]
    pub(crate) model_facts: Option<ModelParametersFactsV1>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DriverRequestV1 {
    pub(crate) contract: String,
    pub(crate) execution_id: String,
    pub(crate) profile: String,
    pub(crate) operation: String,
    pub(crate) prompt: ArtifactAuthority,
    pub(crate) inputs: Vec<ArtifactAuthority>,
    pub(crate) output_schema_sha256: String,
    pub(crate) execution_policy_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DriverResultV1 {
    pub(crate) contract: String,
    pub(crate) execution_id: String,
    pub(crate) terminal_state: TerminalState,
    pub(crate) output: Option<ArtifactAuthority>,
    pub(crate) audit: ArtifactAuthority,
}

/// One exact, model-visible artifact retained from the private staging tree.
///
/// Runtime model steps are textual contracts. Keeping the UTF-8 bytes inline
/// prevents a driver from reopening a caller-controlled path after MDP has
/// hashed it. `authority.sha256` binds the exact bytes in `content_utf8`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DriverArtifactV2 {
    pub(crate) authority: ArtifactAuthority,
    pub(crate) content_utf8: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DriverProviderPolicyV2 {
    pub(crate) provider: String,
    pub(crate) requested_model: String,
    pub(crate) authorized_endpoint: String,
    pub(crate) timeout_ms: u64,
    pub(crate) deadline_at_ms: u64,
    pub(crate) max_output_bytes: u64,
}

/// Canonical MDP-to-driver authority for exactly one selected model step.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DriverRequestV2 {
    pub(crate) contract: String,
    pub(crate) execution_id: String,
    pub(crate) profile: String,
    pub(crate) operation: String,
    pub(crate) job_identity: JobIdentity,
    pub(crate) phase: String,
    pub(crate) prompt_id: String,
    pub(crate) prompt_version: String,
    pub(crate) prompt_canonical_sha256: String,
    pub(crate) prompt: DriverArtifactV2,
    pub(crate) prompt_invocation: DriverArtifactV2,
    pub(crate) inputs: Vec<DriverArtifactV2>,
    pub(crate) canonical_output_schema: serde_json::Value,
    pub(crate) canonical_output_schema_sha256: String,
    pub(crate) provider_output_schema: serde_json::Value,
    pub(crate) provider_output_schema_sha256: String,
    pub(crate) provider_policy: DriverProviderPolicyV2,
    pub(crate) execution_policy_sha256: String,
    pub(crate) request_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DriverOutputV2 {
    pub(crate) schema_id: String,
    pub(crate) media_type: String,
    pub(crate) content_utf8: String,
    pub(crate) byte_count: u64,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DriverProviderObservationV2 {
    pub(crate) provider: String,
    pub(crate) response_id: Option<String>,
    pub(crate) resolved_model: Option<String>,
}

/// Closed result envelope. Diagnostics are stable codes, never raw provider
/// error text. Failed results cannot carry model output authority.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DriverResultV2 {
    pub(crate) contract: String,
    pub(crate) execution_id: String,
    pub(crate) operation: String,
    pub(crate) terminal_state: TerminalState,
    pub(crate) output: Option<DriverOutputV2>,
    pub(crate) provider_request_body_sha256: Option<String>,
    pub(crate) provider_request_schema_id: Option<String>,
    pub(crate) provider_response_body_sha256: Option<String>,
    pub(crate) provider_output_schema_sha256: Option<String>,
    pub(crate) provider_observation: Option<DriverProviderObservationV2>,
    pub(crate) diagnostic_code: Option<String>,
    pub(crate) result_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunnerAuditV1 {
    pub(crate) contract: String,
    pub(crate) execution_id: String,
    pub(crate) runner_version: String,
    pub(crate) runner_build_sha256: Option<String>,
    pub(crate) platform: String,
    pub(crate) snapshot_sha256: String,
    pub(crate) driver_request_sha256: Option<String>,
    pub(crate) driver_result_sha256: Option<String>,
    pub(crate) provider_request_body_sha256: Option<String>,
    pub(crate) provider_request_schema_id: Option<String>,
    #[serde(default)]
    pub(crate) provider_response_body_sha256: Option<String>,
    #[serde(default)]
    pub(crate) provider_observation: Option<DriverProviderObservationV2>,
    #[serde(default)]
    pub(crate) identity_observations: Option<IdentityObservationV1>,
    #[serde(default)]
    pub(crate) deadline: Option<DeadlineObservationV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostic_code: Option<String>,
    pub(crate) terminal_state: TerminalState,
    pub(crate) assurance: Vec<AssuranceDimension>,
    pub(crate) limitations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DecisionAuthority {
    pub(crate) schema_id: String,
    pub(crate) decision: String,
    pub(crate) reason_codes: Vec<String>,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunReceiptV1 {
    pub(crate) contract: String,
    pub(crate) execution_id: String,
    pub(crate) created_at: String,
    pub(crate) profile: String,
    pub(crate) operation: String,
    pub(crate) job_identity: Option<JobIdentity>,
    pub(crate) bundle_sha256: String,
    pub(crate) terminal_state: TerminalState,
    pub(crate) output: Option<ArtifactAuthority>,
    pub(crate) decision: Option<DecisionAuthority>,
    pub(crate) compiled_context: Option<ArtifactAuthority>,
    pub(crate) validation: Option<ArtifactAuthority>,
    pub(crate) runner_audit: ArtifactAuthority,
    #[serde(default)]
    pub(crate) deadline: Option<DeadlineObservationV1>,
    pub(crate) assurance: Vec<AssuranceDimension>,
    pub(crate) limitations: Vec<String>,
    pub(crate) receipt_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunVerificationV1 {
    pub(crate) contract: String,
    pub(crate) valid: bool,
    pub(crate) integrity_only: bool,
    pub(crate) execution_id: String,
    pub(crate) terminal_state: TerminalState,
    pub(crate) recomputed_assurance: Vec<AssuranceDimension>,
    pub(crate) issues: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        AssuranceEvidenceState, CANONICAL_AUTHORITY_BLOCK_V1, DRIVER_REQUEST_V1, DRIVER_REQUEST_V2,
        DRIVER_RESULT_V1, DRIVER_RESULT_V2, EvidenceProvenance, GtmReasonCode,
        PROPOSAL_RUNNER_RESULT_V1, RUN_BUNDLE_V1, RUN_EXECUTION_V1, RUN_RECEIPT_V1, RUN_REQUEST_V1,
        RUN_VERIFICATION_V1, RUNNER_AUDIT_V1, RunMode, TerminalState,
    };

    #[test]
    fn authority_enums_have_stable_wire_values() {
        assert_eq!(RUN_REQUEST_V1, "mdp.run-request.v1");
        assert_eq!(RUN_BUNDLE_V1, "mdp.run-bundle.v1");
        assert_eq!(DRIVER_REQUEST_V1, "mdp.driver-request.v1");
        assert_eq!(DRIVER_RESULT_V1, "mdp.driver-result.v1");
        assert_eq!(DRIVER_REQUEST_V2, "mdp.driver-request.v2");
        assert_eq!(DRIVER_RESULT_V2, "mdp.driver-result.v2");
        assert_eq!(RUNNER_AUDIT_V1, "mdp.runner-audit.v1");
        assert_eq!(RUN_RECEIPT_V1, "mdp.run-receipt.v1");
        assert_eq!(RUN_VERIFICATION_V1, "mdp.run-verification.v1");
        assert_eq!(RUN_EXECUTION_V1, "mdp.run-execution.v1");
        assert_eq!(
            CANONICAL_AUTHORITY_BLOCK_V1,
            "mdp.canonical-authority-block.v1"
        );
        assert_eq!(PROPOSAL_RUNNER_RESULT_V1, "mdp.proposal-runner-result.v1");
        assert!(TerminalState::Success.is_success());
        assert!(!TerminalState::NoDraftRunnerFailed.is_success());
        assert_eq!(
            serde_json::to_string(&RunMode::Deterministic).unwrap(),
            "\"deterministic\""
        );
        assert_eq!(
            serde_json::to_string(&TerminalState::NoDraftAuditIncomplete).unwrap(),
            "\"no-draft:audit-incomplete\""
        );
        assert_eq!(
            serde_json::to_string(&EvidenceProvenance::MdpObserved).unwrap(),
            "\"mdp-observed\""
        );
        assert_eq!(
            serde_json::to_string(&AssuranceEvidenceState::NotApplicable).unwrap(),
            "\"not-applicable\""
        );
        assert_eq!(
            serde_json::to_string(&GtmReasonCode::MissingRequiredSourceAttempt).unwrap(),
            "\"missing-required-source-attempt\""
        );
    }

    #[test]
    fn runner_audit_v1_defaults_new_provider_fields_for_legacy_artifacts() {
        let audit: super::RunnerAuditV1 = serde_json::from_value(serde_json::json!({
            "contract": RUNNER_AUDIT_V1,
            "execution_id": "legacy-exec",
            "runner_version": "0.1.66",
            "runner_build_sha256": null,
            "platform": "test",
            "snapshot_sha256": "a".repeat(64),
            "driver_request_sha256": null,
            "driver_result_sha256": null,
            "provider_request_body_sha256": null,
            "provider_request_schema_id": null,
            "terminal_state": "no-draft:policy-blocked",
            "assurance": [],
            "limitations": []
        }))
        .unwrap();
        assert_eq!(audit.provider_response_body_sha256, None);
        assert_eq!(audit.provider_observation, None);
        assert_eq!(audit.identity_observations, None);
    }
}
