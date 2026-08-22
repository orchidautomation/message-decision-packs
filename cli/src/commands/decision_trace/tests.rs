use super::{
    MAX_TRACE_NODES, TraceBuilder, TraceSource, add_driver_trace,
    project_prompt_output_validation_file, project_source_file, project_source_value,
    read_trace_runner_audit, render_mermaid,
};
use crate::artifact_hash::{canonical_json_sha256, pack_content_sha256};
use crate::cli::SchemaTarget;
use crate::commands::prompt_output::validate_prompt_output_file;
use crate::commands::schemas::schema;
use crate::run_contracts::{
    ArtifactAuthority, AssuranceEvidenceState, EvidenceProvenance, RunnerAuditV1, TerminalState,
};
use serde_json::json;

fn assert_unavailable_without_authority(trace: &super::DecisionTrace) {
    assert_eq!(trace.status, "unavailable");
    assert_eq!(trace.authority.decision_authority, "none");
    assert!(!trace.authority.output_authority);
}

#[test]
fn no_draft_fit_exposes_only_bounded_reasons_and_no_output_authority() {
    let source = json!({
        "contract": "mdp.fit.v0",
        "status": "insufficient-context",
        "context": {
            "missing_requirements": [{"field": "title", "reason": "private prose"}],
            "invalid_requirements": []
        },
        "matches": [],
        "disqualifiers": [],
        "decision": "private decision prose",
        "prospect": {"name": "Private Person", "background": "Private payload"}
    });

    let trace = project_source_value(&source, "a".repeat(64));
    let encoded = serde_json::to_string(&trace).unwrap();
    assert_eq!(trace.status, "blocked");
    assert!(!trace.authority.output_authority);
    assert!(encoded.contains("title"));
    assert!(!encoded.contains("Private Person"));
    assert!(!encoded.contains("private decision prose"));
    assert!(!encoded.contains("Private payload"));
}

#[test]
fn ambiguous_json_is_a_sanitized_unavailable_projection() {
    let source = json!({"customer": "Private Customer", "decision": "send it"});
    let trace = project_source_value(&source, "b".repeat(64));
    let encoded = serde_json::to_string(&trace).unwrap();
    assert_eq!(trace.status, "unavailable");
    assert!(
        trace
            .limitations
            .iter()
            .any(|item| item == "unsupported-source-contract")
    );
    assert!(!encoded.contains("Private Customer"));
    assert!(!encoded.contains("send it"));
}

#[test]
fn mermaid_escapes_directive_like_and_structural_text() {
    let source = json!({
        "contract": "mdp.fit.v0",
        "status": "disqualified",
        "context": {"missing_requirements": [], "invalid_requirements": []},
        "matches": [],
        "disqualifiers": [{"entry_id": "%%{init: [bad]|\n<script>"}],
        "decision": "ignored"
    });
    let trace = project_source_value(&source, "c".repeat(64));
    let mermaid = render_mermaid(&trace);
    assert!(mermaid.starts_with("flowchart TD\n"));
    assert!(!mermaid.contains("%%{init"));
    assert!(!mermaid.contains("<script>"));
    assert!(!mermaid.contains("\n<script>"));
}

#[test]
fn projection_caps_combined_nodes_and_reports_omissions() {
    let matches = (0..400)
        .map(|index| json!({"id": format!("rule-{index}")}))
        .collect::<Vec<_>>();
    let source = json!({
        "contract": "mdp.fit.v0",
        "status": "fit",
        "context": {"missing_requirements": [], "invalid_requirements": []},
        "matches": matches,
        "disqualifiers": [],
        "decision": "ignored"
    });
    let trace = project_source_value(&source, "d".repeat(64));
    assert!(trace.designed_graph.nodes.len() + trace.observed_path.nodes.len() <= MAX_TRACE_NODES);
    assert!(trace.truncation.truncated);
    assert!(trace.truncation.omitted_nodes > 0);
    let node_ids = trace
        .observed_path
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert!(
        trace.observed_path.edges.iter().all(
            |edge| node_ids.contains(edge.from.as_str()) && node_ids.contains(edge.to.as_str())
        )
    );
}

