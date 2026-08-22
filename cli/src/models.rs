use crate::constants::NORMALIZED_DECISION_INPUT_CONTRACT;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Manifest {
    pub(crate) format: String,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) target: Option<TargetIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) profile: Option<Profile>,
    pub(crate) personas: Vec<String>,
    #[serde(default)]
    pub(crate) target_personas: Vec<String>,
    #[serde(default)]
    pub(crate) operator_roles: Vec<String>,
    #[serde(default)]
    pub(crate) supported_channels: Vec<String>,
    #[serde(default)]
    pub(crate) persona_mappings: Vec<PersonaMapping>,
    #[serde(default)]
    pub(crate) lead_input_requirements: LeadInputRequirements,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) qualification_gates: Option<QualificationGates>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) required_primitives: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) primitive_map: BTreeMap<String, PrimitiveMapping>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) decision_input_contracts: Vec<DecisionInputContract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) input_contracts: Vec<InputContract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) jobs: Vec<ProfileJob>,
    #[serde(default, skip_serializing_if = "ProfileEval::is_empty")]
    pub(crate) profile_eval: ProfileEval,
    pub(crate) cards: Vec<CardRef>,
    pub(crate) policy: Policy,
    pub(crate) provenance: Provenance,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct TargetIdentity {
    #[serde(default)]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) aliases: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) external_terms: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) excluded_terms: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) internal_terms: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) source_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct Profile {
    #[serde(default)]
    pub(crate) id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) context_dimensions: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) context_dimension_dependencies: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) product_foundation: Option<ProductFoundationRegistry>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProductFoundationRegistry {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) facets: Vec<ProductFoundationFacet>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProductFoundationFacet {
    #[serde(default)]
    pub(crate) id: String,
    pub(crate) kind: ProductFoundationFacetKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) entries: Vec<ProductFoundationEntryRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) gaps: Vec<ProductFoundationEntryRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) conflicts_with: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProductFoundationFacetKind {
    #[default]
    ProductIdentity,
    ProductExclusions,
    Actors,
    OperatingContext,
    Problems,
    Outcomes,
    Differentiators,
    Alternatives,
    Claims,
    ProofBoundaries,
    Terminology,
    Offers,
    Motions,
    CallsToAction,
    NarrativePosture,
    Gaps,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProductFoundationEntryRef {
    #[serde(default)]
    pub(crate) card_id: String,
    #[serde(default)]
    pub(crate) entry_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct PrimitiveMapping {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) cards: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) prompts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) input_contracts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) jobs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) evals: Vec<String>,
}

