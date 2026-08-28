use crate::commands::init_transaction::GeneratedArtifact;
use crate::skill_catalog::profile_descriptor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EmbeddedTemplateEntry {
    pub(crate) relative: &'static str,
    pub(crate) bytes: &'static [u8],
    pub(crate) kind: &'static str,
    pub(crate) is_directory: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EmbeddedTemplateRoot {
    pub(crate) key: &'static str,
    pub(crate) entries: &'static [EmbeddedTemplateEntry],
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
    validate_registry(DESCRIPTORS, EMBEDDED_ROOTS)
}

fn validate_registry(
    descriptors: &[TemplateDescriptor],
    root_entries: &[EmbeddedTemplateRoot],
) -> Result<(), String> {
    let mut ids = std::collections::BTreeSet::new();
    let mut roots_seen = std::collections::BTreeSet::new();
    let mut referenced_roots = std::collections::BTreeSet::new();
    for descriptor in descriptors {
        if !ids.insert(descriptor.id) || !roots_seen.insert(descriptor.asset_root) {
            return Err("duplicate template registry entry".into());
        }
        if profile_descriptor(descriptor.profile_id).is_none_or(|p| p.template_id != descriptor.id)
        {
            return Err(format!(
                "template '{}' has invalid profile association",
                descriptor.id
            ));
        }
        if !referenced_roots.insert(descriptor.asset_root) {
            return Err("duplicate asset root".into());
        }
        let mut options = std::collections::BTreeSet::new();
        for option in descriptor.options {
            if option.is_empty() || !options.insert(option) {
                return Err(format!(
                    "template '{}' has invalid option metadata",
                    descriptor.id
                ));
            }
        }
        let inventory = root_entries
            .iter()
            .find(|root| root.key == descriptor.asset_root)
            .map(|root| root.entries)
            .ok_or_else(|| format!("missing embedded asset root '{}'", descriptor.asset_root))?;
        let mut entries = std::collections::BTreeSet::new();
        for (index, entry) in inventory.iter().enumerate() {
            if index > 0 && inventory[index - 1].relative > entry.relative {
                return Err(format!(
                    "template '{}' inventory is not sorted",
                    descriptor.id
                ));
            }
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
        for required in descriptor.required_directories {
            if required.is_empty()
                || required.starts_with('/')
                || required.contains('\\')
                || required
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
                || descriptor
                    .required_directories
                    .iter()
                    .filter(|candidate| **candidate == *required)
                    .count()
                    != 1
            {
                return Err(format!(
                    "template '{}' has invalid required-directory metadata",
                    descriptor.id
                ));
            }
            if let Some(entry) = inventory.iter().find(|entry| entry.relative == *required)
                && !entry.is_directory
            {
                return Err(format!(
                    "template '{}' requires directory '{}',",
                    descriptor.id, required
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
            if inventory
                .iter()
                .find(|entry| entry.relative == *required)
                .is_some_and(|entry| entry.is_directory)
            {
                return Err(format!(
                    "template '{}' example '{}' must be a file",
                    descriptor.id, required
                ));
            }
        }
        if !entries.contains(&".mdp/manifest.yaml") {
            return Err(format!("template '{}' is missing manifest", descriptor.id));
        }
    }
    for root in root_entries {
        if !referenced_roots.contains(root.key) {
            return Err(format!("unregistered embedded asset root '{}'", root.key));
        }
    }
    Ok(())
}

fn embedded_root(key: &str) -> Option<&'static [EmbeddedTemplateEntry]> {
    EMBEDDED_ROOTS
        .iter()
        .find(|root| root.key == key)
        .map(|root| root.entries)
}

pub(crate) fn inventory(descriptor: &TemplateDescriptor) -> Vec<GeneratedArtifact> {
    embedded_root(descriptor.asset_root)
        .unwrap_or(&[])
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
    use clap::{CommandFactory, Parser};
    #[test]
    fn canonical_registry_is_valid() {
        validate().expect("registry");
        assert_eq!(ids().collect::<Vec<_>>(), vec!["gtm", "proposal"]);
    }

    #[test]
    fn cli_help_and_capabilities_share_registry_order() {
        let help = crate::cli::Cli::command()
            .find_subcommand_mut("init")
            .expect("init command")
            .render_long_help()
            .to_string();
        assert!(
            help.find("gtm").expect("gtm in help")
                < help.find("proposal").expect("proposal in help")
        );
        assert_eq!(
            crate::commands::capabilities::capabilities()["defaults"]["init_templates"],
            serde_json::json!(["gtm", "proposal"])
        );
        assert!(crate::cli::Cli::try_parse_from(["mdp", "init", "--template", "unknown"]).is_err());
    }

    #[test]
    fn injectable_validation_rejects_missing_and_extra_roots() {
        assert!(validate_registry(DESCRIPTORS, &[]).is_err());
        static EXTRA: &[EmbeddedTemplateEntry] = &[];
        let mut roots = EMBEDDED_ROOTS.to_vec();
        roots.push(EmbeddedTemplateRoot {
            key: "future",
            entries: EXTRA,
        });
        assert!(validate_registry(DESCRIPTORS, &roots).is_err());
    }

    #[test]
    fn injectable_validation_rejects_missing_required_directory_and_manifest() {
        static FILES: &[EmbeddedTemplateEntry] = &[EmbeddedTemplateEntry {
            relative: "other",
            bytes: b"x",
            kind: "yaml-file",
            is_directory: false,
        }];
        let descriptor = TemplateDescriptor {
            id: "gtm",
            default_name: "x",
            profile_id: "gtm",
            asset_root: "basic",
            options: GTM_OPTIONS,
            required_directories: &["missing-dir"],
            examples: &[],
            postprocess: TemplatePostprocess::Gtm,
        };
        assert!(
            validate_registry(
                &[descriptor],
                &[EmbeddedTemplateRoot {
                    key: "basic",
                    entries: FILES
                }]
            )
            .is_err()
        );
    }
}
