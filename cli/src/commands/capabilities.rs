use crate::authority::{SUPPORTED_COMMAND_SURFACES, SUPPORTED_PROJECTION_SURFACES};
use crate::cli::Cli;
use crate::commands::decision_trace::{
    DECISION_TRACE_V1, MAX_MERMAID_BYTES, MAX_TRACE_EDGES, MAX_TRACE_LABEL_BYTES, MAX_TRACE_NODES,
    MAX_TRACE_SOURCE_BYTES,
};
use crate::commands::schemas::{conformance_schemas, model_step_resolution_schema};
use crate::commands::source_binding::source_lineage_version_matrix;
use crate::conformance::{
    BEHAVIORAL_EVALUATION_V1, CONFORMANCE_REPORT_V1, DETERMINISTIC_CONFORMANCE_V1,
    JOB_CONFORMANCE_V1, PUBLIC_CONFORMANCE_REPORT_V1,
};
use crate::constants::{
    DEFAULT_DIR, FORMAT_VERSION, NATIVE_NORMALIZE_REQUEST_CONTRACT,
    NORMALIZED_DECISION_INPUT_CONTRACT, PROMPT_CARD_PATCH_SCHEMA_REF, PROMPT_FORMAT_V1,
    PROMPT_FORMAT_VERSION, PROMPT_OUTPUT_CONTRACT, PROMPT_OUTPUT_VALIDATION_CONTRACT,
    PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF, PROPOSAL_MCP_RUN_RESULT_CONTRACT,
    PROPOSAL_READINESS_REPORT_CONTRACT, PROPOSAL_RUN_MANIFEST_CONTRACT,
    PROPOSAL_RUNNER_RESULT_CONTRACT, REQUIREMENTS_CONTRACT, REQUIREMENTS_CONTRACT_V2,
    ROUTED_CONTEXT_CONTRACT, RUN_RECEIPT_CONTRACT, RUNNER_AUDIT_CONTRACT, SOURCE_AUDIT_CONTRACT,
    SOURCE_BINDING_CONTRACT, SOURCE_BINDING_CONTRACT_V2, SOURCE_BINDING_VALIDATION_CONTRACT,
    SOURCE_INTAKE_CONTRACT,
};
use crate::model_steps::{COMPILED_MODEL_STEP_V1, MODEL_STEP_RESOLUTION_V1};
use crate::models::DecisionInputAttemptStatus;
use crate::output::{
    OUTPUT_MODE_CONFLICT_CODE, presentation_compatibility_matrix, presentation_contract,
    presentation_selectors,
};
use crate::run_contracts::{
    CANONICAL_AUTHORITY_BLOCK_V1, DRIVER_REQUEST_V1, DRIVER_REQUEST_V2, DRIVER_RESULT_V1,
    DRIVER_RESULT_V2, PROPOSAL_RUNNER_RESULT_V1, RUN_BUNDLE_V1, RUN_EXECUTION_V1, RUN_RECEIPT_V1,
    RUN_REQUEST_V1, RUN_VERIFICATION_V1, RUNNER_AUDIT_V1,
};
use crate::run_request_compiler::RUN_REQUEST_COMPILE_V1;
use clap::{Arg, ArgAction, Command, CommandFactory};
use serde_json::{Value, json};