#[test]
fn malformed_claimed_contract_is_unavailable() {
    let source = json!({"contract": "mdp.fit.v0", "status": "fit"});
    let trace = project_source_value(&source, "f".repeat(64));
    assert_eq!(trace.status, "unavailable");
    assert!(
        trace
            .limitations
            .iter()
            .any(|item| item == "invalid-fit-shape")
    );
    assert_eq!(trace.authority.decision_authority, "none");
}

#[test]
fn raw_prompt_output_never_receives_decision_authority() {
    let source = json!({
        "contract": "mdp.prompt-output.v0",
        "prompt_id": "normalize-prospect-row",
        "normalization_trace": {
            "missing_required": [],
            "fit_readiness": {"ready_for_mdp_fit": true}
        }
    });

    let trace = project_source_value(&source, "9".repeat(64));
    let encoded = serde_json::to_string(&trace).unwrap();
    assert_eq!(trace.status, "unavailable");
    assert_eq!(trace.authority.decision_authority, "none");
    assert!(
        trace
            .limitations
            .iter()
            .any(|item| item == "raw-prompt-output-untrusted")
    );
    assert!(!encoded.contains("Validated prompt output"));
    assert!(!encoded.contains("Ready for MDP fit"));
}

#[test]
fn invalid_and_unbound_validation_results_have_stable_diagnostics() {
    let invalid = json!({
        "contract": "mdp.prompt-output-validation.v1",
        "valid": false
    });
    let invalid_trace = project_source_value(&invalid, "7".repeat(64));
    assert_unavailable_without_authority(&invalid_trace);
    assert!(
        invalid_trace
            .limitations
            .iter()
            .any(|item| item == "prompt-output-validation-invalid")
    );

    let unbound = json!({
        "contract": "mdp.prompt-output-validation.v1",
        "valid": true
    });
    let unbound_trace = project_source_value(&unbound, "8".repeat(64));
    assert_unavailable_without_authority(&unbound_trace);
    assert!(
        unbound_trace
            .limitations
            .iter()
            .any(|item| item == "prompt-output-validation-unbound")
    );
}

