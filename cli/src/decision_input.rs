//! The private, profile-neutral boundary used by ingress code.
//!
//! This module intentionally contains no serialized contract.  The public
//! prospect/opportunity spellings are compatibility adapters at this boundary;
//! the rest of the core only sees fields, signals, and attributes.

use crate::models::{LeadInputRequirements, Manifest, ProfileJob, Prospect};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DecisionInput {
    fields: BTreeMap<String, Value>,
    signals: Vec<Value>,
    attributes: BTreeMap<String, Value>,
}

impl DecisionInput {
    pub(crate) fn new(
        fields: BTreeMap<String, Value>,
        signals: Vec<Value>,
        attributes: BTreeMap<String, Value>,
    ) -> Result<Self, AdapterError> {
        if fields.len() > 64 || signals.len() > 128 || attributes.len() > 256 {
            return Err(AdapterError::Invalid(
                "decision input exceeds bounded limits",
            ));
        }
        Ok(Self {
            fields,
            signals,
            attributes,
        })
    }

    pub(crate) fn field(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }
    pub(crate) fn signals(&self) -> &[Value] {
        &self.signals
    }
    pub(crate) fn attributes(&self) -> &BTreeMap<String, Value> {
        &self.attributes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdapterKind {
    GtmProspect,
    ProposalOpportunity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AdapterError {
    MissingOwnership,
    UnknownOwnership(String),
    MixedOwnership,
    Invalid(&'static str),
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingOwnership => write!(f, "decision input adapter ownership is missing"),
            Self::UnknownOwnership(value) => {
                write!(f, "unsupported decision input ownership: {value}")
            }
            Self::MixedOwnership => {
                write!(f, "decision input adapter ownership is ambiguous or mixed")
            }
            Self::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for AdapterError {}

/// Selects an adapter once, from the manifest and the declared input contract.
/// Payload field names are deliberately not consulted.
pub(crate) fn select_adapter(
    manifest: &Manifest,
    input_contracts: &[&str],
) -> Result<AdapterKind, AdapterError> {
    let profile = manifest
        .profile
        .as_ref()
        .map(|p| p.id.trim())
        .filter(|p| !p.is_empty())
        .ok_or(AdapterError::MissingOwnership)?;
    if input_contracts.len() != 1 {
        return Err(if input_contracts.is_empty() {
            AdapterError::MissingOwnership
        } else {
            AdapterError::MixedOwnership
        });
    }
    let input = input_contracts[0].trim();
    match (profile, input) {
        ("gtm", "prospect") => Ok(AdapterKind::GtmProspect),
        ("proposal", "opportunity") => Ok(AdapterKind::ProposalOpportunity),
        (_, value) => Err(AdapterError::UnknownOwnership(format!(
            "{profile} + {value}"
        ))),
    }
}

pub(crate) fn select_adapter_for_job(
    manifest: &Manifest,
    job: &ProfileJob,
) -> Result<AdapterKind, AdapterError> {
    let ids = job
        .input_contracts
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    select_adapter(manifest, &ids)
}

pub(crate) fn requirements_view(requirements: &LeadInputRequirements) -> RequirementsView<'_> {
    RequirementsView { requirements }
}

pub(crate) struct RequirementsView<'a> {
    requirements: &'a LeadInputRequirements,
}
impl<'a> RequirementsView<'a> {
    pub(crate) fn required_fields(&self) -> &'a [String] {
        &self.requirements.required_fields
    }
    pub(crate) fn required_signal_fields(&self) -> &'a [String] {
        &self.requirements.required_signal_fields
    }
    pub(crate) fn required_attributes(&self) -> &'a [String] {
        &self.requirements.required_attributes
    }
    pub(crate) fn value_contracts(&self) -> &'a BTreeMap<String, crate::models::ValueContract> {
        &self.requirements.value_contracts
    }
    pub(crate) fn attribute_definitions(
        &self,
    ) -> &'a BTreeMap<String, crate::models::ValueContract> {
        &self.requirements.attribute_definitions
    }
    pub(crate) fn allow_undeclared_attributes(&self) -> bool {
        self.requirements.allow_undeclared_attributes
    }
}

