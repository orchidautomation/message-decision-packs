use crate::cli::SchemaTarget;
use crate::constants::{FORMAT_VERSION, PROMPT_FORMAT_VERSION, PROMPT_OUTPUT_CONTRACT};
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
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Manifest v0", "type": "object", "required": ["format", "id", "name", "version", "personas", "cards", "policy", "provenance"], "properties": {"format": {"const": FORMAT_VERSION}, "id": {"type": "string"}, "name": {"type": "string"}, "version": {"type": "string"}, "description": {"type": "string"}, "personas": {"type": "array", "items": {"type": "string"}}, "target_personas": {"type": "array", "items": {"type": "string"}}, "operator_roles": {"type": "array", "items": {"type": "string"}}, "supported_channels": {"type": "array", "items": {"type": "string"}}, "persona_mappings": {"type": "array", "items": {"type": "object", "required": ["persona"], "properties": {"persona": {"type": "string"}, "title_keywords": {"type": "array", "items": {"type": "string"}}}}}, "cards": {"type": "array", "items": {"type": "object", "required": ["id", "path", "kind", "description"], "properties": {"id": {"type": "string"}, "path": {"type": "string", "pattern": "^cards/[^/].*\\.ya?ml$"}, "kind": {"enum": card_kinds}, "description": {"type": "string"}, "personas": {"type": "array", "items": {"type": "string"}}, "tags": {"type": "array", "items": {"type": "string"}}}}}, "policy": {"type": "object", "required": ["progressive_disclosure", "load_manifest_first", "max_cards_per_route", "json_contract", "no_auth_required"], "properties": {"progressive_disclosure": {"type": "boolean"}, "load_manifest_first": {"type": "boolean"}, "max_cards_per_route": {"type": "integer", "minimum": 1}, "json_contract": {"type": "string"}, "no_auth_required": {"type": "boolean"}}}, "provenance": {"type": "object", "required": ["owner", "created_by", "notes"], "properties": {"owner": {"type": "string"}, "created_by": {"type": "string"}, "notes": {"type": "array", "items": {"type": "string"}}}}}})
        }
        SchemaTarget::Card => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Card v0", "type": "object", "required": ["id", "kind", "title", "description", "entries"], "properties": {"id": {"type": "string"}, "kind": {"enum": card_kinds}, "title": {"type": "string"}, "description": {"type": "string"}, "personas": {"type": "array", "items": {"type": "string"}}, "tags": {"type": "array", "items": {"type": "string"}}, "entries": {"type": "array", "items": {"type": "object", "required": ["id", "title", "body"], "properties": {"id": {"type": "string"}, "title": {"type": "string"}, "body": {"type": "string"}, "applies_to": {"type": "array", "items": {"type": "string"}}, "evidence": {"type": "array", "items": {"type": "string"}}, "avoid": {"type": "array", "items": {"type": "string"}}}}}}})
        }
        SchemaTarget::Prompt => prompt_schema(card_kinds),
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
        {"type": "object", "required": ["contract", "pack", "channel", "prospect", "prospect_source", "persona", "fit", "draft_status", "job", "required_load_order", "route", "decision_trace", "agent_instruction"], "properties": {"contract": {"const": "mdp.message-brief.v0"}, "pack": pack_schema(), "channel": {"type": "string"}, "prospect": {"type": "object"}, "prospect_source": {"type": "object", "required": ["kind", "synthetic", "guidance"], "properties": {"kind": {"type": "string"}, "synthetic": {"type": "boolean"}, "guidance": {"type": "string"}}}, "persona": {"type": "string"}, "persona_resolution": {"type": "object"}, "fit": {"type": "object", "required": ["contract", "status", "matches", "disqualifiers"]}, "draft_status": {"enum": ["ready", "no-draft"]}, "draft_decision": {"type": "string"}, "job": {"type": "string"}, "required_load_order": string_array(), "route": {"type": "array"}, "context": context_schema(), "decision_trace": {"type": "array"}, "agent_instruction": {"type": "string"}}}
    ]})
}

fn context_schema() -> Value {
    json!({"type": "object", "required": ["contract", "status", "persona", "job", "source_load_order", "entries", "full_card_required", "summary", "policy"], "properties": {"contract": {"const": "mdp.context.v0"}, "status": {"enum": ["ready", "blocked"]}, "reason": {"type": "string"}, "persona": {"type": "string"}, "job": {"type": "string"}, "source_load_order": string_array(), "entries": {"type": "array", "items": {"type": "object", "required": ["card_id", "card_kind", "card_path", "entry_id", "title", "body", "applies_to", "evidence", "avoid", "status", "selection", "reason"], "properties": {"card_id": {"type": "string"}, "card_kind": {"type": "string"}, "card_path": {"type": "string"}, "entry_id": {"type": "string"}, "title": {"type": "string"}, "body": {"type": "string"}, "applies_to": string_array(), "evidence": string_array(), "avoid": string_array(), "status": {"enum": ["required", "supporting"]}, "selection": {"enum": ["matched", "guardrail"]}, "reason": {"type": "string"}}}}, "full_card_required": {"type": "array", "items": {"type": "object", "required": ["card_id", "card_kind", "path", "reason"], "properties": {"card_id": {"type": "string"}, "card_kind": {"type": "string"}, "path": {"type": "string"}, "reason": {"type": "string"}}}}, "summary": {"type": "object", "required": ["card_count", "entry_count", "required_entry_count", "supporting_entry_count", "guardrail_entry_count"], "properties": {"card_count": {"type": "integer"}, "entry_count": {"type": "integer"}, "required_entry_count": {"type": "integer"}, "supporting_entry_count": {"type": "integer"}, "guardrail_entry_count": {"type": "integer"}}}, "policy": {"type": "string"}}})
}

