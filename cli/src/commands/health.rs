use crate::constants::{
    DEFAULT_DIR, FORMAT_NAME, FORMAT_VERSION, NORMALIZED_DECISION_INPUT_CONTRACT,
    NORMALIZED_DECISION_INPUT_CONTRACT_V2, PROMPT_CARD_PATCH_SCHEMA_REF, PROMPT_FORMAT_V1,
    PROMPT_FORMAT_VERSION, PROMPT_OUTPUT_CONTRACT, PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF,
};

use crate::models::{
    Card, CardKind, DecisionInputAttemptStatus, DecisionInputContract, DecisionInputDecisionEffect,
    DecisionInputDisposition, DecisionInputRequirement, InputContract, MAX_SIGNAL_CONTRIBUTORS,
    MAX_SIGNAL_IDENTIFIER_LEN, MAX_SIGNAL_KIND_LEN, MAX_SIGNAL_OBSERVATIONS_PER_ENVELOPE,
    MAX_SIGNAL_PROJECTIONS_PER_CONTRACT, Manifest, PrimitiveMapping, ProductFoundationBinding,
    ProductFoundationConditionFact, ProductFoundationEntryRef, ProductFoundationFacetKind, Profile,
    ProfileEval, ProfileJob, PromptFile, QualificationGates, TargetIdentity, ValueContract,
};
use crate::pack_io::{
    display_pack_path, read_card, read_card_by_id, read_manifest, read_prompt, resolve_pack_path,
};
use crate::product_foundation::{
    ProductFoundationIndex, apply_validation_errors_for_job, resolution_json,
    resolve_product_foundation,
};
use crate::routing::select_cards;
use crate::scope::valid_declared_identifier;
use crate::skill_catalog::{JOB_ROUTE_SPECS, is_packaged_skill, route_spec};
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

const BUILTIN_INTERNAL_TARGET_TERMS: &[&str] = &[
    "MDP",
    "Message Decision Pack",
    "mdp CLI",
    "manifest plus modular cards",
    "local offline decision layer",
    "agent handoffs",
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
    let mut card_entry_index: BTreeMap<String, (CardKind, BTreeSet<String>, BTreeSet<String>)> =
        BTreeMap::new();
    let mut foundation_cards = Vec::new();
    let mut loaded_cards = Vec::new();
    let mut scoped_entry_count = 0usize;
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
    let selector_names: BTreeSet<String> = manifest
        .personas
        .iter()
        .chain(manifest.target_personas.iter())
        .chain(manifest.operator_roles.iter())
        .map(|value| value.to_lowercase())
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
    validate_qualification_gates(manifest.qualification_gates.as_ref(), &mut issues);
    validate_profile(manifest.profile.as_ref(), &mut issues);
    for (card_index, card_ref) in manifest.cards.iter().enumerate() {
        if !card_ids.insert(card_ref.id.clone()) {
            issues.push(issue(
                "duplicate_card_id",
                "error",
                ".mdp/manifest.yaml#/cards",
                format!("duplicate card id {}", card_ref.id),
            ));
        }
        validate_persona_selector(
            &card_ref.personas,
            &selector_names,
            ".mdp/manifest.yaml",
            &format!("/cards/{card_index}/personas"),
            "manifest_card_persona_undeclared",
            "manifest card persona",
            &mut issues,
        );
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
                scoped_entry_count += card
                    .entries
                    .iter()
                    .filter(|entry| !entry.scope.is_empty())
                    .count();
                validate_card_shape(&path, &display_path, &mut issues);
                validate_card_persona_references(
                    &card,
                    &selector_names,
                    &display_path,
                    &mut issues,
                );
                validate_card_entry_scopes(
                    &card,
                    manifest.profile.as_ref(),
                    &display_path,
                    &mut issues,
                );
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
                let mut entry_ids = BTreeSet::new();
                let mut duplicate_entry_ids = BTreeSet::new();
                for entry in &card.entries {
                    if !entry_ids.insert(entry.id.clone()) {
                        duplicate_entry_ids.insert(entry.id.clone());
                    }
                }
                card_entry_index.insert(
                    card_ref.id.clone(),
                    (card.kind.clone(), entry_ids, duplicate_entry_ids),
                );
                loaded_cards.push(json!({"id": card.id, "kind": card_ref.kind, "path": display_path, "entries": card.entries.len()}));
                foundation_cards.push(card);
            }
            Err(err) => issues.push(issue(
                "card_read_failed",
                "error",
                display_path,
                err.to_string(),
            )),
        }
    }
    validate_product_foundation(&manifest, &card_entry_index, &mut issues);
    let product_foundation_index = ProductFoundationIndex::from_cards(&foundation_cards);
    let loaded_prompts = validate_prompts(root, &mut issues)?;
    let prompt_inventory = prompt_inventory(&loaded_prompts);
    validate_decision_input_contracts(&manifest, &prompt_inventory, &mut issues);
    let eval_inventory = collect_eval_inventory(root, &mut issues)?;
    if scoped_entry_count > 0 {
        let (has_selected_scope, has_missing_scope) = portfolio_eval_coverage(root)?;
        if !has_selected_scope || !has_missing_scope {
            issues.push(issue(
                "portfolio_scope_eval_coverage_missing",
                "warning",
                format!("{DEFAULT_DIR}/evals"),
                "packs with scoped entries should include both selected-scope isolation and missing-scope blocking eval fixtures",
            ));
        }
    }
    let profile = validate_profile_mapping(
        &manifest,
        &card_ids,
        &prompt_inventory,
        &eval_inventory,
        &product_foundation_index,
        &mut issues,
    );
    validate_target_identity(root, &manifest, &mut issues)?;
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

fn validate_card_persona_references(
    card: &Card,
    declared_personas: &BTreeSet<String>,
    display_path: &str,
    issues: &mut Vec<Value>,
) {
    validate_persona_selector(
        &card.personas,
        declared_personas,
        display_path,
        "/personas",
        "card_persona_undeclared",
        "card persona",
        issues,
    );
    for (entry_index, entry) in card.entries.iter().enumerate() {
        validate_persona_selector(
            &entry.applies_to,
            declared_personas,
            display_path,
            &format!("/entries/{entry_index}/applies_to"),
            "card_entry_applies_to_persona_undeclared",
            "card entry applies_to persona",
            issues,
        );
    }
}

fn validate_persona_selector(
    values: &[String],
    declared_personas: &BTreeSet<String>,
    display_path: &str,
    pointer: &str,
    code: &str,
    label: &str,
    issues: &mut Vec<Value>,
) {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let normalized = value.trim().to_lowercase();
        if normalized.is_empty() || !seen.insert(normalized.clone()) {
            continue;
        }
        if !declared_personas.contains(&normalized) {
            issues.push(issue(
                code,
                "warning",
                format!("{display_path}#{pointer}/{index}"),
                format!(
                    "{label} '{value}' is not listed in manifest personas; declare it or remove the selector"
                ),
            ));
        }
    }
}

pub(crate) fn profile_activation_decision(
    validation: &Value,
    explicit_activation_blocks: bool,
    job_id: Option<&str>,
) -> Value {
    let profile = &validation["profile"];
    if profile["present"] != true {
        return json!({
            "contract": "mdp.profile-activation-decision.v1",
            "status": "not-applicable",
            "activation_ready": Value::Null,
            "blocker_codes": [],
            "diagnostics": []
        });
    }

    let mut diagnostics = validation["issues"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|diagnostic| match diagnostic["activation"].as_str() {
            Some("blocks") => true,
            Some("blocks-job") => job_id.is_none_or(|job_id| {
                diagnostic["path"]
                    .as_str()
                    .is_some_and(|path| path.contains(&format!("/jobs/{job_id}/")))
            }),
            _ => false,
        })
        .cloned()
        .collect::<Vec<_>>();
    if explicit_activation_blocks {
        diagnostics.push(json!({
            "code": "profile_activation_not_ready",
            "severity": "error",
            "path": ".mdp/manifest.yaml#/profile_eval/activation/status",
            "message": "profile activation requires review or is blocked"
        }));
    }
    let blocker_codes = diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic["code"].as_str().map(str::to_string))
        .collect::<BTreeSet<_>>();
    let activation_ready = blocker_codes.is_empty();

    json!({
        "contract": "mdp.profile-activation-decision.v1",
        "status": if activation_ready { "ready" } else { "blocked" },
        "activation_ready": activation_ready,
        "computed_profile_activation_ready": profile["activation_ready"],
        "blocker_codes": blocker_codes.into_iter().collect::<Vec<_>>(),
        "diagnostics": diagnostics
    })
}

fn validate_target_identity(
    root: &Path,
    manifest: &Manifest,
    issues: &mut Vec<Value>,
) -> Result<()> {
    let Some(target) = manifest.target.as_ref() else {
        return Ok(());
    };
    if !matches!(target.kind.as_str(), "company" | "product" | "project") {
        issues.push(issue(
            "target_identity_kind_invalid",
            "error",
            ".mdp/manifest.yaml#/target/kind",
            "target.kind must be company, product, or project",
        ));
    }
    if target.name.trim().is_empty() {
        issues.push(issue(
            "target_identity_name_empty",
            "error",
            ".mdp/manifest.yaml#/target/name",
            "target.name must resolve the external company, product, or project before authoring",
        ));
    }
    if target.source_ids.is_empty() {
        issues.push(issue(
            "target_identity_sources_empty",
            "error",
            ".mdp/manifest.yaml#/target/source_ids",
            "target identity must cite at least one source ledger entry; unsupported product detail belongs in gaps",
        ));
    } else {
        let source_ids = target_source_ids(root)?;
        for (index, source_id) in target.source_ids.iter().enumerate() {
            if !source_ids.contains(source_id) {
                issues.push(issue(
                    "target_identity_source_missing",
                    "error",
                    format!(".mdp/manifest.yaml#/target/source_ids/{index}"),
                    format!(
                        "target identity source '{source_id}' does not exist in .mdp/sources.yaml"
                    ),
                ));
            }
        }
    }
    let source_claims = target_source_direct_claims(root, &target.source_ids)?;
    for (index, term) in target.external_terms.iter().enumerate() {
        let identity_term = std::iter::once(&target.name)
            .chain(target.aliases.iter())
            .any(|identity| identity.eq_ignore_ascii_case(term));
        if !identity_term && !source_claims.iter().any(|claim| contains_term(claim, term)) {
            issues.push(issue(
                "target_external_term_source_missing",
                "error",
                format!(".mdp/manifest.yaml#/target/external_terms/{index}"),
                format!(
                    "external target term '{term}' must appear in a direct_claim from a source listed in target.source_ids; otherwise keep it as a gap"
                ),
            ));
        }
    }
    for (index, excluded) in target.excluded_terms.iter().enumerate() {
        if excluded.trim().len() < 2 {
            issues.push(issue(
                "target_excluded_term_too_short",
                "error",
                format!(".mdp/manifest.yaml#/target/excluded_terms/{index}"),
                "excluded target terms must contain at least two characters to avoid broad false positives",
            ));
        }
        if std::iter::once(&target.name)
            .chain(target.aliases.iter())
            .chain(target.external_terms.iter())
            .any(|allowed| allowed.eq_ignore_ascii_case(excluded))
        {
            issues.push(issue(
                "target_lexicon_conflict",
                "error",
                format!(".mdp/manifest.yaml#/target/excluded_terms/{index}"),
                format!(
                    "excluded term '{excluded}' conflicts with the active target name or alias"
                ),
            ));
        }
    }

    let files = target_scan_files(root)?;
    for path in files {
        let display = display_target_scan_path(root, &path);
        for excluded in &target.excluded_terms {
            if contains_term(&display, excluded) {
                issues.push(issue(
                    "target_contamination_excluded_term",
                    "error",
                    &display,
                    format!("excluded prior-target or starter term '{excluded}' appears in the file path"),
                ));
            }
        }
        let raw = fs::read_to_string(&path)?;
        let Some(value) = parse_scan_value(&path, &raw) else {
            for (line_index, line) in raw.lines().enumerate() {
                for excluded in &target.excluded_terms {
                    if contains_term(line, excluded) {
                        issues.push(issue(
                            "target_contamination_excluded_term",
                            "error",
                            format!("{display}:{}", line_index + 1),
                            format!("excluded prior-target or starter term '{excluded}' survived in a generated artifact"),
                        ));
                    }
                }
                if is_raw_external_surface(&display) {
                    let external_text = redact_active_target_identity(
                        target,
                        &strip_internal_implementation_tokens(line, is_raw_internal_receipt(line)),
                    );
                    if let Some(internal) = internal_target_terms(target)
                        .filter(|internal| contains_term(&external_text, internal))
                        .max_by_key(|internal| internal.len())
                    {
                        if !internal_term_is_only_negated(&external_text, internal) {
                            issues.push(issue(
                                "target_contamination_internal_vocabulary",
                                "error",
                                format!("{display}:{}", line_index + 1),
                                format!("internal control-plane term '{internal}' appears in positioning copy; position '{}' instead", target.name),
                            ));
                        }
                    }
                }
            }
            continue;
        };
        walk_strings(&value, "", &mut |pointer, text| {
            if display == ".mdp/manifest.yaml"
                && (pointer.starts_with("/target/excluded_terms")
                    || pointer.starts_with("/target/internal_terms"))
            {
                return;
            }
            for excluded in &target.excluded_terms {
                if contains_term(text, excluded) {
                    issues.push(issue(
                        "target_contamination_excluded_term",
                        "error",
                        format!("{display}#{pointer}"),
                        format!("excluded prior-target or starter term '{excluded}' survived target authoring"),
                    ));
                }
            }
            let internal_scan_text = redact_active_target_identity(
                target,
                &strip_internal_implementation_tokens(text, is_internal_receipt_pointer(pointer)),
            );
            if let Some(internal) = internal_target_terms(target)
                .filter(|internal| contains_term(&internal_scan_text, internal))
                .max_by_key(|internal| internal.len())
            {
                if is_external_surface(&display, pointer, text, &value)
                    && !internal_term_is_only_negated(&internal_scan_text, internal)
                {
                    issues.push(issue(
                        "target_contamination_internal_vocabulary",
                        "error",
                        format!("{display}#{pointer}"),
                        format!("internal control-plane term '{internal}' appears on a prospect-facing surface; position '{}' instead", target.name),
                    ));
                }
            }
        });
    }
    Ok(())
}

fn target_source_ids(root: &Path) -> Result<BTreeSet<String>> {
    let path = root.join(DEFAULT_DIR).join("sources.yaml");
    if !path.exists() {
        return Ok(BTreeSet::new());
    }
    let raw = fs::read_to_string(path)?;
    let value = serde_yaml::from_str::<Value>(&raw).unwrap_or(Value::Null);
    Ok(value["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|source| source["id"].as_str().map(str::to_string))
        .collect())
}

fn target_source_direct_claims(root: &Path, allowed_ids: &[String]) -> Result<Vec<String>> {
    let path = root.join(DEFAULT_DIR).join("sources.yaml");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path)?;
    let value = serde_yaml::from_str::<Value>(&raw).unwrap_or(Value::Null);
    Ok(value["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|source| {
            source["id"]
                .as_str()
                .is_some_and(|id| allowed_ids.iter().any(|allowed| allowed == id))
        })
        .flat_map(|source| source["direct_claims"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect())
}

fn target_scan_files(root: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for directory in [root.join(DEFAULT_DIR), root.join("examples")] {
        collect_scan_files(&directory, &mut files)?;
    }
    files.sort();
    Ok(files)
}

fn collect_scan_files(directory: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_scan_files(&path, files)?;
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("yaml" | "yml" | "json" | "md" | "txt")
        ) {
            files.push(path);
        }
    }
    Ok(())
}

fn display_target_scan_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn parse_scan_value(path: &Path, raw: &str) -> Option<Value> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => serde_json::from_str(raw).ok(),
        Some("yaml" | "yml") => serde_yaml::from_str(raw).ok(),
        _ => None,
    }
}

fn walk_strings(value: &Value, pointer: &str, visit: &mut impl FnMut(&str, &str)) {
    match value {
        Value::String(text) => visit(pointer, text),
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                walk_strings(value, &format!("{pointer}/{index}"), visit);
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                let key = key.replace('~', "~0").replace('/', "~1");
                walk_strings(value, &format!("{pointer}/{key}"), visit);
            }
        }
        _ => {}
    }
}

fn is_external_surface(display: &str, pointer: &str, text: &str, root: &Value) -> bool {
    if display.starts_with("examples/") {
        return true;
    }
    if display == ".mdp/manifest.yaml" {
        if pointer.starts_with("/jobs/") {
            return pointer.ends_with("/label") || pointer.ends_with("/description");
        }
        return matches_pointer_prefix(
            pointer,
            &[
                "/description",
                "/personas",
                "/target_personas",
                "/persona_mappings",
                "/profile/context_dimensions",
                "/cards",
            ],
        );
    }
    if display == ".mdp/sources.yaml" {
        return pointer == "/purpose"
            || pointer.contains("/direct_claims/")
            || pointer.contains("/interpretations/")
            || pointer.contains("/gaps/");
    }
    if display.starts_with(".mdp/cards/") {
        if root.get("kind").and_then(Value::as_str) == Some("avoid-rules")
            && pointer.contains("/avoid/")
        {
            return false;
        }
        return matches_pointer_prefix(
            pointer,
            &["/title", "/description", "/tags", "/personas", "/entries"],
        );
    }
    if display.starts_with(".mdp/prompts/") {
        return pointer.starts_with("/output_contract/example/card_patches")
            || pointer.starts_with("/output_contract/example/normalized_prospect")
            || ((pointer == "/description" || pointer.starts_with("/instructions/"))
                && has_explicit_positioning_instruction(text));
    }
    if display.starts_with(".mdp/evals/") {
        let negative_guardrail = root.get("command").and_then(Value::as_str)
            == Some("check-claims")
            && root.get("expect_valid").and_then(Value::as_bool) == Some(false);
        if negative_guardrail
            && (pointer == "/text" || pointer.starts_with("/expect_guardrail_terms_contains"))
        {
            return false;
        }
        return matches_pointer_prefix(
            pointer,
            &[
                "/persona",
                "/job",
                "/text",
                "/prospect",
                "/prompt_output",
                "/expect_entry_titles_contains",
                "/expect_entry_titles_excludes",
            ],
        );
    }
    if display.starts_with(".mdp/briefs/") || display.starts_with(".mdp/traces/") {
        let external_field = pointer.split('/').any(|segment| {
            matches!(
                segment,
                "body"
                    | "copy"
                    | "draft"
                    | "subject"
                    | "message"
                    | "positioning"
                    | "claim"
                    | "claims"
                    | "pain"
                    | "pains"
                    | "hook"
                    | "hooks"
                    | "cta"
                    | "audience"
                    | "job"
                    | "label"
            )
        });
        return external_field || has_external_positioning_intent(text);
    }
    false
}

fn is_raw_external_surface(display: &str) -> bool {
    display.starts_with(".mdp/briefs/") || display.starts_with(".mdp/traces/")
}

