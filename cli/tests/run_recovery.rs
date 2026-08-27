#![cfg(unix)]

use serde_json::{Value, json};
use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "mdp-run-recovery-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    root
}

fn set_old_mtime(path: &Path) {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .saturating_sub(600) as libc::time_t;
    let times = [
        libc::timespec {
            tv_sec: seconds,
            tv_nsec: 0,
        },
        libc::timespec {
            tv_sec: seconds,
            tv_nsec: 0,
        },
    ];
    let path = CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(
        unsafe { libc::utimensat(libc::AT_FDCWD, path.as_ptr(), times.as_ptr(), 0) },
        0
    );
}

fn stranded(root: &Path, leaf: &str, process_id: u32, age_seconds: u64) -> (PathBuf, PathBuf) {
    let transaction_leaf = format!(".{leaf}.tmp-0123456789abcdef0123456789abcdef");
    let transaction = root.join(&transaction_leaf);
    fs::create_dir(&transaction).unwrap();
    fs::set_permissions(&transaction, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(transaction.join("private-candidate"), b"not published\n").unwrap();
    let metadata = fs::symlink_metadata(&transaction).unwrap();
    let claim = root.join(format!(".{leaf}.mdp-run.claim"));
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    fs::write(
        &claim,
        serde_json::to_vec(&json!({
            "contract": "mdp.run-recovery-claim.v2",
            "execution_id": "simulated-killed-run",
            "transaction_leaf": transaction_leaf,
            "created_unix_seconds": now.saturating_sub(age_seconds),
            "owner_uid": unsafe { libc::geteuid() },
            "process_id": process_id,
            "transaction_dev": metadata.dev(),
            "transaction_ino": metadata.ino()
        }))
        .unwrap(),
    )
    .unwrap();
    fs::set_permissions(&claim, fs::Permissions::from_mode(0o600)).unwrap();
    if age_seconds >= 300 {
        set_old_mtime(&transaction);
        set_old_mtime(&claim);
    }
    (claim, transaction)
}

fn run_json(out_dir: &Path, apply: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mdp"));
    command
        .arg("--json")
        .arg("recover-run")
        .arg("--out-dir")
        .arg(out_dir);
    if apply {
        command.arg("--apply");
    }
    command.output().unwrap()
}

fn run_human(out_dir: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mdp"))
        .arg("recover-run")
        .arg("--out-dir")
        .arg(out_dir)
        .output()
        .unwrap()
}

fn data(output: &std::process::Output) -> Value {
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}

#[test]
fn simulated_killed_run_requires_dry_run_then_recovers_only_bound_state() {
    let root = temp_root("killed");
    let output_dir = root.join("run");
    let customer_workdir = root.join("customer-workdir");
    fs::create_dir(&customer_workdir).unwrap();
    fs::write(customer_workdir.join("keep.txt"), "customer\n").unwrap();
    let (claim, transaction) = stranded(&root, "run", 2_000_000_000, 600);

    let preview = run_json(&output_dir, false);
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    let preview = data(&preview);
    assert_eq!(preview["contract"], "mdp.run-recovery.v1");
    assert_eq!(preview["status"], "ready");
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["would_remove"].as_array().unwrap().len(), 2);
    assert!(claim.exists());
    assert!(transaction.exists());

    let human = run_human(&output_dir);
    assert!(human.status.success());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains("mdp.run-recovery.v1"));
    assert!(human.contains("ready"));
    assert!(human.contains("transaction-directory"));

    let applied = run_json(&output_dir, true);
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let applied = data(&applied);
    assert_eq!(applied["status"], "recovered");
    assert_eq!(applied["applied"], true);
    assert!(!claim.exists());
    assert!(!transaction.exists());
    assert_eq!(
        fs::read_to_string(customer_workdir.join("keep.txt")).unwrap(),
        "customer\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn recovery_refuses_recent_live_ambiguous_and_published_state() {
    let recent_root = temp_root("recent");
    let recent_out = recent_root.join("run");
    stranded(&recent_root, "run", 2_000_000_000, 0);
    let recent = run_json(&recent_out, true);
    assert!(!recent.status.success());
    assert_eq!(
        data(&recent)["diagnostics"][0]["code"],
        "recovery-claim-recent"
    );

    let live_root = temp_root("live");
    let live_out = live_root.join("run");
    stranded(&live_root, "run", std::process::id(), 600);
    let live = run_json(&live_out, true);
    assert!(!live.status.success());
    assert_eq!(
        data(&live)["diagnostics"][0]["code"],
        "recovery-process-live-or-unknown"
    );

    let link_root = temp_root("link");
    let link_out = link_root.join("run");
    let outside = link_root.join("outside");
    fs::write(&outside, "keep\n").unwrap();
    symlink(&outside, link_root.join(".run.mdp-run.claim")).unwrap();
    let linked = run_json(&link_out, true);
    assert!(!linked.status.success());
    assert_eq!(fs::read_to_string(&outside).unwrap(), "keep\n");

    let mode_root = temp_root("mode");
    let mode_out = mode_root.join("run");
    let (mode_claim, mode_transaction) = stranded(&mode_root, "run", 2_000_000_000, 600);
    fs::set_permissions(&mode_transaction, fs::Permissions::from_mode(0o755)).unwrap();
    let unsafe_mode = run_json(&mode_out, true);
    assert!(!unsafe_mode.status.success());
    assert_eq!(
        data(&unsafe_mode)["diagnostics"][0]["code"],
        "recovery-transaction-authority-unsafe"
    );
    assert!(mode_claim.exists());
    assert!(mode_transaction.exists());

    let metadata_root = temp_root("metadata");
    let metadata_out = metadata_root.join("run");
    let (metadata_claim, metadata_transaction) =
        stranded(&metadata_root, "run", 2_000_000_000, 600);
    let mut claim_value: Value =
        serde_json::from_slice(&fs::read(&metadata_claim).unwrap()).unwrap();
    claim_value["transaction_ino"] = json!(1);
    fs::write(&metadata_claim, serde_json::to_vec(&claim_value).unwrap()).unwrap();
    fs::set_permissions(&metadata_claim, fs::Permissions::from_mode(0o600)).unwrap();
    set_old_mtime(&metadata_claim);
    let inconsistent = run_json(&metadata_out, true);
    assert!(!inconsistent.status.success());
    assert_eq!(
        data(&inconsistent)["diagnostics"][0]["code"],
        "recovery-transaction-authority-unsafe"
    );
    assert!(metadata_claim.exists());
    assert!(metadata_transaction.exists());

    let published_root = temp_root("published");
    let published_out = published_root.join("run");
    let (claim, transaction) = stranded(&published_root, "run", 2_000_000_000, 600);
    fs::create_dir(&published_out).unwrap();
    fs::write(published_out.join("run-bundle.json"), "published\n").unwrap();
    let published = run_json(&published_out, true);
    assert!(!published.status.success());
    assert_eq!(
        data(&published)["diagnostics"][0]["code"],
        "recovery-destination-present"
    );
    assert!(claim.exists());
    assert!(transaction.exists());
    assert_eq!(
        fs::read_to_string(published_out.join("run-bundle.json")).unwrap(),
        "published\n"
    );

    for root in [
        recent_root,
        live_root,
        link_root,
        mode_root,
        metadata_root,
        published_root,
    ] {
        fs::remove_dir_all(root).unwrap();
    }
}
