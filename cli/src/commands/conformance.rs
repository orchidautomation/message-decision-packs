use crate::artifact_hash::{canonical_json_sha256, pack_content_sha256, parse_authority_json};
use crate::cli::SchemaTarget;
use crate::commands::decision_trace::project_source_value;
use crate::commands::health::validate_pack;
use crate::commands::requirements::requirements;
use crate::commands::run_verification::verify_run;
use crate::commands::schemas::schema;
use crate::commands::skills::skills;
use crate::conformance::{
    AccessClass, BEHAVIORAL_EVALUATION_V1, BehavioralEvaluation, BehavioralQualification,
    BehavioralStatus, CandidateAuthorityRole, ConformanceCandidateV1, ConformanceContract,
    ConformanceJourney, ConformanceReportV1, DETERMINISTIC_CONFORMANCE_V1, EvaluatorInventoryV1,
    JOB_CONFORMANCE_V1, JobConformanceV1, JourneyArtifact, JourneyArtifactRole, JourneyLink,
    JourneyPhase, JourneyRelation, MAX_CONFORMANCE_AUTHORITY_BYTES, MAX_TRIALS_PER_JOB,
    ModelVisibleInput, PUBLIC_CONFORMANCE_REPORT_V1, PrivateRecordPolicyV1,
    PublicConformanceReportV1, PublicEvidenceDigest, PublicJobResult, PublicationApprovalV1,
    StagedAuthorityRef, canonical_authority_sha256, conformance_limits, evaluate_behavioral_trials,
    parse_behavioral_evaluation, parse_candidate, parse_candidate_file,
    parse_conformance_verifier_receipt, parse_deterministic_conformance, parse_evaluator_inventory,
    parse_evaluator_result, parse_invocation, parse_job_conformance, parse_lifecycle_policy,
    parse_publication_approval, parse_trial, read_contained_authority,
    read_contained_authority_bytes, read_contained_file,
};
use crate::pack_io::read_manifest;
use crate::run_contracts::{RunBundleV1, RunReceiptV1};
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
enum AssertionStatus {
    Pass,
    Fail,
    Unassessed,
}

impl AssertionStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Unassessed => "unassessed",
        }
    }
}

struct LoadedAuthorities<'a> {
    root: PathBuf,
    pack_root: PathBuf,
    references: HashMap<CandidateAuthorityRole, &'a StagedAuthorityRef>,
    values: HashMap<CandidateAuthorityRole, Value>,
    bytes: HashMap<CandidateAuthorityRole, Vec<u8>>,
}

pub(crate) fn compile_candidate_file(candidate_path: &Path, artifact_root: &Path) -> Result<Value> {
    let candidate = parse_candidate_file(candidate_path, artifact_root)?;
    let candidate_root = contained_candidate_root(artifact_root, &candidate.artifact_root)?;
    let loaded = load_authorities(&candidate, candidate_root)?;
    compile_candidate(&candidate, &loaded)
}

pub(crate) struct BehavioralEvidencePaths<'a> {
    pub(crate) artifact_root: &'a Path,
    pub(crate) candidate: &'a Path,
    pub(crate) deterministic: &'a Path,
    pub(crate) evaluator_inventory: &'a Path,
    pub(crate) lifecycle_policy: &'a Path,
    pub(crate) invocations: &'a [PathBuf],
    pub(crate) trials: &'a [PathBuf],
    pub(crate) evaluator_results: &'a [PathBuf],
    pub(crate) publication_approvals: &'a [PathBuf],
    pub(crate) verifier_receipts: &'a [PathBuf],
}

pub(crate) fn validate_behavioral_files(paths: BehavioralEvidencePaths<'_>) -> Result<Value> {
    for (label, count) in [
        ("invocation", paths.invocations.len()),
        ("trial", paths.trials.len()),
        ("evaluator result", paths.evaluator_results.len()),
        ("publication approval", paths.publication_approvals.len()),
        ("verifier receipt", paths.verifier_receipts.len()),
    ] {
        if count > MAX_TRIALS_PER_JOB {
            return Err(anyhow!(
                "too many {label} inputs: maximum is {MAX_TRIALS_PER_JOB}"
            ));
        }
    }
    let read = |path: &Path| read_contained_file(paths.artifact_root, path);
    let candidate = parse_candidate(&read(paths.candidate)?)?;
    let deterministic = parse_deterministic_conformance(&read(paths.deterministic)?)?;
    let inventory = parse_evaluator_inventory(&read(paths.evaluator_inventory)?)?;
    validate_inventory_candidate_context(&candidate, paths.artifact_root, &inventory)?;
    let lifecycle = parse_lifecycle_policy(&read(paths.lifecycle_policy)?)?;
    let invocations = paths
        .invocations
        .iter()
        .map(|path| parse_invocation(&read(path)?))
        .collect::<Result<Vec<_>>>()?;
    let trials = paths
        .trials
        .iter()
        .map(|path| parse_trial(&read(path)?))
        .collect::<Result<Vec<_>>>()?;
    let results = paths
        .evaluator_results
        .iter()
        .map(|path| parse_evaluator_result(&read(path)?))
        .collect::<Result<Vec<_>>>()?;
    let approvals = paths
        .publication_approvals
        .iter()
        .map(|path| parse_publication_approval(&read(path)?))
        .collect::<Result<Vec<_>>>()?;
    let verifier_receipts = paths
        .verifier_receipts
        .iter()
        .map(|path| parse_conformance_verifier_receipt(&read(path)?))
        .collect::<Result<Vec<_>>>()?;
    Ok(serde_json::to_value(evaluate_behavioral_trials(
        &candidate,
        &inventory,
        &lifecycle,
        &deterministic,
        &invocations,
        &trials,
        &results,
        &approvals,
        &verifier_receipts,
    )?)?)
}

fn validate_inventory_candidate_context(
    candidate: &ConformanceCandidateV1,
    artifact_root: &Path,
    inventory: &EvaluatorInventoryV1,
) -> Result<()> {
    let candidate_root = contained_candidate_root(artifact_root, &candidate.artifact_root)?;
    let loaded = load_authorities(candidate, candidate_root)?;
    let requirements = loaded
        .values
        .get(&CandidateAuthorityRole::Requirements)
        .ok_or_else(|| anyhow!("candidate is missing requirements authority"))?;
    if !validate_prompt_integrity(requirements, &loaded)? {
        return Err(anyhow!(
            "candidate compiled prompt authority is not ready or exact"
        ));
    }
    let expected_prompt = requirements["model_task"]["prompt_sha256"]
        .as_str()
        .ok_or_else(|| anyhow!("candidate requirements lack compiled prompt digest"))?;
    let invocation = loaded
        .values
        .get(&CandidateAuthorityRole::PromptInvocation)
        .ok_or_else(|| {
            anyhow!("behavioral qualification requires candidate prompt invocation authority")
        })?;
    if invocation["job_id"].as_str() != Some(candidate.job_id.as_str())
        || invocation["prompt"]["sha256"].as_str() != Some(expected_prompt)
    {
        return Err(anyhow!(
            "candidate prompt invocation does not bind compiled job prompt"
        ));
    }
    let inputs = invocation["inputs"]
        .as_array()
        .ok_or_else(|| anyhow!("candidate prompt invocation inputs must be an array"))?
        .iter()
        .map(|input| {
            let name = input["name"]
                .as_str()
                .ok_or_else(|| anyhow!("candidate prompt invocation input name is missing"))?;
            let sha256 = input["sha256"]
                .as_str()
                .ok_or_else(|| anyhow!("candidate prompt invocation input digest is missing"))?;
            if sha256.len() != 64
                || !sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                return Err(anyhow!(
                    "candidate prompt invocation input digest is invalid"
                ));
            }
            Ok(ModelVisibleInput {
                name: name.to_string(),
                sha256: sha256.to_string(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let prompt_ref = authority_ref(&loaded, CandidateAuthorityRole::Prompt)?;
    let prompt: crate::models::PromptFile = serde_yaml::from_slice(
        loaded
            .bytes
            .get(&prompt_ref.role)
            .ok_or_else(|| anyhow!("candidate prompt bytes were not retained"))?,
    )?;
    let declared = prompt
        .inputs
        .iter()
        .map(|input| input.name.as_str())
        .collect::<Vec<_>>();
    let observed = inputs
        .iter()
        .map(|input| input.name.as_str())
        .collect::<Vec<_>>();
    let expected_order = declared
        .iter()
        .copied()
        .filter(|name| observed.contains(name))
        .collect::<Vec<_>>();
    if observed != expected_order
        || inputs
            .iter()
            .any(|input| !declared.contains(&input.name.as_str()))
        || prompt.inputs.iter().any(|input| {
            input.required
                && input.name != "prompt_receipt"
                && input.name != "invocation_receipt_sha256"
                && !observed.contains(&input.name.as_str())
        })
    {
        return Err(anyhow!(
            "candidate prompt invocation does not exactly follow declared model inputs"
        ));
    }
    let context_sha256 = crate::artifact_hash::canonical_json_sha256_for_domain(
        "mdp.model-visible-context.v1",
        &serde_json::to_value(&inputs)?,
    )?;
    let challenge = matching_inventory_challenge(candidate, inventory)?;
    if challenge.trial_slots.iter().any(|slot| {
        slot.prompt_sha256 != expected_prompt
            || slot.input_artifacts != inputs
            || slot.model_visible_context_sha256 != context_sha256
    }) {
        return Err(anyhow!(
            "evaluator inventory does not bind candidate compiled model context"
        ));
    }
    Ok(())
}

fn matching_inventory_challenge<'a>(
    candidate: &ConformanceCandidateV1,
    inventory: &'a EvaluatorInventoryV1,
) -> Result<&'a crate::conformance::EvaluatorChallenge> {
    let matching = inventory
        .challenges
        .iter()
        .filter(|challenge| {
            challenge.job_id == candidate.job_id && challenge.fixture_id == candidate.fixture_id
        })
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [challenge]
            if candidate
                .challenge_id
                .as_deref()
                .is_none_or(|id| id == challenge.challenge_id) =>
        {
            Ok(*challenge)
        }
        _ => Err(anyhow!(
            "evaluator inventory must contain one exact job and fixture challenge"
        )),
    }
}

pub(crate) struct AssembleConformancePaths<'a> {
    pub(crate) candidate: &'a Path,
    pub(crate) deterministic: &'a Path,
    pub(crate) behavioral: &'a Path,
    pub(crate) trials: &'a [PathBuf],
    pub(crate) artifact_root: &'a Path,
}