fn pack_schema() -> Value {
    json!({"type": "object", "required": ["id", "name", "version"], "properties": {"id": {"type": "string"}, "name": {"type": "string"}, "version": {"type": "string"}}})
}

fn prompt_schema(card_kinds: [&str; 14]) -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Extraction Prompt Contract v0",
        "type": "object",
        "required": [
            "format",
            "id",
            "title",
            "description",
            "target_card_kinds",
            "inputs",
            "instructions",
            "output_contract"
        ],
        "properties": {
            "format": {"const": PROMPT_FORMAT_VERSION},
            "id": {"type": "string"},
            "title": {"type": "string"},
            "description": {"type": "string"},
            "target_card_kinds": {
                "type": "array",
                "minItems": 1,
                "items": {"enum": card_kinds}
            },
            "tags": {"type": "array", "items": {"type": "string"}},
            "inputs": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["name", "description", "required", "default", "missing_behavior"],
                    "properties": {
                        "name": {"type": "string"},
                        "description": {"type": "string"},
                        "required": {"type": "boolean"},
                        "default": {
                            "type": "string",
                            "description": "Use N/A or another explicit neutral default rather than omitting missing values."
                        },
                        "missing_behavior": {
                            "type": "string",
                            "description": "How the agent should represent missing input without inventing facts."
                        }
                    }
                }
            },
            "instructions": {
                "type": "array",
                "minItems": 1,
                "items": {"type": "string"}
            },
            "output_contract": {
                "type": "object",
                "required": [
                    "contract",
                    "strict_json_only",
                    "required_top_level",
                    "entry_defaults",
                    "example"
                ],
                "properties": {
                    "contract": {"const": PROMPT_OUTPUT_CONTRACT},
                    "strict_json_only": {"const": true},
                    "required_top_level": {
                        "type": "array",
                        "items": {
                            "enum": [
                                "contract",
                                "prompt_id",
                                "source_summary",
                                "card_patches",
                                "gaps",
                                "rejected_claims"
                            ]
                        }
                    },
                    "entry_defaults": {
                        "type": "object",
                        "required": [
                            "body",
                            "applies_to",
                            "evidence",
                            "avoid",
                            "confidence",
                            "provenance"
                        ],
                        "properties": {
                            "body": {"const": "N/A"},
                            "applies_to": {"type": "array", "maxItems": 0},
                            "evidence": {"type": "array", "maxItems": 0},
                            "avoid": {"type": "array", "maxItems": 0},
                            "confidence": {"type": "string"},
                            "provenance": {"type": "array", "maxItems": 0}
                        }
                    },
                    "example": prompt_output_schema(card_kinds)
                }
            }
        }
    })
}

fn prompt_output_schema(card_kinds: [&str; 14]) -> Value {
    json!({
        "type": "object",
        "required": [
            "contract",
            "prompt_id",
            "source_summary",
            "card_patches",
            "gaps",
            "rejected_claims"
        ],
        "properties": {
            "contract": {"const": PROMPT_OUTPUT_CONTRACT},
            "prompt_id": {"type": "string"},
            "source_summary": {
                "type": "object",
                "required": ["company_domain", "company_name", "inputs_used", "confidence"],
                "properties": {
                    "company_domain": {"type": "string"},
                    "company_name": {"type": "string"},
                    "person_name": {"type": "string"},
                    "person_title": {"type": "string"},
                    "account_name": {"type": "string"},
                    "inputs_used": string_array(),
                    "confidence": {"type": "string"}
                }
            },
            "card_patches": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["card_id", "kind", "entries"],
                    "properties": {
                        "card_id": {"type": "string"},
                        "kind": {"enum": card_kinds},
                        "entries": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": [
                                    "id",
                                    "title",
                                    "body",
                                    "applies_to",
                                    "evidence",
                                    "avoid",
                                    "confidence",
                                    "provenance",
                                    "status"
                                ],
                                "properties": {
                                    "id": {"type": "string"},
                                    "title": {"type": "string"},
                                    "body": {"type": "string"},
                                    "applies_to": string_array(),
                                    "evidence": string_array(),
                                    "avoid": string_array(),
                                    "confidence": {"enum": ["high", "medium", "low", "unknown"]},
                                    "provenance": string_array(),
                                    "status": {
                                        "enum": ["candidate", "needs-review", "gap", "rejected"]
                                    },
                                    "notes": string_array()
                                }
                            }
                        }
                    }
                }
            },
            "gaps": string_array(),
            "rejected_claims": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["claim", "reason"],
                    "properties": {
                        "claim": {"type": "string"},
                        "reason": {"type": "string"},
                        "source": {"type": "string"}
                    }
                }
            }
        }
    })
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
        assert_eq!(
            result["properties"]["persona_mappings"]["items"]["properties"]["title_keywords"]["type"],
            "array"
        );
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

    #[test]
    fn prompt_schema_requires_safe_output_contract() {
        let result = schema(SchemaTarget::Prompt);

        assert_eq!(
            result["properties"]["format"]["const"],
            PROMPT_FORMAT_VERSION
        );
        assert_eq!(
            result["properties"]["output_contract"]["properties"]["strict_json_only"]["const"],
            true
        );
        assert_eq!(
            result["properties"]["output_contract"]["properties"]["example"]["required"][0],
            "contract"
        );
    }
}
