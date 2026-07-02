use crate::cli::SchemaTarget;
use crate::constants::{
    FORMAT_VERSION, PROMPT_CARD_PATCH_SCHEMA_REF, PROMPT_FORMAT_VERSION, PROMPT_OUTPUT_CONTRACT,
    PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF,
};
use crate::runtime_context::runtime_context_schema;
use serde_json::{Value, json};

pub(crate) fn schema(target: SchemaTarget) -> Value {
    let card_kinds = [
        "personas",
        "pains",
        "motions",
        "hooks",
        "avoid-rules",
        "output-rules",
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
        SchemaTarget::Manifest => manifest_schema(card_kinds),
        SchemaTarget::Card => {
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "title": "MDP Card v0",
                "type": "object",
                "required": ["id", "kind", "title", "description", "entries"],
                "properties": {
                    "id": {"type": "string"},
                    "kind": {"enum": card_kinds},
                    "title": {"type": "string"},
                    "description": {"type": "string"},
                    "personas": {"type": "array", "items": {"type": "string"}},
                    "tags": {"type": "array", "items": {"type": "string"}},
                    "entries": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["id", "title", "body"],
                            "properties": {
                                "id": {"type": "string"},
                                "title": {"type": "string"},
                                "body": {"type": "string"},
                                "applies_to": {"type": "array", "items": {"type": "string"}},
                                "evidence": {"type": "array", "items": {"type": "string"}},
                                "avoid": {"type": "array", "items": {"type": "string"}},
                                "exact_paragraphs": {"type": "integer", "minimum": 1},
                                "constraints": constraints_schema(),
                                "metadata": metadata_schema()
                            }
                        }
                    }
                }
            })
        }
        SchemaTarget::Prompt => prompt_schema(card_kinds),
        SchemaTarget::Brief => brief_schema(),
        SchemaTarget::RuntimeContext => runtime_context_schema(),
        SchemaTarget::Prospect => {
            let mut value = prospect_schema();
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    "$schema".to_string(),
                    json!("https://json-schema.org/draft/2020-12/schema"),
                );
                object.insert("title".to_string(), json!("MDP Prospect Input v0"));
            }
            value
        }
        SchemaTarget::Eval => {
            json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Eval Fixture v0", "type": "object", "required": ["id", "command"], "properties": {"id": {"type": "string"}, "command": {"enum": ["route", "fit", "brief", "gaps", "check-claims", "validate-prompt-output"]}, "profile_eval": profile_eval_fixture_schema(), "persona": {"type": "string"}, "job": {"type": "string"}, "channel": {"type": "string"}, "prospect": {"type": "object"}, "prompt_id": {"type": "string"}, "prompt_output": {"type": "object"}, "text": {"type": "string"}, "subject": {"type": "string"}, "expect_load_order_contains": string_array(), "expect_load_order_excludes": string_array(), "expect_entry_titles_contains": string_array(), "expect_entry_titles_excludes": string_array(), "expect_status": {"type": "string"}, "expect_draft_status": {"type": "string"}, "expect_valid": {"type": "boolean"}, "expect_normalization_ready": {"type": "boolean"}, "expect_issue_codes_contains": string_array()}})
        }
        SchemaTarget::AgentSurface => agent_surface_schema(),
    }
}