pub(crate) fn from_gtm_prospect(prospect: &Prospect) -> Result<DecisionInput, AdapterError> {
    let value = serde_json::to_value(prospect)
        .map_err(|_| AdapterError::Invalid("prospect could not be converted"))?;
    from_wire_object(
        value
            .as_object()
            .ok_or(AdapterError::Invalid("prospect must be an object"))?,
        AdapterKind::GtmProspect,
    )
}

/// Adapt the governed GTM projection.  This deliberately has a separate
/// entry point from proposal output: the two public producers happen to use
/// compatible v0 field spellings, but ownership is never inferred from those
/// spellings.
pub(crate) fn from_gtm_normalized(
    prospect: &Map<String, Value>,
) -> Result<DecisionInput, AdapterError> {
    from_wire_object(prospect, AdapterKind::GtmProspect)
}

pub(crate) fn from_proposal_output(
    output: &Map<String, Value>,
) -> Result<DecisionInput, AdapterError> {
    let legacy = output.get("normalized_prospect");
    let opportunity = output.get("normalized_opportunity");
    let selected = match (legacy, opportunity) {
        (Some(a), Some(b)) if a == b => a,
        (Some(_), Some(_)) => {
            return Err(AdapterError::Invalid(
                "normalized_opportunity must exactly match normalized_prospect",
            ));
        }
        (Some(a), None) => a,
        // The opportunity spelling is a readable proposal alias, not a new
        // unversioned producer contract.  It is accepted only alongside its
        // required v0 compatibility peer.
        (None, Some(_)) => {
            return Err(AdapterError::Invalid(
                "normalized_opportunity requires normalized_prospect",
            ));
        }
        (None, None) => {
            return Err(AdapterError::Invalid(
                "proposal normalization is missing its decision input",
            ));
        }
    };
    let object = selected.as_object().ok_or(AdapterError::Invalid(
        "normalized decision input must be an object",
    ))?;
    from_wire_object(object, AdapterKind::ProposalOpportunity)
}

