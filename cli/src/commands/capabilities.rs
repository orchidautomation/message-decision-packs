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
    PROMPT_FORMAT_VERSION, PROMPT_OUTPUT_CONTRACT, PROMPT_PROSPECT_NORMALIZATION_SCHEMA_REF,
    PROPOSAL_MCP_RUN_RESULT_CONTRACT, PROPOSAL_READINESS_REPORT_CONTRACT,
    PROPOSAL_RUN_MANIFEST_CONTRACT, PROPOSAL_RUNNER_RESULT_CONTRACT, REQUIREMENTS_CONTRACT,
    REQUIREMENTS_CONTRACT_V2, ROUTED_CONTEXT_CONTRACT, RUN_RECEIPT_CONTRACT, RUNNER_AUDIT_CONTRACT,
    SOURCE_AUDIT_CONTRACT, SOURCE_BINDING_CONTRACT, SOURCE_BINDING_CONTRACT_V2,
    SOURCE_BINDING_VALIDATION_CONTRACT, SOURCE_INTAKE_CONTRACT,
};
use crate::model_steps::{COMPILED_MODEL_STEP_V1, MODEL_STEP_RESOLUTION_V1};
use crate::models::DecisionInputAttemptStatus;
use crate::run_contracts::{
    CANONICAL_AUTHORITY_BLOCK_V1, DRIVER_REQUEST_V1, DRIVER_REQUEST_V2, DRIVER_RESULT_V1,
    DRIVER_RESULT_V2, PROPOSAL_RUNNER_RESULT_V1, RUN_BUNDLE_V1, RUN_EXECUTION_V1, RUN_RECEIPT_V1,
    RUN_REQUEST_V1, RUN_VERIFICATION_V1, RUNNER_AUDIT_V1,
};
use serde_json::{Value, json};

