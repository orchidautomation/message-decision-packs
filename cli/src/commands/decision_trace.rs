use crate::artifact_hash::{AuthorityJsonLimits, parse_authority_json, sha256_hex};
use crate::commands::run_verification::verify_run;
use crate::conformance::{
    AccessClass, JOB_CONFORMANCE_V1, JourneyRelation, canonical_authority_sha256,
    parse_job_conformance, read_contained_file,
};
use crate::run_contracts::{
    EvidenceProvenance, RUN_BUNDLE_V1, RUN_EXECUTION_V1, RUN_RECEIPT_V1, RunBundleV1, RunReceiptV1,
};
use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::Path;

mod schema;
pub(crate) use schema::decision_trace_schema;

mod render;
pub(crate) use render::render_mermaid;

#[cfg(test)]
mod tests;

pub(crate) const DECISION_TRACE_V1: &str = "mdp.decision-trace.v1";
pub(crate) const MAX_TRACE_SOURCE_BYTES: usize = 1_048_576;
pub(crate) const MAX_TRACE_NODES: usize = 256;
pub(crate) const MAX_TRACE_EDGES: usize = 512;
pub(crate) const MAX_TRACE_LABEL_BYTES: usize = 120;
pub(crate) const MAX_MERMAID_BYTES: usize = 262_144;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct DecisionTrace {
    pub(crate) contract: &'static str,
    pub(crate) status: &'static str,
    pub(crate) source: TraceSource,
    pub(crate) authority: TraceAuthority,
    pub(crate) designed_graph: TraceGraph,
    pub(crate) observed_path: TraceGraph,
    pub(crate) truncation: TraceTruncation,
    pub(crate) limitations: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TraceSource {
    pub(crate) contract: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) command: Option<String>,
    pub(crate) sha256: String,
    pub(crate) class: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TraceAuthority {
    pub(crate) projection_only: bool,
    pub(crate) decision_authority: &'static str,
    pub(crate) output_authority: bool,
    pub(crate) verification_state: &'static str,
    pub(crate) notice: &'static str,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct TraceGraph {
    pub(crate) nodes: Vec<TraceNode>,
    pub(crate) edges: Vec<TraceEdge>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TraceNode {
    pub(crate) id: String,
    pub(crate) kind: &'static str,
    pub(crate) label: String,
    pub(crate) state: &'static str,
    pub(crate) evidence_provenance: EvidenceProvenance,
    pub(crate) artifact_refs: Vec<TraceArtifactRef>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TraceEdge {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) kind: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TraceArtifactRef {
    pub(crate) schema_id: String,
    pub(crate) sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) logical_name: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct TraceTruncation {
    pub(crate) truncated: bool,
    pub(crate) omitted_nodes: usize,
    pub(crate) omitted_edges: usize,
    pub(crate) labels_truncated: usize,
}

pub(crate) fn project_source_file(path: &Path) -> Result<DecisionTrace> {
    if fs::metadata(path)
        .map(|metadata| metadata.len() > MAX_TRACE_SOURCE_BYTES as u64)
        .unwrap_or(false)
    {
        return Ok(unavailable("source-oversized", String::new()));
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(unavailable("source-unreadable", String::new())),
    };
    if bytes.len() > MAX_TRACE_SOURCE_BYTES {
        return Ok(unavailable("source-oversized", sha256_hex(&bytes)));
    }
    let hash = sha256_hex(&bytes);
    let value: Value = match parse_authority_json(&bytes, AuthorityJsonLimits::default()) {
        Ok(value) => value,
        Err(_) => return Ok(unavailable("source-malformed", hash)),
    };
    Ok(project_source_value(&value, hash))
}

pub(crate) fn project_conformance_file(
    relative_path: &Path,
    artifact_root: &Path,
) -> Result<DecisionTrace> {
    let bytes = read_contained_file(artifact_root, relative_path)?;
    let composite = parse_job_conformance(&bytes)?;
    crate::commands::conformance::validate_composite_members(&composite, artifact_root)?;
    let all_public = composite.journey.artifacts.iter().all(|artifact| {
        matches!(
            artifact.access_class,
            AccessClass::Synthetic | AccessClass::SanitizedPublic
        )
    });
    let source_hash = if all_public {
        canonical_authority_sha256(&composite)?
    } else {
        String::new()
    };
    let mut builder = TraceBuilder::new(TraceSource {
        contract: JOB_CONFORMANCE_V1.into(),
        command: Some("conformance-assemble".into()),
        sha256: source_hash,
        class: "composite-conformance",
    });
    builder.authority.decision_authority = "composite-conformance";
    builder.authority.verification_state = "verified";
    builder.authority.output_authority = false;
    builder.add_designed(
        "candidate",
        "source",
        "Frozen candidate authority",
        "designed",
    );
    builder.add_designed(
        "deterministic",
        "gate",
        "Deterministic sufficiency",
        "designed",
    );
    builder.add_designed("behavioral", "gate", "Behavioral qualification", "designed");
    builder.add_designed("verdict", "decision", "Job conformance verdict", "designed");
    builder.link_designed("candidate", "deterministic", "evaluated-by");
    builder.link_designed("deterministic", "behavioral", "evaluated-by");
    builder.link_designed("behavioral", "verdict", "records");

    for (index, artifact) in composite.journey.artifacts.iter().enumerate() {
        let id = format!("artifact-{index}");
        let state = match artifact.role {
            crate::conformance::JourneyArtifactRole::DeterministicEvaluation => {
                if composite.deterministic_status == crate::conformance::DeterministicStatus::Passed
                {
                    "verified"
                } else {
                    "blocked"
                }
            }
            crate::conformance::JourneyArtifactRole::BehavioralEvaluation => {
                if composite.behavioral_status == crate::conformance::BehavioralStatus::Passed {
                    "verified"
                } else if composite.behavioral_status
                    == crate::conformance::BehavioralStatus::Unassessed
                {
                    "observed"
                } else {
                    "blocked"
                }
            }
            _ => "observed",
        };
        let public_digest = matches!(
            artifact.access_class,
            AccessClass::Synthetic | AccessClass::SanitizedPublic
        );
        let label = format!("{} phase artifact", trace_role(artifact.role));
        if public_digest {
            builder.add_observed_ref(
                &id,
                trace_kind(artifact.role),
                &label,
                state,
                TraceArtifactRef {
                    schema_id: artifact.contract.clone(),
                    sha256: artifact.authority_sha256.clone(),
                    logical_name: None,
                },
            );
        } else {
            builder.add_observed(&id, trace_kind(artifact.role), &label, state);
        }
    }
    for link in &composite.journey.links {
        let Some(from) = composite
            .journey
            .artifacts
            .iter()
            .position(|artifact| artifact.artifact_id == link.from_artifact_id)
        else {
            continue;
        };
        let Some(to) = composite
            .journey
            .artifacts
            .iter()
            .position(|artifact| artifact.artifact_id == link.to_artifact_id)
        else {
            continue;
        };
        builder.link_observed(
            &format!("artifact-{from}"),
            &format!("artifact-{to}"),
            trace_relation(link.relation),
        );
    }
    builder.add_observed(
        "conformance-verdict",
        "decision",
        &format!("Verdict: {}", verdict_token(composite.verdict)),
        if matches!(
            composite.verdict,
            crate::conformance::QualificationVerdict::QualifiedForJobUnderEnvelope
        ) {
            "verified"
        } else if matches!(
            composite.verdict,
            crate::conformance::QualificationVerdict::Unassessed
        ) {
            "observed"
        } else {
            "blocked"
        },
    );
    builder.limitations.extend(
        composite
            .limitations
            .iter()
            .map(|_| "recorded-limitation".into()),
    );
    builder
        .limitations
        .push("composite-remains-authoritative".into());
    Ok(builder.finish(
        if matches!(
            composite.verdict,
            crate::conformance::QualificationVerdict::NotSufficientForJob
                | crate::conformance::QualificationVerdict::NotQualifiedForJobUnderEnvelope
        ) {
            "blocked"
        } else {
            "available"
        },
    ))
}

fn trace_kind(role: crate::conformance::JourneyArtifactRole) -> &'static str {
    use crate::conformance::JourneyArtifactRole::*;
    match role {
        Candidate | PackRelease | Requirements | ProductFoundation | EvaluatorInventory
        | PrivateRecordPolicy | PublicationApproval => "authority",
        SourceLineage => "source",
        NormalizedInput => "normalization",
        RoutedContext => "selection",
        DeterministicEvaluation | BehavioralEvaluation | ClaimsValidation | RunVerification => {
            "gate"
        }
        DecisionResult => "decision",
        _ => "authority",
    }
}

fn trace_role(role: crate::conformance::JourneyArtifactRole) -> &'static str {
    use crate::conformance::JourneyArtifactRole::*;
    match role {
        Candidate => "Candidate",
        PackRelease => "Pack release",
        Requirements => "Requirements",
        ProductFoundation => "Product foundation",
        SkillsRoute => "Skills route",
        Prompt => "Prompt",
        PromptInvocation => "Prompt invocation",
        SourceLineage => "Source lineage",
        NormalizedInput => "Normalized input",
        RoutedContext => "Routed context",
        GovernedOutput => "Governed output",
        ClaimsValidation => "Claims validation",
        DecisionResult => "Decision result",
        RunBundle => "Run bundle",
        RunReceipt => "Run receipt",
        RunVerification => "Run verification",
        EvaluatorInventory => "Evaluator inventory",
        PrivateRecordPolicy => "Private record policy",
        PublicationApproval => "Publication approval",
        DeterministicEvaluation => "Deterministic evaluation",
        BehavioralEvaluation => "Behavioral evaluation",
        Trial => "Behavioral trial",
    }
}

fn trace_relation(relation: JourneyRelation) -> &'static str {
    match relation {
        JourneyRelation::Declares | JourneyRelation::BoundTo => "bound-to",
        JourneyRelation::Normalizes | JourneyRelation::Generates | JourneyRelation::Reviews => {
            "records"
        }
        JourneyRelation::Selects => "selected",
        JourneyRelation::Evaluates => "evaluated-by",
        JourneyRelation::Verifies | JourneyRelation::Approves => "verified-by",
        JourneyRelation::Blocks => "blocked-by",
    }
}