pub(crate) fn capabilities() -> Value {
    let authority_surfaces = SUPPORTED_COMMAND_SURFACES
        .iter()
        .map(|(command, role)| json!({"command": command, "role": role}))
        .collect::<Vec<_>>();
    let authority_projections = SUPPORTED_PROJECTION_SURFACES
        .iter()
        .map(|(surface, role)| json!({"surface": surface, "role": role}))
        .collect::<Vec<_>>();
    let conformance_contracts = conformance_schemas()
        .into_iter()
        .map(|(schema_target, contract, _)| {
            json!({"contract":contract,"schema_target":schema_target})
        })
        .collect::<Vec<_>>();
    json!({
        "contract": "mdp.capabilities.v1",
        "schema_version": 1,
        "tool": "mdp",
        "format_version": FORMAT_VERSION,
        "defaults": {
            "pack_dir": DEFAULT_DIR,
            "offline_by_default": true,
            "auth_required": false,
            "init_templates": ["gtm", "proposal"]
        },
        "prepare_run": {
            "contract": RUN_REQUEST_COMPILE_V1,
            "offline": true,
            "provider_authorization": "required-at-execution",
            "execution_authority": "mdp.run",
            "forbidden_caller_fields": ["execution_id", "idempotency_key", "pack_release_id", "prompt_path", "driver_hash", "policy_hash", "model_parameter_hash"]
        },
        "global_options": [
            {"name": "--json", "description": "Emit stable machine-readable JSON"},
            {"name": "--summary", "description": "Emit a compact status summary"}
        ],
        "cli": cli_contract(),
        "command_summary_compatibility": {
            "status": "deprecated",
            "field": "commands",
            "replacement": "cli.commands",
            "note": "The legacy command summaries retain side-effect and output-contract annotations. cli.commands is the authoritative syntactic projection generated from Clap."
        },
        "route_budget_contracts": {
            "full": {
                "contract": "mdp.route-budget.v0",
                "schema_target": "route-budget",
                "authority": "complete evaluated route matrix",
                "selectors": ["--job", "--persona"]
            },
            "summary": {
                "contract": "mdp.route-budget-summary.v1",
                "schema_target": "route-budget-summary-v1",
                "authority": "bounded projection of full route-budget.v0",
                "selector_behavior": "same exact selectors; route arrays and entry bodies omitted"
            },
            "canonical_job_field": "job_id",
            "compatibility_alias": {"field": "job", "equals": "job_id", "status": "deprecated-v0"}
        },
        "prompt_contracts": {
            "prompt_format": PROMPT_FORMAT_VERSION,
            "prompt_formats": [PROMPT_FORMAT_VERSION, PROMPT_FORMAT_V1],
            "prompt_output": PROMPT_OUTPUT_CONTRACT,
            "job_owned_output_kind": "governed-artifact",
            "source_audit": SOURCE_AUDIT_CONTRACT,
            "runner_audit": RUNNER_AUDIT_CONTRACT,
            "routed_context": {"contract": ROUTED_CONTEXT_CONTRACT, "schema_target": "routed-context-v1"},
            "card_patch_schema_ref": PROMPT_CARD_PATCH_SCHEMA_REF,
            "prospect_normalization_schema_ref": PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF
        },
        "proposal_evidence_contracts": {
            "source_intake": {
                "contract": SOURCE_INTAKE_CONTRACT,
                "schema_target": "source-intake",
                "required_for": ["real client-source approval and proposal runner receipt binding"],
                "caveat": "Only a human may approve exact bytes for proposal-review; the local runner verifies and receipt-binds the ledger but never self-approves it."
            },
            "source_audit": {
                "contract": SOURCE_AUDIT_CONTRACT,
                "schema_target": "source-audit",
                "required_for": ["source-cited prompt validation", "audit-grade proposal review when source audit is required"],
                "caveat": "Citation ledger only; it does not prove source approval or privacy classification."
            },
            "native_normalize_request": {
                "contract": NATIVE_NORMALIZE_REQUEST_CONTRACT,
                "schema_target": "native-normalize-request",
                "required_for": ["native proposal normalization runner"],
                "caveat": "Request shape does not prove that a provider invocation occurred."
            },
            "prompt_output": {
                "contract": PROMPT_OUTPUT_CONTRACT,
                "schema_target": "prompt-output",
                "required_for": ["validate-prompt-output", "run-receipt prompt binding"]
            },
            "runner_audit": {
                "contract": RUNNER_AUDIT_CONTRACT,
                "schema_target": "runner-audit",
                "required_for": ["run-receipt --require-runner-audit"],
                "caveat": "Fixture/mock/demo evidence must remain non-audit-grade."
            },
            "run_receipt": {
                "contract": RUN_RECEIPT_CONTRACT,
                "schema_target": "run-receipt",
                "required_for": ["per-invocation proposal assurance decision"]
            },
            "proposal_run_manifest": {
                "contract": PROPOSAL_RUN_MANIFEST_CONTRACT,
                "schema_target": "proposal-run-manifest",
                "required_for": ["proposal workdir ownership, concurrency refusal, and terminal run state"],
                "caveat": "In-progress and blocked manifests are not audit-grade evidence."
            },
            "proposal_runner_result": {
                "contract": PROPOSAL_RUNNER_RESULT_CONTRACT,
                "schema_target": "proposal-runner-result",
                "required_for": ["local proposal runner summary"]
            },
            "proposal_runner_result_v1": {
                "contract": PROPOSAL_RUNNER_RESULT_V1,
                "schema_target": "proposal-runner-result-v1",
                "required_for": ["proposal compatibility handoff to canonical clean-run authority"]
            },
            "proposal_readiness_report": {
                "contract": PROPOSAL_READINESS_REPORT_CONTRACT,
                "schema_target": "proposal-readiness-report",
                "required_for": ["deterministic proposal readiness findings and evidence anchors"],
                "caveat": "Confidence describes evidence anchoring, not semantic truth or submission approval."
            },
            "proposal_mcp_run_result": {
                "contract": PROPOSAL_MCP_RUN_RESULT_CONTRACT,
                "schema_target": "proposal-mcp-run-result",
                "required_for": ["local stdio MCP proposal runner response"],
                "caveat": "MCP transport is not model-isolation evidence."
            }
        },
        "clean_run_contracts": {
            "run_request": {"contract": RUN_REQUEST_V1, "schema_target": "run-request-v1"},
            "run_bundle": {"contract": RUN_BUNDLE_V1, "schema_target": "run-bundle-v1"},
            "driver_request": {"contract": DRIVER_REQUEST_V1, "schema_target": "driver-request-v1"},
            "driver_result": {"contract": DRIVER_RESULT_V1, "schema_target": "driver-result-v1"},
            "model_driver_request": {"contract": DRIVER_REQUEST_V2, "schema_target": "driver-request-v2"},
            "model_driver_result": {"contract": DRIVER_RESULT_V2, "schema_target": "driver-result-v2"},
            "runner_audit": {"contract": RUNNER_AUDIT_V1, "schema_target": "runner-audit-v1"},
            "run_receipt": {"contract": RUN_RECEIPT_V1, "schema_target": "run-receipt-v1"},
            "run_verification": {"contract": RUN_VERIFICATION_V1, "schema_target": "run-verification-v1"},
            "run_execution": {"contract": RUN_EXECUTION_V1, "schema_target": "run-execution-v1"},
            "output_directory": {
                "policy": "new external directory outside the active pack",
                "generated_artifact_inside_pack": "output-directory-inside-pack",
                "cleanup": "validation and preflight never delete existing pack artifacts"
            },
            "canonical_authority_block": {"contract": CANONICAL_AUTHORITY_BLOCK_V1, "schema_target": "canonical-authority-block-v1"},
            "assurance": "Vector-valued evidence; v0 labels and driver assertions never silently elevate."
        },
        "authority_conformance": {
            "contract": "mdp.authority-conformance-corpus.v1",
            "source": "plugin/assets/authority-conformance/corpus.json",
            "authority_owner": "rust-cli",
            "authority_levels": ["unavailable", "informational", "authoritative"],
            "dispositions": ["undetermined", "allow", "block"],
            "projection_rule": "preserve-or-reduce; authoritative disposition is immutable on faithful projections",
            "governed_generation_rule": "available only for authoritative allow after every required gate obligation passes",
            "transport_rule": "well-formed decisions are data; MCP errors are transport-owned only",
            "registered_commands": authority_surfaces,
            "registered_projections": authority_projections
        },
        "model_step_contracts": {
            "resolution": MODEL_STEP_RESOLUTION_V1,
            "compiled_step": COMPILED_MODEL_STEP_V1,
            "phase_order": ["normalization", "generation", "review"],
            "step_id_format": "model:{job_id}/{phase}",
            "unbound_behavior": "unassessed; prompts are never inferred from filenames or skill prose",
            "schema": model_step_resolution_schema()
        },
        "cold_model_conformance_contracts": {
            "contracts": conformance_contracts,
            "commands": {
                "compile": {"argv": ["conformance", "compile"], "output_contract": DETERMINISTIC_CONFORMANCE_V1, "model_calls": false},
                "validate": {"argv": ["conformance", "validate"], "output_contract": BEHAVIORAL_EVALUATION_V1, "model_calls": false},
                "assemble": {"argv": ["conformance", "assemble"], "output_contract": JOB_CONFORMANCE_V1, "model_calls": false, "contained_members_required": true},
                "report": {"argv": ["conformance", "report"], "output_contracts": [CONFORMANCE_REPORT_V1, PUBLIC_CONFORMANCE_REPORT_V1], "model_calls": false, "source_authority": JOB_CONFORMANCE_V1}
            },
            "model_execution": "external-only",
            "behavioral_calls_in_validation": false,
            "candidate_expectations": "evaluator-inventory-only",
            "sampling": {"hard_boundaries":"3/3","useful_completion":"2/3"},
            "public_digest_policy": "synthetic-or-exact-hash-sanitized-public-approval",
            "limits": {
                "authority_bytes": crate::conformance::MAX_CONFORMANCE_AUTHORITY_BYTES,
                "json_depth": crate::conformance::MAX_CONFORMANCE_DEPTH,
                "array_items": crate::conformance::MAX_CONFORMANCE_ARRAY_ITEMS,
                "model_visible_inputs": crate::conformance::MAX_MODEL_VISIBLE_INPUTS,
                "candidate_authorities": crate::conformance::MAX_CANDIDATE_AUTHORITIES,
                "trials_per_job": crate::conformance::MAX_TRIALS_PER_JOB,
                "journey_links": crate::conformance::MAX_JOURNEY_LINKS
            }
        },
        "decision_trace_contract": {
            "contract": DECISION_TRACE_V1,
            "schema_target": "decision-trace-v1",
            "projection_only": true,
            "source_authority_retained": true,
            "prompt_output_authority": {
                "validation_contract": PROMPT_OUTPUT_VALIDATION_CONTRACT,
                "schema_target": "prompt-output-validation-v1",
                "raw_output_behavior": "unavailable-untrusted",
                "required_bindings": ["pack", "prompt", "job-when-unambiguous", "validator-input-bytes", "prompt-output-bytes", "validation-result-bytes"]
            },
            "limits": {
                "source_bytes": MAX_TRACE_SOURCE_BYTES,
                "nodes": MAX_TRACE_NODES,
                "edges": MAX_TRACE_EDGES,
                "label_bytes": MAX_TRACE_LABEL_BYTES,
                "mermaid_bytes": MAX_MERMAID_BYTES
            }
        },
        "profile_contracts": {
            "manifest_profile": "mdp.profile.v0",
            "skills": "mdp.skills.v1",
            "profile_metadata_optional": true,
            "context_dimensions": "Optional profile-owned applicability dimensions such as product, capability, solution, or segment; agnostic primitives remain unchanged.",
            "entry_scope": "OR within an entry dimension and AND across dimensions; unscoped entries are global."
        },
        "persona_reference_integrity": {
            "authority": "manifest.personas plus manifest.target_personas and manifest.operator_roles",
            "matching": "case-insensitive; authored display values are preserved",
            "selectors": ["manifest.cards[].personas", "loaded cards[].personas", "loaded cards[].entries[].applies_to"],
            "empty_selector_behavior": "empty lists and empty values retain universal/no-selector compatibility",
            "prose_behavior": "titles, descriptions, and bodies are not interpreted as persona references",
            "default_validation": "undeclared selectors emit path-specific warnings while preserving legacy validity",
            "strict_validation": "warnings fail mdp validate --strict"
        },
        "decision_input_contracts": {
            "requirements": REQUIREMENTS_CONTRACT,
            "requirements_contracts": [REQUIREMENTS_CONTRACT, REQUIREMENTS_CONTRACT_V2],
            "normalized_input": NORMALIZED_DECISION_INPUT_CONTRACT,
            "source_binding": SOURCE_BINDING_CONTRACT,
            "source_binding_contracts": [SOURCE_BINDING_CONTRACT, SOURCE_BINDING_CONTRACT_V2],
            "source_binding_validation": SOURCE_BINDING_VALIDATION_CONTRACT,
            "version_matrix": source_lineage_version_matrix(),
            "attempt_statuses": DecisionInputAttemptStatus::ALL,
            "requirement_classes": ["required", "optional", "conditional", "hard-gate"],
            "boundary": "The pack and CLI own questions and deterministic decisions. The customer or host owns source collection, provider access, model calls, copy generation, and sequencing.",
            "job_ingress": {
                "contract": "mdp.job-ingress.v1",
                "governed_default": "A selected job with a direct or transitive decision-input contract requires lineage-validated normalized input.",
                "detached_behavior": "blocked with governed_job_requires_normalized_input; never fit, ready, or draft authority",
                "legacy_boundary": "Detached prospect compatibility applies only to a selected job with no direct or transitive decision-input contract binding."
            }
        },
        "target_contracts": {
            "manifest_target": "Optional for existing/reference packs; required by the target-aware GTM authoring path.",
            "kinds": ["company", "product", "project"],
            "external_vs_internal": "External target terms may drive positioning. MDP, CLI, schema, prompt, card, and eval vocabulary remains internal implementation context.",
            "contamination_issue_codes": ["target_contamination_excluded_term", "target_contamination_internal_vocabulary"]
        },
        "presentation_contract": {
            "contract": "mdp.presentation.v0",
            "selectors": presentation_selectors(),
            "matrix": presentation_compatibility_matrix(),
            "policy": presentation_contract(),
            "summary_selector": {
                "selector": "--summary",
                "json_compatible": true,
                "envelope": "summary"
            },
            "conflict": {
                "code": OUTPUT_MODE_CONFLICT_CODE,
                "exit_code": 1,
                "stdout": "single_json_envelope",
                "stderr": "empty"
            },
            "display": {
                "help": {
                    "command": "help",
                    "envelope_shape": {"ok": true, "command": "help", "data": {"text": "..."}},
                    "exit_code": 0,
                    "stdout": "single_json_envelope",
                    "stderr": "empty"
                },
                "version": {
                    "command": "version",
                    "envelope_shape": {"ok": true, "command": "version", "data": {"text": "..."}},
                    "exit_code": 0,
                    "stdout": "single_json_envelope",
                    "stderr": "empty"
                }
            },
            "stdout_invariant": {
                "mode": "json",
                "selector": "--json",
                "value_count": 1,
                "prelude_allowed": false,
                "trailing_text_allowed": false,
                "stderr_empty": true
            }
        },
        "commands": [
            command("capabilities", "mdp.capabilities.v1", "read-only", false, false, false, &[]),
            nested_command("compile", DETERMINISTIC_CONFORMANCE_V1, &["--candidate", "--artifact-root"], &[], &["--out", "--dry-run"]),
            nested_command("validate", BEHAVIORAL_EVALUATION_V1, &["--artifact-root", "--candidate", "--evaluator-inventory", "--lifecycle-policy", "--deterministic", "--invocation", "--trial", "--verifier-receipt"], &["--invocation", "--trial", "--verifier-receipt", "--evaluator-result", "--publication-approval"], &["--evaluator-result", "--publication-approval", "--out", "--dry-run"]),
            nested_command("assemble", JOB_CONFORMANCE_V1, &["--candidate", "--deterministic", "--behavioral", "--artifact-root"], &["--trial"], &["--out", "--dry-run"]),
            nested_command_with_outputs("report", &[CONFORMANCE_REPORT_V1, PUBLIC_CONFORMANCE_REPORT_V1], &["--conformance", "--artifact-root", "--visibility", "--generated-at"], &[], &["--out", "--dry-run"]),
            command("init", "mdp.init.v0", "writes-files", true, false, false, &["--name", "--target-name", "--target-kind", "--target-alias", "--exclude-term", "--dir", "--template", "--force", "--include-output-schemas", "--dry-run"]),
            command("doctor", "mdp.doctor.v0", "read-only", false, false, false, &["--dir"]),
            command("skills", "mdp.skills.v1", "read-only", false, false, false, &["--dir", "--job"]),
            command("requirements", REQUIREMENTS_CONTRACT, "read-only", false, false, false, &["--dir", "--job"]),
            command("prepare-run", RUN_REQUEST_COMPILE_V1, "read-only-unless-out", false, true, false, &["--dir", "--job", "--operation", "--input", "--model", "--retention-policy", "--created-at", "--out", "--manifest-out", "--full"]),
            command("rebind-synthetic-chain", "mdp.synthetic-v2-chain.v1", "writes-external-chain", true, false, false, &["--dir", "--job", "--out-dir", "--input-dir", "--as-of", "--seed", "--dry-run", "--apply", "--force"]),
            command("validate-source-binding", SOURCE_BINDING_VALIDATION_CONTRACT, "read-only", false, false, false, &["--dir", "--job", "--file"]),
            command("validate", "mdp.validate.v0", "read-only", false, false, true, &["--dir", "--strict"]),
            command("validate-prompt-output", PROMPT_OUTPUT_VALIDATION_CONTRACT, "read-only", false, false, true, &["--dir", "--file", "--source-audit", "--source-binding", "--source-attempt-request", "--collected-attempt-results", "--invocation-receipt", "--routed-context", "--prompt", "--prompt-id", "--strict"]),
            command("run-receipt", RUN_RECEIPT_CONTRACT, "writes-files-with-out", true, true, false, &["--dir", "--workflow", "--isolation", "--declared-inputs-only", "--prompt-id", "--prompt-output", "--validation", "--source-audit", "--runner-audit", "--require-runner-audit", "--artifact", "--out", "--dry-run"]),
            command("verify-run", RUN_VERIFICATION_V1, "read-only", false, false, false, &["--bundle", "--receipt", "--artifact-root"]),
            command("trace", DECISION_TRACE_V1, "read-only-unless-out", false, true, false, &["--file", "--dir", "--prompt-output", "--validation-input", "--bundle", "--receipt", "--artifact-root", "--format", "--out"]),
            command("consume-run", "mdp.run-consumption-result.v1", "writes-local-ledger", false, false, false, &["--ledger", "--job-id", "--idempotency-key", "--receipt-sha256", "--expected-prior-version", "--permit-exact-replay"]),
            command("run", RUN_EXECUTION_V1, "writes-new-run-directory", false, true, false, &["--request", "--out-dir", "--transport-timeout-ms"]),
            command("run-preflight", "mdp.run-preflight.v1", "read-only", false, false, false, &["--request", "--transport-timeout-ms"]),
            command("verify-output", "mdp.verify-output.v0", "read-only", false, false, false, &["--dir", "--file", "--readable"]),
            command("author-proof-output", "mdp.author-proof-output.v0", "writes-files-with-out", true, true, false, &["--dir", "--draft", "--out", "--dry-run"]),
            command("render-brief", "mdp.human-brief.v0", "writes-files-with-out", false, true, true, &["--dir", "--file", "--template", "--format", "--out", "--strict"]),
            command("explain", "mdp.explain.v0", "read-only", false, false, false, &["--dir", "--persona"]),
            command("route", "mdp.route.v0", "read-only", false, false, false, &["--dir", "--persona", "--job", "--scope", "--entries", "--eval-fixture"]),
            command("route-budget", "mdp.route-budget.v0", "read-only", false, false, true, &["--dir", "--strict", "--job", "--persona", "--summary"]),
            command("sample-leads", "mdp.sample-leads.v0", "read-only", false, false, false, &["--dir", "--persona", "--job", "--count", "--seed", "--format"]),
            command("fit", "mdp.fit.v0", "read-only", false, false, false, &["--dir", "--prospect", "--normalized-input", "--prompt", "--source-binding", "--source-attempt-request", "--collected-attempt-results", "--job"]),
            command("check-claims", "mdp.claim-check.v0", "read-only", false, false, true, &["--dir", "--text", "--file", "--subject", "--persona", "--job", "--scope", "--strict"]),
            command("gaps", "mdp.gaps.v0", "read-only", false, false, false, &["--dir"]),
            command("eval", "mdp.eval.v0", "read-only", false, false, true, &["--dir", "--strict"]),
            command("brief", "mdp.message-brief.v0", "writes-files-with-out", true, true, false, &["--dir", "--prospect", "--normalized-input", "--prompt", "--source-binding", "--source-attempt-request", "--collected-attempt-results", "--channel", "--job", "--context", "--routed-context-out", "--readable", "--out", "--dry-run"]),
            command("copy", "mdp.copy-demo.v0", "writes-files-with-out", false, true, false, &["--dir", "--prospect", "--channel", "--out"]),
            command("emit-brief", "mdp.brief.v0", "writes-files-with-out", true, true, false, &["--dir", "--persona", "--motion", "--job", "--scope", "--routed-context-out", "--out", "--dry-run"]),
            command("pack", "mdp.pack.v0", "writes-files-with-out", true, true, false, &["--dir", "--out", "--dry-run"]),
            command("readme-check", "mdp.readme-inventory.v1", "read-only", false, false, true, &["--dir"]),
            command("readme-refresh", "mdp.readme-inventory.v1", "writes-files-with-out", true, true, false, &["--dir", "--out", "--dry-run"]),
            command("schema", "mdp.schema.v0", "read-only", false, false, false, &["target"])
        ],
        "stable_error_codes": [
            {"code": "pack_not_found", "meaning": "A pack manifest or required .mdp path could not be read"},
            {"code": "invalid_manifest", "meaning": "A pack manifest could not be parsed or uses invalid structure"},
            {"code": "model-step-ambiguous", "meaning": "The selected job has more than one model step; provide an exact --operation"},
            {"code": "declared-input-missing", "meaning": "A required model-step input was not provided"},
            {"code": "declared-input-authority-missing", "meaning": "A selected step/job input has no explicit schema authority"},
            {"code": "declared-input-unsafe", "meaning": "An input was not a regular single-link non-symlink file"},
            {"code": "governed-lineage-duplicate-alias", "meaning": "A governed lineage artifact was supplied through duplicate aliases"},
            {"code": "identity-observation-unavailable", "meaning": "MDP-231 runtime identity observations could not be established"},
            {"code": "invalid_prospect", "meaning": "A prospect input uses unsupported fields or invalid structure"},
            {"code": "missing_card", "meaning": "A referenced card could not be found or read"},
            {"code": "readme_marker_layout_invalid", "meaning": "Machine-owned README region markers are malformed, duplicated, nested, or unmatched"},
            {"code": "readme_inventory_drift", "meaning": "The generated README inventory block does not match loaded structured authority"},
            {"code": "unsupported_claim", "meaning": "Draft text contains unsupported claims or claim-check failures"},
            {"code": "invalid_proof_output", "meaning": "A proof-output artifact is malformed or references missing or incompatible pack IDs"},
            {"code": "invalid_human_brief", "meaning": "A human-brief source artifact is malformed or missing required gate/proof fields"},
            {"code": "insufficient_context", "meaning": "A fit or drafting path lacks enough context to proceed"},
            {"code": "route_card_cap_excluded_applicable", "meaning": "The configured route-card cap excluded an otherwise applicable card"},
            {"code": "route_budget_filter_not_found", "meaning": "An exact route-budget job or declared persona selector did not match"},
            {"code": "write_conflict", "meaning": "A write would overwrite an existing file without explicit permission"},
            {"code": "output-directory-inside-pack", "meaning": "A clean-run output directory resolves to the active pack or one of its descendants"},
            {"code": "synthetic_chain_v2_required", "meaning": "The selected job is not an available signal-aware mdp.requirements.v2 contract"},
            {"code": "synthetic_chain_real_provenance", "meaning": "Synthetic rebinding found a non-synthetic source class or provenance entry"},
            {"code": "synthetic_chain_private_provenance", "meaning": "Synthetic rebinding found a private, customer, credential, or provider provenance field"},
            {"code": "synthetic_chain_provenance_ambiguous", "meaning": "Synthetic rebinding found provenance whose locator or marker is not explicit"},
            {"code": "synthetic_chain_provenance_missing", "meaning": "Synthetic rebinding found no explicit synthetic source provenance"},
            {"code": "synthetic_chain_provenance_not_synthetic", "meaning": "Synthetic rebinding found a missing or false synthetic marker"},
            {"code": "synthetic_chain_unsafe_locator", "meaning": "Synthetic rebinding found a URL, absolute path, or unsafe locator"},
            {"code": "synthetic_chain_mixed_version", "meaning": "Synthetic rebinding found chain files from different or unsupported contract versions"},
            {"code": "synthetic_chain_job_mismatch", "meaning": "Synthetic rebinding found a chain file bound to a different job"},
            {"code": "synthetic_chain_rebind_failed", "meaning": "Synthetic rebinding could not preserve the input chain's JSON structure"},
            {"code": "synthetic_chain_recipe_unsupported", "meaning": "The selected v2 contract cannot be rendered as a safe synthetic fixture"},
            {"code": "synthetic_chain_as_of_invalid", "meaning": "The deterministic synthetic timestamp is not an exact UTC timestamp"},
            {"code": "synthetic_chain_write_conflict", "meaning": "A changed destination chain requires explicit apply and force"},
            {"code": "synthetic_chain_backup_collision", "meaning": "A recoverable digest-keyed backup path could not be allocated"},
            {"code": "invalid_argument", "meaning": "CLI arguments are missing, conflicting, or unsupported"},
            {"code": OUTPUT_MODE_CONFLICT_CODE, "meaning": "Global --json was combined with a human-only presentation selector; one JSON output_mode_conflict envelope is written to stdout and stderr is empty"},
            {"code": "mdp_error", "meaning": "Fallback for uncategorized MDP errors"}
        ],
        "boundaries": [
            "No auth, hosted API, scraping, enrichment, CRM writeback, sending, sequencing, or BI behavior.",
            "Dry-run reports local file writes only; it does not perform network calls or mutate packs.",
            "Strict mode is opt-in and preserves default compatibility."
        ]
    })
}