impl PrimitiveMapping {
    pub(crate) fn is_empty(&self) -> bool {
        self.cards.is_empty()
            && self.prompts.is_empty()
            && self.input_contracts.is_empty()
            && self.jobs.is_empty()
            && self.evals.is_empty()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct InputContract {
    #[serde(default)]
    pub(crate) id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) schema_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) normalizes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) decision_input_contracts: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProfileJob {
    #[serde(default)]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) skill_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) required_primitives: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) input_contracts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) decision_input_contracts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) product_foundation: Option<ProductFoundationBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) model_task: Option<JobModelTask>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) context_budget: Option<JobContextBudget>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct JobContextBudget {
    #[serde(default)]
    pub(crate) max_entries: usize,
    #[serde(default)]
    pub(crate) max_bytes: usize,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) optional_kind_quotas: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct JobModelTask {
    #[serde(default)]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) prompt: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProductFoundationBinding {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) required: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) conditional: Vec<ProductFoundationConditionalFacet>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) optional: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) excluded: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProductFoundationConditionalFacet {
    #[serde(default)]
    pub(crate) facet_id: String,
    #[serde(default)]
    pub(crate) when: ProductFoundationCondition,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProductFoundationCondition {
    pub(crate) fact: ProductFoundationConditionFact,
    #[serde(default)]
    pub(crate) equals: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProductFoundationConditionFact {
    #[default]
    ManifestId,
    ProfileId,
    JobId,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct DecisionInputContract {
    #[serde(default)]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) normalization: DecisionInputNormalization,
    #[serde(default)]
    pub(crate) source_classes: Vec<DecisionInputSourceClass>,
    #[serde(default)]
    pub(crate) attributes: Vec<DecisionInputAttribute>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) signal_projections: Vec<DecisionInputSignalProjection>,
}

pub(crate) const MAX_SIGNAL_PROJECTIONS_PER_CONTRACT: usize = 32;
pub(crate) const MAX_SIGNAL_OBSERVATIONS_PER_ENVELOPE: usize = 128;
pub(crate) const MAX_SIGNAL_CONTRIBUTORS: usize = 16;
pub(crate) const MAX_SIGNAL_ATTEMPTS: usize = 32;
pub(crate) const MAX_SIGNAL_IDENTIFIER_LEN: usize = 64;
pub(crate) const MAX_SIGNAL_QUALIFIED_ID_LEN: usize = MAX_SIGNAL_IDENTIFIER_LEN * 2 + 1;
pub(crate) const MAX_SIGNAL_KIND_LEN: usize = 64;
pub(crate) const MAX_SIGNAL_LOCATOR_LEN: usize = 512;
pub(crate) const SIGNAL_OBSERVATION_CONTRACT_V2: &str = "mdp.signal-observation.v2";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(crate) struct DecisionInputSignalProjection {
    pub(crate) id: String,
    pub(crate) kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) roles: Vec<DecisionInputSignalRole>,
    pub(crate) contributor_attribute_ids: Vec<String>,
    pub(crate) value: ValueContract,
    pub(crate) cardinality: DecisionInputSignalCardinality,
    pub(crate) conflict_policy: DecisionInputSignalConflictPolicy,
    pub(crate) decision_effects: Vec<DecisionInputDecisionEffect>,
}

impl DecisionInputSignalProjection {
    pub(crate) fn qualified_id(&self, contract_id: &str) -> String {
        format!("{contract_id}#{}", self.id)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DecisionInputSignalRole {
    Fit,
    WhyNow,
    PersonResolution,
    Disqualifier,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DecisionInputSignalCardinality {
    pub(crate) min: usize,
    pub(crate) max: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DecisionInputSignalConflictPolicy {
    RequireAgreement,
    AnyDisqualifies,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct DecisionInputNormalization {
    #[serde(default)]
    pub(crate) prompt: String,
    #[serde(default)]
    pub(crate) prompt_version: String,
    #[serde(default = "default_decision_input_schema_ref")]
    pub(crate) normalized_schema_ref: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DecisionInputSourceClass {
    UserProvided,
    CustomerSystem,
    ReviewedInternal,
    PublicWeb,
    SyntheticFixture,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct DecisionInputAttribute {
    #[serde(default)]
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) question: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) output_path: String,
    #[serde(default)]
    pub(crate) value: ValueContract,
    pub(crate) requirement: DecisionInputRequirement,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) applies_when: Vec<DecisionInputCondition>,
    #[serde(default)]
    pub(crate) decision_effects: Vec<DecisionInputDecisionEffect>,
    #[serde(default)]
    pub(crate) source_classes: Vec<DecisionInputSourceClass>,
    #[serde(default)]
    pub(crate) provenance: DecisionInputProvenancePolicy,
    #[serde(default)]
    pub(crate) confidence: DecisionInputConfidencePolicy,
    #[serde(default)]
    pub(crate) freshness: DecisionInputFreshnessPolicy,
    #[serde(default)]
    pub(crate) sensitivity: DecisionInputSensitivity,
    #[serde(default)]
    pub(crate) status_behavior: BTreeMap<DecisionInputAttemptStatus, DecisionInputDisposition>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DecisionInputRequirement {
    #[default]
    Required,
    Optional,
    Conditional,
    HardGate,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct DecisionInputCondition {
    #[serde(default)]
    pub(crate) attribute: String,
    pub(crate) operator: DecisionInputConditionOperator,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) values: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DecisionInputConditionOperator {
    #[default]
    Exists,
    Equals,
    NotEquals,
    In,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DecisionInputDecisionEffect {
    Readiness,
    Fit,
    Disqualification,
    Routing,
    Brief,
    Gaps,
    HumanReview,
    NoDraft,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct DecisionInputProvenancePolicy {
    #[serde(default)]
    pub(crate) required: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) required_fields: Vec<DecisionInputProvenanceField>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DecisionInputProvenanceField {
    AttemptId,
    SourceClass,
    SourceLocator,
    ObservedAt,
    Excerpt,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct DecisionInputConfidencePolicy {
    #[serde(default)]
    pub(crate) required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) minimum: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct DecisionInputFreshnessPolicy {
    #[serde(default)]
    pub(crate) required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_age_days: Option<u32>,
    #[serde(default)]
    pub(crate) allow_unknown: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DecisionInputSensitivity {
    #[default]
    Public,
    CustomerPrivate,
    PersonalData,
    Restricted,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DecisionInputAttemptStatus {
    Observed,
    NotFound,
    NotApplicable,
    Blocked,
    Error,
}

impl DecisionInputAttemptStatus {
    pub(crate) const ALL: [Self; 5] = [
        Self::Observed,
        Self::NotFound,
        Self::NotApplicable,
        Self::Blocked,
        Self::Error,
    ];
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DecisionInputDisposition {
    Accept,
    Evaluate,
    Gap,
    Block,
    Disqualify,
    HumanReview,
}

fn default_decision_input_schema_ref() -> String {
    NORMALIZED_DECISION_INPUT_CONTRACT.to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProfileEval {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) required_categories: Vec<String>,
    #[serde(default, skip_serializing_if = "ProfileActivation::is_empty")]
    pub(crate) activation: ProfileActivation,
}

impl ProfileEval {
    pub(crate) fn is_empty(&self) -> bool {
        self.required_categories.is_empty() && self.activation.is_empty()
    }

    pub(crate) fn blocks_activation(&self) -> bool {
        matches!(
            self.activation.status.as_deref(),
            Some("needs-review" | "blocked")
        )
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProfileActivation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) summary: Option<String>,
}

impl ProfileActivation {
    pub(crate) fn is_empty(&self) -> bool {
        self.status.is_none() && self.summary.is_none()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(crate) struct LeadInputRequirements {
    #[serde(default)]
    pub(crate) required_fields: Vec<String>,
    #[serde(default)]
    pub(crate) required_signal_fields: Vec<String>,
    #[serde(default)]
    pub(crate) required_attributes: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) value_contracts: BTreeMap<String, ValueContract>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) attribute_definitions: BTreeMap<String, ValueContract>,
    #[serde(
        default = "default_allow_undeclared_attributes",
        skip_serializing_if = "is_true"
    )]
    pub(crate) allow_undeclared_attributes: bool,
}

impl Default for LeadInputRequirements {
    fn default() -> Self {
        Self {
            required_fields: vec![
                "trigger".to_string(),
                "persona".to_string(),
                "segment".to_string(),
                "signals".to_string(),
            ],
            required_signal_fields: vec!["source".to_string()],
            required_attributes: Vec::new(),
            value_contracts: BTreeMap::new(),
            attribute_definitions: BTreeMap::new(),
            allow_undeclared_attributes: default_allow_undeclared_attributes(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct ValueContract {
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub(crate) value_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) format: Option<String>,
    #[serde(default, rename = "enum", skip_serializing_if = "Vec::is_empty")]
    pub(crate) enum_values: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct QualificationGates {
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) require_person_resolution: bool,
    #[serde(default, skip_serializing_if = "QualificationSignalGates::is_empty")]
    pub(crate) signals: QualificationSignalGates,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fail_policy: Option<QualificationFailPolicy>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct QualificationSignalGates {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) min: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max: Option<usize>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) require_fit_signal: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) require_why_now_signal: bool,
}

impl QualificationSignalGates {
    pub(crate) fn is_empty(&self) -> bool {
        self.min.is_none()
            && self.max.is_none()
            && !self.require_fit_signal
            && !self.require_why_now_signal
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QualificationFailPolicy {
    InsufficientContext,
}

fn default_allow_undeclared_attributes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PersonaMapping {
    pub(crate) persona: String,
    #[serde(default)]
    pub(crate) title_keywords: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CardRef {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) kind: CardKind,
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) personas: Vec<String>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CardKind {
    Personas,
    Pains,
    Motions,
    Hooks,
    AvoidRules,
    OutputRules,
    CopyPatterns,
    Ctas,
    FitRules,
    Claims,
    Signals,
    Positioning,
    ChannelPolicies,
    Objections,
    Gaps,
}

impl CardKind {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Personas => "personas",
            Self::Pains => "pains",
            Self::Motions => "motions",
            Self::Hooks => "hooks",
            Self::AvoidRules => "avoid-rules",
            Self::OutputRules => "output-rules",
            Self::CopyPatterns => "copy-patterns",
            Self::Ctas => "ctas",
            Self::FitRules => "fit-rules",
            Self::Claims => "claims",
            Self::Signals => "signals",
            Self::Positioning => "positioning",
            Self::ChannelPolicies => "channel-policies",
            Self::Objections => "objections",
            Self::Gaps => "gaps",
        }
    }

    pub(crate) fn optional_quota_allowed(&self) -> bool {
        !matches!(
            self,
            Self::Personas | Self::AvoidRules | Self::OutputRules | Self::FitRules
        )
    }

    pub(crate) fn optional_quota_names() -> [&'static str; 11] {
        [
            "pains",
            "motions",
            "hooks",
            "copy-patterns",
            "ctas",
            "claims",
            "signals",
            "positioning",
            "channel-policies",
            "objections",
            "gaps",
        ]
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(crate) struct PromptFile {
    pub(crate) format: String,
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) version: Option<String>,
    #[serde(default)]
    pub(crate) kind: Option<String>,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) target_card_kinds: Vec<CardKind>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    pub(crate) inputs: Vec<PromptInput>,
    pub(crate) instructions: Vec<String>,
    #[serde(default)]
    pub(crate) role: Option<String>,
    #[serde(default)]
    pub(crate) objective: Option<String>,
    #[serde(default)]
    pub(crate) procedure: Vec<String>,
    #[serde(default)]
    pub(crate) selection_rules: Vec<String>,
    #[serde(default)]
    pub(crate) ambiguity_policy: Vec<String>,
    #[serde(default)]
    pub(crate) provenance_policy: Vec<String>,
    #[serde(default)]
    pub(crate) evidence_policy: Vec<String>,
    #[serde(default)]
    pub(crate) negative_examples: Vec<String>,
    #[serde(default)]
    pub(crate) final_checklist: Vec<String>,
    pub(crate) output_contract: PromptOutputContract,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(crate) struct PromptInput {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) required: bool,
    pub(crate) default: String,
    pub(crate) missing_behavior: String,
    #[serde(default)]
    pub(crate) producer: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(crate) struct PromptOutputContract {
    pub(crate) contract: String,
    #[serde(default)]
    pub(crate) output_kind: Option<String>,
    pub(crate) strict_json_only: bool,
    pub(crate) required_top_level: Vec<String>,
    pub(crate) entry_defaults: PromptEntryDefaults,
    #[serde(default)]
    pub(crate) schema_ref: Option<String>,
    #[serde(default)]
    pub(crate) schema: Option<serde_json::Value>,
    pub(crate) example: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(crate) struct PromptEntryDefaults {
    pub(crate) body: String,
    pub(crate) applies_to: Vec<String>,
    pub(crate) evidence: Vec<String>,
    pub(crate) avoid: Vec<String>,
    pub(crate) confidence: String,
    pub(crate) provenance: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Policy {
    pub(crate) progressive_disclosure: bool,
    pub(crate) load_manifest_first: bool,
    pub(crate) max_cards_per_route: usize,
    pub(crate) json_contract: String,
    pub(crate) no_auth_required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Provenance {
    pub(crate) owner: String,
    pub(crate) created_by: String,
    pub(crate) notes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Card {
    pub(crate) id: String,
    pub(crate) kind: CardKind,
    pub(crate) title: String,
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) personas: Vec<String>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    #[serde(default)]
    pub(crate) entries: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Entry {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) body: String,
    #[serde(default)]
    pub(crate) applies_to: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) scope: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub(crate) evidence: Vec<String>,
    #[serde(default)]
    pub(crate) avoid: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) exact_paragraphs: Option<usize>,
    #[serde(default, skip_serializing_if = "EntryConstraints::is_empty")]
    pub(crate) constraints: EntryConstraints,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct EntryConstraints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) word_count: Option<CountConstraint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) subject_words: Option<CountConstraint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) subject_avoid: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_questions: Option<usize>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) forbid_links: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) forbid_attachments: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) forbid_images: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) forbid_html: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) forbid_tracking: bool,
    #[serde(default, skip_serializing_if = "ProofOutputConstraints::is_empty")]
    pub(crate) proof_output: ProofOutputConstraints,
}

impl EntryConstraints {
    pub(crate) fn is_empty(&self) -> bool {
        self.word_count.is_none()
            && self.subject_words.is_none()
            && self.subject_avoid.is_empty()
            && self.max_questions.is_none()
            && !self.forbid_links
            && !self.forbid_attachments
            && !self.forbid_images
            && !self.forbid_html
            && !self.forbid_tracking
            && self.proof_output.is_empty()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct CountConstraint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) min: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) target_min: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) target_max: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProofOutputConstraints {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) required_segment_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) min_segments: BTreeMap<String, usize>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) require_source_refs_for_claims: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_connective_words: Option<usize>,
}

impl ProofOutputConstraints {
    pub(crate) fn is_empty(&self) -> bool {
        self.required_segment_kinds.is_empty()
            && self.min_segments.is_empty()
            && !self.require_source_refs_for_claims
            && self.max_connective_words.is_none()
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct Prospect {
    pub(crate) name: String,
    pub(crate) title: String,
    pub(crate) company: String,
    #[serde(default)]
    pub(crate) company_domain: Option<String>,
    #[serde(default)]
    pub(crate) source_kind: Option<String>,
    #[serde(default)]
    pub(crate) synthetic: bool,
    #[serde(default)]
    pub(crate) linkedin_url: Option<String>,
    #[serde(default)]
    pub(crate) company_url: Option<String>,
    #[serde(default)]
    pub(crate) background: Option<String>,
    #[serde(default)]
    pub(crate) trigger: Option<String>,
    #[serde(default)]
    pub(crate) persona: Option<String>,
    #[serde(default)]
    pub(crate) segment: Option<String>,
    #[serde(default)]
    pub(crate) signals: Vec<Signal>,
    #[serde(default)]
    pub(crate) attributes: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct Signal {
    pub(crate) id: String,
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) source: Option<String>,
    #[serde(default)]
    pub(crate) confidence: Option<String>,
    #[serde(default)]
    pub(crate) freshness: Option<String>,
    #[serde(default)]
    pub(crate) state_as: Option<String>,
}

#[cfg(test)]
mod sourced_signal_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn legacy_signal_shape_remains_unchanged() {
        let signal: Signal = serde_json::from_value(json!({
            "id": "urgent-fit",
            "title": "Strong fit and urgent timing",
            "source": "legacy-import",
            "confidence": "high",
            "freshness": "recent"
        }))
        .expect("legacy signal should deserialize");

        let serialized = serde_json::to_value(signal).expect("legacy signal should serialize");
        assert_eq!(serialized["source"], "legacy-import");
        assert!(serialized.get("contract").is_none());
        assert!(serialized.get("roles").is_none());
    }
}