/// Builds the sole cross-phase authority. Every supplied path is relative to
/// the staged root and is read through the shared containment boundary.
pub(crate) fn assemble_conformance(paths: AssembleConformancePaths<'_>) -> Result<Value> {
    let candidate_bytes = read_contained_file(paths.artifact_root, paths.candidate)?;
    let candidate = parse_candidate(&candidate_bytes)?;
    let candidate_sha256 = canonical_authority_sha256(&candidate)?;

    let deterministic_bytes = read_contained_file(paths.artifact_root, paths.deterministic)?;
    let deterministic = parse_deterministic_conformance(&deterministic_bytes)?;
    validate_deterministic_binding(&candidate, &deterministic)?;
    if deterministic != recompute_deterministic(&candidate, paths.artifact_root)? {
        return Err(anyhow!(
            "deterministic evaluation does not equal authoritative staged compilation"
        ));
    }
    let deterministic_sha256 = canonical_authority_sha256(&deterministic)?;

    let behavioral_bytes = read_contained_file(paths.artifact_root, paths.behavioral)?;
    let behavioral = parse_behavioral_evaluation(&behavioral_bytes)?;
    if behavioral.job_id != candidate.job_id
        || behavioral.candidate_sha256 != candidate_sha256
        || behavioral.evaluator_inventory_sha256 != candidate.evaluator_inventory_sha256
        || behavioral.lifecycle_policy_sha256 != candidate.lifecycle_policy_sha256
        || behavioral.deterministic_evaluation_sha256 != deterministic_sha256
    {
        return Err(anyhow!("behavioral evaluation does not bind the candidate"));
    }
    let behavioral_sha256 = canonical_authority_sha256(&behavioral)?;

    let mut parsed_trials = Vec::new();
    let mut trial_paths = Vec::new();
    for trial_path in paths.trials {
        let bytes = read_contained_file(paths.artifact_root, trial_path)?;
        let trial = parse_trial(&bytes)?;
        if trial.candidate_sha256 != candidate_sha256
            || trial.lifecycle_policy_sha256 != candidate.lifecycle_policy_sha256
        {
            return Err(anyhow!(
                "trial does not bind the candidate and lifecycle policy"
            ));
        }
        trial_paths.push((trial_path.clone(), bytes.len() as u64));
        parsed_trials.push(trial);
    }
    let trial_sha256s = parsed_trials
        .iter()
        .map(canonical_authority_sha256)
        .collect::<Result<Vec<_>>>()?;
    if trial_sha256s != behavioral.trial_sha256s {
        return Err(anyhow!(
            "trial set does not exactly match behavioral evaluation"
        ));
    }

    let candidate_root = contained_candidate_root(paths.artifact_root, &candidate.artifact_root)?;
    let lifecycle_ref = candidate
        .authorities
        .iter()
        .find(|reference| reference.role == CandidateAuthorityRole::PrivateRecordPolicy)
        .ok_or_else(|| anyhow!("candidate is missing private-record-policy authority"))?;
    let lifecycle: PrivateRecordPolicyV1 = read_contained_authority(
        &candidate_root,
        &lifecycle_ref.relative_path,
        &lifecycle_ref.sha256,
        conformance_limits(),
    )?;
    lifecycle.validate()?;
    if canonical_authority_sha256(&lifecycle)? != candidate.lifecycle_policy_sha256 {
        return Err(anyhow!("candidate lifecycle policy digest mismatch"));
    }
    let inventory_ref = candidate
        .authorities
        .iter()
        .find(|reference| reference.role == CandidateAuthorityRole::EvaluatorInventory)
        .ok_or_else(|| anyhow!("candidate is missing evaluator inventory authority"))?;
    let inventory = parse_evaluator_inventory(&read_contained_authority_bytes(
        &candidate_root,
        inventory_ref,
        MAX_CONFORMANCE_AUTHORITY_BYTES,
    )?)?;
    let approvals = load_candidate_approvals(&candidate, &candidate_root, &inventory)?;

    let mut artifacts = Vec::new();
    artifacts.push(JourneyArtifact {
        artifact_id: "candidate".into(),
        phase: JourneyPhase::Candidate,
        role: JourneyArtifactRole::Candidate,
        contract: candidate.contract.clone(),
        relative_path: Some(path_string(paths.candidate)?),
        opaque_artifact_id: None,
        authority_sha256: candidate_sha256.clone(),
        byte_count: Some(candidate_bytes.len() as u64),
        access_class: public_access(&lifecycle, &approvals, &candidate_sha256),
        publication_approval_sha256: approval_hash(&lifecycle, &approvals, &candidate_sha256)?,
    });
    for (index, reference) in candidate.authorities.iter().enumerate() {
        let role = journey_role(reference.role);
        let digest = match reference.role {
            CandidateAuthorityRole::EvaluatorInventory => {
                let bytes = read_contained_authority_bytes(
                    &candidate_root,
                    reference,
                    crate::conformance::MAX_CONFORMANCE_AUTHORITY_BYTES,
                )?;
                parse_evaluator_inventory(&bytes)?.inventory_sha256
            }
            CandidateAuthorityRole::PrivateRecordPolicy => canonical_authority_sha256(&lifecycle)?,
            CandidateAuthorityRole::PublicationApproval => {
                let bytes = read_contained_authority_bytes(
                    &candidate_root,
                    reference,
                    crate::conformance::MAX_CONFORMANCE_AUTHORITY_BYTES,
                )?;
                canonical_authority_sha256(&parse_publication_approval(&bytes)?)?
            }
            _ => reference.sha256.clone(),
        };
        artifacts.push(JourneyArtifact {
            artifact_id: format!("authority-{index}-{}", role_token(role)),
            phase: journey_phase(reference.role),
            role,
            contract: reference.contract.clone(),
            relative_path: Some(path_string(
                &Path::new(&candidate.artifact_root).join(&reference.relative_path),
            )?),
            opaque_artifact_id: None,
            authority_sha256: digest.clone(),
            byte_count: Some(reference.byte_count),
            access_class: public_access(&lifecycle, &approvals, &digest),
            publication_approval_sha256: approval_hash(&lifecycle, &approvals, &digest)?,
        });
    }
    artifacts.push(JourneyArtifact {
        artifact_id: "deterministic-evaluation".into(),
        phase: JourneyPhase::DeterministicEvaluation,
        role: JourneyArtifactRole::DeterministicEvaluation,
        contract: DETERMINISTIC_CONFORMANCE_V1.into(),
        relative_path: Some(path_string(paths.deterministic)?),
        opaque_artifact_id: None,
        authority_sha256: deterministic_sha256.clone(),
        byte_count: Some(deterministic_bytes.len() as u64),
        access_class: public_access(&lifecycle, &approvals, &deterministic_sha256),
        publication_approval_sha256: approval_hash(&lifecycle, &approvals, &deterministic_sha256)?,
    });
    artifacts.push(JourneyArtifact {
        artifact_id: "behavioral-evaluation".into(),
        phase: JourneyPhase::BehavioralEvaluation,
        role: JourneyArtifactRole::BehavioralEvaluation,
        contract: BEHAVIORAL_EVALUATION_V1.into(),
        relative_path: Some(path_string(paths.behavioral)?),
        opaque_artifact_id: None,
        authority_sha256: behavioral_sha256.clone(),
        byte_count: Some(behavioral_bytes.len() as u64),
        access_class: public_access(&lifecycle, &approvals, &behavioral_sha256),
        publication_approval_sha256: approval_hash(&lifecycle, &approvals, &behavioral_sha256)?,
    });
    for (index, ((path, byte_count), digest)) in
        trial_paths.iter().zip(trial_sha256s.iter()).enumerate()
    {
        artifacts.push(JourneyArtifact {
            artifact_id: format!("trial-{}", index + 1),
            phase: JourneyPhase::BehavioralEvaluation,
            role: JourneyArtifactRole::Trial,
            contract: crate::conformance::CONFORMANCE_TRIAL_V1.into(),
            relative_path: Some(path_string(path)?),
            opaque_artifact_id: None,
            authority_sha256: digest.clone(),
            byte_count: Some(*byte_count),
            access_class: public_access(&lifecycle, &approvals, digest),
            publication_approval_sha256: approval_hash(&lifecycle, &approvals, digest)?,
        });
    }

    let links = build_journey_links(&artifacts);
    let behavioral_status = aggregate_behavioral_status(&behavioral);
    let composite = JobConformanceV1 {
        contract: JOB_CONFORMANCE_V1.into(),
        candidate_id: candidate.candidate_id.clone(),
        job_id: candidate.job_id.clone(),
        fixture_id: candidate.fixture_id.clone(),
        pack_release: candidate.pack_release.clone(),
        candidate_sha256,
        evaluator_inventory_sha256: candidate.evaluator_inventory_sha256.clone(),
        lifecycle_policy_sha256: candidate.lifecycle_policy_sha256.clone(),
        deterministic_evaluation_sha256: deterministic_sha256,
        behavioral_evaluation_sha256: behavioral_sha256,
        deterministic_status: behavioral.deterministic_status,
        behavioral_status,
        verdict: behavioral.overall_result,
        trial_sha256s,
        journey: ConformanceJourney {
            subject_class: "synthetic-prospect".into(),
            synthetic_subject: true,
            artifacts,
            links,
        },
        limitations: behavioral.reason_codes.clone(),
    };
    composite.validate()?;
    validate_composite_members(&composite, paths.artifact_root)?;
    Ok(serde_json::to_value(composite)?)
}

pub(crate) fn project_conformance_report(
    composite_path: &Path,
    artifact_root: &Path,
    generated_at: &str,
    public: bool,
) -> Result<Value> {
    if !crate::value_contracts::valid_date_time(generated_at) {
        return Err(anyhow!("generated_at must be an RFC 3339 date-time"));
    }
    let bytes = read_contained_file(artifact_root, composite_path)?;
    let composite = parse_job_conformance(&bytes)?;
    validate_composite_members(&composite, artifact_root)?;
    let composite_sha256 = canonical_authority_sha256(&composite)?;
    let inventory = load_inventory_from_composite(&composite, artifact_root)?;
    if public {
        let evidence = composite
            .journey
            .artifacts
            .iter()
            .map(|artifact| PublicEvidenceDigest {
                artifact_role: artifact.role,
                artifact_sha256: matches!(
                    artifact.access_class,
                    AccessClass::Synthetic | AccessClass::SanitizedPublic
                )
                .then(|| artifact.authority_sha256.clone()),
                classification: artifact.access_class,
                publication_approved: artifact.publication_approval_sha256.is_some(),
            })
            .collect();
        let report = PublicConformanceReportV1 {
            contract: PUBLIC_CONFORMANCE_REPORT_V1.into(),
            report_id: format!(
                "{}:{}:{}",
                composite.pack_release.pack_id, composite.pack_release.release_id, composite.job_id
            ),
            pack_id: composite.pack_release.pack_id.clone(),
            release_id: composite.pack_release.release_id.clone(),
            evaluator_id: inventory.evaluator_id,
            evaluator_version: inventory.evaluator_version,
            generated_at: generated_at.to_string(),
            jobs: vec![PublicJobResult {
                job_id: composite.job_id.clone(),
                deterministic_status: composite.deterministic_status,
                behavioral_status: composite.behavioral_status,
                verdict: composite.verdict,
                evidence,
                limitations: composite.limitations.clone(),
            }],
        };
        report.validate()?;
        Ok(serde_json::to_value(report)?)
    } else {
        let report = ConformanceReportV1 {
            contract: crate::conformance::CONFORMANCE_REPORT_V1.into(),
            report_id: format!("private-{}", &composite_sha256[..16]),
            pack_release: composite.pack_release.clone(),
            evaluator_inventory_sha256: composite.evaluator_inventory_sha256.clone(),
            job_conformance_sha256s: vec![composite_sha256],
            generated_at: generated_at.to_string(),
            lifecycle_policy_sha256: composite.lifecycle_policy_sha256.clone(),
        };
        report.validate()?;
        Ok(serde_json::to_value(report)?)
    }
}

