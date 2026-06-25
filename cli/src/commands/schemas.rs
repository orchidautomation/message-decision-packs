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
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Manifest v0", "type": "object", "required": ["format", "id", "name", "version", "personas", "cards", "policy", "provenance"], "properties": {"format": {"const": FORMAT_VERSION}, "id": {"type": "string"}, "name": {"type": "string"}, "version": {"type": "string"}, "description": {"type": "string"}, "personas": {"type": "array", "items": {"type": "string"}}, "target_personas": {"type": "array", "items": {"type": "string"}}, "operator_roles": {"type": "array", "items": {"type": "string"}}, "supported_channels": {"type": "array", "items": {"type": "string"}}, "cards": {"type": "array", "items": {"type": "object", "required": ["id", "path", "kind", "description"], "properties": {"id": {"type": "string"}, "path": {"type": "string", "pattern": "^cards/[^/].*\\.ya?ml$"}, "kind": {"enum": card_kinds}, "description": {"type": "string"}, "personas": {"type": "array", "items": {"type": "string"}}, "tags": {"type": "array", "items": {"type": "string"}}}}}, "policy": {"type": "object", "required": ["progressive_disclosure", "load_manifest_first", "max_cards_per_route", "json_contract", "no_auth_required"], "properties": {"progressive_disclosure": {"type": "boolean"}, "load_manifest_first": {"type": "boolean"}, "max_cards_per_route": {"type": "integer", "minimum": 1}, "json_contract": {"type": "string"}, "no_auth_required": {"type": "boolean"}}}, "provenance": {"type": "object", "required": ["owner", "created_by", "notes"], "properties": {"owner": {"type": "string"}, "created_by": {"type": "string"}, "notes": {"type": "array", "items": {"type": "string"}}}}}})
        }
        SchemaTarget::Card => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Card v0", "type": "object", "required": ["id", "kind", "title", "description", "entries"], "properties": {"id": {"type": "string"}, "kind": {"enum": card_kinds}, "title": {"type": "string"}, "description": {"type": "string"}, "personas": {"type": "array", "items": {"type": "string"}}, "tags": {"type": "array", "items": {"type": "string"}}, "entries": {"type": "array", "items": {"type": "object", "required": ["id", "title", "body"], "properties": {"id": {"type": "string"}, "title": {"type": "string"}, "body": {"type": "string"}, "applies_to": {"type": "array", "items": {"type": "string"}}, "evidence": {"type": "array", "items": {"type": "string"}}, "avoid": {"type": "array", "items": {"type": "string"}}}}}}})
        }
        SchemaTarget::Brief => brief_schema(),
        SchemaTarget::Prospect => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Prospect Input v0", "type": "object", "required": ["name", "title", "company"], "properties": {"name": {"type": "string"}, "title": {"type": "string"}, "company": {"type": "string"}, "source_kind": {"type": "string", "description": "Optional provenance marker such as synthetic-example, sanitized-example, user-provided-row, or private-scratch-row."}, "synthetic": {"type": "boolean", "description": "True only for generated or fictional fixtures that must not be mistaken for real prospects."}, "linkedin_url": {"type": "string"}, "company_url": {"type": "string"}, "background": {"type": "string"}, "trigger": {"type": "string"}, "persona": {"type": "string"}, "segment": {"type": "string"}, "signals": {"type": "array", "items": {"type": "object", "required": ["id", "title"], "properties": {"id": {"type": "string"}, "title": {"type": "string"}, "source": {"type": "string"}, "confidence": {"type": "string"}, "freshness": {"type": "string"}, "state_as": {"type": "string"}}}}}})
        }
        SchemaTarget::Eval => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Eval Fixture v0", "type": "object", "required": ["id", "command"], "properties": {"id": {"type": "string"}, "command": {"enum": ["route", "fit", "brief", "check-claims"]}, "persona": {"type": "string"}, "job": {"type": "string"}, "channel": {"type": "string"}, "prospect": {"type": "object"}, "text": {"type": "string"}, "expect_load_order_contains": string_array(), "expect_load_order_excludes": string_array(), "expect_entry_titles_contains": string_array(), "expect_entry_titles_excludes": string_array(), "expect_status": {"type": "string"}, "expect_draft_status": {"type": "string"}, "expect_valid": {"type": "boolean"}}})
        }
    }
}

fn string_array() -> Value {
    json!({"type": "array", "items": {"type": "string"}})
}

