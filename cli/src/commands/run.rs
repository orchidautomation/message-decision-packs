use crate::artifact_hash::{AuthorityJsonLimits, parse_authority_json};
use crate::authority::SourceAuthority;
use crate::run_contracts::RunRequestV1;
use crate::run_runtime::{
    RunDiagnostic, RunFailure, RunFailureKind, deadline_preflight, execute_run_with_transport,
};
use anyhow::Result;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

#[cfg(unix)]
pub(crate) fn secure_run_request_file(
    request_path: &Path,
    output_leaf: &str,
    display_output_dir: &Path,
    dir_fd: std::os::fd::RawFd,
    expected_dev: u64,
    expected_ino: u64,
    transport_timeout_ms: Option<u64>,
) -> Result<Value> {
    use std::mem::MaybeUninit;

    if output_leaf.is_empty()
        || output_leaf == "."
        || output_leaf == ".."
        || output_leaf.as_bytes().contains(&b'/')
        || output_leaf.as_bytes().contains(&0)
    {
        anyhow::bail!("secure run output leaf is invalid");
    }
    if !display_output_dir.is_absolute()
        || display_output_dir
            .file_name()
            .and_then(|name| name.to_str())
            != Some(output_leaf)
    {
        anyhow::bail!("secure run display output is invalid");
    }
    let mut stat = MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(dir_fd, stat.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let stat = unsafe { stat.assume_init() };
    if (stat.st_mode & libc::S_IFMT) != libc::S_IFDIR
        || stat.st_dev as u64 != expected_dev
        || stat.st_ino as u64 != expected_ino
    {
        anyhow::bail!("secure run output parent identity mismatch");
    }
    if unsafe { libc::fchdir(dir_fd) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // All output operations now resolve from the inherited, identity-checked
    // directory descriptor. Replacing its former pathname cannot redirect it.
    let descriptor_relative_output = Path::new(".").join(output_leaf);
    let mut result = run_request_file_with_transport(
        request_path,
        &descriptor_relative_output,
        transport_timeout_ms,
    )?;
    // The runtime must resolve only relative to the checked descriptor, but the
    // public contract continues to report the policy-approved absolute path.
    // The adapter discards this value if that public parent identity changed.
    if result.get("run_dir").and_then(Value::as_str).is_some() {
        result["run_dir"] = json!(display_output_dir.display().to_string());
        if let Some(verification) = result
            .get_mut("authority_block")
            .and_then(|value| value.get_mut("verification"))
        {
            verification["bundle"] = json!(display_output_dir.join("run-bundle.json"));
            verification["receipt"] = json!(display_output_dir.join("run-receipt.json"));
            verification["artifact_root"] = json!(display_output_dir.display().to_string());
        }
    }
    Ok(result)
}

pub(crate) fn recover_run_output(output_root: &Path, apply: bool) -> Result<Value> {
    crate::run_runtime::recover_run_output(output_root, apply)
}

pub(crate) fn run_request_file(request_path: &Path, output_root: &Path) -> Result<Value> {
    run_request_file_with_transport(request_path, output_root, None)
}

pub(crate) fn run_request_file_with_transport(
    request_path: &Path,
    output_root: &Path,
    transport_timeout_ms: Option<u64>,
) -> Result<Value> {
    let bytes = match fs::read(request_path) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(preflight_refusal("unavailable", "request-unreadable")),
    };
    let request: RunRequestV1 = match parse_authority_json(&bytes, AuthorityJsonLimits::default()) {
        Ok(request) => request,
        Err(_) => return Ok(preflight_refusal("unavailable", "request-invalid")),
    };
    match execute_run_with_transport(&request, output_root, transport_timeout_ms) {
        Ok(execution) => Ok(serde_json::to_value(execution)?),
        Err(error) => {
            if let Some(failure) = error.downcast_ref::<RunFailure>() {
                Ok(failure_result(
                    &request.execution_id,
                    failure.kind(),
                    failure.code(),
                    failure.diagnostics(),
                    failure.deadline(),
                ))
            } else {
                Ok(failure_result(
                    &request.execution_id,
                    RunFailureKind::RunnerFailed,
                    "runner-failed",
                    &[],
                    None,
                ))
            }
        }
    }
}

pub(crate) fn run_preflight_file(
    request_path: &Path,
    transport_timeout_ms: Option<u64>,
) -> Result<Value> {
    let bytes = match fs::read(request_path) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(preflight_refusal("unavailable", "request-unreadable")),
    };
    let request: RunRequestV1 = match parse_authority_json(&bytes, AuthorityJsonLimits::default()) {
        Ok(request) => request,
        Err(_) => return Ok(preflight_refusal("unavailable", "request-invalid")),
    };
    match deadline_preflight(&request, transport_timeout_ms) {
        Ok(value) => Ok(value),
        Err(error) => {
            if let Some(failure) = error.downcast_ref::<RunFailure>() {
                Ok(failure_result(
                    &request.execution_id,
                    failure.kind(),
                    failure.code(),
                    failure.diagnostics(),
                    failure.deadline(),
                ))
            } else {
                Ok(preflight_refusal(&request.execution_id, "preflight-failed"))
            }
        }
    }
}

fn preflight_refusal(execution_id: &str, reason_code: &str) -> Value {
    failure_result(
        execution_id,
        RunFailureKind::Preflight,
        reason_code,
        &[],
        None,
    )
}

fn failure_result(
    execution_id: &str,
    kind: RunFailureKind,
    reason_code: &str,
    diagnostics: &[RunDiagnostic],
    deadline: Option<&crate::run_contracts::DeadlineObservationV1>,
) -> Value {
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
    let diagnostics = if matches!(kind, RunFailureKind::PolicyBlocked) {
        serde_json::to_value(diagnostics).unwrap_or_else(|_| serde_json::json!([]))
    } else if reason_code == "output-directory-claimed" {
        json!([{"code": "output-directory-claimed"}])
    } else {
        serde_json::json!([])
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
            "diagnostics": diagnostics,
            "deadline": deadline,
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
        assert_eq!(
            value["authority_block"]["diagnostics"][0]["stage"],
            "run-preflight"
        );
        assert_eq!(value["authority_block"]["diagnostics"][0]["gate"], "policy");
        assert_eq!(
            value["authority_block"]["diagnostics"][0]["code"],
            "internal-contract-mismatch"
        );
        let serialized = serde_json::to_string(&value).unwrap();
        assert!(!serialized.contains("/private/customer"));
        assert!(!run.exists());
        let _ = fs::remove_dir_all(root);
    }
}