fn from_wire_object(
    object: &Map<String, Value>,
    kind: AdapterKind,
) -> Result<DecisionInput, AdapterError> {
    // Both adapters read the established v0 scalar vocabulary.  Keeping the
    // match here makes the ownership distinction explicit and prevents a
    // future adapter from silently inheriting this wire contract.
    let allowed: BTreeSet<&str> = match kind {
        AdapterKind::GtmProspect | AdapterKind::ProposalOpportunity => [
            "name",
            "title",
            "company",
            "company_domain",
            "source_kind",
            "synthetic",
            "linkedin_url",
            "company_url",
            "background",
            "trigger",
            "persona",
            "segment",
            "signals",
            "attributes",
        ]
        .into_iter()
        .collect(),
    };
    if object.keys().any(|key| !allowed.contains(key.as_str())) {
        return Err(AdapterError::Invalid(
            "decision input contains an unknown field",
        ));
    }
    let mut fields = BTreeMap::new();
    let mut signals = Vec::new();
    let mut attributes = BTreeMap::new();
    for (key, value) in object {
        match key.as_str() {
            // serde emits null for omitted Option fields on typed Prospect;
            // null is absence, not a neutral scalar value.
            _ if value.is_null() => {}
            "signals" => {
                signals = value
                    .as_array()
                    .ok_or(AdapterError::Invalid("signals must be an array"))?
                    .iter()
                    .map(|signal| {
                        let signal = signal
                            .as_object()
                            .ok_or(AdapterError::Invalid("signals must contain objects"))?;
                        let allowed = [
                            "id",
                            "title",
                            "source",
                            "confidence",
                            "freshness",
                            "state_as",
                        ];
                        if signal.keys().any(|key| !allowed.contains(&key.as_str())) {
                            return Err(AdapterError::Invalid(
                                "decision input contains an unknown signal field",
                            ));
                        }
                        Ok(Value::Object(
                            signal
                                .iter()
                                .filter(|(_, value)| !value.is_null())
                                .map(|(key, value)| (key.clone(), value.clone()))
                                .collect(),
                        ))
                    })
                    .collect::<Result<Vec<_>, AdapterError>>()?
            }
            "attributes" => {
                attributes = value
                    .as_object()
                    .ok_or(AdapterError::Invalid("attributes must be an object"))?
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            }
            _ if value.is_string() || value.is_number() || value.is_boolean() => {
                fields.insert(key.clone(), value.clone());
            }
            _ => {
                return Err(AdapterError::Invalid(
                    "decision input fields must be scalar values",
                ));
            }
        }
    }
    DecisionInput::new(fields, signals, attributes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Profile, Prospect};
    use serde_json::json;

    #[test]
    fn neutral_input_does_not_require_person_vocabulary() {
        let input = DecisionInput::new(
            BTreeMap::from([("status".into(), json!("open"))]),
            vec![],
            BTreeMap::new(),
        )
        .unwrap();
        assert!(input.field("title").is_none());
        assert_eq!(input.field("status"), Some(&json!("open")));

        let requirements = LeadInputRequirements {
            required_fields: vec!["status".into()],
            required_signal_fields: Vec::new(),
            required_attributes: Vec::new(),
            value_contracts: BTreeMap::new(),
            attribute_definitions: BTreeMap::new(),
            allow_undeclared_attributes: true,
        };
        assert!(
            crate::value_contracts::decision_input_contract_violations(
                &requirements_view(&requirements),
                &input,
            )
            .is_empty()
        );
    }

    #[test]
    fn selection_is_closed_and_does_not_guess_from_payload() {
        let manifest = Manifest {
            profile: Some(Profile {
                id: "proposal".into(),
                ..Default::default()
            }),
            ..test_manifest()
        };
        assert_eq!(
            select_adapter(&manifest, &["opportunity"]).unwrap(),
            AdapterKind::ProposalOpportunity
        );
        assert!(select_adapter(&manifest, &["prospect"]).is_err());
        assert!(matches!(
            select_adapter(&manifest, &[]),
            Err(AdapterError::MissingOwnership)
        ));
        assert!(matches!(
            select_adapter(&manifest, &["opportunity", "prospect"]),
            Err(AdapterError::MixedOwnership)
        ));

        let unknown = Manifest {
            profile: Some(Profile {
                id: "unknown".into(),
                ..Default::default()
            }),
            ..test_manifest()
        };
        assert!(matches!(
            select_adapter(&unknown, &["opportunity"]),
            Err(AdapterError::UnknownOwnership(_))
        ));
    }

    #[test]
    fn proposal_alias_must_be_exact() {
        let mut output = Map::new();
        output.insert("normalized_prospect".into(), json!({"status":"open"}));
        output.insert("normalized_opportunity".into(), json!({"status":"closed"}));
        assert!(from_proposal_output(&output).is_err());
    }

    #[test]
    fn proposal_alias_cannot_become_an_opportunity_only_wire_shape() {
        let mut output = Map::new();
        output.insert(
            "normalized_opportunity".into(),
            json!({"company": "ExampleCo"}),
        );
        assert!(matches!(
            from_proposal_output(&output),
            Err(AdapterError::Invalid(
                "normalized_opportunity requires normalized_prospect"
            ))
        ));
    }

    #[test]
    fn wire_adapter_does_not_authorize_unversioned_opportunity_fields() {
        let mut output = Map::new();
        output.insert(
            "normalized_prospect".into(),
            json!({"name": "Taylor", "title": "Lead", "company": "ExampleCo", "amount": 10}),
        );
        assert!(from_proposal_output(&output).is_err());
    }

    #[test]
    fn old_proposal_prospect_only_artifact_remains_readable() {
        let mut output = Map::new();
        output.insert(
            "normalized_prospect".into(),
            json!({"name": "Taylor", "title": "Lead", "company": "ExampleCo"}),
        );
        assert!(from_proposal_output(&output).is_ok());
    }

    fn test_manifest() -> Manifest {
        serde_json::from_value(json!({"format":"mdp.manifest.v0","id":"x","name":"x","version":"0","personas":[],"cards":[],"policy":{"progressive_disclosure":false,"load_manifest_first":true,"max_cards_per_route":1,"json_contract":"x","no_auth_required":true},"provenance":{"owner":"x","created_by":"x","notes":[]}})).unwrap()
    }
    #[allow(dead_code)]
    fn _prospect_type_is_kept() {
        let _: Option<Prospect> = None;
    }
}
