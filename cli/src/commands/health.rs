use crate::constants::{
    DEFAULT_DIR, FORMAT_NAME, FORMAT_VERSION, PROMPT_CARD_PATCH_SCHEMA_REF, PROMPT_FORMAT_VERSION,
    PROMPT_OUTPUT_CONTRACT, PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF,
};
use crate::models::{
    AgentSurface, CardKind, InputContract, Manifest, PrimitiveMapping, Profile, ProfileEval,
    ProfileJob, PromptFile, ValueContract,
};
use crate::pack_io::{
    display_pack_path, read_card, read_card_by_id, read_manifest, read_prompt, resolve_pack_path,
};
use crate::routing::select_cards;
use crate::value_contracts::PROSPECT_CONTRACT_FIELDS;
use anyhow::Result;
use serde_json::{Value, json};
use serde_yaml::Value as YamlValue;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub(crate) const KNOWN_PRIMITIVES: &[&str] = &[
    "actors",
    "decision-criteria",
    "source-signals",
    "needs-requirements",
    "evidence-proof",
    "boundaries",
    "output-contracts",
    "routing-jobs",
    "gaps",
    "evals",
];

pub(crate) const KNOWN_PROFILE_EVAL_CATEGORIES: &[&str] = &[
    "proceed",
    "insufficient-context",
    "refusal",
    "unsafe-output",
    "job-routing",
    "account-context-present",
    "account-context-missing",
    "account-only-no-draft",
    "prompt-output-validation",
];

pub(crate) fn doctor(root: &Path) -> Value {
    let pack_dir = root.join(DEFAULT_DIR);
    let manifest_path = pack_dir.join("manifest.yaml");
    let mut issues = Vec::new();
    let mut checks = BTreeMap::new();
    checks.insert("auth_required", json!(false));
    checks.insert("offline_mode", json!(true));
    checks.insert("pack_dir_exists", json!(pack_dir.exists()));
    checks.insert("manifest_exists", json!(manifest_path.exists()));
    if !pack_dir.exists() {
        issues.push(issue(
            "pack_dir_missing",
            "error",
            DEFAULT_DIR,
            format!("missing {}", pack_dir.display()),
        ));
    }
    if !manifest_path.exists() {
        issues.push(issue(
            "manifest_missing",
            "error",
            ".mdp/manifest.yaml",
            format!("missing {}", manifest_path.display()),
        ));
    }
    if manifest_path.exists() {
        match read_manifest(root) {
            Ok(manifest) => {
                checks.insert("format", json!(manifest.format));
                checks.insert("manifest_parseable", json!(true));
            }
            Err(err) => {
                checks.insert("manifest_parseable", json!(false));
                issues.push(issue(
                    "manifest_parse_failed",
                    "error",
                    ".mdp/manifest.yaml",
                    err.to_string(),
                ));
            }
        }
    }
    json!({
        "tool": "mdp",
        "format_name": FORMAT_NAME,
        "expected_format": FORMAT_VERSION,
        "valid": issues.is_empty(),
        "checks": checks,
        "issues": issues,
        "setup": if issues.is_empty() { Value::Null } else { json!("Run `mdp init --name <name>` from the repo or workspace root.") }
    })
}

pub(crate) fn validate_pack(root: &Path) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let mut issues = Vec::new();
    validate_manifest_shape(root, &mut issues);
    let mut card_ids = BTreeSet::new();
    let mut loaded_cards = Vec::new();
    if manifest.format != FORMAT_VERSION {
        issues.push(issue(
            "manifest_format",
            "error",
            ".mdp/manifest.yaml#/format",
            format!(
                "manifest format must be {FORMAT_VERSION}, found {}",
                manifest.format
            ),
        ));
    }
    if manifest.personas.is_empty() {
        issues.push(issue(
            "manifest_personas_empty",
            "error",
            ".mdp/manifest.yaml#/personas",
            "manifest personas must not be empty",
        ));
    }
    if manifest.cards.is_empty() {
        issues.push(issue(
            "manifest_cards_empty",
            "error",
            ".mdp/manifest.yaml#/cards",
            "manifest cards must not be empty",
        ));
    }
    if !manifest.policy.progressive_disclosure {
        issues.push(issue(
            "policy_progressive_disclosure",
            "warning",
            ".mdp/manifest.yaml#/policy/progressive_disclosure",
            "policy.progressive_disclosure should be true",
        ));
    }
    let persona_names: BTreeSet<String> = manifest
        .personas
        .iter()
        .map(|persona| persona.to_lowercase())
        .collect();
    for (index, mapping) in manifest.persona_mappings.iter().enumerate() {
        if mapping.persona.trim().is_empty() {
            issues.push(issue(
                "persona_mapping_persona_empty",
                "error",
                format!(".mdp/manifest.yaml#/persona_mappings/{index}/persona"),
                "persona_mappings entries must name a persona",
            ));
        } else if !persona_names.contains(&mapping.persona.to_lowercase()) {
            issues.push(issue(
                "persona_mapping_unknown_persona",
                "warning",
                format!(".mdp/manifest.yaml#/persona_mappings/{index}/persona"),
                format!(
                    "persona mapping references {}, which is not listed in manifest personas",
                    mapping.persona
                ),
            ));
        }
        if mapping.title_keywords.is_empty() {
            issues.push(issue(
                "persona_mapping_keywords_empty",
                "warning",
                format!(".mdp/manifest.yaml#/persona_mappings/{index}/title_keywords"),
                "persona mapping has no title keywords and cannot infer from prospect titles",
            ));
        }
        for (keyword_index, keyword) in mapping.title_keywords.iter().enumerate() {
            if keyword.trim().is_empty() {
                issues.push(issue(
                    "persona_mapping_keyword_empty",
                    "warning",
                    format!(
                        ".mdp/manifest.yaml#/persona_mappings/{index}/title_keywords/{keyword_index}"
                    ),
                    "persona mapping title keywords should not be empty",
                ));
            }
        }
    }
    validate_lead_input_requirements(&manifest, &mut issues);
    validate_profile(manifest.profile.as_ref(), &mut issues);
    for card_ref in &manifest.cards {
        if !card_ids.insert(card_ref.id.clone()) {
            issues.push(issue(
                "duplicate_card_id",
                "error",
                ".mdp/manifest.yaml#/cards",
                format!("duplicate card id {}", card_ref.id),
            ));
        }
        let path = match resolve_pack_path(root, &card_ref.path) {
            Ok(path) => path,
            Err(err) => {
                issues.push(issue(
                    "invalid_card_path",
                    "error",
                    format!(".mdp/manifest.yaml#/cards/{}", card_ref.id),
                    err.to_string(),
                ));
                continue;
            }
        };
        let display_path = display_pack_path(&card_ref.path);
        match read_card(&path) {
            Ok(card) => {
                validate_card_shape(&path, &display_path, &mut issues);
                if card.id != card_ref.id {
                    issues.push(issue(
                        "card_id_mismatch",
                        "error",
                        &display_path,
                        format!("manifest has {}, card has {}", card_ref.id, card.id),
                    ));
                }
                if card.kind != card_ref.kind {
                    issues.push(issue(
                        "card_kind_mismatch",
                        "error",
                        &display_path,
                        "card kind does not match manifest",
                    ));
                }
                if card.entries.is_empty() {
                    issues.push(issue(
                        "card_entries_empty",
                        "error",
                        &display_path,
                        "card has no entries",
                    ));
                }
                loaded_cards.push(json!({"id": card.id, "kind": card_ref.kind, "path": display_path, "entries": card.entries.len()}));
            }
            Err(err) => issues.push(issue(
                "card_read_failed",
                "error",
                display_path,
                err.to_string(),
            )),
        }
    }
    let loaded_prompts = validate_prompts(root, &mut issues)?;
    let prompt_inventory = prompt_inventory(&loaded_prompts);
    let eval_inventory = collect_eval_inventory(root, &mut issues)?;
    let profile = validate_profile_mapping(
        &manifest,
        &card_ids,
        &prompt_inventory,
        &eval_inventory,
        &mut issues,
    );
    let error_count = issue_count(&issues, "error");
    let warning_count = issue_count(&issues, "warning");
    Ok(json!({
        "valid": error_count == 0,
        "error_count": error_count,
        "warning_count": warning_count,
        "manifest": format!("{DEFAULT_DIR}/manifest.yaml"),
        "cards": loaded_cards,
        "prompts": loaded_prompts,
        "profile": profile,
        "issues": issues
    }))
}

fn validate_manifest_shape(root: &Path, issues: &mut Vec<Value>) {
    let path = root.join(DEFAULT_DIR).join("manifest.yaml");
    let Ok(raw) = fs::read_to_string(&path) else {
        return;
    };
    let Ok(value) = serde_yaml::from_str::<YamlValue>(&raw) else {
        return;
    };

    validate_object_keys(
        &value,
        &[
            "format",
            "id",
            "name",
            "version",
            "description",
            "profile",
            "personas",
            "target_personas",
            "operator_roles",
            "supported_channels",
            "persona_mappings",
            "lead_input_requirements",
            "required_primitives",
            "primitive_map",
            "input_contracts",
            "jobs",
            "profile_eval",
            "cards",
            "policy",
            "provenance",
        ],
        ".mdp/manifest.yaml",
        "manifest_unknown_field",
        issues,
    );
    let profile = yaml_get(&value, "profile").unwrap_or(&YamlValue::Null);
    validate_object_keys(
        profile,
        &["id", "label", "version", "agent_surface"],
        ".mdp/manifest.yaml#/profile",
        "manifest_profile_unknown_field",
        issues,
    );
    let agent_surface = yaml_get(profile, "agent_surface").unwrap_or(&YamlValue::Null);
    validate_object_keys(
        agent_surface,
        &[
            "recommended_skills",
            "allowed_skills",
            "blocked_skills",
            "job_skills",
        ],
        ".mdp/manifest.yaml#/profile/agent_surface",
        "manifest_agent_surface_unknown_field",
        issues,
    );
    validate_sequence_object_keys(
        yaml_get(agent_surface, "blocked_skills"),
        &["name", "reason"],
        ".mdp/manifest.yaml#/profile/agent_surface/blocked_skills",
        "manifest_blocked_skill_unknown_field",
        issues,
    );
    validate_sequence_object_keys(
        yaml_get(agent_surface, "job_skills"),
        &["job", "skills"],
        ".mdp/manifest.yaml#/profile/agent_surface/job_skills",
        "manifest_job_skill_unknown_field",
        issues,
    );
    validate_primitive_map_shape(
        yaml_get(&value, "primitive_map"),
        ".mdp/manifest.yaml#/primitive_map",
        issues,
    );
    validate_sequence_object_keys(
        yaml_get(&value, "input_contracts"),
        &["id", "description", "schema_ref", "prompt", "normalizes"],
        ".mdp/manifest.yaml#/input_contracts",
        "manifest_input_contract_unknown_field",
        issues,
    );
    validate_sequence_object_keys(
        yaml_get(&value, "jobs"),
        &[
            "id",
            "label",
            "description",
            "required_primitives",
            "input_contracts",
        ],
        ".mdp/manifest.yaml#/jobs",
        "manifest_profile_job_unknown_field",
        issues,
    );
    validate_object_keys(
        yaml_get(&value, "profile_eval").unwrap_or(&YamlValue::Null),
        &["required_categories", "activation"],
        ".mdp/manifest.yaml#/profile_eval",
        "manifest_profile_eval_unknown_field",
        issues,
    );
    validate_object_keys(
        yaml_get(
            yaml_get(&value, "profile_eval").unwrap_or(&YamlValue::Null),
            "activation",
        )
        .unwrap_or(&YamlValue::Null),
        &["status", "summary"],
        ".mdp/manifest.yaml#/profile_eval/activation",
        "manifest_profile_eval_activation_unknown_field",
        issues,
    );
    validate_sequence_object_keys(
        yaml_get(&value, "cards"),
        &["id", "path", "kind", "description", "personas", "tags"],
        ".mdp/manifest.yaml#/cards",
        "manifest_card_ref_unknown_field",
        issues,
    );
    validate_sequence_object_keys(
        yaml_get(&value, "persona_mappings"),
        &["persona", "title_keywords"],
        ".mdp/manifest.yaml#/persona_mappings",
        "manifest_persona_mapping_unknown_field",
        issues,
    );
    validate_object_keys(
        yaml_get(&value, "lead_input_requirements").unwrap_or(&YamlValue::Null),
        &[
            "required_fields",
            "required_signal_fields",
            "required_attributes",
            "value_contracts",
            "attribute_definitions",
            "allow_undeclared_attributes",
        ],
        ".mdp/manifest.yaml#/lead_input_requirements",
        "manifest_lead_input_requirements_unknown_field",
        issues,
    );
    validate_value_contract_shapes(
        yaml_get(
            yaml_get(&value, "lead_input_requirements").unwrap_or(&YamlValue::Null),
            "value_contracts",
        ),
        ".mdp/manifest.yaml#/lead_input_requirements/value_contracts",
        issues,
    );
    validate_value_contract_shapes(
        yaml_get(
            yaml_get(&value, "lead_input_requirements").unwrap_or(&YamlValue::Null),
            "attribute_definitions",
        ),
        ".mdp/manifest.yaml#/lead_input_requirements/attribute_definitions",
        issues,
    );
    validate_object_keys(
        yaml_get(&value, "policy").unwrap_or(&YamlValue::Null),
        &[
            "progressive_disclosure",
            "load_manifest_first",
            "max_cards_per_route",
            "json_contract",
            "no_auth_required",
        ],
        ".mdp/manifest.yaml#/policy",
        "manifest_policy_unknown_field",
        issues,
    );
    validate_object_keys(
        yaml_get(&value, "provenance").unwrap_or(&YamlValue::Null),
        &["owner", "created_by", "notes"],
        ".mdp/manifest.yaml#/provenance",
        "manifest_provenance_unknown_field",
        issues,
    );
}

