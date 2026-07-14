use crate::models::{Manifest, ProfileJob};
use crate::pack_io::read_manifest;
use crate::skill_catalog::{BOOTSTRAP_SKILL_IDS, JOB_ROUTE_SPECS, PACKAGED_SKILL_IDS, route_spec};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::path::Path;

const CONTRACT: &str = "mdp.skills.v1";

pub(crate) fn skills(root: Option<&Path>, requested_job: Option<&str>) -> Value {
    let Some(root) = root else {
        return bootstrap_payload(
            true,
            "bootstrap",
            json!({"status": "not-supplied"}),
            json!({"status": "not-supplied"}),
            Vec::new(),
        );
    };

    let manifest = match read_manifest(root) {
        Ok(manifest) => manifest,
        Err(error) => {
            return bootstrap_payload(
                false,
                "bootstrap",
                json!({"path": root.display().to_string(), "status": "unavailable"}),
                json!({"status": "unresolved"}),
                vec![diagnostic(
                    "pack_unavailable",
                    ".mdp/manifest.yaml",
                    error.to_string(),
                )],
            );
        }
    };

    let validation = match crate::commands::health::validate_pack(root) {
        Ok(validation) => validation,
        Err(error) => {
            return bootstrap_payload(
                false,
                "bootstrap",
                pack_payload(&manifest),
                profile_payload(&manifest),
                vec![diagnostic(
                    "pack_validation_unavailable",
                    ".mdp/manifest.yaml",
                    error.to_string(),
                )],
            );
        }
    };
    let mut diagnostics = validation["issues"].as_array().cloned().unwrap_or_default();
    if !validation["valid"].as_bool().unwrap_or(false) {
        return bootstrap_payload(
            false,
            "unresolved",
            pack_payload(&manifest),
            profile_payload(&manifest),
            diagnostics,
        );
    }

    let Some(profile) = manifest.profile.as_ref() else {
        if let Some(job_id) = requested_job {
            diagnostics.push(diagnostic(
                "skills_job_not_found",
                ".mdp/manifest.yaml#/jobs",
                format!("job {job_id} is not available without an active profile"),
            ));
        }
        let mut payload = bootstrap_payload(
            requested_job.is_none(),
            if requested_job.is_none() {
                "ready"
            } else {
                "unresolved"
            },
            pack_payload(&manifest),
            profile_payload(&manifest),
            diagnostics,
        );
        payload["requested_job"] = requested_job.map_or(Value::Null, |job| json!(job));
        return payload;
    };

    let mut routes = JOB_ROUTE_SPECS
        .iter()
        .filter(|spec| spec.profile_id == profile.id)
        .filter_map(|spec| {
            manifest
                .jobs
                .iter()
                .find(|job| job.id == spec.job_id && job.skill_id == spec.skill_id)
                .map(|job| route_payload(&manifest, job))
        })
        .collect::<Vec<_>>();

    let recommendation = if let Some(job_id) = requested_job {
        if route_spec(&profile.id, job_id).is_none() {
            diagnostics.push(diagnostic(
                "skills_job_not_found",
                ".mdp/manifest.yaml#/jobs",
                format!(
                    "job {job_id} is not a supported {} profile route",
                    profile.id
                ),
            ));
            routes.clear();
            Value::Null
        } else if let Some(route) = routes
            .iter()
            .find(|route| route["job_id"] == job_id)
            .cloned()
        {
            routes = vec![route.clone()];
            route
        } else {
            diagnostics.push(diagnostic(
                "skills_job_not_bound",
                ".mdp/manifest.yaml#/jobs",
                format!("job {job_id} has no valid canonical skill binding"),
            ));
            routes.clear();
            Value::Null
        }
    } else {
        Value::Null
    };

    let mut eligible = BOOTSTRAP_SKILL_IDS.to_vec();
    let bound = manifest
        .jobs
        .iter()
        .map(|job| job.skill_id.as_str())
        .collect::<BTreeSet<_>>();
    for skill_id in PACKAGED_SKILL_IDS {
        if bound.contains(skill_id) && !eligible.contains(&skill_id) {
            eligible.push(skill_id);
        }
    }
    let ineligible = PACKAGED_SKILL_IDS
        .iter()
        .filter(|skill_id| !eligible.contains(skill_id))
        .map(|skill_id| {
            json!({
                "skill_id": skill_id,
                "reason": format!("No active {} job binds this skill.", profile.id)
            })
        })
        .collect::<Vec<_>>();
    let valid = diagnostics
        .iter()
        .all(|diagnostic| diagnostic["severity"] != "error");

    json!({
        "contract": CONTRACT,
        "status": if valid { "ready" } else { "unresolved" },
        "valid": valid,
        "pack": pack_payload(&manifest),
        "profile": profile_payload(&manifest),
        "packaged_skill_ids": PACKAGED_SKILL_IDS,
        "host_discovery": host_discovery_payload(),
        "eligibility": {
            "eligible_skill_ids": eligible,
            "ineligible_skills": ineligible
        },
        "requested_job": requested_job,
        "recommendation": recommendation,
        "job_routes": routes,
        "diagnostics": diagnostics
    })
}