fn verdict_token(verdict: crate::conformance::QualificationVerdict) -> &'static str {
    use crate::conformance::QualificationVerdict::*;
    match verdict {
        QualifiedForJobUnderEnvelope => "qualified-for-job-under-envelope",
        NotQualifiedForJobUnderEnvelope => "not-qualified-for-job-under-envelope",
        NotSufficientForJob => "not-sufficient-for-job",
        Unassessed => "unassessed",
    }
}

pub(crate) fn project_run_files(
    bundle_path: &Path,
    receipt_path: &Path,
    artifact_root: Option<&Path>,
) -> Result<DecisionTrace> {
    if [bundle_path, receipt_path].into_iter().any(|path| {
        fs::metadata(path)
            .map(|metadata| metadata.len() > MAX_TRACE_SOURCE_BYTES as u64)
            .unwrap_or(false)
    }) {
        return Ok(unavailable("run-authority-oversized", String::new()));
    }
    let bundle_bytes = match fs::read(bundle_path) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(unavailable("run-bundle-unreadable", String::new())),
    };
    let receipt_bytes = match fs::read(receipt_path) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(unavailable("run-receipt-unreadable", String::new())),
    };
    if bundle_bytes.len() > MAX_TRACE_SOURCE_BYTES || receipt_bytes.len() > MAX_TRACE_SOURCE_BYTES {
        return Ok(unavailable("run-authority-oversized", String::new()));
    }
    let bundle_hash = sha256_hex(&bundle_bytes);
    let receipt_hash = sha256_hex(&receipt_bytes);
    let bundle: RunBundleV1 =
        match parse_authority_json(&bundle_bytes, AuthorityJsonLimits::default()) {
            Ok(value) => value,
            Err(_) => return Ok(unavailable("run-bundle-malformed", bundle_hash)),
        };
    let receipt: RunReceiptV1 =
        match parse_authority_json(&receipt_bytes, AuthorityJsonLimits::default()) {
            Ok(value) => value,
            Err(_) => return Ok(unavailable("run-receipt-malformed", receipt_hash)),
        };
    let verification = verify_run(&bundle, &receipt, artifact_root)?;
    let decision_blocks_output = receipt
        .decision
        .as_ref()
        .is_some_and(|decision| blocks_output(&decision.decision, &decision.reason_codes));
    let mut builder = TraceBuilder::new(run_source(bundle_hash.clone(), receipt_hash.clone()));
    builder.authority.decision_authority = if verification.valid {
        "run-receipt"
    } else {
        "none"
    };
    builder.authority.verification_state = if verification.valid {
        "verified"
    } else {
        "failed"
    };
    builder.authority.output_authority = verification.valid
        && receipt.terminal_state.is_success()
        && receipt.output.is_some()
        && !decision_blocks_output;
    builder.add_designed("run-policy", "policy", "Declared run policy", "designed");
    builder.add_observed_ref(
        "run-bundle",
        "source",
        "Immutable run bundle",
        "observed",
        TraceArtifactRef {
            schema_id: RUN_BUNDLE_V1.into(),
            sha256: receipt.bundle_sha256.clone(),
            logical_name: None,
        },
    );
    builder.add_observed_ref(
        "run-receipt",
        "authority",
        "Run receipt authority",
        if verification.valid {
            "verified"
        } else {
            "blocked"
        },
        TraceArtifactRef {
            schema_id: RUN_RECEIPT_V1.into(),
            sha256: receipt.receipt_sha256.clone(),
            logical_name: None,
        },
    );
    builder.link_observed("run-bundle", "run-receipt", "bound-to");
    if let Some(decision) = &receipt.decision {
        if verification.valid {
            builder.add_observed_ref(
                "decision",
                "decision",
                &format!("Decision: {}", safe_token(&decision.decision)),
                "observed",
                TraceArtifactRef {
                    schema_id: decision.schema_id.clone(),
                    sha256: decision.sha256.clone(),
                    logical_name: None,
                },
            );
            builder.link_observed("run-receipt", "decision", "records");
            for code in &decision.reason_codes {
                let id = format!("reason-{}", builder.observed.nodes.len());
                if builder.add_observed(
                    &id,
                    "reason",
                    &format!("Reason code: {}", safe_token(code)),
                    if decision_blocks_output {
                        "blocked"
                    } else {
                        "observed"
                    },
                ) {
                    builder.link_observed(
                        "decision",
                        &id,
                        if decision_blocks_output {
                            "blocked-by"
                        } else {
                            "records"
                        },
                    );
                }
            }
        }
    }
    for (id, label, artifact, is_output) in [
        (
            "compiled-context",
            "Bound compiled context",
            receipt.compiled_context.as_ref(),
            false,
        ),
        (
            "validation",
            "Bound validation result",
            receipt.validation.as_ref(),
            false,
        ),
        (
            "output",
            "Bound output artifact",
            receipt.output.as_ref(),
            true,
        ),
    ] {
        let Some(artifact) = artifact else {
            continue;
        };
        let state = if !verification.valid || (is_output && !builder.authority.output_authority) {
            "blocked"
        } else {
            "verified"
        };
        if builder.add_observed_ref(
            id,
            "authority",
            label,
            state,
            TraceArtifactRef {
                schema_id: artifact.schema_id.clone(),
                sha256: artifact.sha256.clone(),
                logical_name: None,
            },
        ) {
            builder.link_observed("run-receipt", id, "bound-to");
        }
    }
    for issue in verification.issues.iter().take(32) {
        builder.add_observed(
            &format!("verification-{}", builder.observed.nodes.len()),
            "blocker",
            &format!("Verification issue: {}", safe_token(issue)),
            "blocked",
        );
    }
    if !verification.valid {
        builder.limitations.push("run-verification-failed".into());
    }
    if artifact_root.is_none() {
        builder
            .limitations
            .push("artifact-bytes-not-recomputed".into());
    }
    Ok(
        builder.finish(if verification.valid && !decision_blocks_output {
            "available"
        } else {
            "blocked"
        }),
    )
}

