use crate::cli::SchemaTarget;
use crate::constants::{
    FORMAT_VERSION, PROMPT_CARD_PATCH_SCHEMA_REF, PROMPT_FORMAT_VERSION, PROMPT_OUTPUT_CONTRACT,
    PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF, RUNNER_AUDIT_CONTRACT,
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
                                "scope": scope_map_schema(),
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
        SchemaTarget::ProofOutput => proof_output_schema(),
        SchemaTarget::ProofOutputDraft => proof_output_draft_schema(),
        SchemaTarget::RunReceipt => run_receipt_schema(),
        SchemaTarget::RunnerAudit => runner_audit_schema(),
        SchemaTarget::Brief => brief_schema(),
        SchemaTarget::HumanBrief => human_brief_schema(),
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
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "title": "MDP Eval Fixture v0",
                "type": "object",
                "required": ["id", "command"],
                "properties": {
                    "id": {"type": "string"},
                    "command": {"enum": ["route", "fit", "brief", "gaps", "check-claims", "validate-prompt-output", "verify-output"]},
                    "profile_eval": profile_eval_fixture_schema(),
                    "persona": {"type": "string"},
                    "job": {"type": "string"},
                    "scope": string_array(),
                    "channel": {"type": "string"},
                    "prospect": {"type": "object"},
                    "prompt": {"type": "string"},
                    "prompt_id": {"type": "string"},
                    "prompt_output": {"type": "object"},
                    "proof_output": proof_output_schema(),
                    "proof_output_file": {"type": "string"},
                    "text": {"type": "string"},
                    "subject": {"type": "string"},
                    "expect_load_order_contains": string_array(),
                    "expect_load_order_excludes": string_array(),
                    "expect_entry_titles_contains": string_array(),
                    "expect_entry_titles_excludes": string_array(),
                    "expect_status": {"type": "string"},
                    "expect_decision": {"type": "string"},
                    "expect_draft_status": {"type": "string"},
                    "expect_valid": {"type": "boolean"},
                    "expect_normalization_ready": {"type": "boolean"},
                    "expect_issue_codes_contains": string_array(),
                    "expect_scope_issue_codes_contains": string_array(),
                    "expect_entry_gap_reasons_contains": string_array(),
                    "expect_gap_titles_contains": string_array(),
                    "expect_guardrail_terms_contains": string_array(),
                    "expect_unsupported_claims_contains": string_array()
                }
            })
        }
        SchemaTarget::Skills => skills_schema(),
    }
}

fn run_receipt_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Run Receipt v0",
        "type": "object",
        "required": ["contract", "valid", "decision", "workflow", "boundary", "runner", "prompt", "artifacts", "issues"],
        "additionalProperties": true,
        "properties": {
            "contract": {"const": "mdp.run-receipt.v0"},
            "valid": {"type": "boolean", "description": "True only when the receipt is audit-grade."},
            "decision": {"enum": ["audit-grade", "advisory", "blocked"]},
            "workflow": {"enum": ["proposal-review", "gtm-prospect", "pack-build", "custom"]},
            "pack": {
                "type": "object",
                "required": ["dir", "manifest"],
                "additionalProperties": true,
                "properties": {
                    "dir": {"type": "string"},
                    "manifest": {"type": "string"},
                    "id": {"type": "string"},
                    "name": {"type": "string"},
                    "version": {"type": "string"},
                    "profile_id": {"type": "string"}
                }
            },
            "boundary": {
                "type": "object",
                "required": ["isolation", "conversation_context_used", "declared_inputs_only"],
                "additionalProperties": true,
                "properties": {
                    "isolation": {"enum": ["isolated", "ambient", "unknown"]},
                    "conversation_context_used": {"type": ["boolean", "null"]},
                    "declared_inputs_only": {"type": "boolean"}
                }
            },
            "runner": {
                "type": "object",
                "required": ["runner_audit_required", "assurance"],
                "additionalProperties": true,
                "properties": {
                    "runner_audit": {"type": ["string", "null"]},
                    "runner_audit_required": {"type": "boolean"},
                    "assurance": {"enum": ["headless-verified", "stateless-api-verified", "asserted", "missing", "invalid"]},
                    "summary": {"type": "object"}
                }
            },
            "prompt": {
                "type": "object",
                "required": ["source_audit_required"],
                "additionalProperties": true,
                "properties": {
                    "id": {"type": ["string", "null"]},
                    "prompt_output": {"type": ["string", "null"]},
                    "validation": {"type": ["string", "null"]},
                    "source_audit": {"type": ["string", "null"]},
                    "source_audit_required": {"type": "boolean"}
                }
            },
            "artifacts": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["kind", "path", "required", "exists", "bytes", "sha256"],
                    "additionalProperties": false,
                    "properties": {
                        "kind": {"type": "string"},
                        "path": {"type": "string"},
                        "required": {"type": "boolean"},
                        "exists": {"type": "boolean"},
                        "bytes": {"type": ["integer", "null"], "minimum": 0},
                        "sha256": {"type": ["string", "null"]}
                    }
                }
            },
            "issues": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["code", "severity", "path", "message"],
                    "additionalProperties": false,
                    "properties": {
                        "code": {"type": "string"},
                        "severity": {"enum": ["error", "warning"]},
                        "path": {"type": "string"},
                        "message": {"type": "string"}
                    }
                }
            },
            "error_count": {"type": "integer", "minimum": 0},
            "warning_count": {"type": "integer", "minimum": 0}
        }
    })
}