fn command(
    name: &str,
    output_contract: &str,
    side_effects: &str,
    dry_run: bool,
    out: bool,
    strict: bool,
    args: &[&str],
) -> Value {
    json!({
        "name": name,
        "output_contract": output_contract,
        "side_effects": side_effects,
        "supports_json": true,
        "supports_summary": true,
        "supports_out": out,
        "supports_dry_run": dry_run,
        "supports_strict": strict,
        "args": args
    })
}

fn nested_command(
    subcommand: &str,
    output_contract: &str,
    required_args: &[&str],
    repeatable_args: &[&str],
    optional_args: &[&str],
) -> Value {
    let mut args = required_args.to_vec();
    for arg in repeatable_args.iter().chain(optional_args) {
        if !args.contains(arg) {
            args.push(arg);
        }
    }
    json!({
        "name": format!("conformance {subcommand}"),
        "argv": ["conformance", subcommand],
        "output_contract": output_contract,
        "side_effects": "writes-files-with-out",
        "supports_json": true,
        "supports_summary": true,
        "supports_out": true,
        "supports_dry_run": true,
        "supports_strict": false,
        "required_args": required_args,
        "repeatable_args": repeatable_args,
        "optional_args": optional_args,
        "args": args
    })
}

fn nested_command_with_outputs(
    subcommand: &str,
    output_contracts: &[&str],
    required_args: &[&str],
    repeatable_args: &[&str],
    optional_args: &[&str],
) -> Value {
    let mut value = nested_command(
        subcommand,
        "",
        required_args,
        repeatable_args,
        optional_args,
    );
    value
        .as_object_mut()
        .expect("command metadata object")
        .remove("output_contract");
    value["output_contracts"] = json!(output_contracts);
    value
}

