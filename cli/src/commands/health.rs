use crate::constants::{
    DEFAULT_DIR, FORMAT_NAME, FORMAT_VERSION, PROMPT_FORMAT_VERSION, PROMPT_OUTPUT_CONTRACT,
};
use crate::models::{CardKind, PromptFile};
use crate::pack_io::{
    display_pack_path, read_card, read_card_by_id, read_manifest, read_prompt, resolve_pack_path,
};
use crate::routing::select_cards;
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

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
    Ok(
        json!({"valid": issues.is_empty(), "manifest": format!("{DEFAULT_DIR}/manifest.yaml"), "cards": loaded_cards, "prompts": loaded_prompts, "issues": issues}),
    )
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
        init_pack(&root, "Example Message Pack", "gtm", true)
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
        assert_eq!(
            result["prompts"].as_array().expect("prompts array").len(),
            9
        );

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