fn runner_audit_schema() -> Value {
    let mut properties = serde_json::Map::new();
    properties.insert(
        "contract".to_string(),
        json!({"const": RUNNER_AUDIT_CONTRACT}),
    );
    properties.insert(
        "runner".to_string(),
        json!({"enum": ["native-api", "codex-exec", "claude-print", "cursor-print", "opencode-run", "custom-headless"]}),
    );
    properties.insert("model".to_string(), json!({"type": ["string", "null"]}));
    properties.insert(
        "isolated_invocation".to_string(),
        json!({"type": "boolean"}),
    );
    properties.insert(
        "conversation_resume".to_string(),
        json!({"type": "boolean"}),
    );
    properties.insert(
        "declared_inputs_only".to_string(),
        json!({"type": "boolean"}),
    );
    properties.insert("output_schema_used".to_string(), json!({"type": "boolean"}));
    properties.insert(
        "prompt_input_audited".to_string(),
        json!({"type": "boolean"}),
    );
    properties.insert(
        "session_persistence".to_string(),
        json!({"type": "boolean"}),
    );
    properties.insert(
        "config_discovery_disabled".to_string(),
        json!({"type": "boolean"}),
    );
    properties.insert(
        "instructions_discovery_disabled".to_string(),
        json!({"type": "boolean"}),
    );
    properties.insert("tools_disabled".to_string(), json!({"type": "boolean"}));
    properties.insert(
        "tool_invocations_observed".to_string(),
        json!({"type": "integer", "minimum": 0}),
    );
    properties.insert("full_tool_access".to_string(), json!({"type": "boolean"}));
    properties.insert("force_enabled".to_string(), json!({"type": "boolean"}));
    properties.insert("pure".to_string(), json!({"type": "boolean"}));
    properties.insert(
        "default_plugins_disabled".to_string(),
        json!({"type": "boolean"}),
    );
    properties.insert(
        "claude_code_discovery_disabled".to_string(),
        json!({"type": "boolean"}),
    );
    properties.insert(
        "project_rules_discovery_disabled".to_string(),
        json!({"type": "boolean"}),
    );
    properties.insert("sterile_workdir".to_string(), json!({"type": "boolean"}));
    properties.insert("ephemeral".to_string(), json!({"type": "boolean"}));
    properties.insert("bare".to_string(), json!({"type": "boolean"}));
    properties.insert(
        "sandbox".to_string(),
        json!({"enum": ["read-only", "workspace-write", "danger-full-access", "unknown"]}),
    );
    properties.insert("stateless_request".to_string(), json!({"type": "boolean"}));
    properties.insert(
        "prior_messages_included".to_string(),
        json!({"type": "boolean"}),
    );
    properties.insert("endpoint".to_string(), json!({"type": "string"}));
    properties.insert("store".to_string(), json!({"type": "boolean"}));
    properties.insert("prompt_id".to_string(), json!({"type": "string"}));
    properties.insert(
        "prompt_output_sha256".to_string(),
        json!({"type": "string"}),
    );
    properties.insert("request_sha256".to_string(), json!({"type": "string"}));
    properties.insert(
        "response_id".to_string(),
        json!({"type": ["string", "null"]}),
    );
    properties.insert("mock_response".to_string(), json!({"type": "boolean"}));
    properties.insert("demo_fixture".to_string(), json!({"type": "boolean"}));
    properties.insert("fixture".to_string(), json!({"type": "boolean"}));
    properties.insert(
        "notes".to_string(),
        json!({"type": "array", "items": {"type": "string"}}),
    );

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Runner Audit v0",
        "type": "object",
        "required": [
            "contract",
            "runner",
            "isolated_invocation",
            "conversation_resume",
            "declared_inputs_only",
            "output_schema_used",
            "prompt_id",
            "prompt_output_sha256",
            "tool_invocations_observed"
        ],
        "additionalProperties": true,
        "properties": properties
    })
}

