use crate::cli::SchemaTarget;
use crate::constants::FORMAT_VERSION;
use serde_json::{Value, json};

pub(crate) fn schema(target: SchemaTarget) -> Value {
    let card_kinds = [
        "personas",
        "pains",
        "motions",
        "hooks",
        "avoid-rules",
        "copy-patterns",
        "ctas",
        "fit-rules",
        "claims",
        "signals",
        "positioning",
        "channel-policies",
        "objections",
        "gaps",
    ];
    match target {
        SchemaTarget::Manifest => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Manifest v0", "type": "object", "required": ["format", "id", "name", "version", "personas", "cards", "policy", "provenance"], "properties": {"format": {"const": FORMAT_VERSION}, "id": {"type": "string"}, "name": {"type": "string"}, "version": {"type": "string"}, "personas": {"type": "array", "items": {"type": "string"}}, "target_personas": {"type": "array", "items": {"type": "string"}}, "operator_roles": {"type": "array", "items": {"type": "string"}}, "supported_channels": {"type": "array", "items": {"type": "string"}}, "cards": {"type": "array"}}})
        }
        SchemaTarget::Card => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Card v0", "type": "object", "required": ["id", "kind", "title", "description", "entries"], "properties": {"id": {"type": "string"}, "kind": {"enum": card_kinds}, "personas": {"type": "array", "items": {"type": "string"}}, "tags": {"type": "array", "items": {"type": "string"}}, "entries": {"type": "array", "items": {"type": "object", "required": ["id", "title", "body"], "properties": {"id": {"type": "string"}, "title": {"type": "string"}, "body": {"type": "string"}, "applies_to": {"type": "array", "items": {"type": "string"}}, "evidence": {"type": "array", "items": {"type": "string"}}, "avoid": {"type": "array", "items": {"type": "string"}}}}}}})
        }
        SchemaTarget::Brief => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Brief v0", "type": "object", "required": ["contract", "pack", "inputs", "required_load_order", "decision_trace", "output_requirements"], "properties": {"contract": {"const": "mdp.brief.v0"}, "required_load_order": {"type": "array", "items": {"type": "string"}}, "decision_trace": {"type": "array"}}})
        }
        SchemaTarget::Prospect => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Prospect Input v0", "type": "object", "required": ["name", "title", "company"], "properties": {"name": {"type": "string"}, "title": {"type": "string"}, "company": {"type": "string"}, "linkedin_url": {"type": "string"}, "company_url": {"type": "string"}, "background": {"type": "string"}, "trigger": {"type": "string"}, "persona": {"type": "string"}, "segment": {"type": "string"}, "signals": {"type": "array", "items": {"type": "object", "required": ["id", "title"], "properties": {"id": {"type": "string"}, "title": {"type": "string"}, "source": {"type": "string"}, "confidence": {"type": "string"}, "freshness": {"type": "string"}, "state_as": {"type": "string"}}}}}})
        }
        SchemaTarget::Eval => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Eval Fixture v0", "type": "object", "required": ["id", "persona", "job", "expect_load_order_contains"], "properties": {"id": {"type": "string"}, "persona": {"type": "string"}, "job": {"type": "string"}, "expect_load_order_contains": {"type": "array", "items": {"type": "string"}}}})
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prospect_schema_keeps_required_skill_input_fields() {
        let result = schema(SchemaTarget::Prospect);
        let required = result["required"]
            .as_array()
            .expect("schema required field should be an array");

        assert!(required.iter().any(|field| field == "name"));
        assert!(required.iter().any(|field| field == "title"));
        assert!(required.iter().any(|field| field == "company"));
    }
}
