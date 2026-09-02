use crate::commands::decision_trace::{DecisionTrace, render_mermaid};
use serde::Serialize;
use serde_json::{Value, json};

pub(crate) const DECISION_CARD_V1: &str = "mdp.decision-card.v1";
const MAX_CARD_ITEMS: usize = 64;
const MAX_CARD_LABEL_BYTES: usize = 160;

#[derive(Debug, Serialize)]
pub(crate) struct DecisionCard {
    contract: &'static str,
    status: &'static str,
    subject: CardSubject,
    decision: CardDecision,
    authority: CardAuthority,
    source: CardSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    persona_resolution: Option<CardPersonaResolution>,
    classifications: Vec<CardClassification>,
    reasons: Vec<CardItem>,
    evidence: Vec<CardEvidence>,
    gaps: Vec<CardItem>,
    next_action: String,
    trace: DecisionTrace,
    limitations: Vec<String>,
    truncation: CardTruncation,
}

#[derive(Debug, Serialize)]
struct CardSubject {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_label: Option<String>,
}

#[derive(Debug, Serialize)]
struct CardDecision {
    outcome: String,
    action_gate: &'static str,
}

#[derive(Debug, Serialize)]
struct CardAuthority {
    projection_only: bool,
    decision_authority: &'static str,
    output_authority: bool,
    verification_state: &'static str,
    notice: &'static str,
}

#[derive(Debug, Serialize)]
struct CardSource {
    contract: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct CardItem {
    id: String,
    label: String,
}

#[derive(Debug, Serialize)]
struct CardPersonaResolution {
    persona: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fit_usable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    needs_review: Option<bool>,
}

#[derive(Debug, Serialize)]
struct CardClassification {
    attribute_id: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    taxonomy_id: String,
    taxonomy_version: String,
    basis: String,
    derived_from: Vec<String>,
}

#[derive(Debug, Serialize)]
struct CardEvidence {
    id: String,
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    signal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    observed_at: Option<String>,
    attempt_ids: Vec<String>,
    artifact_refs: Vec<CardArtifactRef>,
}

#[derive(Debug, Serialize)]
struct CardArtifactRef {
    schema_id: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct CardTruncation {
    truncated: bool,
    omitted_items: usize,
    trace_truncated: bool,
}

#[cfg(test)]
pub(crate) fn project_decision_card(trace: DecisionTrace) -> DecisionCard {
    project_decision_card_with_source(trace, None)
}

pub(crate) fn project_decision_card_with_source(
    trace: DecisionTrace,
    source_value: Option<&Value>,
) -> DecisionCard {
    let status = trace.status;
    let decision_node = trace
        .observed_path
        .nodes
        .iter()
        .find(|node| node.kind == "decision");
    let outcome = decision_node
        .map(|node| value_after_colon(&node.label))
        .unwrap_or_else(|| status.to_string());
    let action_gate = action_gate(&trace, &outcome);

    let mut omitted_items = 0usize;
    let fit_source = fit_source_data(&trace, source_value);
    let classifications = fit_source
        .map(|source| collect_fit_classifications(source, &mut omitted_items))
        .unwrap_or_default();
    let persona_resolution = fit_source.and_then(collect_persona_resolution);
    let reasons = fit_source
        .map(|source| collect_fit_reasons(source, &mut omitted_items))
        .unwrap_or_else(|| {
            collect_items(
                &trace,
                |node| node.kind == "reason" || node.id.starts_with("match-"),
                &mut omitted_items,
            )
        });
    let gaps = collect_items(
        &trace,
        |node| node.state == "blocked" && matches!(node.kind, "reason" | "blocker"),
        &mut omitted_items,
    );
    let evidence = fit_source
        .map(|source| collect_fit_evidence(source, &mut omitted_items))
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| collect_evidence(&trace, &mut omitted_items));
    let source_contract = bounded(&trace.source.contract);
    let source_sha256 = trace.source.sha256.clone();
    let trace_truncated = trace.truncation.truncated;
    let mut limitations = trace.limitations.clone();
    limitations.push(
        "card-content-is-deterministically-derived-from-trace-and-allowlisted-source-fields".into(),
    );
    if trace.source.class == "row-level" {
        limitations.push("subject-display-label-not-projected-from-private-row-fields".into());
    }
    if omitted_items > 0 {
        limitations.push("decision-card-items-truncated-at-fixed-limits".into());
    }

