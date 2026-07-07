use crate::commands::health::issue;
use crate::commands::routing::check_claims;
use crate::models::{Card, CardKind, Entry, Manifest, PromptFile};
use crate::pack_io::{read_card, read_manifest, read_prompt, resolve_pack_path};
use crate::routing::select_cards;
use crate::utils::{resolve_persona_label, routable_persona};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const PROOF_OUTPUT_CONTRACT: &str = "mdp.proof-output.v0";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofOutputArtifact {
    contract: String,
    pack: ProofPack,
    #[serde(default)]
    route: Option<ProofRoute>,
    output: ProofOutput,
    coverage: ProofCoverage,
    #[serde(default)]
    segments: Vec<ProofSegment>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofPack {
    id: String,
    #[serde(default)]
    profile_id: Option<String>,
    #[serde(default)]
    pack_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofRoute {
    #[serde(default)]
    persona: Option<String>,
    #[serde(default)]
    job: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofOutput {
    kind: String,
    format: String,
    text: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofCoverage {
    mode: String,
    material_policy: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofSegment {
    id: String,
    kind: SegmentKind,
    text: String,
    #[serde(default)]
    material: Option<bool>,
    #[serde(default)]
    refs: Vec<ProofRef>,
    #[serde(default)]
    gap: Option<ProofGap>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SegmentKind {
    Claim,
    RequirementStatus,
    TemplateText,
    Gap,
    Connective,
    Formatting,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofGap {
    code: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum ProofRef {
    CardEntry {
        role: String,
        card_id: String,
        entry_id: String,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        primitive: Option<String>,
    },
    Source {
        role: String,
        source_id: String,
    },
    PromptInput {
        role: String,
        prompt_id: String,
        input_name: String,
    },
    InputContract {
        role: String,
        input_contract_id: String,
    },
    Route {
        role: String,
        persona: String,
        job: String,
    },
}

#[derive(Debug)]
struct PackInventory {
    manifest: Manifest,
    cards: BTreeMap<String, LoadedCard>,
    sources: BTreeSet<String>,
    prompts: BTreeMap<String, PromptFile>,
    pack_hash: String,
}

#[derive(Debug)]
struct LoadedCard {
    kind: CardKind,
    personas: Vec<String>,
    entries: BTreeMap<String, Entry>,
}

#[derive(Debug, Default)]
struct SegmentBindings {
    refs: usize,
    resolved_refs: usize,
    card_refs: Vec<ResolvedCardRef>,
    source_refs: BTreeSet<String>,
    binding_values: Vec<Value>,
}

#[derive(Debug, Clone)]
struct ResolvedCardRef {
    card_id: String,
    entry_id: String,
    card_kind: CardKind,
    primitive: Option<String>,
    role: String,
    evidence: Vec<String>,
}

#[derive(Debug)]
struct RouteContext {
    persona: String,
    requested_persona: String,
    job: String,
    selected_card_ids: BTreeSet<String>,
}

#[derive(Debug, Default)]
struct Counts {
    segments: usize,
    material_segments: usize,
    non_material_segments: usize,
    gap_segments: usize,
    binding_refs: usize,
    resolved_refs: usize,
}

pub(crate) fn verify_output_file(root: &Path, file: &Path) -> Result<Value> {
    let raw = fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    verify_output_raw(root, &raw, &file.display().to_string())
}

pub(crate) fn verify_output_value(
    root: &Path,
    value: &Value,
    artifact_path: &str,
) -> Result<Value> {
    let raw = serde_json::to_string(value)?;
    verify_output_raw(root, &raw, artifact_path)
}

fn verify_output_raw(root: &Path, raw: &str, artifact_path: &str) -> Result<Value> {
    let mut issues = Vec::new();
    let artifact: ProofOutputArtifact = match serde_json::from_str(raw) {
        Ok(artifact) => artifact,
        Err(err) => {
            issues.push(issue(
                "proof_output_malformed",
                "error",
                artifact_path,
                format!("proof-output artifact must be valid {PROOF_OUTPUT_CONTRACT} JSON: {err}"),
            ));
            return Ok(result(
                false,
                "blocked",
                issues,
                Counts::default(),
                Vec::new(),
                Value::Null,
            ));
        }
    };

    let inventory = load_inventory(root)?;
    validate_artifact(root, artifact_path, &artifact, &inventory, &mut issues)
}

fn validate_artifact(
    root: &Path,
    artifact_path: &str,
    artifact: &ProofOutputArtifact,
    inventory: &PackInventory,
    issues: &mut Vec<Value>,
) -> Result<Value> {
    if artifact.contract != PROOF_OUTPUT_CONTRACT {
        issues.push(issue(
            "proof_output_contract_unknown",
            "error",
            format!("{artifact_path}#/contract"),
            format!(
                "contract must be {PROOF_OUTPUT_CONTRACT}, found {}",
                artifact.contract
            ),
        ));
    }

    validate_pack_identity(artifact_path, artifact, inventory, issues);
    let route = validate_route(artifact_path, artifact.route.as_ref(), inventory, issues);
    validate_output_shape(artifact_path, artifact, issues);

    let mut counts = Counts {
        segments: artifact.segments.len(),
        ..Counts::default()
    };
    let mut referenced_bindings = Vec::new();
    let mut seen_segment_ids = BTreeSet::new();
    let mut joined_text = String::new();

    if artifact.segments.is_empty() {
        issues.push(issue(
            "proof_output_malformed",
            "error",
            format!("{artifact_path}#/segments"),
            "segments must contain a complete ordered segmentation",
        ));
    }

    for (index, segment) in artifact.segments.iter().enumerate() {
        let path = format!("{artifact_path}#/segments/{index}");
        validate_segment_shape(&path, segment, &mut seen_segment_ids, issues);
        joined_text.push_str(&segment.text);

        match segment.kind {
            SegmentKind::Claim | SegmentKind::RequirementStatus | SegmentKind::TemplateText => {
                counts.material_segments += 1;
            }
            SegmentKind::Gap => {
                counts.gap_segments += 1;
            }
            SegmentKind::Connective | SegmentKind::Formatting => {
                counts.non_material_segments += 1;
            }
        }

        let bindings = resolve_refs(&path, segment, inventory, route.as_ref(), issues);
        counts.binding_refs += bindings.refs;
        counts.resolved_refs += bindings.resolved_refs;
        referenced_bindings.extend(bindings.binding_values.clone());
        validate_segment_bindings(&path, segment, &bindings, inventory, issues);
    }

    if joined_text != artifact.output.text {
        issues.push(issue(
            "proof_output_text_mismatch",
            "error",
            format!("{artifact_path}#/segments"),
            "joined segments[].text must exactly equal output.text",
        ));
    }

    let claim_check = if artifact.output.text.trim().is_empty() {
        Value::Null
    } else {
        run_claim_check(root, artifact_path, artifact, route.as_ref(), issues)?
    };

    let decision = decision_for(issues);
    let valid = issues.is_empty();
    Ok(result(
        valid,
        decision,
        std::mem::take(issues),
        counts,
        unique_values(referenced_bindings),
        claim_check,
    ))
}

fn validate_pack_identity(
    artifact_path: &str,
    artifact: &ProofOutputArtifact,
    inventory: &PackInventory,
    issues: &mut Vec<Value>,
) {
    if artifact.pack.id.trim().is_empty() {
        issues.push(issue(
            "proof_output_malformed",
            "error",
            format!("{artifact_path}#/pack/id"),
            "pack.id must not be empty",
        ));
    } else if artifact.pack.id != inventory.manifest.id {
        issues.push(issue(
            "proof_output_pack_mismatch",
            "error",
            format!("{artifact_path}#/pack/id"),
            format!(
                "artifact pack.id {} does not match loaded pack {}",
                artifact.pack.id, inventory.manifest.id
            ),
        ));
    }

    if let Some(profile_id) = &artifact.pack.profile_id {
        let loaded_profile = inventory
            .manifest
            .profile
            .as_ref()
            .map(|profile| profile.id.as_str());
        if loaded_profile != Some(profile_id.as_str()) {
            issues.push(issue(
                "proof_output_pack_mismatch",
                "error",
                format!("{artifact_path}#/pack/profile_id"),
                format!(
                    "artifact profile_id {profile_id} does not match loaded profile {}",
                    loaded_profile.unwrap_or("<none>")
                ),
            ));
        }
    }

    if let Some(pack_hash) = &artifact.pack.pack_hash {
        let expected = pack_hash
            .trim()
            .strip_prefix("sha256:")
            .unwrap_or(pack_hash);
        if expected != inventory.pack_hash {
            issues.push(issue(
                "proof_output_stale_pack",
                "error",
                format!("{artifact_path}#/pack/pack_hash"),
                "artifact pack_hash does not match the loaded pack",
            ));
        }
    }
}

fn validate_route(
    artifact_path: &str,
    route: Option<&ProofRoute>,
    inventory: &PackInventory,
    issues: &mut Vec<Value>,
) -> Option<RouteContext> {
    let route = route?;
    let persona = route.persona.as_deref().unwrap_or("").trim();
    let job = route.job.as_deref().unwrap_or("").trim();
    if persona.is_empty() || job.is_empty() {
        issues.push(issue(
            "proof_output_malformed",
            "error",
            format!("{artifact_path}#/route"),
            "route must include both persona and job when present",
        ));
        return None;
    }
    let persona_resolution = resolve_persona_label(&inventory.manifest, persona);
    let resolved_persona = routable_persona(persona, &persona_resolution).to_string();
    if !inventory
        .manifest
        .personas
        .iter()
        .any(|candidate| candidate == &resolved_persona)
    {
        issues.push(issue(
            "proof_output_incompatible_ref",
            "error",
            format!("{artifact_path}#/route/persona"),
            format!("route persona {persona} does not resolve to a pack persona"),
        ));
    }
    if !job_matches(&inventory.manifest, job) {
        issues.push(issue(
            "proof_output_incompatible_ref",
            "error",
            format!("{artifact_path}#/route/job"),
            format!("route job {job} does not match a pack job id, label, or route description"),
        ));
    }

    let selected_card_ids = select_cards(&inventory.manifest, Some(&resolved_persona), Some(job))
        .into_iter()
        .filter_map(|value| value["id"].as_str().map(str::to_string))
        .collect();
    Some(RouteContext {
        persona: resolved_persona,
        requested_persona: persona.to_string(),
        job: job.to_string(),
        selected_card_ids,
    })
}

fn validate_output_shape(
    artifact_path: &str,
    artifact: &ProofOutputArtifact,
    issues: &mut Vec<Value>,
) {
    if artifact.output.kind.trim().is_empty() {
        issues.push(issue(
            "proof_output_malformed",
            "error",
            format!("{artifact_path}#/output/kind"),
            "output.kind must not be empty",
        ));
    }
    if artifact.output.format.trim().is_empty() {
        issues.push(issue(
            "proof_output_malformed",
            "error",
            format!("{artifact_path}#/output/format"),
            "output.format must not be empty",
        ));
    }
    if artifact.coverage.mode != "full-segmentation" {
        issues.push(issue(
            "proof_output_malformed",
            "error",
            format!("{artifact_path}#/coverage/mode"),
            "coverage.mode must be full-segmentation",
        ));
    }
    if artifact.coverage.material_policy != "bound-or-gap" {
        issues.push(issue(
            "proof_output_malformed",
            "error",
            format!("{artifact_path}#/coverage/material_policy"),
            "coverage.material_policy must be bound-or-gap",
        ));
    }
}

fn validate_segment_shape(
    path: &str,
    segment: &ProofSegment,
    seen_segment_ids: &mut BTreeSet<String>,
    issues: &mut Vec<Value>,
) {
    if segment.id.trim().is_empty() {
        issues.push(issue(
            "proof_output_malformed",
            "error",
            format!("{path}/id"),
            "segment id must not be empty",
        ));
    } else if !seen_segment_ids.insert(segment.id.clone()) {
        issues.push(issue(
            "proof_output_malformed",
            "error",
            format!("{path}/id"),
            format!("duplicate segment id {}", segment.id),
        ));
    }
    if segment.text.is_empty() {
        issues.push(issue(
            "proof_output_malformed",
            "error",
            format!("{path}/text"),
            "segment text must not be empty",
        ));
    }
}

fn resolve_refs(
    path: &str,
    segment: &ProofSegment,
    inventory: &PackInventory,
    route: Option<&RouteContext>,
    issues: &mut Vec<Value>,
) -> SegmentBindings {
    let mut bindings = SegmentBindings {
        refs: segment.refs.len(),
        ..SegmentBindings::default()
    };
    let mut seen_refs = BTreeSet::new();

    for (index, reference) in segment.refs.iter().enumerate() {
        let ref_path = format!("{path}/refs/{index}");
        validate_ref_role(&ref_path, reference.role(), issues);
        let ref_key = reference.key();
        if !seen_refs.insert(ref_key.clone()) {
            issues.push(issue(
                "proof_output_incompatible_ref",
                "warning",
                &ref_path,
                format!("duplicate proof reference {ref_key}"),
            ));
        }
        match reference {
            ProofRef::CardEntry {
                role,
                card_id,
                entry_id,
                kind,
                primitive,
            } => resolve_card_entry_ref(
                &ref_path,
                role,
                card_id,
                entry_id,
                kind.as_deref(),
                primitive.as_deref(),
                inventory,
                route,
                issues,
                &mut bindings,
            ),
            ProofRef::Source { role, source_id } => {
                if inventory.sources.contains(source_id) {
                    bindings.resolved_refs += 1;
                    bindings.source_refs.insert(source_id.clone());
                    bindings.binding_values.push(json!({
                        "type": "source",
                        "role": role,
                        "source_id": source_id
                    }));
                } else {
                    issues.push(issue(
                        "proof_output_fake_id",
                        "error",
                        format!("{ref_path}/source_id"),
                        format!("source_id {source_id} does not exist in .mdp/sources.yaml"),
                    ));
                }
            }
            ProofRef::PromptInput {
                role,
                prompt_id,
                input_name,
            } => {
                if let Some(prompt) = inventory.prompts.get(prompt_id) {
                    if prompt.inputs.iter().any(|input| input.name == *input_name) {
                        bindings.resolved_refs += 1;
                        bindings.binding_values.push(json!({
                            "type": "prompt_input",
                            "role": role,
                            "prompt_id": prompt_id,
                            "input_name": input_name
                        }));
                    } else {
                        issues.push(issue(
                            "proof_output_fake_id",
                            "error",
                            format!("{ref_path}/input_name"),
                            format!("prompt {prompt_id} has no input {input_name}"),
                        ));
                    }
                } else {
                    issues.push(issue(
                        "proof_output_fake_id",
                        "error",
                        format!("{ref_path}/prompt_id"),
                        format!("prompt_id {prompt_id} does not exist"),
                    ));
                }
            }
            ProofRef::InputContract {
                role,
                input_contract_id,
            } => {
                if inventory
                    .manifest
                    .input_contracts
                    .iter()
                    .any(|contract| contract.id == *input_contract_id)
                {
                    bindings.resolved_refs += 1;
                    bindings.binding_values.push(json!({
                        "type": "input_contract",
                        "role": role,
                        "input_contract_id": input_contract_id
                    }));
                } else {
                    issues.push(issue(
                        "proof_output_fake_id",
                        "error",
                        format!("{ref_path}/input_contract_id"),
                        format!("input_contract_id {input_contract_id} does not exist"),
                    ));
                }
            }
            ProofRef::Route { role, persona, job } => {
                let persona = persona.trim();
                let job = job.trim();
                if persona.is_empty() || job.is_empty() {
                    issues.push(issue(
                        "proof_output_malformed",
                        "error",
                        &ref_path,
                        "route ref must include non-empty persona and job",
                    ));
                    continue;
                }
                if route_ref_matches(persona, job, inventory) {
                    bindings.resolved_refs += 1;
                    bindings.binding_values.push(json!({
                        "type": "route",
                        "role": role,
                        "persona": persona,
                        "job": job
                    }));
                } else {
                    issues.push(issue(
                        "proof_output_fake_id",
                        "error",
                        &ref_path,
                        format!(
                            "route ref persona {persona} and job {job} do not resolve in this pack"
                        ),
                    ));
                }
                if let Some(route) = route {
                    if !same_route(route, persona, job) {
                        issues.push(issue(
                            "proof_output_incompatible_ref",
                            "error",
                            &ref_path,
                            "route ref must match artifact.route when artifact.route is present",
                        ));
                    }
                }
            }
        }
    }
    bindings
}

#[allow(clippy::too_many_arguments)]
fn resolve_card_entry_ref(
    path: &str,
    role: &str,
    card_id: &str,
    entry_id: &str,
    claimed_kind: Option<&str>,
    claimed_primitive: Option<&str>,
    inventory: &PackInventory,
    route: Option<&RouteContext>,
    issues: &mut Vec<Value>,
    bindings: &mut SegmentBindings,
) {
    let Some(card) = inventory.cards.get(card_id) else {
        issues.push(issue(
            "proof_output_fake_id",
            "error",
            format!("{path}/card_id"),
            format!("card_id {card_id} does not exist in the loaded pack"),
        ));
        return;
    };
    let Some(entry) = card.entries.get(entry_id) else {
        issues.push(issue(
            "proof_output_fake_id",
            "error",
            format!("{path}/entry_id"),
            format!("entry_id {entry_id} does not exist on card {card_id}"),
        ));
        return;
    };

    bindings.resolved_refs += 1;
    let actual_kind = card_kind_name(&card.kind);
    let actual_primitive = entry_primitive(entry)
        .or_else(|| primitive_for_card(&inventory.manifest, card_id))
        .map(str::to_string);

    if let Some(kind) = claimed_kind {
        if kind != actual_kind {
            issues.push(issue(
                "proof_output_incompatible_ref",
                "error",
                format!("{path}/kind"),
                format!("ref claims kind {kind}, but {card_id} resolves as {actual_kind}"),
            ));
        }
    }
    if let Some(primitive) = claimed_primitive {
        let primitive_matches = actual_primitive.as_deref() == Some(primitive)
            || inventory
                .manifest
                .primitive_map
                .get(primitive)
                .is_some_and(|mapping| mapping.cards.iter().any(|id| id == card_id));
        if !primitive_matches {
            issues.push(issue(
                "proof_output_incompatible_ref",
                "error",
                format!("{path}/primitive"),
                format!("ref claims primitive {primitive}, but {card_id}:{entry_id} does not resolve to it"),
            ));
        }
    }

    if let Some(route) = route {
        if !route.selected_card_ids.contains(card_id) {
            issues.push(issue(
                "proof_output_incompatible_ref",
                "error",
                path,
                format!(
                    "card {card_id} is not selected for route persona {} job {}",
                    route.persona, route.job
                ),
            ));
        }
        let is_guardrail_role = matches!(role, "constrains" | "supports-gap");
        if !is_guardrail_role
            && entry.applies_to.is_empty()
            && !card.personas.is_empty()
            && !contains_case_insensitive(&card.personas, &route.persona)
        {
            issues.push(issue(
                "proof_output_incompatible_ref",
                "error",
                path,
                format!("card {card_id} is not scoped to persona {}", route.persona),
            ));
        }
        if !is_guardrail_role
            && !entry.applies_to.is_empty()
            && !contains_case_insensitive(&entry.applies_to, &route.persona)
        {
            issues.push(issue(
                "proof_output_incompatible_ref",
                "error",
                path,
                format!(
                    "entry {card_id}:{entry_id} is not scoped to persona {}",
                    route.persona
                ),
            ));
        }
    }

    bindings.card_refs.push(ResolvedCardRef {
        card_id: card_id.to_string(),
        entry_id: entry_id.to_string(),
        card_kind: card.kind.clone(),
        primitive: actual_primitive.clone(),
        role: role.to_string(),
        evidence: entry.evidence.clone(),
    });
    bindings.binding_values.push(json!({
        "type": "card_entry",
        "role": role,
        "card_id": card_id,
        "entry_id": entry_id,
        "kind": actual_kind,
        "primitive": actual_primitive
    }));
}

fn validate_segment_bindings(
    path: &str,
    segment: &ProofSegment,
    bindings: &SegmentBindings,
    inventory: &PackInventory,
    issues: &mut Vec<Value>,
) {
    match segment.kind {
        SegmentKind::Claim => validate_claim_segment(path, bindings, inventory, issues),
        SegmentKind::RequirementStatus => validate_requirement_segment(path, bindings, issues),
        SegmentKind::TemplateText => validate_template_segment(path, bindings, issues),
        SegmentKind::Gap => validate_gap_segment(path, segment, bindings, issues),
        SegmentKind::Connective | SegmentKind::Formatting => {
            validate_non_material_segment(path, segment, issues)
        }
    }
}

fn validate_claim_segment(
    path: &str,
    bindings: &SegmentBindings,
    inventory: &PackInventory,
    issues: &mut Vec<Value>,
) {
    let proof_refs = bindings
        .card_refs
        .iter()
        .filter(|resolved| {
            resolved.card_kind == CardKind::Claims
                || resolved.primitive.as_deref() == Some("evidence-proof")
        })
        .collect::<Vec<_>>();
    if proof_refs.is_empty() {
        issues.push(issue(
            "proof_output_insufficient_binding",
            "error",
            path,
            "claim segments require a claims/evidence-proof card_entry binding",
        ));
        return;
    }

    for proof_ref in proof_refs {
        let source_evidence = proof_ref
            .evidence
            .iter()
            .filter(|evidence| inventory.sources.contains(*evidence))
            .collect::<Vec<_>>();
        if !source_evidence.is_empty()
            && !source_evidence
                .iter()
                .any(|source_id| bindings.source_refs.contains(*source_id))
        {
            issues.push(issue(
                "proof_output_insufficient_binding",
                "error",
                path,
                format!(
                    "claim binding {}:{} requires a source ref for one of: {}",
                    proof_ref.card_id,
                    proof_ref.entry_id,
                    source_evidence
                        .iter()
                        .map(|value| value.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
    }
}

fn validate_requirement_segment(path: &str, bindings: &SegmentBindings, issues: &mut Vec<Value>) {
    let has_requirement_ref = bindings.card_refs.iter().any(|resolved| {
        resolved.primitive.as_deref() == Some("needs-requirements")
            || resolved.primitive.as_deref() == Some("source-signals")
            || matches!(resolved.card_kind, CardKind::Pains | CardKind::Signals)
    });
    if !has_requirement_ref {
        issues.push(issue(
            "proof_output_insufficient_binding",
            "error",
            path,
            "requirement_status segments require a requirement or source-signal card_entry binding",
        ));
    }
}

fn validate_template_segment(path: &str, bindings: &SegmentBindings, issues: &mut Vec<Value>) {
    let has_template_ref = bindings.card_refs.iter().any(|resolved| {
        resolved.role == "renders"
            && (resolved.primitive.as_deref() == Some("output-contracts")
                || matches!(
                    resolved.card_kind,
                    CardKind::CopyPatterns | CardKind::OutputRules
                ))
    });
    if !has_template_ref {
        issues.push(issue(
            "proof_output_insufficient_binding",
            "error",
            path,
            "template_text segments require a renders binding to an output-contract card entry",
        ));
    }
}

fn validate_gap_segment(
    path: &str,
    segment: &ProofSegment,
    bindings: &SegmentBindings,
    issues: &mut Vec<Value>,
) {
    let Some(gap) = &segment.gap else {
        issues.push(issue(
            "proof_output_insufficient_binding",
            "error",
            format!("{path}/gap"),
            "gap segments require gap.code and gap.reason",
        ));
        return;
    };
    if gap.code.trim().is_empty() || gap.reason.trim().is_empty() {
        issues.push(issue(
            "proof_output_insufficient_binding",
            "error",
            format!("{path}/gap"),
            "gap.code and gap.reason must not be empty",
        ));
    }
    let has_gap_ref = bindings.source_refs.iter().next().is_some()
        || bindings
            .card_refs
            .iter()
            .any(|resolved| matches!(resolved.role.as_str(), "constrains" | "supports-gap"));
    if !has_gap_ref {
        issues.push(issue(
            "proof_output_insufficient_binding",
            "error",
            path,
            "gap segments require a constraining card_entry or source reference",
        ));
    }
    if !looks_like_gap_text(&segment.text) {
        issues.push(issue(
            "proof_output_gap_smoothed",
            "error",
            format!("{path}/text"),
            "gap segment text must explicitly describe missing proof, source context, or reviewer decision need",
        ));
    }
}

fn validate_non_material_segment(path: &str, segment: &ProofSegment, issues: &mut Vec<Value>) {
    if segment.material != Some(false) {
        issues.push(issue(
            "proof_output_insufficient_binding",
            "error",
            format!("{path}/material"),
            "connective/formatting segments without proof bindings must explicitly set material: false",
        ));
    }
    if !segment.refs.is_empty() {
        issues.push(issue(
            "proof_output_incompatible_ref",
            "warning",
            format!("{path}/refs"),
            "connective/formatting segments should not carry proof refs; use template_text for rendered output-contract text",
        ));
    }
    if connective_too_risky(&segment.text) {
        issues.push(issue(
            "proof_output_connective_too_risky",
            "error",
            format!("{path}/text"),
            "connective/formatting text contains material claim language; classify and bind it as material or gap text",
        ));
    }
}

fn run_claim_check(
    root: &Path,
    artifact_path: &str,
    artifact: &ProofOutputArtifact,
    route: Option<&RouteContext>,
    issues: &mut Vec<Value>,
) -> Result<Value> {
    let (persona, job) = route
        .map(|route| {
            (
                Some(route.requested_persona.as_str()),
                Some(route.job.as_str()),
            )
        })
        .unwrap_or((None, None));
    let claim_check = check_claims(root, Some(&artifact.output.text), None, None, persona, job)?;

    for (index, hit) in claim_check["guardrail_hits"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        issues.push(issue(
            "claim_check_guardrail_hit",
            "error",
            format!("{artifact_path}#/output/text"),
            format!("claim guardrail hit {index}: {hit}"),
        ));
    }
    for (index, hit) in claim_check["claim_gaps"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        issues.push(issue(
            "claim_check_guardrail_hit",
            "error",
            format!("{artifact_path}#/output/text"),
            format!("claim gap hit {index}: {hit}"),
        ));
    }
    for (index, hit) in claim_check["unsupported_claims"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        issues.push(issue(
            "claim_check_unsupported_claim",
            "error",
            format!("{artifact_path}#/output/text"),
            format!("unsupported claim hit {index}: {hit}"),
        ));
    }
    Ok(claim_check)
}

fn load_inventory(root: &Path) -> Result<PackInventory> {
    let manifest = read_manifest(root)?;
    let mut cards = BTreeMap::new();
    for card_ref in &manifest.cards {
        let card = read_card(&resolve_pack_path(root, &card_ref.path)?)?;
        cards.insert(card.id.clone(), loaded_card(card));
    }
    let sources = read_source_ids(root)?;
    let prompts = read_prompts(root)?;
    let pack_hash = compute_pack_hash(root, &manifest)?;
    Ok(PackInventory {
        manifest,
        cards,
        sources,
        prompts,
        pack_hash,
    })
}

fn loaded_card(card: Card) -> LoadedCard {
    LoadedCard {
        kind: card.kind,
        personas: card.personas,
        entries: card
            .entries
            .into_iter()
            .map(|entry| (entry.id.clone(), entry))
            .collect(),
    }
}

fn read_source_ids(root: &Path) -> Result<BTreeSet<String>> {
    let path = root
        .join(crate::constants::DEFAULT_DIR)
        .join("sources.yaml");
    if !path.exists() {
        return Ok(BTreeSet::new());
    }
    let raw = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let value: Value =
        serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    Ok(value["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|source| source["id"].as_str().map(str::to_string))
        .collect())
}

fn read_prompts(root: &Path) -> Result<BTreeMap<String, PromptFile>> {
    let prompts_dir = root.join(crate::constants::DEFAULT_DIR).join("prompts");
    if !prompts_dir.exists() {
        return Ok(BTreeMap::new());
    }
    let mut prompts = BTreeMap::new();
    for entry in
        fs::read_dir(&prompts_dir).with_context(|| format!("reading {}", prompts_dir.display()))?
    {
        let path = entry?.path();
        if !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("yaml" | "yml")
        ) {
            continue;
        }
        let prompt = read_prompt(&path)?;
        prompts.insert(prompt.id.clone(), prompt);
    }
    Ok(prompts)
}

fn compute_pack_hash(root: &Path, manifest: &Manifest) -> Result<String> {
    let mut hasher = Sha256::new();
    let manifest_path = root
        .join(crate::constants::DEFAULT_DIR)
        .join("manifest.yaml");
    hash_file(&mut hasher, &manifest_path)?;
    let sources_path = root
        .join(crate::constants::DEFAULT_DIR)
        .join("sources.yaml");
    if sources_path.exists() {
        hash_file(&mut hasher, &sources_path)?;
    }
    for card_ref in &manifest.cards {
        let path = resolve_pack_path(root, &card_ref.path)?;
        hash_file(&mut hasher, &path)?;
    }
    let mut prompt_paths = fs::read_dir(root.join(crate::constants::DEFAULT_DIR).join("prompts"))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("yaml" | "yml")
            )
        })
        .collect::<Vec<_>>();
    prompt_paths.sort();
    for path in prompt_paths {
        hash_file(&mut hasher, &path)?;
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn hash_file(hasher: &mut Sha256, path: &Path) -> Result<()> {
    hasher.update(path.display().to_string().as_bytes());
    hasher.update(b"\0");
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    hasher.update(bytes);
    hasher.update(b"\0");
    Ok(())
}

fn validate_ref_role(path: &str, role: &str, issues: &mut Vec<Value>) {
    if !matches!(
        role,
        "supports" | "constrains" | "renders" | "requires" | "supports-gap"
    ) {
        issues.push(issue(
            "proof_output_incompatible_ref",
            "error",
            format!("{path}/role"),
            format!("unknown proof reference role {role}"),
        ));
    }
}

impl ProofRef {
    fn role(&self) -> &str {
        match self {
            ProofRef::CardEntry { role, .. }
            | ProofRef::Source { role, .. }
            | ProofRef::PromptInput { role, .. }
            | ProofRef::InputContract { role, .. }
            | ProofRef::Route { role, .. } => role,
        }
    }

    fn key(&self) -> String {
        match self {
            ProofRef::CardEntry {
                role,
                card_id,
                entry_id,
                ..
            } => format!("card_entry:{role}:{card_id}:{entry_id}"),
            ProofRef::Source { role, source_id } => format!("source:{role}:{source_id}"),
            ProofRef::PromptInput {
                role,
                prompt_id,
                input_name,
            } => format!("prompt_input:{role}:{prompt_id}:{input_name}"),
            ProofRef::InputContract {
                role,
                input_contract_id,
            } => format!("input_contract:{role}:{input_contract_id}"),
            ProofRef::Route { role, persona, job } => format!("route:{role}:{persona}:{job}"),
        }
    }
}

fn decision_for(issues: &[Value]) -> &'static str {
    if issues.iter().any(|issue| {
        issue["severity"].as_str() == Some("error")
            && !matches!(
                issue["code"].as_str(),
                Some("claim_check_guardrail_hit")
                    | Some("claim_check_unsupported_claim")
                    | Some("proof_output_connective_too_risky")
            )
    }) {
        "blocked"
    } else if issues.is_empty() {
        "proof-safe"
    } else {
        "needs-revision"
    }
}

fn result(
    valid: bool,
    decision: &str,
    issues: Vec<Value>,
    counts: Counts,
    referenced_bindings: Vec<Value>,
    claim_check: Value,
) -> Value {
    let error_count = issues
        .iter()
        .filter(|issue| issue["severity"].as_str() == Some("error"))
        .count();
    let warning_count = issues
        .iter()
        .filter(|issue| issue["severity"].as_str() == Some("warning"))
        .count();
    json!({
        "contract": "mdp.verify-output.v0",
        "proof_contract": PROOF_OUTPUT_CONTRACT,
        "valid": valid,
        "decision": decision,
        "error_count": error_count,
        "warning_count": warning_count,
        "issues": issues,
        "checked": {
            "segments": counts.segments,
            "material_segments": counts.material_segments,
            "non_material_segments": counts.non_material_segments,
            "gap_segments": counts.gap_segments,
            "binding_refs": counts.binding_refs,
            "resolved_refs": counts.resolved_refs,
            "claim_check": !claim_check.is_null()
        },
        "referenced_bindings": referenced_bindings,
        "claim_check": claim_check
    })
}

fn unique_values(values: Vec<Value>) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for value in values {
        let key = value.to_string();
        if seen.insert(key) {
            unique.push(value);
        }
    }
    unique
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

fn entry_primitive(entry: &Entry) -> Option<&str> {
    entry
        .metadata
        .get("primitive")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn primitive_for_card<'a>(manifest: &'a Manifest, card_id: &str) -> Option<&'a str> {
    manifest
        .primitive_map
        .iter()
        .find(|(_, mapping)| mapping.cards.iter().any(|id| id == card_id))
        .map(|(primitive, _)| primitive.as_str())
}

fn contains_case_insensitive(values: &[String], needle: &str) -> bool {
    values
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(needle))
}

fn job_matches(manifest: &Manifest, job: &str) -> bool {
    let job = job.trim();
    if job.is_empty() {
        return false;
    }
    let normalized_job = normalize_route_token(job);
    manifest.jobs.iter().any(|candidate| {
        normalize_route_token(&candidate.id) == normalized_job
            || candidate
                .label
                .as_deref()
                .is_some_and(|label| normalize_route_token(label) == normalized_job)
            || candidate
                .description
                .as_deref()
                .is_some_and(|description| description.to_lowercase().contains(&job.to_lowercase()))
    })
}

fn route_ref_matches(persona: &str, job: &str, inventory: &PackInventory) -> bool {
    let persona = persona.trim();
    let job = job.trim();
    if persona.is_empty() || job.is_empty() {
        return false;
    }
    let persona_resolution = resolve_persona_label(&inventory.manifest, persona);
    let resolved_persona = routable_persona(persona, &persona_resolution);
    inventory
        .manifest
        .personas
        .iter()
        .any(|candidate| candidate == resolved_persona)
        && job_matches(&inventory.manifest, job)
}

fn same_route(route: &RouteContext, persona: &str, job: &str) -> bool {
    (route.persona.eq_ignore_ascii_case(persona)
        || route.requested_persona.eq_ignore_ascii_case(persona))
        && normalize_route_token(&route.job) == normalize_route_token(job)
}

fn normalize_route_token(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_like_gap_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "gap",
        "missing",
        "no approved",
        "not supported",
        "needs reviewer",
        "needs human",
        "needs source",
        "lacks",
        "insufficient",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn connective_too_risky(text: &str) -> bool {
    let lower = text.to_lowercase();
    let has_digit = text.chars().any(|character| character.is_ascii_digit());
    has_digit
        || lower.len() > 120
        || [
            "compliant",
            "compliance",
            "security",
            "secure",
            "certified",
            "certification",
            "customer",
            "past performance",
            "guarantee",
            "integrates",
            "integration",
            "proof",
            "approved",
            "trusted by",
            "production",
            "revenue",
            "roi",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn verify_output_accepts_bound_material_and_gap_segments() {
        let root = temp_proposal_pack("proof-valid");
        let result = verify_output_value(&root, &valid_artifact(), "inline")
            .expect("verify-output should run");

        assert_eq!(result["valid"], true, "{result}");
        assert_eq!(result["decision"], "proof-safe");
        assert_eq!(result["checked"]["segments"], 4);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn verify_output_rejects_fake_source_ids() {
        let root = temp_proposal_pack("proof-fake-source");
        let mut artifact = valid_artifact();
        artifact["segments"][2]["refs"][1]["source_id"] = json!("invented-source");

        let result = verify_output_value(&root, &artifact, "inline").expect("verify should run");
        assert_eq!(result["valid"], false);
        assert!(issue_codes(&result).contains(&"proof_output_fake_id"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn verify_output_rejects_blank_route_ref_values() {
        let root = temp_proposal_pack("proof-blank-route-ref");
        let mut artifact = valid_artifact();
        artifact["segments"][2]["refs"]
            .as_array_mut()
            .expect("claim refs should be an array")
            .push(json!({
                "type": "route",
                "role": "supports",
                "persona": "Proposal Lead",
                "job": "   "
            }));

        let result = verify_output_value(&root, &artifact, "inline").expect("verify should run");
        assert_eq!(result["valid"], false);
        assert!(issue_codes(&result).contains(&"proof_output_malformed"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn verify_output_rejects_unbound_claim_segments() {
        let root = temp_proposal_pack("proof-missing-binding");
        let mut artifact = valid_artifact();
        artifact["segments"][2]["refs"] = json!([]);

        let result = verify_output_value(&root, &artifact, "inline").expect("verify should run");
        assert_eq!(result["valid"], false);
        assert!(issue_codes(&result).contains(&"proof_output_insufficient_binding"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn verify_output_rejects_unknown_contracts() {
        let root = temp_proposal_pack("proof-unknown-contract");
        let mut artifact = valid_artifact();
        artifact["contract"] = json!("mdp.proof-output.v999");

        let result = verify_output_value(&root, &artifact, "inline").expect("verify should run");
        assert_eq!(result["valid"], false);
        assert_eq!(result["decision"], "blocked");
        assert!(issue_codes(&result).contains(&"proof_output_contract_unknown"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn verify_output_allows_explicit_non_material_connective_text() {
        let root = temp_proposal_pack("proof-connective");
        let artifact = json!({
            "contract": "mdp.proof-output.v0",
            "pack": {"id": "proposal-mdp-sample", "profile_id": "proposal"},
            "output": {"kind": "proposal-review-section", "format": "markdown", "text": "Summary: "},
            "coverage": {"mode": "full-segmentation", "material_policy": "bound-or-gap"},
            "segments": [
                {"id": "seg-001", "kind": "connective", "material": false, "text": "Summary: "}
            ]
        });

        let result = verify_output_value(&root, &artifact, "inline").expect("verify should run");
        assert_eq!(result["valid"], true);
        assert_eq!(result["checked"]["non_material_segments"], 1);

        let _ = std::fs::remove_dir_all(root);
    }

    fn temp_proposal_pack(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("mdp-{prefix}-{nonce}"));
        init_pack(
            &root,
            "Proposal Reference Profile Sample",
            "proposal",
            true,
            false,
        )
        .expect("proposal pack should initialize");
        root
    }

    fn valid_artifact() -> Value {
        json!({
            "contract": "mdp.proof-output.v0",
            "pack": {"id": "proposal-mdp-sample", "profile_id": "proposal"},
            "route": {"persona": "Proposal Lead", "job": "compliance review"},
            "output": {
                "kind": "proposal-review-section",
                "format": "markdown",
                "text": "Requirement status: Source-backed. The synthetic sample may discuss phased rollout planning and training readiness. Gap: the sample pack has no approved certification proof."
            },
            "coverage": {"mode": "full-segmentation", "material_policy": "bound-or-gap"},
            "segments": [
                {
                    "id": "seg-001",
                    "kind": "template_text",
                    "text": "Requirement status: ",
                    "refs": [
                        {"type": "card_entry", "role": "renders", "card_id": "review-outputs", "entry_id": "compliance-matrix", "kind": "copy-patterns", "primitive": "output-contracts"}
                    ]
                },
                {
                    "id": "seg-002",
                    "kind": "requirement_status",
                    "text": "Source-backed. ",
                    "refs": [
                        {"type": "card_entry", "role": "requires", "card_id": "requirements-matrix", "entry_id": "must-answer-sections", "kind": "pains", "primitive": "needs-requirements"}
                    ]
                },
                {
                    "id": "seg-003",
                    "kind": "claim",
                    "text": "The synthetic sample may discuss phased rollout planning and training readiness.",
                    "refs": [
                        {"type": "card_entry", "role": "supports", "card_id": "proof-library", "entry_id": "approved-synthetic-proof", "kind": "claims", "primitive": "evidence-proof"},
                        {"type": "source", "role": "supports", "source_id": "synthetic-proof-inventory"}
                    ]
                },
                {
                    "id": "seg-004",
                    "kind": "gap",
                    "text": " Gap: the sample pack has no approved certification proof.",
                    "gap": {"code": "missing-approved-proof", "reason": "Certification proof is not present in the synthetic proof library."},
                    "refs": [
                        {"type": "card_entry", "role": "constrains", "card_id": "proof-library", "entry_id": "unsupported-certifications", "kind": "claims", "primitive": "boundaries"},
                        {"type": "source", "role": "supports-gap", "source_id": "synthetic-proof-inventory"}
                    ]
                }
            ]
        })
    }

    fn issue_codes(result: &Value) -> Vec<&str> {
        result["issues"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|issue| issue["code"].as_str())
            .collect()
    }
}