fn strip_internal_implementation_tokens(text: &str, allow_contract_token: bool) -> String {
    let tokens = text.split_whitespace().collect::<Vec<_>>();
    tokens
        .iter()
        .enumerate()
        .filter_map(|(index, raw)| {
            let token = normalized_scan_token(raw);
            let next = tokens
                .get(index + 1)
                .map(|next| normalized_scan_token(next));
            let implementation_token = token.contains(".mdp/")
                || (allow_contract_token && is_mdp_contract_token(&token))
                || token.starts_with("mdp.prompt")
                || token.starts_with("mdp.fit")
                || token.starts_with("mdp.brief")
                || token.starts_with("mdp.route")
                || (token == "mdp" && next.as_deref().is_some_and(is_mdp_cli_command_token));
            (!implementation_token).then_some(*raw)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_internal_receipt_pointer(pointer: &str) -> bool {
    let field = pointer.rsplit('/').next().unwrap_or_default();
    matches!(
        field,
        "artifact_type"
            | "brief_contract"
            | "context_contract"
            | "contract"
            | "implementation_ref"
            | "runtime_ref"
            | "schema_ref"
            | "source_artifact_type"
    ) || field.ends_with("_contract")
        || field.ends_with("_schema_ref")
}

fn is_raw_internal_receipt(line: &str) -> bool {
    let line = line.trim().trim_start_matches('-').trim();
    let Some((field, _)) = line.split_once(':') else {
        return false;
    };
    matches!(
        field.trim(),
        "artifact_type"
            | "brief_contract"
            | "context_contract"
            | "contract"
            | "implementation_ref"
            | "runtime_ref"
            | "schema_ref"
            | "source_artifact_type"
    ) || field.trim().ends_with("_contract")
        || field.trim().ends_with("_schema_ref")
}

fn normalized_scan_token(token: &str) -> String {
    token
        .trim_matches(|character: char| {
            matches!(
                character,
                '`' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | ':'
            )
        })
        .to_lowercase()
}

fn is_mdp_contract_token(token: &str) -> bool {
    token.starts_with("mdp.")
        && token.rsplit_once(".v").is_some_and(|(_, version)| {
            !version.is_empty() && version.chars().all(|character| character.is_ascii_digit())
        })
}

fn is_mdp_cli_command_token(token: &str) -> bool {
    matches!(
        token,
        "--json"
            | "--summary"
            | "brief"
            | "capabilities"
            | "check-claims"
            | "doctor"
            | "eval"
            | "explain"
            | "fit"
            | "gaps"
            | "init"
            | "render-brief"
            | "route"
            | "sample-leads"
            | "schemas"
            | "skills"
            | "validate"
            | "validate-prompt-output"
            | "verify-output"
    )
}

fn has_explicit_positioning_instruction(text: &str) -> bool {
    positioning_clauses(text).into_iter().any(|clause| {
        !has_positioning_negation(&clause)
            && [
                "position ",
                "positioning ",
                "sell ",
                "sold product",
                "pitch ",
                "market as",
                "prospect-facing",
                "customer-facing",
                "outbound copy",
            ]
            .iter()
            .any(|term| clause.contains(term))
    })
}

fn has_external_positioning_intent(text: &str) -> bool {
    has_explicit_positioning_instruction(text)
        || positioning_clauses(text).into_iter().any(|clause| {
            !has_positioning_negation(&clause)
                && [
                    " is a ",
                    " is the ",
                    " helps ",
                    " improves ",
                    " enables ",
                    " provides ",
                    " delivers ",
                ]
                .iter()
                .any(|term| clause.contains(term))
        })
}

fn has_positioning_negation(text: &str) -> bool {
    let text = text.to_lowercase();
    if [
        "do not avoid positioning",
        "don't avoid positioning",
        "must not avoid positioning",
        "never avoid positioning",
        "not avoid positioning",
        "do not reject positioning",
        "don't reject positioning",
        "must not reject positioning",
        "never reject positioning",
        "not reject positioning",
    ]
    .iter()
    .any(|term| text.contains(term))
    {
        return false;
    }
    [
        "do not position",
        "must not position",
        "never position",
        "not position",
        "instead of positioning",
        "reject positioning",
        "avoid positioning",
        "do not sell",
        "must not sell",
        "never sell",
        "not sold",
        "do not pitch",
        "must not pitch",
        "never pitch",
        "do not market",
        "must not market",
        "never market",
        " is not ",
    ]
    .iter()
    .any(|term| text.contains(term))
}

fn internal_term_is_only_negated(text: &str, internal: &str) -> bool {
    let mut found = false;
    for clause in positioning_clauses(text)
        .into_iter()
        .filter(|clause| contains_term(clause, internal))
    {
        found = true;
        if !has_positioning_negation(&clause) {
            return false;
        }
    }
    found
}

fn positioning_clauses(text: &str) -> Vec<String> {
    let mut text = text.to_lowercase();
    for adversative in [" but ", " however ", " instead ", " yet "] {
        text = text.replace(adversative, ".");
    }
    text.split(['.', ';', '\n'])
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .map(str::to_string)
        .collect()
}

fn matches_pointer_prefix(pointer: &str, prefixes: &[&str]) -> bool {
    prefixes
        .iter()
        .any(|prefix| pointer == *prefix || pointer.starts_with(&format!("{prefix}/")))
}

fn redact_active_target_identity(target: &TargetIdentity, text: &str) -> String {
    let mut identities = std::iter::once(target.name.as_str())
        .chain(target.aliases.iter().map(String::as_str))
        .filter(|identity| {
            internal_target_terms(target).any(|internal| contains_term(identity, internal))
        })
        .collect::<Vec<_>>();
    identities.sort_by_key(|identity| std::cmp::Reverse(identity.len()));

    identities
        .into_iter()
        .fold(text.to_string(), |redacted, identity| {
            redact_bounded_term(&redacted, identity)
        })
}

fn redact_bounded_term(text: &str, term: &str) -> String {
    let term = term.trim();
    if term.is_empty() {
        return text.to_string();
    }

    let mut redacted = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for (start, _) in text.char_indices() {
        if start < cursor {
            continue;
        }
        let end = start + term.len();
        if end > text.len()
            || !text.is_char_boundary(end)
            || !text[start..end].eq_ignore_ascii_case(term)
        {
            continue;
        }
        let before_ok = start == 0
            || !text[..start]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric);
        let after_ok = end == text.len()
            || !text[end..]
                .chars()
                .next()
                .is_some_and(char::is_alphanumeric);
        if before_ok && after_ok {
            redacted.push_str(&text[cursor..start]);
            redacted.push(' ');
            cursor = end;
        }
    }
    redacted.push_str(&text[cursor..]);
    redacted
}

fn internal_target_terms(target: &TargetIdentity) -> impl Iterator<Item = &str> {
    BUILTIN_INTERNAL_TARGET_TERMS
        .iter()
        .copied()
        .chain(target.internal_terms.iter().map(String::as_str))
}

fn contains_term(text: &str, term: &str) -> bool {
    let text = text.to_lowercase();
    let term = term.trim().to_lowercase();
    if term.is_empty() {
        return false;
    }
    let mut offset = 0usize;
    while let Some(relative) = text[offset..].find(&term) {
        let start = offset + relative;
        let end = start + term.len();
        let before_ok = start == 0
            || !text[..start]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric);
        let after_ok = end == text.len()
            || !text[end..]
                .chars()
                .next()
                .is_some_and(char::is_alphanumeric);
        if before_ok && after_ok {
            return true;
        }
        offset = end;
    }
    false
}

fn portfolio_eval_coverage(root: &Path) -> Result<(bool, bool)> {
    let eval_dir = root.join(DEFAULT_DIR).join("evals");
    if !eval_dir.exists() {
        return Ok((false, false));
    }
    let mut has_selected_scope = false;
    let mut has_missing_scope = false;
    for entry in fs::read_dir(eval_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("yaml") {
            continue;
        }
        let raw = fs::read_to_string(path)?;
        let Ok(value) = serde_yaml::from_str::<YamlValue>(&raw) else {
            continue;
        };
        let scope_selected = yaml_get(&value, "scope")
            .and_then(YamlValue::as_sequence)
            .is_some_and(|values| !values.is_empty());
        let command = yaml_get(&value, "command")
            .and_then(YamlValue::as_str)
            .unwrap_or("route");
        let has_inclusion = yaml_get(&value, "expect_entry_titles_contains")
            .and_then(YamlValue::as_sequence)
            .is_some_and(|values| !values.is_empty());
        let has_exclusion = yaml_get(&value, "expect_entry_titles_excludes")
            .and_then(YamlValue::as_sequence)
            .is_some_and(|values| !values.is_empty());
        let has_scope_gap = yaml_get(&value, "expect_entry_gap_reasons_contains")
            .and_then(YamlValue::as_sequence)
            .is_some_and(|values| {
                values.iter().any(|value| {
                    value.as_str().is_some_and(|reason| {
                        matches!(reason, "scope_dimension_missing" | "scope_value_mismatch")
                    })
                })
            })
            || yaml_get(&value, "expect_scope_issue_codes_contains")
                .and_then(YamlValue::as_sequence)
                .is_some_and(|values| !values.is_empty());
        has_selected_scope |=
            command == "route" && scope_selected && has_inclusion && has_exclusion;
        has_missing_scope |= command == "route"
            && !scope_selected
            && yaml_get(&value, "expect_draft_status")
                .and_then(YamlValue::as_str)
                .is_some_and(|status| matches!(status, "blocked" | "no-draft"))
            && has_scope_gap;
    }
    Ok((has_selected_scope, has_missing_scope))
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
            "target",
            "profile",
            "personas",
            "target_personas",
            "operator_roles",
            "supported_channels",
            "persona_mappings",
            "lead_input_requirements",
            "qualification_gates",
            "required_primitives",
            "primitive_map",
            "decision_input_contracts",
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
    let target = yaml_get(&value, "target").unwrap_or(&YamlValue::Null);
    validate_object_keys(
        target,
        &[
            "kind",
            "name",
            "aliases",
            "external_terms",
            "excluded_terms",
            "internal_terms",
            "source_ids",
        ],
        ".mdp/manifest.yaml#/target",
        "manifest_target_unknown_field",
        issues,
    );
    let profile = yaml_get(&value, "profile").unwrap_or(&YamlValue::Null);
    validate_object_keys(
        profile,
        &[
            "id",
            "label",
            "version",
            "context_dimensions",
            "context_dimension_dependencies",
            "product_foundation",
        ],
        ".mdp/manifest.yaml#/profile",
        "manifest_profile_unknown_field",
        issues,
    );
    validate_product_foundation_shapes(
        yaml_get(profile, "product_foundation"),
        ".mdp/manifest.yaml#/profile/product_foundation",
        issues,
    );
    validate_primitive_map_shape(
        yaml_get(&value, "primitive_map"),
        ".mdp/manifest.yaml#/primitive_map",
        issues,
    );
    validate_decision_input_contract_shapes(
        yaml_get(&value, "decision_input_contracts"),
        ".mdp/manifest.yaml#/decision_input_contracts",
        issues,
    );
    validate_sequence_object_keys(
        yaml_get(&value, "input_contracts"),
        &[
            "id",
            "description",
            "schema_ref",
            "prompt",
            "normalizes",
            "decision_input_contracts",
        ],
        ".mdp/manifest.yaml#/input_contracts",
        "manifest_input_contract_unknown_field",
        issues,
    );
    validate_sequence_object_keys(
        yaml_get(&value, "jobs"),
        &[
            "id",
            "skill_id",
            "label",
            "description",
            "required_primitives",
            "input_contracts",
            "decision_input_contracts",
            "product_foundation",
            "model_task",
            "context_budget",
        ],
        ".mdp/manifest.yaml#/jobs",
        "manifest_profile_job_unknown_field",
        issues,
    );
    validate_profile_job_product_foundation_shapes(yaml_get(&value, "jobs"), issues);
    if let Some(jobs) = yaml_get(&value, "jobs").and_then(YamlValue::as_sequence) {
        for (index, job) in jobs.iter().enumerate() {
            validate_object_keys(
                yaml_get(job, "model_task").unwrap_or(&YamlValue::Null),
                &["kind", "prompt"],
                &format!(".mdp/manifest.yaml#/jobs/{index}/model_task"),
                "manifest_profile_job_model_task_unknown_field",
                issues,
            );
            let budget = yaml_get(job, "context_budget").unwrap_or(&YamlValue::Null);
            validate_object_keys(
                budget,
                &["max_entries", "max_bytes"],
                &format!(".mdp/manifest.yaml#/jobs/{index}/context_budget"),
                "manifest_profile_job_context_budget_unknown_field",
                issues,
            );
            if !budget.is_null() {
                for field in ["max_entries", "max_bytes"] {
                    let valid = yaml_get(budget, field)
                        .and_then(YamlValue::as_u64)
                        .is_some_and(|value| value > 0);
                    if !valid {
                        issues.push(issue(
                            "profile_job_context_budget_limit_invalid",
                            "error",
                            format!(".mdp/manifest.yaml#/jobs/{index}/context_budget/{field}"),
                            format!("context_budget.{field} must be a positive integer"),
                        ));
                    }
                }
            }
        }
    }
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
        yaml_get(&value, "qualification_gates").unwrap_or(&YamlValue::Null),
        &["require_person_resolution", "signals", "fail_policy"],
        ".mdp/manifest.yaml#/qualification_gates",
        "manifest_qualification_gates_unknown_field",
        issues,
    );
    validate_object_keys(
        yaml_get(
            yaml_get(&value, "qualification_gates").unwrap_or(&YamlValue::Null),
            "signals",
        )
        .unwrap_or(&YamlValue::Null),
        &["min", "max", "require_fit_signal", "require_why_now_signal"],
        ".mdp/manifest.yaml#/qualification_gates/signals",
        "manifest_qualification_signal_gates_unknown_field",
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

fn validate_product_foundation_shapes(
    value: Option<&YamlValue>,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let Some(registry) = value else {
        return;
    };
    validate_object_keys_with_severity(
        registry,
        &["facets"],
        path,
        "manifest_product_foundation_unknown_field",
        "error",
        issues,
    );
    validate_required_object_keys(
        registry,
        &["facets"],
        path,
        "manifest_product_foundation_required_field_missing",
        issues,
    );
    let Some(facets) = yaml_get(registry, "facets").and_then(YamlValue::as_sequence) else {
        return;
    };
    for (facet_index, facet) in facets.iter().enumerate() {
        let facet_path = format!("{path}/facets/{facet_index}");
        validate_object_keys_with_severity(
            facet,
            &["id", "kind", "entries", "gaps", "conflicts_with"],
            &facet_path,
            "manifest_product_foundation_facet_unknown_field",
            "error",
            issues,
        );
        validate_required_object_keys(
            facet,
            &["id", "kind"],
            &facet_path,
            "manifest_product_foundation_facet_required_field_missing",
            issues,
        );
        for refs_key in ["entries", "gaps"] {
            let Some(refs) = yaml_get(facet, refs_key).and_then(YamlValue::as_sequence) else {
                continue;
            };
            for (ref_index, reference) in refs.iter().enumerate() {
                let ref_path = format!("{facet_path}/{refs_key}/{ref_index}");
                validate_object_keys_with_severity(
                    reference,
                    &["card_id", "entry_id"],
                    &ref_path,
                    "manifest_product_foundation_reference_unknown_field",
                    "error",
                    issues,
                );
                validate_required_object_keys(
                    reference,
                    &["card_id", "entry_id"],
                    &ref_path,
                    "manifest_product_foundation_reference_required_field_missing",
                    issues,
                );
            }
        }
    }
}

fn validate_profile_job_product_foundation_shapes(
    value: Option<&YamlValue>,
    issues: &mut Vec<Value>,
) {
    let Some(jobs) = value.and_then(YamlValue::as_sequence) else {
        return;
    };
    for (job_index, job) in jobs.iter().enumerate() {
        let Some(binding) = yaml_get(job, "product_foundation") else {
            continue;
        };
        let path = format!(".mdp/manifest.yaml#/jobs/{job_index}/product_foundation");
        validate_object_keys_with_severity(
            binding,
            &["required", "conditional", "optional", "excluded"],
            &path,
            "manifest_profile_job_product_foundation_unknown_field",
            "error",
            issues,
        );
        let Some(conditionals) = yaml_get(binding, "conditional").and_then(YamlValue::as_sequence)
        else {
            continue;
        };
        for (conditional_index, conditional) in conditionals.iter().enumerate() {
            let conditional_path = format!("{path}/conditional/{conditional_index}");
            validate_object_keys_with_severity(
                conditional,
                &["facet_id", "when"],
                &conditional_path,
                "manifest_product_foundation_conditional_unknown_field",
                "error",
                issues,
            );
            validate_required_object_keys(
                conditional,
                &["facet_id", "when"],
                &conditional_path,
                "manifest_product_foundation_conditional_required_field_missing",
                issues,
            );
            let Some(condition) = yaml_get(conditional, "when") else {
                continue;
            };
            validate_object_keys_with_severity(
                condition,
                &["fact", "equals"],
                &format!("{conditional_path}/when"),
                "manifest_product_foundation_condition_unknown_field",
                "error",
                issues,
            );
            validate_required_object_keys(
                condition,
                &["fact", "equals"],
                &format!("{conditional_path}/when"),
                "manifest_product_foundation_condition_required_field_missing",
                issues,
            );
        }
    }
}

fn validate_product_foundation(
    manifest: &Manifest,
    card_entry_index: &BTreeMap<String, (CardKind, BTreeSet<String>, BTreeSet<String>)>,
    issues: &mut Vec<Value>,
) {
    let registry = manifest
        .profile
        .as_ref()
        .and_then(|profile| profile.product_foundation.as_ref());
    let opted_in = manifest
        .jobs
        .iter()
        .any(|job| job.product_foundation.is_some());
    let mut facet_ids = BTreeSet::new();

    if let Some(registry) = registry {
        for (facet_index, facet) in registry.facets.iter().enumerate() {
            let path =
                format!(".mdp/manifest.yaml#/profile/product_foundation/facets/{facet_index}");
            if facet.id.trim().is_empty() || !valid_declared_identifier(&facet.id) {
                issues.push(issue(
                    "product_foundation_facet_id_invalid",
                    "error",
                    format!("{path}/id"),
                    "product foundation facet ids must use lowercase kebab-case",
                ));
            } else if !facet_ids.insert(facet.id.clone()) {
                issues.push(issue(
                    "product_foundation_facet_duplicate",
                    "error",
                    format!("{path}/id"),
                    format!("duplicate product foundation facet {}", facet.id),
                ));
            }
            if facet.kind == ProductFoundationFacetKind::Unknown {
                issues.push(issue(
                    "product_foundation_facet_kind_unknown",
                    "error",
                    format!("{path}/kind"),
                    "unknown product foundation facet kind",
                ));
            }
        }
    }

    if !opted_in {
        return;
    }

    let Some(registry) = registry else {
        issues.push(issue(
            "product_foundation_registry_missing",
            "error",
            ".mdp/manifest.yaml#/profile/product_foundation",
            "a job product foundation binding requires profile.product_foundation",
        ));
        for (job_index, job) in manifest.jobs.iter().enumerate() {
            if let Some(binding) = &job.product_foundation {
                validate_product_foundation_binding(
                    manifest, job_index, binding, &facet_ids, issues,
                );
            }
        }
        return;
    };

    for (facet_index, facet) in registry.facets.iter().enumerate() {
        let path = format!(".mdp/manifest.yaml#/profile/product_foundation/facets/{facet_index}");
        validate_product_foundation_entry_refs(
            &facet.entries,
            &format!("{path}/entries"),
            false,
            card_entry_index,
            issues,
        );
        validate_product_foundation_entry_refs(
            &facet.gaps,
            &format!("{path}/gaps"),
            true,
            card_entry_index,
            issues,
        );
        let mut conflicts = BTreeSet::new();
        for (conflict_index, conflict) in facet.conflicts_with.iter().enumerate() {
            let conflict_path = format!("{path}/conflicts_with/{conflict_index}");
            if conflict == &facet.id {
                issues.push(issue(
                    "product_foundation_conflict_self",
                    "error",
                    &conflict_path,
                    format!("facet {} cannot conflict with itself", facet.id),
                ));
            } else if !facet_ids.contains(conflict) {
                issues.push(issue(
                    "product_foundation_conflict_facet_missing",
                    "error",
                    &conflict_path,
                    format!("conflict references missing facet {conflict}"),
                ));
            }
            if !conflicts.insert(conflict) {
                issues.push(issue(
                    "product_foundation_conflict_duplicate",
                    "error",
                    conflict_path,
                    format!("duplicate conflict reference {conflict}"),
                ));
            }
        }
    }

    for (job_index, job) in manifest.jobs.iter().enumerate() {
        if let Some(binding) = &job.product_foundation {
            validate_product_foundation_binding(manifest, job_index, binding, &facet_ids, issues);
        }
    }
}

fn validate_product_foundation_entry_refs(
    refs: &[ProductFoundationEntryRef],
    path: &str,
    require_gap_card: bool,
    card_entry_index: &BTreeMap<String, (CardKind, BTreeSet<String>, BTreeSet<String>)>,
    issues: &mut Vec<Value>,
) {
    let mut seen = BTreeSet::new();
    for (index, reference) in refs.iter().enumerate() {
        let ref_path = format!("{path}/{index}");
        let key = (reference.card_id.as_str(), reference.entry_id.as_str());
        if !seen.insert(key) {
            issues.push(issue(
                "product_foundation_reference_duplicate",
                "error",
                &ref_path,
                format!(
                    "duplicate product foundation reference {}#{}",
                    reference.card_id, reference.entry_id
                ),
            ));
        }
        let Some((card_kind, entry_ids, duplicate_entry_ids)) =
            card_entry_index.get(&reference.card_id)
        else {
            issues.push(issue(
                "product_foundation_card_missing",
                "error",
                format!("{ref_path}/card_id"),
                format!(
                    "product foundation references missing card {}",
                    reference.card_id
                ),
            ));
            continue;
        };
        if require_gap_card && card_kind != &CardKind::Gaps {
            issues.push(issue(
                "product_foundation_gap_card_kind_invalid",
                "error",
                format!("{ref_path}/card_id"),
                format!(
                    "gap reference card {} must have kind gaps",
                    reference.card_id
                ),
            ));
        } else if !require_gap_card && card_kind == &CardKind::Gaps {
            issues.push(issue(
                "product_foundation_entry_card_kind_invalid",
                "error",
                format!("{ref_path}/card_id"),
                format!(
                    "authoritative entry reference card {} must not have kind gaps; declare unresolved authority under facet.gaps",
                    reference.card_id
                ),
            ));
        }
        if duplicate_entry_ids.contains(&reference.entry_id) {
            issues.push(issue(
                "product_foundation_entry_ambiguous",
                "error",
                format!("{ref_path}/entry_id"),
                format!(
                    "product foundation references ambiguous duplicate entry {}#{}",
                    reference.card_id, reference.entry_id
                ),
            ));
        }
        if !entry_ids.contains(&reference.entry_id) {
            issues.push(issue(
                if require_gap_card {
                    "product_foundation_gap_missing"
                } else {
                    "product_foundation_entry_missing"
                },
                "error",
                format!("{ref_path}/entry_id"),
                format!(
                    "product foundation references missing entry {}#{}",
                    reference.card_id, reference.entry_id
                ),
            ));
        }
    }
}

fn validate_product_foundation_binding(
    manifest: &Manifest,
    job_index: usize,
    binding: &ProductFoundationBinding,
    facet_ids: &BTreeSet<String>,
    issues: &mut Vec<Value>,
) {
    let path = format!(".mdp/manifest.yaml#/jobs/{job_index}/product_foundation");
    let mut classified = BTreeSet::new();
    for (class, refs) in [
        ("required", binding.required.as_slice()),
        ("optional", binding.optional.as_slice()),
        ("excluded", binding.excluded.as_slice()),
    ] {
        for (index, facet_id) in refs.iter().enumerate() {
            validate_product_foundation_classification(
                facet_id,
                &format!("{path}/{class}/{index}"),
                facet_ids,
                &mut classified,
                issues,
            );
        }
    }
    for (index, conditional) in binding.conditional.iter().enumerate() {
        let conditional_path = format!("{path}/conditional/{index}");
        validate_product_foundation_classification(
            &conditional.facet_id,
            &format!("{conditional_path}/facet_id"),
            facet_ids,
            &mut classified,
            issues,
        );
        if conditional.when.fact == ProductFoundationConditionFact::Unknown {
            issues.push(issue(
                "product_foundation_condition_fact_unknown",
                "error",
                format!("{conditional_path}/when/fact"),
                "conditional fact must be manifest_id, profile_id, or job_id",
            ));
        }
        if conditional.when.equals.trim().is_empty() {
            issues.push(issue(
                "product_foundation_condition_value_empty",
                "error",
                format!("{conditional_path}/when/equals"),
                "conditional equals value must not be empty",
            ));
        }
        if conditional.when.fact == ProductFoundationConditionFact::ProfileId
            && manifest.profile.is_none()
        {
            issues.push(issue(
                "product_foundation_condition_fact_unavailable",
                "error",
                format!("{conditional_path}/when/fact"),
                "profile_id is unavailable because the manifest has no profile",
            ));
        }
    }
}

fn validate_product_foundation_classification(
    facet_id: &str,
    path: &str,
    facet_ids: &BTreeSet<String>,
    classified: &mut BTreeSet<String>,
    issues: &mut Vec<Value>,
) {
    if !facet_ids.contains(facet_id) {
        issues.push(issue(
            "profile_job_product_foundation_facet_missing",
            "error",
            path,
            format!("job product foundation references missing facet {facet_id}"),
        ));
    }
    if !classified.insert(facet_id.to_string()) {
        issues.push(issue(
            "profile_job_product_foundation_facet_duplicate",
            "error",
            path,
            format!("facet {facet_id} is classified more than once for this job"),
        ));
    }
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
    for (dimension, values) in &profile.context_dimensions {
        if !valid_declared_identifier(dimension) {
            issues.push(issue(
                "profile_context_dimension_invalid",
                "error",
                format!(".mdp/manifest.yaml#/profile/context_dimensions/{dimension}"),
                "context dimension identifiers must use lowercase kebab-case",
            ));
        }
        if values.is_empty() {
            issues.push(issue(
                "profile_context_dimension_values_empty",
                "error",
                format!(".mdp/manifest.yaml#/profile/context_dimensions/{dimension}"),
                "context dimensions must declare at least one allowed value",
            ));
        }
        let mut seen = BTreeSet::new();
        for (index, value) in values.iter().enumerate() {
            let path =
                format!(".mdp/manifest.yaml#/profile/context_dimensions/{dimension}/{index}");
            if !valid_declared_identifier(value) {
                issues.push(issue(
                    "profile_context_dimension_value_invalid",
                    "error",
                    path,
                    "context dimension values must use lowercase kebab-case",
                ));
            } else if !seen.insert(value.to_ascii_lowercase()) {
                issues.push(issue(
                    "profile_context_dimension_value_duplicate",
                    "error",
                    path,
                    format!("duplicate context dimension value {value}"),
                ));
            }
        }
    }
    for (dimension, dependencies) in &profile.context_dimension_dependencies {
        let path =
            format!(".mdp/manifest.yaml#/profile/context_dimension_dependencies/{dimension}");
        if !profile.context_dimensions.contains_key(dimension) {
            issues.push(issue(
                "profile_context_dependency_dimension_unknown",
                "error",
                &path,
                format!("dependency source dimension {dimension} is not declared"),
            ));
        }
        let mut seen = BTreeSet::new();
        for (index, dependency) in dependencies.iter().enumerate() {
            if dependency == dimension || !profile.context_dimensions.contains_key(dependency) {
                issues.push(issue(
                    "profile_context_dependency_invalid",
                    "error",
                    format!("{path}/{index}"),
                    format!("dependency {dependency} must name another declared context dimension"),
                ));
            } else if !seen.insert(dependency.to_ascii_lowercase()) {
                issues.push(issue(
                    "profile_context_dependency_duplicate",
                    "error",
                    format!("{path}/{index}"),
                    format!("duplicate context dependency {dependency}"),
                ));
            }
        }
    }
}

fn validate_profile_mapping(
    manifest: &Manifest,
    card_ids: &BTreeSet<String>,
    prompt_inventory: &PromptInventory,
    eval_inventory: &EvalInventory,
    product_foundation_index: &ProductFoundationIndex,
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
            "activation_policy": "profile.id and jobs[].skill_id route agents, while activation requires required_primitives, primitive_map, input_contracts, jobs, and profile_eval coverage."
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
    let decision_input_contract_ids = manifest
        .decision_input_contracts
        .iter()
        .map(|contract| contract.id.clone())
        .collect::<BTreeSet<_>>();
    let input_contract_ids = validate_input_contracts(
        &manifest.input_contracts,
        &decision_input_contract_ids,
        prompt_inventory,
        ".mdp/manifest.yaml#/input_contracts",
        issues,
    );
    let job_ids = validate_profile_jobs(
        &manifest.jobs,
        manifest
            .profile
            .as_ref()
            .map(|profile| profile.id.as_str())
            .unwrap_or_default(),
        &known_primitives,
        &input_contract_ids,
        &decision_input_contract_ids,
        prompt_inventory,
        ".mdp/manifest.yaml#/jobs",
        issues,
    );
    validate_job_decision_input_composition(manifest, issues);
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

    let explicit_activation_blocks = manifest.profile_eval.blocks_activation();
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
        let mut product_foundation =
            resolve_product_foundation(manifest, product_foundation_index, &job.id);
        apply_validation_errors_for_job(&mut product_foundation, manifest, issues.as_slice());
        let model_task = model_task_summary(job, prompt_inventory);
        let activation_ready = missing_job_primitives.is_empty()
            && !explicit_activation_blocks
            && !product_foundation.blocks_activation()
            && model_task["status"] != "blocked";
        job_summaries.push(json!({
            "id": &job.id,
            "label": &job.label,
            "required_primitives": &job.required_primitives,
            "missing_required_primitives": missing_job_primitives,
            "product_foundation": resolution_json(&product_foundation),
            "model_task": model_task,
            "activation_ready": activation_ready
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
        && !explicit_activation_blocks
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
        "activation_policy": "Errors fail validation. Missing required primitive coverage and missing profile eval categories are warning-first by default, fail under --strict, and block profile activation. Explicit needs-review or blocked profile eval activation and blocked selected product foundation authority block job and profile activation."
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
    decision_input_contract_ids: &BTreeSet<String>,
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
                "error",
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
        validate_reference_list(
            &contract.decision_input_contracts,
            decision_input_contract_ids,
            &format!("{contract_path}/decision_input_contracts"),
            "profile_input_contract_decision_input_contract_missing",
            "decision input contract",
            issues,
        );
    }
    seen
}

fn validate_job_model_task(
    job: &ProfileJob,
    prompt_inventory: &PromptInventory,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let Some(binding) = job.model_task.as_ref() else {
        return;
    };
    if !matches!(binding.kind.as_str(), "generation" | "review") {
        issues.push(issue(
            "profile_job_model_task_kind_invalid",
            "error",
            format!("{path}/model_task/kind"),
            "job model_task kind must be generation or review",
        ));
    }
    if binding.prompt.trim().is_empty() {
        issues.push(issue(
            "profile_job_model_task_prompt_empty",
            "error",
            format!("{path}/model_task/prompt"),
            "job model_task prompt must not be empty",
        ));
        return;
    }
    let Some(prompt) = prompt_inventory.get(&binding.prompt) else {
        issues.push(issue(
            "profile_job_model_task_prompt_missing",
            "error",
            format!("{path}/model_task/prompt"),
            format!(
                "job {} references missing model-task prompt {}",
                job.id, binding.prompt
            ),
        ));
        return;
    };
    if prompt.format.as_deref() != Some(PROMPT_FORMAT_V1) {
        issues.push(issue(
            "profile_job_model_task_prompt_format_invalid",
            "error",
            format!("{path}/model_task/prompt"),
            format!(
                "job-owned model-task prompt {} must use {PROMPT_FORMAT_V1}",
                binding.prompt
            ),
        ));
    }
    if prompt.id.as_deref() != Some(binding.prompt.as_str()) {
        issues.push(issue(
            "profile_job_model_task_prompt_reference_invalid",
            "error",
            format!("{path}/model_task/prompt"),
            "job model_task must reference the canonical prompt id, not a file path or alias",
        ));
    }
    if prompt.kind.as_deref() != Some(binding.kind.as_str()) {
        issues.push(issue(
            "profile_job_model_task_kind_mismatch",
            "error",
            format!("{path}/model_task/kind"),
            format!(
                "job model_task kind {} does not match prompt kind",
                binding.kind
            ),
        ));
    }
    if prompt.output_kind.as_deref() != Some("governed-artifact") {
        issues.push(issue(
            "profile_job_model_task_output_kind_invalid",
            "error",
            format!("{path}/model_task/prompt"),
            "generation and review model-task prompts must use output_kind governed-artifact",
        ));
    }
    if prompt
        .version
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        issues.push(issue(
            "profile_job_model_task_prompt_version_missing",
            "error",
            format!("{path}/model_task/prompt"),
            "job-owned model-task prompt must declare a non-blank version",
        ));
    }
    if job.context_budget.is_some() && !prompt.required_inputs.contains("routed_context") {
        issues.push(issue(
            "profile_job_model_task_routed_context_input_missing",
            "error",
            format!("{path}/model_task/prompt"),
            "a context-budgeted model task prompt must declare routed_context as a required input",
        ));
    }
}

fn model_task_summary(job: &ProfileJob, prompt_inventory: &PromptInventory) -> Value {
    let Some(binding) = job.model_task.as_ref() else {
        return json!({
            "status": "unassessed",
            "reason": "job does not declare a pack-owned model task"
        });
    };
    let Some(prompt) = prompt_inventory.get(&binding.prompt) else {
        return json!({
            "status": "blocked",
            "kind": binding.kind,
            "prompt": binding.prompt,
            "reason": "declared prompt is missing"
        });
    };
    let ready = matches!(binding.kind.as_str(), "generation" | "review")
        && prompt.format.as_deref() == Some(PROMPT_FORMAT_V1)
        && prompt.id.as_deref() == Some(binding.prompt.as_str())
        && prompt.kind.as_deref() == Some(binding.kind.as_str())
        && prompt.output_kind.as_deref() == Some("governed-artifact")
        && prompt
            .version
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        && (job.context_budget.is_none() || prompt.required_inputs.contains("routed_context"));
    json!({
        "status": if ready { "ready" } else { "blocked" },
        "kind": binding.kind,
        "prompt": binding.prompt,
        "prompt_path": prompt.canonical_path,
        "prompt_version": prompt.version
    })
}

fn validate_profile_jobs(
    jobs: &[ProfileJob],
    profile_id: &str,
    known_primitives: &BTreeSet<&str>,
    input_contract_ids: &BTreeSet<String>,
    decision_input_contract_ids: &BTreeSet<String>,
    prompt_inventory: &PromptInventory,
    path: &str,
    issues: &mut Vec<Value>,
) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut model_task_prompt_owners = BTreeMap::new();
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
                "error",
                format!("{job_path}/id"),
                format!("duplicate profile job {}", job.id),
            ));
        }
        if job.skill_id.trim().is_empty() {
            issues.push(issue(
                "profile_job_skill_id_empty",
                "error",
                format!("{job_path}/skill_id"),
                "jobs entries must bind exactly one canonical skill_id",
            ));
        } else if !is_packaged_skill(&job.skill_id) {
            issues.push(issue(
                "profile_job_skill_unknown",
                "error",
                format!("{job_path}/skill_id"),
                format!("unknown canonical skill_id {}", job.skill_id),
            ));
        } else if let Some(route) = route_spec(profile_id, &job.id) {
            if route.skill_id != job.skill_id {
                issues.push(issue(
                    "profile_job_route_incompatible",
                    "error",
                    format!("{job_path}/skill_id"),
                    format!(
                        "{} profile job {} must bind {}",
                        profile_id, job.id, route.skill_id
                    ),
                ));
            }
        } else if JOB_ROUTE_SPECS.iter().any(|route| route.job_id == job.id) {
            issues.push(issue(
                "profile_job_route_incompatible",
                "error",
                format!("{job_path}/id"),
                format!("job {} is not valid for profile {}", job.id, profile_id),
            ));
        } else {
            issues.push(issue(
                "profile_job_route_unknown",
                "error",
                format!("{job_path}/id"),
                format!("job {} is not in the closed routing vocabulary", job.id),
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
        validate_job_model_task(job, prompt_inventory, &job_path, issues);
        if let Some(binding) = job.model_task.as_ref()
            && !binding.prompt.trim().is_empty()
            && let Some(first_job_id) =
                model_task_prompt_owners.insert(binding.prompt.clone(), job.id.clone())
        {
            issues.push(issue(
                "profile_job_model_task_prompt_reused",
                "error",
                format!("{job_path}/model_task/prompt"),
                format!(
                    "model-task prompt {} is already bound to job {}; each generation or review job must own one prompt",
                    binding.prompt, first_job_id
                ),
            ));
        }
        validate_reference_list(
            &job.decision_input_contracts,
            decision_input_contract_ids,
            &format!("{job_path}/decision_input_contracts"),
            "profile_job_decision_input_contract_missing",
            "decision input contract",
            issues,
        );
    }
    seen
}

fn validate_job_decision_input_composition(manifest: &Manifest, issues: &mut Vec<Value>) {
    let input_contracts = manifest
        .input_contracts
        .iter()
        .map(|contract| (contract.id.as_str(), contract))
        .collect::<BTreeMap<_, _>>();
    let decision_contracts = manifest
        .decision_input_contracts
        .iter()
        .map(|contract| (contract.id.as_str(), contract))
        .collect::<BTreeMap<_, _>>();

    for (job_index, job) in manifest.jobs.iter().enumerate() {
        let mut contract_ids = job
            .decision_input_contracts
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for input_contract_id in &job.input_contracts {
            if let Some(input_contract) = input_contracts.get(input_contract_id.as_str()) {
                contract_ids.extend(
                    input_contract
                        .decision_input_contracts
                        .iter()
                        .map(String::as_str),
                );
            }
        }

        let mut attribute_owners = BTreeMap::new();
        let mut output_path_owners = BTreeMap::new();
        let mut normalization_owner: Option<(&str, &str, &str)> = None;
        for contract_id in contract_ids {
            let Some(contract) = decision_contracts.get(contract_id) else {
                continue;
            };
            let prompt = contract.normalization.prompt.as_str();
            let prompt_version = contract.normalization.prompt_version.as_str();
            if let Some((first_contract, first_prompt, first_prompt_version)) = normalization_owner
            {
                if first_prompt != prompt || first_prompt_version != prompt_version {
                    issues.push(issue(
                        "decision_input_job_normalization_mismatch",
                        "error",
                        format!(".mdp/manifest.yaml#/jobs/{job_index}/decision_input_contracts"),
                        format!(
                            "job {} composes decision input contracts {} and {} with different normalization bindings ({}@{} vs {}@{})",
                            job.id,
                            first_contract,
                            contract_id,
                            first_prompt,
                            first_prompt_version,
                            prompt,
                            prompt_version
                        ),
                    ));
                }
            } else {
                normalization_owner = Some((contract_id, prompt, prompt_version));
            }
            for attribute in &contract.attributes {
                if let Some(first_contract) =
                    attribute_owners.insert(attribute.id.as_str(), contract_id)
                {
                    if first_contract != contract_id {
                        issues.push(issue(
                            "decision_input_job_attribute_duplicate",
                            "error",
                            format!(
                                ".mdp/manifest.yaml#/jobs/{job_index}/decision_input_contracts"
                            ),
                            format!(
                                "job {} composes decision input contracts {} and {} with duplicate attribute id {}",
                                job.id, first_contract, contract_id, attribute.id
                            ),
                        ));
                    }
                }
                if let Some(first_contract) =
                    output_path_owners.insert(attribute.output_path.as_str(), contract_id)
                {
                    if first_contract != contract_id {
                        issues.push(issue(
                            "decision_input_job_output_path_duplicate",
                            "error",
                            format!(
                                ".mdp/manifest.yaml#/jobs/{job_index}/decision_input_contracts"
                            ),
                            format!(
                                "job {} composes decision input contracts {} and {} with duplicate output path {}",
                                job.id, first_contract, contract_id, attribute.output_path
                            ),
                        ));
                    }
                }
            }
        }
    }
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

fn validate_decision_input_contracts(
    manifest: &Manifest,
    prompt_inventory: &PromptInventory,
    issues: &mut Vec<Value>,
) {
    let mut contract_ids = BTreeSet::new();
    for (contract_index, contract) in manifest.decision_input_contracts.iter().enumerate() {
        let path = format!(".mdp/manifest.yaml#/decision_input_contracts/{contract_index}");
        if contract.id.trim().is_empty() {
            issues.push(issue(
                "decision_input_contract_id_empty",
                "error",
                format!("{path}/id"),
                "decision input contracts must name a stable id",
            ));
        } else if !contract_ids.insert(contract.id.clone()) {
            issues.push(issue(
                "decision_input_contract_duplicate",
                "error",
                format!("{path}/id"),
                format!("duplicate decision input contract {}", contract.id),
            ));
        }
        if contract.version.trim().is_empty() {
            issues.push(issue(
                "decision_input_contract_version_empty",
                "error",
                format!("{path}/version"),
                "decision input contract version must not be empty",
            ));
        }
        if contract.normalization.prompt.trim().is_empty() {
            issues.push(issue(
                "decision_input_normalization_prompt_empty",
                "error",
                format!("{path}/normalization/prompt"),
                "decision input normalization must reference a prompt",
            ));
        } else if !prompt_inventory.contains(&contract.normalization.prompt) {
            issues.push(issue(
                "decision_input_normalization_prompt_missing",
                "error",
                format!("{path}/normalization/prompt"),
                format!(
                    "decision input contract {} references missing prompt {}",
                    contract.id, contract.normalization.prompt
                ),
            ));
        } else if let Some(prompt) = prompt_inventory.get(&contract.normalization.prompt) {
            if prompt.canonical_path.as_deref() != Some(contract.normalization.prompt.as_str()) {
                issues.push(issue(
                    "decision_input_normalization_prompt_path_required",
                    "error",
                    format!("{path}/normalization/prompt"),
                    format!(
                        "decision input contract {} must bind the canonical pack-relative prompt path {}; prompt ids are not runtime-resolvable bindings",
                        contract.id,
                        prompt.canonical_path.as_deref().unwrap_or("<missing>")
                    ),
                ));
            }
            let expected_normalized_contract = if contract.signal_projections.is_empty() {
                NORMALIZED_DECISION_INPUT_CONTRACT
            } else {
                NORMALIZED_DECISION_INPUT_CONTRACT_V2
            };
            if prompt.contract.as_deref() != Some(expected_normalized_contract)
                || prompt.output_kind.as_deref() != Some("decision-input-normalization")
                || prompt.schema_ref.as_deref() != Some(expected_normalized_contract)
            {
                issues.push(issue(
                    "decision_input_normalization_prompt_contract_mismatch",
                    "error",
                    format!("{path}/normalization/prompt"),
                    format!(
                        "decision input contract {} must bind a decision-input-normalization prompt whose contract and schema_ref are {}",
                        contract.id, expected_normalized_contract
                    ),
                ));
            }
            if prompt.version.as_deref() != Some(contract.normalization.prompt_version.as_str()) {
                issues.push(issue(
                    "decision_input_normalization_prompt_version_mismatch",
                    "error",
                    format!("{path}/normalization/prompt_version"),
                    format!(
                        "decision input contract {} prompt_version {} must match the bound prompt version {}",
                        contract.id,
                        contract.normalization.prompt_version,
                        prompt.version.as_deref().unwrap_or("<missing>")
                    ),
                ));
            }
        }
        if contract.normalization.prompt_version.trim().is_empty() {
            issues.push(issue(
                "decision_input_normalization_prompt_version_empty",
                "error",
                format!("{path}/normalization/prompt_version"),
                "decision input normalization must declare a prompt version",
            ));
        }
        let expected_normalized_contract = if contract.signal_projections.is_empty() {
            NORMALIZED_DECISION_INPUT_CONTRACT
        } else {
            NORMALIZED_DECISION_INPUT_CONTRACT_V2
        };
        if contract.normalization.normalized_schema_ref != expected_normalized_contract {
            issues.push(issue(
                "decision_input_normalized_schema_unknown",
                "error",
                format!("{path}/normalization/normalized_schema_ref"),
                format!("normalized_schema_ref must be {expected_normalized_contract}"),
            ));
        }
        if contract.source_classes.is_empty() {
            issues.push(issue(
                "decision_input_source_classes_empty",
                "error",
                format!("{path}/source_classes"),
                "decision input contracts must declare at least one permitted source class",
            ));
        }
        let declared_sources = contract
            .source_classes
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if declared_sources.len() != contract.source_classes.len() {
            issues.push(issue(
                "decision_input_source_class_duplicate",
                "error",
                format!("{path}/source_classes"),
                "decision input contract source classes must be unique",
            ));
        }
        validate_decision_input_attributes(manifest, contract, &declared_sources, &path, issues);
        validate_decision_input_signal_projections(contract, &path, issues);
    }
}

fn validate_decision_input_signal_projections(
    contract: &DecisionInputContract,
    path: &str,
    issues: &mut Vec<Value>,
) {
    if contract.signal_projections.len() > MAX_SIGNAL_PROJECTIONS_PER_CONTRACT {
        issues.push(issue(
            "decision_input_signal_projection_limit_exceeded",
            "error",
            format!("{path}/signal_projections"),
            format!(
                "decision input contracts may declare at most {MAX_SIGNAL_PROJECTIONS_PER_CONTRACT} signal projections"
            ),
        ));
    }

    let mut seen_ids = BTreeSet::new();
    for (projection_index, projection) in contract.signal_projections.iter().enumerate() {
        let projection_path = format!("{path}/signal_projections/{projection_index}");
        if !valid_attribute_key(&projection.id) || projection.id.len() > MAX_SIGNAL_IDENTIFIER_LEN {
            issues.push(issue(
                "decision_input_signal_projection_id_invalid",
                "error",
                format!("{projection_path}/id"),
                "signal projection ids must use the bounded manifest identifier format",
            ));
        } else if !seen_ids.insert(projection.id.to_ascii_lowercase()) {
            issues.push(issue(
                "decision_input_signal_projection_duplicate",
                "error",
                format!("{projection_path}/id"),
                format!(
                    "duplicate qualified signal projection {}",
                    projection.qualified_id(&contract.id)
                ),
            ));
        }

        if !valid_attribute_key(&projection.kind) || projection.kind.len() > MAX_SIGNAL_KIND_LEN {
            issues.push(issue(
                "decision_input_signal_kind_invalid",
                "error",
                format!("{projection_path}/kind"),
                "signal kinds are profile-defined but must use the bounded manifest identifier format",
            ));
        }
        if projection.roles.iter().collect::<BTreeSet<_>>().len() != projection.roles.len() {
            issues.push(issue(
                "decision_input_signal_role_duplicate",
                "error",
                format!("{projection_path}/roles"),
                "signal projection roles must be unique",
            ));
        }

        if projection.contributor_attribute_ids.is_empty() {
            issues.push(issue(
                "decision_input_signal_contributors_empty",
                "error",
                format!("{projection_path}/contributor_attribute_ids"),
                "signal projections require at least one contributing scalar attribute",
            ));
        } else if projection.contributor_attribute_ids.len() > MAX_SIGNAL_CONTRIBUTORS {
            issues.push(issue(
                "decision_input_signal_contributor_limit_exceeded",
                "error",
                format!("{projection_path}/contributor_attribute_ids"),
                format!(
                    "signal projections may declare at most {MAX_SIGNAL_CONTRIBUTORS} contributing attributes"
                ),
            ));
        }
        let mut seen_contributors = BTreeSet::new();
        for (contributor_index, contributor) in
            projection.contributor_attribute_ids.iter().enumerate()
        {
            if !seen_contributors.insert(contributor.to_ascii_lowercase()) {
                issues.push(issue(
                    "decision_input_signal_contributor_duplicate",
                    "error",
                    format!("{projection_path}/contributor_attribute_ids/{contributor_index}"),
                    format!("duplicate contributing attribute {contributor}"),
                ));
            }
            let matches = contract
                .attributes
                .iter()
                .filter(|attribute| attribute.id == *contributor)
                .count();
            let (code, message) = match matches {
                0 => (
                    "decision_input_signal_contributor_undeclared",
                    format!(
                        "contributing attribute {contributor} must be declared by the same decision input contract"
                    ),
                ),
                1 => continue,
                _ => (
                    "decision_input_signal_contributor_ambiguous",
                    format!(
                        "contributing attribute {contributor} is ambiguous because its declaration is duplicated"
                    ),
                ),
            };
            issues.push(issue(
                code,
                "error",
                format!("{projection_path}/contributor_attribute_ids/{contributor_index}"),
                message,
            ));
        }

        validate_value_contract(
            &projection.value,
            &format!("{projection_path}/value"),
            issues,
        );
        if projection.cardinality.max == 0
            || projection.cardinality.min > projection.cardinality.max
            || projection.cardinality.max > MAX_SIGNAL_OBSERVATIONS_PER_ENVELOPE
        {
            issues.push(issue(
                "decision_input_signal_cardinality_invalid",
                "error",
                format!("{projection_path}/cardinality"),
                format!(
                    "signal cardinality must satisfy 0 <= min <= max <= {MAX_SIGNAL_OBSERVATIONS_PER_ENVELOPE}"
                ),
            ));
        }
        if projection.decision_effects.is_empty() {
            issues.push(issue(
                "decision_input_signal_decision_effects_empty",
                "error",
                format!("{projection_path}/decision_effects"),
                "signal projections must declare at least one deterministic decision effect",
            ));
        } else if projection
            .decision_effects
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != projection.decision_effects.len()
        {
            issues.push(issue(
                "decision_input_signal_decision_effect_duplicate",
                "error",
                format!("{projection_path}/decision_effects"),
                "signal projection decision effects must be unique",
            ));
        }
        if projection.conflict_policy
            == crate::models::DecisionInputSignalConflictPolicy::AnyDisqualifies
            && !projection
                .roles
                .contains(&crate::models::DecisionInputSignalRole::Disqualifier)
        {
            issues.push(issue(
                "decision_input_signal_conflict_policy_role_mismatch",
                "error",
                format!("{projection_path}/conflict_policy"),
                "any-disqualifies requires the closed disqualifier role",
            ));
        }
    }
}

fn validate_decision_input_attributes(
    manifest: &Manifest,
    contract: &DecisionInputContract,
    declared_sources: &BTreeSet<crate::models::DecisionInputSourceClass>,
    path: &str,
    issues: &mut Vec<Value>,
) {
    if contract.attributes.is_empty() {
        issues.push(issue(
            "decision_input_attributes_empty",
            "error",
            format!("{path}/attributes"),
            "decision input contracts must declare at least one attribute",
        ));
        return;
    }
    let attribute_ids = contract
        .attributes
        .iter()
        .map(|attribute| attribute.id.as_str())
        .collect::<BTreeSet<_>>();
    let attribute_value_types = contract
        .attributes
        .iter()
        .map(|attribute| {
            (
                attribute.id.as_str(),
                attribute.value.value_type.as_deref().unwrap_or("string"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let attribute_requirements = contract
        .attributes
        .iter()
        .map(|attribute| (attribute.id.as_str(), &attribute.requirement))
        .collect::<BTreeMap<_, _>>();
    let attribute_value_enums = contract
        .attributes
        .iter()
        .map(|attribute| {
            (
                attribute.id.as_str(),
                attribute
                    .value
                    .enum_values
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let required_attributes = manifest
        .lead_input_requirements
        .required_attributes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let required_fields = manifest
        .lead_input_requirements
        .required_fields
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut seen_ids = BTreeSet::new();
    let mut seen_paths = BTreeSet::new();
    for (attribute_index, attribute) in contract.attributes.iter().enumerate() {
        let attribute_path = format!("{path}/attributes/{attribute_index}");
        if !valid_attribute_key(&attribute.id) {
            issues.push(issue(
                "decision_input_attribute_id_invalid",
                "error",
                format!("{attribute_path}/id"),
                "decision input attribute ids must use the manifest attribute identifier format",
            ));
        } else if !seen_ids.insert(attribute.id.to_ascii_lowercase()) {
            issues.push(issue(
                "decision_input_attribute_duplicate",
                "error",
                format!("{attribute_path}/id"),
                format!("duplicate decision input attribute {}", attribute.id),
            ));
        }
        if attribute.question.trim().is_empty() {
            issues.push(issue(
                "decision_input_attribute_question_empty",
                "error",
                format!("{attribute_path}/question"),
                "decision input attributes must state the data question they answer",
            ));
        }
        if !valid_decision_input_output_path(&attribute.output_path) {
            issues.push(issue(
                "decision_input_output_path_invalid",
                "error",
                format!("{attribute_path}/output_path"),
                format!(
                    "unsupported normalized prospect output path {}",
                    attribute.output_path
                ),
            ));
        } else if !seen_paths.insert(attribute.output_path.clone()) {
            issues.push(issue(
                "decision_input_output_path_duplicate",
                "error",
                format!("{attribute_path}/output_path"),
                format!(
                    "multiple decision input attributes map to {}",
                    attribute.output_path
                ),
            ));
        } else {
            validate_decision_input_readiness_alignment(
                manifest,
                attribute,
                &required_attributes,
                &required_fields,
                &attribute_path,
                issues,
            );
        }
        validate_value_contract(&attribute.value, &format!("{attribute_path}/value"), issues);
        if attribute.requirement == DecisionInputRequirement::Conditional
            && attribute.applies_when.is_empty()
        {
            issues.push(issue(
                "decision_input_conditional_missing_applicability",
                "error",
                format!("{attribute_path}/applies_when"),
                "conditional decision input attributes must declare applies_when",
            ));
        }
        for (condition_index, condition) in attribute.applies_when.iter().enumerate() {
            let condition_path = format!("{attribute_path}/applies_when/{condition_index}");
            if condition.attribute == attribute.id
                || !attribute_ids.contains(condition.attribute.as_str())
            {
                issues.push(issue(
                    "decision_input_applicability_dependency_invalid",
                    "error",
                    format!("{condition_path}/attribute"),
                    format!(
                        "applicability dependency {} must name another attribute in the same contract",
                        condition.attribute
                    ),
                ));
            }
            if condition.operator != crate::models::DecisionInputConditionOperator::Exists
                && condition.values.is_empty()
            {
                issues.push(issue(
                    "decision_input_applicability_values_empty",
                    "error",
                    format!("{condition_path}/values"),
                    "equals, not_equals, and in applicability conditions require values",
                ));
            }
            if condition.operator == crate::models::DecisionInputConditionOperator::Exists
                && !condition.values.is_empty()
            {
                issues.push(issue(
                    "decision_input_applicability_exists_values_forbidden",
                    "error",
                    format!("{condition_path}/values"),
                    "exists applicability conditions do not accept values",
                ));
            }
            if condition.operator == crate::models::DecisionInputConditionOperator::Equals
                && condition.values.len() != 1
            {
                issues.push(issue(
                    "decision_input_applicability_equals_cardinality",
                    "error",
                    format!("{condition_path}/values"),
                    "equals applicability conditions require exactly one value; use in for multiple values",
                ));
            }
            if condition.operator != crate::models::DecisionInputConditionOperator::Exists
                && attribute_value_types
                    .get(condition.attribute.as_str())
                    .is_some_and(|value_type| *value_type != "string")
            {
                issues.push(issue(
                    "decision_input_applicability_operand_type_unsupported",
                    "error",
                    format!("{condition_path}/operator"),
                    format!(
                        "{} conditions currently require a string dependency; use exists for typed dependencies",
                        serde_json::to_value(&condition.operator)
                            .ok()
                            .and_then(|value| value.as_str().map(str::to_string))
                            .unwrap_or_else(|| "comparison".to_string())
                    ),
                ));
            }
            if condition.operator != crate::models::DecisionInputConditionOperator::Exists {
                if let Some(domain) = attribute_value_enums
                    .get(condition.attribute.as_str())
                    .filter(|domain| !domain.is_empty())
                {
                    for (value_index, value) in condition.values.iter().enumerate() {
                        if !domain.contains(value.as_str()) {
                            issues.push(issue(
                                "decision_input_applicability_value_out_of_domain",
                                "error",
                                format!("{condition_path}/values/{value_index}"),
                                format!(
                                    "applicability value {value} is not declared by dependency attribute {}",
                                    condition.attribute
                                ),
                            ));
                        }
                    }
                }
            }
            if attribute_requirements
                .get(condition.attribute.as_str())
                .is_some_and(|requirement| {
                    !matches!(
                        requirement,
                        DecisionInputRequirement::Required | DecisionInputRequirement::HardGate
                    )
                })
            {
                issues.push(issue(
                    "decision_input_applicability_dependency_not_readiness_required",
                    "error",
                    format!("{condition_path}/attribute"),
                    "applicability dependencies must be required or hard-gate attributes so unresolved dependency states cannot certify readiness",
                ));
            }
        }
        if attribute.decision_effects.is_empty() {
            issues.push(issue(
                "decision_input_decision_effects_empty",
                "error",
                format!("{attribute_path}/decision_effects"),
                "decision input attributes must declare at least one deterministic decision effect",
            ));
        } else if attribute
            .decision_effects
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != attribute.decision_effects.len()
        {
            issues.push(issue(
                "decision_input_decision_effect_duplicate",
                "error",
                format!("{attribute_path}/decision_effects"),
                "decision effects must be unique",
            ));
        }
        if attribute.source_classes.is_empty() {
            issues.push(issue(
                "decision_input_attribute_source_classes_empty",
                "error",
                format!("{attribute_path}/source_classes"),
                "decision input attributes must declare permitted source classes",
            ));
        } else if attribute
            .source_classes
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != attribute.source_classes.len()
        {
            issues.push(issue(
                "decision_input_attribute_source_class_duplicate",
                "error",
                format!("{attribute_path}/source_classes"),
                "decision input attribute source classes must be unique",
            ));
        }
        for (source_index, source_class) in attribute.source_classes.iter().enumerate() {
            if !declared_sources.contains(source_class) {
                issues.push(issue(
                    "decision_input_attribute_source_class_undeclared",
                    "error",
                    format!("{attribute_path}/source_classes/{source_index}"),
                    "attribute source class must be declared by its decision input contract",
                ));
            }
        }
        if attribute.provenance.required && attribute.provenance.required_fields.is_empty() {
            issues.push(issue(
                "decision_input_provenance_fields_empty",
                "error",
                format!("{attribute_path}/provenance/required_fields"),
                "required provenance must declare the fields a normalizer must preserve",
            ));
        }
        if attribute.provenance.required
            && !attribute
                .provenance
                .required_fields
                .contains(&crate::models::DecisionInputProvenanceField::AttemptId)
        {
            issues.push(issue(
                "decision_input_provenance_attempt_id_required",
                "error",
                format!("{attribute_path}/provenance/required_fields"),
                "required provenance must include attempt_id so evidence binds to the exact source-attempt request",
            ));
        }
        if attribute.freshness.required && !attribute.provenance.required {
            issues.push(issue(
                "decision_input_freshness_provenance_timestamp_required",
                "error",
                format!("{attribute_path}/provenance/required_fields"),
                "required freshness must bind to required provenance",
            ));
        } else if attribute.freshness.required
            && !attribute
                .provenance
                .required_fields
                .contains(&crate::models::DecisionInputProvenanceField::ObservedAt)
        {
            issues.push(issue(
                "decision_input_freshness_provenance_timestamp_required",
                "error",
                format!("{attribute_path}/provenance/required_fields"),
                "required freshness must bind to required provenance observed_at timestamps",
            ));
        }
        if attribute
            .confidence
            .minimum
            .is_some_and(|minimum| minimum > 100)
        {
            issues.push(issue(
                "decision_input_confidence_minimum_invalid",
                "error",
                format!("{attribute_path}/confidence/minimum"),
                "confidence minimum must be from 0 through 100",
            ));
        }
        if attribute.requirement == DecisionInputRequirement::HardGate {
            validate_hard_gate_status_policy(attribute, &attribute_path, issues);
        } else {
            validate_readiness_status_policy(attribute, &attribute_path, issues);
        }
    }
    validate_decision_input_applicability_cycles(contract, path, issues);
}

fn validate_decision_input_applicability_cycles(
    contract: &DecisionInputContract,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let attribute_indices = contract
        .attributes
        .iter()
        .enumerate()
        .map(|(index, attribute)| (attribute.id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let dependencies = contract
        .attributes
        .iter()
        .map(|attribute| {
            let valid_dependencies = attribute
                .applies_when
                .iter()
                .filter_map(|condition| {
                    attribute_indices
                        .contains_key(condition.attribute.as_str())
                        .then_some(condition.attribute.as_str())
                })
                .collect::<Vec<_>>();
            (attribute.id.as_str(), valid_dependencies)
        })
        .collect::<BTreeMap<_, _>>();
    let mut visited = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    let mut stack = Vec::new();
    let mut reported = BTreeSet::<String>::new();
    for attribute in &contract.attributes {
        detect_decision_input_applicability_cycle(
            attribute.id.as_str(),
            &dependencies,
            &attribute_indices,
            path,
            &mut visited,
            &mut visiting,
            &mut stack,
            &mut reported,
            issues,
        );
    }
}

fn detect_decision_input_applicability_cycle<'a>(
    attribute_id: &'a str,
    dependencies: &BTreeMap<&'a str, Vec<&'a str>>,
    attribute_indices: &BTreeMap<&'a str, usize>,
    path: &str,
    visited: &mut BTreeSet<&'a str>,
    visiting: &mut BTreeSet<&'a str>,
    stack: &mut Vec<&'a str>,
    reported: &mut BTreeSet<String>,
    issues: &mut Vec<Value>,
) {
    if visited.contains(attribute_id) {
        return;
    }
    if visiting.contains(attribute_id) {
        if let Some(start) = stack
            .iter()
            .position(|candidate| *candidate == attribute_id)
        {
            let cycle = stack[start..].to_vec();
            let cycle_key = cycle.join(" -> ");
            if reported.insert(cycle_key) {
                let issue_attribute = cycle.first().copied().unwrap_or(attribute_id);
                let issue_index = attribute_indices
                    .get(issue_attribute)
                    .copied()
                    .unwrap_or_default();
                let mut cycle_path = cycle.clone();
                cycle_path.push(issue_attribute);
                issues.push(issue(
                    "decision_input_applicability_cycle",
                    "error",
                    format!("{path}/attributes/{issue_index}/applies_when"),
                    format!(
                        "decision input applicability dependencies must be acyclic; found {}",
                        cycle_path.join(" -> ")
                    ),
                ));
            }
        }
        return;
    }
    visiting.insert(attribute_id);
    stack.push(attribute_id);
    if let Some(next_attributes) = dependencies.get(attribute_id) {
        for next_attribute in next_attributes {
            detect_decision_input_applicability_cycle(
                next_attribute,
                dependencies,
                attribute_indices,
                path,
                visited,
                visiting,
                stack,
                reported,
                issues,
            );
        }
    }
    stack.pop();
    visiting.remove(attribute_id);
    visited.insert(attribute_id);
}

fn validate_hard_gate_status_policy(
    attribute: &crate::models::DecisionInputAttribute,
    path: &str,
    issues: &mut Vec<Value>,
) {
    for status in DecisionInputAttemptStatus::ALL {
        let disposition = attribute.status_behavior.get(&status);
        if disposition.is_none() {
            issues.push(issue(
                "decision_input_hard_gate_status_behavior_missing",
                "error",
                format!("{path}/status_behavior"),
                format!(
                    "hard-gate attribute {} must declare behavior for every attempt status, including {:?}",
                    attribute.id, status
                ),
            ));
            continue;
        }
        let disposition = disposition.expect("checked above");
        let unsafe_non_observed = status != DecisionInputAttemptStatus::Observed
            && matches!(
                disposition,
                DecisionInputDisposition::Accept | DecisionInputDisposition::Evaluate
            );
        let unsafe_provider_failure = matches!(
            status,
            DecisionInputAttemptStatus::Blocked | DecisionInputAttemptStatus::Error
        ) && !matches!(
            disposition,
            DecisionInputDisposition::Block | DecisionInputDisposition::HumanReview
        );
        if unsafe_non_observed || unsafe_provider_failure {
            issues.push(issue(
                "decision_input_hard_gate_status_behavior_unsafe",
                "error",
                format!("{path}/status_behavior"),
                format!(
                    "hard-gate attribute {} maps {:?} to {:?}; unresolved hard-gate attempts must fail closed",
                    attribute.id, status, disposition
                ),
            ));
        }
    }
    if !attribute
        .decision_effects
        .contains(&DecisionInputDecisionEffect::NoDraft)
    {
        issues.push(issue(
            "decision_input_hard_gate_no_draft_missing",
            "error",
            format!("{path}/decision_effects"),
            "hard-gate attributes must include the no-draft decision effect",
        ));
    }
}

fn validate_readiness_status_policy(
    attribute: &crate::models::DecisionInputAttribute,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let must_fail_closed = match attribute.requirement {
        DecisionInputRequirement::Required => vec![
            DecisionInputAttemptStatus::NotFound,
            DecisionInputAttemptStatus::NotApplicable,
            DecisionInputAttemptStatus::Blocked,
            DecisionInputAttemptStatus::Error,
        ],
        DecisionInputRequirement::Conditional => vec![
            DecisionInputAttemptStatus::NotFound,
            DecisionInputAttemptStatus::Blocked,
            DecisionInputAttemptStatus::Error,
        ],
        DecisionInputRequirement::Optional => vec![
            DecisionInputAttemptStatus::Blocked,
            DecisionInputAttemptStatus::Error,
        ],
        DecisionInputRequirement::HardGate => Vec::new(),
    };
    for status in must_fail_closed {
        let Some(disposition) = attribute.status_behavior.get(&status) else {
            continue;
        };
        let permits_ready = matches!(
            disposition,
            DecisionInputDisposition::Accept | DecisionInputDisposition::Evaluate
        ) || (attribute.requirement == DecisionInputRequirement::Optional
            && *disposition == DecisionInputDisposition::Gap);
        if permits_ready {
            issues.push(issue(
                "decision_input_status_behavior_unsafe",
                "error",
                format!("{path}/status_behavior"),
                format!(
                    "{:?} attribute {} maps {:?} to {:?}; missing or failed required evidence must not certify readiness",
                    attribute.requirement, attribute.id, status, disposition
                ),
            ));
        }
    }
}

fn validate_decision_input_readiness_alignment(
    manifest: &Manifest,
    attribute: &crate::models::DecisionInputAttribute,
    required_attributes: &BTreeSet<&str>,
    required_fields: &BTreeSet<&str>,
    path: &str,
    issues: &mut Vec<Value>,
) {
    for mismatch in decision_input_readiness_mismatches(
        manifest,
        attribute,
        required_attributes,
        required_fields,
    ) {
        issues.push(issue(
            mismatch.code,
            "error",
            format!("{path}/{}", mismatch.field),
            mismatch.message,
        ));
    }
}

struct DecisionInputReadinessMismatch {
    code: &'static str,
    field: &'static str,
    message: String,
}

fn decision_input_readiness_mismatches(
    manifest: &Manifest,
    attribute: &crate::models::DecisionInputAttribute,
    required_attributes: &BTreeSet<&str>,
    required_fields: &BTreeSet<&str>,
) -> Vec<DecisionInputReadinessMismatch> {
    let mut mismatches = Vec::new();
    let readiness_required = matches!(
        attribute.requirement,
        DecisionInputRequirement::Required | DecisionInputRequirement::HardGate
    );
    if let Some(attribute_name) = attribute.output_path.strip_prefix("attributes.") {
        let definition = manifest
            .lead_input_requirements
            .attribute_definitions
            .get(attribute_name);
        if definition.is_none() {
            mismatches.push(DecisionInputReadinessMismatch {
                code: "decision_input_attribute_definition_missing",
                field: "output_path",
                message: format!(
                    "{} must be declared in lead_input_requirements.attribute_definitions",
                    attribute.output_path
                ),
            });
        } else if definition.is_some_and(|definition| {
            !value_contract_constraints_match(definition, &attribute.value)
        }) {
            mismatches.push(DecisionInputReadinessMismatch {
                code: "decision_input_value_contract_mismatch",
                field: "value",
                message: format!(
                    "{} value contract must match lead_input_requirements.attribute_definitions.{attribute_name}",
                    attribute.output_path
                ),
            });
        }
        if readiness_required && !required_attributes.contains(attribute_name) {
            mismatches.push(DecisionInputReadinessMismatch {
                code: "decision_input_readiness_requirement_missing",
                field: "output_path",
                message: format!(
                    "{} is {} but is not listed in lead_input_requirements.required_attributes",
                    attribute.output_path,
                    if attribute.requirement == DecisionInputRequirement::HardGate {
                        "a hard gate"
                    } else {
                        "required"
                    }
                ),
            });
        } else if !readiness_required && required_attributes.contains(attribute_name) {
            mismatches.push(DecisionInputReadinessMismatch {
                code: "decision_input_readiness_requirement_conflict",
                field: "requirement",
                message: format!(
                    "{} is {} in the decision input contract but required by lead_input_requirements.required_attributes",
                    attribute.output_path,
                    if attribute.requirement == DecisionInputRequirement::Conditional {
                        "conditional"
                    } else {
                        "optional"
                    }
                ),
            });
        }
    } else {
        let actual_type = attribute.value.value_type.as_deref().unwrap_or("string");
        let expected_type = if attribute.output_path == "synthetic" {
            "boolean"
        } else {
            "string"
        };
        if actual_type != expected_type {
            mismatches.push(DecisionInputReadinessMismatch {
                code: "decision_input_prospect_output_type_mismatch",
                field: "value",
                message: format!(
                    "{} requires a {expected_type} decision-input value, found {actual_type}",
                    attribute.output_path
                ),
            });
        }
        let lead_required = required_fields.contains(attribute.output_path.as_str());
        if readiness_required && !lead_required {
            mismatches.push(DecisionInputReadinessMismatch {
                code: "decision_input_readiness_requirement_missing",
                field: "output_path",
                message: format!(
                    "{} is required by the decision input contract but not by lead_input_requirements.required_fields",
                    attribute.output_path
                ),
            });
        } else if !readiness_required && lead_required {
            mismatches.push(DecisionInputReadinessMismatch {
                code: "decision_input_readiness_requirement_conflict",
                field: "requirement",
                message: format!(
                    "{} is optional or conditional in the decision input contract but required by lead_input_requirements.required_fields",
                    attribute.output_path
                ),
            });
        }
    }
    mismatches
}

fn value_contract_constraints_match(left: &ValueContract, right: &ValueContract) -> bool {
    left.value_type == right.value_type
        && left.format == right.format
        && left.enum_values == right.enum_values
        && left.required == right.required
}

fn valid_decision_input_output_path(path: &str) -> bool {
    PROSPECT_CONTRACT_FIELDS.contains(&path)
        || path
            .strip_prefix("attributes.")
            .is_some_and(valid_attribute_key)
}

fn validate_qualification_gates(gate: Option<&QualificationGates>, issues: &mut Vec<Value>) {
    let Some(gate) = gate else {
        return;
    };
    if gate.signals.min == Some(0) {
        issues.push(issue(
            "qualification_gate_signal_min_zero",
            "error",
            ".mdp/manifest.yaml#/qualification_gates/signals/min",
            "qualification_gates.signals.min must be at least 1 when present",
        ));
    }
    if gate.signals.max == Some(0) {
        issues.push(issue(
            "qualification_gate_signal_max_zero",
            "error",
            ".mdp/manifest.yaml#/qualification_gates/signals/max",
            "qualification_gates.signals.max must be at least 1 when present",
        ));
    }
    if let (Some(min), Some(max)) = (gate.signals.min, gate.signals.max) {
        if min > max {
            issues.push(issue(
                "qualification_gate_signal_min_gt_max",
                "error",
                ".mdp/manifest.yaml#/qualification_gates/signals",
                format!("qualification_gates.signals.min ({min}) must not exceed max ({max})"),
            ));
        }
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

fn validate_card_entry_scopes(
    card: &Card,
    profile: Option<&Profile>,
    display_path: &str,
    issues: &mut Vec<Value>,
) {
    for (entry_index, entry) in card.entries.iter().enumerate() {
        for (dimension, values) in &entry.scope {
            let path = format!("{display_path}#/entries/{entry_index}/scope/{dimension}");
            let Some(allowed_values) =
                profile.and_then(|profile| profile.context_dimensions.get(dimension))
            else {
                issues.push(issue(
                    "card_entry_scope_dimension_unknown",
                    "error",
                    &path,
                    format!(
                        "entry scope dimension {dimension} is not declared by profile.context_dimensions"
                    ),
                ));
                continue;
            };
            if values.is_empty() {
                issues.push(issue(
                    "card_entry_scope_values_empty",
                    "error",
                    &path,
                    "entry scope dimensions must select at least one declared value",
                ));
            }
            let mut seen = BTreeSet::new();
            for (value_index, value) in values.iter().enumerate() {
                if !allowed_values.contains(value) {
                    issues.push(issue(
                        "card_entry_scope_value_unknown",
                        "error",
                        format!("{path}/{value_index}"),
                        format!(
                            "entry scope value {value} is not declared for dimension {dimension}"
                        ),
                    ));
                } else if !seen.insert(value.to_ascii_lowercase()) {
                    issues.push(issue(
                        "card_entry_scope_value_duplicate",
                        "error",
                        format!("{path}/{value_index}"),
                        format!("duplicate entry scope value {value}"),
                    ));
                }
            }
            if let Some(dependencies) =
                profile.and_then(|profile| profile.context_dimension_dependencies.get(dimension))
            {
                for dependency in dependencies {
                    if !entry.scope.contains_key(dependency) {
                        issues.push(issue(
                            "card_entry_scope_dependency_missing",
                            "error",
                            &path,
                            format!(
                                "entry scope dimension {dimension} requires companion dimension {dependency}"
                            ),
                        ));
                    }
                }
            }
        }
    }
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
                "scope",
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

fn validate_decision_input_contract_shapes(
    value: Option<&YamlValue>,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let Some(contracts) = value.and_then(YamlValue::as_sequence) else {
        return;
    };
    for (contract_index, contract) in contracts.iter().enumerate() {
        let contract_path = format!("{path}/{contract_index}");
        validate_object_keys_with_severity(
            contract,
            &[
                "id",
                "version",
                "description",
                "normalization",
                "source_classes",
                "attributes",
                "signal_projections",
            ],
            &contract_path,
            "manifest_decision_input_contract_unknown_field",
            "error",
            issues,
        );
        validate_required_object_keys(
            contract,
            &[
                "id",
                "version",
                "normalization",
                "source_classes",
                "attributes",
            ],
            &contract_path,
            "manifest_decision_input_contract_required_field_missing",
            issues,
        );
        if let Some(normalization) = yaml_get(contract, "normalization") {
            validate_object_keys_with_severity(
                normalization,
                &["prompt", "prompt_version", "normalized_schema_ref"],
                &format!("{contract_path}/normalization"),
                "manifest_decision_input_normalization_unknown_field",
                "error",
                issues,
            );
            validate_required_object_keys(
                normalization,
                &["prompt", "prompt_version", "normalized_schema_ref"],
                &format!("{contract_path}/normalization"),
                "manifest_decision_input_normalization_required_field_missing",
                issues,
            );
        }
        if let Some(projections) =
            yaml_get(contract, "signal_projections").and_then(YamlValue::as_sequence)
        {
            for (projection_index, projection) in projections.iter().enumerate() {
                let projection_path =
                    format!("{contract_path}/signal_projections/{projection_index}");
                validate_object_keys_with_severity(
                    projection,
                    &[
                        "id",
                        "kind",
                        "roles",
                        "contributor_attribute_ids",
                        "value",
                        "cardinality",
                        "conflict_policy",
                        "decision_effects",
                    ],
                    &projection_path,
                    "manifest_decision_input_signal_projection_unknown_field",
                    "error",
                    issues,
                );
                validate_required_object_keys(
                    projection,
                    &[
                        "id",
                        "kind",
                        "roles",
                        "contributor_attribute_ids",
                        "value",
                        "cardinality",
                        "conflict_policy",
                        "decision_effects",
                    ],
                    &projection_path,
                    "manifest_decision_input_signal_projection_required_field_missing",
                    issues,
                );
                validate_object_keys_with_severity(
                    yaml_get(projection, "value").unwrap_or(&YamlValue::Null),
                    &["type", "format", "enum", "required", "description"],
                    &format!("{projection_path}/value"),
                    "manifest_decision_input_signal_value_unknown_field",
                    "error",
                    issues,
                );
                validate_object_keys_with_severity(
                    yaml_get(projection, "cardinality").unwrap_or(&YamlValue::Null),
                    &["min", "max"],
                    &format!("{projection_path}/cardinality"),
                    "manifest_decision_input_signal_cardinality_unknown_field",
                    "error",
                    issues,
                );
                if let Some(cardinality) = yaml_get(projection, "cardinality") {
                    validate_required_object_keys(
                        cardinality,
                        &["min", "max"],
                        &format!("{projection_path}/cardinality"),
                        "manifest_decision_input_signal_cardinality_required_field_missing",
                        issues,
                    );
                }
            }
        }
        let Some(attributes) = yaml_get(contract, "attributes").and_then(YamlValue::as_sequence)
        else {
            continue;
        };
        for (attribute_index, attribute) in attributes.iter().enumerate() {
            let attribute_path = format!("{contract_path}/attributes/{attribute_index}");
            validate_object_keys_with_severity(
                attribute,
                &[
                    "id",
                    "question",
                    "description",
                    "output_path",
                    "value",
                    "requirement",
                    "applies_when",
                    "decision_effects",
                    "source_classes",
                    "provenance",
                    "confidence",
                    "freshness",
                    "sensitivity",
                    "status_behavior",
                ],
                &attribute_path,
                "manifest_decision_input_attribute_unknown_field",
                "error",
                issues,
            );
            validate_required_object_keys(
                attribute,
                &[
                    "id",
                    "question",
                    "output_path",
                    "value",
                    "requirement",
                    "decision_effects",
                    "source_classes",
                    "provenance",
                    "confidence",
                    "freshness",
                    "sensitivity",
                ],
                &attribute_path,
                "manifest_decision_input_attribute_required_field_missing",
                issues,
            );
            validate_object_keys_with_severity(
                yaml_get(attribute, "value").unwrap_or(&YamlValue::Null),
                &["type", "format", "enum", "required", "description"],
                &format!("{attribute_path}/value"),
                "manifest_decision_input_value_unknown_field",
                "error",
                issues,
            );
            validate_sequence_object_keys_with_severity(
                yaml_get(attribute, "applies_when"),
                &["attribute", "operator", "values"],
                &format!("{attribute_path}/applies_when"),
                "manifest_decision_input_applicability_unknown_field",
                "error",
                issues,
            );
            if let Some(conditions) =
                yaml_get(attribute, "applies_when").and_then(YamlValue::as_sequence)
            {
                for (condition_index, condition) in conditions.iter().enumerate() {
                    validate_required_object_keys(
                        condition,
                        &["attribute", "operator"],
                        &format!("{attribute_path}/applies_when/{condition_index}"),
                        "manifest_decision_input_applicability_required_field_missing",
                        issues,
                    );
                }
            }
            validate_object_keys_with_severity(
                yaml_get(attribute, "provenance").unwrap_or(&YamlValue::Null),
                &["required", "required_fields"],
                &format!("{attribute_path}/provenance"),
                "manifest_decision_input_provenance_unknown_field",
                "error",
                issues,
            );
            if let Some(provenance) = yaml_get(attribute, "provenance") {
                validate_required_object_keys(
                    provenance,
                    &["required", "required_fields"],
                    &format!("{attribute_path}/provenance"),
                    "manifest_decision_input_provenance_required_field_missing",
                    issues,
                );
            }
            validate_object_keys_with_severity(
                yaml_get(attribute, "confidence").unwrap_or(&YamlValue::Null),
                &["required", "minimum"],
                &format!("{attribute_path}/confidence"),
                "manifest_decision_input_confidence_unknown_field",
                "error",
                issues,
            );
            if let Some(confidence) = yaml_get(attribute, "confidence") {
                validate_required_object_keys(
                    confidence,
                    &["required"],
                    &format!("{attribute_path}/confidence"),
                    "manifest_decision_input_confidence_required_field_missing",
                    issues,
                );
            }
            validate_object_keys_with_severity(
                yaml_get(attribute, "freshness").unwrap_or(&YamlValue::Null),
                &["required", "max_age_days", "allow_unknown"],
                &format!("{attribute_path}/freshness"),
                "manifest_decision_input_freshness_unknown_field",
                "error",
                issues,
            );
            if let Some(freshness) = yaml_get(attribute, "freshness") {
                validate_required_object_keys(
                    freshness,
                    &["required", "allow_unknown"],
                    &format!("{attribute_path}/freshness"),
                    "manifest_decision_input_freshness_required_field_missing",
                    issues,
                );
            }
            validate_object_keys_with_severity(
                yaml_get(attribute, "status_behavior").unwrap_or(&YamlValue::Null),
                &[
                    "observed",
                    "not_found",
                    "not_applicable",
                    "blocked",
                    "error",
                ],
                &format!("{attribute_path}/status_behavior"),
                "manifest_decision_input_status_behavior_unknown_field",
                "error",
                issues,
            );
        }
    }
}

fn validate_sequence_object_keys_with_severity(
    value: Option<&YamlValue>,
    allowed: &[&str],
    path: &str,
    code: &str,
    severity: &str,
    issues: &mut Vec<Value>,
) {
    let Some(items) = value.and_then(YamlValue::as_sequence) else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        validate_object_keys_with_severity(
            item,
            allowed,
            &format!("{path}/{index}"),
            code,
            severity,
            issues,
        );
    }
}

fn validate_object_keys(
    value: &YamlValue,
    allowed: &[&str],
    path: &str,
    code: &str,
    issues: &mut Vec<Value>,
) {
    validate_object_keys_with_severity(value, allowed, path, code, "warning", issues);
}

fn validate_object_keys_with_severity(
    value: &YamlValue,
    allowed: &[&str],
    path: &str,
    code: &str,
    severity: &str,
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
                severity,
                format!("{path}/{key}"),
                format!(
                    "unsupported field {key} is parsed but ignored; put advisory extension data under entry metadata"
                ),
            ));
        }
    }
}

fn validate_required_object_keys(
    value: &YamlValue,
    required: &[&str],
    path: &str,
    code: &str,
    issues: &mut Vec<Value>,
) {
    let Some(map) = value.as_mapping() else {
        return;
    };
    for key in required {
        if !map.contains_key(YamlValue::String((*key).to_string())) {
            issues.push(issue(
                code,
                "error",
                format!("{path}/{key}"),
                format!("required field {key} is missing"),
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
    refs: BTreeMap<String, PromptInventoryEntry>,
}

#[derive(Clone, Debug, Default)]
struct PromptInventoryEntry {
    id: Option<String>,
    format: Option<String>,
    kind: Option<String>,
    contract: Option<String>,
    output_kind: Option<String>,
    schema_ref: Option<String>,
    version: Option<String>,
    canonical_path: Option<String>,
    required_inputs: BTreeSet<String>,
}

impl PromptInventory {
    fn contains(&self, value: &str) -> bool {
        self.refs.contains_key(value)
    }

    fn get(&self, value: &str) -> Option<&PromptInventoryEntry> {
        self.refs.get(value)
    }
}

fn prompt_inventory(loaded_prompts: &[Value]) -> PromptInventory {
    let mut inventory = PromptInventory::default();
    for prompt in loaded_prompts {
        let entry = PromptInventoryEntry {
            id: prompt["id"].as_str().map(ToOwned::to_owned),
            format: prompt["format"].as_str().map(ToOwned::to_owned),
            kind: prompt["kind"].as_str().map(ToOwned::to_owned),
            contract: prompt["output_contract"]["contract"]
                .as_str()
                .map(ToOwned::to_owned),
            output_kind: prompt["output_contract"]["output_kind"]
                .as_str()
                .map(ToOwned::to_owned),
            schema_ref: prompt["output_contract"]["schema_ref"]
                .as_str()
                .map(ToOwned::to_owned),
            version: prompt["version"].as_str().map(ToOwned::to_owned),
            canonical_path: prompt["path"]
                .as_str()
                .map(|path| path.strip_prefix(".mdp/").unwrap_or(path).to_string()),
            required_inputs: prompt["required_inputs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect(),
        };
        if let Some(id) = prompt["id"].as_str() {
            inventory.refs.insert(id.to_string(), entry.clone());
        }
        if let Some(path) = prompt["path"].as_str() {
            inventory.refs.insert(path.to_string(), entry.clone());
            if let Some(stripped) = path.strip_prefix(".mdp/") {
                inventory.refs.insert(stripped.to_string(), entry.clone());
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
                    "format": prompt.format,
                    "id": prompt.id,
                    "version": prompt.version,
                    "kind": prompt.kind,
                    "path": display_path,
                    "target_card_kinds": prompt.target_card_kinds,
                    "inputs": prompt.inputs.len(),
                    "required_inputs": prompt.inputs.iter()
                        .filter(|input| input.required)
                        .map(|input| input.name.as_str())
                        .collect::<Vec<_>>(),
                    "output_contract": {
                        "contract": prompt.output_contract.contract,
                        "output_kind": prompt.output_contract.output_kind,
                        "schema_ref": prompt.output_contract.schema_ref
                    }
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
            "version",
            "kind",
            "title",
            "description",
            "target_card_kinds",
            "tags",
            "inputs",
            "instructions",
            "role",
            "objective",
            "procedure",
            "selection_rules",
            "ambiguity_policy",
            "provenance_policy",
            "evidence_policy",
            "negative_examples",
            "final_checklist",
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
            "producer",
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
    if !matches!(
        prompt.format.as_str(),
        PROMPT_FORMAT_VERSION | PROMPT_FORMAT_V1
    ) {
        issues.push(issue(
            "prompt_format",
            "error",
            format!("{path}#/format"),
            format!(
                "prompt format must be {PROMPT_FORMAT_VERSION} or {PROMPT_FORMAT_V1}, found {}",
                prompt.format
            ),
        ));
    }
    if prompt.format == PROMPT_FORMAT_V1 {
        validate_prompt_v1(prompt, path, issues);
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
    let mut input_names = BTreeSet::new();
    for (index, input) in prompt.inputs.iter().enumerate() {
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
        let input_name = input.name.trim();
        if !input_name.is_empty() && !input_names.insert(input_name) {
            issues.push(issue(
                "prompt_input_name_duplicate",
                "error",
                format!("{path}#/inputs/{index}/name"),
                format!("prompt input name {} must be unique", input.name),
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

fn validate_prompt_v1(prompt: &PromptFile, path: &str, issues: &mut Vec<Value>) {
    if prompt
        .version
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        issues.push(issue(
            "prompt_v1_version_required",
            "error",
            format!("{path}#/version"),
            "mdp.prompt.v1 prompts must declare a non-blank version",
        ));
    }
    if !matches!(
        prompt.kind.as_deref(),
        Some("normalization" | "generation" | "review")
    ) {
        issues.push(issue(
            "prompt_v1_kind_required",
            "error",
            format!("{path}#/kind"),
            "mdp.prompt.v1 prompts must declare normalization, generation, or review",
        ));
    }
    for (field, value) in [("role", &prompt.role), ("objective", &prompt.objective)] {
        if value.as_deref().is_none_or(|value| value.trim().is_empty()) {
            issues.push(issue(
                "prompt_v1_text_required",
                "error",
                format!("{path}#/{field}"),
                format!("mdp.prompt.v1 prompts must declare a non-blank {field}"),
            ));
        }
    }
    for (field, values) in [
        ("procedure", &prompt.procedure),
        ("selection_rules", &prompt.selection_rules),
        ("ambiguity_policy", &prompt.ambiguity_policy),
        ("provenance_policy", &prompt.provenance_policy),
        ("evidence_policy", &prompt.evidence_policy),
        ("negative_examples", &prompt.negative_examples),
        ("final_checklist", &prompt.final_checklist),
    ] {
        if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
            issues.push(issue(
                "prompt_v1_rules_required",
                "error",
                format!("{path}#/{field}"),
                format!("mdp.prompt.v1 prompts must declare non-empty {field}"),
            ));
        }
    }
    for (index, input) in prompt.inputs.iter().enumerate() {
        if !matches!(
            input.producer.as_deref(),
            Some("host" | "pack" | "runtime" | "source" | "prior-step")
        ) {
            issues.push(issue(
                "prompt_v1_input_producer_required",
                "error",
                format!("{path}#/inputs/{index}/producer"),
                "mdp.prompt.v1 inputs must declare host, pack, runtime, source, or prior-step as producer",
            ));
        }
    }
}

fn validate_prompt_output_contract(prompt: &PromptFile, path: &str, issues: &mut Vec<Value>) {
    let contract = &prompt.output_contract;
    let output_kind = contract.output_kind.as_deref().unwrap_or("card-patches");
    let is_decision_input_normalization = output_kind == "decision-input-normalization"
        || matches!(
            contract.contract.as_str(),
            NORMALIZED_DECISION_INPUT_CONTRACT | NORMALIZED_DECISION_INPUT_CONTRACT_V2
        );
    if !matches!(
        output_kind,
        "card-patches"
            | "prospect-normalization"
            | "decision-input-normalization"
            | "governed-artifact"
    ) {
        issues.push(issue(
            "prompt_output_kind_unknown",
            "error",
            format!("{path}#/output_contract/output_kind"),
            format!(
                "prompt output_kind must be card-patches, prospect-normalization, decision-input-normalization, or governed-artifact, found {output_kind}"
            ),
        ));
    }
    let expected_contract = if is_decision_input_normalization {
        contract
            .schema_ref
            .as_deref()
            .unwrap_or(NORMALIZED_DECISION_INPUT_CONTRACT)
    } else {
        PROMPT_OUTPUT_CONTRACT
    };
    if contract.contract != expected_contract {
        issues.push(issue(
            "prompt_output_contract",
            "error",
            format!("{path}#/output_contract/contract"),
            format!(
                "prompt output contract must be {expected_contract}, found {}",
                contract.contract
            ),
        ));
    }
    if is_decision_input_normalization {
        if prompt
            .version
            .as_deref()
            .is_none_or(|version| version.trim().is_empty())
        {
            issues.push(issue(
                "decision_input_prompt_version_required",
                "error",
                format!("{path}#/version"),
                "decision-input-normalization prompts must declare a non-blank version",
            ));
        }
        if !matches!(
            contract.schema_ref.as_deref(),
            Some(NORMALIZED_DECISION_INPUT_CONTRACT | NORMALIZED_DECISION_INPUT_CONTRACT_V2)
        ) {
            issues.push(issue(
                "decision_input_prompt_schema_ref_required",
                "error",
                format!("{path}#/output_contract/schema_ref"),
                format!(
                    "decision-input-normalization prompts must use schema_ref {NORMALIZED_DECISION_INPUT_CONTRACT} or {NORMALIZED_DECISION_INPUT_CONTRACT_V2}"
                ),
            ));
        }
        if contract.schema.is_some() {
            issues.push(issue(
                "decision_input_prompt_inline_schema_unsupported",
                "error",
                format!("{path}#/output_contract/schema"),
                "decision-input-normalization prompts must use the canonical schema_ref; inline schemas are not supported for the job-compiled normalized envelope",
            ));
        }
    }
    if !contract.strict_json_only {
        issues.push(issue(
            "prompt_output_not_strict_json",
            "error",
            format!("{path}#/output_contract/strict_json_only"),
            "prompt outputs must be strict JSON only",
        ));
    }

    if output_kind == "governed-artifact" {
        match prompt
            .inputs
            .iter()
            .enumerate()
            .find(|(_, input)| input.name == "invocation_receipt_sha256")
        {
            None => issues.push(issue(
                "governed_artifact_invocation_receipt_input_missing",
                "error",
                format!("{path}#/inputs"),
                "governed-artifact prompts must declare invocation_receipt_sha256 as a required host-produced input",
            )),
            Some((index, input)) => {
                if !input.required {
                    issues.push(issue(
                        "governed_artifact_invocation_receipt_input_optional",
                        "error",
                        format!("{path}#/inputs/{index}/required"),
                        "governed-artifact prompt input invocation_receipt_sha256 must be required",
                    ));
                }
                if input.producer.as_deref() != Some("host") {
                    issues.push(issue(
                        "governed_artifact_invocation_receipt_input_producer_invalid",
                        "error",
                        format!("{path}#/inputs/{index}/producer"),
                        "governed-artifact prompt input invocation_receipt_sha256 must be produced by the host",
                    ));
                }
            }
        }
        for field in [
            "contract",
            "job_id",
            "prompt_id",
            "prompt_version",
            "prompt_sha256",
            "invocation_receipt_sha256",
            "source_summary",
            "selected_authority",
            "artifact",
            "gaps",
            "rejected_claims",
        ] {
            if !contract
                .required_top_level
                .iter()
                .any(|value| value == field)
            {
                issues.push(issue(
                    "prompt_output_required_field_missing",
                    "error",
                    format!("{path}#/output_contract/required_top_level"),
                    format!("governed-artifact prompt output contract must require {field}"),
                ));
            }
        }
        if let Some(schema) = contract.schema.as_ref() {
            validate_prompt_output_schema(prompt, schema, path, output_kind, issues);
            validate_governed_artifact_schema(prompt, schema, path, issues);
            if jsonschema::draft202012::validate(schema, &contract.example).is_err() {
                issues.push(issue(
                    "prompt_output_example_schema_mismatch",
                    "error",
                    format!("{path}#/output_contract/example"),
                    "governed-artifact example must satisfy the declared inline JSON Schema",
                ));
            }
        } else {
            issues.push(issue(
                "prompt_output_schema_missing",
                "error",
                format!("{path}#/output_contract/schema"),
                "governed-artifact prompts must declare an inline JSON Schema",
            ));
        }
        return;
    }

    let required = if is_decision_input_normalization {
        [
            "contract",
            "job_id",
            "decision_input_contracts",
            "normalization",
            "attributes",
            "normalized_prospect",
            "outcome",
            "draft_allowed",
        ]
        .as_slice()
    } else {
        [
            "contract",
            "prompt_id",
            "source_summary",
            "card_patches",
            "gaps",
            "rejected_claims",
        ]
        .as_slice()
    };
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

    if is_decision_input_normalization {
        validate_decision_input_prompt_example(prompt, path, issues);
    } else {
        validate_prompt_example(prompt, path, issues);
        validate_prompt_example_input_references(prompt, path, issues);
    }
    validate_prompt_schema_ref(prompt, path, output_kind, issues);
    if let Some(schema) = prompt
        .output_contract
        .schema
        .as_ref()
        .filter(|_| !is_decision_input_normalization)
    {
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

fn validate_governed_artifact_schema(
    prompt: &PromptFile,
    schema: &Value,
    path: &str,
    issues: &mut Vec<Value>,
) {
    let source_summary = &schema["properties"]["source_summary"];
    if source_summary["type"].as_str() != Some("object")
        || source_summary["additionalProperties"].as_bool() != Some(false)
        || !schema_array_contains(&source_summary["required"], "inputs_used")
    {
        issues.push(issue(
            "governed_artifact_source_summary_schema_invalid",
            "error",
            format!("{path}#/output_contract/schema/properties/source_summary"),
            "governed-artifact source_summary must be closed and require inputs_used",
        ));
    }
    let declared = prompt
        .inputs
        .iter()
        .map(|input| input.name.as_str())
        .collect::<BTreeSet<_>>();
    let input_enum = &source_summary["properties"]["inputs_used"]["items"]["enum"];
    let Some(values) = input_enum.as_array() else {
        issues.push(issue(
            "governed_artifact_inputs_used_schema_invalid",
            "error",
            format!("{path}#/output_contract/schema/properties/source_summary/properties/inputs_used/items/enum"),
            "governed-artifact inputs_used must enumerate declared prompt input names",
        ));
        return;
    };
    for (index, value) in values.iter().enumerate() {
        if value.as_str().is_none_or(|value| !declared.contains(value)) {
            issues.push(issue(
                "governed_artifact_inputs_used_undeclared",
                "error",
                format!("{path}#/output_contract/schema/properties/source_summary/properties/inputs_used/items/enum/{index}"),
                "governed-artifact inputs_used may enumerate declared prompt inputs only",
            ));
        }
    }
    let enumerated = values
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for input in declared.difference(&enumerated) {
        issues.push(issue(
            "governed_artifact_inputs_used_declared_input_missing",
            "error",
            format!("{path}#/output_contract/schema/properties/source_summary/properties/inputs_used/items/enum"),
            format!("governed-artifact inputs_used enum must include declared prompt input {input}"),
        ));
    }
    if schema["properties"]["selected_authority"]["type"].as_str() != Some("array")
        || schema["properties"]["artifact"]["type"].as_str() != Some("object")
    {
        issues.push(issue(
            "governed_artifact_trace_schema_invalid",
            "error",
            format!("{path}#/output_contract/schema/properties"),
            "governed-artifact schema must define selected_authority as an array and artifact as an object",
        ));
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
    let expected = match output_kind {
        "prospect-normalization" => PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF,
        "decision-input-normalization" => prompt.output_contract.contract.as_str(),
        _ => PROMPT_CARD_PATCH_SCHEMA_REF,
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

fn validate_decision_input_prompt_example(
    prompt: &PromptFile,
    path: &str,
    issues: &mut Vec<Value>,
) {
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
    let expected_contract = prompt.output_contract.contract.as_str();
    if example["contract"].as_str() != Some(expected_contract) {
        issues.push(issue(
            "prompt_example_contract",
            "error",
            format!("{path}#/output_contract/example/contract"),
            format!("decision-input normalization example contract must be {expected_contract}"),
        ));
    }
    if example["draft_allowed"].as_bool() != Some(false) {
        issues.push(issue(
            "prompt_example_draft_allowed",
            "error",
            format!("{path}#/output_contract/example/draft_allowed"),
            "decision-input normalization examples must set draft_allowed to false",
        ));
    }
    if example["normalized_prospect"].as_object().is_none() {
        issues.push(issue(
            "prompt_normalized_prospect_missing",
            "error",
            format!("{path}#/output_contract/example/normalized_prospect"),
            "decision-input normalization examples must include normalized_prospect object",
        ));
    }
    if example["attributes"].as_object().is_none() {
        issues.push(issue(
            "prompt_decision_input_attributes_missing",
            "error",
            format!("{path}#/output_contract/example/attributes"),
            "decision-input normalization examples must include per-attribute attempt results",
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
    } else if output_kind == "card-patches" {
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
                format!("prompt example source_summary.inputs_used must use declared prompt input names only; {input} is not declared"),
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
                    "prompt example reference {reference} must start with a declared prompt input name"
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
    if let Some(alias) = example.get("normalized_opportunity") {
        if !alias.is_object() {
            issues.push(issue(
                "prompt_normalized_opportunity_type",
                "error",
                format!("{path}#/output_contract/example/normalized_opportunity"),
                "normalized_opportunity must be an object when provided",
            ));
        } else if alias != &example["normalized_prospect"] {
            issues.push(issue(
                "prompt_normalized_opportunity_mismatch",
                "error",
                format!("{path}#/output_contract/example/normalized_opportunity"),
                "normalized_opportunity must exactly match normalized_prospect; it is a proposal-readable alias, not a separate core opportunity object",
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
    let mut decision_input_contract_gaps = Vec::new();
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
    let input_contracts_by_id = manifest
        .input_contracts
        .iter()
        .map(|contract| (contract.id.as_str(), contract))
        .collect::<BTreeMap<_, _>>();
    for input_contract in &manifest.input_contracts {
        if input_contract.decision_input_contracts.is_empty() {
            decision_input_contract_gaps.push(json!({
                "scope": "input-contract",
                "id": &input_contract.id,
                "reason": "no decision input contract articulates what an upstream collector or normalizer must attempt"
            }));
        }
    }
    for job in &manifest.jobs {
        let inherited = job.input_contracts.iter().any(|id| {
            input_contracts_by_id
                .get(id.as_str())
                .is_some_and(|contract| !contract.decision_input_contracts.is_empty())
        });
        if job.decision_input_contracts.is_empty() && !inherited {
            decision_input_contract_gaps.push(json!({
                "scope": "job",
                "id": &job.id,
                "reason": "job has no direct or input-contract decision input contract binding"
            }));
        }
    }
    let required_attributes = manifest
        .lead_input_requirements
        .required_attributes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let required_fields = manifest
        .lead_input_requirements
        .required_fields
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for contract in &manifest.decision_input_contracts {
        for attribute in &contract.attributes {
            for mismatch in decision_input_readiness_mismatches(
                &manifest,
                attribute,
                &required_attributes,
                &required_fields,
            ) {
                decision_input_contract_gaps.push(json!({
                    "scope": "decision-input-contract",
                    "id": &contract.id,
                    "attribute_id": &attribute.id,
                    "code": mismatch.code,
                    "reason": mismatch.message
                }));
            }
        }
    }
    let durable_count = durable_gaps.len();
    let evidence_count = evidence_gaps.len();
    let decision_input_count = decision_input_contract_gaps.len();
    Ok(json!({
        "contract": "mdp.gaps.v0",
        "durable_gaps": durable_gaps,
        "evidence_gaps": evidence_gaps,
        "decision_input_contract_gaps": decision_input_contract_gaps,
        "summary": {
            "durable": durable_count,
            "evidence": evidence_count,
            "decision_input_contract": decision_input_count
        }
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
    use crate::commands::init::{TargetInitOptions, init_pack, init_pack_targeted};
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

    fn targeted_pack(name: &str, excluded: &[String]) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-targeted-{nonce}"));
        init_pack_targeted(
            &root,
            &format!("{name} Messaging"),
            "gtm",
            &TargetInitOptions {
                custom_name: true,
                name: Some(name),
                excluded_terms: excluded,
                ..TargetInitOptions::default()
            },
            true,
            false,
        )
        .expect("targeted pack should initialize");
        root
    }

    fn opt_in_generation_prompt(root: &Path) {
        let prompt_path = root.join(".mdp/prompts/normalize-prospect.yaml");
        let raw = std::fs::read_to_string(&prompt_path).expect("prompt should be readable");
        let mut prompt: YamlValue = serde_yaml::from_str(&raw).expect("prompt should parse");
        prompt["format"] = YamlValue::String(PROMPT_FORMAT_V1.to_string());
        prompt["version"] = YamlValue::String("1".to_string());
        prompt["kind"] = YamlValue::String("generation".to_string());
        prompt["role"] = YamlValue::String("Grounded outbound copy writer".to_string());
        prompt["objective"] =
            YamlValue::String("Produce one governed outbound draft artifact.".to_string());
        for field in [
            "procedure",
            "selection_rules",
            "ambiguity_policy",
            "provenance_policy",
            "evidence_policy",
            "negative_examples",
            "final_checklist",
        ] {
            prompt[field] =
                YamlValue::Sequence(vec![YamlValue::String(format!("Explicit {field} rule"))]);
        }
        for input in prompt["inputs"]
            .as_sequence_mut()
            .expect("inputs should be a sequence")
        {
            input["producer"] = YamlValue::String("host".to_string());
        }
        prompt["inputs"]
            .as_sequence_mut()
            .expect("inputs should be a sequence")
            .push(
                serde_yaml::from_str(
                    r#"
name: routed_context
description: Exact MDP-compiled routed context.
required: true
default: N/A
missing_behavior: Return a gap or refusal.
producer: host
"#,
                )
                .expect("routed context input should parse"),
            );
        prompt["inputs"]
            .as_sequence_mut()
            .expect("inputs should be a sequence")
            .push(
                serde_yaml::from_str(
                    r#"
name: invocation_receipt_sha256
description: Detached SHA-256 of the exact host invocation receipt bytes.
required: true
default: N/A
missing_behavior: Return a gap or refusal; never invent the receipt hash.
producer: host
"#,
                )
                .expect("receipt input should parse"),
            );
        prompt["output_contract"]["output_kind"] =
            YamlValue::String("governed-artifact".to_string());
        prompt["output_contract"]["schema_ref"] = YamlValue::Null;
        let output_contract: YamlValue = serde_yaml::from_str(
            r#"
required_top_level:
  - contract
  - job_id
  - prompt_id
  - prompt_version
  - prompt_sha256
  - context_sha256
  - invocation_receipt_sha256
  - source_summary
  - selected_authority
  - artifact
  - gaps
  - rejected_claims
schema:
  type: object
  additionalProperties: false
  required: [contract, job_id, prompt_id, prompt_version, prompt_sha256, context_sha256, invocation_receipt_sha256, source_summary, selected_authority, artifact, gaps, rejected_claims]
  properties:
    contract: {const: mdp.prompt-output.v0}
    job_id: {const: prospect-fit-or-brief}
    prompt_id: {const: normalize-prospect-row}
    prompt_version: {const: "1"}
    prompt_sha256: {type: string, minLength: 64, maxLength: 64}
    context_sha256: {type: string, minLength: 64, maxLength: 64}
    invocation_receipt_sha256: {type: string, minLength: 64, maxLength: 64}
    source_summary:
      type: object
      additionalProperties: false
      required: [inputs_used]
      properties:
        inputs_used:
          type: array
          items: {enum: [raw_row, company_domain, existing_pack_context, runtime_context, source_kind, routed_context, invocation_receipt_sha256]}
    selected_authority: {type: array, items: {type: string}}
    artifact: {type: object}
    gaps: {type: array, items: {type: string}}
    rejected_claims: {type: array, items: {type: string}}
example:
  contract: mdp.prompt-output.v0
  job_id: prospect-fit-or-brief
  prompt_id: normalize-prospect-row
  prompt_version: "1"
  prompt_sha256: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
  context_sha256: cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
  invocation_receipt_sha256: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
  source_summary: {inputs_used: []}
  selected_authority: [positioning/decision-layer]
  artifact: {message_body: "Hello"}
  gaps: []
  rejected_claims: []
"#,
        )
        .expect("output contract should parse");
        for field in ["required_top_level", "schema", "example"] {
            prompt["output_contract"][field] = output_contract[field].clone();
        }
        std::fs::write(
            &prompt_path,
            serde_yaml::to_string(&prompt).expect("prompt should serialize"),
        )
        .expect("prompt should be writable");

        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["jobs"][0]["model_task"] = serde_yaml::from_str(
            r#"
kind: generation
prompt: normalize-prospect-row
"#,
        )
        .expect("model task should parse");
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    #[test]
    fn opted_in_job_owned_generation_prompt_is_ready() {
        let root = temp_pack("job-owned-generation-prompt");
        opt_in_generation_prompt(&root);

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], true, "issues: {}", result["issues"]);
        assert_eq!(
            result["profile"]["jobs"][0]["model_task"]["status"],
            "ready"
        );
        assert_eq!(
            result["profile"]["jobs"][0]["model_task"]["prompt_version"],
            "1"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn context_budgeted_model_task_requires_routed_context_input() {
        let root = temp_pack("job-owned-generation-without-routed-context");
        opt_in_generation_prompt(&root);
        let prompt_path = root.join(".mdp/prompts/normalize-prospect.yaml");
        let raw = std::fs::read_to_string(&prompt_path).expect("prompt should be readable");
        let mut prompt: YamlValue = serde_yaml::from_str(&raw).expect("prompt should parse");
        prompt["inputs"]
            .as_sequence_mut()
            .expect("inputs should be a sequence")
            .retain(|input| input["name"].as_str() != Some("routed_context"));
        std::fs::write(
            &prompt_path,
            serde_yaml::to_string(&prompt).expect("prompt should serialize"),
        )
        .expect("prompt should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues")
                .iter()
                .any(|issue| {
                    issue["code"] == "profile_job_model_task_routed_context_input_missing"
                })
        );
        assert_eq!(
            result["profile"]["jobs"][0]["model_task"]["status"],
            "blocked"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn governed_prompt_requires_receipt_fields_in_required_top_level() {
        let root = temp_pack("governed-required-receipts");
        opt_in_generation_prompt(&root);
        let prompt_path = root.join(".mdp/prompts/normalize-prospect.yaml");
        let raw = std::fs::read_to_string(&prompt_path).expect("prompt should be readable");
        let mut prompt: YamlValue = serde_yaml::from_str(&raw).expect("prompt should parse");
        prompt["output_contract"]["required_top_level"]
            .as_sequence_mut()
            .expect("required fields should be a sequence")
            .retain(|field| field.as_str() != Some("invocation_receipt_sha256"));
        std::fs::write(
            &prompt_path,
            serde_yaml::to_string(&prompt).expect("prompt should serialize"),
        )
        .expect("prompt should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert!(
            result["issues"]
                .as_array()
                .expect("issues")
                .iter()
                .any(|issue| issue["code"] == "prompt_output_required_field_missing")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn governed_prompt_requires_host_produced_invocation_receipt_input() {
        for (case, mutation, expected_code) in [
            (
                "missing",
                "missing",
                "governed_artifact_invocation_receipt_input_missing",
            ),
            (
                "optional",
                "optional",
                "governed_artifact_invocation_receipt_input_optional",
            ),
            (
                "wrong-producer",
                "wrong-producer",
                "governed_artifact_invocation_receipt_input_producer_invalid",
            ),
            ("duplicate", "duplicate", "prompt_input_name_duplicate"),
        ] {
            let root = temp_pack(&format!("governed-receipt-input-{case}"));
            opt_in_generation_prompt(&root);
            let prompt_path = root.join(".mdp/prompts/normalize-prospect.yaml");
            let raw = std::fs::read_to_string(&prompt_path).expect("prompt should be readable");
            let mut prompt: YamlValue = serde_yaml::from_str(&raw).expect("prompt should parse");
            let inputs = prompt["inputs"]
                .as_sequence_mut()
                .expect("inputs should be a sequence");
            let receipt_index = inputs
                .iter()
                .position(|input| input["name"].as_str() == Some("invocation_receipt_sha256"))
                .expect("receipt input should exist");
            match mutation {
                "missing" => {
                    inputs.remove(receipt_index);
                }
                "optional" => inputs[receipt_index]["required"] = YamlValue::Bool(false),
                "wrong-producer" => {
                    inputs[receipt_index]["producer"] = YamlValue::String("pack".to_string())
                }
                "duplicate" => {
                    let mut duplicate = inputs[receipt_index].clone();
                    duplicate["name"] = YamlValue::String("invocation_receipt_sha256 ".to_string());
                    duplicate["producer"] = YamlValue::String("pack".to_string());
                    inputs.push(duplicate);
                }
                _ => unreachable!("test mutation is closed"),
            }
            std::fs::write(
                &prompt_path,
                serde_yaml::to_string(&prompt).expect("prompt should serialize"),
            )
            .expect("prompt should be writable");

            let result = validate_pack(&root).expect("validate should return diagnostics");

            assert!(
                result["issues"]
                    .as_array()
                    .expect("issues")
                    .iter()
                    .any(|issue| issue["code"] == expected_code),
                "{case} should emit {expected_code}: {}",
                result["issues"]
            );
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn governed_prompt_inputs_used_enum_must_cover_every_declared_input() {
        let root = temp_pack("governed-input-enum-coverage");
        opt_in_generation_prompt(&root);
        let prompt_path = root.join(".mdp/prompts/normalize-prospect.yaml");
        let raw = std::fs::read_to_string(&prompt_path).expect("prompt should be readable");
        let mut prompt: YamlValue = serde_yaml::from_str(&raw).expect("prompt should parse");
        prompt["output_contract"]["schema"]["properties"]["source_summary"]["properties"]
            ["inputs_used"]["items"]["enum"]
            .as_sequence_mut()
            .expect("inputs_used enum should be a sequence")
            .retain(|input| input.as_str() != Some("raw_row"));
        std::fs::write(
            &prompt_path,
            serde_yaml::to_string(&prompt).expect("prompt should serialize"),
        )
        .expect("prompt should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert!(
            result["issues"]
                .as_array()
                .expect("issues")
                .iter()
                .any(|issue| {
                    issue["code"] == "governed_artifact_inputs_used_declared_input_missing"
                })
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn opted_in_job_owned_prompt_kind_mismatch_fails_closed() {
        let root = temp_pack("job-owned-prompt-kind-mismatch");
        opt_in_generation_prompt(&root);
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace("kind: generation", "kind: review"),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues")
                .iter()
                .any(|issue| { issue["code"] == "profile_job_model_task_kind_mismatch" })
        );
        assert_eq!(
            result["profile"]["jobs"][0]["model_task"]["status"],
            "blocked"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    fn opt_in_product_foundation(root: &Path) {
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile"]["product_foundation"] = serde_yaml::from_str(
            r#"
facets:
  - id: identity
    kind: product_identity
    entries:
      - card_id: positioning
        entry_id: decision-layer
    gaps: []
    conflicts_with: []
  - id: missing-proof
    kind: gaps
    entries: []
    gaps:
      - card_id: gaps
        entry_id: missing-company-proof
    conflicts_with: []
"#,
        )
        .expect("foundation should parse");
        let binding: YamlValue = serde_yaml::from_str(
            r#"
required:
  - identity
conditional:
  - facet_id: missing-proof
    when:
      fact: job_id
      equals: prospect-fit-or-brief
optional: []
excluded: []
"#,
        )
        .expect("binding should parse");
        for job in manifest["jobs"]
            .as_sequence_mut()
            .expect("jobs should be a sequence")
        {
            job["product_foundation"] = binding.clone();
        }
        std::fs::write(
            manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");
    }

    fn product_foundation_issues(root: &Path) -> Vec<Value> {
        validate_pack(root).expect("validate should return diagnostics")["issues"]
            .as_array()
            .expect("issues array")
            .clone()
    }

    #[test]
    fn legacy_manifest_without_product_foundation_remains_valid() {
        let root = temp_pack("foundation-legacy");

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], true, "issues: {}", result["issues"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_invalid_job_context_budget() {
        let root = temp_pack("invalid-job-context-budget");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["jobs"][0]["context_budget"] =
            serde_yaml::from_str("max_entries: 0\nmax_bytes: 1024\nlegacy_limit: 2\n")
                .expect("budget should parse");
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        let issues = result["issues"]
            .as_array()
            .expect("issues should be an array");
        assert!(issues.iter().any(|issue| {
            issue["code"] == "profile_job_context_budget_limit_invalid"
                && issue["path"] == ".mdp/manifest.yaml#/jobs/0/context_budget/max_entries"
        }));
        assert!(issues.iter().any(|issue| {
            issue["code"] == "manifest_profile_job_context_budget_unknown_field"
                && issue["path"] == ".mdp/manifest.yaml#/jobs/0/context_budget/legacy_limit"
        }));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validates_opted_in_product_foundation_contract() {
        let root = temp_pack("foundation-valid");
        opt_in_product_foundation(&root);

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(result["valid"], true, "issues: {}", result["issues"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn selected_product_foundation_gap_blocks_job_and_profile_activation() {
        let root = temp_pack("foundation-gap-blocks-activation");
        opt_in_product_foundation(&root);

        let result = validate_pack(&root).expect("validate should return diagnostics");
        let selected_job = &result["profile"]["jobs"][0];

        assert_eq!(result["valid"], true, "issues: {}", result["issues"]);
        assert_eq!(selected_job["product_foundation"]["status"], "blocked");
        assert_eq!(selected_job["activation_ready"], false);
        assert_eq!(result["profile"]["activation_ready"], false);
        assert!(
            selected_job["product_foundation"]["diagnostics"]
                .as_array()
                .expect("foundation diagnostics")
                .iter()
                .any(|diagnostic| {
                    diagnostic["code"] == "product_foundation_selected_facet_has_gaps"
                })
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_unknown_and_duplicate_product_foundation_facets() {
        let root = temp_pack("foundation-facet-shape");
        opt_in_product_foundation(&root);
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile"]["product_foundation"]["facets"][0]["kind"] =
            YamlValue::String("invented_kind".to_string());
        let duplicate = manifest["profile"]["product_foundation"]["facets"][0].clone();
        manifest["profile"]["product_foundation"]["facets"]
            .as_sequence_mut()
            .expect("facets should be a sequence")
            .push(duplicate);
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let issues = product_foundation_issues(&root);
        assert!(issues.iter().any(|issue| {
            issue["code"] == "product_foundation_facet_kind_unknown"
                && issue["path"] == ".mdp/manifest.yaml#/profile/product_foundation/facets/0/kind"
        }));
        assert!(issues.iter().any(|issue| {
            issue["code"] == "product_foundation_facet_duplicate"
                && issue["path"] == ".mdp/manifest.yaml#/profile/product_foundation/facets/2/id"
        }));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_dangling_product_foundation_references() {
        let root = temp_pack("foundation-dangling");
        opt_in_product_foundation(&root);
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        let facets = manifest["profile"]["product_foundation"]["facets"]
            .as_sequence_mut()
            .expect("facets should be a sequence");
        facets[0]["entries"] = serde_yaml::from_str(
            "- card_id: missing-card\n  entry_id: missing-entry\n- card_id: positioning\n  entry_id: missing-entry\n",
        )
        .expect("entry refs should parse");
        facets[1]["gaps"] = serde_yaml::from_str(
            "- card_id: positioning\n  entry_id: decision-layer\n- card_id: gaps\n  entry_id: missing-gap\n",
        )
        .expect("gap refs should parse");
        facets[0]["conflicts_with"] =
            serde_yaml::from_str("- missing-facet\n").expect("conflicts should parse");
        manifest["jobs"][0]["product_foundation"]["optional"] =
            serde_yaml::from_str("- missing-binding-facet\n").expect("optional should parse");
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let issues = product_foundation_issues(&root);
        for (code, path) in [
            (
                "product_foundation_card_missing",
                ".mdp/manifest.yaml#/profile/product_foundation/facets/0/entries/0/card_id",
            ),
            (
                "product_foundation_entry_missing",
                ".mdp/manifest.yaml#/profile/product_foundation/facets/0/entries/1/entry_id",
            ),
            (
                "product_foundation_gap_card_kind_invalid",
                ".mdp/manifest.yaml#/profile/product_foundation/facets/1/gaps/0/card_id",
            ),
            (
                "product_foundation_gap_missing",
                ".mdp/manifest.yaml#/profile/product_foundation/facets/1/gaps/1/entry_id",
            ),
            (
                "product_foundation_conflict_facet_missing",
                ".mdp/manifest.yaml#/profile/product_foundation/facets/0/conflicts_with/0",
            ),
            (
                "profile_job_product_foundation_facet_missing",
                ".mdp/manifest.yaml#/jobs/0/product_foundation/optional/0",
            ),
        ] {
            assert!(
                issues
                    .iter()
                    .any(|issue| issue["code"] == code && issue["path"] == path),
                "missing {code} at {path}: {issues:?}"
            );
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_gaps_card_reference_as_authoritative_foundation_entry() {
        let root = temp_pack("foundation-gap-as-entry");
        opt_in_product_foundation(&root);
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile"]["product_foundation"]["facets"][0]["entries"] =
            serde_yaml::from_str("- card_id: gaps\n  entry_id: missing-company-proof\n")
                .expect("entry reference should parse");
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let issues = product_foundation_issues(&root);

        assert!(issues.iter().any(|issue| {
            issue["code"] == "product_foundation_entry_card_kind_invalid"
                && issue["path"]
                    == ".mdp/manifest.yaml#/profile/product_foundation/facets/0/entries/0/card_id"
        }));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_product_foundation_inline_statements_and_unknown_fields() {
        let root = temp_pack("foundation-inline");
        opt_in_product_foundation(&root);
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile"]["product_foundation"]["facets"][0]["statement"] =
            YamlValue::String("Inline authority must be rejected".to_string());
        manifest["jobs"][0]["product_foundation"]["runtime_context"] =
            YamlValue::String("forbidden".to_string());
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let issues = product_foundation_issues(&root);
        assert!(issues.iter().any(|issue| {
            issue["code"] == "manifest_product_foundation_facet_unknown_field"
                && issue["path"]
                    == ".mdp/manifest.yaml#/profile/product_foundation/facets/0/statement"
        }));
        assert!(issues.iter().any(|issue| {
            issue["code"] == "manifest_profile_job_product_foundation_unknown_field"
                && issue["path"] == ".mdp/manifest.yaml#/jobs/0/product_foundation/runtime_context"
        }));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_invalid_product_foundation_conditional_and_duplicate_classification() {
        let root = temp_pack("foundation-condition");
        opt_in_product_foundation(&root);
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["jobs"][0]["product_foundation"]["conditional"][0]["when"]["fact"] =
            YamlValue::String("runtime_input".to_string());
        let duplicate_conditional =
            manifest["jobs"][0]["product_foundation"]["conditional"][0].clone();
        manifest["jobs"][0]["product_foundation"]["conditional"]
            .as_sequence_mut()
            .expect("conditional should be a sequence")
            .push(duplicate_conditional);
        manifest["jobs"][0]["product_foundation"]["optional"] =
            serde_yaml::from_str("- identity\n").expect("optional should parse");
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let issues = product_foundation_issues(&root);
        assert!(issues.iter().any(|issue| {
            issue["code"] == "product_foundation_condition_fact_unknown"
                && issue["path"]
                    == ".mdp/manifest.yaml#/jobs/0/product_foundation/conditional/0/when/fact"
        }));
        assert!(issues.iter().any(|issue| {
            issue["code"] == "profile_job_product_foundation_facet_duplicate"
                && issue["path"] == ".mdp/manifest.yaml#/jobs/0/product_foundation/optional/0"
        }));
        assert!(issues.iter().any(|issue| {
            issue["code"] == "profile_job_product_foundation_facet_duplicate"
                && issue["path"]
                    == ".mdp/manifest.yaml#/jobs/0/product_foundation/conditional/1/facet_id"
        }));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_product_foundation_self_conflict() {
        let root = temp_pack("foundation-self-conflict");
        opt_in_product_foundation(&root);
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["profile"]["product_foundation"]["facets"][0]["conflicts_with"] =
            serde_yaml::from_str("- identity\n").expect("conflicts should parse");
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let issues = product_foundation_issues(&root);
        assert!(issues.iter().any(|issue| {
            issue["code"] == "product_foundation_conflict_self"
                && issue["path"]
                    == ".mdp/manifest.yaml#/profile/product_foundation/facets/0/conflicts_with/0"
        }));
        let _ = std::fs::remove_dir_all(root);
    }

    fn clay_example_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("CLI crate should have a repository parent")
            .join("examples/clay-audiences-self-serve-enterprise-expansion")
    }

    fn temp_clay_pack(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-clay-{name}-{nonce}"));
        copy_test_directory(&clay_example_root().join(".mdp"), &root.join(".mdp"));
        root
    }

    fn copy_test_directory(source: &Path, destination: &Path) {
        std::fs::create_dir_all(destination).expect("test destination should be creatable");
        for entry in std::fs::read_dir(source).expect("test source should be readable") {
            let entry = entry.expect("test source entry should be readable");
            let destination_path = destination.join(entry.file_name());
            if entry
                .file_type()
                .expect("test source entry should have a type")
                .is_dir()
            {
                copy_test_directory(&entry.path(), &destination_path);
            } else {
                std::fs::copy(entry.path(), destination_path)
                    .expect("test source file should copy");
            }
        }
    }

    #[test]
    fn validate_reports_excluded_target_term_with_field_location() {
        let root = targeted_pack("Company B", &["Company A".to_string()]);
        let card_path = root.join(".mdp/cards/hooks.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("hook card should be readable");
        std::fs::write(
            &card_path,
            raw.replace("No hook is approved", "Company A says no hook is approved"),
        )
        .expect("hook card should be writable");
        let prompt_path = root.join(".mdp/prompts/pains.yaml");
        let raw = std::fs::read_to_string(&prompt_path).expect("prompt should be readable");
        std::fs::write(
            &prompt_path,
            raw.replace("Evidence required", "Company A pain"),
        )
        .expect("prompt should be writable");
        let eval_path = root.join(".mdp/evals/target-route.yaml");
        let raw = std::fs::read_to_string(&eval_path).expect("eval should be readable");
        std::fs::write(
            &eval_path,
            raw.replace("create or improve messaging", "Company A messaging"),
        )
        .expect("eval should be writable");
        let source_path = root.join(".mdp/sources.yaml");
        let raw = std::fs::read_to_string(&source_path).expect("sources should be readable");
        std::fs::write(
            &source_path,
            raw.replace("Source ledger for Company B", "Source ledger for Company A"),
        )
        .expect("sources should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        let contamination_paths = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter(|issue| issue["code"] == "target_contamination_excluded_term")
            .filter_map(|issue| issue["path"].as_str())
            .collect::<BTreeSet<_>>();
        assert!(contamination_paths.contains(".mdp/cards/hooks.yaml#/entries/0/body"));
        assert!(contamination_paths.iter().any(|path| {
            path.starts_with(".mdp/prompts/pains.yaml#/output_contract/example/card_patches")
        }));
        assert!(contamination_paths.contains(".mdp/evals/target-route.yaml#/job"));
        assert!(contamination_paths.contains(".mdp/sources.yaml#/purpose"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn targeted_starter_is_valid_under_current_skill_and_job_contract() {
        let root = targeted_pack("Company B", &["Company A".to_string()]);

        let result = validate_pack(&root).expect("validate should return diagnostics");
        assert_eq!(result["valid"], true, "issues: {}", result["issues"]);
        assert_eq!(result["error_count"], 0);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn target_name_only_exempts_its_own_internal_vocabulary_occurrence() {
        let alias_target = TargetIdentity {
            name: "Acme".to_string(),
            aliases: vec!["Acme MDP".to_string()],
            ..TargetIdentity::default()
        };
        let alias_scan = redact_active_target_identity(
            &alias_target,
            "Acme MDP is powered by the Message Decision Pack.",
        );
        assert!(!contains_term(&alias_scan, "MDP"));
        assert!(contains_term(&alias_scan, "Message Decision Pack"));

        let root = targeted_pack("Acme MDP", &[]);

        let baseline = validate_pack(&root).expect("validate should return diagnostics");
        assert_eq!(baseline["valid"], true, "issues: {}", baseline["issues"]);

        let card_path = root.join(".mdp/cards/positioning.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("positioning should be readable");
        let mut positioning: YamlValue =
            serde_yaml::from_str(&raw).expect("positioning should parse");
        positioning["entries"][0]["body"] = YamlValue::String(
            "Acme MDP is powered by the Message Decision Pack and improves agent handoffs."
                .to_string(),
        );
        std::fs::write(
            &card_path,
            serde_yaml::to_string(&positioning).expect("positioning should serialize"),
        )
        .expect("positioning should be writable");

        let brief_path = root.join(".mdp/briefs/outbound.md");
        std::fs::write(
            &brief_path,
            "Acme MDP helps teams.\nAcme MDP is powered by the Message Decision Pack.\n",
        )
        .expect("brief should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        let contamination_paths = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter(|issue| issue["code"] == "target_contamination_internal_vocabulary")
            .filter_map(|issue| issue["path"].as_str())
            .collect::<BTreeSet<_>>();
        assert!(contamination_paths.contains(".mdp/cards/positioning.yaml#/entries/0/body"));
        assert!(!contamination_paths.contains(".mdp/briefs/outbound.md:1"));
        assert!(contamination_paths.contains(".mdp/briefs/outbound.md:2"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_internal_vocabulary_as_positioning_but_allows_negative_eval() {
        let root = targeted_pack("Company B", &[]);
        let card_path = root.join(".mdp/cards/positioning.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("positioning should be readable");
        std::fs::write(
            &card_path,
            raw.replace(
                "Prospect-facing positioning",
                "Message Decision Pack positioning",
            ),
        )
        .expect("positioning should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(
                    |issue| issue["code"] == "target_contamination_internal_vocabulary"
                        && issue["path"] == ".mdp/cards/positioning.yaml#/entries/0/body"
                )
        );
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .all(|issue| issue["path"]
                    != ".mdp/evals/internal-control-plane-rejected.yaml#/text")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_internal_positioning_in_prompt_instructions_and_briefs() {
        let root = targeted_pack("Company B", &[]);
        let prompt_path = root.join(".mdp/prompts/claims-proof.yaml");
        let raw = std::fs::read_to_string(&prompt_path).expect("prompt should be readable");
        let mut prompt: YamlValue = serde_yaml::from_str(&raw).expect("prompt should parse");
        prompt["instructions"][0] = YamlValue::String(
            "Do not position MDP as internal tooling, but position the Message Decision Pack as the sold product."
                .to_string(),
        );
        std::fs::write(
            &prompt_path,
            serde_yaml::to_string(&prompt).expect("prompt should serialize"),
        )
        .expect("prompt should be writable");

        let safe_prompt_path = root.join(".mdp/prompts/gaps.yaml");
        let raw = std::fs::read_to_string(&safe_prompt_path).expect("prompt should be readable");
        let mut safe_prompt: YamlValue = serde_yaml::from_str(&raw).expect("prompt should parse");
        safe_prompt["instructions"][0] =
            YamlValue::String("Never sell the Message Decision Pack as the product.".to_string());
        std::fs::write(
            &safe_prompt_path,
            serde_yaml::to_string(&safe_prompt).expect("prompt should serialize"),
        )
        .expect("prompt should be writable");

        let description_prompt_path = root.join(".mdp/prompts/hooks.yaml");
        let raw =
            std::fs::read_to_string(&description_prompt_path).expect("prompt should be readable");
        let mut description_prompt: YamlValue =
            serde_yaml::from_str(&raw).expect("prompt should parse");
        description_prompt["description"] =
            YamlValue::String("Market the Message Decision Pack as the sold product.".to_string());
        std::fs::write(
            &description_prompt_path,
            serde_yaml::to_string(&description_prompt).expect("prompt should serialize"),
        )
        .expect("prompt should be writable");

        let brief_path = root.join(".mdp/briefs/outbound.md");
        std::fs::write(
            &brief_path,
            "The Message Decision Pack is a local offline decision layer that improves agent handoffs.\nTry the Message Decision Pack today.\nNever sell the Message Decision Pack as the product.\nTry the Message Decision Pack today; details live in .mdp/cards.\nLoaded card: .mdp/cards/positioning.yaml\nmdp.message-brief.v0 helps teams.\n",
        )
        .expect("brief should be writable");
        let traces_dir = root.join(".mdp/traces");
        std::fs::create_dir_all(&traces_dir).expect("trace directory should be writable");
        std::fs::write(
            traces_dir.join("outbound.json"),
            serde_json::to_string_pretty(&json!({
                "label": "Try the Message Decision Pack today.",
                "implementation_ref": "mdp.fit.v0"
            }))
            .expect("trace should serialize"),
        )
        .expect("trace should be writable");
        std::fs::write(
            traces_dir.join("trace-metadata.json"),
            serde_json::to_string_pretty(&json!({
                "label": "Loaded card: .mdp/cards/positioning.yaml",
                "runtime_ref": "mdp.fit.v0"
            }))
            .expect("trace metadata should serialize"),
        )
        .expect("trace metadata should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        let contamination_paths = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter(|issue| issue["code"] == "target_contamination_internal_vocabulary")
            .filter_map(|issue| issue["path"].as_str())
            .collect::<BTreeSet<_>>();
        assert!(contamination_paths.contains(".mdp/prompts/claims-proof.yaml#/instructions/0"));
        assert!(contamination_paths.contains(".mdp/prompts/hooks.yaml#/description"));
        assert!(contamination_paths.contains(".mdp/briefs/outbound.md:1"));
        assert!(contamination_paths.contains(".mdp/briefs/outbound.md:2"));
        assert!(!contamination_paths.contains(".mdp/briefs/outbound.md:3"));
        assert!(contamination_paths.contains(".mdp/briefs/outbound.md:4"));
        assert!(!contamination_paths.contains(".mdp/briefs/outbound.md:5"));
        assert!(contamination_paths.contains(".mdp/briefs/outbound.md:6"));
        assert!(contamination_paths.contains(".mdp/traces/outbound.json#/label"));
        assert!(!contamination_paths.contains(".mdp/traces/trace-metadata.json#/label"));
        assert!(!contamination_paths.contains(".mdp/prompts/gaps.yaml#/instructions/0"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn target_name_with_internal_token_does_not_mask_other_internal_vocabulary() {
        let root = targeted_pack("Acme MDP", &[]);
        let prompt_path = root.join(".mdp/prompts/hooks.yaml");
        let raw = std::fs::read_to_string(&prompt_path).expect("prompt should be readable");
        let mut prompt: YamlValue = serde_yaml::from_str(&raw).expect("prompt should parse");
        prompt["description"] = YamlValue::String(
            "Position Acme MDP as a Message Decision Pack for agent handoffs.".to_string(),
        );
        std::fs::write(
            &prompt_path,
            serde_yaml::to_string(&prompt).expect("prompt should serialize"),
        )
        .expect("prompt should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| {
                    issue["code"] == "target_contamination_internal_vocabulary"
                        && issue["path"] == ".mdp/prompts/hooks.yaml#/description"
                }),
            "internal vocabulary outside the active target name must remain visible: {}",
            result["issues"]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_double_negation_that_reauthorizes_internal_positioning() {
        let root = targeted_pack("Company B", &[]);
        let prompt_path = root.join(".mdp/prompts/claims-proof.yaml");
        let raw = std::fs::read_to_string(&prompt_path).expect("prompt should be readable");
        let mut prompt: YamlValue = serde_yaml::from_str(&raw).expect("prompt should parse");
        prompt["instructions"][0] = YamlValue::String(
            "Do not avoid positioning MDP as the product for Company B.".to_string(),
        );
        std::fs::write(
            &prompt_path,
            serde_yaml::to_string(&prompt).expect("prompt should serialize"),
        )
        .expect("prompt should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| {
                    issue["code"] == "target_contamination_internal_vocabulary"
                        && issue["path"] == ".mdp/prompts/claims-proof.yaml#/instructions/0"
                })
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn internal_contract_and_command_receipts_are_not_positioning() {
        for text in [
            "mdp.prompt-output.v0",
            "mdp.sample-leads.v0",
            "mdp.message-brief.v0",
            "mdp.context.v0",
            "Run mdp validate-prompt-output before accepting this output.",
            "Use mdp --json brief as the machine source of truth.",
            "Loaded .mdp/cards/positioning.yaml.",
        ] {
            let external_text = strip_internal_implementation_tokens(text, true);
            assert!(!contains_term(&external_text, "MDP"), "{text}");
        }
        assert!(contains_term(
            &strip_internal_implementation_tokens("MDP helps teams.", false),
            "MDP"
        ));
        assert!(contains_term(
            &strip_internal_implementation_tokens("mdp.message-brief.v0 helps teams.", false),
            "MDP"
        ));
    }

    #[test]
    fn generated_samples_and_readable_brief_preserve_target_isolation() {
        let root = targeted_pack("Company B", &["Company A".to_string()]);
        let fixtures =
            crate::commands::sample_leads(&root, "Operator", "outbound copy fixture", 2, 0)
                .expect("target-aware sample leads should generate");
        std::fs::write(
            root.join("examples/sample-leads.json"),
            serde_json::to_string_pretty(&fixtures).expect("fixtures should serialize"),
        )
        .expect("fixtures should be writable");

        let prospect_path = root.join("examples/prospect-row.json");
        let brief = crate::commands::prospect_brief_with_context(
            &root,
            &prospect_path,
            "linkedin",
            Some("outbound-copy-brief"),
            true,
        )
        .expect("target-aware brief should generate");
        std::fs::write(
            root.join(".mdp/briefs/prospect.md"),
            crate::commands::render_readable_prospect_brief(&brief),
        )
        .expect("readable brief should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        let contamination = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter(|issue| {
                matches!(
                    issue["code"].as_str(),
                    Some(
                        "target_contamination_internal_vocabulary"
                            | "target_contamination_excluded_term"
                    )
                )
            })
            .filter_map(|issue| issue["path"].as_str())
            .filter(|path| {
                path.starts_with("examples/sample-leads.json")
                    || path.starts_with(".mdp/briefs/prospect.md")
            })
            .collect::<Vec<_>>();
        assert!(
            contamination.is_empty(),
            "generated files must remain target-isolated: {contamination:?}"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_requires_direct_source_claim_for_external_target_terms() {
        let root = targeted_pack("Company B", &[]);
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["target"]["external_terms"] = YamlValue::Sequence(vec![
            YamlValue::String("Company B".to_string()),
            YamlValue::String("AI-powered revenue growth".to_string()),
        ]);
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| {
                    issue["code"] == "target_external_term_source_missing"
                        && issue["path"] == ".mdp/manifest.yaml#/target/external_terms/1"
                })
        );

        let _ = std::fs::remove_dir_all(root);
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
            12
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn targeted_starter_needing_review_is_not_activation_ready() {
        let root = targeted_pack("Company B", &[]);

        let result = validate_pack(&root).expect("validate should return diagnostics");

        assert_eq!(
            result["profile"]["activation_ready"], false,
            "needs-review activation must not appear ready: {}",
            result["profile"]
        );
        assert!(
            result["profile"]["jobs"]
                .as_array()
                .expect("jobs array")
                .iter()
                .all(|job| job["activation_ready"] == false),
            "needs-review activation must block every job: {}",
            result["profile"]["jobs"]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn portfolio_eval_coverage_rejects_vacuous_scope_fixtures() {
        let root = temp_pack("portfolio-vacuous-evals");
        let eval_dir = root.join(".mdp").join("evals");
        std::fs::remove_dir_all(&eval_dir).expect("starter eval directory should be removable");
        std::fs::create_dir_all(&eval_dir).expect("eval directory should be recreated");
        std::fs::write(
            eval_dir.join("selected.yaml"),
            "id: selected\ncommand: route\npersona: PMM\njob: portfolio scope example\nscope:\n- product=local-cli\n",
        )
        .expect("selected fixture should be writable");
        std::fs::write(
            eval_dir.join("missing.yaml"),
            "id: missing\ncommand: route\npersona: PMM\njob: portfolio scope example\nexpect_draft_status: blocked\n",
        )
        .expect("missing fixture should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        assert!(
            result["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| issue["code"] == "portfolio_scope_eval_coverage_missing")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_distinguishes_profile_metadata_from_activation() {
        let root = temp_pack("profile-metadata-only");
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
                "  gaps:\n    cards:\n    - gaps\n    evals:\n    - fit-insufficient-context\n    - brief-insufficient-context\n    - account-context-missing\n    - account-only-no-draft\n    - decision-input-contract\n",
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
            "attribute_definitions:",
            "attribute_definitions:\n    renewal date:\n      type: string\n      format: month\n    fiscal_year_override:\n      type: integer\n      enum:\n      - \"2027\"\n    close_date:\n      type: string\n      enumm: []",
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
    fn validate_reports_bad_profile_job_skill_bindings() {
        let root = temp_pack("profile-job-skill-bindings");
        let manifest_path = root.join(".mdp").join("manifest.yaml");
        let mut raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        raw = raw.replace(
            "  id: gtm\n  label: GTM Messaging",
            "  id: ''\n  unknown_profile_field: ignored\n  label: GTM Messaging",
        );
        raw = raw.replace(
            "- id: prospect-fit-or-brief\n  skill_id: mdp-gtm-brief",
            "- id: prospect-fit-or-brief\n  skill_id: ''",
        );
        raw = raw.replace(
            "- id: outbound-copy-brief\n  skill_id: mdp-gtm-brief",
            "- id: outbound-copy-brief\n  skill_id: unknown-skill",
        );
        raw = raw.replace(
            "- id: outbound-copy-review\n  skill_id: mdp-gtm-brief",
            "- id: outbound-copy-review\n  skill_id: mdp-proposal-review",
        );
        raw = raw.replace(
            "profile_eval:",
            "- id: custom-job\n  skill_id: mdp-pack-review\n  required_primitives: []\nprofile_eval:",
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
        assert!(codes.contains(&"profile_job_skill_id_empty"));
        assert!(codes.contains(&"profile_job_skill_unknown"));
        assert!(codes.contains(&"profile_job_route_incompatible"));
        assert!(codes.contains(&"profile_job_route_unknown"));

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
    fn persona_references_report_deterministic_paths_without_duplicate_noise() {
        let root = temp_pack("persona-reference-integrity");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["cards"][0]["personas"] =
            serde_yaml::from_str("- Architect\n- architect\n").expect("personas should parse");
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        for (card_name, persona) in [
            ("personas.yaml", "Architect"),
            ("positioning.yaml", "Marketer"),
        ] {
            let card_path = root.join(".mdp/cards").join(card_name);
            let raw = std::fs::read_to_string(&card_path).expect("card should be readable");
            let mut card: YamlValue = serde_yaml::from_str(&raw).expect("card should parse");
            card["personas"] =
                serde_yaml::from_str(&format!("- {persona}\n- {}\n", persona.to_lowercase()))
                    .expect("card personas should parse");
            card["entries"][0]["applies_to"] =
                serde_yaml::from_str(&format!("- {persona}\n- {}\n", persona.to_lowercase()))
                    .expect("entry applicability should parse");
            std::fs::write(
                card_path,
                serde_yaml::to_string(&card).expect("card should serialize"),
            )
            .expect("card should be writable");
        }

        let result = validate_pack(&root).expect("validate should return diagnostics");
        let persona_issues = result["issues"]
            .as_array()
            .expect("issues")
            .iter()
            .filter(|issue| {
                issue["code"] == "manifest_card_persona_undeclared"
                    || issue["code"] == "card_persona_undeclared"
                    || issue["code"] == "card_entry_applies_to_persona_undeclared"
            })
            .map(|issue| (issue["code"].clone(), issue["path"].clone()))
            .collect::<Vec<_>>();

        assert_eq!(
            persona_issues,
            vec![
                (
                    json!("manifest_card_persona_undeclared"),
                    json!(".mdp/manifest.yaml#/cards/0/personas/0")
                ),
                (
                    json!("card_persona_undeclared"),
                    json!(".mdp/cards/personas.yaml#/personas/0")
                ),
                (
                    json!("card_entry_applies_to_persona_undeclared"),
                    json!(".mdp/cards/personas.yaml#/entries/0/applies_to/0")
                ),
                (
                    json!("card_persona_undeclared"),
                    json!(".mdp/cards/positioning.yaml#/personas/0")
                ),
                (
                    json!("card_entry_applies_to_persona_undeclared"),
                    json!(".mdp/cards/positioning.yaml#/entries/0/applies_to/0")
                ),
            ]
        );
        assert_eq!(
            result["valid"], true,
            "ordinary validation stays warning-compatible"
        );
        assert!(
            result["issues"]
                .as_array()
                .expect("issues")
                .iter()
                .filter(|issue| {
                    issue["code"] == "manifest_card_persona_undeclared"
                        || issue["code"] == "card_persona_undeclared"
                        || issue["code"] == "card_entry_applies_to_persona_undeclared"
                })
                .all(|issue| issue["severity"] == "warning")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn persona_references_match_case_insensitively_and_ignore_empty_or_prose_values() {
        let root = temp_pack("persona-reference-case-and-prose");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["cards"][0]["personas"] =
            serde_yaml::from_str("- pmm\n- ''\n").expect("card personas should parse");
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let card_path = root.join(".mdp/cards/personas.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("card should be readable");
        let mut card: YamlValue = serde_yaml::from_str(&raw).expect("card should parse");
        card["personas"] =
            serde_yaml::from_str("- pMm\n- '  '\n").expect("card personas should parse");
        card["entries"][0]["applies_to"] =
            serde_yaml::from_str("- pmm\n- ''\n").expect("entry applicability should parse");
        card["entries"][0]["title"] = YamlValue::String("For solutions architects".into());
        card["entries"][0]["body"] =
            YamlValue::String("A marketer may collaborate on this prose-only example.".into());
        std::fs::write(
            card_path,
            serde_yaml::to_string(&card).expect("card should serialize"),
        )
        .expect("card should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        assert!(
            result["issues"]
                .as_array()
                .expect("issues")
                .iter()
                .all(|issue| {
                    !matches!(
                        issue["code"].as_str(),
                        Some(
                            "manifest_card_persona_undeclared"
                                | "card_persona_undeclared"
                                | "card_entry_applies_to_persona_undeclared"
                        )
                    )
                })
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn persona_references_accept_declared_operator_roles_and_target_personas() {
        let root = temp_pack("persona-reference-routable-roles");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut manifest: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        manifest["operator_roles"] =
            serde_yaml::from_str("- GTM Engineering\n- Operator\n").expect("roles should parse");
        manifest["target_personas"] =
            serde_yaml::from_str("- Target Buyer\n").expect("target personas should parse");
        manifest["cards"][0]["personas"] = serde_yaml::from_str("- Operator\n- target buyer\n")
            .expect("card personas should parse");
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let card_path = root.join(".mdp/cards/personas.yaml");
        let raw = std::fs::read_to_string(&card_path).expect("card should be readable");
        let mut card: YamlValue = serde_yaml::from_str(&raw).expect("card should parse");
        card["personas"] = serde_yaml::from_str("- operator\n- TARGET BUYER\n")
            .expect("card personas should parse");
        card["entries"][0]["applies_to"] = serde_yaml::from_str("- Operator\n- target buyer\n")
            .expect("entry applicability should parse");
        std::fs::write(
            card_path,
            serde_yaml::to_string(&card).expect("card should serialize"),
        )
        .expect("card should be writable");

        let result = validate_pack(&root).expect("validate should return diagnostics");
        assert!(
            result["issues"]
                .as_array()
                .expect("issues")
                .iter()
                .all(|issue| {
                    !matches!(
                        issue["code"].as_str(),
                        Some(
                            "manifest_card_persona_undeclared"
                                | "card_persona_undeclared"
                                | "card_entry_applies_to_persona_undeclared"
                        )
                    )
                })
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

    #[test]
    fn decision_input_contract_rejects_missing_normalization_prompt() {
        let manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let mut issues = Vec::new();

        validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);

        assert!(
            issues
                .iter()
                .any(|issue| issue["code"] == "decision_input_normalization_prompt_missing")
        );
    }

    #[test]
    fn decision_input_contract_rejects_legacy_normalization_prompt_binding() {
        let root = temp_clay_pack("legacy-normalization-prompt");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replacen(
                "prompt: prompts/normalize-prospect.yaml",
                "prompt: prompts/hooks.yaml",
                1,
            ),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validation should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| {
                    issue["code"] == "decision_input_normalization_prompt_contract_mismatch"
                        && issue["severity"] == "error"
                })
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn decision_input_normalization_rejects_inline_schema_replacement() {
        let root = temp_clay_pack("inline-decision-input-schema");
        let prompt_path = root.join(".mdp/prompts/normalize-prospect.yaml");
        let raw = std::fs::read_to_string(&prompt_path).expect("prompt should be readable");
        std::fs::write(
            &prompt_path,
            raw.replace(
                "  schema_ref: mdp.normalized-decision-input.v2\n",
                "  schema:\n    type: object\n    additionalProperties: false\n    properties: {}\n    required: []\n",
            ),
        )
        .expect("prompt should be writable");

        let result = validate_pack(&root).expect("validation should return diagnostics");
        let codes = result["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .filter_map(|issue| issue["code"].as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(result["valid"], false);
        assert!(codes.contains("decision_input_prompt_schema_ref_required"));
        assert!(codes.contains("decision_input_prompt_inline_schema_unsupported"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn decision_input_contract_rejects_prompt_version_drift() {
        let root = temp_clay_pack("decision-input-prompt-version-drift");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replacen(
                "prompt_version: clay-self-serve-enterprise-expansion.v4",
                "prompt_version: bogus.v999",
                1,
            ),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validation should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| {
                    issue["code"] == "decision_input_normalization_prompt_version_mismatch"
                        && issue["severity"] == "error"
                })
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn decision_input_contract_rejects_prompt_id_binding() {
        let root = temp_clay_pack("decision-input-prompt-id-binding");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replacen(
                "prompt: prompts/normalize-prospect.yaml",
                "prompt: normalize-prospect-row",
                1,
            ),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("validation should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| {
                    issue["code"] == "decision_input_normalization_prompt_path_required"
                        && issue["severity"] == "error"
                })
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn required_provenance_must_bind_to_a_source_attempt() {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let attribute = manifest.decision_input_contracts[0]
            .attributes
            .iter_mut()
            .find(|attribute| attribute.id == "company_name")
            .expect("Clay example should include company_name");
        attribute.provenance.required_fields =
            vec![crate::models::DecisionInputProvenanceField::Excerpt];
        let mut issues = Vec::new();

        validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);

        assert!(issues.iter().any(|issue| {
            issue["code"] == "decision_input_provenance_attempt_id_required"
                && issue["severity"] == "error"
        }));
    }

    #[test]
    fn required_freshness_for_temporal_values_requires_provenance_observed_at() {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let attribute = manifest.decision_input_contracts[0]
            .attributes
            .iter_mut()
            .find(|attribute| attribute.id == "last_meaningful_touch")
            .expect("Clay example should include last_meaningful_touch");
        attribute.provenance.required_fields = vec![
            crate::models::DecisionInputProvenanceField::AttemptId,
            crate::models::DecisionInputProvenanceField::SourceClass,
            crate::models::DecisionInputProvenanceField::SourceLocator,
        ];
        let mut issues = Vec::new();

        validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);

        assert!(issues.iter().any(|issue| {
            issue["code"] == "decision_input_freshness_provenance_timestamp_required"
                && issue["severity"] == "error"
        }));
    }

    #[test]
    fn duplicate_input_contract_ids_are_validation_errors() {
        let duplicate = InputContract {
            id: "duplicate-machine-contract".to_string(),
            ..InputContract::default()
        };
        let mut issues = Vec::new();

        validate_input_contracts(
            &[duplicate.clone(), duplicate],
            &BTreeSet::new(),
            &PromptInventory::default(),
            ".mdp/manifest.yaml#/input_contracts",
            &mut issues,
        );

        assert!(issues.iter().any(|issue| {
            issue["code"] == "profile_input_contract_duplicate" && issue["severity"] == "error"
        }));
    }

    #[test]
    fn decision_input_equals_applicability_requires_one_value() {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let condition = &mut manifest.decision_input_contracts[0]
            .attributes
            .iter_mut()
            .find(|attribute| attribute.id == "latest_support_context")
            .expect("Clay example should include latest_support_context")
            .applies_when[0];
        condition.values.push("another-value".to_string());
        let mut issues = Vec::new();

        validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);

        assert!(
            issues
                .iter()
                .any(|issue| issue["code"] == "decision_input_applicability_equals_cardinality")
        );
    }

    #[test]
    fn decision_input_applicability_rejects_out_of_domain_values() {
        for operator in [
            crate::models::DecisionInputConditionOperator::Equals,
            crate::models::DecisionInputConditionOperator::NotEquals,
            crate::models::DecisionInputConditionOperator::In,
        ] {
            let mut manifest =
                read_manifest(&clay_example_root()).expect("Clay example manifest should load");
            let condition = &mut manifest.decision_input_contracts[0]
                .attributes
                .iter_mut()
                .find(|attribute| attribute.id == "latest_support_context")
                .expect("Clay example should include latest_support_context")
                .applies_when[0];
            condition.operator = operator.clone();
            condition.values = vec!["not-a-real-enum-value".to_string()];
            let mut issues = Vec::new();

            validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);

            assert!(
                issues.iter().any(|issue| {
                    issue["code"] == "decision_input_applicability_value_out_of_domain"
                        && issue["severity"] == "error"
                }),
                "{operator:?} must reject enum operands outside the dependency domain"
            );
        }

        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let condition = &mut manifest.decision_input_contracts[0]
            .attributes
            .iter_mut()
            .find(|attribute| attribute.id == "latest_support_context")
            .expect("Clay example should include latest_support_context")
            .applies_when[0];
        condition.operator = crate::models::DecisionInputConditionOperator::Exists;
        condition.values = vec!["ignored-value".to_string()];
        let mut issues = Vec::new();
        validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);
        assert!(issues.iter().any(|issue| {
            issue["code"] == "decision_input_applicability_exists_values_forbidden"
                && issue["severity"] == "error"
        }));
    }

    #[test]
    fn decision_input_attribute_rejects_duplicate_source_classes() {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let attribute = manifest.decision_input_contracts[0]
            .attributes
            .iter_mut()
            .find(|attribute| attribute.id == "company_domain")
            .expect("Clay example should include company_domain");
        attribute
            .source_classes
            .push(attribute.source_classes[0].clone());
        let mut issues = Vec::new();

        validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);

        assert!(issues.iter().any(|issue| {
            issue["code"] == "decision_input_attribute_source_class_duplicate"
                && issue["severity"] == "error"
        }));
    }

    #[test]
    fn decision_input_nested_unknown_fields_fail_closed() {
        let manifest_path = clay_example_root().join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(manifest_path).expect("manifest should be readable");
        let mut value: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        let contract = &mut value["decision_input_contracts"][0];
        contract["normalization"]["prompt_versoin"] = YamlValue::String("ignored typo".to_string());
        let attribute = &mut contract["attributes"][0];
        attribute["questoin"] = YamlValue::String("ignored typo".to_string());
        attribute["value"]["tyep"] = YamlValue::String("string".to_string());
        attribute["provenance"]["required_fieldz"] = YamlValue::Sequence(Vec::new());
        attribute["confidence"]["minimun"] = YamlValue::Number(90.into());
        attribute["freshness"]["max_age_dayz"] = YamlValue::Number(30.into());
        attribute["status_behavior"]["not_foud"] = YamlValue::String("gap".to_string());
        let mut issues = Vec::new();

        validate_decision_input_contract_shapes(
            yaml_get(&value, "decision_input_contracts"),
            ".mdp/manifest.yaml#/decision_input_contracts",
            &mut issues,
        );

        let codes = issues
            .iter()
            .filter_map(|issue| issue["code"].as_str())
            .collect::<BTreeSet<_>>();
        for expected in [
            "manifest_decision_input_normalization_unknown_field",
            "manifest_decision_input_attribute_unknown_field",
            "manifest_decision_input_value_unknown_field",
            "manifest_decision_input_provenance_unknown_field",
            "manifest_decision_input_confidence_unknown_field",
            "manifest_decision_input_freshness_unknown_field",
            "manifest_decision_input_status_behavior_unknown_field",
        ] {
            assert!(
                codes.contains(expected),
                "missing nested typo issue {expected}"
            );
        }
        assert!(
            issues.iter().all(|issue| issue["severity"] == "error"),
            "unknown decision input contract fields must invalidate requirements"
        );
    }

    #[test]
    fn decision_input_missing_required_fields_fail_closed() {
        let manifest_path = clay_example_root().join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(manifest_path).expect("manifest should be readable");
        let mut value: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        let contract = &mut value["decision_input_contracts"][0];
        contract["normalization"]
            .as_mapping_mut()
            .expect("normalization should be an object")
            .remove(YamlValue::String("normalized_schema_ref".to_string()));
        let attribute = &mut contract["attributes"][0];
        for field in [
            "provenance",
            "confidence",
            "freshness",
            "sensitivity",
            "source_classes",
        ] {
            attribute
                .as_mapping_mut()
                .expect("attribute should be an object")
                .remove(YamlValue::String(field.to_string()));
        }
        let nested_policy_attribute = &mut contract["attributes"][1];
        nested_policy_attribute["provenance"]
            .as_mapping_mut()
            .expect("provenance should be an object")
            .remove(YamlValue::String("required_fields".to_string()));
        nested_policy_attribute["confidence"]
            .as_mapping_mut()
            .expect("confidence should be an object")
            .remove(YamlValue::String("required".to_string()));
        nested_policy_attribute["freshness"]
            .as_mapping_mut()
            .expect("freshness should be an object")
            .remove(YamlValue::String("allow_unknown".to_string()));
        let mut issues = Vec::new();

        validate_decision_input_contract_shapes(
            yaml_get(&value, "decision_input_contracts"),
            ".mdp/manifest.yaml#/decision_input_contracts",
            &mut issues,
        );

        for (code, field) in [
            (
                "manifest_decision_input_normalization_required_field_missing",
                "normalized_schema_ref",
            ),
            (
                "manifest_decision_input_attribute_required_field_missing",
                "provenance",
            ),
            (
                "manifest_decision_input_attribute_required_field_missing",
                "confidence",
            ),
            (
                "manifest_decision_input_attribute_required_field_missing",
                "freshness",
            ),
            (
                "manifest_decision_input_attribute_required_field_missing",
                "sensitivity",
            ),
            (
                "manifest_decision_input_attribute_required_field_missing",
                "source_classes",
            ),
            (
                "manifest_decision_input_provenance_required_field_missing",
                "required_fields",
            ),
            (
                "manifest_decision_input_confidence_required_field_missing",
                "required",
            ),
            (
                "manifest_decision_input_freshness_required_field_missing",
                "allow_unknown",
            ),
        ] {
            assert!(
                issues.iter().any(|issue| {
                    issue["code"] == code
                        && issue["path"]
                            .as_str()
                            .is_some_and(|path| path.ends_with(field))
                        && issue["severity"] == "error"
                }),
                "missing required-field issue {code} for {field}"
            );
        }
    }

    #[test]
    fn strict_validation_rejects_omitted_decision_input_policy_object() {
        let root = temp_clay_pack("missing-decision-input-policy");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        let mut value: YamlValue = serde_yaml::from_str(&raw).expect("manifest should parse");
        value["decision_input_contracts"][0]["attributes"][0]
            .as_mapping_mut()
            .expect("attribute should be an object")
            .remove(YamlValue::String("provenance".to_string()));
        std::fs::write(
            &manifest_path,
            serde_yaml::to_string(&value).expect("manifest should serialize"),
        )
        .expect("manifest should be writable");

        let result = validate_pack(&root).expect("strict validation should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| {
                    issue["code"] == "manifest_decision_input_attribute_required_field_missing"
                        && issue["path"]
                            .as_str()
                            .is_some_and(|path| path.ends_with("/provenance"))
                })
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_rejects_nested_decision_input_freshness_typo() {
        let root = temp_clay_pack("nested-freshness-typo");
        let manifest_path = root.join(".mdp/manifest.yaml");
        let raw = std::fs::read_to_string(&manifest_path).expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replacen("max_age_days: 180", "max_age_dayz: 180", 1),
        )
        .expect("manifest typo fixture should be writable");

        let result = validate_pack(&root).expect("validation should return diagnostics");

        assert_eq!(result["valid"], false);
        assert!(
            result["issues"]
                .as_array()
                .expect("issues array")
                .iter()
                .any(|issue| {
                    issue["code"] == "manifest_decision_input_freshness_unknown_field"
                        && issue["severity"] == "error"
                })
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn decision_input_comparisons_reject_boolean_and_numeric_dependencies() {
        for value_type in ["boolean", "number"] {
            let mut manifest =
                read_manifest(&clay_example_root()).expect("Clay example manifest should load");
            let contract = &mut manifest.decision_input_contracts[0];
            contract
                .attributes
                .iter_mut()
                .find(|attribute| attribute.id == "open_support_escalation")
                .expect("Clay example should include open_support_escalation")
                .value
                .value_type = Some(value_type.to_string());
            let mut issues = Vec::new();

            validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);

            assert!(
                issues.iter().any(|issue| {
                    issue["code"] == "decision_input_applicability_operand_type_unsupported"
                }),
                "{value_type} comparison dependency must fail validation"
            );
        }
    }

    #[test]
    fn decision_input_applicability_rejects_two_node_cycles() {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let contract = &mut manifest.decision_input_contracts[0];
        contract
            .attributes
            .iter_mut()
            .find(|attribute| attribute.id == "open_support_escalation")
            .expect("Clay example should include open_support_escalation")
            .applies_when = vec![crate::models::DecisionInputCondition {
            attribute: "latest_support_context".to_string(),
            operator: crate::models::DecisionInputConditionOperator::Exists,
            values: Vec::new(),
        }];
        let mut issues = Vec::new();

        validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);

        assert!(
            issues
                .iter()
                .any(|issue| issue["code"] == "decision_input_applicability_cycle")
        );
    }

    #[test]
    fn decision_input_applicability_rejects_three_node_cycles() {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let contract = &mut manifest.decision_input_contracts[0];
        contract
            .attributes
            .iter_mut()
            .find(|attribute| attribute.id == "enterprise_eligibility")
            .expect("Clay example should include enterprise_eligibility")
            .applies_when = vec![crate::models::DecisionInputCondition {
            attribute: "latest_support_context".to_string(),
            operator: crate::models::DecisionInputConditionOperator::Exists,
            values: Vec::new(),
        }];
        contract
            .attributes
            .iter_mut()
            .find(|attribute| attribute.id == "open_support_escalation")
            .expect("Clay example should include open_support_escalation")
            .applies_when = vec![crate::models::DecisionInputCondition {
            attribute: "enterprise_eligibility".to_string(),
            operator: crate::models::DecisionInputConditionOperator::Exists,
            values: Vec::new(),
        }];
        let mut issues = Vec::new();

        validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);

        assert!(
            issues
                .iter()
                .any(|issue| issue["code"] == "decision_input_applicability_cycle")
        );
    }

    #[test]
    fn decision_input_applicability_allows_acyclic_chains() {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let contract = &mut manifest.decision_input_contracts[0];
        contract
            .attributes
            .iter_mut()
            .find(|attribute| attribute.id == "enterprise_eligibility")
            .expect("Clay example should include enterprise_eligibility")
            .applies_when = vec![crate::models::DecisionInputCondition {
            attribute: "company_name".to_string(),
            operator: crate::models::DecisionInputConditionOperator::Exists,
            values: Vec::new(),
        }];
        let mut issues = Vec::new();

        validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);

        assert!(
            issues
                .iter()
                .all(|issue| issue["code"] != "decision_input_applicability_cycle")
        );
    }

    #[test]
    fn decision_input_applicability_rejects_unresolved_dependency_classes() {
        for requirement in [
            DecisionInputRequirement::Optional,
            DecisionInputRequirement::Conditional,
        ] {
            let mut manifest =
                read_manifest(&clay_example_root()).expect("Clay example manifest should load");
            let dependency = manifest.decision_input_contracts[0]
                .attributes
                .iter_mut()
                .find(|attribute| attribute.id == "open_support_escalation")
                .expect("Clay example should include open_support_escalation");
            dependency.requirement = requirement;
            let mut issues = Vec::new();

            validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);

            assert!(issues.iter().any(|issue| {
                issue["code"] == "decision_input_applicability_dependency_not_readiness_required"
            }));
        }
    }

    #[test]
    fn decision_input_output_path_rejects_composite_signals() {
        assert!(!valid_decision_input_output_path("signals"));
        assert!(!valid_decision_input_output_path("signals.0"));
        assert!(valid_decision_input_output_path(
            "attributes.reviewed_signal"
        ));
    }

    #[test]
    fn signal_projection_validation_rejects_duplicate_ids_unknown_contributors_and_invalid_bounds()
    {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let contract = &mut manifest.decision_input_contracts[0];
        let mut projection = crate::models::DecisionInputSignalProjection {
            id: "hiring-change".to_string(),
            kind: "hiring_change".to_string(),
            roles: vec![crate::models::DecisionInputSignalRole::WhyNow],
            contributor_attribute_ids: vec!["missing_attribute".to_string()],
            value: ValueContract {
                value_type: Some("boolean".to_string()),
                ..ValueContract::default()
            },
            cardinality: crate::models::DecisionInputSignalCardinality { min: 2, max: 1 },
            conflict_policy: crate::models::DecisionInputSignalConflictPolicy::RequireAgreement,
            decision_effects: vec![DecisionInputDecisionEffect::Brief],
        };
        contract.signal_projections.push(projection.clone());
        projection.kind = "other_kind".to_string();
        contract.signal_projections.push(projection);
        let mut issues = Vec::new();

        validate_decision_input_signal_projections(contract, "test", &mut issues);
        let codes = issues
            .iter()
            .filter_map(|issue| issue["code"].as_str())
            .collect::<BTreeSet<_>>();

        assert!(codes.contains("decision_input_signal_projection_duplicate"));
        assert!(codes.contains("decision_input_signal_contributor_undeclared"));
        assert!(codes.contains("decision_input_signal_cardinality_invalid"));
    }

    #[test]
    fn signal_projection_validation_accepts_profile_defined_kind_and_zero_minimum() {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let contract = &mut manifest.decision_input_contracts[0];
        let contributor = contract.attributes[0].id.clone();
        contract
            .signal_projections
            .push(crate::models::DecisionInputSignalProjection {
                id: "hiring-change".to_string(),
                kind: "profile_specific_hiring_change".to_string(),
                roles: vec![crate::models::DecisionInputSignalRole::WhyNow],
                contributor_attribute_ids: vec![contributor],
                value: ValueContract {
                    value_type: Some("boolean".to_string()),
                    ..ValueContract::default()
                },
                cardinality: crate::models::DecisionInputSignalCardinality { min: 0, max: 4 },
                conflict_policy: crate::models::DecisionInputSignalConflictPolicy::RequireAgreement,
                decision_effects: vec![DecisionInputDecisionEffect::Brief],
            });
        let mut issues = Vec::new();

        validate_decision_input_signal_projections(contract, "test", &mut issues);

        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn signal_aware_contract_requires_v2_normalized_schema_discriminator() {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let contract = &mut manifest.decision_input_contracts[0];
        contract.normalization.normalized_schema_ref =
            NORMALIZED_DECISION_INPUT_CONTRACT.to_string();
        contract
            .signal_projections
            .push(crate::models::DecisionInputSignalProjection {
                id: "signal-v2".to_string(),
                kind: "profile_signal".to_string(),
                roles: vec![crate::models::DecisionInputSignalRole::Fit],
                contributor_attribute_ids: vec![contract.attributes[0].id.clone()],
                value: ValueContract {
                    value_type: Some("string".to_string()),
                    ..ValueContract::default()
                },
                cardinality: crate::models::DecisionInputSignalCardinality { min: 0, max: 2 },
                conflict_policy: crate::models::DecisionInputSignalConflictPolicy::RequireAgreement,
                decision_effects: vec![DecisionInputDecisionEffect::Fit],
            });

        let mut issues = Vec::new();
        validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);
        assert!(
            issues
                .iter()
                .any(|issue| { issue["code"] == "decision_input_normalized_schema_unknown" })
        );

        manifest.decision_input_contracts[0]
            .normalization
            .normalized_schema_ref = NORMALIZED_DECISION_INPUT_CONTRACT_V2.to_string();
        issues.clear();
        validate_decision_input_contracts(&manifest, &PromptInventory::default(), &mut issues);
        assert!(
            issues
                .iter()
                .all(|issue| { issue["code"] != "decision_input_normalized_schema_unknown" })
        );
    }

    #[test]
    fn signal_projection_validation_rejects_ambiguous_contributors() {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let contract = &mut manifest.decision_input_contracts[0];
        let duplicate = contract.attributes[0].clone();
        let contributor = duplicate.id.clone();
        contract.attributes.push(duplicate);
        contract
            .signal_projections
            .push(crate::models::DecisionInputSignalProjection {
                id: "ambiguous-projection".to_string(),
                kind: "profile_specific_kind".to_string(),
                roles: Vec::new(),
                contributor_attribute_ids: vec![contributor],
                value: ValueContract {
                    value_type: Some("string".to_string()),
                    ..ValueContract::default()
                },
                cardinality: crate::models::DecisionInputSignalCardinality { min: 0, max: 2 },
                conflict_policy: crate::models::DecisionInputSignalConflictPolicy::RequireAgreement,
                decision_effects: vec![DecisionInputDecisionEffect::HumanReview],
            });
        let mut issues = Vec::new();

        validate_decision_input_signal_projections(contract, "test", &mut issues);

        assert!(
            issues
                .iter()
                .any(|issue| issue["code"] == "decision_input_signal_contributor_ambiguous")
        );
    }

    #[test]
    fn validate_rejects_nested_decision_input_policy_typos() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-decision-input-policy-typo-{nonce}"));
        let pack_dir = root.join(".mdp");
        std::fs::create_dir_all(&pack_dir).expect("pack dir should be writable");
        let manifest_path = pack_dir.join("manifest.yaml");
        let raw = std::fs::read_to_string(clay_example_root().join(".mdp").join("manifest.yaml"))
            .expect("manifest should be readable");
        std::fs::write(
            &manifest_path,
            raw.replace("max_age_days: 90", "max_age_dayz: 90")
                .replace("required_fields:", "required_fieldz:")
                .replace("minimum: 90", "minimim: 90"),
        )
        .expect("manifest should be writable");
        let mut issues = Vec::new();

        validate_manifest_shape(&root, &mut issues);
        let codes = issues
            .iter()
            .map(|issue| issue["code"].as_str().expect("issue code"))
            .collect::<BTreeSet<_>>();

        assert!(
            issues
                .iter()
                .any(|issue| issue["severity"].as_str() == Some("error"))
        );
        assert!(codes.contains("manifest_decision_input_freshness_unknown_field"));
        assert!(codes.contains("manifest_decision_input_provenance_unknown_field"));
        assert!(codes.contains("manifest_decision_input_confidence_unknown_field"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn hard_gate_status_behavior_rejects_fail_open_provider_states() {
        let manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let mut attribute = manifest.decision_input_contracts[0]
            .attributes
            .iter()
            .find(|attribute| attribute.id == "do_not_contact")
            .expect("Clay example should include do_not_contact")
            .clone();
        attribute.status_behavior.insert(
            DecisionInputAttemptStatus::Blocked,
            DecisionInputDisposition::Accept,
        );
        let mut issues = Vec::new();

        validate_hard_gate_status_policy(&attribute, "test", &mut issues);

        assert!(
            issues
                .iter()
                .any(|issue| issue["code"] == "decision_input_hard_gate_status_behavior_unsafe")
        );
    }

    #[test]
    fn readiness_status_behavior_rejects_fail_open_missing_evidence() {
        let manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let mut required = manifest.decision_input_contracts[0]
            .attributes
            .iter()
            .find(|attribute| attribute.id == "company_domain")
            .expect("Clay example should include company_domain")
            .clone();
        required.status_behavior.insert(
            DecisionInputAttemptStatus::NotFound,
            DecisionInputDisposition::Accept,
        );
        let mut issues = Vec::new();

        validate_readiness_status_policy(&required, "test", &mut issues);

        assert!(issues.iter().any(|issue| issue["code"]
            == "decision_input_status_behavior_unsafe"
            && issue["severity"] == "error"));

        let mut optional = manifest.decision_input_contracts[0]
            .attributes
            .iter()
            .find(|attribute| attribute.id == "employee_band")
            .expect("Clay example should include employee_band")
            .clone();
        optional.status_behavior.insert(
            DecisionInputAttemptStatus::NotFound,
            DecisionInputDisposition::Accept,
        );
        let mut optional_issues = Vec::new();
        validate_readiness_status_policy(&optional, "test", &mut optional_issues);
        assert!(
            optional_issues.is_empty(),
            "optional absence must remain a valid nonblocking policy"
        );
    }

    #[test]
    fn readiness_alignment_rejects_optional_runtime_required_field() {
        let manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let mut attribute = manifest.decision_input_contracts[0]
            .attributes
            .iter()
            .find(|attribute| attribute.id == "company_name")
            .expect("Clay example should include company_name")
            .clone();
        attribute.requirement = DecisionInputRequirement::Optional;
        let required_attributes = manifest
            .lead_input_requirements
            .required_attributes
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let required_fields = manifest
            .lead_input_requirements
            .required_fields
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();

        let mismatches = decision_input_readiness_mismatches(
            &manifest,
            &attribute,
            &required_attributes,
            &required_fields,
        );

        assert!(
            mismatches
                .iter()
                .any(|mismatch| mismatch.code == "decision_input_readiness_requirement_conflict")
        );
    }

    #[test]
    fn readiness_alignment_rejects_attribute_value_contract_drift() {
        let manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let mut attribute = manifest.decision_input_contracts[0]
            .attributes
            .iter()
            .find(|attribute| attribute.id == "enterprise_eligibility")
            .expect("Clay example should include enterprise_eligibility")
            .clone();
        attribute.value.enum_values.push("unknown".to_string());
        let required_attributes = manifest
            .lead_input_requirements
            .required_attributes
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let required_fields = manifest
            .lead_input_requirements
            .required_fields
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();

        let mismatches = decision_input_readiness_mismatches(
            &manifest,
            &attribute,
            &required_attributes,
            &required_fields,
        );

        assert!(
            mismatches
                .iter()
                .any(|mismatch| mismatch.code == "decision_input_value_contract_mismatch")
        );
    }

    #[test]
    fn readiness_alignment_rejects_impossible_direct_prospect_types() {
        let manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let required_attributes = manifest
            .lead_input_requirements
            .required_attributes
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let required_fields = manifest
            .lead_input_requirements
            .required_fields
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();

        let mut string_to_boolean = manifest.decision_input_contracts[0]
            .attributes
            .iter()
            .find(|attribute| attribute.id == "company_name")
            .expect("Clay example should include company_name")
            .clone();
        string_to_boolean.output_path = "synthetic".to_string();
        assert!(
            decision_input_readiness_mismatches(
                &manifest,
                &string_to_boolean,
                &required_attributes,
                &required_fields,
            )
            .iter()
            .any(|mismatch| mismatch.code == "decision_input_prospect_output_type_mismatch")
        );

        let mut boolean_to_string = manifest.decision_input_contracts[0]
            .attributes
            .iter()
            .find(|attribute| attribute.id == "do_not_contact")
            .expect("Clay example should include do_not_contact")
            .clone();
        boolean_to_string.output_path = "company".to_string();
        boolean_to_string.value.value_type = Some("boolean".to_string());
        boolean_to_string.value.enum_values.clear();
        assert!(
            decision_input_readiness_mismatches(
                &manifest,
                &boolean_to_string,
                &required_attributes,
                &required_fields,
            )
            .iter()
            .any(|mismatch| mismatch.code == "decision_input_prospect_output_type_mismatch")
        );
    }

    #[test]
    fn job_composition_rejects_cross_contract_attribute_collisions() {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let mut duplicate = manifest.decision_input_contracts[0].clone();
        duplicate.id = "clay.audiences.duplicate".to_string();
        manifest.decision_input_contracts.push(duplicate);
        manifest.jobs[0]
            .decision_input_contracts
            .push("clay.audiences.duplicate".to_string());
        let mut issues = Vec::new();

        validate_job_decision_input_composition(&manifest, &mut issues);

        assert!(
            issues
                .iter()
                .any(|issue| issue["code"] == "decision_input_job_attribute_duplicate")
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue["code"] == "decision_input_job_output_path_duplicate")
        );
    }

    fn duplicate_contract_with_unique_projection(manifest: &Manifest) -> DecisionInputContract {
        let mut duplicate = manifest.decision_input_contracts[0].clone();
        duplicate.id = "clay.audiences.other-normalizer".to_string();
        duplicate.normalization.prompt = "prompts/other-normalize-prospect.yaml".to_string();
        duplicate.normalization.prompt_version = "other-normalizer.v1".to_string();
        for (index, attribute) in duplicate.attributes.iter_mut().enumerate() {
            attribute.id = format!("other_attribute_{index}");
            attribute.output_path = format!("attributes.other_attribute_{index}");
            attribute.applies_when.clear();
        }
        duplicate
    }

    #[test]
    fn job_composition_rejects_direct_normalization_mismatch() {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let duplicate = duplicate_contract_with_unique_projection(&manifest);
        manifest.jobs[0]
            .decision_input_contracts
            .push(duplicate.id.clone());
        manifest.decision_input_contracts.push(duplicate);
        let mut issues = Vec::new();

        validate_job_decision_input_composition(&manifest, &mut issues);

        assert!(
            issues
                .iter()
                .any(|issue| { issue["code"] == "decision_input_job_normalization_mismatch" })
        );
    }

    #[test]
    fn job_composition_rejects_inherited_normalization_mismatch() {
        let mut manifest =
            read_manifest(&clay_example_root()).expect("Clay example manifest should load");
        let duplicate = duplicate_contract_with_unique_projection(&manifest);
        let duplicate_id = duplicate.id.clone();
        manifest.decision_input_contracts.push(duplicate);
        manifest.input_contracts.push(InputContract {
            id: "other-normalization-input".to_string(),
            decision_input_contracts: vec![duplicate_id],
            ..InputContract::default()
        });
        manifest.jobs[0]
            .input_contracts
            .push("other-normalization-input".to_string());
        let mut issues = Vec::new();

        validate_job_decision_input_composition(&manifest, &mut issues);

        assert!(
            issues
                .iter()
                .any(|issue| { issue["code"] == "decision_input_job_normalization_mismatch" })
        );
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