/// Return the authoritative syntactic CLI contract directly from Clap.
///
/// Semantic annotations such as output contracts and side-effect classes remain
/// in the compatibility `commands` array. Argument shape is reflected from
/// [`Cli`]. Clap does not expose conditional requirements through its stable
/// reflection API, so those few edges live in the closed registry below and are
/// exercised against real Clap parsing in the parity tests.
fn cli_contract() -> Value {
    let mut root = Cli::command();
    root.build();

    let root_arguments = arguments_for(&root, &[]);
    let root_groups = groups_for(&root);
    let mut commands = Vec::new();
    collect_commands(&root, &[], &mut commands);

    json!({
        "contract": "mdp.cli-graph.v1",
        "source": "clap",
        "canonical_invocation": "mdp",
        "subcommand_required": root.is_subcommand_required_set(),
        "classification": if root.is_subcommand_required_set() { "namespace" } else { "agent-callable" },
        "root_arguments": root_arguments,
        "root_argument_groups": root_groups,
        "commands": commands
    })
}

fn collect_commands(command: &Command, parent_path: &[String], output: &mut Vec<Value>) {
    for subcommand in command.get_subcommands() {
        let mut path = parent_path.to_vec();
        path.push(subcommand.get_name().to_string());
        let human_only_reason = path.iter().any(|segment| segment == "help").then_some(
            "Clap-generated help navigation; use --json with an authored command for a machine envelope",
        );
        let subcommand_required = subcommand.is_subcommand_required_set();
        output.push(json!({
            "name": subcommand.get_name(),
            "path": path,
            "argv": path.clone(),
            "aliases": subcommand.get_all_aliases().collect::<Vec<_>>(),
            "about": subcommand.get_about().map(ToString::to_string),
            "arguments": arguments_for(subcommand, &path),
            "argument_groups": groups_for(subcommand),
            "subcommand_required": subcommand_required,
            "classification": if human_only_reason.is_some() { "human-only" } else if subcommand_required { "namespace" } else { "agent-callable" },
            "human_only_reason": human_only_reason
        }));
        collect_commands(subcommand, &path, output);
    }
}