fn validate_profile(profile: Option<&Profile>, issues: &mut Vec<Value>) {
    let Some(profile) = profile else {
        return;
    };

    if profile.id.trim().is_empty() {
        issues.push(issue(
            "profile_id_empty",
            "error",
            ".mdp/manifest.yaml#/profile/id",
            "profile.id must not be empty when profile metadata is present",
        ));
    }
    if profile
        .version
        .as_deref()
        .is_some_and(|version| version != "mdp.profile.v0")
    {
        issues.push(issue(
            "profile_version_unknown",
            "warning",
            ".mdp/manifest.yaml#/profile/version",
            "profile.version should be mdp.profile.v0 for the current profile contract",
        ));
    }
    validate_agent_surface(&profile.agent_surface, issues);
}

fn validate_agent_surface(surface: &AgentSurface, issues: &mut Vec<Value>) {
    if surface.is_empty() {
        issues.push(issue(
            "profile_agent_surface_empty",
            "warning",
            ".mdp/manifest.yaml#/profile/agent_surface",
            "profile.agent_surface is empty; agents will fall back to generic MDP skill selection",
        ));
        return;
    }

    let recommended = validate_skill_list(
        &surface.recommended_skills,
        ".mdp/manifest.yaml#/profile/agent_surface/recommended_skills",
        "profile_recommended_skill",
        issues,
    );
    let allowed = validate_skill_list(
        &surface.allowed_skills,
        ".mdp/manifest.yaml#/profile/agent_surface/allowed_skills",
        "profile_allowed_skill",
        issues,
    );

    for (index, skill) in surface.blocked_skills.iter().enumerate() {
        let path = format!(".mdp/manifest.yaml#/profile/agent_surface/blocked_skills/{index}");
        if skill.name.trim().is_empty() {
            issues.push(issue(
                "profile_blocked_skill_name_empty",
                "error",
                format!("{path}/name"),
                "blocked_skills entries must name a skill",
            ));
            continue;
        }
        if skill.reason.trim().is_empty() {
            issues.push(issue(
                "profile_blocked_skill_reason_empty",
                "error",
                format!("{path}/reason"),
                "blocked_skills entries must explain the reroute reason",
            ));
        }
        let key = skill.name.to_lowercase();
        if recommended.contains(&key) || allowed.contains(&key) {
            issues.push(issue(
                "profile_blocked_skill_overlap",
                "warning",
                format!("{path}/name"),
                format!(
                    "blocked skill {} also appears in recommended_skills or allowed_skills",
                    skill.name
                ),
            ));
        }
    }

    for skill in &surface.recommended_skills {
        if !allowed.is_empty() && !allowed.contains(&skill.to_lowercase()) {
            issues.push(issue(
                "profile_recommended_skill_not_allowed",
                "warning",
                ".mdp/manifest.yaml#/profile/agent_surface/recommended_skills",
                format!(
                    "recommended skill {skill} is not listed in allowed_skills; add it or leave allowed_skills empty"
                ),
            ));
        }
    }

    for (index, route) in surface.job_skills.iter().enumerate() {
        let path = format!(".mdp/manifest.yaml#/profile/agent_surface/job_skills/{index}");
        if route.job.trim().is_empty() {
            issues.push(issue(
                "profile_job_skill_job_empty",
                "error",
                format!("{path}/job"),
                "job_skills entries must name a job",
            ));
        }
        if route.skills.is_empty() {
            issues.push(issue(
                "profile_job_skill_skills_empty",
                "warning",
                format!("{path}/skills"),
                "job_skills entries should include at least one skill",
            ));
        }
        for (skill_index, skill) in route.skills.iter().enumerate() {
            if skill.trim().is_empty() {
                issues.push(issue(
                    "profile_job_skill_skill_empty",
                    "error",
                    format!("{path}/skills/{skill_index}"),
                    "job skill names must not be empty",
                ));
            } else if !allowed.is_empty() && !allowed.contains(&skill.to_lowercase()) {
                issues.push(issue(
                    "profile_job_skill_not_allowed",
                    "warning",
                    format!("{path}/skills/{skill_index}"),
                    format!(
                        "job skill {skill} is not listed in allowed_skills; add it or leave allowed_skills empty"
                    ),
                ));
            }
        }
    }
}

fn validate_skill_list(
    values: &[String],
    path: &str,
    code_prefix: &str,
    issues: &mut Vec<Value>,
) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if value.trim().is_empty() {
            issues.push(issue(
                &format!("{code_prefix}_empty"),
                "error",
                format!("{path}/{index}"),
                "skill names must not be empty",
            ));
        } else if !seen.insert(value.to_lowercase()) {
            issues.push(issue(
                &format!("{code_prefix}_duplicate"),
                "warning",
                format!("{path}/{index}"),
                format!("duplicate skill {value}"),
            ));
        }
    }
    seen
}

fn validate_profile_mapping(
    manifest: &Manifest,
    card_ids: &BTreeSet<String>,
    prompt_inventory: &PromptInventory,
    eval_inventory: &EvalInventory,
    issues: &mut Vec<Value>,
) -> Value {
    let activation_contract_present = !manifest.required_primitives.is_empty()
        || !manifest.primitive_map.is_empty()
        || !manifest.input_contracts.is_empty()
        || !manifest.jobs.is_empty()
        || !manifest.profile_eval.is_empty();
    let profile_present = manifest.profile.is_some() || activation_contract_present;
    if !profile_present {
        return json!({
            "present": false,
            "activation_ready": Value::Null
        });
    }
    if !activation_contract_present {
        return json!({
            "present": true,
            "id": manifest.profile.as_ref().map(|profile| profile.id.as_str()),
            "activation_ready": false,
            "required_primitives": [],
            "covered_primitives": [],
            "missing_required_primitives": [],
            "eval_categories": {},
            "missing_eval_categories": [],
            "jobs": [],
            "activation_policy": "profile.id and profile.agent_surface route agents, but full activation requires required_primitives, primitive_map, input_contracts, jobs, and profile_eval coverage."
        });
    }

    let starting_issue_count = issues.len();
    let known_primitives = KNOWN_PRIMITIVES.iter().copied().collect::<BTreeSet<_>>();
    let required_primitives = validate_primitive_list(
        &manifest.required_primitives,
        ".mdp/manifest.yaml#/required_primitives",
        "profile_required_primitive",
        &known_primitives,
        issues,
    );
    let input_contract_ids = validate_input_contracts(
        &manifest.input_contracts,
        prompt_inventory,
        ".mdp/manifest.yaml#/input_contracts",
        issues,
    );
    let job_ids = validate_profile_jobs(
        &manifest.jobs,
        &known_primitives,
        &input_contract_ids,
        ".mdp/manifest.yaml#/jobs",
        issues,
    );
    let missing_activation_sections = validate_activation_sections(manifest, issues);
    validate_eval_profile_refs(eval_inventory, &known_primitives, &job_ids, issues);

    let mut covered_primitives = BTreeSet::new();
    for (primitive, mapping) in &manifest.primitive_map {
        if !known_primitives.contains(primitive.as_str()) {
            issues.push(issue(
                "profile_primitive_unknown",
                "error",
                format!(".mdp/manifest.yaml#/primitive_map/{primitive}"),
                format!(
                    "unknown primitive id {primitive}; expected one of {}",
                    KNOWN_PRIMITIVES.join(", ")
                ),
            ));
            continue;
        }
        if !mapping.is_empty() {
            covered_primitives.insert(primitive.clone());
        }
        validate_primitive_mapping_refs(
            primitive,
            mapping,
            card_ids,
            prompt_inventory,
            &input_contract_ids,
            &job_ids,
            eval_inventory,
            issues,
        );
    }

    let mut missing_required_primitives = Vec::new();
    for primitive in &required_primitives {
        if !covered_primitives.contains(primitive) {
            missing_required_primitives.push(primitive.clone());
            issues.push(issue_with_gate(
                "profile_required_primitive_unmapped",
                "warning",
                format!(".mdp/manifest.yaml#/required_primitives/{primitive}"),
                format!("required primitive {primitive} has no mapped cards, prompts, input contracts, jobs, or evals"),
                "fails",
                "blocks",
            ));
        }
    }

    let mut job_summaries = Vec::new();
    for job in &manifest.jobs {
        let mut missing_job_primitives = Vec::new();
        for primitive in &job.required_primitives {
            if known_primitives.contains(primitive.as_str())
                && !covered_primitives.contains(primitive)
            {
                missing_job_primitives.push(primitive.clone());
                issues.push(issue_with_gate(
                    "profile_job_required_primitive_unmapped",
                    "warning",
                    format!(".mdp/manifest.yaml#/jobs/{}/required_primitives/{primitive}", job.id),
                    format!("job {} requires primitive {primitive}, but that primitive has no mapped coverage", job.id),
                    "fails",
                    "blocks-job",
                ));
            }
        }
        job_summaries.push(json!({
            "id": &job.id,
            "label": &job.label,
            "required_primitives": &job.required_primitives,
            "missing_required_primitives": missing_job_primitives,
            "activation_ready": missing_job_primitives.is_empty()
        }));
    }

    let (eval_categories, missing_eval_categories) =
        validate_profile_eval(&manifest.profile_eval, eval_inventory, issues);
    let profile_error_count = issues[starting_issue_count..]
        .iter()
        .filter(|issue| issue["severity"].as_str() == Some("error"))
        .count();
    let activation_ready = profile_error_count == 0
        && missing_activation_sections.is_empty()
        && missing_required_primitives.is_empty()
        && missing_eval_categories.is_empty()
        && job_summaries
            .iter()
            .all(|job| job["activation_ready"].as_bool() == Some(true));

    json!({
        "present": true,
        "id": manifest.profile.as_ref().map(|profile| profile.id.as_str()),
        "activation_ready": activation_ready,
        "required_primitives": &manifest.required_primitives,
        "covered_primitives": covered_primitives.into_iter().collect::<Vec<_>>(),
        "missing_activation_sections": missing_activation_sections,
        "missing_required_primitives": missing_required_primitives,
        "eval_categories": eval_categories,
        "missing_eval_categories": missing_eval_categories,
        "jobs": job_summaries,
        "activation_policy": "Errors fail validation. Missing required primitive coverage and missing profile eval categories are warning-first by default, fail under --strict, and block profile activation."
    })
}