fn proof_output_draft_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Proof Output Draft v0",
        "type": "object",
        "required": ["contract", "output", "segments"],
        "additionalProperties": false,
        "properties": {
            "contract": {"const": "mdp.proof-output-draft.v0"},
            "route": {
                "type": "object",
                "required": ["persona", "job"],
                "additionalProperties": false,
                "properties": {
                    "persona": non_blank_string_schema(),
                    "job": non_blank_string_schema()
                }
            },
            "output": {
                "type": "object",
                "required": ["kind", "format"],
                "additionalProperties": false,
                "properties": {
                    "kind": {"type": "string"},
                    "format": {"type": "string"}
                }
            },
            "coverage": {
                "type": "object",
                "required": ["mode", "material_policy"],
                "additionalProperties": false,
                "properties": {
                    "mode": {"const": "full-segmentation"},
                    "material_policy": {"const": "bound-or-gap"}
                },
                "description": "Optional. author-proof-output defaults this to full-segmentation / bound-or-gap when omitted."
            },
            "segments": proof_segments_schema()
        }
    })
}

fn proof_output_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Proof Output v0",
        "type": "object",
        "required": ["contract", "pack", "output", "coverage", "segments"],
        "additionalProperties": false,
        "properties": {
            "contract": {"const": "mdp.proof-output.v0"},
            "pack": {
                "type": "object",
                "required": ["id"],
                "additionalProperties": false,
                "properties": {
                    "id": {"type": "string"},
                    "profile_id": {"type": "string"},
                    "pack_hash": {"type": "string"}
                }
            },
            "route": {
                "type": "object",
                "required": ["persona", "job"],
                "additionalProperties": false,
                "properties": {
                    "persona": non_blank_string_schema(),
                    "job": non_blank_string_schema()
                }
            },
            "output": {
                "type": "object",
                "required": ["kind", "format", "text"],
                "additionalProperties": false,
                "properties": {
                    "kind": {"type": "string"},
                    "format": {"type": "string"},
                    "text": {"type": "string"}
                }
            },
            "coverage": {
                "type": "object",
                "required": ["mode", "material_policy"],
                "additionalProperties": false,
                "properties": {
                    "mode": {"const": "full-segmentation"},
                    "material_policy": {"const": "bound-or-gap"}
                }
            },
            "segments": proof_segments_schema()
        }
    })
}

fn proof_segments_schema() -> Value {
    json!({
        "type": "array",
        "minItems": 1,
        "items": {
            "type": "object",
            "required": ["id", "kind", "text"],
            "additionalProperties": false,
            "properties": {
                "id": {"type": "string"},
                "kind": {"enum": ["claim", "requirement_status", "template_text", "gap", "connective", "formatting"]},
                "text": {"type": "string"},
                "material": {"type": "boolean", "description": "Set false for connective or formatting-only text that carries no proof binding."},
                "gap": {
                    "type": "object",
                    "required": ["code", "reason"],
                    "additionalProperties": false,
                    "properties": {
                        "code": {"type": "string"},
                        "reason": {"type": "string"}
                    }
                },
                "refs": {
                    "type": "array",
                    "items": {
                        "oneOf": [
                            proof_card_entry_ref_schema(),
                            proof_source_ref_schema(),
                            proof_prompt_input_ref_schema(),
                            proof_input_contract_ref_schema(),
                            proof_route_ref_schema()
                        ]
                    }
                }
            }
        }
    })
}

fn proof_card_entry_ref_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "role", "card_id", "entry_id"],
        "additionalProperties": false,
        "properties": {
            "type": {"const": "card_entry"},
            "role": proof_ref_role_schema(),
            "card_id": {"type": "string"},
            "entry_id": {"type": "string"},
            "kind": {"type": "string"},
            "primitive": {"type": "string"}
        }
    })
}

fn proof_source_ref_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "role", "source_id"],
        "additionalProperties": false,
        "properties": {
            "type": {"const": "source"},
            "role": proof_ref_role_schema(),
            "source_id": {"type": "string"}
        }
    })
}