fn groups_for(command: &Command) -> Vec<Value> {
    command
        .get_groups()
        .filter_map(|group| {
            let arguments = group
                .get_args()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            (!arguments.is_empty()).then(|| {
                json!({
                    "id": group.get_id().as_str(),
                    "arguments": arguments,
                    "required": group.is_required_set()
                })
            })
        })
        .collect()
}

fn arguments_for(command: &Command, path: &[String]) -> Vec<Value> {
    command
        .get_arguments()
        .map(|argument| argument_contract(command, argument, path))
        .collect()
}

fn argument_contract(command: &Command, argument: &Arg, path: &[String]) -> Value {
    let action = action_name(argument.get_action());
    let long = argument.get_long().map(|name| format!("--{name}"));
    let short = argument.get_short().map(|name| format!("-{name}"));
    let canonical = long
        .clone()
        .or(short.clone())
        .unwrap_or_else(|| argument.get_id().to_string());
    let long_aliases = argument
        .get_all_aliases()
        .unwrap_or_default()
        .into_iter()
        .map(|alias| format!("--{alias}"))
        .collect::<Vec<_>>();
    let short_aliases = argument
        .get_all_short_aliases()
        .unwrap_or_default()
        .into_iter()
        .map(|alias| format!("-{alias}"))
        .collect::<Vec<_>>();
    let mut aliases = long_aliases;
    aliases.extend(short_aliases);
    let value_range = argument.get_num_args();
    let min_values = value_range.map(|range| range.min_values()).unwrap_or(0);
    let max_values = value_range.map(|range| range.max_values()).unwrap_or(0);
    let repeatable =
        matches!(argument.get_action(), ArgAction::Append | ArgAction::Count) || max_values > 1;
    let enum_values = argument
        .get_possible_values()
        .into_iter()
        .map(|value| {
            let names = value.get_name_and_aliases().collect::<Vec<_>>();
            json!({"name": value.get_name(), "aliases": &names[1..]})
        })
        .collect::<Vec<_>>();
    let conflicts = command
        .get_arg_conflicts_with(argument)
        .into_iter()
        .map(|conflict| {
            conflict
                .get_long()
                .map(|name| format!("--{name}"))
                .or_else(|| conflict.get_short().map(|name| format!("-{name}")))
                .unwrap_or_else(|| conflict.get_id().to_string())
        })
        .collect::<Vec<_>>();
    let human_only = matches!(
        argument.get_action(),
        ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong | ArgAction::Version
    );
    let conditional = conditional_requirements(path, argument.get_id().as_str());
    let requires = canonical_argument_ids(command, conditional.requires_when_present);
    let required_unless_present =
        canonical_argument_ids(command, conditional.required_unless_present);

    json!({
        "id": argument.get_id().as_str(),
        "canonical": canonical,
        "long": long,
        "short": short,
        "aliases": aliases,
        "kind": if argument.is_positional() { "positional" } else if argument.get_action().takes_values() { "option" } else { "flag" },
        "action": action,
        "required": argument.is_required_set(),
        "optional": !argument.is_required_set(),
        "repeatable": repeatable,
        "global": argument.is_global_set(),
        "value_arity": {"min": min_values, "max": max_values},
        "value_names": argument.get_value_names().map(|names| names.iter().map(ToString::to_string).collect::<Vec<_>>()).unwrap_or_default(),
        "default_values": argument.get_default_values().iter().map(|value| value.to_string_lossy().into_owned()).collect::<Vec<_>>(),
        "enum_values": enum_values,
        "conflicts_with": conflicts,
        "requires_when_present": requires,
        "required_unless_present": required_unless_present,
        "help": argument.get_help().map(ToString::to_string),
        "classification": if human_only { "human-only" } else { "agent-callable" },
        "human_only_reason": human_only.then_some("Clap display action; it does not execute a product command")
    })
}