pub(crate) fn capabilities() -> Value {
    let conformance_contracts = conformance_schemas()
        .into_iter()
        .map(|(schema_target, contract, _)| {
            json!({"contract":contract,"schema_target":schema_target})
        })
        .collect::<Vec<_>>();
    json!({
        "contract": "mdp.capabilities.v0",
        "tool": "mdp",
        "format_version": FORMAT_VERSION,
        "defaults": {
            "pack_dir": DEFAULT_DIR,
            "offline_by_default": true,
            "auth_required": false,
            "init_templates": ["gtm", "proposal"]
        },
        "global_options": [
            {"name": "--json", "description": "Emit stable machine-readable JSON"},
            {"name": "--summary", "description": "Emit a compact status summary"}
        ],
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
            "canonical_authority_block": {"contract": CANONICAL_AUTHORITY_BLOCK_V1, "schema_target": "canonical-authority-block-v1"},
            "assurance": "Vector-valued evidence; v0 labels and driver assertions never silently elevate."
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
            "boundary": "The pack and CLI own questions and deterministic decisions. The customer or host owns source collection, provider access, model calls, copy generation, and sequencing."
        },
        "target_contracts": {
            "manifest_target": "Optional for existing/reference packs; required by the target-aware GTM authoring path.",
            "kinds": ["company", "product", "project"],
            "external_vs_internal": "External target terms may drive positioning. MDP, CLI, schema, prompt, card, and eval vocabulary remains internal implementation context.",
            "contamination_issue_codes": ["target_contamination_excluded_term", "target_contamination_internal_vocabulary"]
        },
        "commands": [
            command("capabilities", "mdp.capabilities.v0", "read-only", false, false, false, &[]),
            nested_command("compile", DETERMINISTIC_CONFORMANCE_V1, &["--candidate", "--artifact-root"], &[], &["--out", "--dry-run"]),
            nested_command("validate", BEHAVIORAL_EVALUATION_V1, &["--artifact-root", "--candidate", "--evaluator-inventory", "--lifecycle-policy", "--deterministic", "--invocation", "--trial", "--verifier-receipt"], &["--invocation", "--trial", "--verifier-receipt", "--evaluator-result", "--publication-approval"], &["--evaluator-result", "--publication-approval", "--out", "--dry-run"]),
            nested_command("assemble", JOB_CONFORMANCE_V1, &["--candidate", "--deterministic", "--behavioral", "--artifact-root"], &["--trial"], &["--out", "--dry-run"]),
            nested_command_with_outputs("report", &[CONFORMANCE_REPORT_V1, PUBLIC_CONFORMANCE_REPORT_V1], &["--conformance", "--artifact-root", "--visibility", "--generated-at"], &[], &["--out", "--dry-run"]),
            command("init", "mdp.init.v0", "writes-files", true, false, false, &["--name", "--target-name", "--target-kind", "--target-alias", "--exclude-term", "--dir", "--template", "--force", "--include-output-schemas", "--dry-run"]),
            command("doctor", "mdp.doctor.v0", "read-only", false, false, false, &["--dir"]),
            command("skills", "mdp.skills.v1", "read-only", false, false, false, &["--dir", "--job"]),
            command("requirements", REQUIREMENTS_CONTRACT, "read-only", false, false, false, &["--dir", "--job"]),
            command("validate-source-binding", SOURCE_BINDING_VALIDATION_CONTRACT, "read-only", false, false, false, &["--dir", "--job", "--file"]),
            command("validate", "mdp.validate.v0", "read-only", false, false, true, &["--dir", "--strict"]),
            command("validate-prompt-output", "mdp.validate-prompt-output.v0", "read-only", false, false, true, &["--dir", "--file", "--source-audit", "--source-binding", "--source-attempt-request", "--collected-attempt-results", "--invocation-receipt", "--routed-context", "--prompt", "--prompt-id", "--strict"]),
            command("run-receipt", RUN_RECEIPT_CONTRACT, "writes-files-with-out", true, true, false, &["--dir", "--workflow", "--isolation", "--declared-inputs-only", "--prompt-id", "--prompt-output", "--validation", "--source-audit", "--runner-audit", "--require-runner-audit", "--artifact", "--out", "--dry-run"]),
            command("verify-run", RUN_VERIFICATION_V1, "read-only", false, false, false, &["--bundle", "--receipt", "--artifact-root"]),
            command("trace", DECISION_TRACE_V1, "read-only-unless-out", false, true, false, &["--file", "--bundle", "--receipt", "--artifact-root", "--format", "--out"]),
            command("consume-run", "mdp.run-consumption-result.v1", "writes-local-ledger", false, false, false, &["--ledger", "--job-id", "--idempotency-key", "--receipt-sha256", "--expected-prior-version", "--permit-exact-replay"]),
            command("run", RUN_EXECUTION_V1, "writes-new-run-directory", false, true, false, &["--request", "--out-dir"]),
            command("verify-output", "mdp.verify-output.v0", "read-only", false, false, false, &["--dir", "--file", "--readable"]),
            command("author-proof-output", "mdp.author-proof-output.v0", "writes-files-with-out", true, true, false, &["--dir", "--draft", "--out", "--dry-run"]),
            command("render-brief", "mdp.human-brief.v0", "writes-files-with-out", false, true, true, &["--dir", "--file", "--template", "--format", "--out", "--strict"]),
            command("explain", "mdp.explain.v0", "read-only", false, false, false, &["--dir", "--persona"]),
            command("route", "mdp.route.v0", "read-only", false, false, false, &["--dir", "--persona", "--job", "--scope", "--entries", "--eval-fixture"]),
            command("sample-leads", "mdp.sample-leads.v0", "read-only", false, false, false, &["--dir", "--persona", "--job", "--count", "--seed", "--format"]),
            command("fit", "mdp.fit.v0", "read-only", false, false, false, &["--dir", "--prospect", "--normalized-input", "--prompt", "--source-binding", "--source-attempt-request", "--collected-attempt-results", "--job"]),
            command("check-claims", "mdp.claim-check.v0", "read-only", false, false, true, &["--dir", "--text", "--file", "--subject", "--persona", "--job", "--scope", "--strict"]),
            command("gaps", "mdp.gaps.v0", "read-only", false, false, false, &["--dir"]),
            command("eval", "mdp.eval.v0", "read-only", false, false, true, &["--dir", "--strict"]),
            command("brief", "mdp.message-brief.v0", "writes-files-with-out", true, true, false, &["--dir", "--prospect", "--normalized-input", "--prompt", "--source-binding", "--source-attempt-request", "--collected-attempt-results", "--channel", "--job", "--context", "--routed-context-out", "--readable", "--out", "--dry-run"]),
            command("copy", "mdp.copy-demo.v0", "writes-files-with-out", false, true, false, &["--dir", "--prospect", "--channel", "--out"]),
            command("emit-brief", "mdp.brief.v0", "writes-files-with-out", true, true, false, &["--dir", "--persona", "--motion", "--job", "--scope", "--routed-context-out", "--out", "--dry-run"]),
            command("pack", "mdp.pack.v0", "writes-files-with-out", true, true, false, &["--dir", "--out", "--dry-run"]),
            command("schema", "mdp.schema.v0", "read-only", false, false, false, &["target"])
        ],
        "stable_error_codes": [
            {"code": "pack_not_found", "meaning": "A pack manifest or required .mdp path could not be read"},
            {"code": "invalid_manifest", "meaning": "A pack manifest could not be parsed or uses invalid structure"},
            {"code": "invalid_prospect", "meaning": "A prospect input uses unsupported fields or invalid structure"},
            {"code": "missing_card", "meaning": "A referenced card could not be found or read"},
            {"code": "unsupported_claim", "meaning": "Draft text contains unsupported claims or claim-check failures"},
            {"code": "invalid_proof_output", "meaning": "A proof-output artifact is malformed or references missing or incompatible pack IDs"},
            {"code": "invalid_human_brief", "meaning": "A human-brief source artifact is malformed or missing required gate/proof fields"},
            {"code": "insufficient_context", "meaning": "A fit or drafting path lacks enough context to proceed"},
            {"code": "write_conflict", "meaning": "A write would overwrite an existing file without explicit permission"},
            {"code": "invalid_argument", "meaning": "CLI arguments are missing, conflicting, or unsupported"},
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_exposes_agent_driving_contracts() {
        let result = capabilities();
        assert_eq!(result["contract"], "mdp.capabilities.v0");
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
        assert_eq!(
            result["decision_trace_contract"]["contract"],
            DECISION_TRACE_V1
        );
        assert_eq!(result["decision_trace_contract"]["projection_only"], true);
        assert!(
            result["commands"]
                .as_array()
                .expect("commands array")
                .iter()
                .any(|command| command["name"] == "trace"
                    && command["output_contract"] == DECISION_TRACE_V1
                    && command["supports_out"] == true)
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
}
