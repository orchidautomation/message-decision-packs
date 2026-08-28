use crate::commands::init_transaction::GeneratedArtifact;
use crate::skill_catalog::profile_descriptor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EmbeddedTemplateEntry {
    pub(crate) relative: &'static str,
    pub(crate) bytes: &'static [u8],
    pub(crate) kind: &'static str,
    pub(crate) is_directory: bool,
}

include!(concat!(env!("OUT_DIR"), "/template_inventory.rs"));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TemplatePostprocess {
    Gtm,
    Proposal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TemplateDescriptor {
    pub(crate) id: &'static str,
    pub(crate) default_name: &'static str,
    pub(crate) profile_id: &'static str,
    pub(crate) asset_root: &'static str,
    pub(crate) options: &'static [&'static str],
    pub(crate) required_directories: &'static [&'static str],
    pub(crate) examples: &'static [&'static str],
    pub(crate) postprocess: TemplatePostprocess,
    pub(crate) inventory: &'static [EmbeddedTemplateEntry],
}

const GTM_OPTIONS: &[&str] = &[
    "name",
    "target-name",
    "target-kind",
    "target-alias",
    "exclude-term",
    "include-output-schemas",
];
const PROPOSAL_OPTIONS: &[&str] = &["name"];
const GTM_DIRS: &[&str] = &[
    ".mdp",
    ".mdp/briefs",
    ".mdp/cards",
    ".mdp/evals",
    ".mdp/prompts",
    "examples",
];
const PROPOSAL_DIRS: &[&str] = &[
    ".mdp",
    ".mdp/briefs",
    ".mdp/cards",
    ".mdp/evals",
    ".mdp/prompts",
    "examples",
    "examples/proof-output",
    "examples/proof-output-drafts",
];
const GTM_EXAMPLES: &[&str] = &[
    "examples/clay-row.json",
    "examples/decision-input-scenarios.json",
];
const PROPOSAL_EXAMPLES: &[&str] = &["examples/proof-output/valid-binding.json"];

static DESCRIPTORS: &[TemplateDescriptor] = &[
    TemplateDescriptor {
        id: "gtm",
        default_name: "Example Message Pack",
        profile_id: "gtm",
        asset_root: "basic",
        options: GTM_OPTIONS,
        required_directories: GTM_DIRS,
        examples: GTM_EXAMPLES,
        postprocess: TemplatePostprocess::Gtm,
        inventory: BASIC,
    },
    TemplateDescriptor {
        id: "proposal",
        default_name: "Proposal Reference Profile Sample",
        profile_id: "proposal",
        asset_root: "proposal",
        options: PROPOSAL_OPTIONS,
        required_directories: PROPOSAL_DIRS,
        examples: PROPOSAL_EXAMPLES,
        postprocess: TemplatePostprocess::Proposal,
        inventory: PROPOSAL,
    },
];

fn registry() -> &'static [TemplateDescriptor] {
    validate().expect("canonical template registry must be valid");
    DESCRIPTORS
}
pub(crate) fn descriptors() -> &'static [TemplateDescriptor] {
    registry()
}
pub(crate) fn ids() -> impl Iterator<Item = &'static str> {
    registry().iter().map(|d| d.id)
}
pub(crate) fn available() -> String {
    ids().collect::<Vec<_>>().join(", ")
}
pub(crate) fn lookup(id: &str) -> Option<&'static TemplateDescriptor> {
    registry().iter().find(|d| d.id == id)
}
pub(crate) fn default_name(id: &str) -> Option<&'static str> {
    lookup(id).map(|d| d.default_name)
}

pub(crate) fn validate() -> Result<(), String> {
    let mut ids = std::collections::BTreeSet::new();
    let mut roots = std::collections::BTreeSet::new();
    for descriptor in DESCRIPTORS {
        if !ids.insert(descriptor.id) || !roots.insert(descriptor.asset_root) {
            return Err("duplicate template registry entry".into());
        }
        if profile_descriptor(descriptor.profile_id).is_none_or(|p| p.template_id != descriptor.id)
        {
            return Err(format!(
                "template '{}' has invalid profile association",
                descriptor.id
            ));
        }
        let mut entries = std::collections::BTreeSet::new();
        for entry in descriptor.inventory {
            if !entries.insert(entry.relative) {
                return Err(format!(
                    "template '{}' has duplicate inventory entry",
                    descriptor.id
                ));
            }
            if entry.relative.is_empty()
                || entry.relative.starts_with('/')
                || entry
                    .relative
                    .split('/')
                    .any(|p| p.is_empty() || p == "." || p == ".." || p.contains('\\'))
            {
                return Err(format!(
                    "template '{}' has unsafe inventory path",
                    descriptor.id
                ));
            }
        }
        for required in descriptor.examples {
            if !entries.contains(required) {
                return Err(format!(
                    "template '{}' is missing '{}',",
                    descriptor.id, required
                ));
            }
        }
        if !entries.contains(&".mdp/manifest.yaml") {
            return Err(format!("template '{}' is missing manifest", descriptor.id));
        }
    }
    Ok(())
}

pub(crate) fn inventory(descriptor: &TemplateDescriptor) -> Vec<GeneratedArtifact> {
    descriptor
        .inventory
        .iter()
        .map(|entry| GeneratedArtifact {
            relative: entry.relative.to_string(),
            bytes: entry.bytes.to_vec(),
            kind: entry.kind,
            eligible_for_force: true,
            is_directory: entry.is_directory,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_registry_is_valid() {
        validate().expect("registry");
        assert_eq!(ids().collect::<Vec<_>>(), vec!["gtm", "proposal"]);
    }
}
