use crate::artifact_hash::{
    AuthorityJsonLimits, canonical_json_sha256_for_domain, parse_authority_json, sha256_hex,
};
use crate::constants::RUN_RECEIPT_CONTRACT;
use crate::run_contracts::{
    ArtifactAuthority, AssuranceEvidenceState, EvidenceProvenance, RUN_BUNDLE_V1, RUN_RECEIPT_V1,
    RUN_VERIFICATION_V1, RUNNER_AUDIT_V1, RunBundleV1, RunMode, RunReceiptV1, RunVerificationV1,
    RunnerAuditV1,
};
use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::{Component, Path};

pub(crate) fn verify_run_files(
    bundle_path: Option<&Path>,
    receipt_path: &Path,
    artifact_root: Option<&Path>,
) -> Result<Value> {
    let receipt_bytes = fs::read(receipt_path)
        .with_context(|| format!("reading run receipt {}", receipt_path.display()))?;
    let receipt_value: Value =
        parse_authority_json(&receipt_bytes, AuthorityJsonLimits::default())?;
    if receipt_value["contract"].as_str() == Some(RUN_RECEIPT_CONTRACT) {
        return verify_legacy_v0_receipt(&receipt_value, artifact_root);
    }
    let bundle_path = bundle_path.context("--bundle is required for mdp.run-receipt.v1")?;
    let bundle_bytes = fs::read(bundle_path)
        .with_context(|| format!("reading run bundle {}", bundle_path.display()))?;
    let bundle: RunBundleV1 = parse_authority_json(&bundle_bytes, AuthorityJsonLimits::default())?;
    let receipt: RunReceiptV1 = serde_json::from_value(receipt_value)?;
    let verification = verify_run(&bundle, &receipt, artifact_root)?;
    Ok(serde_json::to_value(verification)?)
}

fn verify_legacy_v0_receipt(receipt: &Value, artifact_root: Option<&Path>) -> Result<Value> {
    let legacy_hash = canonical_json_sha256_for_domain(RUN_RECEIPT_CONTRACT, receipt)?;
    let mut issues = vec!["legacy-v0-cannot-upgrade-to-v1-assurance".to_string()];
    let mut artifact_state = AssuranceEvidenceState::Unknown;
    if let Some(root) = artifact_root {
        let mut checked = 0usize;
        for record in receipt["artifacts"].as_array().into_iter().flatten() {
            let Some(path) = record["path"].as_str() else {
                issues.push("legacy-artifact-path-missing".to_string());
                continue;
            };
            let logical = Path::new(path);
            if logical.is_absolute()
                || logical
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_)))
            {
                issues.push("legacy-artifact-path-not-portable".to_string());
                continue;
            }
            let bytes = match fs::read(root.join(logical)) {
                Ok(bytes) => bytes,
                Err(_) => {
                    issues.push("legacy-artifact-unreadable".to_string());
                    continue;
                }
            };
            checked += 1;
            if record["sha256"].as_str() != Some(sha256_hex(&bytes).as_str()) {
                issues.push("legacy-artifact-hash-mismatch".to_string());
            }
        }
        if checked > 0 && !issues.iter().any(|issue| issue.contains("artifact-")) {
            artifact_state = AssuranceEvidenceState::Verified;
        }
    }
    let assurance = vec![
        crate::run_contracts::AssuranceDimension {
            dimension: "legacy-artifact-integrity".into(),
            state: artifact_state,
            provenance: if artifact_state == AssuranceEvidenceState::Verified {
                EvidenceProvenance::VerifierRecomputed
            } else {
                EvidenceProvenance::Unknown
            },
            evidence_refs: vec![legacy_hash.clone()],
            limitations: vec!["v0 paths are machine-local and may not be portable".into()],
        },
        crate::run_contracts::AssuranceDimension {
            dimension: "declared-input-isolation".into(),
            state: AssuranceEvidenceState::Unknown,
            provenance: EvidenceProvenance::HostAttested,
            evidence_refs: vec![],
            limitations: vec![
                "v0 assertion-only isolation and audit-grade labels cannot become v1 verified evidence"
                    .into(),
            ],
        },
    ];
    Ok(serde_json::to_value(RunVerificationV1 {
        contract: RUN_VERIFICATION_V1.into(),
        valid: false,
        integrity_only: true,
        execution_id: format!("legacy-v0:{legacy_hash}"),
        terminal_state: crate::run_contracts::TerminalState::NoDraftAuditIncomplete,
        recomputed_assurance: assurance,
        issues,
    })?)
}