    DecisionCard {
        contract: DECISION_CARD_V1,
        status,
        subject: CardSubject {
            kind: subject_kind(&source_contract),
            display_label: safe_subject_label(&trace),
        },
        decision: CardDecision {
            outcome,
            action_gate,
        },
        authority: CardAuthority {
            projection_only: true,
            decision_authority: trace.authority.decision_authority,
            output_authority: trace.authority.output_authority,
            verification_state: trace.authority.verification_state,
            notice: "This decision card is an operator projection. The identified source artifact or receipt retains authority.",
        },
        source: CardSource {
            contract: source_contract,
            sha256: source_sha256,
        },
        persona_resolution,
        classifications,
        reasons,
        evidence,
        gaps,
        next_action: next_action(action_gate).into(),
        trace,
        limitations,
        truncation: CardTruncation {
            truncated: trace_truncated || omitted_items > 0,
            omitted_items,
            trace_truncated,
        },
    }
}

fn collect_items(
    trace: &DecisionTrace,
    predicate: impl Fn(&crate::commands::decision_trace::TraceNode) -> bool,
    omitted: &mut usize,
) -> Vec<CardItem> {
    let mut items = Vec::new();
    for node in trace
        .observed_path
        .nodes
        .iter()
        .filter(|node| predicate(node))
    {
        if items.len() == MAX_CARD_ITEMS {
            *omitted += 1;
            continue;
        }
        items.push(CardItem {
            id: bounded(&node.id),
            label: bounded(&node.label),
        });
    }
    items
}

fn fit_source_data<'a>(trace: &DecisionTrace, source: Option<&'a Value>) -> Option<&'a Value> {
    if trace.source.contract != "mdp.fit.v0" || trace.status == "unavailable" {
        return None;
    }
    let value = source?;
    let data = if value["ok"].as_bool() == Some(true) && value["command"].as_str() == Some("fit") {
        value.get("data")?
    } else {
        value
    };
    (data["contract"].as_str() == Some("mdp.fit.v0")).then_some(data)
}

fn collect_persona_resolution(source: &Value) -> Option<CardPersonaResolution> {
    let persona = source["persona_resolution"]["persona"].as_str()?;
    Some(CardPersonaResolution {
        persona: bounded(persona),
        source: source["persona_resolution"]["source"].as_str().map(bounded),
        confidence: scalar_token(&source["persona_resolution"]["confidence"]),
        resolved: source["persona_resolution"]["resolved"].as_bool(),
        fit_usable: source["persona_resolution"]["fit_usable"].as_bool(),
        needs_review: source["persona_resolution"]["needs_review"].as_bool(),
    })
}

fn collect_fit_classifications(source: &Value, omitted: &mut usize) -> Vec<CardClassification> {
    let classifications = source["classifications"]
        .as_object()
        .or_else(|| source["normalization"]["classifications"].as_object());
    let mut result = Vec::new();
    for (attribute_id, classification) in classifications.into_iter().flatten() {
        if result.len() == MAX_CARD_ITEMS {
            *omitted += 1;
            continue;
        }
        let Some(status) = classification["status"].as_str() else {
            continue;
        };
        let Some(taxonomy_id) = classification["taxonomy_id"].as_str() else {
            continue;
        };
        let Some(taxonomy_version) = classification["taxonomy_version"].as_str() else {
            continue;
        };
        let Some(basis) = classification["basis"].as_str() else {
            continue;
        };
        let Some(derived_from) = string_array(&classification["derived_from"]) else {
            continue;
        };
        result.push(CardClassification {
            attribute_id: bounded(attribute_id),
            status: bounded(status),
            value: scalar_token(&classification["value"]),
            taxonomy_id: bounded(taxonomy_id),
            taxonomy_version: bounded(taxonomy_version),
            basis: bounded(basis),
            derived_from,
        });
    }
    result
}