fn validate_activation_sections(manifest: &Manifest, issues: &mut Vec<Value>) -> Vec<String> {
    let mut missing = Vec::new();
    if manifest.profile.is_none() {
        missing.push("profile".to_string());
        issues.push(issue_with_gate(
            "profile_activation_section_missing",
            "warning",
            ".mdp/manifest.yaml#/profile",
            "profile activation requires profile metadata",
            "fails",
            "blocks",
        ));
    }
    for (section, path, message, missing_when_empty) in [
        (
            "required_primitives",
            ".mdp/manifest.yaml#/required_primitives",
            "profile activation requires required_primitives",
            manifest.required_primitives.is_empty(),
        ),
        (
            "primitive_map",
            ".mdp/manifest.yaml#/primitive_map",
            "profile activation requires primitive_map",
            manifest.primitive_map.is_empty(),
        ),
        (
            "input_contracts",
            ".mdp/manifest.yaml#/input_contracts",
            "profile activation requires input_contracts",
            manifest.input_contracts.is_empty(),
        ),
        (
            "jobs",
            ".mdp/manifest.yaml#/jobs",
            "profile activation requires jobs",
            manifest.jobs.is_empty(),
        ),
        (
            "profile_eval.required_categories",
            ".mdp/manifest.yaml#/profile_eval/required_categories",
            "profile activation requires profile_eval.required_categories",
            manifest.profile_eval.required_categories.is_empty(),
        ),
    ] {
        if missing_when_empty {
            missing.push(section.to_string());
            issues.push(issue_with_gate(
                "profile_activation_section_missing",
                "warning",
                path,
                message,
                "fails",
                "blocks",
            ));
        }
    }
    missing
}

fn validate_primitive_list(
    values: &[String],
    path: &str,
    code_prefix: &str,
    known_primitives: &BTreeSet<&str>,
    issues: &mut Vec<Value>,
) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    for (index, primitive) in values.iter().enumerate() {
        if !known_primitives.contains(primitive.as_str()) {
            issues.push(issue(
                &format!("{code_prefix}_unknown"),
                "error",
                format!("{path}/{index}"),
                format!(
                    "unknown primitive id {primitive}; expected one of {}",
                    KNOWN_PRIMITIVES.join(", ")
                ),
            ));
        } else if !seen.insert(primitive.clone()) {
            issues.push(issue(
                &format!("{code_prefix}_duplicate"),
                "warning",
                format!("{path}/{index}"),
                format!("duplicate primitive {primitive}"),
            ));
        }
    }
    seen
}

fn validate_input_contracts(
    input_contracts: &[InputContract],
    prompt_inventory: &PromptInventory,
    path: &str,
    issues: &mut Vec<Value>,
) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    for (index, contract) in input_contracts.iter().enumerate() {
        let contract_path = format!("{path}/{index}");
        if contract.id.trim().is_empty() {
            issues.push(issue(
                "profile_input_contract_id_empty",
                "error",
                format!("{contract_path}/id"),
                "input_contracts entries must name an id",
            ));
        } else if !seen.insert(contract.id.clone()) {
            issues.push(issue(
                "profile_input_contract_duplicate",
                "warning",
                format!("{contract_path}/id"),
                format!("duplicate input contract {}", contract.id),
            ));
        }
        if contract
            .schema_ref
            .as_deref()
            .is_some_and(|schema_ref| schema_ref.trim().is_empty())
        {
            issues.push(issue(
                "profile_input_contract_schema_ref_empty",
                "error",
                format!("{contract_path}/schema_ref"),
                "input contract schema_ref must not be empty when present",
            ));
        }
        if let Some(prompt) = contract.prompt.as_deref() {
            if prompt.trim().is_empty() {
                issues.push(issue(
                    "profile_input_contract_prompt_empty",
                    "error",
                    format!("{contract_path}/prompt"),
                    "input contract prompt must not be empty when present",
                ));
            } else if !prompt_inventory.contains(prompt) {
                issues.push(issue(
                    "profile_input_contract_prompt_missing",
                    "error",
                    format!("{contract_path}/prompt"),
                    format!(
                        "input contract {} references missing prompt {prompt}",
                        contract.id
                    ),
                ));
            }
        }
        validate_non_empty_unique_strings(
            &contract.normalizes,
            &format!("{contract_path}/normalizes"),
            "profile_input_contract_normalizes",
            issues,
        );
    }
    seen
}

fn validate_profile_jobs(
    jobs: &[ProfileJob],
    known_primitives: &BTreeSet<&str>,
    input_contract_ids: &BTreeSet<String>,
    path: &str,
    issues: &mut Vec<Value>,
) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    for (index, job) in jobs.iter().enumerate() {
        let job_path = format!("{path}/{index}");
        if job.id.trim().is_empty() {
            issues.push(issue(
                "profile_job_id_empty",
                "error",
                format!("{job_path}/id"),
                "jobs entries must name an id",
            ));
        } else if !seen.insert(job.id.clone()) {
            issues.push(issue(
                "profile_job_duplicate",
                "warning",
                format!("{job_path}/id"),
                format!("duplicate profile job {}", job.id),
            ));
        }
        validate_primitive_list(
            &job.required_primitives,
            &format!("{job_path}/required_primitives"),
            "profile_job_required_primitive",
            known_primitives,
            issues,
        );
        validate_reference_list(
            &job.input_contracts,
            input_contract_ids,
            &format!("{job_path}/input_contracts"),
            "profile_job_input_contract_missing",
            "input contract",
            issues,
        );
    }
    seen
}

fn validate_primitive_mapping_refs(
    primitive: &str,
    mapping: &PrimitiveMapping,
    card_ids: &BTreeSet<String>,
    prompt_inventory: &PromptInventory,
    input_contract_ids: &BTreeSet<String>,
    job_ids: &BTreeSet<String>,
    eval_inventory: &EvalInventory,
    issues: &mut Vec<Value>,
) {
    let path = format!(".mdp/manifest.yaml#/primitive_map/{primitive}");
    validate_reference_list(
        &mapping.cards,
        card_ids,
        &format!("{path}/cards"),
        "profile_primitive_card_missing",
        "card",
        issues,
    );
    validate_prompt_reference_list(
        &mapping.prompts,
        prompt_inventory,
        &format!("{path}/prompts"),
        issues,
    );
    validate_reference_list(
        &mapping.input_contracts,
        input_contract_ids,
        &format!("{path}/input_contracts"),
        "profile_primitive_input_contract_missing",
        "input contract",
        issues,
    );
    validate_reference_list(
        &mapping.jobs,
        job_ids,
        &format!("{path}/jobs"),
        "profile_primitive_job_missing",
        "job",
        issues,
    );
    validate_eval_reference_list(
        &mapping.evals,
        eval_inventory,
        &format!("{path}/evals"),
        issues,
    );
}

fn validate_reference_list(
    values: &[String],
    allowed: &BTreeSet<String>,
    path: &str,
    missing_code: &str,
    label: &str,
    issues: &mut Vec<Value>,
) {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if value.trim().is_empty() {
            issues.push(issue(
                &format!("{missing_code}_empty"),
                "error",
                format!("{path}/{index}"),
                format!("{label} references must not be empty"),
            ));
        } else if !allowed.contains(value) {
            issues.push(issue(
                missing_code,
                "error",
                format!("{path}/{index}"),
                format!("mapped {label} {value} does not exist"),
            ));
        } else if !seen.insert(value) {
            issues.push(issue(
                &format!("{missing_code}_duplicate"),
                "warning",
                format!("{path}/{index}"),
                format!("duplicate mapped {label} {value}"),
            ));
        }
    }
}

fn validate_prompt_reference_list(
    values: &[String],
    prompt_inventory: &PromptInventory,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if value.trim().is_empty() {
            issues.push(issue(
                "profile_primitive_prompt_missing_empty",
                "error",
                format!("{path}/{index}"),
                "prompt references must not be empty",
            ));
        } else if !prompt_inventory.contains(value) {
            issues.push(issue(
                "profile_primitive_prompt_missing",
                "error",
                format!("{path}/{index}"),
                format!("mapped prompt {value} does not exist"),
            ));
        } else if !seen.insert(value) {
            issues.push(issue(
                "profile_primitive_prompt_missing_duplicate",
                "warning",
                format!("{path}/{index}"),
                format!("duplicate mapped prompt {value}"),
            ));
        }
    }
}

fn validate_eval_reference_list(
    values: &[String],
    eval_inventory: &EvalInventory,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if value.trim().is_empty() {
            issues.push(issue(
                "profile_primitive_eval_missing_empty",
                "error",
                format!("{path}/{index}"),
                "eval references must not be empty",
            ));
        } else if !eval_inventory.contains(value) {
            issues.push(issue(
                "profile_primitive_eval_missing",
                "error",
                format!("{path}/{index}"),
                format!("mapped eval {value} does not exist"),
            ));
        } else if !seen.insert(value) {
            issues.push(issue(
                "profile_primitive_eval_missing_duplicate",
                "warning",
                format!("{path}/{index}"),
                format!("duplicate mapped eval {value}"),
            ));
        }
    }
}

fn validate_profile_eval(
    profile_eval: &ProfileEval,
    eval_inventory: &EvalInventory,
    issues: &mut Vec<Value>,
) -> (Value, Vec<String>) {
    let known_categories = KNOWN_PROFILE_EVAL_CATEGORIES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut missing = Vec::new();
    let mut categories = BTreeMap::new();
    for (index, category) in profile_eval.required_categories.iter().enumerate() {
        if !known_categories.contains(category.as_str()) {
            issues.push(issue(
                "profile_eval_category_unknown",
                "error",
                format!(".mdp/manifest.yaml#/profile_eval/required_categories/{index}"),
                format!(
                    "unknown profile eval category {category}; expected one of {}",
                    KNOWN_PROFILE_EVAL_CATEGORIES.join(", ")
                ),
            ));
        } else if !seen.insert(category.clone()) {
            issues.push(issue(
                "profile_eval_category_duplicate",
                "warning",
                format!(".mdp/manifest.yaml#/profile_eval/required_categories/{index}"),
                format!("duplicate profile eval category {category}"),
            ));
        }

        if eval_inventory.categories.contains_key(category) {
            categories.insert(category.clone(), json!("present"));
        } else {
            categories.insert(category.clone(), json!("missing"));
            missing.push(category.clone());
            issues.push(issue_with_gate(
                "profile_eval_category_missing",
                "warning",
                format!(".mdp/manifest.yaml#/profile_eval/required_categories/{index}"),
                format!(
                    "profile eval category {category} has no matching .mdp/evals fixture metadata"
                ),
                "fails",
                "blocks",
            ));
        }
    }
    for category in eval_inventory.categories.keys() {
        categories
            .entry(category.clone())
            .or_insert_with(|| json!("present"));
    }
    if let Some(status) = profile_eval.activation.status.as_deref() {
        if !matches!(status, "ready" | "needs-review" | "blocked") {
            issues.push(issue(
                "profile_eval_activation_status_unknown",
                "warning",
                ".mdp/manifest.yaml#/profile_eval/activation/status",
                "profile_eval.activation.status should be ready, needs-review, or blocked",
            ));
        }
    }
    (json!(categories), missing)
}