pub(crate) fn verify_run(
    bundle: &RunBundleV1,
    receipt: &RunReceiptV1,
    artifact_root: Option<&Path>,
) -> Result<RunVerificationV1> {
    let mut issues = Vec::new();
    if bundle.contract != RUN_BUNDLE_V1 {
        issues.push("unsupported-run-bundle-contract".to_string());
    }
    if receipt.contract != RUN_RECEIPT_V1 {
        issues.push("unsupported-run-receipt-contract".to_string());
    }
    if bundle.execution_id != receipt.execution_id {
        issues.push("execution-id-mismatch".to_string());
    }
    if bundle.profile != receipt.profile || bundle.operation != receipt.operation {
        issues.push("profile-operation-mismatch".to_string());
    }
    if bundle.job_identity != receipt.job_identity {
        issues.push("job-identity-mismatch".to_string());
    }
    if bundle.pack.profile_id != bundle.profile {
        issues.push("pack-profile-mismatch".to_string());
    }
    match bundle.mode {
        RunMode::Deterministic => {
            if bundle.prompt.is_some() || bundle.driver.is_some() || bundle.model.is_some() {
                issues.push("deterministic-inference-authority-present".to_string());
            }
        }
        RunMode::Generative => {
            if bundle.prompt.is_none() || bundle.driver.is_none() || bundle.model.is_none() {
                issues.push("generative-inference-authority-incomplete".to_string());
            }
        }
    }

    let bundle_value = serde_json::to_value(bundle)?;
    let bundle_hash = canonical_json_sha256_for_domain(RUN_BUNDLE_V1, &bundle_value)?;
    if bundle_hash != receipt.bundle_sha256 {
        issues.push("bundle-hash-mismatch".to_string());
    }

    let mut receipt_for_hash = receipt.clone();
    receipt_for_hash.receipt_sha256.clear();
    let receipt_value = serde_json::to_value(&receipt_for_hash)?;
    let receipt_hash = canonical_json_sha256_for_domain(RUN_RECEIPT_V1, &receipt_value)?;
    if receipt_hash != receipt.receipt_sha256 {
        issues.push("receipt-hash-mismatch".to_string());
    }
    if let Some(decision) = &receipt.decision {
        let mut decision_for_hash = decision.clone();
        decision_for_hash.sha256.clear();
        let decision_hash = canonical_json_sha256_for_domain(
            &decision.schema_id,
            &serde_json::to_value(decision_for_hash)?,
        )?;
        if decision_hash != decision.sha256 {
            issues.push("decision-hash-mismatch".to_string());
        }
    }

    if receipt.terminal_state.is_success() {
        if receipt.output.is_none()
            || receipt.decision.is_none()
            || receipt.compiled_context.is_none()
            || receipt.validation.is_none()
        {
            issues.push("success-artifacts-incomplete".to_string());
        }
    } else if receipt.output.is_some()
        || receipt.decision.is_some()
        || receipt.compiled_context.is_some()
    {
        issues.push("no-draft-authority-leak".to_string());
    }

    let mut dimensions = std::collections::HashSet::new();
    let expected_dimensions = [
        "declared-input-isolation",
        "declared-input-byte-binding",
        "source-mutation-resistance",
        "stateless-inference",
        "audit-evidence",
    ];
    for dimension in &receipt.assurance {
        if !dimensions.insert(dimension.dimension.as_str()) {
            issues.push(format!(
                "duplicate-assurance-dimension:{}",
                dimension.dimension
            ));
        }
        if !expected_dimensions.contains(&dimension.dimension.as_str()) {
            issues.push(format!(
                "unexpected-assurance-dimension:{}",
                dimension.dimension
            ));
        }
        if matches!(dimension.provenance, EvidenceProvenance::DriverAttested)
            && matches!(
                dimension.state,
                AssuranceEvidenceState::Enforced | AssuranceEvidenceState::Verified
            )
        {
            issues.push(format!(
                "driver-attestation-cannot-elevate:{}",
                dimension.dimension
            ));
        }
    }

    let recomputed_assurance = recompute_assurance(
        bundle.mode,
        receipt.terminal_state,
        &bundle_hash,
        &receipt.assurance,
    );
    if receipt.assurance != recomputed_assurance {
        issues.push("assurance-not-verifier-derived".to_string());
    }

    if let Some(root) = artifact_root {
        verify_artifact(root, &receipt.runner_audit, &mut issues);
        verify_runner_audit(
            root,
            bundle,
            receipt,
            &bundle_hash,
            &recomputed_assurance,
            &mut issues,
        );
        for artifact in [
            receipt.output.as_ref(),
            receipt.compiled_context.as_ref(),
            receipt.validation.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            verify_artifact(root, artifact, &mut issues);
        }
    }

    Ok(RunVerificationV1 {
        contract: RUN_VERIFICATION_V1.to_string(),
        valid: issues.is_empty(),
        integrity_only: true,
        execution_id: receipt.execution_id.clone(),
        terminal_state: receipt.terminal_state,
        recomputed_assurance,
        issues,
    })
}