fn collect_fit_reasons(source: &Value, omitted: &mut usize) -> Vec<CardItem> {
    let mut result = Vec::new();
    for (items, key, prefix) in [
        (&source["matches"], "id", "Matched rule"),
        (&source["disqualifiers"], "entry_id", "Disqualifying rule"),
        (
            &source["context"]["missing_requirements"],
            "field",
            "Missing required field",
        ),
        (
            &source["context"]["invalid_requirements"],
            "field",
            "Invalid field",
        ),
    ] {
        for item in items.as_array().into_iter().flatten() {
            let Some(id) = item[key].as_str() else {
                continue;
            };
            if result.len() == MAX_CARD_ITEMS {
                *omitted += 1;
                continue;
            }
            result.push(CardItem {
                id: bounded(id),
                label: bounded(&format!("{prefix}: {id}")),
            });
        }
    }
    result
}

fn collect_fit_evidence(source: &Value, omitted: &mut usize) -> Vec<CardEvidence> {
    let mut result = Vec::new();
    for signal in source["signal_authority"]["accepted"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let Some(signal_id) = signal["signal_id"].as_str() else {
            continue;
        };
        let receipts = signal["observation_receipts"].as_array();
        if receipts.is_none_or(Vec::is_empty) {
            push_evidence(
                &mut result,
                omitted,
                CardEvidence {
                    id: bounded(signal_id),
                    label: bounded(&format!("Accepted signal: {signal_id}")),
                    signal_id: Some(bounded(signal_id)),
                    source_class: None,
                    confidence: None,
                    observed_at: None,
                    attempt_ids: Vec::new(),
                    artifact_refs: Vec::new(),
                },
            );
            continue;
        }
        for receipt in receipts.into_iter().flatten() {
            let Some(observation_id) = receipt["id"].as_str() else {
                continue;
            };
            let Some(attempt_ids) = string_array(&receipt["attempt_ids"]) else {
                continue;
            };
            push_evidence(
                &mut result,
                omitted,
                CardEvidence {
                    id: bounded(observation_id),
                    label: bounded(&format!("Accepted signal: {signal_id}")),
                    signal_id: Some(bounded(signal_id)),
                    source_class: scalar_token(&receipt["source_class"]),
                    confidence: scalar_token(&receipt["confidence"]),
                    observed_at: scalar_token(&receipt["observed_at"]),
                    attempt_ids,
                    artifact_refs: Vec::new(),
                },
            );
        }
    }
    result
}

fn push_evidence(result: &mut Vec<CardEvidence>, omitted: &mut usize, item: CardEvidence) {
    if result.len() == MAX_CARD_ITEMS {
        *omitted += 1;
    } else {
        result.push(item);
    }
}

fn string_array(value: &Value) -> Option<Vec<String>> {
    value.as_array().map(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .take(16)
            .map(bounded)
            .collect()
    })
}

fn scalar_token(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(bounded(value)),
        Value::Number(value) => Some(bounded(&value.to_string())),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn collect_evidence(trace: &DecisionTrace, omitted: &mut usize) -> Vec<CardEvidence> {
    let mut evidence = Vec::new();
    for node in trace.observed_path.nodes.iter().filter(|node| {
        node.kind == "authority"
            || node.id.starts_with("match-")
            || node.id.starts_with("observation-")
            || node.id.starts_with("accepted-signal-")
            || !node.artifact_refs.is_empty()
    }) {
        if evidence.len() == MAX_CARD_ITEMS {
            *omitted += 1;
            continue;
        }
        evidence.push(CardEvidence {
            id: bounded(&node.id),
            label: bounded(&node.label),
            signal_id: None,
            source_class: None,
            confidence: None,
            observed_at: None,
            attempt_ids: Vec::new(),
            artifact_refs: node
                .artifact_refs
                .iter()
                .map(|artifact| CardArtifactRef {
                    schema_id: bounded(&artifact.schema_id),
                    sha256: artifact.sha256.clone(),
                })
                .collect(),
        });
    }
    evidence
}

fn action_gate(trace: &DecisionTrace, outcome: &str) -> &'static str {
    if trace.status == "unavailable" {
        "unavailable"
    } else if outcome == "disqualified" || outcome.starts_with("no-draft") {
        "no-draft"
    } else if trace.status == "blocked" || outcome == "blocked" {
        "blocked"
    } else if trace.authority.output_authority {
        "allow-review"
    } else if trace.authority.decision_authority == "none" {
        "blocked"
    } else {
        "needs-review"
    }
}