pub(crate) fn project_source_value(value: &Value, source_sha256: String) -> DecisionTrace {
    let (command, data) = match unwrap_cli_result(value) {
        Some(parts) => parts,
        None => (None, value),
    };
    let contract = data["contract"].as_str().map(str::to_string).or_else(|| {
        command
            .as_deref()
            .and_then(command_contract)
            .map(str::to_string)
    });
    let Some(contract) = contract else {
        return unavailable("unsupported-source-contract", source_sha256);
    };
    let source = TraceSource {
        contract: contract.clone(),
        command,
        sha256: source_sha256,
        class: if contract == RUN_EXECUTION_V1 {
            "run-execution"
        } else {
            "row-level"
        },
    };
    match contract.as_str() {
        "mdp.fit.v0" => project_fit(data, source),
        "mdp.route.v0" => project_route(data, source),
        "mdp.brief.v0" | "mdp.message-brief.v0" => project_brief(data, source),
        "mdp.prompt-output.v0" => project_prompt_output(data, source),
        RUN_EXECUTION_V1 => project_run_execution(data, source),
        JOB_CONFORMANCE_V1 => unavailable_with_source("composite-artifact-root-required", source),
        _ => unavailable_with_source("unsupported-source-contract", source),
    }
}

fn unwrap_cli_result(value: &Value) -> Option<(Option<String>, &Value)> {
    if value["ok"].as_bool() != Some(true) {
        return None;
    }
    let command = value["command"].as_str()?.to_string();
    let data = value.get("data")?;
    data.is_object().then_some((Some(command), data))
}