fn validate_eval_profile_refs(
    eval_inventory: &EvalInventory,
    known_primitives: &BTreeSet<&str>,
    job_ids: &BTreeSet<String>,
    issues: &mut Vec<Value>,
) {
    for metadata in &eval_inventory.profile_metadata {
        validate_profile_eval_string_refs(
            &metadata.primitives,
            known_primitives,
            &format!("{}#/profile_eval/primitives", metadata.path),
            "eval_profile_primitive_unknown",
            "primitive",
            issues,
        );
        validate_reference_list(
            &metadata.jobs,
            job_ids,
            &format!("{}#/profile_eval/jobs", metadata.path),
            "eval_profile_job_missing",
            "profile job",
            issues,
        );
    }
}

fn validate_profile_eval_string_refs(
    values: &[String],
    known: &BTreeSet<&str>,
    path: &str,
    code: &str,
    label: &str,
    issues: &mut Vec<Value>,
) {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if value.trim().is_empty() {
            issues.push(issue(
                &format!("{code}_empty"),
                "error",
                format!("{path}/{index}"),
                format!("{label} references must not be empty"),
            ));
        } else if !known.contains(value.as_str()) {
            issues.push(issue(
                code,
                "error",
                format!("{path}/{index}"),
                format!(
                    "profile eval fixture references unknown {label} {value}; expected one of {}",
                    KNOWN_PRIMITIVES.join(", ")
                ),
            ));
        } else if !seen.insert(value) {
            issues.push(issue(
                &format!("{code}_duplicate"),
                "warning",
                format!("{path}/{index}"),
                format!("duplicate profile eval {label} {value}"),
            ));
        }
    }
}

fn validate_non_empty_unique_strings(
    values: &[String],
    path: &str,
    code_prefix: &str,
    issues: &mut Vec<Value>,
) {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if value.trim().is_empty() {
            issues.push(issue(
                &format!("{code_prefix}_empty"),
                "error",
                format!("{path}/{index}"),
                "values must not be empty",
            ));
        } else if !seen.insert(value) {
            issues.push(issue(
                &format!("{code_prefix}_duplicate"),
                "warning",
                format!("{path}/{index}"),
                format!("duplicate value {value}"),
            ));
        }
    }
}

fn validate_lead_input_requirements(manifest: &crate::models::Manifest, issues: &mut Vec<Value>) {
    let allowed_fields = [
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
    ];
    let allowed_signal_fields = [
        "id",
        "title",
        "source",
        "confidence",
        "freshness",
        "state_as",
    ];
    validate_requirement_values(
        &manifest.lead_input_requirements.required_fields,
        &allowed_fields,
        ".mdp/manifest.yaml#/lead_input_requirements/required_fields",
        "lead_input_required_field_unknown",
        "required_fields entries must be supported prospect fields",
        issues,
    );
    validate_requirement_values(
        &manifest.lead_input_requirements.required_signal_fields,
        &allowed_signal_fields,
        ".mdp/manifest.yaml#/lead_input_requirements/required_signal_fields",
        "lead_input_required_signal_field_unknown",
        "required_signal_fields entries must be supported signal fields",
        issues,
    );

    let mut seen_attributes = BTreeSet::new();
    for (index, attribute) in manifest
        .lead_input_requirements
        .required_attributes
        .iter()
        .enumerate()
    {
        if !valid_attribute_key(attribute) {
            issues.push(issue(
                "lead_input_required_attribute_invalid",
                "error",
                format!(
                    ".mdp/manifest.yaml#/lead_input_requirements/required_attributes/{index}"
                ),
                "required_attributes entries must start with a letter and contain only letters, numbers, underscores, or hyphens",
            ));
        } else if !seen_attributes.insert(attribute.to_lowercase()) {
            issues.push(issue(
                "lead_input_required_attribute_duplicate",
                "warning",
                format!(".mdp/manifest.yaml#/lead_input_requirements/required_attributes/{index}"),
                format!("duplicate required attribute {attribute}"),
            ));
        }
    }

    for (field, contract) in &manifest.lead_input_requirements.value_contracts {
        if !PROSPECT_CONTRACT_FIELDS.contains(&field.as_str()) {
            issues.push(issue(
                "lead_input_value_contract_field_unknown",
                "error",
                format!(".mdp/manifest.yaml#/lead_input_requirements/value_contracts/{field}"),
                format!("value_contracts key {field} must be a supported prospect scalar field"),
            ));
        }
        validate_value_contract(
            contract,
            &format!(".mdp/manifest.yaml#/lead_input_requirements/value_contracts/{field}"),
            issues,
        );
    }

    for (attribute, contract) in &manifest.lead_input_requirements.attribute_definitions {
        if !valid_attribute_key(attribute) {
            issues.push(issue(
                "lead_input_attribute_definition_key_invalid",
                "error",
                format!(
                    ".mdp/manifest.yaml#/lead_input_requirements/attribute_definitions/{attribute}"
                ),
                "attribute_definitions keys must start with a letter and contain only letters, numbers, underscores, or hyphens",
            ));
        }
        validate_value_contract(
            contract,
            &format!(
                ".mdp/manifest.yaml#/lead_input_requirements/attribute_definitions/{attribute}"
            ),
            issues,
        );
    }
}

fn validate_value_contract(contract: &ValueContract, path: &str, issues: &mut Vec<Value>) {
    if let Some(value_type) = contract.value_type.as_deref() {
        if !matches!(value_type, "string" | "number" | "integer" | "boolean") {
            issues.push(issue(
                "lead_input_value_contract_type_unknown",
                "error",
                format!("{path}/type"),
                format!("value contract type must be string, number, integer, or boolean; found {value_type}"),
            ));
        }
    }

    if let Some(format) = contract.format.as_deref() {
        if !matches!(format, "date" | "date-time") {
            issues.push(issue(
                "lead_input_value_contract_format_unknown",
                "error",
                format!("{path}/format"),
                format!("value contract format must be date or date-time; found {format}"),
            ));
        }
        if contract
            .value_type
            .as_deref()
            .is_some_and(|value_type| value_type != "string")
        {
            issues.push(issue(
                "lead_input_value_contract_format_type",
                "error",
                format!("{path}/format"),
                "date and date-time formats require type: string",
            ));
        }
    }

    if !contract.enum_values.is_empty() && contract.value_type.as_deref() != Some("string") {
        issues.push(issue(
            "lead_input_value_contract_enum_type",
            "error",
            format!("{path}/enum"),
            "enum contracts require type: string because runtime enum validation is string-only",
        ));
    }

    let mut seen = BTreeSet::new();
    for (index, value) in contract.enum_values.iter().enumerate() {
        if value.trim().is_empty() {
            issues.push(issue(
                "lead_input_value_contract_enum_empty",
                "error",
                format!("{path}/enum/{index}"),
                "enum values must not be empty",
            ));
        } else if !seen.insert(value) {
            issues.push(issue(
                "lead_input_value_contract_enum_duplicate",
                "warning",
                format!("{path}/enum/{index}"),
                format!("duplicate enum value {value}"),
            ));
        }
    }
}

fn validate_value_contract_shapes(value: Option<&YamlValue>, path: &str, issues: &mut Vec<Value>) {
    let Some(contracts) = value.and_then(YamlValue::as_mapping) else {
        return;
    };
    let allowed = ["type", "format", "enum", "required", "description"]
        .into_iter()
        .collect::<BTreeSet<_>>();
    for (contract_name, contract) in contracts {
        let Some(contract_name) = contract_name.as_str() else {
            continue;
        };
        let Some(contract) = contract.as_mapping() else {
            continue;
        };
        for key in contract.keys() {
            let Some(key) = key.as_str() else {
                continue;
            };
            if !allowed.contains(key) {
                issues.push(issue(
                    "lead_input_value_contract_unknown_field",
                    "error",
                    format!("{path}/{contract_name}/{key}"),
                    format!("unsupported value contract field {key}; expected type, format, enum, required, or description"),
                ));
            }
        }
    }
}

fn validate_requirement_values(
    values: &[String],
    allowed: &[&str],
    path: &str,
    code: &str,
    message: &str,
    issues: &mut Vec<Value>,
) {
    let allowed = allowed.iter().copied().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if !allowed.contains(value.as_str()) {
            issues.push(issue(
                code,
                "error",
                format!("{path}/{index}"),
                format!("{message}; found {value}"),
            ));
        } else if !seen.insert(value.as_str()) {
            let duplicate_code = format!("{code}_duplicate");
            issues.push(issue(
                &duplicate_code,
                "warning",
                format!("{path}/{index}"),
                format!("duplicate requirement {value}"),
            ));
        }
    }
}

fn valid_attribute_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && key.len() <= 64
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn validate_card_shape(path: &Path, display_path: &str, issues: &mut Vec<Value>) {
    let Ok(raw) = fs::read_to_string(path) else {
        return;
    };
    let Ok(value) = serde_yaml::from_str::<YamlValue>(&raw) else {
        return;
    };

    validate_object_keys(
        &value,
        &[
            "id",
            "kind",
            "title",
            "description",
            "personas",
            "tags",
            "entries",
        ],
        display_path,
        "card_unknown_field",
        issues,
    );

    let Some(entries) = yaml_get(&value, "entries").and_then(YamlValue::as_sequence) else {
        return;
    };
    for (index, entry) in entries.iter().enumerate() {
        let entry_path = format!("{display_path}#/entries/{index}");
        validate_object_keys(
            entry,
            &[
                "id",
                "title",
                "body",
                "applies_to",
                "evidence",
                "avoid",
                "exact_paragraphs",
                "constraints",
                "metadata",
            ],
            &entry_path,
            "card_entry_unknown_field",
            issues,
        );
        if let Some(metadata) = yaml_get(entry, "metadata") {
            if !metadata.is_mapping() {
                issues.push(issue(
                    "card_entry_metadata_type",
                    "error",
                    format!("{entry_path}/metadata"),
                    "entry metadata must be an object/map; metadata is surfaced for agents but not enforced by the CLI",
                ));
            }
        }
        validate_entry_constraints_shape(entry, &entry_path, issues);
    }
}

fn validate_entry_constraints_shape(entry: &YamlValue, entry_path: &str, issues: &mut Vec<Value>) {
    let Some(constraints) = yaml_get(entry, "constraints") else {
        return;
    };
    validate_object_keys(
        constraints,
        &[
            "word_count",
            "subject_words",
            "subject_avoid",
            "max_questions",
            "forbid_links",
            "forbid_attachments",
            "forbid_images",
            "forbid_html",
            "forbid_tracking",
            "proof_output",
        ],
        &format!("{entry_path}/constraints"),
        "unsupported_constraint_field",
        issues,
    );
    if let Some(proof_output) = yaml_get(constraints, "proof_output") {
        validate_object_keys(
            proof_output,
            &[
                "required_segment_kinds",
                "min_segments",
                "require_source_refs_for_claims",
                "max_connective_words",
            ],
            &format!("{entry_path}/constraints/proof_output"),
            "unsupported_constraint_field",
            issues,
        );
    }
}