#[derive(Clone, Copy, Default)]
struct ConditionalRequirements {
    requires_when_present: &'static [&'static str],
    required_unless_present: &'static [&'static str],
}

fn conditional_requirements(path: &[String], argument_id: &str) -> ConditionalRequirements {
    let path = path.iter().map(String::as_str).collect::<Vec<_>>();
    match (path.as_slice(), argument_id) {
        (["skills"], "job") => ConditionalRequirements {
            requires_when_present: &["dir"],
            ..Default::default()
        },
        (["rebind-synthetic-chain"], "force") => ConditionalRequirements {
            requires_when_present: &["apply"],
            ..Default::default()
        },
        (["trace"], "dir") => ConditionalRequirements {
            requires_when_present: &["file", "prompt_output"],
            ..Default::default()
        },
        (["trace"], "prompt_output") => ConditionalRequirements {
            requires_when_present: &["file", "dir"],
            ..Default::default()
        },
        (["trace"], "validation_inputs") => ConditionalRequirements {
            requires_when_present: &["file", "dir", "prompt_output"],
            ..Default::default()
        },
        (["trace"], "bundle") => ConditionalRequirements {
            requires_when_present: &["receipt"],
            ..Default::default()
        },
        (["trace"], "receipt") => ConditionalRequirements {
            requires_when_present: &["bundle"],
            ..Default::default()
        },
        (["fit"], "prospect") | (["brief"], "prospect") => ConditionalRequirements {
            required_unless_present: &["normalized_input"],
            ..Default::default()
        },
        (["fit"], "prompt")
        | (["fit"], "source_binding")
        | (["fit"], "source_attempt_request")
        | (["fit"], "collected_attempt_results")
        | (["fit"], "job")
        | (["brief"], "prompt")
        | (["brief"], "source_binding")
        | (["brief"], "source_attempt_request")
        | (["brief"], "collected_attempt_results") => ConditionalRequirements {
            requires_when_present: &["normalized_input"],
            ..Default::default()
        },
        _ => ConditionalRequirements::default(),
    }
}

fn canonical_argument_ids(command: &Command, ids: &[&str]) -> Vec<String> {
    ids.iter()
        .map(|id| {
            let argument = command
                .get_arguments()
                .find(|argument| argument.get_id().as_str() == *id)
                .unwrap_or_else(|| {
                    panic!("conditional requirement references unknown argument {id}")
                });
            argument
                .get_long()
                .map(|name| format!("--{name}"))
                .or_else(|| argument.get_short().map(|name| format!("-{name}")))
                .unwrap_or_else(|| argument.get_id().to_string())
        })
        .collect()
}