fn proof_prompt_input_ref_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "role", "prompt_id", "input_name"],
        "additionalProperties": false,
        "properties": {
            "type": {"const": "prompt_input"},
            "role": proof_ref_role_schema(),
            "prompt_id": {"type": "string"},
            "input_name": {"type": "string"}
        }
    })
}

fn proof_input_contract_ref_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "role", "input_contract_id"],
        "additionalProperties": false,
        "properties": {
            "type": {"const": "input_contract"},
            "role": proof_ref_role_schema(),
            "input_contract_id": {"type": "string"}
        }
    })
}

fn proof_route_ref_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "role", "persona", "job"],
        "additionalProperties": false,
        "properties": {
            "type": {"const": "route"},
            "role": proof_ref_role_schema(),
            "persona": non_blank_string_schema(),
            "job": non_blank_string_schema()
        }
    })
}

fn proof_ref_role_schema() -> Value {
    json!({"enum": ["supports", "constrains", "renders", "requires", "supports-gap"]})
}

fn non_blank_string_schema() -> Value {
    json!({"type": "string", "pattern": "\\S"})
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
            "target": target_identity_schema(),
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
            "qualification_gates": qualification_gates_schema(),
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

fn target_identity_schema() -> Value {
    json!({
        "type": "object",
        "description": "Optional sold-target identity and contamination lexicon for target-aware authoring.",
        "required": ["kind", "name"],
        "additionalProperties": false,
        "properties": {
            "kind": {"enum": ["company", "product", "project"]},
            "name": {"type": "string", "minLength": 1},
            "aliases": string_array(),
            "external_terms": string_array(),
            "excluded_terms": string_array(),
            "internal_terms": string_array(),
            "source_ids": string_array()
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
            "required": ["id", "skill_id", "required_primitives"],
            "additionalProperties": false,
            "oneOf": canonical_job_skill_pairs("id"),
            "properties": {
                "id": {"type": "string"},
                "skill_id": canonical_skill_id_schema(),
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
        "additionalProperties": false,
        "properties": {
            "id": {"type": "string"},
            "label": {"type": "string"},
            "version": {"const": "mdp.profile.v0"},
            "context_dimensions": scope_map_schema(),
            "context_dimension_dependencies": scope_map_schema()
        }
    })
}

fn skills_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Skills v1",
        "type": "object",
        "required": ["contract", "status", "valid", "pack", "profile", "packaged_skill_ids", "host_discovery", "eligibility", "requested_job", "recommendation", "job_routes", "diagnostics"],
        "additionalProperties": false,
        "properties": {
            "contract": {"const": "mdp.skills.v1"},
            "status": {"enum": ["bootstrap", "ready", "unresolved"]},
            "valid": {"type": "boolean"},
            "pack": {"type": "object"},
            "profile": {"type": "object"},
            "packaged_skill_ids": canonical_skill_id_array_schema(),
            "host_discovery": {
                "type": "object",
                "required": ["status", "managed_by", "guidance"],
                "additionalProperties": false,
                "properties": {
                    "status": {"const": "unobserved"},
                    "managed_by": {"const": "agent-host"},
                    "guidance": {"type": "string"}
                }
            },
            "eligibility": {
                "type": "object",
                "required": ["eligible_skill_ids", "ineligible_skills"],
                "additionalProperties": false,
                "properties": {
                    "eligible_skill_ids": canonical_skill_id_array_schema(),
                    "ineligible_skills": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["skill_id", "reason"],
                            "additionalProperties": false,
                            "properties": {
                                "skill_id": canonical_skill_id_schema(),
                                "reason": {"type": "string"}
                            }
                        }
                    }
                }
            },
            "requested_job": {"type": ["string", "null"]},
            "recommendation": {"oneOf": [{"type": "null"}, job_route_schema()]},
            "job_routes": {"type": "array", "items": job_route_schema()},
            "diagnostics": {"type": "array", "items": {"type": "object"}}
        }
    })
}

fn job_route_schema() -> Value {
    json!({
        "type": "object",
        "required": ["job_id", "skill_id", "pack_ready", "missing_primitives", "required_input_contracts"],
        "additionalProperties": false,
        "oneOf": canonical_job_skill_pairs("job_id"),
        "properties": {
            "job_id": {"type": "string"},
            "skill_id": canonical_skill_id_schema(),
            "pack_ready": {"type": "boolean"},
            "missing_primitives": string_array(),
            "required_input_contracts": string_array()
        }
    })
}