fn recompute_assurance(
    mode: RunMode,
    terminal_state: crate::run_contracts::TerminalState,
    bundle_sha256: &str,
    claimed: &[crate::run_contracts::AssuranceDimension],
) -> Vec<crate::run_contracts::AssuranceDimension> {
    use crate::run_contracts::{AssuranceDimension, TerminalState};

    if mode == RunMode::Deterministic {
        return vec![
            AssuranceDimension {
                dimension: "declared-input-isolation".into(),
                state: AssuranceEvidenceState::Observed,
                provenance: EvidenceProvenance::MdpObserved,
                evidence_refs: vec![bundle_sha256.into()],
                limitations: vec![
                    "OS-level access outside the private staging tree is not attested".into(),
                ],
            },
            AssuranceDimension {
                dimension: "declared-input-byte-binding".into(),
                state: AssuranceEvidenceState::Verified,
                provenance: EvidenceProvenance::MdpObserved,
                evidence_refs: vec![bundle_sha256.into()],
                limitations: vec![
                    "exact source and staged bytes were re-read and matched during this local invocation"
                        .into(),
                ],
            },
            AssuranceDimension {
                dimension: "source-mutation-resistance".into(),
                state: if terminal_state == TerminalState::NoDraftAuditIncomplete {
                    AssuranceEvidenceState::Unknown
                } else {
                    AssuranceEvidenceState::Verified
                },
                provenance: EvidenceProvenance::VerifierRecomputed,
                evidence_refs: vec![bundle_sha256.into()],
                limitations: vec![],
            },
            AssuranceDimension {
                dimension: "stateless-inference".into(),
                state: AssuranceEvidenceState::NotApplicable,
                provenance: EvidenceProvenance::MdpObserved,
                evidence_refs: vec![],
                limitations: vec!["this operation performs no model inference".into()],
            },
            AssuranceDimension {
                dimension: "audit-evidence".into(),
                state: if terminal_state == TerminalState::NoDraftAuditIncomplete {
                    AssuranceEvidenceState::Unknown
                } else {
                    AssuranceEvidenceState::Observed
                },
                provenance: EvidenceProvenance::MdpObserved,
                evidence_refs: vec![bundle_sha256.into()],
                limitations: vec![
                    "receipt integrity is locally recomputable; host durability is not attested"
                        .into(),
                ],
            },
        ];
    }

    // Generative assurance is a closed verifier-owned contract. Never preserve
    // caller-added dimensions or caller-selected MDP/verifier provenance.
    let mutation_state = if terminal_state == TerminalState::NoDraftAuditIncomplete {
        AssuranceEvidenceState::Unknown
    } else {
        AssuranceEvidenceState::Verified
    };
    let _ = claimed;
    vec![
        AssuranceDimension {
            dimension: "declared-input-isolation".into(),
            state: AssuranceEvidenceState::Observed,
            provenance: EvidenceProvenance::MdpObserved,
            evidence_refs: vec![bundle_sha256.into()],
            limitations: vec![
                "OS-level access outside the private staging tree is not attested".into(),
            ],
        },
        AssuranceDimension {
            dimension: "declared-input-byte-binding".into(),
            state: AssuranceEvidenceState::Verified,
            provenance: EvidenceProvenance::MdpObserved,
            evidence_refs: vec![bundle_sha256.into()],
            limitations: vec![
                "exact source and staged bytes were re-read and matched during this local invocation"
                    .into(),
            ],
        },
        AssuranceDimension {
            dimension: "source-mutation-resistance".into(),
            state: mutation_state,
            provenance: EvidenceProvenance::VerifierRecomputed,
            evidence_refs: vec![bundle_sha256.into()],
            limitations: vec![],
        },
        AssuranceDimension {
            dimension: "stateless-inference".into(),
            state: AssuranceEvidenceState::Declared,
            provenance: EvidenceProvenance::DriverAttested,
            evidence_refs: vec![bundle_sha256.into()],
            limitations: vec![
                "store:false and fresh-request behavior are driver-declared; provider-side retention remains provider-controlled"
                    .into(),
            ],
        },
        AssuranceDimension {
            dimension: "audit-evidence".into(),
            state: if terminal_state == TerminalState::NoDraftAuditIncomplete {
                AssuranceEvidenceState::Unknown
            } else {
                AssuranceEvidenceState::Observed
            },
            provenance: EvidenceProvenance::MdpObserved,
            evidence_refs: vec![bundle_sha256.into()],
            limitations: vec![
                "receipt integrity is locally recomputable; host durability is not attested".into(),
            ],
        },
    ]
}

