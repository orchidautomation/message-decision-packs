use super::{DECISION_TRACE_V1, MAX_TRACE_EDGES, MAX_TRACE_NODES};
use serde_json::{Value, json};

pub(crate) fn decision_trace_schema() -> Value {
    let artifact_ref = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_id", "sha256"],
        "properties": {
            "schema_id": {"type": "string", "maxLength": 160},
            "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
            "logical_name": {"type": "string", "maxLength": 240}
        }
    });
    let node = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "kind", "label", "state", "evidence_provenance", "artifact_refs"],
        "properties": {
            "id": {"type": "string", "pattern": "^[A-Za-z][A-Za-z0-9._:-]{0,119}$"},
            "kind": {"enum": ["source", "policy", "gate", "normalization", "selection", "reason", "decision", "authority", "blocker"]},
            "label": {"type": "string", "maxLength": 120},
            "state": {"enum": ["designed", "observed", "verified", "blocked"]},
            "evidence_provenance": {"enum": ["mdp-observed", "provider-returned", "customer-attested", "host-attested", "driver-attested", "verifier-recomputed", "unknown"]},
            "artifact_refs": {"type": "array", "maxItems": 16, "items": artifact_ref}
        }
    });
    let edge = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["from", "to", "kind"],
        "properties": {
            "from": {"type": "string"},
            "to": {"type": "string"},
            "kind": {"enum": ["governs", "records", "selected", "blocked-by", "bound-to", "verified-by", "evaluated-by"]}
        }
    });
    let graph = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["nodes", "edges"],
        "properties": {
            "nodes": {"type": "array", "maxItems": MAX_TRACE_NODES, "items": node},
            "edges": {"type": "array", "maxItems": MAX_TRACE_EDGES, "items": edge}
        }
    });
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Decision Trace v1",
        "type": "object",
        "additionalProperties": false,
        "required": ["contract", "status", "source", "authority", "designed_graph", "observed_path", "truncation", "limitations"],
        "properties": {
            "contract": {"const": DECISION_TRACE_V1},
            "status": {"enum": ["available", "blocked", "unavailable"]},
            "source": {
                "type": "object", "additionalProperties": false,
                "required": ["contract", "sha256", "class"],
                "properties": {
                    "contract": {"type": "string"},
                    "command": {"type": "string"},
                    "sha256": {"type": "string", "pattern": "^$|^[0-9a-f]{64}$"},
                    "class": {"enum": ["unknown", "row-level", "run-execution", "receipt-backed-run", "composite-conformance"]}
                }
            },
            "authority": {
                "type": "object", "additionalProperties": false,
                "required": ["projection_only", "decision_authority", "output_authority", "verification_state", "notice"],
                "properties": {
                    "projection_only": {"const": true},
                    "decision_authority": {"enum": ["none", "source-artifact", "run-receipt-reference", "run-receipt", "composite-conformance"]},
                    "output_authority": {"type": "boolean"},
                    "verification_state": {"enum": ["not-verified", "not-created", "referenced", "verified", "failed"]},
                    "notice": {"type": "string"}
                }
            },
            "designed_graph": graph.clone(),
            "observed_path": graph,
            "truncation": {
                "type": "object", "additionalProperties": false,
                "required": ["truncated", "omitted_nodes", "omitted_edges", "labels_truncated"],
                "properties": {
                    "truncated": {"type": "boolean"},
                    "omitted_nodes": {"type": "integer", "minimum": 0},
                    "omitted_edges": {"type": "integer", "minimum": 0},
                    "labels_truncated": {"type": "integer", "minimum": 0}
                }
            },
            "limitations": {"type": "array", "maxItems": 128, "items": {"type": "string", "maxLength": 160}}
        }
    })
}