fn manifest_schema(card_kinds: [&str; 15]) -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Manifest v0",
        "type": "object",
        "required": ["format", "id", "name", "version", "personas", "cards", "policy", "provenance"],
        "properties": {
            "format": {"const": FORMAT_VERSION},
            "id": {"type": "string"},
            "name": {"type": "string"},
            "version": {"type": "string"},
            "description": {"type": "string"},
            "profile": profile_schema(),
            "personas": {"type": "array", "items": {"type": "string"}},
            "target_personas": {"type": "array", "items": {"type": "string"}},
            "operator_roles": {"type": "array", "items": {"type": "string"}},
            "supported_channels": {"type": "array", "items": {"type": "string"}},
            "persona_mappings": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["persona"],
                    "properties": {
                        "persona": {"type": "string"},
                        "title_keywords": {"type": "array", "items": {"type": "string"}}
                    }
                }
            },
            "lead_input_requirements": lead_input_requirements_schema(),
            "required_primitives": primitive_id_array_schema(),
            "primitive_map": primitive_map_schema(),
            "input_contracts": input_contracts_schema(),
            "jobs": profile_jobs_schema(),
            "profile_eval": profile_eval_schema(),
            "cards": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "path", "kind", "description"],
                    "properties": {
                        "id": {"type": "string"},
                        "path": {"type": "string", "pattern": "^cards/[^/].*\\.ya?ml$"},
                        "kind": {"enum": card_kinds},
                        "description": {"type": "string"},
                        "personas": {"type": "array", "items": {"type": "string"}},
                        "tags": {"type": "array", "items": {"type": "string"}}
                    }
                }
            },
            "policy": {
                "type": "object",
                "required": ["progressive_disclosure", "load_manifest_first", "max_cards_per_route", "json_contract", "no_auth_required"],
                "properties": {
                    "progressive_disclosure": {"type": "boolean"},
                    "load_manifest_first": {"type": "boolean"},
                    "max_cards_per_route": {"type": "integer", "minimum": 1},
                    "json_contract": {"type": "string"},
                    "no_auth_required": {"type": "boolean"}
                }
            },
            "provenance": {
                "type": "object",
                "required": ["owner", "created_by", "notes"],
                "properties": {
                    "owner": {"type": "string"},
                    "created_by": {"type": "string"},
                    "notes": {"type": "array", "items": {"type": "string"}}
                }
            }
        }
    })
}

fn primitive_ids() -> [&'static str; 10] {
    [
        "actors",
        "decision-criteria",
        "source-signals",
        "needs-requirements",
        "evidence-proof",
        "boundaries",
        "output-contracts",
        "routing-jobs",
        "gaps",
        "evals",
    ]
}

fn profile_eval_categories() -> [&'static str; 9] {
    [
        "proceed",
        "insufficient-context",
        "refusal",
        "unsafe-output",
        "job-routing",
        "account-context-present",
        "account-context-missing",
        "account-only-no-draft",
        "prompt-output-validation",
    ]
}

fn primitive_id_array_schema() -> Value {
    json!({
        "type": "array",
        "description": "Optional universal primitive IDs this profile must cover before activation.",
        "items": {"enum": primitive_ids()}
    })
}

fn primitive_map_schema() -> Value {
    json!({
        "type": "object",
        "description": "Manifest-level mapping from universal primitives to profile-owned cards, prompts, input contracts, jobs, and eval fixtures.",
        "propertyNames": {"enum": primitive_ids()},
        "additionalProperties": primitive_mapping_schema()
    })
}

fn primitive_mapping_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "cards": string_array(),
            "prompts": string_array(),
            "input_contracts": string_array(),
            "jobs": string_array(),
            "evals": string_array()
        }
    })
}

fn input_contracts_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": {"type": "string"},
                "description": {"type": "string"},
                "schema_ref": {"type": "string"},
                "prompt": {"type": "string", "description": "Prompt id or .mdp-relative prompt path used to normalize this profile input, when the profile has one."},
                "normalizes": string_array()
            }
        }
    })
}

fn profile_jobs_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "type": "object",
            "required": ["id", "required_primitives"],
            "properties": {
                "id": {"type": "string"},
                "label": {"type": "string"},
                "description": {"type": "string"},
                "required_primitives": primitive_id_array_schema(),
                "input_contracts": string_array()
            }
        }
    })
}

fn profile_eval_schema() -> Value {
    json!({
        "type": "object",
        "description": "Optional activation metadata for profile eval category coverage. Validation computes readiness from fixture metadata.",
        "properties": {
            "required_categories": {
                "type": "array",
                "items": {"enum": profile_eval_categories()}
            },
            "activation": {
                "type": "object",
                "properties": {
                    "status": {"enum": ["ready", "needs-review", "blocked"]},
                    "summary": {"type": "string"}
                }
            }
        }
    })
}

fn profile_eval_fixture_schema() -> Value {
    json!({
        "type": "object",
        "required": ["category"],
        "properties": {
            "category": {"enum": profile_eval_categories()},
            "primitives": primitive_id_array_schema(),
            "jobs": string_array()
        }
    })
}

fn profile_schema() -> Value {
    json!({
        "type": "object",
        "description": "Optional pack profile metadata for domain-aware agent orchestration. Existing packs remain valid without this block.",
        "required": ["id"],
        "properties": {
            "id": {"type": "string"},
            "label": {"type": "string"},
            "version": {"const": "mdp.profile.v0"},
            "agent_surface": agent_surface_properties_schema()
        }
    })
}