fn canonical_job_skill_pairs(job_field: &str) -> Vec<Value> {
    [
        ("prospect-fit-or-brief", "mdp-gtm-brief"),
        ("outbound-copy-brief", "mdp-gtm-brief"),
        ("outbound-copy-review", "mdp-gtm-brief"),
        ("bid-no-bid-review", "mdp-proposal-review"),
        ("compliance-review", "mdp-proposal-review"),
        ("proof-review", "mdp-proposal-review"),
        ("red-team-review", "mdp-proposal-review"),
    ]
    .into_iter()
    .map(|(job_id, skill_id)| {
        json!({
            "properties": {
                (job_field): {"const": job_id},
                "skill_id": {"const": skill_id}
            },
            "required": [job_field, "skill_id"]
        })
    })
    .collect()
}

fn canonical_skill_id_schema() -> Value {
    json!({"enum": ["mdp", "mdp-pack-builder", "mdp-pack-review", "mdp-gtm-brief", "mdp-proposal-review"]})
}

fn canonical_skill_id_array_schema() -> Value {
    json!({"type": "array", "items": canonical_skill_id_schema(), "uniqueItems": true})
}

fn brief_schema() -> Value {
    json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "MDP Brief Contracts v0", "oneOf": [
        {"type": "object", "required": ["contract", "pack", "runtime_context", "inputs", "scope", "portfolio_sensitive", "draft_status", "required_load_order", "context", "decision_trace", "output_requirements"], "properties": {"contract": {"const": "mdp.brief.v0"}, "pack": pack_schema(), "runtime_context": runtime_context_schema(), "inputs": {"type": "object", "required": ["persona", "job"], "properties": {"persona": {"type": "string"}, "motion": {"type": ["string", "null"]}, "job": {"type": "string"}}}, "scope": scope_resolution_schema(), "portfolio_sensitive": {"type": "boolean"}, "draft_status": {"enum": ["ready", "blocked"]}, "required_load_order": string_array(), "context": context_schema(), "decision_trace": {"type": "array"}, "output_requirements": {"type": "object"}}},
        {"type": "object", "required": ["contract", "pack", "runtime_context", "channel", "prospect", "prospect_source", "persona", "scope", "portfolio_sensitive", "fit", "draft_status", "job", "required_load_order", "route", "decision_trace", "agent_instruction"], "properties": {"contract": {"const": "mdp.message-brief.v0"}, "pack": pack_schema(), "runtime_context": runtime_context_schema(), "channel": {"type": "string"}, "prospect": {"type": "object"}, "prospect_source": {"type": "object", "required": ["kind", "synthetic", "guidance"], "properties": {"kind": {"type": "string"}, "synthetic": {"type": "boolean"}, "guidance": {"type": "string"}}}, "persona": {"type": "string"}, "persona_resolution": {"type": "object"}, "scope": scope_resolution_schema(), "portfolio_sensitive": {"type": "boolean"}, "fit": {"type": "object", "required": ["contract", "status", "matches", "disqualifiers"]}, "draft_status": {"enum": ["ready", "no-draft"]}, "draft_decision": {"type": "string"}, "no_draft_reason": {"type": ["string", "null"]}, "job": {"type": "string"}, "required_load_order": string_array(), "route": {"type": "array"}, "context": context_schema(), "decision_trace": {"type": "array"}, "agent_instruction": {"type": "string"}}}
    ]})
}

fn human_brief_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "MDP Human Brief v0",
        "type": "object",
        "required": ["artifact_type", "pack_id", "pack_version", "source_artifact_type", "template_id", "decision", "sections", "audit"],
        "additionalProperties": false,
        "properties": {
            "artifact_type": {"const": "mdp.human-brief.v0"},
            "pack_id": {"type": "string"},
            "pack_version": {"type": "string"},
            "source_artifact_type": {"type": "string"},
            "template_id": {"type": "string"},
            "decision": {"enum": ["ready", "needs-review", "no-draft", "proof-gap", "blocked"]},
            "title": {"type": "string"},
            "sections": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "title", "body", "refs"],
                    "additionalProperties": false,
                    "properties": {
                        "id": {"type": "string"},
                        "title": {"type": "string"},
                        "body": {"type": "string"},
                        "refs": string_array()
                    }
                }
            },
            "audit": {
                "type": "object",
                "required": ["source_artifact", "mdp_commands", "warnings"],
                "additionalProperties": false,
                "properties": {
                    "source_artifact": {"type": "string"},
                    "mdp_commands": string_array(),
                    "warnings": string_array()
                }
            },
            "artifact": {"type": "object"}
        }
    })
}