fn brief_schema() -> Value {
    json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Brief Contracts v0", "oneOf": [
        {"type": "object", "required": ["contract", "pack", "inputs", "required_load_order", "decision_trace", "output_requirements"], "properties": {"contract": {"const": "mdp.brief.v0"}, "pack": pack_schema(), "inputs": {"type": "object", "required": ["persona", "job"], "properties": {"persona": {"type": "string"}, "motion": {"type": ["string", "null"]}, "job": {"type": "string"}}}, "required_load_order": string_array(), "decision_trace": {"type": "array"}, "output_requirements": {"type": "object"}}},
        {"type": "object", "required": ["contract", "pack", "channel", "prospect", "prospect_source", "persona", "fit", "draft_status", "job", "required_load_order", "route", "decision_trace", "agent_instruction"], "properties": {"contract": {"const": "mdp.message-brief.v0"}, "pack": pack_schema(), "channel": {"type": "string"}, "prospect": {"type": "object"}, "prospect_source": {"type": "object", "required": ["kind", "synthetic", "guidance"], "properties": {"kind": {"type": "string"}, "synthetic": {"type": "boolean"}, "guidance": {"type": "string"}}}, "persona": {"type": "string"}, "fit": {"type": "object", "required": ["contract", "status", "matches", "disqualifiers"]}, "draft_status": {"enum": ["ready", "no-draft"]}, "draft_decision": {"type": "string"}, "job": {"type": "string"}, "required_load_order": string_array(), "route": {"type": "array"}, "context": context_schema(), "decision_trace": {"type": "array"}, "agent_instruction": {"type": "string"}}}
    ]})
}

fn context_schema() -> Value {
    json!({"type": "object", "required": ["contract", "status", "persona", "job", "source_load_order", "entries", "full_card_required", "summary", "policy"], "properties": {"contract": {"const": "mdp.context.v0"}, "status": {"enum": ["ready", "blocked"]}, "reason": {"type": "string"}, "persona": {"type": "string"}, "job": {"type": "string"}, "source_load_order": string_array(), "entries": {"type": "array", "items": {"type": "object", "required": ["card_id", "card_kind", "card_path", "entry_id", "title", "body", "applies_to", "evidence", "avoid", "status", "selection", "reason"], "properties": {"card_id": {"type": "string"}, "card_kind": {"type": "string"}, "card_path": {"type": "string"}, "entry_id": {"type": "string"}, "title": {"type": "string"}, "body": {"type": "string"}, "applies_to": string_array(), "evidence": string_array(), "avoid": string_array(), "status": {"enum": ["required", "supporting"]}, "selection": {"enum": ["matched", "guardrail"]}, "reason": {"type": "string"}}}}, "full_card_required": {"type": "array", "items": {"type": "object", "required": ["card_id", "card_kind", "path", "reason"], "properties": {"card_id": {"type": "string"}, "card_kind": {"type": "string"}, "path": {"type": "string"}, "reason": {"type": "string"}}}}, "summary": {"type": "object", "required": ["card_count", "entry_count", "required_entry_count", "supporting_entry_count", "guardrail_entry_count"], "properties": {"card_count": {"type": "integer"}, "entry_count": {"type": "integer"}, "required_entry_count": {"type": "integer"}, "supporting_entry_count": {"type": "integer"}, "guardrail_entry_count": {"type": "integer"}}}, "policy": {"type": "string"}}})
}

fn pack_schema() -> Value {
    json!({"type": "object", "required": ["id", "name", "version"], "properties": {"id": {"type": "string"}, "name": {"type": "string"}, "version": {"type": "string"}}})
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

    #[test]
    fn manifest_schema_defines_required_nested_contracts() {
        let result = schema(SchemaTarget::Manifest);

        assert_eq!(result["properties"]["policy"]["type"], "object");
        assert_eq!(result["properties"]["provenance"]["type"], "object");
        assert_eq!(result["properties"]["cards"]["items"]["required"][0], "id");
    }

    #[test]
    fn brief_schema_covers_emit_and_message_brief_contracts() {
        let result = schema(SchemaTarget::Brief);
        let contracts: Vec<&str> = result["oneOf"]
            .as_array()
            .expect("oneOf array")
            .iter()
            .filter_map(|item| item["properties"]["contract"]["const"].as_str())
            .collect();

        assert!(contracts.contains(&"mdp.brief.v0"));
        assert!(contracts.contains(&"mdp.message-brief.v0"));
        assert_eq!(
            result["oneOf"][1]["properties"]["context"]["properties"]["contract"]["const"],
            "mdp.context.v0"
        );
    }
}
