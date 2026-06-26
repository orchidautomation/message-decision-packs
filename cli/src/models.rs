use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Manifest {
    pub(crate) format: String,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) description: Option<String>,
    pub(crate) personas: Vec<String>,
    #[serde(default)]
    pub(crate) target_personas: Vec<String>,
    #[serde(default)]
    pub(crate) operator_roles: Vec<String>,
    #[serde(default)]
    pub(crate) supported_channels: Vec<String>,
    #[serde(default)]
    pub(crate) persona_mappings: Vec<PersonaMapping>,
    pub(crate) cards: Vec<CardRef>,
    pub(crate) policy: Policy,
    pub(crate) provenance: Provenance,
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct Prospect {
    pub(crate) name: String,
    pub(crate) title: String,
    pub(crate) company: String,
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