fn command_contract(command: &str) -> Option<&'static str> {
    match command {
        "fit" => Some("mdp.fit.v0"),
        "route" => Some("mdp.route.v0"),
        "brief" => Some("mdp.message-brief.v0"),
        "emit-brief" => Some("mdp.brief.v0"),
        "run" => Some(RUN_EXECUTION_V1),
        _ => None,
    }
}

fn project_fit(data: &Value, source: TraceSource) -> DecisionTrace {
    if !data["context"].is_object()
        || !data["context"]["missing_requirements"].is_array()
        || !data["context"]["invalid_requirements"].is_array()
        || !data["matches"].is_array()
        || !data["disqualifiers"].is_array()
    {
        return unavailable_with_source("invalid-fit-shape", source);
    }
    let status = data["status"].as_str().unwrap_or("unavailable");
    if !matches!(status, "fit" | "insufficient-context" | "disqualified") {
        return unavailable_with_source("invalid-fit-status", source);
    }
    let blocked = status != "fit";
    let mut builder = TraceBuilder::new(source);
    builder.authority.decision_authority = "source-artifact";
    builder.add_designed("fit-policy", "policy", "Pack fit policy", "designed");
    builder.add_designed("draft-gate", "gate", "Draft eligibility gate", "designed");
    builder.link_designed("fit-policy", "draft-gate", "governs");
    builder.add_observed("source", "source", "Saved fit result", "observed");
    builder.add_observed(
        "fit-decision",
        "decision",
        &format!("Fit status: {status}"),
        if blocked { "blocked" } else { "observed" },
    );
    builder.link_observed("source", "fit-decision", "records");
    add_field_reasons(
        &mut builder,
        &data["context"]["missing_requirements"],
        "fit-decision",
        "missing-field",
        "Missing required field",
    );
    add_field_reasons(
        &mut builder,
        &data["context"]["invalid_requirements"],
        "fit-decision",
        "invalid-field",
        "Invalid field",
    );
    add_id_reasons(
        &mut builder,
        &data["disqualifiers"],
        "entry_id",
        "disqualifier",
        "Blocked by rule",
    );
    add_id_reasons(
        &mut builder,
        &data["matches"],
        "id",
        "match",
        "Matched rule",
    );
    builder.finish(if blocked { "blocked" } else { "available" })
}

