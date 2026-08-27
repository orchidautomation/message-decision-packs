use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

fn checked_name(name: &str) -> Result<CString> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        bail!("secure install name must be one path component");
    }
    CString::new(name).map_err(|_| anyhow!("secure install name contains NUL"))
}

fn checked_directory(fd: RawFd, expected_dev: u64, expected_ino: u64) -> Result<()> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    let stat = unsafe { stat.assume_init() };
    if stat.st_dev as u64 != expected_dev || stat.st_ino as u64 != expected_ino {
        bail!("secure install directory identity mismatch");
    }
    Ok(())
}

fn opened_identity(fd: RawFd) -> Result<(u64, u64)> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    let stat = unsafe { stat.assume_init() };
    Ok((stat.st_dev as u64, stat.st_ino as u64))
}

fn named_identity(dir_fd: RawFd, name: &CString) -> Result<(u64, u64)> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            dir_fd,
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    let stat = unsafe { stat.assume_init() };
    Ok((stat.st_dev as u64, stat.st_ino as u64))
}

fn file_sha256(file: &mut File) -> Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn quarantine_name() -> Result<CString> {
    let mut random = [0_u8; 16];
    File::open("/dev/urandom")?.read_exact(&mut random)?;
    CString::new(format!(
        ".mdp-quarantine-{}",
        random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
    .map_err(Into::into)
}

#[cfg(target_os = "linux")]
fn rename_no_replace(dir_fd: RawFd, from: &CString, to: &CString) -> Result<()> {
    let status = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            dir_fd,
            from.as_ptr(),
            dir_fd,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if status != 0 {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn rename_no_replace(dir_fd: RawFd, from: &CString, to: &CString) -> Result<()> {
    const RENAME_EXCL: u32 = 0x00000004;
    let status =
        unsafe { libc::renameatx_np(dir_fd, from.as_ptr(), dir_fd, to.as_ptr(), RENAME_EXCL) };
    if status != 0 {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn rename_no_replace(_dir_fd: RawFd, _from: &CString, _to: &CString) -> Result<()> {
    bail!("secure identity-conditional removal is unsupported on this platform")
}

fn remove_if_identity(
    dir_fd: RawFd,
    name: &CString,
    expected_file_dev: u64,
    expected_file_ino: u64,
) -> Result<()> {
    let quarantine = quarantine_name()?;
    rename_no_replace(dir_fd, name, &quarantine)?;
    let moved = named_identity(dir_fd, &quarantine);
    match moved {
        Ok((dev, ino)) if dev == expected_file_dev && ino == expected_file_ino => {
            if unsafe { libc::unlinkat(dir_fd, quarantine.as_ptr(), 0) } != 0 {
                return Err(io::Error::last_os_error()).map_err(Into::into);
            }
            Ok(())
        }
        result => {
            let restore = rename_no_replace(dir_fd, &quarantine, name);
            match (result, restore) {
                (Ok(_), Ok(())) => bail!("secure remove file identity mismatch"),
                (Err(error), Ok(())) => Err(error),
                (_, Err(error)) => Err(anyhow!(
                    "secure remove preserved a quarantined replacement after restore failed: {error}"
                )),
            }
        }
    }
}

fn secure_install_with_hook<F: FnOnce()>(
    source: &Path,
    name: &CString,
    dir_fd: RawFd,
    receipt_fd: RawFd,
    before_path_check: F,
) -> Result<Value> {
    let metadata = std::fs::symlink_metadata(source)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        bail!("secure install source must be a regular file");
    }
    let mut input = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(source)?;
    let mut receipt = unsafe { File::from_raw_fd(receipt_fd) };
    let staging_name = quarantine_name()?;
    let target_fd = unsafe {
        libc::openat(
            dir_fd,
            staging_name.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if target_fd < 0 {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    let mut target = unsafe { File::from_raw_fd(target_fd) };
    let opened = opened_identity(target_fd)?;
    let mut published = false;
    let installed = (|| -> Result<(u64, u64, String)> {
        std::io::copy(&mut input, &mut target)?;
        target.sync_all()?;
        let identity = opened;
        let source_sha256 = file_sha256(&mut input)?;
        let target_sha256 = file_sha256(&mut target)?;
        if source_sha256 != target_sha256 {
            bail!("secure install content mismatch");
        }
        receipt.set_len(0)?;
        receipt.seek(SeekFrom::Start(0))?;
        serde_json::to_writer(
            &mut receipt,
            &json!({
                "contract": "mdp.secure-install-receipt.v1",
                "dev": identity.0.to_string(),
                "ino": identity.1.to_string(),
                "staging_leaf": staging_name.to_str()?
            }),
        )?;
        receipt.write_all(b"\n")?;
        receipt.sync_all()?;
        rename_no_replace(dir_fd, &staging_name, name)?;
        published = true;
        before_path_check();
        if named_identity(dir_fd, name)? != identity {
            bail!("secure install leaf identity mismatch");
        }
        Ok((identity.0, identity.1, target_sha256))
    })();
    drop(target);
    match installed {
        Ok((dev, ino, sha256)) => Ok(json!({
            "contract": "mdp.secure-install.v1",
            "status": "installed",
            "dev": dev.to_string(),
            "ino": ino.to_string(),
            "sha256": sha256
        })),
        Err(error) => {
            let cleanup_name = if published { name } else { &staging_name };
            let _ = remove_if_identity(dir_fd, cleanup_name, opened.0, opened.1);
            Err(error)
        }
    }
}

pub(crate) fn secure_install(
    action: &str,
    source: Option<&Path>,
    name: &str,
    dir_fd: RawFd,
    expected_dev: u64,
    expected_ino: u64,
    expected_file_dev: Option<u64>,
    expected_file_ino: Option<u64>,
    receipt_fd: Option<RawFd>,
) -> Result<Value> {
    checked_directory(dir_fd, expected_dev, expected_ino)?;
    let name = checked_name(name)?;
    match action {
        "install" => {
            let source = source.ok_or_else(|| anyhow!("secure install source is required"))?;
            let receipt_fd =
                receipt_fd.ok_or_else(|| anyhow!("secure install receipt fd is required"))?;
            secure_install_with_hook(source, &name, dir_fd, receipt_fd, || {})
        }
        "remove" => {
            let expected_file_dev =
                expected_file_dev.ok_or_else(|| anyhow!("expected file dev is required"))?;
            let expected_file_ino =
                expected_file_ino.ok_or_else(|| anyhow!("expected file ino is required"))?;
            match named_identity(dir_fd, &name) {
                Err(error)
                    if error
                        .downcast_ref::<io::Error>()
                        .is_some_and(|error| error.kind() == io::ErrorKind::NotFound) =>
                {
                    return Ok(json!({"contract": "mdp.secure-install.v1", "status": "absent"}));
                }
                Err(error) => return Err(error),
                Ok(_) => {}
            }
            remove_if_identity(dir_fd, &name, expected_file_dev, expected_file_ino)?;
            Ok(json!({"contract": "mdp.secure-install.v1", "status": "removed"}))
        }
        "verify" => {
            let source = source.ok_or_else(|| anyhow!("secure verify source is required"))?;
            let expected_file_dev =
                expected_file_dev.ok_or_else(|| anyhow!("expected file dev is required"))?;
            let expected_file_ino =
                expected_file_ino.ok_or_else(|| anyhow!("expected file ino is required"))?;
            let mut input = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(source)?;
            let target_fd = unsafe {
                libc::openat(
                    dir_fd,
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if target_fd < 0 {
                return Err(io::Error::last_os_error()).map_err(Into::into);
            }
            let mut target = unsafe { File::from_raw_fd(target_fd) };
            let identity = opened_identity(target_fd)?;
            if identity != (expected_file_dev, expected_file_ino)
                || named_identity(dir_fd, &name)? != identity
            {
                bail!("secure verify file identity mismatch");
            }
            let source_sha256 = file_sha256(&mut input)?;
            let target_sha256 = file_sha256(&mut target)?;
            if source_sha256 != target_sha256 {
                bail!("secure verify content mismatch");
            }
            if named_identity(dir_fd, &name)? != identity {
                bail!("secure verify leaf identity changed during verification");
            }
            Ok(json!({
                "contract": "mdp.secure-install.v1",
                "status": "verified",
                "dev": identity.0.to_string(),
                "ino": identity.1.to_string(),
                "sha256": target_sha256
            }))
        }
        _ => bail!("secure install action must be install, verify, or remove"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::fd::{AsRawFd, IntoRawFd};
    use std::os::unix::fs::{MetadataExt, symlink};

    #[test]
    fn installs_and_removes_relative_to_renamed_directory_handle() {
        let root = std::env::temp_dir().join(format!(
            "mdp-secure-install-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let parent = root.join("parent");
        let renamed = root.join("renamed");
        let escaped = root.join("escaped");
        std::fs::create_dir_all(&parent).unwrap();
        std::fs::create_dir_all(&escaped).unwrap();
        let source = root.join("source.json");
        File::create(&source)
            .unwrap()
            .write_all(b"fixture")
            .unwrap();
        let directory = File::open(&parent).unwrap();
        let receipt_fd = File::create(root.join("receipt.json"))
            .unwrap()
            .into_raw_fd();
        let identity = directory.metadata().unwrap();
        std::fs::rename(&parent, &renamed).unwrap();
        symlink(&escaped, &parent).unwrap();

        let installed = secure_install(
            "install",
            Some(&source),
            "request.json",
            directory.as_raw_fd(),
            identity.dev(),
            identity.ino(),
            None,
            None,
            Some(receipt_fd),
        )
        .unwrap();
        let receipt: Value =
            serde_json::from_slice(&std::fs::read(root.join("receipt.json")).unwrap()).unwrap();
        assert_eq!(receipt["contract"], "mdp.secure-install-receipt.v1");
        assert_eq!(receipt["dev"], installed["dev"]);
        assert_eq!(receipt["ino"], installed["ino"]);
        assert!(
            receipt["staging_leaf"]
                .as_str()
                .unwrap()
                .starts_with(".mdp-quarantine-")
        );
        assert_eq!(
            std::fs::read(renamed.join("request.json")).unwrap(),
            b"fixture"
        );
        assert!(!escaped.join("request.json").exists());

        secure_install(
            "remove",
            None,
            "request.json",
            directory.as_raw_fd(),
            identity.dev(),
            identity.ino(),
            Some(installed["dev"].as_str().unwrap().parse().unwrap()),
            Some(installed["ino"].as_str().unwrap().parse().unwrap()),
            None,
        )
        .unwrap();
        assert!(!renamed.join("request.json").exists());
        let absent = secure_install(
            "remove",
            None,
            "request.json",
            directory.as_raw_fd(),
            identity.dev(),
            identity.ino(),
            Some(installed["dev"].as_str().unwrap().parse().unwrap()),
            Some(installed["ino"].as_str().unwrap().parse().unwrap()),
            None,
        )
        .unwrap();
        assert_eq!(absent["status"], "absent");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_path_components_and_mismatched_directory_identity_without_writing() {
        let root = std::env::temp_dir().join(format!(
            "mdp-secure-install-denial-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source.json");
        std::fs::write(&source, b"fixture").unwrap();
        let directory = File::open(&root).unwrap();
        let identity = directory.metadata().unwrap();

        assert!(
            secure_install(
                "install",
                Some(&source),
                "../escaped.json",
                directory.as_raw_fd(),
                identity.dev(),
                identity.ino(),
                None,
                None,
                None,
            )
            .is_err()
        );
        assert!(
            secure_install(
                "install",
                Some(&source),
                "request.json",
                directory.as_raw_fd(),
                identity.dev(),
                identity.ino().wrapping_add(1),
                None,
                None,
                None,
            )
            .is_err()
        );
        assert!(
            secure_install(
                "install",
                None,
                "request.json",
                directory.as_raw_fd(),
                identity.dev(),
                identity.ino(),
                None,
                None,
                None,
            )
            .is_err()
        );
        assert!(
            secure_install(
                "install",
                Some(&source),
                "request.json",
                directory.as_raw_fd(),
                identity.dev(),
                identity.ino(),
                None,
                None,
                None,
            )
            .is_err()
        );
        assert!(
            secure_install(
                "unknown",
                Some(&source),
                "request.json",
                directory.as_raw_fd(),
                identity.dev(),
                identity.ino(),
                None,
                None,
                None,
            )
            .is_err()
        );
        assert!(!root.join("request.json").exists());
        assert!(!root.parent().unwrap().join("escaped.json").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn install_rejects_a_replaced_leaf_and_preserves_the_replacement() {
        let root = std::env::temp_dir().join(format!(
            "mdp-secure-install-leaf-race-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source.json");
        std::fs::write(&source, b"expected").unwrap();
        let directory = File::open(&root).unwrap();
        let name = CString::new("request.json").unwrap();
        let receipt_fd = File::create(root.join("receipt.json"))
            .unwrap()
            .into_raw_fd();
        let displaced = root.join("displaced.json");

        let result =
            secure_install_with_hook(&source, &name, directory.as_raw_fd(), receipt_fd, || {
                std::fs::rename(root.join("request.json"), &displaced).unwrap();
                std::fs::write(root.join("request.json"), b"concurrent replacement").unwrap();
            });

        assert!(result.is_err());
        assert_eq!(
            std::fs::read(root.join("request.json")).unwrap(),
            b"concurrent replacement"
        );
        assert_eq!(std::fs::read(displaced).unwrap(), b"expected");
        assert!(std::fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("mdp-quarantine")
        }));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn remove_quarantines_then_restores_a_concurrent_replacement() {
        let root = std::env::temp_dir().join(format!(
            "mdp-secure-remove-race-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("request.json");
        std::fs::write(&target, b"owned").unwrap();
        let owned = target.metadata().unwrap();
        std::fs::rename(&target, root.join("owned-moved.json")).unwrap();
        std::fs::write(&target, b"concurrent replacement").unwrap();
        let directory = File::open(&root).unwrap();

        let result = secure_install(
            "remove",
            None,
            "request.json",
            directory.as_raw_fd(),
            directory.metadata().unwrap().dev(),
            directory.metadata().unwrap().ino(),
            Some(owned.dev()),
            Some(owned.ino()),
            None,
        );

        assert!(result.is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"concurrent replacement");
        assert!(std::fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("mdp-quarantine")
        }));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