pub(crate) fn validate_composite_members(
    composite: &JobConformanceV1,
    artifact_root: &Path,
) -> Result<()> {
    composite.validate()?;
    let mut approval_members = HashMap::new();
    let mut parsed_candidate = None;
    let mut parsed_deterministic = None;
    let mut parsed_behavioral = None;
    let mut parsed_inventory = None;
    let mut parsed_lifecycle = None;
    let mut parsed_trials = Vec::new();
    for artifact in &composite.journey.artifacts {
        let Some(path) = artifact.relative_path.as_deref() else {
            continue;
        };
        let bytes = read_contained_file(artifact_root, Path::new(path))?;
        if bytes.len() as u64 != artifact.byte_count.unwrap_or_default() {
            return Err(anyhow!("journey artifact byte count mismatch"));
        }
        let mut parsed_approval = None;
        let actual = match artifact.role {
            JourneyArtifactRole::Candidate => {
                if artifact.contract != crate::conformance::CONFORMANCE_CANDIDATE_V1 {
                    return Err(anyhow!("candidate role has wrong contract"));
                }
                let value = parse_candidate(&bytes)?;
                let digest = canonical_authority_sha256(&value)?;
                parsed_candidate = Some(value);
                digest
            }
            JourneyArtifactRole::Trial => {
                if artifact.contract != crate::conformance::CONFORMANCE_TRIAL_V1 {
                    return Err(anyhow!("trial role has wrong contract"));
                }
                let value = parse_trial(&bytes)?;
                let digest = canonical_authority_sha256(&value)?;
                parsed_trials.push((digest.clone(), value));
                digest
            }
            JourneyArtifactRole::BehavioralEvaluation => {
                if artifact.contract != BEHAVIORAL_EVALUATION_V1 {
                    return Err(anyhow!("behavioral role has wrong contract"));
                }
                let value = parse_behavioral_evaluation(&bytes)?;
                let digest = canonical_authority_sha256(&value)?;
                parsed_behavioral = Some(value);
                digest
            }
            JourneyArtifactRole::EvaluatorInventory => {
                if artifact.contract != crate::conformance::EVALUATOR_INVENTORY_V1 {
                    return Err(anyhow!("evaluator inventory role has wrong contract"));
                }
                let value = parse_evaluator_inventory(&bytes)?;
                let digest = value.inventory_sha256.clone();
                parsed_inventory = Some(value);
                digest
            }
            JourneyArtifactRole::PrivateRecordPolicy => {
                if artifact.contract != crate::conformance::PRIVATE_RECORD_POLICY_V1 {
                    return Err(anyhow!("private record policy role has wrong contract"));
                }
                let value = parse_lifecycle_policy(&bytes)?;
                let digest = canonical_authority_sha256(&value)?;
                parsed_lifecycle = Some(value);
                digest
            }
            JourneyArtifactRole::PublicationApproval => {
                if artifact.contract != crate::conformance::PUBLICATION_APPROVAL_V1 {
                    return Err(anyhow!("publication approval role has wrong contract"));
                }
                let approval = parse_publication_approval(&bytes)?;
                let digest = canonical_authority_sha256(&approval)?;
                parsed_approval = Some(approval);
                digest
            }
            JourneyArtifactRole::DeterministicEvaluation => {
                if artifact.contract != DETERMINISTIC_CONFORMANCE_V1 {
                    return Err(anyhow!("deterministic role has wrong contract"));
                }
                let value = parse_deterministic_conformance(&bytes)?;
                let digest = canonical_authority_sha256(&value)?;
                parsed_deterministic = Some(value);
                digest
            }
            _ => {
                let candidate = parsed_candidate
                    .as_ref()
                    .ok_or_else(|| anyhow!("candidate must precede candidate authority members"))?;
                let reference = candidate
                    .authorities
                    .iter()
                    .find(|reference| {
                        journey_role(reference.role) == artifact.role
                            && reference.contract == artifact.contract
                            && artifact.relative_path.as_deref().is_some_and(|path| {
                                path == Path::new(&candidate.artifact_root)
                                    .join(&reference.relative_path)
                                    .to_string_lossy()
                            })
                    })
                    .ok_or_else(|| anyhow!("journey role has unknown or mismatched contract"))?;
                if crate::artifact_hash::sha256_hex(&bytes) != reference.sha256 {
                    return Err(anyhow!("candidate authority raw hash mismatch"));
                }
                reference.sha256.clone()
            }
        };
        if actual != artifact.authority_sha256 {
            return Err(anyhow!("journey artifact hash mismatch"));
        }
        if artifact.role == JourneyArtifactRole::PublicationApproval {
            let approval = parsed_approval
                .ok_or_else(|| anyhow!("publication approval role has wrong contract"))?;
            approval_members.insert(actual, approval);
        }
    }
    let candidate = parsed_candidate.ok_or_else(|| anyhow!("composite lacks candidate member"))?;
    let candidate_member_roles = candidate
        .authorities
        .iter()
        .map(|reference| journey_role(reference.role))
        .collect::<Vec<_>>();
    let supplied_candidate_members = composite
        .journey
        .artifacts
        .iter()
        .filter(|artifact| candidate_member_roles.contains(&artifact.role))
        .count();
    if supplied_candidate_members != candidate.authorities.len() {
        return Err(anyhow!(
            "journey must contain exactly one member for every candidate authority"
        ));
    }
    for reference in &candidate.authorities {
        let expected_path = Path::new(&candidate.artifact_root)
            .join(&reference.relative_path)
            .to_string_lossy()
            .into_owned();
        let count = composite
            .journey
            .artifacts
            .iter()
            .filter(|artifact| {
                artifact.role == journey_role(reference.role)
                    && artifact.contract == reference.contract
                    && artifact.relative_path.as_deref() == Some(expected_path.as_str())
            })
            .count();
        if count != 1 {
            return Err(anyhow!(
                "journey candidate authority member is omitted, duplicated, or mismatched"
            ));
        }
    }
    let deterministic =
        parsed_deterministic.ok_or_else(|| anyhow!("composite lacks deterministic member"))?;
    let behavioral =
        parsed_behavioral.ok_or_else(|| anyhow!("composite lacks behavioral member"))?;
    let lifecycle =
        parsed_lifecycle.ok_or_else(|| anyhow!("composite lacks lifecycle policy member"))?;
    let inventory =
        parsed_inventory.ok_or_else(|| anyhow!("composite lacks evaluator inventory member"))?;
    let candidate_hash = canonical_authority_sha256(&candidate)?;
    let deterministic_hash = canonical_authority_sha256(&deterministic)?;
    let behavioral_hash = canonical_authority_sha256(&behavioral)?;
    let lifecycle_hash = canonical_authority_sha256(&lifecycle)?;
    validate_deterministic_binding(&candidate, &deterministic)?;
    if deterministic != recompute_deterministic(&candidate, artifact_root)? {
        return Err(anyhow!(
            "contained deterministic evaluation does not equal authoritative staged compilation"
        ));
    }
    let trial_hashes = parsed_trials
        .iter()
        .map(|(digest, _)| digest.clone())
        .collect::<Vec<_>>();
    if composite.candidate_id != candidate.candidate_id
        || composite.job_id != candidate.job_id
        || composite.fixture_id != candidate.fixture_id
        || composite.pack_release != candidate.pack_release
        || composite.candidate_sha256 != candidate_hash
        || composite.evaluator_inventory_sha256 != candidate.evaluator_inventory_sha256
        || composite.evaluator_inventory_sha256 != inventory.inventory_sha256
        || composite.lifecycle_policy_sha256 != lifecycle_hash
        || composite.deterministic_evaluation_sha256 != deterministic_hash
        || composite.behavioral_evaluation_sha256 != behavioral_hash
        || composite.deterministic_status != deterministic.derived_status()
        || behavioral.deterministic_evaluation_sha256 != deterministic_hash
        || behavioral.deterministic_status != deterministic.derived_status()
        || composite.behavioral_status != aggregate_behavioral_status(&behavioral)
        || composite.verdict != behavioral.overall_result
        || composite.trial_sha256s != trial_hashes
        || behavioral.trial_sha256s != trial_hashes
        || composite.limitations != behavioral.reason_codes
    {
        return Err(anyhow!(
            "composite top-level fields do not match contained authorities"
        ));
    }
    for artifact in &composite.journey.artifacts {
        let expected_approval = approval_members.iter().find_map(|(hash, approval)| {
            (approval.approves_exact_hash(&artifact.authority_sha256)
                && inventory
                    .trusted_publication_authorities
                    .iter()
                    .any(|trusted| {
                        trusted.reviewer_role == approval.reviewer_role
                            && trusted.identity_authority_sha256
                                == approval.identity_authority_sha256
                            && approval.verify_signature(&trusted.public_key_hex).is_ok()
                    }))
            .then_some(hash.as_str())
        });
        let expected_access = match lifecycle.access_class {
            AccessClass::Synthetic => AccessClass::Synthetic,
            AccessClass::SanitizedPublic if expected_approval.is_some() => {
                AccessClass::SanitizedPublic
            }
            _ => AccessClass::Private,
        };
        if artifact.access_class != expected_access
            || artifact.publication_approval_sha256.as_deref()
                != if expected_access == AccessClass::SanitizedPublic {
                    expected_approval
                } else {
                    None
                }
        {
            return Err(anyhow!(
                "journey access classification does not match contained authority"
            ));
        }
    }
    Ok(())
}

fn validate_deterministic_binding(
    candidate: &ConformanceCandidateV1,
    deterministic: &crate::conformance::DeterministicConformanceV1,
) -> Result<()> {
    if deterministic.candidate_id != candidate.candidate_id
        || deterministic.job_id != candidate.job_id
        || deterministic.fixture_id != candidate.fixture_id
        || deterministic.challenge_id != candidate.challenge_id
        || deterministic.evaluator.inventory_sha256 != candidate.evaluator_inventory_sha256
        || deterministic.pack_release != candidate.pack_release
    {
        return Err(anyhow!(
            "deterministic evaluation does not bind the candidate"
        ));
    }
    Ok(())
}

fn recompute_deterministic(
    candidate: &ConformanceCandidateV1,
    artifact_root: &Path,
) -> Result<crate::conformance::DeterministicConformanceV1> {
    let candidate_root = contained_candidate_root(artifact_root, &candidate.artifact_root)?;
    let loaded = load_authorities(candidate, candidate_root)?;
    let value = compile_candidate(candidate, &loaded)?;
    let recomputed: crate::conformance::DeterministicConformanceV1 = serde_json::from_value(value)?;
    recomputed.validate()?;
    Ok(recomputed)
}