fn agent_surface_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Agent Surface v0",
        "type": "object",
        "required": ["contract", "pack", "profile", "agent_surface", "legacy_profile", "guidance"],
        "properties": {
            "contract": {"const": "mdp.agent-surface.v0"},
            "pack": pack_schema(),
            "profile": {
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": {"type": "string"},
                    "label": {"type": ["string", "null"]},
                    "version": {"type": ["string", "null"]}
                }
            },
            "agent_surface": agent_surface_properties_schema(),
            "legacy_profile": {"type": "boolean"},
            "guidance": string_array()
        }
    })
}

fn agent_surface_properties_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "recommended_skills": string_array(),
            "allowed_skills": string_array(),
            "blocked_skills": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["name", "reason"],
                    "properties": {
                        "name": {"type": "string"},
                        "reason": {"type": "string"}
                    }
                }
            },
            "job_skills": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["job", "skills"],
                    "properties": {
                        "job": {"type": "string"},
                        "skills": string_array()
                    }
                }
            }
        }
    })
}

fn brief_schema() -> Value {
    json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Brief Contracts v0", "oneOf": [
        {"type": "object", "required": ["contract", "pack", "runtime_context", "inputs", "required_load_order", "decision_trace", "output_requirements"], "properties": {"contract": {"const": "mdp.brief.v0"}, "pack": pack_schema(), "runtime_context": runtime_context_schema(), "inputs": {"type": "object", "required": ["persona", "job"], "properties": {"persona": {"type": "string"}, "motion": {"type": ["string", "null"]}, "job": {"type": "string"}}}, "required_load_order": string_array(), "decision_trace": {"type": "array"}, "output_requirements": {"type": "object"}}},
        {"type": "object", "required": ["contract", "pack", "runtime_context", "channel", "prospect", "prospect_source", "persona", "fit", "draft_status", "job", "required_load_order", "route", "decision_trace", "agent_instruction"], "properties": {"contract": {"const": "mdp.message-brief.v0"}, "pack": pack_schema(), "runtime_context": runtime_context_schema(), "channel": {"type": "string"}, "prospect": {"type": "object"}, "prospect_source": {"type": "object", "required": ["kind", "synthetic", "guidance"], "properties": {"kind": {"type": "string"}, "synthetic": {"type": "boolean"}, "guidance": {"type": "string"}}}, "persona": {"type": "string"}, "persona_resolution": {"type": "object"}, "fit": {"type": "object", "required": ["contract", "status", "matches", "disqualifiers"]}, "draft_status": {"enum": ["ready", "no-draft"]}, "draft_decision": {"type": "string"}, "job": {"type": "string"}, "required_load_order": string_array(), "route": {"type": "array"}, "context": context_schema(), "decision_trace": {"type": "array"}, "agent_instruction": {"type": "string"}}}
    ]})
}

fn context_schema() -> Value {
    json!({"type": "object", "required": ["contract", "status", "runtime_context", "persona", "job", "source_load_order", "entries", "full_card_required", "summary", "policy"], "properties": {"contract": {"const": "mdp.context.v0"}, "status": {"enum": ["ready", "blocked"]}, "runtime_context": runtime_context_schema(), "reason": {"type": "string"}, "persona": {"type": "string"}, "job": {"type": "string"}, "source_load_order": string_array(), "entries": {"type": "array", "items": {"type": "object", "required": ["card_id", "card_kind", "card_path", "entry_id", "title", "body", "applies_to", "evidence", "avoid", "constraints", "metadata", "status", "selection", "reason"], "properties": {"card_id": {"type": "string"}, "card_kind": {"type": "string"}, "card_path": {"type": "string"}, "entry_id": {"type": "string"}, "title": {"type": "string"}, "body": {"type": "string"}, "applies_to": string_array(), "evidence": string_array(), "avoid": string_array(), "exact_paragraphs": {"type": ["integer", "null"], "minimum": 1}, "constraints": constraints_schema(), "metadata": metadata_schema(), "status": {"enum": ["required", "supporting"]}, "selection": {"enum": ["matched", "guardrail"]}, "reason": {"type": "string"}}}}, "full_card_required": {"type": "array", "items": {"type": "object", "required": ["card_id", "card_kind", "path", "reason"], "properties": {"card_id": {"type": "string"}, "card_kind": {"type": "string"}, "path": {"type": "string"}, "reason": {"type": "string"}}}}, "summary": {"type": "object", "required": ["card_count", "entry_count", "required_entry_count", "supporting_entry_count", "guardrail_entry_count"], "properties": {"card_count": {"type": "integer"}, "entry_count": {"type": "integer"}, "required_entry_count": {"type": "integer"}, "supporting_entry_count": {"type": "integer"}, "guardrail_entry_count": {"type": "integer"}}}, "policy": {"type": "string"}}})
}