fn context_schema() -> Value {
    json!({"type": "object", "required": ["contract", "status", "runtime_context", "persona", "job", "scope", "portfolio_sensitive", "source_load_order", "gaps", "entries", "full_card_required", "summary", "policy"], "properties": {"contract": {"const": "mdp.context.v0"}, "status": {"enum": ["ready", "blocked"]}, "runtime_context": runtime_context_schema(), "reason": {"type": "string"}, "persona": {"type": "string"}, "job": {"type": "string"}, "scope": scope_resolution_schema(), "portfolio_sensitive": {"type": "boolean"}, "source_load_order": string_array(), "gaps": {"type": "array", "items": {"type": "object"}}, "entries": {"type": "array", "items": {"type": "object", "required": ["card_id", "card_kind", "card_path", "entry_id", "title", "body", "applies_to", "scope", "evidence", "avoid", "constraints", "metadata", "status", "selection", "reason"], "properties": {"card_id": {"type": "string"}, "card_kind": {"type": "string"}, "card_path": {"type": "string"}, "entry_id": {"type": "string"}, "title": {"type": "string"}, "body": {"type": "string"}, "applies_to": string_array(), "scope": scope_map_schema(), "evidence": string_array(), "avoid": string_array(), "exact_paragraphs": {"type": ["integer", "null"], "minimum": 1}, "constraints": constraints_schema(), "metadata": metadata_schema(), "status": {"enum": ["required", "supporting"]}, "selection": {"enum": ["matched", "guardrail"]}, "reason": {"type": "string"}}}}, "full_card_required": {"type": "array", "items": {"type": "object", "required": ["card_id", "card_kind", "path", "reason"], "properties": {"card_id": {"type": "string"}, "card_kind": {"type": "string"}, "path": {"type": "string"}, "reason": {"type": "string"}}}}, "summary": {"type": "object", "required": ["card_count", "entry_count", "required_entry_count", "supporting_entry_count", "guardrail_entry_count"], "properties": {"card_count": {"type": "integer"}, "entry_count": {"type": "integer"}, "required_entry_count": {"type": "integer"}, "supporting_entry_count": {"type": "integer"}, "guardrail_entry_count": {"type": "integer"}}}, "policy": {"type": "string"}}})
}

fn scope_map_schema() -> Value {
    json!({
        "type": "object",
        "propertyNames": {"pattern": "^[a-z0-9]+(?:-[a-z0-9]+)*$"},
        "additionalProperties": {
            "type": "array",
            "minItems": 1,
            "uniqueItems": true,
            "items": {"type": "string", "pattern": "^[a-z0-9]+(?:-[a-z0-9]+)*$"}
        }
    })
}

fn scope_resolution_schema() -> Value {
    json!({
        "type": "object",
        "required": ["requested", "selected", "issues"],
        "properties": {
            "requested": scope_map_schema(),
            "selected": scope_map_schema(),
            "issues": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["code", "dimension", "reason"],
                    "properties": {
                        "code": {"enum": ["scope_dimension_unknown", "scope_value_unknown", "scope_attribute_empty", "scope_attribute_type_invalid", "scope_segment_conflict", "scope_dependency_missing", "scope_dimension_missing", "scope_value_mismatch"]},
                        "dimension": {"type": "string"},
                        "value": {"type": ["string", "null"]},
                        "reason": {"type": "string"}
                    }
                }
            }
        }
    })
}

fn string_array() -> Value {
    json!({"type": "array", "items": {"type": "string"}})
}

fn missing_required_trace_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "oneOf": [
                {"type": "string"},
                {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["field", "reason"],
                    "properties": {
                        "field": {"type": "string"},
                        "path": {"type": "string"},
                        "reason": {
                            "type": "string",
                            "description": "Why the field is absent, such as not_available_in_source, not_extractable_from_source, not_extractable_without_person, or invalid_out_of_contract."
                        },
                        "source_evidence": {
                            "type": "string",
                            "description": "Short source-backed explanation of what was missing or why it could not be extracted."
                        }
                    }
                }
            ]
        }
    })
}

fn constraints_schema() -> Value {
    json!({
        "type": "object",
        "description": "Optional deterministic output constraints for generated drafts and structured proof-output artifacts. Draft-text fields are checked by check-claims; proof_output fields are checked by verify-output.",
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
            "forbid_tracking": {"type": "boolean"},
            "proof_output": proof_output_constraints_schema()
        }
    })
}