fn bootstrap_payload(
    valid: bool,
    status: &str,
    pack: Value,
    profile: Value,
    diagnostics: Vec<Value>,
) -> Value {
    let ineligible = PACKAGED_SKILL_IDS
        .iter()
        .filter(|skill_id| !BOOTSTRAP_SKILL_IDS.contains(skill_id))
        .map(|skill_id| {
            json!({
                "skill_id": skill_id,
                "reason": "No valid active pack job binds this skill."
            })
        })
        .collect::<Vec<_>>();
    json!({
        "contract": CONTRACT,
        "status": status,
        "valid": valid,
        "pack": pack,
        "profile": profile,
        "packaged_skill_ids": PACKAGED_SKILL_IDS,
        "host_discovery": host_discovery_payload(),
        "eligibility": {
            "eligible_skill_ids": BOOTSTRAP_SKILL_IDS,
            "ineligible_skills": ineligible
        },
        "requested_job": Value::Null,
        "recommendation": Value::Null,
        "job_routes": [],
        "diagnostics": diagnostics
    })
}

fn host_discovery_payload() -> Value {
    json!({
        "status": "unobserved",
        "managed_by": "agent-host",
        "guidance": "MDP eligibility does not hide skills already discovered by the host."
    })
}

fn pack_payload(manifest: &Manifest) -> Value {
    json!({
        "id": manifest.id,
        "name": manifest.name,
        "version": manifest.version
    })
}

fn profile_payload(manifest: &Manifest) -> Value {
    manifest.profile.as_ref().map_or_else(
        || json!({"status": "not-supplied"}),
        |profile| {
            json!({
                "id": profile.id,
                "label": profile.label,
                "version": profile.version,
                "context_dimensions": profile.context_dimensions,
                "context_dimension_dependencies": profile.context_dimension_dependencies
            })
        },
    )
}

fn route_payload(manifest: &Manifest, job: &ProfileJob) -> Value {
    let missing_primitives = job
        .required_primitives
        .iter()
        .filter(|primitive| {
            manifest
                .primitive_map
                .get(*primitive)
                .is_none_or(|mapping| mapping.is_empty())
        })
        .cloned()
        .collect::<Vec<_>>();
    json!({
        "job_id": job.id,
        "skill_id": job.skill_id,
        "pack_ready": missing_primitives.is_empty(),
        "missing_primitives": missing_primitives,
        "required_input_contracts": job.input_contracts
    })
}