fn verify_runner_audit(
    root: &Path,
    bundle: &RunBundleV1,
    receipt: &RunReceiptV1,
    bundle_sha256: &str,
    recomputed_assurance: &[crate::run_contracts::AssuranceDimension],
    issues: &mut Vec<String>,
) {
    let path = root.join(&receipt.runner_audit.logical_name);
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };
    let audit: RunnerAuditV1 = match parse_authority_json(&bytes, AuthorityJsonLimits::default()) {
        Ok(audit) => audit,
        Err(_) => {
            issues.push("runner-audit-invalid".to_string());
            return;
        }
    };
    if audit.contract != RUNNER_AUDIT_V1 {
        issues.push("runner-audit-contract-mismatch".to_string());
    }
    if audit.execution_id != receipt.execution_id || audit.terminal_state != receipt.terminal_state
    {
        issues.push("runner-audit-authority-mismatch".to_string());
    }
    if audit.snapshot_sha256 != bundle_sha256 || audit.snapshot_sha256 != receipt.bundle_sha256 {
        issues.push("runner-audit-snapshot-mismatch".to_string());
    }
    match bundle.mode {
        RunMode::Deterministic => {
            if audit.provider_request_body_sha256.is_some()
                || audit.provider_request_schema_id.is_some()
                || audit.provider_response_body_sha256.is_some()
                || audit.provider_observation.is_some()
            {
                issues.push("deterministic-provider-request-evidence-present".to_string());
            }
            if audit.driver_request_sha256.is_some() || audit.driver_result_sha256.is_some() {
                issues.push("deterministic-driver-evidence-present".to_string());
            }
        }
        RunMode::Generative => {
            if !audit
                .driver_request_sha256
                .as_deref()
                .is_some_and(is_canonical_sha256)
                || !audit
                    .driver_result_sha256
                    .as_deref()
                    .is_some_and(is_canonical_sha256)
            {
                issues.push("generative-driver-evidence-missing".to_string());
            }
            if let Some(issue) = provider_request_evidence_issue(
                receipt.terminal_state.is_success(),
                audit.provider_request_body_sha256.as_deref(),
                audit.provider_request_schema_id.as_deref(),
            ) {
                issues.push(issue.to_string());
            }
            if audit
                .provider_request_body_sha256
                .as_deref()
                .is_some_and(|sha256| !is_canonical_sha256(sha256))
            {
                issues.push("generative-provider-request-hash-invalid".to_string());
            }
            if let Some(issue) = provider_response_evidence_issue(
                receipt.terminal_state.is_success(),
                audit.provider_response_body_sha256.as_deref(),
            ) {
                issues.push(issue.to_string());
            }
            if receipt.terminal_state.is_success() {
                let observation_valid = bundle.model.as_ref().is_some_and(|model| {
                    audit
                        .provider_observation
                        .as_ref()
                        .is_some_and(|observation| {
                            observation.provider == model.provider
                                && observation
                                    .resolved_model
                                    .as_deref()
                                    .is_some_and(|resolved| !resolved.trim().is_empty())
                        })
                });
                if !observation_valid {
                    issues.push("generative-provider-observation-missing".to_string());
                }
            }
        }
    }
    if audit.assurance != recomputed_assurance {
        issues.push("runner-audit-assurance-mismatch".to_string());
    }
    if audit.limitations != receipt.limitations {
        issues.push("runner-audit-limitations-mismatch".to_string());
    }
}