fn project_route(data: &Value, source: TraceSource) -> DecisionTrace {
    if !data["load_order"].is_array() {
        return unavailable_with_source("invalid-route-shape", source);
    }
    let status = data["draft_status"].as_str().unwrap_or("unavailable");
    if !matches!(status, "ready" | "blocked") {
        return unavailable_with_source("invalid-route-status", source);
    }
    let mut builder = TraceBuilder::new(source);
    builder.authority.decision_authority = "source-artifact";
    builder.add_designed(
        "persona-policy",
        "policy",
        "Persona resolution policy",
        "designed",
    );
    builder.add_designed("route-policy", "policy", "Card routing policy", "designed");
    builder.add_designed("draft-gate", "gate", "Draft eligibility gate", "designed");
    builder.link_designed("persona-policy", "route-policy", "governs");
    builder.link_designed("route-policy", "draft-gate", "governs");
    builder.add_observed("source", "source", "Saved route result", "observed");
    builder.add_observed(
        "route-decision",
        "decision",
        &format!("Route status: {status}"),
        if status == "blocked" {
            "blocked"
        } else {
            "observed"
        },
    );
    builder.link_observed("source", "route-decision", "records");
    let count = data["load_order"].as_array().map_or(0, Vec::len);
    builder.add_observed(
        "selected-context",
        "selection",
        &format!("Selected card count: {count}"),
        "observed",
    );
    builder.link_observed("route-decision", "selected-context", "selected");
    builder.finish(if status == "blocked" {
        "blocked"
    } else {
        "available"
    })
}