fn load_candidate_approvals(
    candidate: &ConformanceCandidateV1,
    root: &Path,
    inventory: &EvaluatorInventoryV1,
) -> Result<Vec<(String, PublicationApprovalV1)>> {
    candidate
        .authorities
        .iter()
        .filter(|reference| reference.role == CandidateAuthorityRole::PublicationApproval)
        .map(|reference| {
            let bytes = read_contained_authority_bytes(
                root,
                reference,
                crate::conformance::MAX_CONFORMANCE_AUTHORITY_BYTES,
            )?;
            let approval = parse_publication_approval(&bytes)?;
            if !inventory
                .trusted_publication_authorities
                .iter()
                .any(|trusted| {
                    trusted.reviewer_role == approval.reviewer_role
                        && trusted.identity_authority_sha256 == approval.identity_authority_sha256
                        && approval.verify_signature(&trusted.public_key_hex).is_ok()
                })
            {
                return Err(anyhow!(
                    "publication approval authority is not trusted by evaluator inventory"
                ));
            }
            Ok((canonical_authority_sha256(&approval)?, approval))
        })
        .collect()
}

fn public_access(
    lifecycle: &PrivateRecordPolicyV1,
    approvals: &[(String, PublicationApprovalV1)],
    digest: &str,
) -> AccessClass {
    match lifecycle.access_class {
        AccessClass::Synthetic => AccessClass::Synthetic,
        AccessClass::SanitizedPublic
            if approvals
                .iter()
                .any(|(_, approval)| approval.approves_exact_hash(digest)) =>
        {
            AccessClass::SanitizedPublic
        }
        _ => AccessClass::Private,
    }
}

fn approval_hash(
    lifecycle: &PrivateRecordPolicyV1,
    approvals: &[(String, PublicationApprovalV1)],
    digest: &str,
) -> Result<Option<String>> {
    if lifecycle.access_class != AccessClass::SanitizedPublic {
        return Ok(None);
    }
    Ok(approvals
        .iter()
        .find(|(_, approval)| approval.approves_exact_hash(digest))
        .map(|(hash, _)| hash.clone()))
}

fn journey_role(role: CandidateAuthorityRole) -> JourneyArtifactRole {
    match role {
        CandidateAuthorityRole::PackManifest => JourneyArtifactRole::PackRelease,
        CandidateAuthorityRole::Requirements => JourneyArtifactRole::Requirements,
        CandidateAuthorityRole::ProductFoundation => JourneyArtifactRole::ProductFoundation,
        CandidateAuthorityRole::SkillsRoute => JourneyArtifactRole::SkillsRoute,
        CandidateAuthorityRole::Prompt => JourneyArtifactRole::Prompt,
        CandidateAuthorityRole::PromptInvocation => JourneyArtifactRole::PromptInvocation,
        CandidateAuthorityRole::SourceLineage => JourneyArtifactRole::SourceLineage,
        CandidateAuthorityRole::NormalizedInput => JourneyArtifactRole::NormalizedInput,
        CandidateAuthorityRole::RoutedContext => JourneyArtifactRole::RoutedContext,
        CandidateAuthorityRole::GovernedOutput => JourneyArtifactRole::GovernedOutput,
        CandidateAuthorityRole::ClaimsValidation => JourneyArtifactRole::ClaimsValidation,
        CandidateAuthorityRole::DecisionResult => JourneyArtifactRole::DecisionResult,
        CandidateAuthorityRole::RunBundle => JourneyArtifactRole::RunBundle,
        CandidateAuthorityRole::RunReceipt => JourneyArtifactRole::RunReceipt,
        CandidateAuthorityRole::RunVerification => JourneyArtifactRole::RunVerification,
        CandidateAuthorityRole::EvaluatorInventory => JourneyArtifactRole::EvaluatorInventory,
        CandidateAuthorityRole::PrivateRecordPolicy => JourneyArtifactRole::PrivateRecordPolicy,
        CandidateAuthorityRole::PublicationApproval => JourneyArtifactRole::PublicationApproval,
    }
}

fn journey_phase(role: CandidateAuthorityRole) -> JourneyPhase {
    match role {
        CandidateAuthorityRole::NormalizedInput
        | CandidateAuthorityRole::SourceLineage
        | CandidateAuthorityRole::PromptInvocation => JourneyPhase::Normalization,
        CandidateAuthorityRole::RoutedContext | CandidateAuthorityRole::DecisionResult => {
            JourneyPhase::Selection
        }
        CandidateAuthorityRole::GovernedOutput
        | CandidateAuthorityRole::RunBundle
        | CandidateAuthorityRole::RunReceipt => JourneyPhase::Generation,
        CandidateAuthorityRole::ClaimsValidation | CandidateAuthorityRole::RunVerification => {
            JourneyPhase::Review
        }
        CandidateAuthorityRole::PublicationApproval => JourneyPhase::Publication,
        _ => JourneyPhase::Candidate,
    }
}

fn role_token(role: JourneyArtifactRole) -> &'static str {
    match role {
        JourneyArtifactRole::Candidate => "candidate",
        JourneyArtifactRole::PackRelease => "pack-release",
        JourneyArtifactRole::Requirements => "requirements",
        JourneyArtifactRole::ProductFoundation => "product-foundation",
        JourneyArtifactRole::SkillsRoute => "skills-route",
        JourneyArtifactRole::Prompt => "prompt",
        JourneyArtifactRole::PromptInvocation => "prompt-invocation",
        JourneyArtifactRole::SourceLineage => "source-lineage",
        JourneyArtifactRole::NormalizedInput => "normalized-input",
        JourneyArtifactRole::RoutedContext => "routed-context",
        JourneyArtifactRole::GovernedOutput => "governed-output",
        JourneyArtifactRole::ClaimsValidation => "claims-validation",
        JourneyArtifactRole::DecisionResult => "decision-result",
        JourneyArtifactRole::RunBundle => "run-bundle",
        JourneyArtifactRole::RunReceipt => "run-receipt",
        JourneyArtifactRole::RunVerification => "run-verification",
        JourneyArtifactRole::EvaluatorInventory => "evaluator-inventory",
        JourneyArtifactRole::PrivateRecordPolicy => "private-record-policy",
        JourneyArtifactRole::PublicationApproval => "publication-approval",
        JourneyArtifactRole::DeterministicEvaluation => "deterministic-evaluation",
        JourneyArtifactRole::BehavioralEvaluation => "behavioral-evaluation",
        JourneyArtifactRole::Trial => "trial",
    }
}

fn build_journey_links(artifacts: &[JourneyArtifact]) -> Vec<JourneyLink> {
    let mut links = Vec::new();
    for artifact in artifacts
        .iter()
        .filter(|item| item.artifact_id != "candidate")
    {
        links.push(JourneyLink {
            from_artifact_id: "candidate".into(),
            to_artifact_id: artifact.artifact_id.clone(),
            relation: JourneyRelation::Declares,
        });
    }
    if let Some(requirements) = artifacts
        .iter()
        .find(|item| item.role == JourneyArtifactRole::Requirements)
    {
        links.push(JourneyLink {
            from_artifact_id: requirements.artifact_id.clone(),
            to_artifact_id: "deterministic-evaluation".into(),
            relation: JourneyRelation::Evaluates,
        });
    }
    links.push(JourneyLink {
        from_artifact_id: "deterministic-evaluation".into(),
        to_artifact_id: "behavioral-evaluation".into(),
        relation: JourneyRelation::BoundTo,
    });
    for artifact in artifacts
        .iter()
        .filter(|item| item.role == JourneyArtifactRole::Trial)
    {
        links.push(JourneyLink {
            from_artifact_id: artifact.artifact_id.clone(),
            to_artifact_id: "behavioral-evaluation".into(),
            relation: JourneyRelation::Evaluates,
        });
    }
    links
}

fn aggregate_behavioral_status(evaluation: &BehavioralEvaluation) -> BehavioralStatus {
    if evaluation
        .trials
        .iter()
        .any(|trial| trial.status == BehavioralStatus::Malformed)
    {
        BehavioralStatus::Malformed
    } else {
        match evaluation.behavioral_qualification {
            BehavioralQualification::QualifiedForJobUnderEnvelope => BehavioralStatus::Passed,
            BehavioralQualification::NotQualifiedForJobUnderEnvelope => BehavioralStatus::Failed,
            BehavioralQualification::Unassessed => BehavioralStatus::Unassessed,
        }
    }
}

fn load_inventory_from_composite(
    composite: &JobConformanceV1,
    artifact_root: &Path,
) -> Result<EvaluatorInventoryV1> {
    let artifact = composite
        .journey
        .artifacts
        .iter()
        .find(|artifact| artifact.role == JourneyArtifactRole::EvaluatorInventory)
        .ok_or_else(|| anyhow!("composite lacks evaluator inventory"))?;
    let path = artifact
        .relative_path
        .as_deref()
        .ok_or_else(|| anyhow!("evaluator inventory cannot be opaque"))?;
    let inventory =
        parse_evaluator_inventory(&read_contained_file(artifact_root, Path::new(path))?)?;
    if inventory.inventory_sha256 != composite.evaluator_inventory_sha256 {
        return Err(anyhow!("composite evaluator inventory binding mismatch"));
    }
    Ok(inventory)
}

fn path_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("authority path must be valid UTF-8"))
}

fn contained_candidate_root(artifact_root: &Path, relative: &str) -> Result<PathBuf> {
    let staged_root = artifact_root.canonicalize()?;
    let candidate_root = staged_root.join(relative).canonicalize()?;
    if !candidate_root.starts_with(&staged_root) || !candidate_root.is_dir() {
        return Err(anyhow!(
            "candidate artifact root is not a contained directory"
        ));
    }
    Ok(candidate_root)
}

fn load_authorities<'a>(
    candidate: &'a ConformanceCandidateV1,
    root: PathBuf,
) -> Result<LoadedAuthorities<'a>> {
    let mut references = HashMap::new();
    for authority in &candidate.authorities {
        if references.insert(authority.role, authority).is_some() {
            return Err(anyhow!("candidate contains duplicate authority roles"));
        }
    }
    let pack_manifest = references
        .get(&CandidateAuthorityRole::PackManifest)
        .copied()
        .ok_or_else(|| anyhow!("candidate is missing pack manifest authority"))?;
    let pack_root = pack_root_from_manifest(&root, &pack_manifest.relative_path)?;

    let mut values = HashMap::new();
    let mut bytes = HashMap::new();
    for authority in &candidate.authorities {
        let authority_bytes =
            read_contained_authority_bytes(&root, authority, MAX_CONFORMANCE_AUTHORITY_BYTES)?;
        if matches!(authority.role, CandidateAuthorityRole::PackManifest) {
            bytes.insert(authority.role, authority_bytes);
            continue;
        }
        if matches!(authority.role, CandidateAuthorityRole::Prompt) {
            bytes.insert(authority.role, authority_bytes);
            continue;
        }
        let value: Value = parse_authority_json(&authority_bytes, conformance_limits())?;
        if value["contract"].as_str() != Some(authority.contract.as_str()) {
            return Err(anyhow!("authority contract discriminator mismatch"));
        }
        values.insert(authority.role, value);
        bytes.insert(authority.role, authority_bytes);
    }
    Ok(LoadedAuthorities {
        root,
        pack_root,
        references,
        values,
        bytes,
    })
}

