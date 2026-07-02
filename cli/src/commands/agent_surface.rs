use crate::models::AgentSurface;
use crate::pack_io::read_manifest;
use anyhow::Result;
use serde_json::{Value, json};
use std::path::Path;

const CONTRACT: &str = "mdp.agent-surface.v0";

pub(crate) fn agent_surface(root: &Path) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let pack = json!({
        "id": manifest.id,
        "name": manifest.name,
        "version": manifest.version
    });

    if let Some(profile) = manifest.profile {
        let profile_id = profile.id;
        let profile_label = profile.label;
        let profile_version = profile.version;
        let surface = profile.agent_surface;
        if surface.is_empty() {
            return Ok(fallback_payload(
                pack,
                json!({
                    "id": profile_id,
                    "label": profile_label,
                    "version": profile_version
                }),
                "This pack has profile metadata but no profile.agent_surface entries, so use generic MDP skills and CLI commands.",
            ));
        }
        return Ok(json!({
            "contract": CONTRACT,
            "pack": pack,
            "profile": {
                "id": profile_id,
                "label": profile_label,
                "version": profile_version
            },
            "agent_surface": agent_surface_payload(surface),
            "legacy_profile": false,
            "guidance": [
                "Use recommended_skills first when they match the requested job.",
                "Treat blocked_skills as deterministic reroute guidance for this pack profile.",
                "If a needed skill is not listed, fall back to the generic mdp skill and CLI commands."
            ]
        }));
    }

    Ok(fallback_payload(
        pack,
        json!({
            "id": "legacy",
            "label": "Legacy MDP pack",
            "version": Value::Null
        }),
        "This pack has no profile.agent_surface metadata, so use generic MDP skills and CLI commands.",
    ))
}

fn fallback_payload(pack: Value, profile: Value, guidance: &str) -> Value {
    json!({
        "contract": CONTRACT,
        "pack": pack,
        "profile": profile,
        "agent_surface": agent_surface_payload(legacy_surface()),
        "legacy_profile": true,
        "guidance": [
            guidance,
            "Add optional profile.agent_surface metadata to make domain-specific skill routing deterministic."
        ]
    })
}

fn legacy_surface() -> AgentSurface {
    AgentSurface {
        recommended_skills: vec![
            "mdp".to_string(),
            "mdp-create-pack".to_string(),
            "mdp-pack-review".to_string(),
            "mdp-pack-eval".to_string(),
        ],
        allowed_skills: vec![
            "mdp".to_string(),
            "mdp-create-pack".to_string(),
            "mdp-source-extract".to_string(),
            "mdp-pack-review".to_string(),
            "mdp-pack-eval".to_string(),
        ],
        blocked_skills: Vec::new(),
        job_skills: Vec::new(),
    }
}

fn agent_surface_payload(surface: AgentSurface) -> Value {
    json!({
        "recommended_skills": surface.recommended_skills,
        "allowed_skills": surface.allowed_skills,
        "blocked_skills": surface.blocked_skills,
        "job_skills": surface.job_skills
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn agent_surface_reads_profile_metadata_from_starter_pack() {
        let root = temp_root("agent-surface-profile");
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");

        let result = agent_surface(&root).expect("surface should load");

        assert_eq!(result["contract"], CONTRACT);
        assert_eq!(result["profile"]["id"], "gtm");
        assert_eq!(result["legacy_profile"], false);
        assert!(
            result["agent_surface"]["recommended_skills"]
                .as_array()
                .expect("recommended skills")
                .iter()
                .any(|skill| skill == "mdp-icp-builder")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn agent_surface_falls_back_for_legacy_manifest() {
        let root = temp_root("agent-surface-legacy");
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let without_profile = raw
            .split_once("personas:\n")
            .map(|(_, tail)| format!("format: mdp.v0\nid: example-message-pack\nname: Example Message Pack\nversion: 0.1.0\npersonas:\n{tail}"))
            .expect("starter manifest should contain personas");
        std::fs::write(&manifest_path, without_profile).expect("manifest should be writable");

        let result = agent_surface(&root).expect("legacy surface should load");

        assert_eq!(result["profile"]["id"], "legacy");
        assert_eq!(result["legacy_profile"], true);
        assert!(
            result["agent_surface"]["recommended_skills"]
                .as_array()
                .expect("recommended skills")
                .iter()
                .any(|skill| skill == "mdp")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn agent_surface_falls_back_when_profile_surface_is_empty() {
        let root = temp_root("agent-surface-empty-profile");
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let without_agent_surface = raw
            .split_once("  agent_surface:\n")
            .and_then(|(head, tail)| {
                tail.split_once("personas:\n")
                    .map(|(_, rest)| format!("{head}personas:\n{rest}"))
            })
            .expect("starter manifest should contain profile.agent_surface");
        std::fs::write(&manifest_path, without_agent_surface).expect("manifest should be writable");

        let result = agent_surface(&root).expect("surface should load");

        assert_eq!(result["profile"]["id"], "gtm");
        assert_eq!(result["legacy_profile"], true);
        assert!(
            result["agent_surface"]["recommended_skills"]
                .as_array()
                .expect("recommended skills")
                .iter()
                .any(|skill| skill == "mdp")
        );
        assert!(
            !result["agent_surface"]["recommended_skills"]
                .as_array()
                .expect("recommended skills")
                .iter()
                .any(|skill| skill == "mdp-icp-builder")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("mdp-{name}-{nonce}"))
    }
}