fn project_brief(data: &Value, source: TraceSource) -> DecisionTrace {
    if !data["required_load_order"].is_array() || !data["decision_trace"].is_array() {
        return unavailable_with_source("invalid-brief-shape", source);
    }
    let status = data["draft_status"].as_str().unwrap_or("unavailable");
    if !matches!(status, "ready" | "blocked" | "no-draft") {
        return unavailable_with_source("invalid-brief-status", source);
    }
    let blocked = status != "ready";
    let mut builder = TraceBuilder::new(source);
    builder.authority.decision_authority = "source-artifact";
    builder.authority.output_authority = !blocked;
    for (id, label) in [
        ("load-policy", "Pack loading policy"),
        ("route-policy", "Context routing policy"),
        ("draft-gate", "Draft eligibility gate"),
    ] {
        builder.add_designed(
            id,
            if id == "draft-gate" { "gate" } else { "policy" },
            label,
            "designed",
        );
    }
    builder.link_designed("load-policy", "route-policy", "governs");
    builder.link_designed("route-policy", "draft-gate", "governs");
    builder.add_observed("source", "source", "Saved brief result", "observed");
    builder.add_observed(
        "brief-decision",
        "decision",
        &format!("Draft status: {status}"),
        if blocked { "blocked" } else { "observed" },
    );
    builder.link_observed("source", "brief-decision", "records");
    let count = data["required_load_order"].as_array().map_or(0, Vec::len);
    builder.add_observed(
        "selected-context",
        "selection",
        &format!("Selected card count: {count}"),
        "observed",
    );
    builder.link_observed("brief-decision", "selected-context", "selected");
    builder.finish(if blocked { "blocked" } else { "available" })
}

fn project_prompt_output(data: &Value, source: TraceSource) -> DecisionTrace {
    if !data["normalization_trace"].is_object()
        || !data["normalization_trace"]["missing_required"].is_array()
    {
        return unavailable_with_source("normalization-trace-missing", source);
    }
    let Some(ready) = data["normalization_trace"]["fit_readiness"]["ready_for_mdp_fit"].as_bool()
    else {
        return unavailable_with_source("invalid-fit-readiness", source);
    };
    let mut builder = TraceBuilder::new(source);
    builder.authority.decision_authority = "source-artifact";
    builder.add_designed(
        "normalization-policy",
        "policy",
        "Declared normalization contract",
        "designed",
    );
    builder.add_designed("fit-readiness", "gate", "Fit readiness gate", "designed");
    builder.link_designed("normalization-policy", "fit-readiness", "governs");
    builder.add_observed("source", "source", "Validated prompt output", "observed");
    builder.add_observed(
        "normalization",
        "normalization",
        "Normalization trace present",
        "observed",
    );
    builder.add_observed(
        "readiness",
        "decision",
        if ready {
            "Ready for MDP fit"
        } else {
            "Not ready for MDP fit"
        },
        if ready { "observed" } else { "blocked" },
    );
    builder.link_observed("source", "normalization", "records");
    builder.link_observed("normalization", "readiness", "evaluated-by");
    add_field_reasons(
        &mut builder,
        &data["normalization_trace"]["missing_required"],
        "readiness",
        "missing-field",
        "Missing required field",
    );
    builder.finish(if ready { "available" } else { "blocked" })
}