fn provider_request_evidence_issue(
    success: bool,
    request_sha256: Option<&str>,
    schema_id: Option<&str>,
) -> Option<&'static str> {
    let complete = request_sha256.is_some() && schema_id.is_some();
    let absent = request_sha256.is_none() && schema_id.is_none();
    (!(complete || absent) || (success && !complete))
        .then_some("generative-provider-request-evidence-missing")
}

fn provider_response_evidence_issue(
    success: bool,
    response_sha256: Option<&str>,
) -> Option<&'static str> {
    match response_sha256 {
        Some(sha256) if !is_canonical_sha256(sha256) => {
            Some("generative-provider-response-hash-invalid")
        }
        None if success => Some("generative-provider-response-evidence-missing"),
        _ => None,
    }
}

fn is_canonical_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn verify_artifact(root: &Path, authority: &ArtifactAuthority, issues: &mut Vec<String>) {
    let logical = Path::new(&authority.logical_name);
    if logical.as_os_str().is_empty()
        || !authority.logical_name.is_ascii()
        || logical.is_absolute()
        || logical
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        issues.push(format!(
            "invalid-artifact-logical-name:{}",
            authority.logical_name
        ));
        return;
    }
    let path = root.join(logical);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(_) => {
            issues.push(format!("artifact-missing:{}", authority.logical_name));
            return;
        }
    };
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        issues.push(format!("artifact-not-regular:{}", authority.logical_name));
        return;
    }
    match fs::read(&path) {
        Ok(bytes) => {
            if bytes.len() as u64 != authority.byte_count {
                issues.push(format!(
                    "artifact-byte-count-mismatch:{}",
                    authority.logical_name
                ));
            }
            if sha256_hex(&bytes) != authority.sha256 {
                issues.push(format!("artifact-hash-mismatch:{}", authority.logical_name));
            }
        }
        Err(_) => issues.push(format!("artifact-unreadable:{}", authority.logical_name)),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_canonical_sha256, provider_request_evidence_issue, provider_response_evidence_issue,
        recompute_assurance, verify_legacy_v0_receipt, verify_run,
    };
    use crate::artifact_hash::{canonical_json_sha256_for_domain, sha256_hex};
    use crate::run_contracts::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn verifier_rejects_mutation_and_false_driver_elevation() {
        let root = std::env::temp_dir().join(format!(
            "mdp-verify-run-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("output.json"), b"{}\n").unwrap();
        fs::write(root.join("context.json"), b"{}\n").unwrap();
        fs::write(root.join("validation.json"), b"{}\n").unwrap();
        let bundle = sample_bundle();
        let mut receipt = sample_receipt(&bundle, &root);
        write_audit(&root, &receipt);
        receipt.runner_audit = artifact(&root, "audit.json");
        seal_receipt(&mut receipt);
        assert!(verify_run(&bundle, &receipt, Some(&root)).unwrap().valid);

        fs::write(root.join("output.json"), b"tampered\n").unwrap();
        assert!(!verify_run(&bundle, &receipt, Some(&root)).unwrap().valid);

        fs::write(root.join("output.json"), b"{}\n").unwrap();
        receipt.assurance[0].state = AssuranceEvidenceState::Verified;
        receipt.assurance[0].provenance = EvidenceProvenance::MdpObserved;
        write_audit(&root, &receipt);
        receipt.runner_audit = artifact(&root, "audit.json");
        seal_receipt(&mut receipt);
        let result = verify_run(&bundle, &receipt, Some(&root)).unwrap();
        assert!(
            result
                .issues
                .iter()
                .any(|issue| issue == "assurance-not-verifier-derived")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn driver_hashes_use_the_closed_canonical_sha256_form() {
        assert!(is_canonical_sha256(&"a".repeat(64)));
        assert!(!is_canonical_sha256(&"A".repeat(64)));
        assert!(!is_canonical_sha256(&"a".repeat(63)));
        assert!(!is_canonical_sha256(&format!("{}g", "a".repeat(63))));
    }

    #[test]
    fn partial_provider_evidence_maps_to_one_stable_diagnostic() {
        assert_eq!(
            provider_request_evidence_issue(false, Some(&"a".repeat(64)), None),
            Some("generative-provider-request-evidence-missing")
        );
        assert_eq!(
            provider_request_evidence_issue(false, None, Some("schema.v1")),
            Some("generative-provider-request-evidence-missing")
        );
        assert_eq!(provider_request_evidence_issue(false, None, None), None);
    }

    #[test]
    fn provider_response_hash_maps_to_exactly_one_stable_diagnostic() {
        assert_eq!(
            provider_response_evidence_issue(true, None),
            Some("generative-provider-response-evidence-missing")
        );
        assert_eq!(
            provider_response_evidence_issue(true, Some("malformed")),
            Some("generative-provider-response-hash-invalid")
        );
        assert_eq!(
            provider_response_evidence_issue(true, Some(&"a".repeat(64))),
            None
        );
    }

    #[test]
    fn generative_assurance_is_reconstructed_from_a_closed_dimension_set() {
        let claimed = vec![
            AssuranceDimension {
                dimension: "declared-input-isolation".into(),
                state: AssuranceEvidenceState::Enforced,
                provenance: EvidenceProvenance::MdpObserved,
                evidence_refs: vec!["caller-selected".into()],
                limitations: vec![],
            },
            AssuranceDimension {
                dimension: "made-up-verifier-proof".into(),
                state: AssuranceEvidenceState::Verified,
                provenance: EvidenceProvenance::VerifierRecomputed,
                evidence_refs: vec!["caller-selected".into()],
                limitations: vec![],
            },
        ];

        let recomputed = recompute_assurance(
            RunMode::Generative,
            TerminalState::Success,
            &"a".repeat(64),
            &claimed,
        );
        assert_eq!(recomputed.len(), 5);
        assert_eq!(recomputed[0].dimension, "declared-input-isolation");
        assert_eq!(recomputed[0].state, AssuranceEvidenceState::Observed);
        assert!(
            !recomputed
                .iter()
                .any(|dimension| { dimension.dimension == "made-up-verifier-proof" })
        );
    }

    #[test]
    fn no_draft_receipt_cannot_publish_decision_authority() {
        let bundle = sample_bundle();
        let mut receipt = sample_receipt(&bundle, Path::new("."));
        receipt.terminal_state = TerminalState::NoDraftPolicyBlocked;
        receipt.output = None;
        receipt.compiled_context = None;
        receipt.validation = None;
        seal_receipt(&mut receipt);
        assert!(
            verify_run(&bundle, &receipt, None)
                .unwrap()
                .issues
                .contains(&"no-draft-authority-leak".to_string())
        );
    }

    #[test]
    fn verifier_binds_runner_audit_snapshot_and_limitations() {
        let root = std::env::temp_dir().join(format!(
            "mdp-verify-audit-binding-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        for name in ["output.json", "context.json", "validation.json"] {
            fs::write(root.join(name), b"{}\n").unwrap();
        }
        let bundle = sample_bundle();
        let mut receipt = sample_receipt(&bundle, &root);
        write_audit(&root, &receipt);
        let mut audit: RunnerAuditV1 =
            serde_json::from_slice(&fs::read(root.join("audit.json")).unwrap()).unwrap();
        audit.snapshot_sha256 = "f".repeat(64);
        audit.limitations.push("caller-added".into());
        fs::write(root.join("audit.json"), serde_json::to_vec(&audit).unwrap()).unwrap();
        receipt.runner_audit = artifact(&root, "audit.json");
        seal_receipt(&mut receipt);

        let result = verify_run(&bundle, &receipt, Some(&root)).unwrap();
        assert!(
            result
                .issues
                .contains(&"runner-audit-snapshot-mismatch".to_string())
        );
        assert!(
            result
                .issues
                .contains(&"runner-audit-limitations-mismatch".to_string())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deterministic_run_rejects_generative_driver_hashes() {
        let root = std::env::temp_dir().join(format!(
            "mdp-verify-deterministic-driver-evidence-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        for name in ["output.json", "context.json", "validation.json"] {
            fs::write(root.join(name), b"{}\n").unwrap();
        }
        let bundle = sample_bundle();
        let mut receipt = sample_receipt(&bundle, &root);
        write_audit(&root, &receipt);
        let mut audit: RunnerAuditV1 =
            serde_json::from_slice(&fs::read(root.join("audit.json")).unwrap()).unwrap();
        audit.driver_request_sha256 = Some("a".repeat(64));
        audit.driver_result_sha256 = Some("b".repeat(64));
        fs::write(root.join("audit.json"), serde_json::to_vec(&audit).unwrap()).unwrap();
        receipt.runner_audit = artifact(&root, "audit.json");
        seal_receipt(&mut receipt);

        let result = verify_run(&bundle, &receipt, Some(&root)).unwrap();
        assert!(
            result
                .issues
                .contains(&"deterministic-driver-evidence-present".to_string())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_audit_grade_label_never_upgrades_to_v1_assurance() {
        let legacy = serde_json::json!({
            "contract": "mdp.run-receipt.v0",
            "valid": true,
            "decision": "audit-grade",
            "artifacts": []
        });
        let result = verify_legacy_v0_receipt(&legacy, None).unwrap();
        assert_eq!(result["valid"], false);
        assert_eq!(result["terminal_state"], "no-draft:audit-incomplete");
        assert_eq!(result["recomputed_assurance"][1]["state"], "unknown");
        assert!(
            result["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| issue == "legacy-v0-cannot-upgrade-to-v1-assurance")
        );
    }

    use std::path::Path;

    fn sample_bundle() -> RunBundleV1 {
        RunBundleV1 {
            contract: RUN_BUNDLE_V1.into(),
            execution_id: "exec-1".into(),
            created_at: "2026-08-03T00:00:00Z".into(),
            profile: "gtm".into(),
            operation: "qualify".into(),
            mode: RunMode::Deterministic,
            job_identity: None,
            pack: PackAuthority {
                release_id: "release-1".into(),
                pack_id: "pack-1".into(),
                version: "1".into(),
                profile_id: "gtm".into(),
                portable_digest: "a".repeat(64),
                files: vec![],
            },
            prompt: None,
            inputs: vec![],
            execution_policy_sha256: "b".repeat(64),
            driver: None,
            model: None,
        }
    }

    fn sample_receipt(bundle: &RunBundleV1, root: &Path) -> RunReceiptV1 {
        let bundle_sha256 =
            canonical_json_sha256_for_domain(RUN_BUNDLE_V1, &serde_json::to_value(bundle).unwrap())
                .unwrap();
        RunReceiptV1 {
            contract: RUN_RECEIPT_V1.into(),
            execution_id: bundle.execution_id.clone(),
            created_at: bundle.created_at.clone(),
            profile: bundle.profile.clone(),
            operation: bundle.operation.clone(),
            job_identity: None,
            bundle_sha256: bundle_sha256.clone(),
            terminal_state: TerminalState::Success,
            output: Some(artifact_or_placeholder(root, "output.json")),
            decision: Some(sealed_decision()),
            compiled_context: Some(artifact_or_placeholder(root, "context.json")),
            validation: Some(artifact_or_placeholder(root, "validation.json")),
            runner_audit: artifact_or_placeholder(root, "audit.json"),
            assurance: recompute_assurance(
                bundle.mode,
                TerminalState::Success,
                &bundle_sha256,
                &[],
            ),
            limitations: vec![],
            receipt_sha256: String::new(),
        }
    }

    fn seal_receipt(receipt: &mut RunReceiptV1) {
        receipt.receipt_sha256.clear();
        receipt.receipt_sha256 = canonical_json_sha256_for_domain(
            RUN_RECEIPT_V1,
            &serde_json::to_value(&*receipt).unwrap(),
        )
        .unwrap();
    }

    fn sealed_decision() -> DecisionAuthority {
        let mut decision = DecisionAuthority {
            schema_id: "decision.v1".into(),
            decision: "ready".into(),
            reason_codes: vec!["ready".into()],
            sha256: String::new(),
        };
        decision.sha256 = canonical_json_sha256_for_domain(
            &decision.schema_id,
            &serde_json::to_value(&decision).unwrap(),
        )
        .unwrap();
        decision
    }

    fn artifact(root: &Path, name: &str) -> ArtifactAuthority {
        let bytes = fs::read(root.join(name)).unwrap();
        ArtifactAuthority {
            logical_name: name.into(),
            schema_id: if name == "audit.json" {
                RUNNER_AUDIT_V1.into()
            } else {
                "example.v1".into()
            },
            media_type: "application/json".into(),
            byte_count: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
            provenance: EvidenceProvenance::MdpObserved,
            provenance_refs: vec![],
        }
    }

    fn artifact_or_placeholder(root: &Path, name: &str) -> ArtifactAuthority {
        if root.join(name).exists() {
            artifact(root, name)
        } else {
            ArtifactAuthority {
                logical_name: name.into(),
                schema_id: if name == "audit.json" {
                    RUNNER_AUDIT_V1.into()
                } else {
                    "example.v1".into()
                },
                media_type: "application/json".into(),
                byte_count: 0,
                sha256: sha256_hex(b""),
                provenance: EvidenceProvenance::MdpObserved,
                provenance_refs: vec![],
            }
        }
    }

    fn write_audit(root: &Path, receipt: &RunReceiptV1) {
        let audit = RunnerAuditV1 {
            contract: RUNNER_AUDIT_V1.into(),
            execution_id: receipt.execution_id.clone(),
            runner_version: "0.1.56".into(),
            runner_build_sha256: None,
            platform: "test".into(),
            snapshot_sha256: receipt.bundle_sha256.clone(),
            driver_request_sha256: None,
            driver_result_sha256: None,
            provider_request_body_sha256: None,
            provider_request_schema_id: None,
            provider_response_body_sha256: None,
            provider_observation: None,
            terminal_state: receipt.terminal_state,
            assurance: receipt.assurance.clone(),
            limitations: vec![],
        };
        fs::write(root.join("audit.json"), serde_json::to_vec(&audit).unwrap()).unwrap();
    }
}