fn yaml_get<'a>(value: &'a YamlValue, key: &str) -> Option<&'a YamlValue> {
    value.as_mapping()?.get(YamlValue::String(key.to_string()))
}

fn validate_sequence_object_keys(
    value: Option<&YamlValue>,
    allowed: &[&str],
    path: &str,
    code: &str,
    issues: &mut Vec<Value>,
) {
    let Some(items) = value.and_then(YamlValue::as_sequence) else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        validate_object_keys(item, allowed, &format!("{path}/{index}"), code, issues);
    }
}

fn validate_object_keys(
    value: &YamlValue,
    allowed: &[&str],
    path: &str,
    code: &str,
    issues: &mut Vec<Value>,
) {
    let Some(map) = value.as_mapping() else {
        return;
    };
    let allowed = allowed.iter().copied().collect::<BTreeSet<_>>();
    for key in map.keys() {
        let Some(key) = key.as_str() else {
            continue;
        };
        if !allowed.contains(key) {
            issues.push(issue(
                code,
                "warning",
                format!("{path}/{key}"),
                format!(
                    "unsupported field {key} is parsed but ignored; put advisory extension data under entry metadata"
                ),
            ));
        }
    }
}

fn validate_primitive_map_shape(value: Option<&YamlValue>, path: &str, issues: &mut Vec<Value>) {
    let Some(map) = value.and_then(YamlValue::as_mapping) else {
        return;
    };
    for (primitive, mapping) in map {
        let Some(primitive) = primitive.as_str() else {
            continue;
        };
        validate_object_keys(
            mapping,
            &["cards", "prompts", "input_contracts", "jobs", "evals"],
            &format!("{path}/{primitive}"),
            "manifest_primitive_map_unknown_field",
            issues,
        );
    }
}

#[derive(Debug, Default)]
struct PromptInventory {
    refs: BTreeSet<String>,
}

impl PromptInventory {
    fn contains(&self, value: &str) -> bool {
        self.refs.contains(value)
    }
}

fn prompt_inventory(loaded_prompts: &[Value]) -> PromptInventory {
    let mut inventory = PromptInventory::default();
    for prompt in loaded_prompts {
        if let Some(id) = prompt["id"].as_str() {
            inventory.refs.insert(id.to_string());
        }
        if let Some(path) = prompt["path"].as_str() {
            inventory.refs.insert(path.to_string());
            if let Some(stripped) = path.strip_prefix(".mdp/") {
                inventory.refs.insert(stripped.to_string());
            }
        }
    }
    inventory
}

#[derive(Debug, Default)]
struct EvalInventory {
    refs: BTreeSet<String>,
    categories: BTreeMap<String, Vec<String>>,
    profile_metadata: Vec<EvalProfileMetadata>,
}

impl EvalInventory {
    fn contains(&self, value: &str) -> bool {
        self.refs.contains(value)
    }
}

#[derive(Debug, Default)]
struct EvalProfileMetadata {
    path: String,
    primitives: Vec<String>,
    jobs: Vec<String>,
}

fn collect_eval_inventory(root: &Path, issues: &mut Vec<Value>) -> Result<EvalInventory> {
    let eval_dir = root.join(DEFAULT_DIR).join("evals");
    let mut inventory = EvalInventory::default();
    if !eval_dir.exists() {
        return Ok(inventory);
    }
    let mut paths = fs::read_dir(&eval_dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<invalid>");
        let display_path = format!("{DEFAULT_DIR}/evals/{filename}");
        let extension = path.extension().and_then(|extension| extension.to_str());
        if !matches!(extension, Some("yaml" | "yml")) {
            issues.push(issue(
                "eval_path_extension",
                "error",
                &display_path,
                "eval fixture files must use .yaml or .yml",
            ));
            continue;
        }
        inventory.refs.insert(display_path.clone());
        inventory
            .refs
            .insert(display_path.trim_start_matches(".mdp/").to_string());
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_yaml::from_str::<YamlValue>(&raw) else {
            issues.push(issue(
                "eval_fixture_parse_failed",
                "error",
                &display_path,
                "eval fixture could not be parsed while collecting profile metadata",
            ));
            continue;
        };
        if let Some(id) = yaml_get(&value, "id").and_then(YamlValue::as_str) {
            inventory.refs.insert(id.to_string());
        }
        let profile_eval = yaml_get(&value, "profile_eval").unwrap_or(&YamlValue::Null);
        if let Some(category) = yaml_get(profile_eval, "category").and_then(YamlValue::as_str) {
            if !KNOWN_PROFILE_EVAL_CATEGORIES.contains(&category) {
                issues.push(issue(
                    "eval_profile_category_unknown",
                    "error",
                    format!("{display_path}#/profile_eval/category"),
                    format!(
                        "unknown profile eval category {category}; expected one of {}",
                        KNOWN_PROFILE_EVAL_CATEGORIES.join(", ")
                    ),
                ));
            }
            inventory
                .categories
                .entry(category.to_string())
                .or_default()
                .push(display_path.clone());
        }
        if !matches!(profile_eval, YamlValue::Null) {
            inventory.profile_metadata.push(EvalProfileMetadata {
                path: display_path.clone(),
                primitives: yaml_string_sequence(
                    profile_eval,
                    "primitives",
                    &format!("{display_path}#/profile_eval/primitives"),
                    issues,
                ),
                jobs: yaml_string_sequence(
                    profile_eval,
                    "jobs",
                    &format!("{display_path}#/profile_eval/jobs"),
                    issues,
                ),
            });
        }
    }
    Ok(inventory)
}

fn yaml_string_sequence(
    value: &YamlValue,
    key: &str,
    path: &str,
    issues: &mut Vec<Value>,
) -> Vec<String> {
    let Some(raw) = yaml_get(value, key) else {
        return vec![];
    };
    let Some(items) = raw.as_sequence() else {
        issues.push(issue(
            "eval_profile_metadata_not_sequence",
            "error",
            path,
            "profile_eval metadata fields must be sequences of strings",
        ));
        return vec![];
    };
    let mut values = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if let Some(value) = item.as_str() {
            values.push(value.to_string());
        } else {
            issues.push(issue(
                "eval_profile_metadata_not_string",
                "error",
                format!("{path}/{index}"),
                "profile_eval metadata entries must be strings",
            ));
        }
    }
    values
}