fn string_array() -> Value {
    json!({"type": "array", "items": {"type": "string"}})
}

fn constraints_schema() -> Value {
    json!({
        "type": "object",
        "description": "Optional deterministic output constraints for generated drafts. Hard min/max and forbid fields can be checked from supplied draft text when the relevant draft part is present; target ranges are advisory.",
        "properties": {
            "word_count": count_constraint_schema("Body word count limits."),
            "subject_words": count_constraint_schema("Subject line word count limits."),
            "subject_avoid": {
                "type": "array",
                "items": {"type": "string"},
                "description": "Case-insensitive subject literals to avoid, such as Re: or Fwd:."
            },
            "max_questions": {
                "type": "integer",
                "minimum": 0,
                "description": "Maximum number of question marks allowed in the supplied draft body."
            },
            "forbid_links": {"type": "boolean"},
            "forbid_attachments": {"type": "boolean"},
            "forbid_images": {"type": "boolean"},
            "forbid_html": {"type": "boolean"},
            "forbid_tracking": {"type": "boolean"}
        }
    })
}

fn count_constraint_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "description": description,
        "properties": {
            "min": {"type": "integer", "minimum": 0},
            "max": {"type": "integer", "minimum": 0},
            "target_min": {"type": "integer", "minimum": 0},
            "target_max": {"type": "integer", "minimum": 0}
        }
    })
}

fn metadata_schema() -> Value {
    json!({
        "type": "object",
        "description": "Optional advisory extension data preserved in route and brief context. The CLI surfaces metadata for agents but does not enforce unknown metadata keys.",
        "additionalProperties": true
    })
}

fn pack_schema() -> Value {
    json!({"type": "object", "required": ["id", "name", "version"], "properties": {"id": {"type": "string"}, "name": {"type": "string"}, "version": {"type": "string"}}})
}

fn lead_input_requirements_schema() -> Value {
    json!({
        "type": "object",
        "description": "Pack-owned readiness requirements checked deterministically by mdp fit.",
        "properties": {
            "required_fields": {
                "type": "array",
                "items": {
                    "enum": [
                        "name",
                        "title",
                        "company",
                        "company_domain",
                        "source_kind",
                        "synthetic",
                        "linkedin_url",
                        "company_url",
                        "background",
                        "trigger",
                        "persona",
                        "segment",
                        "signals"
                    ]
                }
            },
            "required_signal_fields": {
                "type": "array",
                "items": {
                    "enum": ["id", "title", "source", "confidence", "freshness", "state_as"]
                }
            },
            "required_attributes": {
                "type": "array",
                "items": {"type": "string", "pattern": "^[A-Za-z][A-Za-z0-9_-]{0,63}$"}
            },
            "value_contracts": {
                "type": "object",
                "description": "Optional pack-owned value domains for normalized prospect scalar fields. These contracts are enforced by validate-prompt-output and fit readiness.",
                "propertyNames": {
                    "enum": [
                        "name",
                        "title",
                        "company",
                        "company_domain",
                        "source_kind",
                        "synthetic",
                        "linkedin_url",
                        "company_url",
                        "background",
                        "trigger",
                        "persona",
                        "segment"
                    ]
                },
                "additionalProperties": value_contract_schema()
            },
            "attribute_definitions": {
                "type": "object",
                "description": "Optional pack-owned contracts for prospect attributes. Undeclared attributes remain allowed unless allow_undeclared_attributes is false.",
                "propertyNames": {"pattern": "^[A-Za-z][A-Za-z0-9_-]{0,63}$"},
                "additionalProperties": value_contract_schema()
            },
            "allow_undeclared_attributes": {
                "type": "boolean",
                "default": true,
                "description": "When false, prospect attributes must be declared in attribute_definitions."
            }
        }
    })
}