fn next_action(gate: &str) -> &'static str {
    match gate {
        "allow-review" => {
            "Review the authoritative source and trace before any human-controlled downstream action."
        }
        "needs-review" => {
            "Resolve the stated gaps and obtain human review; this card grants no drafting or sending authority."
        }
        "no-draft" => {
            "Do not draft or send. Resolve blocking evidence or policy conditions in the authoritative workflow."
        }
        "blocked" => "Stop and verify the authoritative source before taking downstream action.",
        _ => "No action is authorized from this unavailable projection.",
    }
}

fn subject_kind(contract: &str) -> &'static str {
    match contract {
        "mdp.fit.v0" | "mdp.route.v0" | "mdp.brief.v0" | "mdp.message-brief.v0" => "record",
        "mdp.run-execution.v1" => "run",
        value if value.contains("run-bundle") || value.contains("run-receipt") => "run",
        value if value.contains("conformance") => "conformance",
        _ => "unknown",
    }
}

fn safe_subject_label(trace: &DecisionTrace) -> Option<String> {
    trace
        .observed_path
        .nodes
        .iter()
        .find(|node| node.kind == "source")
        .map(|node| bounded(&node.label))
}

fn value_after_colon(value: &str) -> String {
    bounded(value.split_once(':').map_or(value, |(_, tail)| tail.trim()))
}

fn bounded(value: &str) -> String {
    let clean = value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if clean.len() <= MAX_CARD_LABEL_BYTES {
        return clean;
    }
    let mut boundary = MAX_CARD_LABEL_BYTES;
    while !clean.is_char_boundary(boundary) {
        boundary -= 1;
    }
    clean[..boundary].to_string()
}