fn pack_root_from_manifest(root: &Path, relative_path: &str) -> Result<PathBuf> {
    let manifest_path = Path::new(relative_path);
    if manifest_path.file_name().and_then(|name| name.to_str()) != Some("manifest.yaml")
        || manifest_path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            != Some(".mdp")
    {
        return Err(anyhow!(
            "pack manifest authority must identify .mdp/manifest.yaml"
        ));
    }
    let prefix = manifest_path
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new(""));
    let pack_root = root.join(prefix).canonicalize()?;
    if !pack_root.starts_with(root) || !pack_root.is_dir() {
        return Err(anyhow!("pack root escapes candidate artifact root"));
    }
    Ok(pack_root)
}

fn compile_candidate(
    candidate: &ConformanceCandidateV1,
    loaded: &LoadedAuthorities<'_>,
) -> Result<Value> {
    let manifest = read_manifest(&loaded.pack_root)?;
    let validation = validate_pack(&loaded.pack_root)?;
    let compiled_requirements = requirements(&loaded.pack_root, &candidate.job_id)?;
    let compiled_skills = skills(Some(&loaded.pack_root), Some(&candidate.job_id));
    let staged_requirements = loaded.values.get(&CandidateAuthorityRole::Requirements);

    let release_matches = manifest.id == candidate.pack_release.pack_id
        && manifest.version == candidate.pack_release.version
        && pack_content_sha256(&loaded.pack_root)? == candidate.pack_release.portable_digest
        && candidate.cli_version == env!("CARGO_PKG_VERSION");
    let selected_job_valid = compiled_requirements["valid"] == true
        && compiled_requirements["job"]["id"] == candidate.job_id;
    let requirements_match = staged_requirements == Some(&compiled_requirements);
    let skills_ready = compiled_skills["valid"] == true
        && compiled_skills["requested_job"] == candidate.job_id
        && !compiled_skills["recommendation"].is_null();

    let evaluator = load_evaluator(candidate, loaded)?;
    let lifecycle = load_lifecycle(candidate, loaded)?;
    let privacy_valid = validate_fixture_privacy(&lifecycle, &evaluator, loaded)?;
    let challenge = evaluator.challenges.iter().find(|challenge| {
        challenge.fixture_id == candidate.fixture_id
            && challenge.job_id == candidate.job_id
            && candidate
                .challenge_id
                .as_deref()
                .is_none_or(|id| id == challenge.challenge_id)
    });
    let challenge_valid = challenge.is_some();

    let prompt_integrity = validate_prompt_integrity(&compiled_requirements, loaded)?;
    let decision = loaded.values.get(&CandidateAuthorityRole::DecisionResult);
    let decision_valid = decision
        .zip(challenge)
        .is_some_and(|(value, challenge)| decision_matches_expected(value, challenge));
    let routed = loaded.values.get(&CandidateAuthorityRole::RoutedContext);
    let routed_valid = routed.is_some_and(|value| {
        jsonschema::draft202012::validate(&schema(SchemaTarget::RoutedContextV1), value).is_ok()
    });
    let output_validation = validate_governed_output(loaded, candidate)?;
    let gap_propagation =
        validate_gap_propagation(candidate, &evaluator, decision, output_validation.as_ref());
    let trace_agreement = validate_trace_agreement(loaded)?;

    let mut assertions = Vec::with_capacity(12);
    assertions.push(assertion(
        "D1",
        "release-integrity",
        "release",
        pass_if(validation["valid"] == true && release_matches),
        loaded,
        &[CandidateAuthorityRole::PackManifest],
        "d1-release-integrity",
    ));
    assertions.push(assertion(
        "D2",
        "job-closure",
        "release",
        pass_if(selected_job_valid && requirements_match && skills_ready),
        loaded,
        &[
            CandidateAuthorityRole::Requirements,
            CandidateAuthorityRole::SkillsRoute,
        ],
        "d2-job-closure",
    ));
    assertions.push(assertion(
        "D3",
        "input-completeness",
        "fixture",
        if loaded
            .references
            .contains_key(&CandidateAuthorityRole::SourceLineage)
        {
            pass_if(
                selected_job_valid
                    && authority_passes(loaded, CandidateAuthorityRole::SourceLineage),
            )
        } else {
            AssertionStatus::Unassessed
        },
        loaded,
        &[
            CandidateAuthorityRole::Requirements,
            CandidateAuthorityRole::SourceLineage,
            CandidateAuthorityRole::NormalizedInput,
        ],
        "d3-input-completeness",
    ));
    assertions.push(assertion(
        "D4",
        "vocabulary-closure",
        "release",
        pass_if(selected_job_valid && requirements_match),
        loaded,
        &[CandidateAuthorityRole::Requirements],
        "d4-vocabulary-closure",
    ));
    assertions.push(assertion(
        "D5",
        "prompt-integrity",
        "release",
        pass_if(prompt_integrity),
        loaded,
        &[
            CandidateAuthorityRole::Requirements,
            CandidateAuthorityRole::Prompt,
            CandidateAuthorityRole::PromptInvocation,
        ],
        "d5-prompt-integrity",
    ));
    assertions.push(assertion(
        "D6",
        "decision-authority",
        "fixture",
        optional_pass(decision.is_some(), decision_valid && challenge_valid),
        loaded,
        &[CandidateAuthorityRole::DecisionResult],
        "d6-decision-authority",
    ));
    assertions.push(assertion(
        "D7",
        "bounded-routing",
        "fixture",
        optional_pass(routed.is_some(), routed_valid),
        loaded,
        &[CandidateAuthorityRole::RoutedContext],
        "d7-bounded-routing",
    ));
    assertions.push(assertion(
        "D8",
        "output-validity",
        "fixture",
        output_validation
            .as_ref()
            .map_or(AssertionStatus::Unassessed, |value| {
                pass_if(existing_result_passes(value))
            }),
        loaded,
        &[
            CandidateAuthorityRole::GovernedOutput,
            CandidateAuthorityRole::ClaimsValidation,
        ],
        "d8-output-validity",
    ));
    assertions.push(assertion(
        "D9",
        "gap-propagation",
        "fixture",
        gap_propagation,
        loaded,
        &[
            CandidateAuthorityRole::DecisionResult,
            CandidateAuthorityRole::GovernedOutput,
        ],
        "d9-gap-propagation",
    ));
    assertions.push(assertion(
        "D10",
        "trace-agreement",
        "fixture",
        trace_agreement,
        loaded,
        &[
            CandidateAuthorityRole::RunBundle,
            CandidateAuthorityRole::RunReceipt,
            CandidateAuthorityRole::RunVerification,
            CandidateAuthorityRole::DecisionResult,
        ],
        "d10-trace-agreement",
    ));
    assertions.push(assertion(
        "D11",
        "discoverability",
        "release",
        pass_if(selected_job_valid && skills_ready),
        loaded,
        &[
            CandidateAuthorityRole::Requirements,
            CandidateAuthorityRole::SkillsRoute,
        ],
        "d11-discoverability",
    ));
    assertions.push(assertion(
        "D12",
        "pack-fixture-privacy",
        "fixture",
        pass_if(privacy_valid),
        loaded,
        &[
            CandidateAuthorityRole::PrivateRecordPolicy,
            CandidateAuthorityRole::PublicationApproval,
        ],
        "d12-pack-fixture-privacy",
    ));

    let has_failure = assertions.iter().any(|item| item["status"] == "fail");
    let has_unassessed = assertions.iter().any(|item| item["status"] == "unassessed");
    let status = if has_failure {
        "not-sufficient-for-job"
    } else if has_unassessed {
        "unassessed"
    } else {
        "sufficient-for-job"
    };

    Ok(json!({
        "contract": DETERMINISTIC_CONFORMANCE_V1,
        "valid": !has_failure && !has_unassessed,
        "candidate_id": candidate.candidate_id,
        "job_id": candidate.job_id,
        "pack_release": candidate.pack_release,
        "evaluator": {
            "id": evaluator.evaluator_id,
            "version": evaluator.evaluator_version,
            "fixture_set_id": evaluator.fixture_set_id,
            "inventory_sha256": evaluator.inventory_sha256
        },
        "fixture_id": candidate.fixture_id,
        "challenge_id": candidate.challenge_id,
        "status": status,
        "behavioral_qualification_allowed": status == "sufficient-for-job",
        "assertions": assertions,
        "summary": {
            "passed": assertions.iter().filter(|item| item["status"] == "pass").count(),
            "failed": assertions.iter().filter(|item| item["status"] == "fail").count(),
            "unassessed": assertions.iter().filter(|item| item["status"] == "unassessed").count()
        }
    }))
}

fn validate_fixture_privacy(
    lifecycle: &PrivateRecordPolicyV1,
    evaluator: &EvaluatorInventoryV1,
    loaded: &LoadedAuthorities<'_>,
) -> Result<bool> {
    lifecycle.validate()?;
    match lifecycle.access_class {
        AccessClass::Synthetic => Ok(true),
        AccessClass::Private => Ok(true),
        AccessClass::SanitizedPublic => {
            let Some(value) = loaded
                .values
                .get(&CandidateAuthorityRole::PublicationApproval)
            else {
                return Ok(false);
            };
            let approval: PublicationApprovalV1 = serde_json::from_value(value.clone())?;
            approval.validate()?;
            if !publication_approval_is_trusted(evaluator, &approval) {
                return Ok(false);
            }
            let Some(fixture) = loaded
                .references
                .get(&CandidateAuthorityRole::GovernedOutput)
                .or_else(|| {
                    loaded
                        .references
                        .get(&CandidateAuthorityRole::NormalizedInput)
                })
            else {
                return Ok(false);
            };
            Ok(approval.approves_exact_hash(&fixture.sha256))
        }
    }
}

fn publication_approval_is_trusted(
    evaluator: &EvaluatorInventoryV1,
    approval: &PublicationApprovalV1,
) -> bool {
    evaluator
        .trusted_publication_authorities
        .iter()
        .any(|authority| {
            authority.reviewer_role == approval.reviewer_role
                && authority.identity_authority_sha256 == approval.identity_authority_sha256
                && approval.verify_signature(&authority.public_key_hex).is_ok()
        })
}

fn decision_matches_expected(
    decision: &Value,
    challenge: &crate::conformance::EvaluatorChallenge,
) -> bool {
    if challenge.expected_terminal_state.is_success() {
        return existing_result_passes(decision);
    }
    let expected = serde_json::to_value(challenge.expected_terminal_state).ok();
    decision["terminal_state"] == expected.clone().unwrap_or(Value::Null)
        || decision["status"] == expected.unwrap_or(Value::Null)
}