fn diagnostic(code: &str, path: &str, message: impl Into<String>) -> Value {
    let mut diagnostic = Map::new();
    diagnostic.insert("code".to_string(), json!(code));
    diagnostic.insert("severity".to_string(), json!("error"));
    diagnostic.insert("path".to_string(), json!(path));
    diagnostic.insert("message".to_string(), json!(message.into()));
    Value::Object(diagnostic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn skills_without_pack_reports_canonical_inventory_and_bootstrap_eligibility() {
        let result = skills(None, None);

        assert_eq!(result["contract"], "mdp.skills.v1");
        assert_eq!(
            result["packaged_skill_ids"],
            json!([
                "mdp",
                "mdp-pack-builder",
                "mdp-pack-review",
                "mdp-gtm-brief",
                "mdp-proposal-review"
            ])
        );
        assert_eq!(
            result["eligibility"]["eligible_skill_ids"],
            json!(["mdp", "mdp-pack-builder", "mdp-pack-review"])
        );
        assert_eq!(result["host_discovery"]["status"], "unobserved");
        assert_eq!(result["job_routes"], json!([]));
    }

    #[test]
    fn skills_routes_a_valid_gtm_job_to_one_canonical_skill() {
        let root = temp_root("skills-valid-gtm");
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");

        let result = skills(Some(&root), Some("prospect-fit-or-brief"));

        assert_eq!(result["valid"], true);
        assert_eq!(result["profile"]["id"], "gtm");
        assert_eq!(result["job_routes"].as_array().map(Vec::len), Some(1));
        assert_eq!(result["job_routes"][0]["job_id"], "prospect-fit-or-brief");
        assert_eq!(result["job_routes"][0]["skill_id"], "mdp-gtm-brief");
        assert_eq!(result["job_routes"][0]["pack_ready"], true);
        assert_eq!(result["recommendation"]["skill_id"], "mdp-gtm-brief");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skills_rejects_an_unknown_job_without_falling_back() {
        let root = temp_root("skills-unknown-job");
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");

        let result = skills(Some(&root), Some("write-and-send-campaign"));

        assert_eq!(result["valid"], false);
        assert_eq!(result["job_routes"], json!([]));
        assert!(result["recommendation"].is_null());
        assert!(
            result["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["code"] == "skills_job_not_found")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skills_routes_each_proposal_job_through_one_review_skill() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("plugin")
            .join("assets")
            .join("templates")
            .join("proposal");

        let all_routes = skills(Some(&root), None);
        assert_eq!(all_routes["valid"], true);
        assert_eq!(all_routes["job_routes"].as_array().map(Vec::len), Some(4));
        assert!(
            all_routes["job_routes"]
                .as_array()
                .expect("routes")
                .iter()
                .all(|route| route["skill_id"] == "mdp-proposal-review")
        );

        let selected = skills(Some(&root), Some("compliance-review"));
        assert_eq!(selected["valid"], true);
        assert_eq!(selected["job_routes"].as_array().map(Vec::len), Some(1));
        assert_eq!(selected["recommendation"]["job_id"], "compliance-review");
        assert_eq!(
            selected["recommendation"]["skill_id"],
            "mdp-proposal-review"
        );
    }

    #[test]
    fn skills_rejects_profile_crossing_job_without_fallback() {
        let root = temp_root("skills-profile-crossing");
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");

        let result = skills(Some(&root), Some("compliance-review"));

        assert_eq!(result["valid"], false);
        assert_eq!(result["job_routes"], json!([]));
        assert!(result["recommendation"].is_null());
        assert!(
            result["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["code"] == "skills_job_not_found")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skills_malformed_pack_still_reports_inventory_and_diagnostics() {
        let root = temp_root("skills-malformed-pack");
        let pack_dir = root.join(".mdp");
        std::fs::create_dir_all(&pack_dir).expect("pack directory should be writable");
        std::fs::write(pack_dir.join("manifest.yaml"), "profile: [not: valid")
            .expect("manifest should be writable");

        let result = skills(Some(&root), None);

        assert_eq!(result["contract"], "mdp.skills.v1");
        assert_eq!(result["valid"], false);
        assert_eq!(
            result["packaged_skill_ids"].as_array().map(Vec::len),
            Some(5)
        );
        assert_eq!(result["job_routes"], json!([]));
        assert!(
            result["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["code"] == "pack_unavailable")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skills_missing_pack_returns_diagnostics_and_only_bootstrap_skills() {
        let root = temp_root("skills-missing-pack");

        let result = skills(Some(&root), None);

        assert_eq!(result["valid"], false);
        assert_eq!(
            result["eligibility"]["eligible_skill_ids"],
            json!(["mdp", "mdp-pack-builder", "mdp-pack-review"])
        );
        assert_eq!(result["job_routes"], json!([]));
        assert!(
            result["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["code"] == "pack_unavailable")
        );
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("mdp-{name}-{nonce}"))
    }
}