fn project_run_execution(data: &Value, source: TraceSource) -> DecisionTrace {
    if data["valid"].as_bool().is_none()
        || !data["authority_block"].is_object()
        || !data["authority_block"]["reason_codes"].is_array()
    {
        return unavailable_with_source("invalid-run-execution-shape", source);
    }
    let terminal = data["terminal_state"].as_str().unwrap_or("unavailable");
    if !terminal.starts_with("no-draft:") && terminal != "success" {
        return unavailable_with_source("invalid-run-terminal-state", source);
    }
    let reason_codes = data["authority_block"]["reason_codes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let decision = data["authority_block"]["decision"]["decision"].as_str();
    let decision_blocks_output = decision.is_some_and(|value| blocks_output(value, &reason_codes));
    let success =
        terminal == "success" && data["valid"].as_bool() == Some(true) && !decision_blocks_output;
    let mut builder = TraceBuilder::new(source);
    builder.authority.decision_authority = if success {
        "run-receipt-reference"
    } else {
        "none"
    };
    builder.authority.output_authority = success;
    builder.authority.verification_state = if success { "referenced" } else { "not-created" };
    builder.add_designed("preflight", "gate", "Run preflight boundary", "designed");
    builder.add_designed(
        "publication",
        "gate",
        "Immutable publication boundary",
        "designed",
    );
    builder.link_designed("preflight", "publication", "governs");
    builder.add_observed("source", "source", "Run execution result", "observed");
    builder.add_observed(
        "terminal",
        "decision",
        &format!("Terminal state: {}", safe_token(terminal)),
        if success { "observed" } else { "blocked" },
    );
    builder.link_observed("source", "terminal", "records");
    for code in reason_codes.into_iter().take(32) {
        let index = builder.observed.nodes.len();
        if builder.add_observed(
            &format!("reason-{index}"),
            "reason",
            &format!("Reason code: {}", safe_token(code)),
            "blocked",
        ) {
            builder.link_observed("terminal", &format!("reason-{index}"), "blocked-by");
        }
    }
    builder.limitations.push(
        if success {
            "receipt-remains-authoritative"
        } else {
            "no-run-authority-created"
        }
        .into(),
    );
    builder.finish(if success { "available" } else { "blocked" })
}

fn add_field_reasons(
    builder: &mut TraceBuilder,
    value: &Value,
    parent: &str,
    prefix: &str,
    label: &str,
) {
    for entry in value.as_array().into_iter().flatten() {
        let Some(field) = entry["field"].as_str() else {
            continue;
        };
        let token = safe_field_token(field);
        let id = format!("{prefix}-{}", builder.observed.nodes.len());
        if builder.add_observed(&id, "reason", &format!("{label}: {token}"), "blocked") {
            builder.link_observed(parent, &id, "blocked-by");
        }
    }
}

fn add_id_reasons(builder: &mut TraceBuilder, value: &Value, key: &str, prefix: &str, label: &str) {
    for entry in value.as_array().into_iter().flatten() {
        let Some(raw) = entry[key].as_str() else {
            continue;
        };
        let token = safe_token(raw);
        let id = format!("{prefix}-{}", builder.observed.nodes.len());
        if builder.add_observed(
            &id,
            if prefix == "match" {
                "selection"
            } else {
                "reason"
            },
            &format!("{label}: {token}"),
            if prefix == "match" {
                "observed"
            } else {
                "blocked"
            },
        ) {
            builder.link_observed(
                "fit-decision",
                &id,
                if prefix == "match" {
                    "selected"
                } else {
                    "blocked-by"
                },
            );
        }
    }
}

struct TraceBuilder {
    source: TraceSource,
    authority: TraceAuthority,
    designed: TraceGraph,
    observed: TraceGraph,
    truncation: TraceTruncation,
    limitations: Vec<String>,
}

impl TraceBuilder {
    fn new(source: TraceSource) -> Self {
        Self {
            source,
            authority: TraceAuthority {
                projection_only: true,
                decision_authority: "none",
                output_authority: false,
                verification_state: "not-verified",
                notice: "This decision trace is a read-only projection. Its source artifacts retain all decision, output, and assurance authority.",
            },
            designed: TraceGraph::default(),
            observed: TraceGraph::default(),
            truncation: TraceTruncation::default(),
            limitations: vec!["projection-does-not-prove-source-truth".into()],
        }
    }

    fn add_designed(
        &mut self,
        id: &str,
        kind: &'static str,
        label: &str,
        state: &'static str,
    ) -> bool {
        if !self.reserve_node() {
            return false;
        }
        add_node(
            &mut self.designed,
            &mut self.truncation,
            id,
            kind,
            label,
            state,
            EvidenceProvenance::MdpObserved,
            vec![],
        );
        true
    }
    fn add_observed(
        &mut self,
        id: &str,
        kind: &'static str,
        label: &str,
        state: &'static str,
    ) -> bool {
        if !self.reserve_node() {
            return false;
        }
        add_node(
            &mut self.observed,
            &mut self.truncation,
            id,
            kind,
            label,
            state,
            EvidenceProvenance::MdpObserved,
            vec![],
        );
        true
    }
    fn add_observed_ref(
        &mut self,
        id: &str,
        kind: &'static str,
        label: &str,
        state: &'static str,
        artifact_ref: TraceArtifactRef,
    ) -> bool {
        if !self.reserve_node() {
            return false;
        }
        add_node(
            &mut self.observed,
            &mut self.truncation,
            id,
            kind,
            label,
            state,
            EvidenceProvenance::VerifierRecomputed,
            vec![artifact_ref],
        );
        true
    }
    fn link_designed(&mut self, from: &str, to: &str, kind: &'static str) {
        if !self.reserve_edge() {
            return;
        }
        add_edge(&mut self.designed, from, to, kind);
    }
    fn link_observed(&mut self, from: &str, to: &str, kind: &'static str) {
        if !self.reserve_edge() {
            return;
        }
        add_edge(&mut self.observed, from, to, kind);
    }
    fn reserve_node(&mut self) -> bool {
        if self.designed.nodes.len() + self.observed.nodes.len() < MAX_TRACE_NODES {
            return true;
        }
        self.truncation.truncated = true;
        self.truncation.omitted_nodes += 1;
        false
    }
    fn reserve_edge(&mut self) -> bool {
        if self.designed.edges.len() + self.observed.edges.len() < MAX_TRACE_EDGES {
            return true;
        }
        self.truncation.truncated = true;
        self.truncation.omitted_edges += 1;
        false
    }
    fn finish(mut self, status: &'static str) -> DecisionTrace {
        if self.truncation.truncated {
            self.limitations
                .push("projection-truncated-at-fixed-limits".into());
        }
        DecisionTrace {
            contract: DECISION_TRACE_V1,
            status,
            source: self.source,
            authority: self.authority,
            designed_graph: self.designed,
            observed_path: self.observed,
            truncation: self.truncation,
            limitations: self.limitations,
        }
    }
}

fn add_node(
    graph: &mut TraceGraph,
    truncation: &mut TraceTruncation,
    id: &str,
    kind: &'static str,
    label: &str,
    state: &'static str,
    evidence_provenance: EvidenceProvenance,
    artifact_refs: Vec<TraceArtifactRef>,
) {
    let (label, was_truncated) = truncate_utf8(&sanitize_label(label), MAX_TRACE_LABEL_BYTES);
    if was_truncated {
        truncation.truncated = true;
        truncation.labels_truncated += 1;
    }
    graph.nodes.push(TraceNode {
        id: safe_id(id),
        kind,
        label,
        state,
        evidence_provenance,
        artifact_refs,
    });
}

fn add_edge(graph: &mut TraceGraph, from: &str, to: &str, kind: &'static str) {
    graph.edges.push(TraceEdge {
        from: safe_id(from),
        to: safe_id(to),
        kind,
    });
}

fn unavailable(reason: &str, sha256: String) -> DecisionTrace {
    unavailable_with_source(
        reason,
        TraceSource {
            contract: "unknown".into(),
            command: None,
            sha256,
            class: "unknown",
        },
    )
}

fn unavailable_with_source(reason: &str, source: TraceSource) -> DecisionTrace {
    let mut builder = TraceBuilder::new(source);
    builder.limitations.push(reason.into());
    builder.finish("unavailable")
}

fn run_source(bundle_sha256: String, receipt_sha256: String) -> TraceSource {
    TraceSource {
        contract: format!("{RUN_BUNDLE_V1}+{RUN_RECEIPT_V1}"),
        command: Some("trace".into()),
        sha256: sha256_hex(format!("{bundle_sha256}:{receipt_sha256}").as_bytes()),
        class: "receipt-backed-run",
    }
}

fn safe_field_token(value: &str) -> String {
    if value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        value.to_string()
    } else {
        "redacted-field".into()
    }
}

fn blocks_output<T: AsRef<str>>(decision: &str, reason_codes: &[T]) -> bool {
    matches!(decision, "no-draft" | "blocked")
        || reason_codes.iter().any(|code| {
            matches!(
                code.as_ref(),
                "insufficient-context"
                    | "disqualified"
                    | "policy-blocked"
                    | "hard-gate-failed"
                    | "validation-failed"
            )
        })
}

fn safe_token(value: &str) -> String {
    let filtered: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let token = filtered.trim_matches('-');
    if token.is_empty() {
        "redacted".into()
    } else {
        truncate_utf8(token, 64).0
    }
}

fn safe_id(value: &str) -> String {
    let value = safe_token(value);
    if value.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        format!("n-{value}")
    } else {
        value
    }
}

fn sanitize_label(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_utf8(value: &str, max_bytes: usize) -> (String, bool) {
    if value.len() <= max_bytes {
        return (value.to_string(), false);
    }
    let mut end = max_bytes.saturating_sub(3).min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}...", &value[..end]), true)
}
