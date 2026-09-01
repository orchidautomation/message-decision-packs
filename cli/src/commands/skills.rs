use crate::commands::health::profile_activation_decision;
use crate::models::{Manifest, ProfileJob};
use crate::pack_io::read_manifest;
use crate::product_foundation::{
    ProductFoundationClassification, ProductFoundationResolution, apply_validation_errors,
    apply_validation_errors_for_job, resolve_product_foundation_for_pack,
    validation_errors_block_job, validation_issues_for_job,
};
use crate::skill_catalog::{
    BOOTSTRAP_SKILL_IDS, PACKAGED_SKILL_IDS, profile_descriptor, route_spec,
};
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
            requested_job,
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
                requested_job,
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
                requested_job,
                vec![diagnostic(
                    "pack_validation_unavailable",
                    ".mdp/manifest.yaml",
                    error.to_string(),
                )],
            );
        }
    };
    let pack_valid = validation["valid"].as_bool().unwrap_or(false);
    let profile_activation = profile_activation_decision(
        &validation,
        manifest.profile_eval.blocks_activation(),
        requested_job,
    );
    let activation_blocked = profile_activation["status"] == "blocked";
    let mut diagnostics = validation["issues"].as_array().cloned().unwrap_or_default();
    if !pack_valid
        && !crate::commands::requirements::validation_has_only_foundation_errors(&validation)
    {
        return bootstrap_payload(
            false,
            "unresolved",
            pack_payload(&manifest),
            profile_payload(&manifest),
            requested_job,
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
        return bootstrap_payload(
            requested_job.is_none(),
            if requested_job.is_none() {
                "ready"
            } else {
                "unresolved"
            },
            pack_payload(&manifest),
            profile_payload(&manifest),
            requested_job,
            diagnostics,
        );
    };

    if let Some(job_id) = requested_job
        && let Some(job) = manifest.jobs.iter().find(|job| job.id == job_id)
        && let Ok(resolution) = resolve_product_foundation_for_pack(root, &manifest, &job.id)
    {
        diagnostics = validation_issues_for_job(&manifest, &resolution, &diagnostics);
    }

    let mut routes = Vec::new();
    let Some(descriptor) = profile_descriptor(&profile.id) else {
        diagnostics.push(diagnostic(
            "skills_profile_unknown",
            ".mdp/manifest.yaml#/profile/id",
            format!(
                "profile {} is not in the closed profile registry",
                profile.id
            ),
        ));
        return bootstrap_payload(
            false,
            "unresolved",
            pack_payload(&manifest),
            profile_payload(&manifest),
            requested_job,
            diagnostics,
        );
    };
    for spec in descriptor.jobs {
        let Some(job) = manifest
            .jobs
            .iter()
            .find(|job| job.id == spec.job_id && job.skill_id == spec.skill_id)
        else {
            continue;
        };
        match resolve_product_foundation_for_pack(root, &manifest, &job.id) {
            Ok(mut product_foundation) => {
                if let Some(issues) = validation["issues"].as_array() {
                    if requested_job.is_some() {
                        apply_validation_errors_for_job(&mut product_foundation, &manifest, issues);
                    } else {
                        apply_validation_errors(&mut product_foundation, issues);
                    }
                }
                let route_pack_valid = if requested_job.is_some() {
                    !validation_errors_block_job(
                        &manifest,
                        &product_foundation,
                        validation["issues"]
                            .as_array()
                            .map(Vec::as_slice)
                            .unwrap_or(&[]),
                    )
                } else {
                    pack_valid
                };
                routes.push(route_payload(
                    &manifest,
                    job,
                    &product_foundation,
                    &profile_activation,
                    route_pack_valid,
                ));
            }
            Err(error) => diagnostics.push(diagnostic(
                "product_foundation_resolution_failed",
                ".mdp/manifest.yaml#/cards",
                error.to_string(),
            )),
        }
    }

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
        "status": if valid && !activation_blocked { "ready" } else { "unresolved" },
        "valid": valid,
        "pack": pack_payload(&manifest),
        "profile": profile_payload(&manifest),
        "profile_activation": profile_activation,
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
    requested_job: Option<&str>,
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
        "profile_activation": {
            "contract": "mdp.profile-activation-decision.v1",
            "status": "unavailable",
            "activation_ready": Value::Null,
            "blocker_codes": [],
            "diagnostics": []
        },
        "packaged_skill_ids": PACKAGED_SKILL_IDS,
        "host_discovery": host_discovery_payload(),
        "eligibility": {
            "eligible_skill_ids": BOOTSTRAP_SKILL_IDS,
            "ineligible_skills": ineligible
        },
        "requested_job": requested_job,
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

fn route_payload(
    manifest: &Manifest,
    job: &ProfileJob,
    product_foundation: &ProductFoundationResolution,
    profile_activation: &Value,
    pack_valid: bool,
) -> Value {
    let model_task = job.model_task.as_ref().map_or_else(
        || json!({"status": "unassessed"}),
        |binding| {
            json!({
                "status": "declared",
                "kind": binding.kind,
                "prompt": binding.prompt,
                "inspect_with": format!("mdp --json requirements --job {}", job.id)
            })
        },
    );
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
    let selected_facet_ids = product_foundation
        .selected_facets
        .iter()
        .map(|facet| facet.id.clone())
        .collect::<Vec<_>>();
    let required_facet_ids = product_foundation
        .selected_facets
        .iter()
        .filter(|facet| facet.classification == ProductFoundationClassification::Required)
        .map(|facet| facet.id.clone())
        .collect::<Vec<_>>();
    json!({
        "job_id": job.id,
        "skill_id": job.skill_id,
        "pack_ready": missing_primitives.is_empty()
            && pack_valid
            && profile_activation["status"] != "blocked"
            && !product_foundation.blocks_activation(),
        "missing_primitives": missing_primitives,
        "required_input_contracts": job.input_contracts,
        "model_task": model_task,
        "profile_activation": profile_activation,
        "product_foundation": {
            "status": product_foundation.status,
            "selected_facet_ids": selected_facet_ids,
            "required_facet_ids": required_facet_ids,
            "diagnostics": product_foundation.diagnostics
        },
        "readiness_policy": "Product foundation, a declared model-task contract, pack validation, and computed profile activation may veto readiness. Inspect the exact compiled prompt with mdp requirements --job."
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
                "mdp-pack-apply"
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
        assert_eq!(result["job_routes"][0]["skill_id"], "mdp-pack-apply");
        assert_eq!(result["job_routes"][0]["pack_ready"], true);
        assert_eq!(
            result["job_routes"][0]["product_foundation"]["status"],
            "ready"
        );
        assert_eq!(result["recommendation"]["skill_id"], "mdp-pack-apply");
        jsonschema::draft202012::validate(
            &crate::commands::schemas::schema(crate::cli::SchemaTarget::Skills),
            &result,
        )
        .expect("skills output should satisfy its additive schema");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skills_computed_activation_vetoes_ready_and_pack_ready() {
        let root = temp_root("skills-computed-activation");
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        for entry in std::fs::read_dir(root.join(".mdp/evals")).expect("evals should be readable") {
            let path = entry.expect("eval entry should load").path();
            let raw = std::fs::read_to_string(&path).expect("eval should be readable");
            std::fs::write(
                path,
                raw.replace("category: prompt-output-validation", "category: proceed"),
            )
            .expect("eval should be writable");
        }

        let result = skills(Some(&root), Some("prospect-fit-or-brief"));

        assert_eq!(result["status"], "unresolved");
        assert_eq!(result["valid"], true);
        assert_eq!(result["profile_activation"]["status"], "blocked");
        assert_eq!(result["job_routes"][0]["pack_ready"], false);
        assert_eq!(result["recommendation"]["pack_ready"], false);
        assert!(
            result["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["code"] == "profile_eval_category_missing")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skills_missing_required_primitive_vetoes_pack_ready() {
        let root = temp_root("skills-missing-required-primitive");
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace(
                "  gaps:\n    cards:\n    - gaps\n    evals:\n    - fit-insufficient-context\n    - brief-insufficient-context\n    - account-context-missing\n    - account-only-no-draft\n    - decision-input-contract\n",
                "",
            ),
        )
        .expect("manifest should be writable");

        let result = skills(Some(&root), Some("prospect-fit-or-brief"));

        assert_eq!(result["status"], "unresolved");
        assert_eq!(result["valid"], true);
        assert_eq!(result["profile_activation"]["status"], "blocked");
        assert_eq!(result["recommendation"]["pack_ready"], false);
        assert!(
            result["profile_activation"]["blocker_codes"]
                .as_array()
                .expect("blocker codes")
                .iter()
                .any(|code| code == "profile_required_primitive_unmapped")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skills_needs_review_activation_vetoes_pack_ready() {
        let root = temp_root("skills-needs-review");
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile_eval"]["activation"]["status"] =
            serde_yaml::Value::String("needs-review".to_string());
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let result = skills(Some(&root), Some("prospect-fit-or-brief"));

        assert_eq!(result["job_routes"][0]["pack_ready"], false);
        assert_eq!(result["recommendation"]["pack_ready"], false);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skills_foundation_validation_errors_veto_every_pack_ready_projection() {
        let root = temp_root("skills-invalid-foundation");
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile"]["product_foundation"]["facets"][0]["kind"] =
            serde_yaml::Value::String("unknown_foundation_kind".to_string());
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let all_routes = skills(Some(&root), None);

        assert_eq!(all_routes["valid"], false);
        assert_eq!(all_routes["status"], "unresolved");
        assert!(
            all_routes["job_routes"]
                .as_array()
                .expect("job routes")
                .iter()
                .all(|route| route["pack_ready"] == false)
        );
        assert!(
            all_routes["job_routes"]
                .as_array()
                .expect("job routes")
                .iter()
                .all(|route| route["product_foundation"]["status"] == "blocked")
        );
        assert!(
            all_routes["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .any(|diagnostic| diagnostic["code"] == "product_foundation_facet_kind_unknown")
        );

        let selected = skills(Some(&root), Some("prospect-fit-or-brief"));
        assert_eq!(selected["recommendation"]["pack_ready"], false);
        assert_eq!(selected["job_routes"][0]["pack_ready"], false);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skills_selected_job_ignores_other_job_foundation_errors() {
        let root = temp_root("skills-unrelated-foundation-error");
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["jobs"][1]["product_foundation"]["conditional"] = serde_yaml::from_str(
            r#"
- facet_id: product-identity
  when:
    fact: unsupported_fact
    equals: outbound-copy-brief
"#,
        )
        .expect("conditional binding should parse");
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let result = skills(Some(&root), Some("prospect-fit-or-brief"));

        assert_eq!(result["valid"], true);
        assert_eq!(result["status"], "ready");
        assert_eq!(result["recommendation"]["pack_ready"], true);
        assert_eq!(result["job_routes"][0]["pack_ready"], true);
        assert!(
            result["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .iter()
                .all(|diagnostic| {
                    diagnostic["code"] != "product_foundation_condition_fact_unknown"
                })
        );

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
                .all(|route| route["skill_id"] == "mdp-pack-apply")
        );

        let selected = skills(Some(&root), Some("compliance-review"));
        assert_eq!(selected["valid"], true);
        assert_eq!(selected["job_routes"].as_array().map(Vec::len), Some(1));
        assert_eq!(selected["recommendation"]["job_id"], "compliance-review");
        assert_eq!(selected["recommendation"]["skill_id"], "mdp-pack-apply");
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
            Some(4)
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

    #[test]
    fn skills_missing_pack_preserves_the_requested_job() {
        let root = temp_root("skills-missing-pack-requested-job");

        let result = skills(Some(&root), Some("prospect-fit-or-brief"));

        assert_eq!(result["valid"], false);
        assert_eq!(result["requested_job"], "prospect-fit-or-brief");
        assert!(result["recommendation"].is_null());
    }

    fn add_genuine_gap_to_facet(root: &std::path::Path, facet_id: &str) {
        // Author a genuine missing-authority gap entry in the gaps card.
        let gaps_path = root.join(".mdp/cards/gaps.yaml");
        let raw = std::fs::read_to_string(&gaps_path).expect("gaps card should be readable");
        let mut gaps: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("gaps card should parse");
        let new_entry: serde_yaml::Value = serde_yaml::from_str(
            r#"
- id: missing-portfolio-alternatives
  title: Missing portfolio-wide alternatives evidence
  body: No approved source establishes portfolio-wide alternative positioning. Do not extrapolate from one case; route to the responsible reviewer.
  applies_to: []
  evidence: []
  avoid: []
"#,
        )
        .expect("gap entry should parse");
        if let serde_yaml::Value::Sequence(new_entries) = new_entry {
            if let Some(existing) = gaps["entries"].as_sequence_mut() {
                existing.extend(new_entries);
            }
        }
        std::fs::write(
            &gaps_path,
            serde_yaml::to_string(&gaps).expect("gaps should serialize"),
        )
        .expect("gaps card should be writable");

        // Bind that gap ref onto the selected facet so a genuine hole blocks the
        // job while approved boundaries remain entries on other facets.
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: serde_yaml::Value =
            serde_yaml::from_str(&raw).expect("manifest should parse");
        let facets = manifest["profile"]["product_foundation"]["facets"]
            .as_sequence_mut()
            .expect("facets should be a sequence");
        for facet in facets.iter_mut() {
            if facet["id"].as_str() == Some(facet_id) {
                facet["gaps"] = serde_yaml::from_str(
                    "- card_id: gaps\n  entry_id: missing-portfolio-alternatives\n",
                )
                .expect("gap ref should parse");
            }
        }
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    #[test]
    fn skills_approved_boundary_entries_stay_ready_while_genuine_gap_blocks_job() {
        let root = temp_root("skills-gap-classification");
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        // The starter authors approved boundaries (no-unsourced-claims,
        // no-context-no-copy, no-false-urgency) as entries on proof-boundaries,
        // avoid-rules, and output-rules facets. Keep those as entries. Add one
        // genuine missing-authority gap onto the alternatives facet so only the
        // job that requires alternatives blocks.
        add_genuine_gap_to_facet(&root, "alternatives");

        let ready = skills(Some(&root), Some("prospect-fit-or-brief"));
        assert_eq!(ready["valid"], true);
        assert_eq!(ready["status"], "ready");
        assert_eq!(ready["job_routes"][0]["pack_ready"], true);
        assert_eq!(
            ready["job_routes"][0]["product_foundation"]["status"],
            "ready"
        );
        assert!(
            ready["job_routes"][0]["product_foundation"]["selected_facet_ids"]
                .as_array()
                .expect("selected facet ids")
                .iter()
                .any(|id| id == "proof-boundaries")
        );

        let blocked = skills(Some(&root), Some("outbound-copy-review"));
        assert_eq!(blocked["valid"], true);
        assert_eq!(blocked["job_routes"][0]["pack_ready"], false);
        assert_eq!(
            blocked["job_routes"][0]["product_foundation"]["status"],
            "blocked"
        );
        assert!(
            blocked["job_routes"][0]["product_foundation"]["diagnostics"]
                .as_array()
                .expect("foundation diagnostics")
                .iter()
                .any(|diagnostic| {
                    diagnostic["code"] == "product_foundation_selected_facet_has_gaps"
                })
        );
        assert_eq!(blocked["recommendation"]["pack_ready"], false);

        let requirements =
            crate::commands::requirements::requirements(&root, "outbound-copy-review")
                .expect("requirements should compile");
        let selected = requirements["product_foundation"]["selected_facets"]
            .as_array()
            .expect("selected facets should be present");
        let alternatives = selected
            .iter()
            .find(|facet| facet["id"] == "alternatives")
            .expect("alternatives facet should be selected");
        assert!(
            alternatives["gap_refs"]
                .as_array()
                .expect("gap refs")
                .iter()
                .any(|reference| {
                    reference["card_id"] == "gaps"
                        && reference["entry_id"] == "missing-portfolio-alternatives"
                })
        );
        assert!(
            alternatives["entries"]
                .as_array()
                .is_some_and(|entries| !entries.is_empty())
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skills_pack_with_every_job_blocked_cannot_be_called_ready() {
        let root = temp_root("skills-all-jobs-blocked");
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        // proof-boundaries is required by all three canonical GTM jobs, so a
        // genuine gap on it blocks every advertised job at once.
        add_genuine_gap_to_facet(&root, "proof-boundaries");

        let all_routes = skills(Some(&root), None);
        assert_eq!(all_routes["valid"], true);
        // The CLI surfaces every advertised job as pack_ready:false; the builder
        // handoff must not upgrade this into a complete/ready claim.
        assert!(
            all_routes["job_routes"]
                .as_array()
                .expect("job routes")
                .iter()
                .all(|route| route["pack_ready"] == false)
        );
        assert!(
            all_routes["job_routes"]
                .as_array()
                .expect("job routes")
                .iter()
                .all(|route| route["product_foundation"]["status"] == "blocked")
        );
        assert!(
            all_routes["job_routes"]
                .as_array()
                .expect("job routes")
                .iter()
                .all(|route| {
                    route["product_foundation"]["diagnostics"]
                        .as_array()
                        .expect("foundation diagnostics")
                        .iter()
                        .any(|diagnostic| {
                            diagnostic["code"] == "product_foundation_selected_facet_has_gaps"
                        })
                })
        );

        for job_id in [
            "prospect-fit-or-brief",
            "outbound-copy-brief",
            "outbound-copy-review",
        ] {
            let selected = skills(Some(&root), Some(job_id));
            assert_eq!(selected["recommendation"]["pack_ready"], false);
            assert_eq!(
                selected["job_routes"][0]["product_foundation"]["status"],
                "blocked"
            );
        }

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