fn value_contract_schema() -> Value {
    json!({
        "type": "object",
        "description": "A deterministic value contract for a prompt or prospect field.",
        "additionalProperties": false,
        "properties": {
            "type": {"enum": ["string", "number", "integer", "boolean"]},
            "format": {
                "enum": ["date", "date-time"],
                "description": "Optional format for string values."
            },
            "enum": {
                "type": "array",
                "items": {"type": "string"},
                "description": "Allowed string values. Values are exact and pack-owned."
            },
            "required": {"type": "boolean"},
            "description": {"type": "string"}
        }
    })
}

fn prompt_schema(card_kinds: [&str; 15]) -> Value {
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
                "anyOf": [
                    {"required": ["schema_ref"]},
                    {"required": ["schema"]}
                ],
                "properties": {
                    "contract": {"const": PROMPT_OUTPUT_CONTRACT},
                    "output_kind": {
                        "enum": ["card-patches", "prospect-normalization"],
                        "description": "card-patches proposes reviewed pack entries; prospect-normalization outputs MDP prospect JSON for mdp fit/brief."
                    },
                    "strict_json_only": {"const": true},
                    "required_top_level": {
                        "type": "array",
                        "items": {
                            "enum": [
                                "contract",
                                "prompt_id",
                                "source_summary",
                                "runtime_context",
                                "normalized_prospect",
                                "normalization_trace",
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
                    "schema_ref": {
                        "enum": [
                            PROMPT_CARD_PATCH_SCHEMA_REF,
                            PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF
                        ],
                        "description": "Compact reference to the response schema family. The CLI derives the concrete schema from this ref, output_kind, prompt_id, and target_card_kinds."
                    },
                    "schema": prompt_response_schema_contract(),
                    "example": prompt_output_schema(card_kinds)
                }
            }
        }
    })
}

fn prompt_response_schema_contract() -> Value {
    json!({
        "type": "object",
        "description": "JSON Schema object for the model response. Prompt authors should narrow const, enum, required, and description fields for each prompt.",
        "required": ["type", "additionalProperties", "required", "properties"],
        "properties": {
            "$schema": {"type": "string"},
            "title": {"type": "string"},
            "type": {"const": "object"},
            "additionalProperties": {"const": false},
            "required": {"type": "array", "items": {"type": "string"}},
            "properties": {"type": "object"}
        }
    })
}

fn prompt_output_schema(card_kinds: [&str; 15]) -> Value {
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
            "runtime_context": runtime_context_schema(),
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
                                    "status",
                                    "notes"
                                ],
                                "properties": {
                                    "id": {"type": "string"},
                                    "title": {"type": "string"},
                                    "body": {"type": "string"},
                                    "applies_to": string_array(),
                                    "evidence": string_array(),
                                    "avoid": string_array(),
                                    "exact_paragraphs": {"type": "integer", "minimum": 1},
                                    "constraints": constraints_schema(),
                                    "metadata": metadata_schema(),
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
            "normalized_prospect": prospect_schema(),
            "normalization_trace": {
                "type": "object",
                "properties": {
                    "persona": {"type": "object"},
                    "fit_readiness": {"type": "object"},
                    "preserved_raw_fields": string_array(),
                    "missing_required": string_array()
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

fn prospect_schema() -> Value {
    json!({
        "type": "object",
        "required": ["name", "title", "company"],
        "properties": {
            "name": {"type": "string"},
            "title": {"type": "string"},
            "company": {"type": "string"},
            "company_domain": {
                "type": "string",
                "description": "Preferred account routing key for new lead workflows. The CLI canonicalizes supplied domains or URLs such as https://www.apple.com/ to apple.com; it does not infer a domain from company."
            },
            "source_kind": {"type": "string"},
            "synthetic": {"type": "boolean"},
            "linkedin_url": {"type": "string"},
            "company_url": {"type": "string"},
            "background": {"type": "string"},
            "trigger": {"type": "string"},
            "persona": {"type": "string"},
            "segment": {"type": "string"},
            "signals": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "title"],
                    "properties": {
                        "id": {"type": "string"},
                        "title": {"type": "string"},
                        "source": {"type": "string"},
                        "confidence": {"type": "string"},
                        "freshness": {"type": "string"},
                        "state_as": {"type": "string"}
                    }
                }
            },
            "attributes": attribute_schema()
        }
    })
}

