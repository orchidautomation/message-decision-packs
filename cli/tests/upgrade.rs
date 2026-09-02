#[cfg(unix)]
mod unix {
    use serde_json::Value;
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output, Stdio};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("mdp-upgrade-test-{label}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn script(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn fixture(label: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root = root(label);
        let tools = root.join("tools");
        let install = root.join("install");
        fs::create_dir_all(&tools).unwrap();
        fs::create_dir_all(&install).unwrap();
        script(
            &tools.join("curl"),
            r#"#!/bin/sh
echo "$@" >> "$FAKE_CURL_LOG"
out=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then shift; out="$1"; fi
  shift
done
if [ "${FAKE_CURL_EXIT:-0}" != 0 ]; then exit "$FAKE_CURL_EXIT"; fi
if [ -n "$out" ]; then
  printf '%s\n' '# fake installer' > "$out"
elif [ -n "${FAKE_LATEST_JSON:-}" ]; then
  printf '%s\n' "$FAKE_LATEST_JSON"
else
  printf '%s\n' '{"tag_name":"v0.1.112"}'
fi
"#,
        );
        script(
            &tools.join("bash"),
            r#"#!/bin/sh
echo "args:$* version:${MDP_VERSION:-}" >> "$FAKE_BASH_LOG"
printf '%s\n' 'installer: aligned targets complete'
if [ "${FAKE_BASH_EXIT:-0}" != 0 ]; then printf '%s\n' 'installer failed' >&2; exit "$FAKE_BASH_EXIT"; fi
/bin/mkdir -p "$MDP_INSTALL_DIR"
/bin/cat > "$MDP_INSTALL_DIR/mdp" <<EOF
#!/bin/sh
echo "mdp ${FAKE_INSTALL_VERSION:-0.1.112}"
EOF
/bin/chmod 700 "$MDP_INSTALL_DIR/mdp"
"#,
        );
        (root, tools, install)
    }

    fn command(tools: &Path, install: &Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mdp"));
        command
            .env("PATH", tools)
            .env("HOME", install.parent().unwrap().join("home"))
            .env("MDP_INSTALL_DIR", install)
            .env("FAKE_CURL_LOG", install.parent().unwrap().join("curl.log"))
            .env("FAKE_BASH_LOG", install.parent().unwrap().join("bash.log"));
        command
    }

    fn output_with_input(mut command: Command, input: &str) -> Output {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    #[test]
    fn noninteractive_execution_requires_yes_before_network() {
        let (root, tools, install) = fixture("confirm");
        let output = command(&tools, &install).arg("upgrade").output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("mdp upgrade -y"));
        assert!(!root.join("curl.log").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn json_execution_is_rejected_without_side_effects() {
        let (root, tools, install) = fixture("json-reject");
        let output = command(&tools, &install)
            .args(["--json", "upgrade", "-y"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["error"]["code"], "upgrade_json_execution_unsupported");
        assert!(!root.join("curl.log").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn yes_runs_fixed_installer_with_agents_and_reports_observed_version() {
        let (root, tools, install) = fixture("success");
        let output = command(&tools, &install)
            .env("MDP_VERSION", "v1.0.0")
            .args(["upgrade", "-y", "--version", "v9.8.7"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("CLI result: installer succeeded (observed version 0.1.112)"));
        assert!(stdout.contains("Agent bundle results: aligned installer succeeded"));
        assert_eq!(
            stdout
                .matches("Restart or reload affected open agent applications.")
                .count(),
            1
        );
        let curl = fs::read_to_string(root.join("curl.log")).unwrap();
        assert!(curl.contains("https://mdp.orchidlabs.dev/install.sh"));
        let bash = fs::read_to_string(root.join("bash.log")).unwrap();
        assert!(bash.contains("--agents -y") && bash.contains("version:v9.8.7"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installer_failure_preserves_exit_and_has_no_success_footer() {
        let (root, tools, install) = fixture("installer-fail");
        let output = command(&tools, &install)
            .env("FAKE_BASH_EXIT", "42")
            .args(["upgrade", "-y"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(42));
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("installer: aligned targets complete"));
        assert!(!stdout.contains("CLI result: installer succeeded"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("installer failed"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_check_is_json_and_never_runs_installer() {
        let (root, tools, install) = fixture("check");
        let output = command(&tools, &install)
            .args(["--json", "upgrade", "--check", "--version", "v9.8.7"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["command"], "upgrade");
        assert_eq!(value["data"]["contract"], "mdp.upgrade-check.v1");
        assert_eq!(value["data"]["status"], "update-available");
        assert!(!root.join("curl.log").exists());
        assert!(!root.join("bash.log").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_latest_check_is_unavailable_not_current() {
        let (root, tools, install) = fixture("check-fail");
        let output = command(&tools, &install)
            .env("FAKE_CURL_EXIT", "7")
            .args(["--json", "upgrade", "--check"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["ok"], false);
        assert_eq!(value["data"]["status"], "unavailable");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn latest_check_reports_current_for_the_running_release() {
        let (root, tools, install) = fixture("check-current");
        let output = command(&tools, &install)
            .args(["--json", "upgrade", "--check"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["data"]["status"], "current");
        assert_eq!(value["data"]["target_version"], "v0.1.112");
        assert_eq!(value["data"]["next_command"], "mdp upgrade --check");
        assert!(!root.join("bash.log").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interactive_decline_starts_no_network() {
        let (root, tools, install) = fixture("decline");
        let mut cmd = command(&tools, &install);
        cmd.env("MDP_UPGRADE_TEST_ASSUME_TTY", "1").arg("upgrade");
        let output = output_with_input(cmd, "no\n");
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("cancelled"));
        assert!(!root.join("curl.log").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interactive_accept_runs_the_same_aligned_path() {
        let (root, tools, install) = fixture("accept");
        let mut cmd = command(&tools, &install);
        cmd.env("MDP_UPGRADE_TEST_ASSUME_TTY", "1").arg("upgrade");
        let output = output_with_input(cmd, "yes\n");
        assert!(output.status.success());
        assert!(root.join("curl.log").exists());
        assert!(root.join("bash.log").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn downloader_failure_stops_before_installer() {
        let (root, tools, install) = fixture("download-fail");
        let output = command(&tools, &install)
            .env("FAKE_CURL_EXIT", "22")
            .args(["upgrade", "-y"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("upgrade_download_failed"));
        assert!(!root.join("bash.log").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_required_tool_stops_before_network() {
        let (root, tools, install) = fixture("missing-tool");
        fs::remove_file(tools.join("bash")).unwrap();
        let output = command(&tools, &install)
            .args(["upgrade", "-y"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("missing required tool: bash"));
        assert!(!root.join("curl.log").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