fn prompt_output_trace_fixture() -> (std::path::PathBuf, std::path::PathBuf, serde_json::Value) {
    let root = std::env::temp_dir().join(format!(
        "mdp-prompt-output-trace-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let pack_root = root.join("pack");
    crate::commands::init::init_pack(&pack_root, "Trace Pack", "gtm", true, false).unwrap();
    let prompt: serde_json::Value = serde_yaml::from_slice(
        &std::fs::read(pack_root.join(".mdp/prompts/normalize-prospect.yaml")).unwrap(),
    )
    .unwrap();
    let output = prompt["output_contract"]["example"].clone();
    let output_path = root.join("prompt-output.json");
    std::fs::write(&output_path, serde_json::to_vec_pretty(&output).unwrap()).unwrap();
    let validation = validate_prompt_output_file(
        &pack_root,
        &output_path,
        None,
        Some("normalize-prospect-row"),
    )
    .unwrap();
    assert_eq!(validation["valid"], true);
    (pack_root, output_path, validation)
}

fn write_validation_fixture(
    output_path: &std::path::Path,
    validation: &serde_json::Value,
) -> std::path::PathBuf {
    let validation_path = output_path.with_file_name("validation.json");
    let wrapper = json!({
        "ok": true,
        "command": "validate-prompt-output",
        "data": validation
    });
    std::fs::write(
        &validation_path,
        serde_json::to_vec_pretty(&wrapper).unwrap(),
    )
    .unwrap();
    validation_path
}

fn refresh_validation_binding(validation: &mut serde_json::Value) {
    validation["authority"]
        .as_object_mut()
        .unwrap()
        .remove("binding_sha256");
    let binding = canonical_json_sha256(&validation["authority"]).unwrap();
    validation["authority"]["binding_sha256"] = json!(binding);
}

#[test]
fn bound_valid_prompt_output_receipt_is_available() {
    let (pack_root, output_path, validation) = prompt_output_trace_fixture();
    jsonschema::draft202012::validate(&schema(SchemaTarget::PromptOutputValidationV1), &validation)
        .expect("validation receipt should satisfy its public schema");
    let validation_path = write_validation_fixture(&output_path, &validation);

    let trace =
        project_prompt_output_validation_file(&validation_path, &pack_root, &output_path, &[])
            .unwrap();
    assert_eq!(trace.status, "available");
    assert_eq!(trace.authority.decision_authority, "validation-receipt");
    assert_eq!(trace.authority.verification_state, "verified");
    assert!(trace.authority.output_authority);
    assert!(
        trace
            .observed_path
            .nodes
            .iter()
            .any(|node| node.label == "Exact validated prompt output")
    );

    let _ = std::fs::remove_dir_all(output_path.parent().unwrap());
}

#[test]
fn bound_blocked_prompt_output_receipt_remains_blocked() {
    let (pack_root, output_path, mut validation) = prompt_output_trace_fixture();
    validation["authority"]["decision_state"] = json!("blocked");
    refresh_validation_binding(&mut validation);
    let validation_path = write_validation_fixture(&output_path, &validation);

    let trace =
        project_prompt_output_validation_file(&validation_path, &pack_root, &output_path, &[])
            .unwrap();
    assert_eq!(trace.status, "blocked");
    assert_eq!(trace.authority.decision_authority, "validation-receipt");
    assert!(!trace.authority.output_authority);
    assert!(
        trace
            .observed_path
            .nodes
            .iter()
            .any(|node| node.state == "blocked")
    );

    let _ = std::fs::remove_dir_all(output_path.parent().unwrap());
}

#[test]
fn omitted_required_validation_input_is_unbound() {
    let (pack_root, output_path, mut validation) = prompt_output_trace_fixture();
    validation["artifacts"]["source_audit"] = json!({
        "path": "source-audit.json",
        "sha256": "a".repeat(64)
    });
    validation["authority"]["input_artifacts"] = json!([{
        "logical_name": "source_audit",
        "sha256": "a".repeat(64)
    }]);
    refresh_validation_binding(&mut validation);
    let validation_path = write_validation_fixture(&output_path, &validation);

    let trace =
        project_prompt_output_validation_file(&validation_path, &pack_root, &output_path, &[])
            .unwrap();
    assert_unavailable_without_authority(&trace);
    assert!(
        trace
            .limitations
            .iter()
            .any(|item| item == "prompt-output-validation-unbound")
    );

    let _ = std::fs::remove_dir_all(output_path.parent().unwrap());
}

#[test]
fn changed_prompt_output_bytes_invalidate_validation_authority() {
    let (pack_root, output_path, validation) = prompt_output_trace_fixture();
    let validation_path = write_validation_fixture(&output_path, &validation);
    let mut output: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&output_path).unwrap()).unwrap();
    output["normalized_prospect"]["company"] = json!("Tampered Example");
    std::fs::write(&output_path, serde_json::to_vec_pretty(&output).unwrap()).unwrap();

    let trace =
        project_prompt_output_validation_file(&validation_path, &pack_root, &output_path, &[])
            .unwrap();
    assert_unavailable_without_authority(&trace);
    assert!(
        trace
            .limitations
            .iter()
            .any(|item| item == "prompt-output-tampered")
    );

    let _ = std::fs::remove_dir_all(output_path.parent().unwrap());
}

#[test]
fn wrong_pack_or_input_bytes_fail_closed() {
    let (pack_root, output_path, mut validation) = prompt_output_trace_fixture();
    validation["authority"]["pack"]["sha256"] = json!("f".repeat(64));
    refresh_validation_binding(&mut validation);
    let validation_path = write_validation_fixture(&output_path, &validation);
    let trace =
        project_prompt_output_validation_file(&validation_path, &pack_root, &output_path, &[])
            .unwrap();
    assert_unavailable_without_authority(&trace);
    assert!(
        trace
            .limitations
            .iter()
            .any(|item| item == "prompt-output-validation-mismatch")
    );

    validation["authority"]["pack"]["sha256"] = json!(pack_content_sha256(&pack_root).unwrap());
    validation["artifacts"]["source_audit"] = json!({
        "path": "source-audit.json",
        "sha256": "a".repeat(64)
    });
    validation["authority"]["input_artifacts"] = json!([{
        "logical_name": "source_audit",
        "sha256": "a".repeat(64)
    }]);
    refresh_validation_binding(&mut validation);
    let validation_path = write_validation_fixture(&output_path, &validation);
    let input_path = output_path.with_file_name("wrong-input.json");
    std::fs::write(&input_path, b"{}\n").unwrap();
    let trace = project_prompt_output_validation_file(
        &validation_path,
        &pack_root,
        &output_path,
        &[format!("source_audit={}", input_path.display())],
    )
    .unwrap();
    assert_unavailable_without_authority(&trace);
    assert!(
        trace
            .limitations
            .iter()
            .any(|item| item == "prompt-output-validation-mismatch")
    );

    let _ = std::fs::remove_dir_all(output_path.parent().unwrap());
}