fn attribute_schema() -> Value {
    json!({
        "type": "object",
        "maxProperties": 25,
        "description": "Bounded reviewed metadata for pack-specific routing, such as fiscal_year or segment tier. Use signals with source fields for evidence instead of dumping raw source data here.",
        "propertyNames": {"pattern": "^[A-Za-z][A-Za-z0-9_-]{0,63}$"},
        "additionalProperties": {
            "type": ["string", "number", "integer", "boolean"]
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
        assert!(!required.iter().any(|field| field == "company_domain"));
        assert_eq!(result["properties"]["company_domain"]["type"], "string");
        assert_eq!(result["properties"]["attributes"]["maxProperties"], 25);
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
        assert_eq!(
            result["properties"]["lead_input_requirements"]["properties"]["required_fields"]["items"]
                ["enum"][3],
            "company_domain"
        );
        assert_eq!(
            result["properties"]["lead_input_requirements"]["properties"]["value_contracts"]["additionalProperties"]
                ["additionalProperties"],
            false
        );
        assert_eq!(
            result["properties"]["required_primitives"]["items"]["enum"][0],
            "actors"
        );
        assert_eq!(
            result["properties"]["primitive_map"]["propertyNames"]["enum"][9],
            "evals"
        );
        assert_eq!(
            result["properties"]["input_contracts"]["items"]["properties"]["prompt"]["type"],
            "string"
        );
        assert_eq!(
            result["properties"]["jobs"]["items"]["properties"]["required_primitives"]["items"]["enum"]
                [1],
            "decision-criteria"
        );
        assert_eq!(
            result["properties"]["profile_eval"]["properties"]["required_categories"]["items"]["enum"]
                [0],
            "proceed"
        );
    }

    #[test]
    fn card_schema_exposes_structured_entry_constraints() {
        let result = schema(SchemaTarget::Card);
        let constraints =
            &result["properties"]["entries"]["items"]["properties"]["constraints"]["properties"];

        assert_eq!(
            constraints["word_count"]["properties"]["min"]["type"],
            "integer"
        );
        assert_eq!(
            constraints["subject_words"]["properties"]["max"]["type"],
            "integer"
        );
        assert_eq!(constraints["subject_avoid"]["type"], "array");
        assert_eq!(constraints["max_questions"]["type"], "integer");
        assert_eq!(constraints["forbid_links"]["type"], "boolean");
        assert_eq!(constraints["forbid_tracking"]["type"], "boolean");
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
        assert_eq!(
            result["oneOf"][1]["properties"]["runtime_context"]["properties"]["now_utc"]["format"],
            "date-time"
        );
        assert_eq!(
            result["oneOf"][1]["properties"]["context"]["properties"]["runtime_context"]["properties"]
                ["date_utc"]["format"],
            "date"
        );
    }

    #[test]
    fn runtime_context_schema_is_machine_readable() {
        let result = schema(SchemaTarget::RuntimeContext);

        assert_eq!(
            result["properties"]["contract"]["const"],
            "mdp.runtime-context.v0"
        );
        assert_eq!(result["properties"]["now_utc"]["format"], "date-time");
        assert_eq!(result["properties"]["date_utc"]["format"], "date");
        assert_eq!(result["properties"]["timezone"]["const"], "UTC");
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
        let contract_required = result["properties"]["output_contract"]["required"]
            .as_array()
            .expect("output_contract required should be an array");
        assert!(!contract_required.iter().any(|field| field == "schema"));
        assert_eq!(
            result["properties"]["output_contract"]["anyOf"][0]["required"][0],
            "schema_ref"
        );
        assert_eq!(
            result["properties"]["output_contract"]["properties"]["schema_ref"]["enum"][0],
            PROMPT_CARD_PATCH_SCHEMA_REF
        );
        assert_eq!(
            result["properties"]["output_contract"]["properties"]["schema"]["properties"]["additionalProperties"]
                ["const"],
            false
        );
        let required_fields = result["properties"]["output_contract"]["properties"]
            ["required_top_level"]["items"]["enum"]
            .as_array()
            .expect("required_top_level enum should be an array");
        assert!(
            required_fields
                .iter()
                .any(|field| field == "normalized_prospect")
        );
        assert!(
            required_fields
                .iter()
                .any(|field| field == "normalization_trace")
        );
        assert!(
            required_fields
                .iter()
                .any(|field| field == "runtime_context")
        );
        assert_eq!(
            result["properties"]["output_contract"]["properties"]["output_kind"]["enum"][1],
            "prospect-normalization"
        );
    }
}
