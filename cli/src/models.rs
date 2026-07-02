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
    pub(crate) cards: Vec<CardRef>,
    pub(crate) policy: Policy,
    pub(crate) provenance: Provenance,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct Profile {
    #[serde(default)]
    pub(crate) id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<String>,
    #[serde(default, skip_serializing_if = "AgentSurface::is_empty")]
    pub(crate) agent_surface: AgentSurface,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct AgentSurface {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) recommended_skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) allowed_skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) blocked_skills: Vec<BlockedSkill>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) job_skills: Vec<JobSkillRoute>,
}

impl AgentSurface {
    pub(crate) fn is_empty(&self) -> bool {
        self.recommended_skills.is_empty()
            && self.allowed_skills.is_empty()
            && self.blocked_skills.is_empty()
            && self.job_skills.is_empty()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct BlockedSkill {
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
pub(crate) struct JobSkillRoute {
    #[serde(default)]
    pub(crate) job: String,
    #[serde(default)]
    pub(crate) skills: Vec<String>,
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

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PromptFile {
    pub(crate) format: String,
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) target_card_kinds: Vec<CardKind>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    pub(crate) inputs: Vec<PromptInput>,
    pub(crate) instructions: Vec<String>,
    pub(crate) output_contract: PromptOutputContract,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PromptInput {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) required: bool,
    pub(crate) default: String,
    pub(crate) missing_behavior: String,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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