#[test]
fn changed_validation_binding_or_prompt_job_identity_fail_closed() {
    let (pack_root, output_path, mut validation) = prompt_output_trace_fixture();
    validation["authority"]["decision_state"] = json!("blocked");
    let validation_path = write_validation_fixture(&output_path, &validation);
    let trace =
        project_prompt_output_validation_file(&validation_path, &pack_root, &output_path, &[])
            .unwrap();
    assert_unavailable_without_authority(&trace);
    assert!(
        trace
            .limitations
            .iter()
            .any(|item| item == "prompt-output-validation-receipt-tampered")
    );

    validation["authority"]["decision_state"] = json!("available");
    validation["authority"]["prompt"]["id"] = json!("wrong-prompt");
    validation["authority"]["job_id"] = json!("wrong-job");
    refresh_validation_binding(&mut validation);
    let validation_path = write_validation_fixture(&output_path, &validation);
    let trace =
        project_prompt_output_validation_file(&validation_path, &pack_root, &output_path, &[])
            .unwrap();
    assert_unavailable_without_authority(&trace);
    assert!(
        trace
            .limitations
            .iter()
            .any(|item| item == "prompt-output-validation-mismatch")
    );

    let _ = std::fs::remove_dir_all(output_path.parent().unwrap());
}

#[test]
fn oversized_file_is_refused_without_echoing_its_path() {
    let path = std::env::temp_dir().join("mdp-private-customer-oversized-trace.json");
    std::fs::write(&path, vec![b'x'; super::MAX_TRACE_SOURCE_BYTES + 1]).unwrap();
    let trace = project_source_file(&path).unwrap();
    let encoded = serde_json::to_string(&trace).unwrap();
    assert_eq!(trace.status, "unavailable");
    assert!(
        trace
            .limitations
            .iter()
            .any(|item| item == "source-oversized")
    );
    assert!(!encoded.contains("mdp-private-customer"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn non_regular_trace_source_is_unreadable() {
    let path =
        std::env::temp_dir().join(format!("mdp-trace-source-directory-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    let trace = project_source_file(&path).unwrap();
    assert_eq!(trace.status, "unavailable");
    assert!(
        trace
            .limitations
            .iter()
            .any(|item| item == "source-unreadable")
    );
    let _ = std::fs::remove_dir(path);
}

#[cfg(unix)]
#[test]
fn symlink_trace_source_is_unreadable() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!("mdp-trace-symlink-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let target = root.join("target.json");
    let link = root.join("link.json");
    std::fs::write(&target, b"{}\n").unwrap();
    symlink(&target, &link).unwrap();

    let trace = project_source_file(&link).unwrap();
    assert_eq!(trace.status, "unavailable");
    assert!(
        trace
            .limitations
            .iter()
            .any(|item| item == "source-unreadable")
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn successful_run_with_no_draft_decision_remains_blocked() {
    let source = json!({
        "contract": "mdp.run-execution.v1",
        "valid": false,
        "terminal_state": "success",
        "authority": {
            "authority_level": "authoritative",
            "disposition": "block",
            "terminal": "no-draft"
        },
        "authority_block": {
            "decision": {"decision": "no-draft"},
            "reason_codes": ["insufficient-context"]
        }
    });
    let trace = project_source_value(&source, "e".repeat(64));
    assert_eq!(trace.status, "blocked");
    assert_eq!(trace.authority.decision_authority, "source-artifact");
    assert!(!trace.authority.output_authority);
    assert!(
        serde_json::to_string(&trace)
            .unwrap()
            .contains("insufficient-context")
    );
}

#[test]
fn raw_run_decision_cannot_self_certify_trace_authority() {
    let source = json!({
        "contract": "mdp.run-execution.v1",
        "valid": true,
        "terminal_state": "success",
        "authority_block": {"decision": {"decision": "ready"}, "verification": {}}
    });
    let trace = project_source_value(&source, "f".repeat(64));
    assert_eq!(trace.status, "unavailable");
    assert_eq!(trace.authority.decision_authority, "none");
    assert!(!trace.authority.output_authority);
    assert_eq!(trace.authority.verification_state, "not-verified");
}

#[test]
fn generative_trace_exposes_only_bound_driver_hashes() {
    let mut builder = TraceBuilder::new(TraceSource {
        contract: "mdp.run-receipt.v1".into(),
        command: None,
        sha256: "c".repeat(64),
        class: "receipt-backed-run",
    });
    builder.add_observed("run-bundle", "source", "Immutable run bundle", "observed");
    builder.add_observed("run-receipt", "authority", "Run receipt", "verified");
    let audit = RunnerAuditV1 {
        contract: "mdp.runner-audit.v1".into(),
        execution_id: "exec-1".into(),
        runner_version: "test".into(),
        runner_build_sha256: None,
        platform: "test".into(),
        snapshot_sha256: "c".repeat(64),
        driver_request_sha256: Some("a".repeat(64)),
        driver_result_sha256: Some("b".repeat(64)),
        provider_request_body_sha256: Some("d".repeat(64)),
        provider_request_schema_id: Some("private-provider-schema".into()),
        provider_response_body_sha256: Some("e".repeat(64)),
        provider_observation: None,
        identity_observations: None,
        diagnostic_code: None,
        terminal_state: TerminalState::Success,
        assurance: vec![crate::run_contracts::AssuranceDimension {
            dimension: "stateless-inference".into(),
            state: AssuranceEvidenceState::Declared,
            provenance: EvidenceProvenance::DriverAttested,
            evidence_refs: vec![],
            limitations: vec![],
        }],
        limitations: vec!["private diagnostic prose".into()],
    };

    add_driver_trace(&mut builder, &audit);
    let trace = builder.finish("available");
    let encoded = serde_json::to_string(&trace).unwrap();
    assert!(encoded.contains("mdp.driver-request.v2"));
    assert!(encoded.contains(&"a".repeat(64)));
    assert!(encoded.contains("mdp.driver-result.v2"));
    assert!(encoded.contains(&"b".repeat(64)));
    assert!(!encoded.contains(&"d".repeat(64)));
    assert!(!encoded.contains(&"e".repeat(64)));
    assert!(!encoded.contains("private-provider-schema"));
    assert!(!encoded.contains("private diagnostic prose"));
}

#[test]
fn trace_runner_audit_requires_exact_contained_authority_bytes() {
    let root = std::env::temp_dir().join(format!(
        "mdp-trace-audit-authority-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let audit = RunnerAuditV1 {
        contract: "mdp.runner-audit.v1".into(),
        execution_id: "exec-contained".into(),
        runner_version: "test".into(),
        runner_build_sha256: None,
        platform: "test".into(),
        snapshot_sha256: "c".repeat(64),
        driver_request_sha256: Some("a".repeat(64)),
        driver_result_sha256: Some("b".repeat(64)),
        provider_request_body_sha256: None,
        provider_request_schema_id: None,
        provider_response_body_sha256: None,
        provider_observation: None,
        identity_observations: None,
        diagnostic_code: None,
        terminal_state: TerminalState::Success,
        assurance: vec![],
        limitations: vec![],
    };
    let bytes = serde_json::to_vec(&audit).unwrap();
    std::fs::write(root.join("runner-audit.json"), &bytes).unwrap();
    let mut authority = ArtifactAuthority {
        logical_name: "runner-audit.json".into(),
        schema_id: "mdp.runner-audit.v1".into(),
        media_type: "application/json".into(),
        byte_count: bytes.len() as u64,
        sha256: crate::artifact_hash::sha256_hex(&bytes),
        provenance: EvidenceProvenance::MdpObserved,
        provenance_refs: vec![],
    };

    assert!(read_trace_runner_audit(&root, &authority).is_some());
    authority.byte_count += 1;
    assert!(read_trace_runner_audit(&root, &authority).is_none());
    authority.byte_count -= 1;
    authority.sha256 = "f".repeat(64);
    assert!(read_trace_runner_audit(&root, &authority).is_none());

    let _ = std::fs::remove_dir_all(root);
}