fn action_name(action: &ArgAction) -> &'static str {
    match action {
        ArgAction::Set => "set",
        ArgAction::Append => "append",
        ArgAction::SetTrue => "set-true",
        ArgAction::SetFalse => "set-false",
        ArgAction::Count => "count",
        ArgAction::Help => "help",
        ArgAction::HelpShort => "help-short",
        ArgAction::HelpLong => "help-long",
        ArgAction::Version => "version",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn capabilities_exposes_presentation_contract() {
        let result = capabilities();
        let contract = &result["presentation_contract"];
        assert_eq!(contract["contract"], "mdp.presentation.v0");
        let selectors = contract["selectors"]
            .as_array()
            .expect("presentation selectors array");
        let names: Vec<String> = selectors
            .iter()
            .map(|selector| selector[0].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"trace --format".to_string()));
        assert!(names.contains(&"verify-output --readable".to_string()));
        assert!(names.contains(&"render-brief --format".to_string()));
        assert!(names.contains(&"sample-leads --format".to_string()));
        assert!(names.contains(&"brief --readable".to_string()));
        assert_eq!(contract["conflict"]["code"], "output_mode_conflict");
        assert_eq!(contract["conflict"]["exit_code"], 1);
        assert_eq!(contract["conflict"]["stdout"], "single_json_envelope");
        assert_eq!(contract["conflict"]["stderr"], "empty");
        assert_eq!(contract["display"]["help"]["command"], "help");
        assert_eq!(contract["display"]["help"]["exit_code"], 0);
        assert_eq!(contract["display"]["version"]["command"], "version");
        assert_eq!(contract["display"]["version"]["exit_code"], 0);
        assert_eq!(contract["stdout_invariant"]["value_count"], 1);
        assert_eq!(contract["stdout_invariant"]["stderr_empty"], true);

        let error_codes = result["stable_error_codes"]
            .as_array()
            .expect("error codes array");
        assert!(
            error_codes
                .iter()
                .any(|code| code["code"] == "output_mode_conflict"),
            "stable_error_codes must list output_mode_conflict"
        );
        assert!(
            error_codes
                .iter()
                .any(|code| code["code"] == "readme_marker_layout_invalid"),
            "stable_error_codes must list every validation-emitted README marker error"
        );

        let matrix = contract["matrix"]
            .get("selectors")
            .and_then(|value| value.as_array())
            .expect("matrix selectors");
        let mut has_human_only = false;
        let mut has_json_compatible = false;
        for row in matrix {
            if row["json_compatible"].as_bool().unwrap_or(false) {
                has_json_compatible = true;
            } else {
                has_human_only = true;
            }
        }
        assert!(has_json_compatible);
        assert!(has_human_only);
    }

    #[test]
    fn capabilities_exposes_agent_driving_contracts() {
        let result = capabilities();
        assert_eq!(result["contract"], "mdp.capabilities.v1");
        assert_eq!(
            result["route_budget_contracts"]["full"]["contract"],
            "mdp.route-budget.v0"
        );
        assert_eq!(
            result["route_budget_contracts"]["summary"]["contract"],
            "mdp.route-budget-summary.v1"
        );
        assert_eq!(
            result["route_budget_contracts"]["canonical_job_field"],
            "job_id"
        );
        assert_eq!(
            result["model_step_contracts"]["resolution"],
            MODEL_STEP_RESOLUTION_V1
        );
        assert_eq!(
            result["model_step_contracts"]["compiled_step"],
            COMPILED_MODEL_STEP_V1
        );
        assert_eq!(
            result["model_step_contracts"]["phase_order"],
            json!(["normalization", "generation", "review"])
        );
        assert_eq!(
            result["cold_model_conformance_contracts"]["model_execution"],
            "external-only"
        );
        assert_eq!(
            result["cold_model_conformance_contracts"]["commands"]["validate"]["output_contract"],
            BEHAVIORAL_EVALUATION_V1
        );
        assert_eq!(
            result["cold_model_conformance_contracts"]["commands"]["validate"]["model_calls"],
            false
        );
        assert_eq!(
            result["cold_model_conformance_contracts"]["contracts"]
                .as_array()
                .expect("conformance contracts")
                .len(),
            13
        );
        assert!(
            result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .any(|command| command["name"] == "capabilities")
        );
        let report = result["commands"]
            .as_array()
            .expect("commands array")
            .iter()
            .find(|command| command["argv"] == json!(["conformance", "report"]))
            .expect("conformance-report command");
        assert_eq!(
            report["output_contracts"],
            json!([CONFORMANCE_REPORT_V1, PUBLIC_CONFORMANCE_REPORT_V1])
        );
        assert!(report.get("output_contract").is_none());
        let validate = result["commands"]
            .as_array()
            .expect("commands array")
            .iter()
            .find(|command| command["argv"] == json!(["conformance", "validate"]))
            .expect("conformance validate command");
        assert_eq!(
            validate["required_args"],
            json!([
                "--artifact-root",
                "--candidate",
                "--evaluator-inventory",
                "--lifecycle-policy",
                "--deterministic",
                "--invocation",
                "--trial",
                "--verifier-receipt"
            ])
        );
        assert_eq!(
            validate["repeatable_args"],
            json!([
                "--invocation",
                "--trial",
                "--verifier-receipt",
                "--evaluator-result",
                "--publication-approval"
            ])
        );
        let assemble = result["commands"]
            .as_array()
            .expect("commands array")
            .iter()
            .find(|command| command["argv"] == json!(["conformance", "assemble"]))
            .expect("conformance assemble command");
        assert_eq!(
            assemble["required_args"],
            json!([
                "--candidate",
                "--deterministic",
                "--behavioral",
                "--artifact-root"
            ])
        );
        assert_eq!(assemble["repeatable_args"], json!(["--trial"]));
        assert!(
            result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .any(|command| command["name"] == "init" && command["supports_dry_run"] == true)
        );
        assert_eq!(result["profile_contracts"]["skills"], "mdp.skills.v1");
        assert_eq!(
            result["prompt_contracts"]["routed_context"]["contract"],
            ROUTED_CONTEXT_CONTRACT
        );
        assert_eq!(
            result["prompt_contracts"]["routed_context"]["schema_target"],
            "routed-context-v1"
        );
        assert_eq!(
            result["decision_input_contracts"]["requirements"],
            REQUIREMENTS_CONTRACT
        );
        assert_eq!(
            result["decision_input_contracts"]["job_ingress"]["contract"],
            "mdp.job-ingress.v1"
        );
        assert!(
            result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .any(|command| command["name"] == "requirements"
                    && command["output_contract"] == REQUIREMENTS_CONTRACT)
        );
        assert_eq!(
            result["decision_input_contracts"]["source_binding"],
            SOURCE_BINDING_CONTRACT
        );
        assert_eq!(
            result["decision_input_contracts"]["version_matrix"]["signal_aware_v2"]["requirements"],
            "mdp.requirements.v2"
        );
        assert_eq!(
            result["decision_input_contracts"]["version_matrix"]["scalar_v1"]["normalized_output"],
            NORMALIZED_DECISION_INPUT_CONTRACT
        );
        assert!(
            result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .any(|command| command["name"] == "validate-source-binding"
                    && command["output_contract"] == SOURCE_BINDING_VALIDATION_CONTRACT
                    && command["supports_strict"] == false)
        );
        for command_name in ["validate-prompt-output", "fit", "brief"] {
            let args = result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .find(|command| command["name"] == command_name)
                .and_then(|command| command["args"].as_array())
                .expect("command args");
            for required in [
                "--source-binding",
                "--source-attempt-request",
                "--collected-attempt-results",
            ] {
                assert!(args.iter().any(|arg| arg == required));
            }
        }
        for command_name in ["fit", "brief"] {
            let args = result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .find(|command| command["name"] == command_name)
                .and_then(|command| command["args"].as_array())
                .expect("command args");
            assert!(args.iter().any(|arg| arg == "--normalized-input"));
            assert!(args.iter().any(|arg| arg == "--prompt"));
            assert!(args.iter().any(|arg| arg == "--job"));
        }
        for command_name in ["brief", "emit-brief"] {
            let args = result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .find(|command| command["name"] == command_name)
                .and_then(|command| command["args"].as_array())
                .expect("command args");
            assert!(args.iter().any(|arg| arg == "--routed-context-out"));
        }
        let validation_args = result["commands"]
            .as_array()
            .expect("commands array")
            .iter()
            .find(|command| command["name"] == "validate-prompt-output")
            .and_then(|command| command["args"].as_array())
            .expect("validation args");
        assert!(validation_args.iter().any(|arg| arg == "--routed-context"));
        assert_eq!(
            result["proposal_evidence_contracts"]["native_normalize_request"]["schema_target"],
            "native-normalize-request"
        );
        assert_eq!(
            result["proposal_evidence_contracts"]["source_intake"]["contract"],
            SOURCE_INTAKE_CONTRACT
        );
        assert_eq!(
            result["proposal_evidence_contracts"]["proposal_mcp_run_result"]["contract"],
            PROPOSAL_MCP_RUN_RESULT_CONTRACT
        );
        assert_eq!(
            result["proposal_evidence_contracts"]["proposal_run_manifest"]["contract"],
            PROPOSAL_RUN_MANIFEST_CONTRACT
        );
        assert_eq!(
            result["proposal_evidence_contracts"]["proposal_readiness_report"]["contract"],
            PROPOSAL_READINESS_REPORT_CONTRACT
        );
        assert_eq!(result["target_contracts"]["kinds"][0], "company");
        assert!(
            result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .any(|command| command["name"] == "skills")
        );
        assert!(
            result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .any(|command| command["name"] == "run-receipt"
                    && command["output_contract"] == RUN_RECEIPT_CONTRACT)
        );
        assert!(
            result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .any(|command| command["name"] == "run"
                    && command["output_contract"] == RUN_EXECUTION_V1)
        );
        let legacy_run = result["commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|command| command["name"] == "run")
            .unwrap();
        assert!(
            legacy_run["args"]
                .as_array()
                .unwrap()
                .contains(&json!("--transport-timeout-ms"))
        );
        assert!(
            result["commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|command| command["name"] == "run-preflight"
                    && command["output_contract"] == "mdp.run-preflight.v1")
        );
        assert_eq!(
            result["decision_trace_contract"]["contract"],
            DECISION_TRACE_V1
        );
        assert_eq!(result["decision_trace_contract"]["projection_only"], true);
        assert_eq!(
            result["decision_trace_contract"]["prompt_output_authority"]["validation_contract"],
            PROMPT_OUTPUT_VALIDATION_CONTRACT
        );
        assert!(
            result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .any(|command| command["name"] == "validate-prompt-output"
                    && command["output_contract"] == PROMPT_OUTPUT_VALIDATION_CONTRACT)
        );
        assert!(
            result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .any(|command| command["name"] == "trace"
                    && command["output_contract"] == DECISION_TRACE_V1
                    && command["supports_out"] == true
                    && command["args"]
                        .as_array()
                        .is_some_and(|args| args.contains(&json!("--prompt-output"))))
        );
        assert_eq!(
            result["clean_run_contracts"]["canonical_authority_block"]["contract"],
            CANONICAL_AUTHORITY_BLOCK_V1
        );
        assert!(
            result["stable_error_codes"]
                .as_array()
                .expect("error codes array")
                .iter()
                .any(|code| code["code"] == "write_conflict")
        );
    }

    #[test]
    fn cli_contract_is_an_exact_clap_projection() {
        let result = capabilities();
        assert_eq!(result["cli"]["contract"], "mdp.cli-graph.v1");
        assert_eq!(result["cli"]["source"], "clap");
        assert_eq!(result["cli"]["subcommand_required"], true);
        assert_eq!(result["cli"]["classification"], "namespace");

        let projected = result["cli"]["commands"]
            .as_array()
            .expect("projected commands");
        let mut clap = Cli::command();
        clap.build();
        let mut expected_paths = Vec::new();
        collect_clap_paths(&clap, &[], &mut expected_paths);
        let projected_paths = projected
            .iter()
            .map(|command| {
                command["path"]
                    .as_array()
                    .expect("command path")
                    .iter()
                    .map(|part| part.as_str().unwrap().to_string())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(projected_paths, expected_paths);

        for command in projected {
            let path = command["path"]
                .as_array()
                .unwrap()
                .iter()
                .map(|part| part.as_str().unwrap())
                .collect::<Vec<_>>();
            let clap_command = clap_command_at(&clap, &path);
            let owned_path = path
                .iter()
                .map(|part| (*part).to_string())
                .collect::<Vec<_>>();
            assert_eq!(
                command["arguments"],
                json!(arguments_for(clap_command, &owned_path)),
                "argument semantic drift at {path:?}"
            );
            assert_eq!(
                command["subcommand_required"],
                clap_command.is_subcommand_required_set(),
                "subcommand requirement drift at {path:?}"
            );
        }
    }

    #[test]
    fn cli_contract_preserves_argument_semantics() {
        let result = capabilities();
        let commands = result["cli"]["commands"].as_array().unwrap();

        let preflight = projected_command(commands, &["run-preflight"]);
        let request = projected_argument(preflight, "--request");
        assert_eq!(request["required"], true);
        let timeout = projected_argument(preflight, "--transport-timeout-ms");
        assert_eq!(timeout["required"], false);
        assert_eq!(timeout["kind"], "option");

        let run = projected_command(commands, &["run"]);
        assert_eq!(
            projected_argument(run, "--transport-timeout-ms")["optional"],
            true
        );
        let init = projected_command(commands, &["init"]);
        assert_eq!(
            projected_argument(init, "--target-alias")["repeatable"],
            true
        );
        assert_eq!(
            projected_argument(init, "--dir")["default_values"],
            json!(["."])
        );
        let trace = projected_command(commands, &["trace"]);
        assert_eq!(
            projected_argument(trace, "--format")["enum_values"],
            json!([
                {"name": "json", "aliases": []},
                {"name": "mermaid", "aliases": []}
            ])
        );
        let rebind = projected_command(commands, &["rebind-synthetic-chain"]);
        assert_eq!(
            projected_argument(rebind, "--dry-run")["conflicts_with"],
            json!(["--apply", "--force"])
        );
        assert_eq!(
            projected_argument(rebind, "--force")["requires_when_present"],
            json!(["--apply"])
        );

        let skills = projected_command(commands, &["skills"]);
        assert_eq!(
            projected_argument(skills, "--job")["requires_when_present"],
            json!(["--dir"])
        );
        assert_eq!(
            projected_argument(trace, "--dir")["requires_when_present"],
            json!(["--file", "--prompt-output"])
        );
        assert_eq!(
            projected_argument(trace, "--bundle")["requires_when_present"],
            json!(["--receipt"])
        );
        assert_eq!(
            projected_argument(trace, "--receipt")["requires_when_present"],
            json!(["--bundle"])
        );

        let fit = projected_command(commands, &["fit"]);
        assert_eq!(
            projected_argument(fit, "--prospect")["required_unless_present"],
            json!(["--normalized-input"])
        );
        assert_eq!(
            projected_argument(fit, "--prompt")["requires_when_present"],
            json!(["--normalized-input"])
        );

        let nested_help = projected_command(commands, &["help", "conformance", "compile"]);
        assert_eq!(nested_help["classification"], "human-only");
        assert!(nested_help["human_only_reason"].is_string());
        assert!(
            commands
                .iter()
                .filter(|command| {
                    command["path"]
                        .as_array()
                        .is_some_and(|path| path.first() == Some(&json!("help")))
                })
                .all(|command| command["classification"] == "human-only")
        );
        for namespace in [&["conformance"][..], &["readme"][..]] {
            let command = projected_command(commands, namespace);
            assert_eq!(command["subcommand_required"], true);
            assert_eq!(command["classification"], "namespace");
        }
        assert_eq!(
            projected_command(commands, &["capabilities"])["subcommand_required"],
            false
        );
    }

    #[test]
    fn projected_conditional_requirements_match_clap_parsing() {
        assert!(Cli::try_parse_from(["mdp", "skills", "--job", "outbound-copy-brief"]).is_err());
        assert!(
            Cli::try_parse_from([
                "mdp",
                "skills",
                "--job",
                "outbound-copy-brief",
                "--dir",
                "."
            ])
            .is_ok()
        );

        let rebind = [
            "mdp",
            "rebind-synthetic-chain",
            "--job",
            "outbound-copy-brief",
            "--out-dir",
            "out",
            "--force",
        ];
        assert!(Cli::try_parse_from(rebind).is_err());
        assert!(
            Cli::try_parse_from(rebind.into_iter().chain(["--apply"]).collect::<Vec<_>>()).is_ok()
        );

        assert!(Cli::try_parse_from(["mdp", "trace", "--dir", "."]).is_err());
        assert!(
            Cli::try_parse_from([
                "mdp",
                "trace",
                "--file",
                "result.json",
                "--dir",
                ".",
                "--prompt-output",
                "prompt-output.json"
            ])
            .is_ok()
        );
        assert!(Cli::try_parse_from(["mdp", "trace", "--bundle", "bundle.json"]).is_err());
        assert!(
            Cli::try_parse_from([
                "mdp",
                "trace",
                "--bundle",
                "bundle.json",
                "--receipt",
                "receipt.json"
            ])
            .is_ok()
        );
    }

    fn collect_clap_paths(command: &Command, parent: &[String], output: &mut Vec<Vec<String>>) {
        for subcommand in command.get_subcommands() {
            let mut path = parent.to_vec();
            path.push(subcommand.get_name().to_string());
            output.push(path.clone());
            collect_clap_paths(subcommand, &path, output);
        }
    }

    fn clap_command_at<'a>(root: &'a Command, path: &[&str]) -> &'a Command {
        path.iter().fold(root, |command, name| {
            command
                .get_subcommands()
                .find(|candidate| candidate.get_name() == *name)
                .expect("Clap command path")
        })
    }

    fn projected_command<'a>(commands: &'a [Value], path: &[&str]) -> &'a Value {
        commands
            .iter()
            .find(|command| command["path"] == json!(path))
            .expect("projected command")
    }

    fn projected_argument<'a>(command: &'a Value, canonical: &str) -> &'a Value {
        command["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .find(|argument| argument["canonical"] == canonical)
            .expect("projected argument")
    }
}