fn validate_prompts(root: &Path, issues: &mut Vec<Value>) -> Result<Vec<Value>> {
    let prompts_dir = root.join(DEFAULT_DIR).join("prompts");
    if !prompts_dir.exists() {
        return Ok(vec![]);
    }

    let mut prompt_paths = fs::read_dir(&prompts_dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    prompt_paths.sort();

    let mut loaded_prompts = Vec::new();
    let mut prompt_ids = BTreeSet::new();
    for path in prompt_paths {
        let display_path = format!(
            "{DEFAULT_DIR}/prompts/{}",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("<invalid>")
        );
        let extension = path.extension().and_then(|extension| extension.to_str());
        if !matches!(extension, Some("yaml" | "yml")) {
            issues.push(issue(
                "prompt_path_extension",
                "error",
                &display_path,
                "prompt files must use .yaml or .yml",
            ));
            continue;
        }
        validate_prompt_shape(&path, &display_path, issues);
        match read_prompt(&path) {
            Ok(prompt) => {
                validate_prompt_file(&prompt, &display_path, &mut prompt_ids, issues);
                loaded_prompts.push(json!({
                    "id": prompt.id,
                    "path": display_path,
                    "target_card_kinds": prompt.target_card_kinds,
                    "inputs": prompt.inputs.len()
                }));
            }
            Err(err) => issues.push(issue(
                "prompt_read_failed",
                "error",
                &display_path,
                err.to_string(),
            )),
        }
    }

    Ok(loaded_prompts)
}

fn validate_prompt_shape(path: &Path, display_path: &str, issues: &mut Vec<Value>) {
    let Ok(raw) = fs::read_to_string(path) else {
        return;
    };
    let Ok(value) = serde_yaml::from_str::<YamlValue>(&raw) else {
        return;
    };

    validate_object_keys(
        &value,
        &[
            "format",
            "id",
            "title",
            "description",
            "target_card_kinds",
            "tags",
            "inputs",
            "instructions",
            "output_contract",
        ],
        display_path,
        "prompt_unknown_field",
        issues,
    );
    validate_sequence_object_keys(
        yaml_get(&value, "inputs"),
        &[
            "name",
            "description",
            "required",
            "default",
            "missing_behavior",
        ],
        &format!("{display_path}#/inputs"),
        "prompt_input_unknown_field",
        issues,
    );
    validate_object_keys(
        yaml_get(&value, "output_contract").unwrap_or(&YamlValue::Null),
        &[
            "contract",
            "output_kind",
            "strict_json_only",
            "required_top_level",
            "entry_defaults",
            "schema_ref",
            "schema",
            "example",
        ],
        &format!("{display_path}#/output_contract"),
        "prompt_output_contract_unknown_field",
        issues,
    );
    validate_object_keys(
        yaml_get(
            yaml_get(&value, "output_contract").unwrap_or(&YamlValue::Null),
            "entry_defaults",
        )
        .unwrap_or(&YamlValue::Null),
        &[
            "body",
            "applies_to",
            "evidence",
            "avoid",
            "confidence",
            "provenance",
        ],
        &format!("{display_path}#/output_contract/entry_defaults"),
        "prompt_entry_defaults_unknown_field",
        issues,
    );
}

fn validate_prompt_file(
    prompt: &PromptFile,
    path: &str,
    prompt_ids: &mut BTreeSet<String>,
    issues: &mut Vec<Value>,
) {
    if prompt.format != PROMPT_FORMAT_VERSION {
        issues.push(issue(
            "prompt_format",
            "error",
            format!("{path}#/format"),
            format!(
                "prompt format must be {PROMPT_FORMAT_VERSION}, found {}",
                prompt.format
            ),
        ));
    }
    if prompt.id.trim().is_empty() {
        issues.push(issue(
            "prompt_id_empty",
            "error",
            format!("{path}#/id"),
            "prompt id must not be empty",
        ));
    } else if !prompt_ids.insert(prompt.id.clone()) {
        issues.push(issue(
            "duplicate_prompt_id",
            "error",
            format!("{path}#/id"),
            format!("duplicate prompt id {}", prompt.id),
        ));
    }
    if prompt.target_card_kinds.is_empty() {
        issues.push(issue(
            "prompt_targets_empty",
            "error",
            format!("{path}#/target_card_kinds"),
            "prompt must name at least one target card kind",
        ));
    }
    if prompt.inputs.is_empty() {
        issues.push(issue(
            "prompt_inputs_empty",
            "error",
            format!("{path}#/inputs"),
            "prompt must declare input defaults and missing-data behavior",
        ));
    }
    for input in &prompt.inputs {
        if input.name.trim().is_empty()
            || input.default.trim().is_empty()
            || input.missing_behavior.trim().is_empty()
        {
            issues.push(issue(
                "prompt_input_contract",
                "error",
                format!("{path}#/inputs"),
                "each prompt input must include name, default, and missing_behavior",
            ));
        }
    }
    if prompt.instructions.is_empty()
        || prompt
            .instructions
            .iter()
            .any(|instruction| instruction.trim().is_empty())
    {
        issues.push(issue(
            "prompt_instructions_empty",
            "error",
            format!("{path}#/instructions"),
            "prompt instructions must not be empty",
        ));
    }

    validate_prompt_output_contract(prompt, path, issues);
}

fn validate_prompt_output_contract(prompt: &PromptFile, path: &str, issues: &mut Vec<Value>) {
    let contract = &prompt.output_contract;
    let output_kind = contract.output_kind.as_deref().unwrap_or("card-patches");
    if !matches!(output_kind, "card-patches" | "prospect-normalization") {
        issues.push(issue(
            "prompt_output_kind_unknown",
            "error",
            format!("{path}#/output_contract/output_kind"),
            format!("prompt output_kind must be card-patches or prospect-normalization, found {output_kind}"),
        ));
    }
    if contract.contract != PROMPT_OUTPUT_CONTRACT {
        issues.push(issue(
            "prompt_output_contract",
            "error",
            format!("{path}#/output_contract/contract"),
            format!(
                "prompt output contract must be {PROMPT_OUTPUT_CONTRACT}, found {}",
                contract.contract
            ),
        ));
    }
    if !contract.strict_json_only {
        issues.push(issue(
            "prompt_output_not_strict_json",
            "error",
            format!("{path}#/output_contract/strict_json_only"),
            "prompt outputs must be strict JSON only",
        ));
    }

    let required = [
        "contract",
        "prompt_id",
        "source_summary",
        "card_patches",
        "gaps",
        "rejected_claims",
    ];
    for field in required {
        if !contract
            .required_top_level
            .iter()
            .any(|required_field| required_field == field)
        {
            issues.push(issue(
                "prompt_output_required_field_missing",
                "error",
                format!("{path}#/output_contract/required_top_level"),
                format!("prompt output contract must require {field}"),
            ));
        }
    }
    if output_kind == "prospect-normalization" {
        for field in ["normalized_prospect", "normalization_trace"] {
            if !contract
                .required_top_level
                .iter()
                .any(|required_field| required_field == field)
            {
                issues.push(issue(
                    "prompt_normalization_required_field_missing",
                    "error",
                    format!("{path}#/output_contract/required_top_level"),
                    format!("prospect-normalization prompts must require {field}"),
                ));
            }
        }
    }
    if contract.entry_defaults.body != "N/A"
        || !contract.entry_defaults.applies_to.is_empty()
        || !contract.entry_defaults.evidence.is_empty()
        || !contract.entry_defaults.avoid.is_empty()
        || contract.entry_defaults.confidence.trim().is_empty()
        || !contract.entry_defaults.provenance.is_empty()
    {
        issues.push(issue(
            "prompt_entry_defaults_unsafe",
            "error",
            format!("{path}#/output_contract/entry_defaults"),
            "entry defaults must use body N/A, empty arrays, and an explicit confidence default",
        ));
    }

    validate_prompt_example(prompt, path, issues);
    validate_prompt_example_input_references(prompt, path, issues);
    validate_prompt_schema_ref(prompt, path, output_kind, issues);
    if let Some(schema) = prompt.output_contract.schema.as_ref() {
        validate_prompt_output_schema(prompt, schema, path, output_kind, issues);
    } else if prompt.output_contract.schema_ref.is_none() {
        issues.push(issue(
            "prompt_output_schema_missing",
            "error",
            format!("{path}#/output_contract"),
            "prompt output contract must include schema_ref or an explicit JSON Schema object",
        ));
    }
    if output_kind == "prospect-normalization" {
        validate_prompt_normalization_example(prompt, path, issues);
    }
}

fn validate_prompt_schema_ref(
    prompt: &PromptFile,
    path: &str,
    output_kind: &str,
    issues: &mut Vec<Value>,
) {
    let Some(schema_ref) = prompt.output_contract.schema_ref.as_deref() else {
        return;
    };
    let expected = if output_kind == "prospect-normalization" {
        PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF
    } else {
        PROMPT_CARD_PATCH_SCHEMA_REF
    };
    if schema_ref != expected {
        issues.push(issue(
            "prompt_output_schema_ref",
            "error",
            format!("{path}#/output_contract/schema_ref"),
            format!("prompt schema_ref must be {expected} for output_kind {output_kind}, found {schema_ref}"),
        ));
    }
}

fn validate_prompt_output_schema(
    prompt: &PromptFile,
    schema: &Value,
    path: &str,
    output_kind: &str,
    issues: &mut Vec<Value>,
) {
    if !schema.is_object() {
        issues.push(issue(
            "prompt_output_schema_missing",
            "error",
            format!("{path}#/output_contract/schema"),
            "prompt output contract must include an explicit JSON Schema object",
        ));
        return;
    }

    if schema["type"].as_str() != Some("object") {
        issues.push(issue(
            "prompt_output_schema_root_type",
            "error",
            format!("{path}#/output_contract/schema/type"),
            "prompt output schema root type must be object",
        ));
    }
    if schema["additionalProperties"].as_bool() != Some(false) {
        issues.push(issue(
            "prompt_output_schema_allows_extra_keys",
            "error",
            format!("{path}#/output_contract/schema/additionalProperties"),
            "prompt output schema must set additionalProperties: false at the root",
        ));
    }

    let Some(properties) = schema["properties"].as_object() else {
        issues.push(issue(
            "prompt_output_schema_properties",
            "error",
            format!("{path}#/output_contract/schema/properties"),
            "prompt output schema must define properties",
        ));
        return;
    };

    for field in &prompt.output_contract.required_top_level {
        if !schema_array_contains(&schema["required"], field) {
            issues.push(issue(
                "prompt_output_schema_required_field_missing",
                "error",
                format!("{path}#/output_contract/schema/required"),
                format!("prompt output schema must require {field}"),
            ));
        }
        if !properties.contains_key(field) {
            issues.push(issue(
                "prompt_output_schema_property_missing",
                "error",
                format!("{path}#/output_contract/schema/properties"),
                format!("prompt output schema must define property {field}"),
            ));
        }
    }

    if schema["properties"]["contract"]["const"].as_str() != Some(PROMPT_OUTPUT_CONTRACT) {
        issues.push(issue(
            "prompt_output_schema_contract_const",
            "error",
            format!("{path}#/output_contract/schema/properties/contract/const"),
            format!("prompt output schema contract const must be {PROMPT_OUTPUT_CONTRACT}"),
        ));
    }
    if schema["properties"]["prompt_id"]["const"].as_str() != Some(prompt.id.as_str()) {
        issues.push(issue(
            "prompt_output_schema_prompt_id_const",
            "error",
            format!("{path}#/output_contract/schema/properties/prompt_id/const"),
            "prompt output schema prompt_id const must match prompt id",
        ));
    }

    if output_kind == "prospect-normalization" {
        validate_prompt_normalization_output_schema(schema, path, issues);
    } else {
        validate_prompt_card_patch_output_schema(prompt, schema, path, issues);
    }
}

fn validate_prompt_normalization_output_schema(
    schema: &Value,
    path: &str,
    issues: &mut Vec<Value>,
) {
    if schema["properties"]["card_patches"]["maxItems"].as_u64() != Some(0) {
        issues.push(issue(
            "prompt_output_schema_normalization_card_patches",
            "error",
            format!("{path}#/output_contract/schema/properties/card_patches/maxItems"),
            "prospect-normalization schemas must force card_patches to an empty array",
        ));
    }
    for field in ["name", "title", "company"] {
        if !schema_array_contains(
            &schema["properties"]["normalized_prospect"]["required"],
            field,
        ) {
            issues.push(issue(
                "prompt_output_schema_prospect_required_field",
                "error",
                format!("{path}#/output_contract/schema/properties/normalized_prospect/required"),
                format!("normalized_prospect schema must require {field}"),
            ));
        }
    }
}

fn validate_prompt_card_patch_output_schema(
    prompt: &PromptFile,
    schema: &Value,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let target_kinds = prompt
        .target_card_kinds
        .iter()
        .map(card_kind_name)
        .collect::<BTreeSet<_>>();
    let kind_enum = &schema["properties"]["card_patches"]["items"]["properties"]["kind"]["enum"];
    for target_kind in target_kinds {
        if !schema_array_contains(kind_enum, target_kind) {
            issues.push(issue(
                "prompt_output_schema_target_kind",
                "error",
                format!("{path}#/output_contract/schema/properties/card_patches/items/properties/kind/enum"),
                format!("card_patches.kind enum must include target card kind {target_kind}"),
            ));
        }
    }

    let entry_required = &schema["properties"]["card_patches"]["items"]["properties"]["entries"]["items"]
        ["required"];
    for field in [
        "id",
        "title",
        "body",
        "applies_to",
        "evidence",
        "avoid",
        "confidence",
        "provenance",
        "status",
        "notes",
    ] {
        if !schema_array_contains(entry_required, field) {
            issues.push(issue(
                "prompt_output_schema_entry_required_field",
                "error",
                format!("{path}#/output_contract/schema/properties/card_patches/items/properties/entries/items/required"),
                format!("candidate entry schema must require {field}"),
            ));
        }
    }
}

fn schema_array_contains(value: &Value, expected: &str) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(expected)))
}

fn reference_uses_declared_input(reference: &str, declared_inputs: &BTreeSet<&str>) -> bool {
    declared_inputs.iter().any(|input| {
        reference == *input
            || reference.starts_with(&format!("{input}:"))
            || reference.starts_with(&format!("{input}."))
            || reference.starts_with(&format!("{input}["))
    })
}

fn validate_prompt_example(prompt: &PromptFile, path: &str, issues: &mut Vec<Value>) {
    let example = &prompt.output_contract.example;
    for field in &prompt.output_contract.required_top_level {
        if example.get(field).is_none() {
            issues.push(issue(
                "prompt_example_required_field_missing",
                "error",
                format!("{path}#/output_contract/example"),
                format!("prompt example is missing required field {field}"),
            ));
        }
    }
    if example["contract"].as_str() != Some(PROMPT_OUTPUT_CONTRACT) {
        issues.push(issue(
            "prompt_example_contract",
            "error",
            format!("{path}#/output_contract/example/contract"),
            format!("prompt example contract must be {PROMPT_OUTPUT_CONTRACT}"),
        ));
    }
    if example["prompt_id"].as_str() != Some(prompt.id.as_str()) {
        issues.push(issue(
            "prompt_example_id_mismatch",
            "error",
            format!("{path}#/output_contract/example/prompt_id"),
            "prompt example prompt_id must match prompt id",
        ));
    }
    let target_kinds = prompt
        .target_card_kinds
        .iter()
        .map(card_kind_name)
        .collect::<BTreeSet<_>>();
    let Some(card_patches) = example["card_patches"].as_array() else {
        issues.push(issue(
            "prompt_example_card_patches",
            "error",
            format!("{path}#/output_contract/example/card_patches"),
            "prompt example card_patches must be an array",
        ));
        return;
    };
    for patch in card_patches {
        let kind = patch["kind"].as_str().unwrap_or_default();
        if !target_kinds.contains(kind) {
            issues.push(issue(
                "prompt_example_target_mismatch",
                "error",
                format!("{path}#/output_contract/example/card_patches"),
                format!("example patch kind {kind} is not in target_card_kinds"),
            ));
        }
        let Some(entries) = patch["entries"].as_array() else {
            issues.push(issue(
                "prompt_example_entries",
                "error",
                format!("{path}#/output_contract/example/card_patches"),
                "each card patch must include entries array",
            ));
            continue;
        };
        for entry in entries {
            for field in [
                "id",
                "title",
                "body",
                "applies_to",
                "evidence",
                "avoid",
                "confidence",
                "provenance",
                "status",
                "notes",
            ] {
                if entry.get(field).is_none() {
                    issues.push(issue(
                        "prompt_example_entry_field_missing",
                        "error",
                        format!("{path}#/output_contract/example/card_patches/entries"),
                        format!("example entries must include {field}"),
                    ));
                }
            }
            let body = entry["body"].as_str().unwrap_or_default();
            let status = entry["status"].as_str().unwrap_or_default();
            let evidence_count = entry["evidence"].as_array().map_or(0, |items| items.len());
            let provenance_count = entry["provenance"]
                .as_array()
                .map_or(0, |items| items.len());
            if body != "N/A" && status != "gap" && evidence_count == 0 && provenance_count == 0 {
                issues.push(issue(
                    "prompt_example_entry_unproven",
                    "error",
                    format!("{path}#/output_contract/example/card_patches/entries"),
                    "non-gap example entries with a real body need evidence or provenance",
                ));
            }
        }
    }
}

