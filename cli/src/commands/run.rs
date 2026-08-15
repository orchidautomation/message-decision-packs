use crate::artifact_hash::{AuthorityJsonLimits, parse_authority_json};
use crate::authority::SourceAuthority;
use crate::run_contracts::RunRequestV1;
use crate::run_runtime::{RunFailure, RunFailureKind, execute_run};
use anyhow::Result;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

pub(crate) fn run_request_file(request_path: &Path, output_root: &Path) -> Result<Value> {
    let bytes = match fs::read(request_path) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(preflight_refusal("unavailable", "request-unreadable")),
    };
    let request: RunRequestV1 = match parse_authority_json(&bytes, AuthorityJsonLimits::default()) {
        Ok(request) => request,
        Err(_) => return Ok(preflight_refusal("unavailable", "request-invalid")),
    };
    match execute_run(&request, output_root) {
        Ok(execution) => Ok(serde_json::to_value(execution)?),
        Err(error) => {
            let (kind, code) = error
                .downcast_ref::<RunFailure>()
                .map(|failure| (failure.kind(), failure.code()))
                .unwrap_or((RunFailureKind::RunnerFailed, "runner-failed"));
            Ok(failure_result(&request.execution_id, kind, code))
        }
    }
}

fn preflight_refusal(execution_id: &str, reason_code: &str) -> Value {
    failure_result(execution_id, RunFailureKind::Preflight, reason_code)
}

fn failure_result(execution_id: &str, kind: RunFailureKind, reason_code: &str) -> Value {
    let (terminal_state, limitation, notice) = match kind {
        RunFailureKind::Preflight => (
            "no-draft:preflight-refused",
            "No immutable run bundle or receipt was created because preflight did not complete.",
            "This is a sanitized no-draft preflight result, not a verified run receipt.",
        ),
        RunFailureKind::PolicyBlocked => (
            "no-draft:policy-blocked",
            "No immutable run bundle or receipt was created because the declared boundary could not be honored.",
            "This is a sanitized policy-blocked result, not a verified run receipt.",
        ),
        RunFailureKind::RunnerFailed => (
            "no-draft:runner-failed",
            "No immutable run bundle or receipt was published because execution, cleanup, verification, or publication failed.",
            "This is a sanitized runner-failed result, not a verified run receipt.",
        ),
    };
    let authority = match kind {
        RunFailureKind::PolicyBlocked => SourceAuthority::block(reason_code, "run-policy"),
        RunFailureKind::Preflight | RunFailureKind::RunnerFailed => {
            SourceAuthority::unavailable(reason_code, "run-availability")
        }
    };
    json!({
        "contract": "mdp.run-execution.v1",
        "valid": false,
        "execution_id": execution_id,
        "terminal_state": terminal_state,
        "authority": authority,
        "run_dir": null,
        "bundle_sha256": null,
        "receipt_sha256": null,
        "authority_block": {
            "contract": "mdp.canonical-authority-block.v1",
            "execution_id": execution_id,
            "terminal_state": terminal_state,
            "decision": null,
            "assurance": [],
            "limitations": [
                limitation,
                "The surrounding conversation and its files are not decision authority."
            ],
            "reason_codes": [reason_code],
            "bundle_sha256": null,
            "receipt_sha256": null,
            "verification": null,
            "authority_notice": notice
        }
    })
}

#[cfg(test)]
mod tests {
    use super::run_request_file;
    use crate::commands::init::init_pack;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn missing_request_returns_sanitized_no_draft_without_local_path() {
        let value = run_request_file(
            Path::new("/private/customer/missing-run-request.json"),
            Path::new("/private/customer/output"),
        )
        .unwrap();
        assert_eq!(value["terminal_state"], "no-draft:preflight-refused");
        assert_eq!(
            value["authority_block"]["reason_codes"][0],
            "request-unreadable"
        );
        assert!(
            !serde_json::to_string(&value)
                .unwrap()
                .contains("/private/customer")
        );
    }

    #[test]
    fn pack_profile_mismatch_returns_sanitized_policy_blocked() {
        let root = std::env::temp_dir().join(format!(
            "mdp-run-command-profile-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let pack = root.join("pack");
        let request_path = root.join("request.json");
        let output = root.join("output.json");
        let run = root.join("run");
        init_pack(&pack, "Proposal Run", "proposal", true, false).unwrap();
        fs::write(&output, "{}\n").unwrap();
        fs::write(
            &request_path,
            serde_json::to_vec(&serde_json::json!({
                "contract": "mdp.run-request.v1",
                "execution_id": "profile-mismatch",
                "created_at": "2026-08-03T00:00:00Z",
                "profile": "gtm",
                "operation": "qualify",
                "mode": "deterministic",
                "job_identity": null,
                "pack_dir": pack,
                "pack_release_id": "proposal-release-1",
                "prompt": null,
                "inputs": [{
                    "logical_name": "prompt-output",
                    "source_path": output,
                    "schema_id": "mdp.prompt-output.v0",
                    "media_type": "application/json",
                    "provenance_refs": []
                }],
                "execution_policy": {
                    "environment_allowlist": [],
                    "filesystem_mode": "private-staging",
                    "tool_mode": "none",
                    "network_mode": "none",
                    "authorized_endpoints": [],
                    "max_input_bytes": 1048576,
                    "max_output_bytes": 1048576,
                    "timeout_ms": 30000,
                    "retention_policy": "receipt-only"
                },
                "driver": null,
                "model": null
            }))
            .unwrap(),
        )
        .unwrap();

        let value = run_request_file(&request_path, &run).unwrap();
        assert_eq!(value["terminal_state"], "no-draft:policy-blocked");
        assert_eq!(
            value["authority_block"]["reason_codes"][0],
            "pack-profile-mismatch"
        );
        assert!(!run.exists());
        let _ = fs::remove_dir_all(root);
    }
}
