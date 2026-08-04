use super::{MAX_TRACE_NODES, project_source_file, project_source_value, render_mermaid};
use serde_json::json;

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
fn successful_run_with_no_draft_decision_remains_blocked() {
    let source = json!({
        "contract": "mdp.run-execution.v1",
        "valid": true,
        "terminal_state": "success",
        "authority_block": {
            "decision": {"decision": "no-draft"},
            "reason_codes": ["insufficient-context"]
        }
    });
    let trace = project_source_value(&source, "e".repeat(64));
    assert_eq!(trace.status, "blocked");
    assert!(!trace.authority.output_authority);
    assert!(
        serde_json::to_string(&trace)
            .unwrap()
            .contains("insufficient-context")
    );
}
