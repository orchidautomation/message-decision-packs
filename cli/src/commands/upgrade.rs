use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Command, ExitStatus};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const INSTALLER_URL: &str = "https://mdp.orchidlabs.dev/install.sh";
pub(crate) const UPGRADE_CHECK_CONTRACT: &str = "mdp.upgrade-check.v1";

pub(crate) struct UpgradeExecution {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) installed_version: Option<String>,
}

pub(crate) fn check(version: Option<&str>) -> Value {
    let running = env!("CARGO_PKG_VERSION");
    let requested = version.map(str::to_owned).or_else(|| {
        env::var("MDP_VERSION")
            .ok()
            .filter(|value| !value.trim().is_empty())
    });

    let target = match requested {
        Some(value) => Ok(value),
        None => latest_release(),
    };
    match target {
        Ok(target) => {
            let status = if normalize_version(&target) == normalize_version(running) {
                "current"
            } else {
                "update-available"
            };
            json!({
                "contract": UPGRADE_CHECK_CONTRACT,
                "status": status,
                "running_version": running,
                "target_version": target,
                "cli": {"status": status},
                "agent_bundles": {"status": "unassessed", "reason": "installed bundle versions are not reliably observable"},
                "source": INSTALLER_URL,
                "next_command": if status == "current" { "mdp upgrade --check" } else { "mdp upgrade" }
            })
        }
        Err(error) => json!({
            "contract": UPGRADE_CHECK_CONTRACT,
            "status": "unavailable",
            "running_version": running,
            "target_version": Value::Null,
            "cli": {"status": "unavailable"},
            "agent_bundles": {"status": "unassessed", "reason": "installed bundle versions are not reliably observable"},
            "source": INSTALLER_URL,
            "reason": error.to_string(),
            "next_command": "mdp upgrade --check"
        }),
    }
}

pub(crate) fn can_prompt_interactively() -> bool {
    if cfg!(debug_assertions) && env::var_os("MDP_UPGRADE_TEST_ASSUME_TTY").is_some() {
        return true;
    }
    io::stdin().is_terminal()
}

pub(crate) fn confirm() -> Result<bool> {
    eprint!("Upgrade the mdp CLI and all supported agent bundles? [y/N] ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

pub(crate) fn execute(version: Option<&str>) -> Result<UpgradeExecution> {
    let curl = find_tool("curl")?;
    let bash = find_tool("bash")?;
    let temp = UpgradeTemp::create()?;
    let installer = temp.path.join("install.sh");

    let download = Command::new(curl)
        .args(["-q", "-fsSL", "--proto", "=https", "--tlsv1.2", "-o"])
        .arg(&installer)
        .arg(INSTALLER_URL)
        .status()
        .context("failed to run curl for the MDP installer")?;
    if !download.success() {
        return Err(anyhow!(
            "upgrade_download_failed: curl exited with {download}"
        ));
    }
    let metadata = fs::symlink_metadata(&installer)
        .context("upgrade_download_failed: installer was not created")?;
    if !metadata.file_type().is_file() || metadata.len() == 0 {
        return Err(anyhow!(
            "upgrade_download_failed: installer is not a non-empty regular file"
        ));
    }

    let mut command = Command::new(bash);
    command.arg(&installer).args(["--agents", "-y"]);
    if let Some(version) = version {
        command.env("MDP_VERSION", version);
    }
    let output = command
        .output()
        .context("failed to run the MDP installer")?;
    let installed_version = output
        .status
        .success()
        .then(observe_installed_version)
        .flatten();
    Ok(UpgradeExecution {
        status: output.status,
        stdout: output.stdout,
        stderr: output.stderr,
        installed_version,
    })
}

fn latest_release() -> Result<String> {
    let curl = find_tool("curl")?;
    let repo = env::var("MDP_GITHUB_REPO")
        .unwrap_or_else(|_| "orchidautomation/message-decision-packs".to_string());
    if repo.contains(char::is_whitespace) || !repo.contains('/') {
        return Err(anyhow!("invalid MDP_GITHUB_REPO"));
    }
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let output = Command::new(curl)
        .args(["-q", "-fsSL", "--proto", "=https", "--tlsv1.2", &url])
        .output()
        .context("failed to run curl for the latest release")?;
    if !output.status.success() {
        return Err(anyhow!(
            "latest release request failed with {}",
            output.status
        ));
    }
    let payload: Value = serde_json::from_slice(&output.stdout)
        .context("latest release response was not valid JSON")?;
    payload["tag_name"]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("latest release response did not contain tag_name"))
}

fn find_tool(name: &str) -> Result<PathBuf> {
    let path = env::var_os("PATH").ok_or_else(|| anyhow!("missing required tool: {name}"))?;
    for directory in env::split_paths(&path) {
        let candidate = directory.join(name);
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
    }
    Err(anyhow!("missing required tool: {name}"))
}

fn is_executable_file(path: &PathBuf) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn observe_installed_version() -> Option<String> {
    let path = env::var_os("MDP_INSTALL_DIR")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/bin")))?
        .join("mdp");
    if !path.is_file() {
        return None;
    }
    let output = Command::new(path).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    text.split_whitespace().last().map(str::to_owned)
}

fn normalize_version(value: &str) -> &str {
    value.trim().strip_prefix('v').unwrap_or(value.trim())
}

struct UpgradeTemp {
    path: PathBuf,
}

impl UpgradeTemp {
    fn create() -> Result<Self> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = env::temp_dir().join(format!("mdp-upgrade-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).context("could not create the owned upgrade directory")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        }
        let installer = path.join("install.sh");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&installer)?;
        file.write_all(b"")?;
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&installer, fs::Permissions::from_mode(0o600))?;
        }
        Ok(Self { path })
    }
}

impl Drop for UpgradeTemp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