fn validate_prompt_example_input_references(
    prompt: &PromptFile,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let declared_inputs = prompt
        .inputs
        .iter()
        .map(|input| input.name.as_str())
        .collect::<BTreeSet<_>>();
    let example = &prompt.output_contract.example;
    let inputs_used = example["source_summary"]["inputs_used"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for (index, input) in inputs_used.iter().enumerate() {
        let Some(input) = input.as_str() else {
            continue;
        };
        if !declared_inputs.contains(input) {
            issues.push(issue(
                "prompt_example_inputs_used_undeclared",
                "error",
                format!("{path}#/output_contract/example/source_summary/inputs_used/{index}"),
                format!("prompt example inputs_used references undeclared input {input}"),
            ));
        }
    }

    let Some(card_patches) = example["card_patches"].as_array() else {
        return;
    };
    let mut saw_supporting_reference = false;
    for (patch_index, patch) in card_patches.iter().enumerate() {
        let Some(entries) = patch["entries"].as_array() else {
            continue;
        };
        for (entry_index, entry) in entries.iter().enumerate() {
            validate_prompt_example_references(
                entry["evidence"].as_array(),
                &declared_inputs,
                &format!(
                    "{path}#/output_contract/example/card_patches/{patch_index}/entries/{entry_index}/evidence"
                ),
                "prompt_example_evidence_reference_undeclared",
                &mut saw_supporting_reference,
                issues,
            );
            validate_prompt_example_references(
                entry["provenance"].as_array(),
                &declared_inputs,
                &format!(
                    "{path}#/output_contract/example/card_patches/{patch_index}/entries/{entry_index}/provenance"
                ),
                "prompt_example_provenance_reference_undeclared",
                &mut saw_supporting_reference,
                issues,
            );
        }
    }

    if saw_supporting_reference && inputs_used.is_empty() {
        issues.push(issue(
            "prompt_example_inputs_used_empty",
            "error",
            format!("{path}#/output_contract/example/source_summary/inputs_used"),
            "prompt example source_summary.inputs_used must name declared inputs when evidence or provenance is present",
        ));
    }
}

fn validate_prompt_example_references(
    items: Option<&Vec<Value>>,
    declared_inputs: &BTreeSet<&str>,
    path: &str,
    code: &str,
    saw_supporting_reference: &mut bool,
    issues: &mut Vec<Value>,
) {
    let Some(items) = items else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        let Some(reference) = item.as_str() else {
            continue;
        };
        *saw_supporting_reference = true;
        if !reference_uses_declared_input(reference, declared_inputs) {
            issues.push(issue(
                code,
                "error",
                format!("{path}/{index}"),
                format!(
                    "prompt example reference {reference} does not match a declared prompt input"
                ),
            ));
        }
    }
}

fn validate_prompt_normalization_example(prompt: &PromptFile, path: &str, issues: &mut Vec<Value>) {
    let example = &prompt.output_contract.example;
    let Some(prospect) = example["normalized_prospect"].as_object() else {
        issues.push(issue(
            "prompt_normalized_prospect_missing",
            "error",
            format!("{path}#/output_contract/example/normalized_prospect"),
            "prospect-normalization examples must include normalized_prospect object",
        ));
        return;
    };
    for field in ["name", "title", "company"] {
        if prospect
            .get(field)
            .and_then(|value| value.as_str())
            .is_none_or(|value| value.trim().is_empty())
        {
            issues.push(issue(
                "prompt_normalized_prospect_required_field",
                "error",
                format!("{path}#/output_contract/example/normalized_prospect/{field}"),
                format!("normalized_prospect must include non-empty {field}"),
            ));
        }
    }
    if let Some(signals) = prospect.get("signals") {
        let Some(signals) = signals.as_array() else {
            issues.push(issue(
                "prompt_normalized_prospect_signals",
                "error",
                format!("{path}#/output_contract/example/normalized_prospect/signals"),
                "normalized_prospect.signals must be an array when present",
            ));
            return;
        };
        for signal in signals {
            for field in ["id", "title"] {
                if signal
                    .get(field)
                    .and_then(|value| value.as_str())
                    .is_none_or(|value| value.trim().is_empty())
                {
                    issues.push(issue(
                        "prompt_normalized_prospect_signal_required_field",
                        "error",
                        format!("{path}#/output_contract/example/normalized_prospect/signals"),
                        format!("normalized_prospect signals must include non-empty {field}"),
                    ));
                }
            }
        }
    }
    let Some(trace) = example["normalization_trace"].as_object() else {
        issues.push(issue(
            "prompt_normalization_trace_missing",
            "error",
            format!("{path}#/output_contract/example/normalization_trace"),
            "prospect-normalization examples must include normalization_trace object",
        ));
        return;
    };
    if !trace.contains_key("fit_readiness") {
        issues.push(issue(
            "prompt_normalization_trace_fit_readiness",
            "error",
            format!("{path}#/output_contract/example/normalization_trace/fit_readiness"),
            "normalization_trace must include fit_readiness so upstream agents expose whether mdp fit has enough context",
        ));
    }
}

fn card_kind_name(kind: &CardKind) -> &'static str {
    match kind {
        CardKind::Personas => "personas",
        CardKind::Pains => "pains",
        CardKind::Motions => "motions",
        CardKind::Hooks => "hooks",
        CardKind::AvoidRules => "avoid-rules",
        CardKind::OutputRules => "output-rules",
        CardKind::CopyPatterns => "copy-patterns",
        CardKind::Ctas => "ctas",
        CardKind::FitRules => "fit-rules",
        CardKind::Claims => "claims",
        CardKind::Signals => "signals",
        CardKind::Positioning => "positioning",
        CardKind::ChannelPolicies => "channel-policies",
        CardKind::Objections => "objections",
        CardKind::Gaps => "gaps",
    }
}

pub(crate) fn explain(root: &Path, persona: Option<&str>) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let selected = select_cards(&manifest, persona, None);
    Ok(json!({
        "format": manifest.format,
        "name": manifest.name,
        "principle": "Load the manifest first, then load only the card paths returned for the persona/job.",
        "persona": persona,
        "cards_to_consider": selected,
        "do_not": [
            "Do not ingest every card unless route says the task needs it.",
            "Do not treat the decision pack as a sequencer, CRM, enrichment tool, or execution agent.",
            "Do not invent missing GTM facts; surface gaps in the brief."
        ]
    }))
}

pub(crate) fn gaps(root: &Path) -> Result<Value> {
    let manifest = read_manifest(root)?;
    let mut durable_gaps = Vec::new();
    let mut evidence_gaps = Vec::new();
    if let Ok(card) = read_card_by_id(root, "gaps") {
        for entry in card.entries {
            durable_gaps.push(json!({"id": entry.id, "title": entry.title, "body": entry.body, "applies_to": entry.applies_to}));
        }
    }
    for card_ref in &manifest.cards {
        let card = read_card(&resolve_pack_path(root, &card_ref.path)?)?;
        for entry in &card.entries {
            if entry.evidence.is_empty()
                && !matches!(
                    card.kind,
                    CardKind::AvoidRules | CardKind::OutputRules | CardKind::Gaps | CardKind::Ctas
                )
            {
                evidence_gaps.push(json!({"card_id": card.id, "entry_id": entry.id, "title": entry.title, "reason": "missing evidence"}));
            }
        }
    }
    let durable_count = durable_gaps.len();
    let evidence_count = evidence_gaps.len();
    Ok(json!({
        "contract": "mdp.gaps.v0",
        "durable_gaps": durable_gaps,
        "evidence_gaps": evidence_gaps,
        "summary": {"durable": durable_count, "evidence": evidence_count}
    }))
}