fn load_evaluator(
    candidate: &ConformanceCandidateV1,
    loaded: &LoadedAuthorities<'_>,
) -> Result<EvaluatorInventoryV1> {
    let evaluator: EvaluatorInventoryV1 = serde_json::from_value(
        loaded
            .values
            .get(&CandidateAuthorityRole::EvaluatorInventory)
            .ok_or_else(|| anyhow!("candidate is missing evaluator inventory authority"))?
            .clone(),
    )?;
    evaluator.validate()?;
    if evaluator.inventory_sha256 != candidate.evaluator_inventory_sha256 {
        return Err(anyhow!("candidate evaluator inventory digest mismatch"));
    }
    Ok(evaluator)
}

fn load_lifecycle(
    candidate: &ConformanceCandidateV1,
    loaded: &LoadedAuthorities<'_>,
) -> Result<PrivateRecordPolicyV1> {
    let lifecycle: PrivateRecordPolicyV1 = serde_json::from_value(
        loaded
            .values
            .get(&CandidateAuthorityRole::PrivateRecordPolicy)
            .ok_or_else(|| anyhow!("candidate is missing private record policy authority"))?
            .clone(),
    )?;
    lifecycle.validate()?;
    if canonical_authority_sha256(&lifecycle)? != candidate.lifecycle_policy_sha256 {
        return Err(anyhow!("candidate lifecycle policy digest mismatch"));
    }
    Ok(lifecycle)
}

fn authority_ref<'a>(
    loaded: &'a LoadedAuthorities<'a>,
    role: CandidateAuthorityRole,
) -> Result<&'a StagedAuthorityRef> {
    loaded
        .references
        .get(&role)
        .copied()
        .ok_or_else(|| anyhow!("candidate is missing required authority"))
}

fn validate_prompt_integrity(requirements: &Value, loaded: &LoadedAuthorities<'_>) -> Result<bool> {
    if requirements["model_task"]["status"] != "ready" {
        return Ok(false);
    }
    let prompt = authority_ref(loaded, CandidateAuthorityRole::Prompt)?;
    let prompt_bytes = loaded
        .bytes
        .get(&prompt.role)
        .ok_or_else(|| anyhow!("prompt authority bytes were not retained"))?;
    let prompt: crate::models::PromptFile = serde_yaml::from_slice(prompt_bytes)?;
    let prompt_value = serde_json::to_value(prompt)?;
    Ok(canonical_json_sha256(&prompt_value)? == requirements["model_task"]["prompt_sha256"])
}

fn validate_governed_output(
    loaded: &LoadedAuthorities<'_>,
    candidate: &ConformanceCandidateV1,
) -> Result<Option<Value>> {
    let Some(output) = loaded
        .references
        .get(&CandidateAuthorityRole::GovernedOutput)
    else {
        return Ok(None);
    };
    let prompt = authority_ref(loaded, CandidateAuthorityRole::Prompt)?;
    let routed = loaded
        .references
        .get(&CandidateAuthorityRole::RoutedContext);
    let output_value = loaded
        .values
        .get(&CandidateAuthorityRole::GovernedOutput)
        .ok_or_else(|| anyhow!("governed output authority was not loaded"))?;
    let routed_value = loaded.values.get(&CandidateAuthorityRole::RoutedContext);
    let invocation = loaded
        .references
        .get(&CandidateAuthorityRole::PromptInvocation);
    let invocation_value = loaded.values.get(&CandidateAuthorityRole::PromptInvocation);
    let validation = crate::commands::prompt_output::validate_prompt_output_value_with_inputs(
        &loaded.pack_root,
        output_value,
        &output.relative_path,
        Some(&loaded.root.join(&prompt.relative_path)),
        None,
        None,
        None,
        None,
        None,
        None,
        invocation.zip(invocation_value).map(|(reference, value)| {
            (
                value,
                reference.relative_path.as_str(),
                reference.sha256.as_str(),
            )
        }),
        routed.zip(routed_value).map(|(reference, value)| {
            (
                value,
                reference.relative_path.as_str(),
                reference.sha256.as_str(),
            )
        }),
    )?;
    if validation["job_id"]
        .as_str()
        .is_some_and(|job| job != candidate.job_id)
    {
        return Ok(Some(
            json!({"valid": false, "issues": [{"code": "candidate-job-mismatch"}]}),
        ));
    }
    if let Some(claims) = loaded.values.get(&CandidateAuthorityRole::ClaimsValidation)
        && !existing_result_passes(claims)
    {
        return Ok(Some(
            json!({"valid": false, "issues": [{"code": "claims-validation-failed"}]}),
        ));
    }
    Ok(Some(validation))
}

fn validate_gap_propagation(
    candidate: &ConformanceCandidateV1,
    evaluator: &EvaluatorInventoryV1,
    decision: Option<&Value>,
    output: Option<&Value>,
) -> AssertionStatus {
    let Some(challenge) = evaluator.challenges.iter().find(|challenge| {
        challenge.fixture_id == candidate.fixture_id
            && challenge.job_id == candidate.job_id
            && candidate
                .challenge_id
                .as_deref()
                .is_none_or(|id| id == challenge.challenge_id)
    }) else {
        return AssertionStatus::Fail;
    };
    let expected = serde_json::to_value(challenge.expected_terminal_state)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned));
    let Some(expected) = expected else {
        return AssertionStatus::Fail;
    };
    if expected == "success" {
        return output.map_or(AssertionStatus::Unassessed, |value| {
            pass_if(existing_result_passes(value))
        });
    }
    let Some(decision) = decision else {
        return AssertionStatus::Unassessed;
    };
    let observed = decision["terminal_state"]
        .as_str()
        .or_else(|| decision["status"].as_str());
    pass_if(observed == Some(expected.as_str()) && output.is_none())
}

fn validate_trace_agreement(loaded: &LoadedAuthorities<'_>) -> Result<AssertionStatus> {
    let Some(decision) = loaded.values.get(&CandidateAuthorityRole::DecisionResult) else {
        return Ok(AssertionStatus::Unassessed);
    };
    let Some(reference) = loaded
        .references
        .get(&CandidateAuthorityRole::DecisionResult)
    else {
        return Ok(AssertionStatus::Unassessed);
    };
    let trace = project_source_value(decision, reference.sha256.clone());
    let trace_value = serde_json::to_value(trace).unwrap_or(Value::Null);
    if trace_value["status"] == "unavailable" {
        return Ok(AssertionStatus::Fail);
    }
    let (Some(bundle), Some(receipt), Some(staged_verification)) = (
        loaded.values.get(&CandidateAuthorityRole::RunBundle),
        loaded.values.get(&CandidateAuthorityRole::RunReceipt),
        loaded.values.get(&CandidateAuthorityRole::RunVerification),
    ) else {
        return Ok(AssertionStatus::Unassessed);
    };
    let bundle: RunBundleV1 = serde_json::from_value(bundle.clone())?;
    let receipt: RunReceiptV1 = serde_json::from_value(receipt.clone())?;
    let recomputed = serde_json::to_value(verify_run(&bundle, &receipt, Some(&loaded.root))?)?;
    Ok(pass_if(
        recomputed["valid"] == true && recomputed == *staged_verification,
    ))
}

fn authority_passes(loaded: &LoadedAuthorities<'_>, role: CandidateAuthorityRole) -> bool {
    loaded.values.get(&role).is_some_and(existing_result_passes)
}

fn existing_result_passes(value: &Value) -> bool {
    if let Some(valid) = value["valid"].as_bool() {
        return valid;
    }
    matches!(
        value["status"].as_str(),
        Some("ready" | "passed" | "pass" | "success" | "fit" | "lineage-validated")
    )
}

fn pass_if(condition: bool) -> AssertionStatus {
    if condition {
        AssertionStatus::Pass
    } else {
        AssertionStatus::Fail
    }
}

fn optional_pass(present: bool, condition: bool) -> AssertionStatus {
    if !present {
        AssertionStatus::Unassessed
    } else {
        pass_if(condition)
    }
}