fn proof_output_constraints_schema() -> Value {
    json!({
        "type": "object",
        "description": "Pack-owned Layer 2 constraints enforced by mdp verify-output for mdp.proof-output.v0 artifacts.",
        "properties": {
            "required_segment_kinds": {
                "type": "array",
                "items": {"enum": ["claim", "requirement_status", "template_text", "gap", "connective", "formatting"]},
                "description": "Segment kinds that must be present at least once."
            },
            "min_segments": {
                "type": "object",
                "description": "Minimum segment counts by proof-output segment kind.",
                "propertyNames": {"enum": ["claim", "requirement_status", "template_text", "gap", "connective", "formatting"]},
                "additionalProperties": {"type": "integer", "minimum": 0}
            },
            "require_source_refs_for_claims": {
                "type": "boolean",
                "description": "When true, every claim segment must include at least one resolved source ref."
            },
            "max_connective_words": {
                "type": "integer",
                "minimum": 0,
                "description": "Maximum words allowed in connective or formatting segments."
            }
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

fn qualification_gates_schema() -> Value {
    json!({
        "type": "object",
        "description": "Optional pack-owned qualification gates enforced by mdp fit after prospect input readiness checks.",
        "properties": {
            "require_person_resolution": {
                "type": "boolean",
                "description": "Require public person-level resolution with name, title, and a person-scoped public URL or source-backed person-resolution signal."
            },
            "signals": {
                "type": "object",
                "description": "Source-backed signal evidence gates for qualification.",
                "properties": {
                    "min": {"type": "integer", "minimum": 1},
                    "max": {"type": "integer", "minimum": 1},
                    "require_fit_signal": {
                        "type": "boolean",
                        "description": "Require at least one source-backed signal tied to role, persona, account, ICP, category, or signal fit."
                    },
                    "require_why_now_signal": {
                        "type": "boolean",
                        "description": "Require at least one source-backed signal tied to trigger, timing, priority, change, launch, hiring, demand, or opportunity."
                    }
                }
            },
            "fail_policy": {
                "enum": ["insufficient_context"],
                "default": "insufficient_context",
                "description": "How mdp fit reports qualification gate misses. The first slice supports insufficient_context."
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
                            "description": "Explicit neutral fallback for missing input; use the trace/gaps to explain absent source data instead of inventing facts."
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
                        "description": "card-patches proposes reviewed pack entries; prospect-normalization outputs MDP prospect JSON for mdp fit/brief; proposal packs may also include normalized_opportunity as an exact alias."
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
                                "normalized_opportunity",
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
                    "inputs_used": {
                        "type": "array",
                        "description": "Exact declared prompt input names used to create this output; source locators belong in evidence/provenance fields, signals[].source, or normalization_trace.",
                        "items": {"type": "string"}
                    },
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
                                    "scope": scope_map_schema(),
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
            "normalized_opportunity": prospect_schema(),
            "normalization_trace": {
                "type": "object",
                "properties": {
                    "persona": {"type": "object"},
                    "fit_readiness": {"type": "object"},
                    "preserved_raw_fields": string_array(),
                    "missing_required": missing_required_trace_schema()
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
        "additionalProperties": false,
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
                    "additionalProperties": false,
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
        assert_eq!(result["additionalProperties"], false);
        assert_eq!(
            result["properties"]["signals"]["items"]["additionalProperties"],
            false
        );
        assert_eq!(result["properties"]["company_domain"]["type"], "string");
        assert_eq!(result["properties"]["attributes"]["maxProperties"], 25);
        assert!(result["properties"]["attributes"]["additionalProperties"].is_object());
    }

    #[test]
    fn manifest_schema_defines_required_nested_contracts() {
        let result = schema(SchemaTarget::Manifest);

        assert_eq!(result["properties"]["policy"]["type"], "object");
        assert_eq!(result["properties"]["provenance"]["type"], "object");
        assert_eq!(result["properties"]["target"]["type"], "object");
        assert_eq!(
            result["properties"]["target"]["properties"]["kind"]["enum"][0],
            "company"
        );
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
        assert_eq!(
            constraints["proof_output"]["properties"]["required_segment_kinds"]["items"]["enum"][0],
            "claim"
        );
        assert_eq!(
            constraints["proof_output"]["properties"]["min_segments"]["additionalProperties"]["type"],
            "integer"
        );
        assert_eq!(
            constraints["proof_output"]["properties"]["require_source_refs_for_claims"]["type"],
            "boolean"
        );
        assert_eq!(
            constraints["proof_output"]["properties"]["max_connective_words"]["type"],
            "integer"
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
        assert_eq!(
            result["oneOf"][1]["properties"]["runtime_context"]["properties"]["now_utc"]["format"],
            "date-time"
        );
        assert_eq!(
            result["oneOf"][1]["properties"]["context"]["properties"]["runtime_context"]["properties"]
                ["date_utc"]["format"],
            "date"
        );
        assert_eq!(
            result["oneOf"][0]["properties"]["scope"]["required"][0],
            "requested"
        );
        assert_eq!(
            result["oneOf"][1]["properties"]["portfolio_sensitive"]["type"],
            "boolean"
        );
        assert_eq!(
            result["oneOf"][1]["properties"]["scope"]["properties"]["issues"]["items"]["required"]
                [0],
            "code"
        );
    }

    #[test]
    fn human_brief_schema_exposes_renderer_contract() {
        let result = schema(SchemaTarget::HumanBrief);

        assert_eq!(result["title"], "MDP Human Brief v0");
        assert_eq!(
            result["properties"]["artifact_type"]["const"],
            "mdp.human-brief.v0"
        );
        assert_eq!(result["properties"]["decision"]["enum"][3], "proof-gap");
        assert_eq!(
            result["properties"]["sections"]["items"]["properties"]["refs"]["items"]["type"],
            "string"
        );
    }

    #[test]
    fn run_receipt_schema_exposes_context_boundary_contract() {
        let result = schema(SchemaTarget::RunReceipt);

        assert_eq!(result["title"], "MDP Run Receipt v0");
        assert_eq!(
            result["properties"]["contract"]["const"],
            "mdp.run-receipt.v0"
        );
        assert_eq!(
            result["properties"]["decision"]["enum"],
            json!(["audit-grade", "advisory", "blocked"])
        );
        assert_eq!(
            result["properties"]["boundary"]["properties"]["isolation"]["enum"],
            json!(["isolated", "ambient", "unknown"])
        );
        assert_eq!(
            result["properties"]["runner"]["properties"]["assurance"]["enum"],
            json!([
                "headless-verified",
                "stateless-api-verified",
                "asserted",
                "missing",
                "invalid"
            ])
        );
        assert_eq!(
            result["properties"]["artifacts"]["items"]["required"][5],
            "sha256"
        );
    }

    #[test]
    fn runner_audit_schema_exposes_headless_runner_contract() {
        let result = schema(SchemaTarget::RunnerAudit);

        assert_eq!(result["title"], "MDP Runner Audit v0");
        assert_eq!(
            result["properties"]["contract"]["const"],
            RUNNER_AUDIT_CONTRACT
        );
        assert_eq!(
            result["properties"]["runner"]["enum"],
            json!([
                "native-api",
                "codex-exec",
                "claude-print",
                "cursor-print",
                "opencode-run",
                "custom-headless"
            ])
        );
        assert!(
            result["required"]
                .as_array()
                .expect("required")
                .iter()
                .any(|field| field == "output_schema_used")
        );
        assert!(
            result["required"]
                .as_array()
                .expect("required")
                .iter()
                .any(|field| field == "prompt_output_sha256")
        );
        assert!(
            result["required"]
                .as_array()
                .expect("required")
                .iter()
                .any(|field| field == "tool_invocations_observed")
        );
        assert_eq!(result["properties"]["endpoint"]["type"], "string");
        assert_eq!(result["properties"]["store"]["type"], "boolean");
        assert_eq!(
            result["properties"]["prompt_output_sha256"]["type"],
            "string"
        );
        assert_eq!(result["properties"]["request_sha256"]["type"], "string");
        assert_eq!(result["properties"]["mock_response"]["type"], "boolean");
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
                .any(|field| field == "normalized_opportunity")
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

    #[test]
    fn skills_schema_exposes_only_the_greenfield_contract() {
        let result = schema(SchemaTarget::Skills);

        assert_eq!(result["title"], "MDP Skills v1");
        assert_eq!(result["properties"]["contract"]["const"], "mdp.skills.v1");
        assert_eq!(
            result["properties"]["packaged_skill_ids"]["items"]["enum"],
            json!([
                "mdp",
                "mdp-pack-builder",
                "mdp-pack-review",
                "mdp-gtm-brief",
                "mdp-proposal-review"
            ])
        );
        assert_eq!(
            result["properties"]["host_discovery"]["properties"]["status"]["const"],
            "unobserved"
        );
        assert_eq!(
            result["properties"]["job_routes"]["items"]["oneOf"]
                .as_array()
                .map(Vec::len),
            Some(7)
        );
        assert_eq!(profile_schema()["additionalProperties"], false);
    }
}