pub(crate) fn issue(
    code: &str,
    severity: &str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> Value {
    json!({
        "code": code,
        "severity": severity,
        "path": path.into(),
        "message": message.into()
    })
}

fn issue_with_gate(
    code: &str,
    severity: &str,
    path: impl Into<String>,
    message: impl Into<String>,
    strict: &str,
    activation: &str,
) -> Value {
    json!({
        "code": code,
        "severity": severity,
        "path": path.into(),
        "message": message.into(),
        "strict": strict,
        "activation": activation
    })
}

fn issue_count(issues: &[Value], severity: &str) -> usize {
    issues
        .iter()
        .filter(|issue| issue["severity"].as_str() == Some(severity))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_pack(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-{name}-{nonce}"));
        init_pack(&root, "Example Message Pack", "gtm", true, false)
            .expect("starter pack should initialize");
        root
    }

    #[test]
    fn validate_rejects_manifest_card_path_traversal() {
        let root = temp_pack("path-traversal");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace("path: cards/personas.yaml", "path: ../secrets.yaml"),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "invalid_card_path")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_manifest_absolute_card_path() {
        let root = temp_pack("path-absolute");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace("path: cards/personas.yaml", "path: /tmp/personas.yaml"),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "invalid_card_path")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_accepts_starter_prompts() {
        let root = temp_pack("starter-prompts");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], true);
        assert_eq!(result["error_count"], 0);
        assert_eq!(result["profile"]["activation_ready"], true);
        assert_eq!(
            result["prompts"].as_array().expect("prompts array").len(),
            10
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_distinguishes_profile_surface_from_activation() {
        let root = temp_pack("profile-surface-only");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut value: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        let map = value
            .as_mapping_mut()
            .expect("manifest should be a mapping");
        for key in [
            "required_primitives",
            "primitive_map",
            "input_contracts",
            "jobs",
            "profile_eval",
        ] {
            map.remove(YamlValue::String(key.to_string()));
        }
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&value).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], true);
        assert_eq!(result["profile"]["present"], true);
        assert_eq!(result["profile"]["activation_ready"], false);
        assert_eq!(
            result["profile"]["missing_required_primitives"]
                .as_array()
                .unwrap()
                .len(),
            0
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_marks_partial_activation_metadata_not_ready() {
        let root = temp_pack("profile-partial-activation");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut value: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        value
            .as_mapping_mut()
            .expect("manifest should be a mapping")
            .remove(YamlValue::String("profile_eval".to_string()));
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&value).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        let issues = result["issues"].as_array().expect("issues array");

        assert_eq!(result["valid"], true);
        assert_eq!(result["profile"]["activation_ready"], false);
        assert!(
            result["profile"]["missing_activation_sections"]
                .as_array()
                .expect("missing activation sections")
                .iter()
                .any(|section| section == "profile_eval.required_categories")
        );
        assert!(issues.iter().any(|issue| {
            issue["code"] == "profile_activation_section_missing"
                && issue["activation"] == "blocks"
                && issue["strict"] == "fails"
        }));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_unknown_profile_primitive() {
        let root = temp_pack("profile-primitive-unknown");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replacen("- actors\n", "- account-context\n- actors\n", 1),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        let codes: Vec<&str> = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter_map(|issue| issue["code"].as_str())
            .collect();

        assert_eq!(result["valid"], false);
        assert!(codes.contains(&"profile_required_primitive_unknown"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_missing_profile_mapping_reference() {
        let root = temp_pack("profile-missing-reference");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace("- normalize-prospect-row", "- missing-normalizer"),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "profile_primitive_prompt_missing")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_bad_eval_profile_metadata_refs() {
        let root = temp_pack("profile-eval-metadata-refs");
        let fixture_path = root
            .join(".mdp")
            .join("evals")
            .join("bad-profile-metadata.yaml");
        std::fs::write(
            &fixture_path,
            r#"id: bad-profile-metadata
command: route
persona: PMM
job: linkedin outbound copy
profile_eval:
  category: prompt-output-validation
  primitives:
    - account-context
  jobs:
    - missing-profile-job
expect_load_order_contains:
  - .mdp/cards/personas.yaml
"#,
        )
        .expect("fixture should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        let codes: Vec<&str> = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter_map(|issue| issue["code"].as_str())
            .collect();

        assert_eq!(result["valid"], false);
        assert!(codes.contains(&"eval_profile_primitive_unknown"));
        assert!(codes.contains(&"eval_profile_job_missing"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_warns_when_required_profile_primitive_is_unmapped() {
        let root = temp_pack("profile-required-unmapped");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace(
                "  gaps:\n    cards:\n    - gaps\n    evals:\n    - fit-insufficient-context\n    - brief-insufficient-context\n    - account-context-missing\n    - account-only-no-draft\n    - prompt-output-missing-readiness-boolean\n    - prompt-output-missing-required-field\n    - prompt-output-validation\n",
                "",
            ),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], true);
        assert_eq!(result["profile"]["activation_ready"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(
                    |issue| issue["code"] == "profile_required_primitive_unmapped"
                        && issue["strict"] == "fails"
                        && issue["activation"] == "blocks"
                )
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_unknown_lead_input_requirements() {
        let root = temp_pack("lead-input-requirements");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let mut raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        raw = raw.replace(
            "required_fields:\n  - name",
            "required_fields:\n  - company_size\n  - name",
        );
        raw = raw.replace(
            "required_signal_fields:\n  - source",
            "required_signal_fields:\n  - origin\n  - source",
        );
        raw = raw.replace(
            "required_attributes: []",
            "required_attributes:\n  - fiscal year",
        );
        raw = raw.replace(
            "value_contracts:\n    segment:",
            "value_contracts:\n    unsupported_field:\n      type: object\n      enumm:\n      - enterprise\n    segment:",
        );
        raw = raw.replace(
            "attribute_definitions:\n    fiscal_year:",
            "attribute_definitions:\n    renewal date:\n      type: string\n      format: month\n    fiscal_year:\n      type: integer\n      enum:\n      - \"2027\"\n    close_date:",
        );
        std::fs::write(&manifest_path, raw).expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        let codes: Vec<&str> = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter_map(|issue| issue["code"].as_str())
            .collect();

        assert_eq!(result["valid"], false);
        assert!(codes.contains(&"lead_input_required_field_unknown"));
        assert!(codes.contains(&"lead_input_required_signal_field_unknown"));
        assert!(codes.contains(&"lead_input_required_attribute_invalid"));
        assert!(codes.contains(&"lead_input_value_contract_field_unknown"));
        assert!(codes.contains(&"lead_input_value_contract_type_unknown"));
        assert!(codes.contains(&"lead_input_attribute_definition_key_invalid"));
        assert!(codes.contains(&"lead_input_value_contract_format_unknown"));
        assert!(codes.contains(&"lead_input_value_contract_unknown_field"));
        assert!(codes.contains(&"lead_input_value_contract_enum_type"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_reports_bad_profile_agent_surface() {
        let root = temp_pack("profile-agent-surface");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let mut raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        raw = raw.replace(
            "  id: gtm\n  label: GTM Messaging",
            "  id: ''\n  unknown_profile_field: ignored\n  label: GTM Messaging",
        );
        raw = raw.replace(
            "    recommended_skills:\n    - mdp",
            "    recommended_skills:\n    - ''\n    - mdp",
        );
        raw = raw.replace(
            "    blocked_skills:\n    - name: mdp-proposal-pack-builder",
            "    blocked_skills:\n    - name: mdp\n      reason: ''\n    - name: mdp-proposal-pack-builder",
        );
        raw = raw.replace(
            "    - job: create or improve GTM messaging pack",
            "    - job: ''\n      skills:\n      - ''\n    - job: create or improve GTM messaging pack",
        );
        std::fs::write(&manifest_path, raw).expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        let codes: Vec<&str> = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter_map(|issue| issue["code"].as_str())
            .collect();

        assert_eq!(result["valid"], false);
        assert!(codes.contains(&"manifest_profile_unknown_field"));
        assert!(codes.contains(&"profile_id_empty"));
        assert!(codes.contains(&"profile_recommended_skill_empty"));
        assert!(codes.contains(&"profile_blocked_skill_reason_empty"));
        assert!(codes.contains(&"profile_blocked_skill_overlap"));
        assert!(codes.contains(&"profile_job_skill_job_empty"));
        assert!(codes.contains(&"profile_job_skill_skill_empty"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_prompt_without_strict_json_output() {
        let root = temp_pack("prompt-strict-json");
        let prompt_path = root.join(".mdp").join("prompts").join("icp-persona.yaml");
        let raw = std::fs::read_to_string(&prompt_path).expect("prompt should be readable");
        std::fs::write(
            &prompt_path,
            raw.replace("strict_json_only: true", "strict_json_only: false"),
        )
        .expect("prompt should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "prompt_output_not_strict_json")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_prompt_without_output_schema() {
        let root = temp_pack("prompt-output-schema");
        let prompt_path = root.join(".mdp").join("prompts").join("bad.yaml");
        std::fs::write(
            &prompt_path,
            r#"format: mdp.prompt.v0
id: bad-prompt
title: Bad prompt
description: Bad prompt
target_card_kinds:
- claims
inputs:
- name: company_data
  description: Company data
  required: false
  default: N/A
  missing_behavior: Use N/A.
instructions:
- Return strict JSON only.
output_contract:
  contract: mdp.prompt-output.v0
  strict_json_only: true
  required_top_level:
  - contract
  - prompt_id
  - source_summary
  - card_patches
  - gaps
  - rejected_claims
  entry_defaults:
    body: N/A
    applies_to: []
    evidence: []
    avoid: []
    confidence: unknown
    provenance: []
  example:
    contract: mdp.prompt-output.v0
    prompt_id: bad-prompt
    source_summary:
      company_domain: N/A
      company_name: N/A
      inputs_used: []
      confidence: unknown
    card_patches:
    - card_id: claims
      kind: claims
      entries:
      - id: gap-claim-proof
        title: Missing claim proof
        body: N/A
        applies_to: []
        evidence: []
        avoid: []
        confidence: unknown
        provenance: []
        status: gap
    gaps: []
    rejected_claims: []
"#,
        )
        .expect("bad prompt should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "prompt_output_schema_missing")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_prompt_example_entry_without_proof() {
        let root = temp_pack("prompt-unproven-entry");
        let prompt_path = root.join(".mdp").join("prompts").join("bad.yaml");
        std::fs::write(
            &prompt_path,
            r#"format: mdp.prompt.v0
id: bad-prompt
title: Bad prompt
description: Bad prompt
target_card_kinds:
- claims
inputs:
- name: company_data
  description: Company data
  required: false
  default: N/A
  missing_behavior: Use N/A.
instructions:
- Return JSON.
output_contract:
  contract: mdp.prompt-output.v0
  strict_json_only: true
  required_top_level:
  - contract
  - prompt_id
  - source_summary
  - card_patches
  - gaps
  - rejected_claims
  entry_defaults:
    body: N/A
    applies_to: []
    evidence: []
    avoid: []
    confidence: unknown
    provenance: []
  example:
    contract: mdp.prompt-output.v0
    prompt_id: bad-prompt
    source_summary:
      company_domain: N/A
      company_name: N/A
      inputs_used: []
      confidence: unknown
    card_patches:
    - card_id: claims
      kind: claims
      entries:
      - id: unsupported
        title: Unsupported claim
        body: This company has proven quantified outcomes.
        applies_to: []
        evidence: []
        avoid: []
        confidence: high
        provenance: []
        status: candidate
    gaps: []
    rejected_claims: []
"#,
        )
        .expect("bad prompt should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "prompt_example_entry_unproven")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_warns_on_unknown_persona_mapping_persona() {
        let root = temp_pack("persona-mapping-unknown");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace("persona: PMM", "persona: Sales Development"),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "persona_mapping_unknown_persona")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_warns_on_unsupported_entry_fields() {
        let root = temp_pack("entry-unknown-field");
        let card_path = root.join(".mdp").join("cards").join("hooks.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("card should be readable");
        std::fs::write(
            &card_path,
            raw.replace(
                "  body: Position the pack",
                "  owner: PMM\n  body: Position the pack",
            ),
        )
        .expect("card should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "card_entry_unknown_field"
                    && issue["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with("/owner")))
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_warns_on_unsupported_constraint_fields() {
        let root = temp_pack("constraint-unknown-field");
        let card_path = root
            .join(".mdp")
            .join("cards")
            .join("channel-policies.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("card should be readable");
        std::fs::write(
            &card_path,
            raw.replace(
                "    word_count:",
                "    sentence_count:\n      max: 3\n    word_count:",
            ),
        )
        .expect("card should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "unsupported_constraint_field"
                    && issue["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with("/constraints/sentence_count")))
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_warns_on_unsupported_proof_output_constraint_fields() {
        let root = temp_pack("proof-constraint-unknown-field");
        let card_path = root
            .join(".mdp")
            .join("cards")
            .join("channel-policies.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("card should be readable");
        std::fs::write(
            &card_path,
            raw.replace(
                "    word_count:",
                "    proof_output:\n      required_sections:\n      - Summary\n      max_connective_words: 18\n    word_count:",
            ),
        )
        .expect("card should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "unsupported_constraint_field"
                    && issue["path"].as_str().is_some_and(
                        |path| path.ends_with("/constraints/proof_output/required_sections")
                    ))
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_accepts_known_proof_output_constraint_fields() {
        let root = temp_pack("proof-constraint-known-fields");
        let card_path = root
            .join(".mdp")
            .join("cards")
            .join("channel-policies.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("card should be readable");
        std::fs::write(
            &card_path,
            raw.replace(
                "    word_count:",
                "    proof_output:\n      required_segment_kinds:\n      - claim\n      min_segments:\n        gap: 1\n      require_source_refs_for_claims: true\n      max_connective_words: 18\n    word_count:",
            ),
        )
        .expect("card should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert!(
            !result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "unsupported_constraint_field")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_accepts_entry_metadata_map() {
        let root = temp_pack("entry-metadata");
        let card_path = root.join(".mdp").join("cards").join("hooks.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("card should be readable");
        std::fs::write(
            &card_path,
            raw.replace(
                "  body: Position the pack",
                "  metadata:\n    owner: PMM\n    lifecycle: advisory\n  body: Position the pack",
            ),
        )
        .expect("card should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .all(|issue| issue["code"] != "card_entry_unknown_field"
                    && issue["code"] != "card_entry_metadata_type")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn validate_rejects_manifest_card_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = temp_pack("path-symlink");
        let outside = root.join("outside-card.yaml");
        std::fs::write(
            &outside,
            r#"id: personas
kind: personas
title: Outside
description: Outside
entries: []
"#,
        )
        .expect("outside card should be writable");
        let link = root.join(".mdp").join("cards").join("escaped.yaml");
        symlink(&outside, &link).expect("symlink should be creatable");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace("path: cards/personas.yaml", "path: cards/escaped.yaml"),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| issue["code"] == "invalid_card_path")
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