fn assertion(
    id: &str,
    name: &str,
    scope: &str,
    status: AssertionStatus,
    loaded: &LoadedAuthorities<'_>,
    roles: &[CandidateAuthorityRole],
    reason_prefix: &str,
) -> Value {
    let authority_refs = roles
        .iter()
        .filter_map(|role| {
            loaded
                .references
                .get(role)
                .map(|authority| (role, authority))
        })
        .map(|(role, authority)| {
            json!({
                "role": role,
                "contract": authority.contract,
                "relative_path": authority.relative_path,
                "sha256": authority.sha256
            })
        })
        .collect::<Vec<_>>();
    json!({
        "id": id,
        "name": name,
        "scope": scope,
        "hard": true,
        "status": status.as_str(),
        "authority_refs": authority_refs,
        "reason_codes": [format!("{reason_prefix}-{}", status.as_str())]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_hash::sha256_hex;
    use crate::commands::init::init_pack;
    use crate::conformance::{
        AssertionEvaluationStatus, BEHAVIORAL_EVALUATION_V1, BehavioralEvaluation,
        BehavioralQualification, ConformanceAssertionEvaluation, DeterministicStatus,
        EVALUATOR_INVENTORY_V1, JobSufficiency, PRIVATE_RECORD_POLICY_V1, QualificationVerdict,
        hash_authority_value,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn deterministic_status_requires_all_twelve_assertions_to_pass() {
        assert_eq!(pass_if(true).as_str(), "pass");
        assert_eq!(pass_if(false).as_str(), "fail");
        assert_eq!(optional_pass(false, true).as_str(), "unassessed");
    }

    #[test]
    fn existing_validator_results_are_composed_without_reimplementing_rules() {
        assert!(existing_result_passes(&json!({"valid": true})));
        assert!(existing_result_passes(&json!({"status": "ready"})));
        assert!(!existing_result_passes(&json!({"valid": false})));
        assert!(!existing_result_passes(&json!({"status": "blocked"})));
    }

    #[cfg(unix)]
    #[test]
    fn behavioral_validation_rejects_symlinked_evidence_under_artifact_root() {
        use std::os::unix::fs::symlink;

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let base = std::env::temp_dir().join(format!("mdp-contained-evidence-{nonce}"));
        let root = base.join("staged");
        fs::create_dir_all(&root).unwrap();
        let candidate = json!({
            "contract": "mdp.conformance-candidate.v1",
            "candidate_id": "candidate-1",
            "artifact_root": "candidate",
            "job_id": "outbound-copy-brief",
            "pack_release": {
                "pack_id": "pack",
                "release_id": "release",
                "version": "1.0.0",
                "portable_digest": "a".repeat(64),
                "source_revision": "b".repeat(64)
            },
            "cli_version": "0.1.0",
            "fixture_id": "fixture-1",
            "challenge_id": "challenge-1",
            "evaluator_inventory_sha256": "c".repeat(64),
            "authorities": [
                {"role":"pack-manifest","contract":"mdp.v0","relative_path":"pack/manifest.json","sha256":"d".repeat(64),"byte_count":100},
                {"role":"requirements","contract":"mdp.requirements.v2","relative_path":"pack/requirements.json","sha256":"e".repeat(64),"byte_count":100},
                {"role":"prompt","contract":"mdp.prompt.v1","relative_path":"pack/prompt.json","sha256":"f".repeat(64),"byte_count":100},
                {"role":"evaluator-inventory","contract":"mdp.evaluator-inventory.v1","relative_path":"evaluator/inventory.json","sha256":"c".repeat(64),"byte_count":100},
                {"role":"private-record-policy","contract":"mdp.private-record-policy.v1","relative_path":"policy/private.json","sha256":"9".repeat(64),"byte_count":100}
            ],
            "lifecycle_policy_sha256": "9".repeat(64)
        });
        let candidate_bytes = serde_json::to_vec(&candidate).unwrap();
        parse_candidate(&candidate_bytes).expect("outside fixture must be a valid candidate");
        fs::write(base.join("outside-candidate.json"), candidate_bytes).unwrap();
        symlink(
            base.join("outside-candidate.json"),
            root.join("candidate.json"),
        )
        .unwrap();
        let result = validate_behavioral_files(BehavioralEvidencePaths {
            artifact_root: &root,
            candidate: Path::new("candidate.json"),
            deterministic: Path::new("deterministic.json"),
            evaluator_inventory: Path::new("inventory.json"),
            lifecycle_policy: Path::new("policy.json"),
            invocations: &[],
            trials: &[],
            evaluator_results: &[],
            publication_approvals: &[],
            verifier_receipts: &[],
        });
        assert!(
            result
                .expect_err("symlinked candidate must fail containment")
                .to_string()
                .contains("cannot open contained authority component safely")
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn compile_emits_all_deterministic_assertions_for_one_exact_job() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let stage = std::env::temp_dir().join(format!("mdp-conformance-u2-{nonce}"));
        let candidate_root = stage.join("candidate");
        let pack_root = candidate_root.join("pack");
        let evidence_root = candidate_root.join("evidence");
        fs::create_dir_all(&evidence_root).expect("evidence root should be created");
        init_pack(&pack_root, "Conformance Test", "gtm", true, false)
            .expect("pack should initialize");

        let job_id = "outbound-copy-brief";
        let compiled = requirements(&pack_root, job_id).expect("requirements should compile");
        let requirements_bytes = serde_json::to_vec(&compiled).expect("requirements JSON");
        fs::write(evidence_root.join("requirements.json"), &requirements_bytes)
            .expect("requirements should stage");

        let slot_context = crate::artifact_hash::canonical_json_sha256_for_domain(
            "mdp.model-visible-context.v1",
            &json!([]),
        )
        .unwrap();
        let mut inventory = json!({
            "contract": EVALUATOR_INVENTORY_V1,
            "evaluator_id": "cold-model",
            "evaluator_version": "1.0.0",
            "fixture_set_id": "core",
            "frozen_at": "2026-08-13T10:00:00Z",
            "inventory_sha256": "",
            "trusted_verifiers": [{"verifier_name":"local-verifier","verifier_version":"1.0.0","verifier_config_sha256":"7".repeat(64),"identity_authority_sha256":"21fe31dfa154a261626bf854046fd2271b7bed4b6abe45aa58877ef47f9721b9","public_key_hex":"d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"}],
            "trusted_publication_authorities": [{"reviewer_role":"publication-reviewer","identity_authority_sha256":"21fe31dfa154a261626bf854046fd2271b7bed4b6abe45aa58877ef47f9721b9","public_key_hex":"d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"}],
            "challenges": [{
                "challenge_id": "challenge-1",
                "fixture_id": "fixture-1",
                "job_id": job_id,
                "phase": "generation",
                "expected_terminal_state": "success",
                "protected": true,
                "frozen_before_trials": true,
                "model_visible": false,
                "selection_method": "synthetic-fixture-inventory",
                "selection_version": "1",
                "created_at": "2026-08-13T09:00:00Z",
                "frozen_candidate_sha256": "a".repeat(64),
                "selection_receipt_sha256": "b".repeat(64),
                "prior_exposure": "never-exposed",
                "pack_authored": false,
                "reuse_allowed": true,
                "trial_slots": (1..=3).map(|number| json!({
                    "trial_id": format!("trial-{number}"),
                    "phase": "generation",
                    "requested_model": "requested-model",
                    "resolved_model": "resolved-model",
                    "prompt_sha256": "c".repeat(64),
                    "input_artifacts": [],
                    "model_visible_context_sha256": slot_context
                })).collect::<Vec<_>>()
            }],
            "assertions": (1..=9).map(|number| json!({
                "assertion_id": format!("B{number}"),
                "kind": if number == 6 { "useful-completion" } else { "hard-boundary" },
                "required_trials": 3,
                "minimum_passes": if number == 6 { 2 } else { 3 }
            })).collect::<Vec<_>>()
        });
        let mut unrelated_challenge = inventory["challenges"][0].clone();
        unrelated_challenge["challenge_id"] = json!("challenge-unrelated");
        unrelated_challenge["fixture_id"] = json!("fixture-unrelated");
        unrelated_challenge["job_id"] = json!("job-unrelated");
        inventory["challenges"]
            .as_array_mut()
            .expect("challenge inventory")
            .push(unrelated_challenge);
        let inventory_digest = hash_authority_value(EVALUATOR_INVENTORY_V1, &inventory)
            .expect("inventory should hash");
        inventory["inventory_sha256"] = json!(inventory_digest);
        let inventory_bytes = serde_json::to_vec(&inventory).expect("inventory JSON");
        fs::write(evidence_root.join("inventory.json"), &inventory_bytes)
            .expect("inventory should stage");

        let lifecycle = json!({
            "contract": PRIVATE_RECORD_POLICY_V1,
            "policy_id": "policy-1",
            "access_class": "synthetic",
            "policy_owner_or_ref": "owner:test",
            "retention_until": "2026-09-13T00:00:00Z",
            "deletion_disposition": "delete",
            "host_capabilities": {
                "access": "supported",
                "retention": "supported",
                "deletion": "supported"
            }
        });
        let lifecycle_bytes = serde_json::to_vec(&lifecycle).expect("lifecycle JSON");
        fs::write(evidence_root.join("lifecycle.json"), &lifecycle_bytes)
            .expect("lifecycle should stage");
        let mut private_lifecycle = lifecycle.clone();
        private_lifecycle["access_class"] = json!("private");
        let private_lifecycle = parse_lifecycle_policy(
            &serde_json::to_vec(&private_lifecycle).expect("private lifecycle JSON"),
        )
        .expect("private lifecycle should parse");
        let parsed_inventory =
            parse_evaluator_inventory(&inventory_bytes).expect("inventory should parse");
        let empty_loaded = LoadedAuthorities {
            root: candidate_root.clone(),
            pack_root: pack_root.clone(),
            references: HashMap::new(),
            values: HashMap::new(),
            bytes: HashMap::new(),
        };
        assert!(
            validate_fixture_privacy(&private_lifecycle, &parsed_inventory, &empty_loaded)
                .expect("policy-governed private fixture should be deterministic")
        );
        assert_eq!(
            public_access(&private_lifecycle, &[], &"f".repeat(64)),
            AccessClass::Private,
            "deterministic sufficiency must not relabel private fixture evidence"
        );
        let forged_approval = parse_publication_approval(
            &serde_json::to_vec(&json!({
                "contract": crate::conformance::PUBLICATION_APPROVAL_V1,
                "approval_id": "forged-approval",
                "artifact_sha256": "f".repeat(64),
                "classification": "sanitized-public",
                "approved_by": "Forged Reviewer",
                "reviewer_role": "publication-reviewer",
                "identity_authority_sha256": "21fe31dfa154a261626bf854046fd2271b7bed4b6abe45aa58877ef47f9721b9",
                "approved_at": "2026-08-13T12:00:00Z",
                "purpose": "public-conformance-report",
                "signature_hex": "0".repeat(128)
            }))
            .unwrap(),
        )
        .expect("shape-valid forged approval should parse");
        assert!(!publication_approval_is_trusted(
            &parsed_inventory,
            &forged_approval
        ));

        let manifest_path = pack_root.join(".mdp/manifest.yaml");
        let manifest_bytes = fs::read(&manifest_path).expect("manifest should read");
        let prompt_relative = compiled["model_task"]["prompt_path"]
            .as_str()
            .expect("job should have model prompt");
        let prompt_path = pack_root.join(prompt_relative);
        let prompt_bytes = fs::read(&prompt_path).expect("prompt should read");
        let manifest = read_manifest(&pack_root).expect("manifest should parse");
        let portable_digest = pack_content_sha256(&pack_root).expect("pack should hash");

        let authority = |role: &str, contract: &str, relative_path: &str, bytes: &[u8]| {
            json!({
                "role": role,
                "contract": contract,
                "relative_path": relative_path,
                "sha256": sha256_hex(bytes),
                "byte_count": bytes.len()
            })
        };
        let candidate = json!({
            "contract": crate::conformance::CONFORMANCE_CANDIDATE_V1,
            "candidate_id": "candidate-1",
            "artifact_root": "candidate",
            "job_id": job_id,
            "pack_release": {
                "pack_id": manifest.id,
                "release_id": "release-1",
                "version": manifest.version,
                "portable_digest": portable_digest,
                "source_revision": "a".repeat(64)
            },
            "cli_version": env!("CARGO_PKG_VERSION"),
            "fixture_id": "fixture-1",
            "challenge_id": "challenge-1",
            "evaluator_inventory_sha256": inventory["inventory_sha256"],
            "authorities": [
                authority("pack-manifest", "mdp.v0", "pack/.mdp/manifest.yaml", &manifest_bytes),
                authority("requirements", compiled["contract"].as_str().unwrap(), "evidence/requirements.json", &requirements_bytes),
                authority("prompt", "mdp.prompt.v1", &format!("pack/{prompt_relative}"), &prompt_bytes),
                authority("evaluator-inventory", EVALUATOR_INVENTORY_V1, "evidence/inventory.json", &inventory_bytes),
                authority("private-record-policy", PRIVATE_RECORD_POLICY_V1, "evidence/lifecycle.json", &lifecycle_bytes)
            ],
            "lifecycle_policy_sha256": canonical_authority_sha256(
                &parse_lifecycle_policy(&lifecycle_bytes).expect("lifecycle should parse")
            ).expect("lifecycle should hash")
        });
        let candidate_path = stage.join("candidate.json");
        fs::write(
            &candidate_path,
            serde_json::to_vec(&candidate).expect("candidate JSON"),
        )
        .expect("candidate should stage");

        let result = compile_candidate_file(&candidate_path, &stage)
            .expect("candidate should compile deterministically");
        let output_schema = schema(SchemaTarget::DeterministicConformanceV1);
        jsonschema::draft202012::validate(&output_schema, &result)
            .expect("actual deterministic conformance output should match its schema");
        let mut unknown = result.clone();
        unknown["unexpected"] = json!(true);
        assert!(jsonschema::draft202012::validate(&output_schema, &unknown).is_err());
        let mut invalid_status = result.clone();
        invalid_status["assertions"][0]["status"] = json!("passing");
        assert!(jsonschema::draft202012::validate(&output_schema, &invalid_status).is_err());
        assert_eq!(result["assertions"].as_array().map(Vec::len), Some(12));
        assert_eq!(result["assertions"][0]["id"], "D1");
        assert_eq!(result["assertions"][11]["id"], "D12");
        assert_eq!(result["assertions"][0]["status"], "pass");
        assert_eq!(result["status"], "unassessed");
        assert_eq!(result["behavioral_qualification_allowed"], false);
        let expected_replay_bytes = serde_json::to_vec(&result).expect("result should serialize");
        for _ in 0..20 {
            let replay = compile_candidate_file(&candidate_path, &stage)
                .expect("identical candidate replay should compile");
            assert_eq!(
                serde_json::to_vec(&replay).expect("replay should serialize"),
                expected_replay_bytes,
                "identical candidate compilation must be byte-stable"
            );
        }

        let candidate_contract =
            parse_candidate(&fs::read(&candidate_path).expect("candidate should remain staged"))
                .expect("candidate should parse");
        assert_eq!(
            matching_inventory_challenge(&candidate_contract, &parsed_inventory)
                .expect("unrelated inventory challenges must not affect the selected candidate")
                .challenge_id,
            "challenge-1"
        );
        let deterministic_path = stage.join("deterministic.json");
        fs::write(
            &deterministic_path,
            serde_json::to_vec(&result).expect("deterministic JSON"),
        )
        .expect("deterministic result should stage");
        let preflight = ["Q1", "Q2", "Q3", "Q4"]
            .into_iter()
            .map(|id| ConformanceAssertionEvaluation {
                id: id.into(),
                status: AssertionEvaluationStatus::NotApplicable,
                passed_trials: 0,
                required_trials: crate::conformance::REQUIRED_COLD_TRIALS as u8,
                reason_codes: vec!["behavioral-trials-not-run".into()],
            })
            .collect();
        let behavioral = BehavioralEvaluation {
            contract: BEHAVIORAL_EVALUATION_V1.into(),
            valid: false,
            job_id: job_id.into(),
            candidate_sha256: canonical_authority_sha256(&candidate_contract)
                .expect("candidate should hash"),
            evaluator_inventory_sha256: candidate_contract.evaluator_inventory_sha256.clone(),
            lifecycle_policy_sha256: candidate_contract.lifecycle_policy_sha256.clone(),
            deterministic_evaluation_sha256:
                crate::artifact_hash::canonical_json_sha256_for_domain(
                    DETERMINISTIC_CONFORMANCE_V1,
                    &result,
                )
                .expect("deterministic result should hash"),
            trial_sha256s: vec![],
            deterministic_status: DeterministicStatus::Unassessed,
            job_sufficiency: JobSufficiency::Unassessed,
            preflight_assertions: preflight,
            behavioral_assertions: (1..=9)
                .map(|number| ConformanceAssertionEvaluation {
                    id: format!("B{number}"),
                    status: AssertionEvaluationStatus::NotApplicable,
                    passed_trials: 0,
                    required_trials: crate::conformance::REQUIRED_COLD_TRIALS as u8,
                    reason_codes: vec!["behavioral-trials-not-run".into()],
                })
                .collect(),
            trials: vec![],
            behavioral_qualification: BehavioralQualification::Unassessed,
            overall_result: QualificationVerdict::Unassessed,
            drafting_authority_granted: false,
            reason_codes: vec!["behavioral-trials-not-run".into()],
        };
        behavioral
            .validate()
            .expect("behavioral evaluation should validate");
        let behavioral_path = stage.join("behavioral.json");
        fs::write(
            &behavioral_path,
            serde_json::to_vec(&behavioral).expect("behavioral JSON"),
        )
        .expect("behavioral result should stage");

        let assembled = assemble_conformance(AssembleConformancePaths {
            candidate: Path::new("candidate.json"),
            deterministic: Path::new("deterministic.json"),
            behavioral: Path::new("behavioral.json"),
            trials: &[],
            artifact_root: &stage,
        })
        .expect("complete composite should assemble");
        let composite: JobConformanceV1 = serde_json::from_value(assembled.clone())
            .expect("assembled composite should deserialize");
        composite
            .validate()
            .expect("assembled composite should validate");
        let mut duplicate_authority = composite.clone();
        let mut duplicate_prompt = duplicate_authority
            .journey
            .artifacts
            .iter()
            .find(|artifact| artifact.role == JourneyArtifactRole::Prompt)
            .expect("prompt journey member")
            .clone();
        duplicate_prompt.artifact_id = "duplicate-prompt-authority".into();
        duplicate_authority.journey.artifacts.push(duplicate_prompt);
        assert!(duplicate_authority.validate().is_ok());
        assert!(validate_composite_members(&duplicate_authority, &stage).is_err());
        jsonschema::draft202012::validate(&schema(SchemaTarget::JobConformanceV1), &assembled)
            .expect("assembled composite should match its public schema");
        let mut missing_role = composite.clone();
        missing_role
            .journey
            .artifacts
            .retain(|artifact| artifact.role != JourneyArtifactRole::Prompt);
        assert!(missing_role.validate().is_err());
        let mut missing_link = composite.clone();
        missing_link.journey.links.retain(|link| {
            !(link.from_artifact_id == "deterministic-evaluation"
                && link.to_artifact_id == "behavioral-evaluation")
        });
        assert!(missing_link.validate().is_err());
        let mut cyclic = composite.clone();
        cyclic.journey.links.push(JourneyLink {
            from_artifact_id: "behavioral-evaluation".into(),
            to_artifact_id: "deterministic-evaluation".into(),
            relation: JourneyRelation::Evaluates,
        });
        assert!(cyclic.validate().is_err());
        let composite_path = stage.join("job-conformance.json");
        fs::write(
            &composite_path,
            serde_json::to_vec(&assembled).expect("composite JSON"),
        )
        .expect("composite should stage");

        for public in [false, true] {
            let report = project_conformance_report(
                Path::new("job-conformance.json"),
                &stage,
                "2026-08-13T12:00:00Z",
                public,
            )
            .expect("report projection should validate source authority");
            assert_eq!(
                report["contract"],
                if public {
                    PUBLIC_CONFORMANCE_REPORT_V1
                } else {
                    crate::conformance::CONFORMANCE_REPORT_V1
                }
            );
            jsonschema::draft202012::validate(
                &schema(if public {
                    SchemaTarget::PublicConformanceReportV1
                } else {
                    SchemaTarget::ConformanceReportV1
                }),
                &report,
            )
            .expect("report should match its schema");
        }

        let trace = crate::commands::decision_trace::project_conformance_file(
            Path::new("job-conformance.json"),
            &stage,
        )
        .expect("validated composite should trace");
        let json_once = serde_json::to_string(&trace).expect("trace JSON");
        let json_twice = serde_json::to_string(&trace).expect("trace JSON replay");
        assert_eq!(json_once, json_twice);
        let mermaid_once = crate::commands::decision_trace::render_mermaid(&trace);
        let mermaid_twice = crate::commands::decision_trace::render_mermaid(&trace);
        assert_eq!(mermaid_once, mermaid_twice);
        assert!(
            trace
                .observed_path
                .nodes
                .iter()
                .all(|node| { mermaid_once.contains(&format!("o_{}", node.id.replace('-', "_"))) })
        );

        for (field, value) in [
            ("job_id", json!("other-job")),
            ("fixture_id", json!("other-fixture")),
            (
                "pack_release",
                json!({
                    "pack_id": candidate_contract.pack_release.pack_id,
                    "release_id": "other-release",
                    "version": candidate_contract.pack_release.version,
                    "portable_digest": candidate_contract.pack_release.portable_digest,
                    "source_revision": candidate_contract.pack_release.source_revision
                }),
            ),
        ] {
            let mut mismatch = result.clone();
            mismatch[field] = value;
            fs::write(
                stage.join("deterministic-mismatch.json"),
                serde_json::to_vec(&mismatch).unwrap(),
            )
            .unwrap();
            assert!(
                assemble_conformance(AssembleConformancePaths {
                    candidate: Path::new("candidate.json"),
                    deterministic: Path::new("deterministic-mismatch.json"),
                    behavioral: Path::new("behavioral.json"),
                    trials: &[],
                    artifact_root: &stage,
                })
                .is_err()
            );
        }

        let mut forged_assertion = result.clone();
        forged_assertion["assertions"][0]["name"] = json!("caller-forged-release-integrity");
        fs::write(
            stage.join("deterministic-forged.json"),
            serde_json::to_vec(&forged_assertion).unwrap(),
        )
        .unwrap();
        assert!(
            assemble_conformance(AssembleConformancePaths {
                candidate: Path::new("candidate.json"),
                deterministic: Path::new("deterministic-forged.json"),
                behavioral: Path::new("behavioral.json"),
                trials: &[],
                artifact_root: &stage,
            })
            .is_err()
        );

        let original_deterministic = fs::read(&deterministic_path).unwrap();
        fs::write(&deterministic_path, b"{}").unwrap();
        assert!(
            project_conformance_report(
                Path::new("job-conformance.json"),
                &stage,
                "2026-08-13T12:00:00Z",
                false,
            )
            .is_err()
        );
        fs::write(&deterministic_path, original_deterministic).unwrap();

        let mut private_composite = composite.clone();
        for artifact in &mut private_composite.journey.artifacts {
            artifact.access_class = AccessClass::Private;
            artifact.publication_approval_sha256 = None;
        }
        private_composite.journey.artifacts.push(JourneyArtifact {
            artifact_id: "private-external-evidence".into(),
            phase: JourneyPhase::Review,
            role: JourneyArtifactRole::SourceLineage,
            contract: "external.private-evidence.v1".into(),
            relative_path: None,
            opaque_artifact_id: Some("secret-external-id".into()),
            authority_sha256: "8".repeat(64),
            byte_count: None,
            access_class: AccessClass::Private,
            publication_approval_sha256: None,
        });
        private_composite.journey.links.push(JourneyLink {
            from_artifact_id: "candidate".into(),
            to_artifact_id: "private-external-evidence".into(),
            relation: JourneyRelation::Declares,
        });
        private_composite
            .validate()
            .expect("private projection source should remain valid");
        fs::write(
            stage.join("job-private.json"),
            serde_json::to_vec(&private_composite).unwrap(),
        )
        .unwrap();
        assert!(
            project_conformance_report(
                Path::new("job-private.json"),
                &stage,
                "2026-08-13T12:00:00Z",
                true,
            )
            .is_err()
        );
        let _ = fs::remove_dir_all(stage);
    }
}