pub(crate) fn render_decision_card_markdown(card: &DecisionCard) -> String {
    let mut out = String::new();
    out.push_str("# Executive Decision Card\n\n");
    out.push_str("> **Projection only.** The authoritative source artifact or receipt retains all decision, output, and assurance authority.\n\n");
    section(
        &mut out,
        "Decision and Gate",
        &[
            ("Outcome", &card.decision.outcome),
            ("Action gate", card.decision.action_gate),
            ("Projection status", card.status),
        ],
    );
    section(
        &mut out,
        "Evaluated Subject",
        &[
            ("Kind", card.subject.kind),
            (
                "Label",
                card.subject
                    .display_label
                    .as_deref()
                    .unwrap_or("Unavailable by privacy-safe projection"),
            ),
        ],
    );
    if let Some(persona) = &card.persona_resolution {
        let mut label = format!("Persona: {}", persona.persona);
        if let Some(source) = &persona.source {
            label.push_str(&format!(" (source: {source})"));
        }
        render_items(
            &mut out,
            "Persona Resolution",
            &[CardItem {
                id: "persona".into(),
                label,
            }],
        );
    }
    render_classifications(&mut out, &card.classifications);
    render_items(&mut out, "Why This Decision", &card.reasons);
    if !card.evidence.is_empty() {
        out.push_str("## Evidence Used\n\n");
        for item in &card.evidence {
            out.push_str(&format!(
                "- `{}` — {}",
                markdown(&item.id),
                markdown(&item.label),
            ));
            for artifact in &item.artifact_refs {
                out.push_str(&format!(
                    " — `{}` (`{}`)",
                    markdown(&artifact.schema_id),
                    markdown(&artifact.sha256)
                ));
            }
            if !item.attempt_ids.is_empty() {
                out.push_str(&format!(
                    " — attempts: {}",
                    item.attempt_ids
                        .iter()
                        .map(|id| format!("`{}`", markdown(id)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            for (name, value) in [
                ("source", item.source_class.as_deref()),
                ("confidence", item.confidence.as_deref()),
                ("observed", item.observed_at.as_deref()),
            ] {
                if let Some(value) = value {
                    out.push_str(&format!(" — {name}: `{}`", markdown(value)));
                }
            }
            out.push('\n');
        }
        out.push('\n');
    }
    render_items(&mut out, "Gaps and Blockers", &card.gaps);
    out.push_str("## Allowed Next Action\n\n");
    out.push_str(&format!("{}\n\n", markdown(&card.next_action)));
    out.push_str("## Decision Path\n\n```mermaid\n");
    out.push_str(&render_mermaid(&card.trace));
    out.push_str("```\n\n");
    section(
        &mut out,
        "Authority",
        &[
            ("Source contract", &card.source.contract),
            ("Source SHA-256", &card.source.sha256),
            ("Decision authority", card.authority.decision_authority),
            ("Verification", card.authority.verification_state),
            (
                "Output authority",
                if card.authority.output_authority {
                    "true"
                } else {
                    "false"
                },
            ),
        ],
    );
    if !card.limitations.is_empty() {
        out.push_str("## Limitations\n\n");
        for limitation in &card.limitations {
            out.push_str(&format!("- `{}`\n", markdown(limitation)));
        }
        out.push('\n');
    }
    out
}

fn section(out: &mut String, title: &str, rows: &[(&str, &str)]) {
    out.push_str(&format!("## {title}\n\n"));
    for (label, value) in rows {
        out.push_str(&format!("- **{label}:** {}\n", markdown(value)));
    }
    out.push('\n');
}

fn render_items(out: &mut String, title: &str, items: &[CardItem]) {
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("## {title}\n\n"));
    for item in items {
        out.push_str(&format!(
            "- `{}` — {}\n",
            markdown(&item.id),
            markdown(&item.label)
        ));
    }
    out.push('\n');
}

fn render_classifications(out: &mut String, items: &[CardClassification]) {
    if items.is_empty() {
        return;
    }
    out.push_str("## Classification\n\n");
    for item in items {
        let value = item.value.as_deref().unwrap_or("unavailable");
        out.push_str(&format!(
            "- `{}` — {} (`{}`@`{}`; status `{}`; evidence {})\n",
            markdown(&item.attribute_id),
            markdown(value),
            markdown(&item.taxonomy_id),
            markdown(&item.taxonomy_version),
            markdown(&item.status),
            item.derived_from
                .iter()
                .map(|id| format!("`{}`", markdown(id)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    out.push('\n');
}

fn markdown(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('`', "&#96;")
}

pub(crate) fn decision_card_schema() -> Value {
    let item = json!({
        "type": "object", "additionalProperties": false,
        "required": ["id", "label"],
        "properties": {"id": {"type": "string", "maxLength": 160}, "label": {"type": "string", "maxLength": 160}}
    });
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Decision Card v1",
        "type": "object", "additionalProperties": false,
        "required": ["contract", "status", "subject", "decision", "authority", "source", "classifications", "reasons", "evidence", "gaps", "next_action", "trace", "limitations", "truncation"],
        "properties": {
            "contract": {"const": DECISION_CARD_V1},
            "status": {"enum": ["available", "blocked", "unavailable"]},
            "subject": {"type": "object", "additionalProperties": false, "required": ["kind"], "properties": {"kind": {"enum": ["record", "run", "conformance", "unknown"]}, "display_label": {"type": "string", "maxLength": 160}}},
            "decision": {"type": "object", "additionalProperties": false, "required": ["outcome", "action_gate"], "properties": {"outcome": {"type": "string", "maxLength": 160}, "action_gate": {"enum": ["allow-review", "needs-review", "no-draft", "blocked", "unavailable"]}}},
            "authority": {"type": "object", "additionalProperties": false, "required": ["projection_only", "decision_authority", "output_authority", "verification_state", "notice"], "properties": {"projection_only": {"const": true}, "decision_authority": {"type": "string"}, "output_authority": {"type": "boolean"}, "verification_state": {"type": "string"}, "notice": {"type": "string", "maxLength": 240}}},
            "source": {"type": "object", "additionalProperties": false, "required": ["contract", "sha256"], "properties": {"contract": {"type": "string", "maxLength": 320}, "sha256": {"type": "string", "pattern": "^$|^[0-9a-f]{64}$"}}},
            "persona_resolution": {"type": "object", "additionalProperties": false, "required": ["persona"], "properties": {"persona": {"type": "string", "maxLength": 160}, "source": {"type": "string", "maxLength": 160}, "confidence": {"type": "string", "maxLength": 160}, "resolved": {"type": "boolean"}, "fit_usable": {"type": "boolean"}, "needs_review": {"type": "boolean"}}},
            "classifications": {"type": "array", "maxItems": MAX_CARD_ITEMS, "items": {"type": "object", "additionalProperties": false, "required": ["attribute_id", "status", "taxonomy_id", "taxonomy_version", "basis", "derived_from"], "properties": {"attribute_id": {"type": "string", "maxLength": 160}, "status": {"type": "string", "maxLength": 160}, "value": {"type": "string", "maxLength": 160}, "taxonomy_id": {"type": "string", "maxLength": 160}, "taxonomy_version": {"type": "string", "maxLength": 160}, "basis": {"type": "string", "maxLength": 160}, "derived_from": {"type": "array", "maxItems": 16, "items": {"type": "string", "maxLength": 160}}}}},
            "reasons": {"type": "array", "maxItems": MAX_CARD_ITEMS, "items": item.clone()},
            "evidence": {"type": "array", "maxItems": MAX_CARD_ITEMS, "items": {"type": "object", "additionalProperties": false, "required": ["id", "label", "attempt_ids", "artifact_refs"], "properties": {"id": {"type": "string", "maxLength": 160}, "label": {"type": "string", "maxLength": 160}, "signal_id": {"type": "string", "maxLength": 160}, "source_class": {"type": "string", "maxLength": 160}, "confidence": {"type": "string", "maxLength": 160}, "observed_at": {"type": "string", "maxLength": 160}, "attempt_ids": {"type": "array", "maxItems": 16, "items": {"type": "string", "maxLength": 160}}, "artifact_refs": {"type": "array", "maxItems": 16, "items": {"type": "object", "additionalProperties": false, "required": ["schema_id", "sha256"], "properties": {"schema_id": {"type": "string", "maxLength": 160}, "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"}}}}}}},
            "gaps": {"type": "array", "maxItems": MAX_CARD_ITEMS, "items": item},
            "next_action": {"type": "string", "maxLength": 320},
            "trace": crate::commands::decision_trace::decision_trace_schema(),
            "limitations": {"type": "array", "maxItems": 130, "items": {"type": "string", "maxLength": 160}},
            "truncation": {"type": "object", "additionalProperties": false, "required": ["truncated", "omitted_items", "trace_truncated"], "properties": {"truncated": {"type": "boolean"}, "omitted_items": {"type": "integer", "minimum": 0}, "trace_truncated": {"type": "boolean"}}}
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::decision_trace::project_source_value;
    use serde_json::json;

    #[test]
    fn fit_card_is_schema_valid_and_omits_private_source_prose() {
        let source = json!({
            "contract": "mdp.fit.v0", "status": "fit",
            "context": {"missing_requirements": [], "invalid_requirements": []},
            "matches": [{"id": "rule-safe", "body": "Private rule prose"}],
            "disqualifiers": [],
            "persona_resolution": {"persona": "GTM Engineering", "source": "title-map", "confidence": "high", "resolved": true, "fit_usable": true, "private_note": "Never copy"},
            "classifications": {"segment": {"status": "classified", "value": "enterprise", "taxonomy_id": "company-segment", "taxonomy_version": "1", "basis": "Declared classification basis", "derived_from": ["attempt-safe"]}},
            "signal_authority": {"accepted": [{"signal_id": "signal-safe", "value": "Private observation prose", "observation_receipts": [{"id": "observation-safe", "attempt_ids": ["attempt-safe"], "source_class": "synthetic_fixture", "confidence": 100, "observed_at": "2026-09-02T00:00:00Z", "value": "Never expose"}]}]},
            "prospect": {"name": "Private Person", "email": "private@example.com"}
        });
        let trace = project_source_value(&source, "a".repeat(64));
        let card = project_decision_card_with_source(trace, Some(&source));
        let value = serde_json::to_value(&card).unwrap();
        jsonschema::draft202012::validate(&decision_card_schema(), &value).unwrap();
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(encoded.contains("rule-safe"));
        assert_eq!(value["reasons"][0]["id"], "rule-safe");
        assert_eq!(value["classifications"][0]["attribute_id"], "segment");
        assert_eq!(value["evidence"][0]["id"], "observation-safe");
        assert_eq!(value["evidence"][0]["attempt_ids"][0], "attempt-safe");
        assert_eq!(value["persona_resolution"]["persona"], "GTM Engineering");
        assert!(!encoded.contains("Private Person"));
        assert!(!encoded.contains("private@example.com"));
        assert!(!encoded.contains("Private rule prose"));
        assert!(!encoded.contains("Private observation prose"));
        assert!(!encoded.contains("Never copy"));
        assert!(!encoded.contains("Never expose"));
    }

    #[test]
    fn blocked_fit_never_grants_output_authority_or_drafting() {
        let trace = project_source_value(
            &json!({
                "contract": "mdp.fit.v0", "status": "insufficient-context",
                "context": {"missing_requirements": [{"field": "title"}], "invalid_requirements": []},
                "matches": [], "disqualifiers": []
            }),
            "b".repeat(64),
        );
        let card = project_decision_card(trace);
        let value = serde_json::to_value(&card).unwrap();
        assert_eq!(value["decision"]["action_gate"], "blocked");
        assert_eq!(value["authority"]["output_authority"], false);
        assert!(
            value["gaps"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
    }

    #[test]
    fn disqualified_fit_uses_no_draft_while_blocked_fit_stays_blocked() {
        let trace = project_source_value(
            &json!({
                "contract": "mdp.fit.v0", "status": "disqualified",
                "context": {"missing_requirements": [], "invalid_requirements": []},
                "matches": [], "disqualifiers": [{"entry_id": "do-not-contact"}]
            }),
            "d".repeat(64),
        );
        let value = serde_json::to_value(project_decision_card(trace)).unwrap();
        assert_eq!(value["decision"]["action_gate"], "no-draft");
    }

    #[test]
    fn unsupported_contract_is_sanitized_and_schema_valid() {
        let private_contract = format!("private-person@example.com-{}", "x".repeat(400));
        let trace = project_source_value(&json!({"contract": private_contract}), "e".repeat(64));
        let value = serde_json::to_value(project_decision_card(trace)).unwrap();
        jsonschema::draft202012::validate(&decision_card_schema(), &value).unwrap();
        assert_eq!(value["status"], "unavailable");
        assert_eq!(value["source"]["contract"], "unknown");
        assert!(!value.to_string().contains("private-person"));
    }

    #[test]
    fn markdown_is_outcome_first_and_embeds_the_canonical_trace() {
        let trace = project_source_value(
            &json!({
                "contract": "mdp.fit.v0", "status": "fit",
                "context": {"missing_requirements": [], "invalid_requirements": []},
                "matches": [{"id": "rule-one"}], "disqualifiers": []
            }),
            "c".repeat(64),
        );
        let markdown = render_decision_card_markdown(&project_decision_card(trace));
        assert!(markdown.contains("# Executive Decision Card"));
        assert!(markdown.contains("## Decision and Gate"));
        assert!(markdown.contains("```mermaid\nflowchart TD"));
        assert!(markdown.contains("rule-one"));
    }

    #[test]
    fn authority_corpus_decision_card_cases_match_the_live_projection() {
        let corpus: Value = serde_json::from_str(include_str!(
            "../../../plugin/assets/authority-conformance/corpus.json"
        ))
        .unwrap();
        let cases = corpus["cases"].as_array().unwrap();
        let blocked = cases
            .iter()
            .find(|case| case["id"] == "mdp-339-decision-card-blocked-preserves-block")
            .unwrap();
        let unavailable = cases
            .iter()
            .find(|case| case["id"] == "mdp-339-decision-card-unavailable-preserves-unavailable")
            .unwrap();

        let blocked_card = serde_json::to_value(project_decision_card(project_source_value(
            &json!({
                "contract": "mdp.fit.v0", "status": "insufficient-context",
                "context": {"missing_requirements": [{"field": "title"}], "invalid_requirements": []},
                "matches": [], "disqualifiers": []
            }),
            "f".repeat(64),
        )))
        .unwrap();
        assert_eq!(blocked_card["status"], blocked["expected"]["card_status"]);
        assert_eq!(
            blocked_card["decision"]["action_gate"],
            blocked["expected"]["action_gate"]
        );
        assert_eq!(
            blocked_card["authority"]["output_authority"],
            blocked["expected"]["output_authority"]
        );

        let unavailable_card = serde_json::to_value(project_decision_card(project_source_value(
            &json!({"contract": "private.unsupported"}),
            "0".repeat(64),
        )))
        .unwrap();
        assert_eq!(
            unavailable_card["status"],
            unavailable["expected"]["card_status"]
        );
        assert_eq!(
            unavailable_card["decision"]["action_gate"],
            unavailable["expected"]["action_gate"]
        );
        assert_eq!(
            unavailable_card["authority"]["decision_authority"],
            unavailable["expected"]["decision_authority"]
        );
    }
}
