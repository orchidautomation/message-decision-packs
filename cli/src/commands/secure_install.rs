use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io;
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

pub(crate) fn secure_install(
    action: &str,
    source: Option<&Path>,
    name: &str,
    dir_fd: RawFd,
    expected_dev: u64,
    expected_ino: u64,
    expected_file_dev: Option<u64>,
    expected_file_ino: Option<u64>,
) -> Result<Value> {
    checked_directory(dir_fd, expected_dev, expected_ino)?;
    let name = checked_name(name)?;
    match action {
        "install" => {
            let source = source.ok_or_else(|| anyhow!("secure install source is required"))?;
            let metadata = std::fs::symlink_metadata(source)?;
            if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                bail!("secure install source must be a regular file");
            }
            let mut input = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(source)?;
            let target_fd = unsafe {
                libc::openat(
                    dir_fd,
                    name.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if target_fd < 0 {
                return Err(io::Error::last_os_error()).map_err(Into::into);
            }
            let mut target = unsafe { File::from_raw_fd(target_fd) };
            let installed = (|| -> Result<(u64, u64)> {
                std::io::copy(&mut input, &mut target)?;
                target.sync_all()?;
                opened_identity(target_fd)
            })();
            drop(target);
            match installed {
                Ok((dev, ino)) => Ok(json!({
                    "contract": "mdp.secure-install.v1",
                    "status": "installed",
                    "dev": dev.to_string(),
                    "ino": ino.to_string()
                })),
                Err(error) => {
                    unsafe { libc::unlinkat(dir_fd, name.as_ptr(), 0) };
                    Err(error)
                }
            }
        }
        "remove" => {
            let expected_file_dev =
                expected_file_dev.ok_or_else(|| anyhow!("expected file dev is required"))?;
            let expected_file_ino =
                expected_file_ino.ok_or_else(|| anyhow!("expected file ino is required"))?;
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
            let identity = opened_identity(target_fd);
            unsafe { libc::close(target_fd) };
            let (dev, ino) = identity?;
            if dev != expected_file_dev || ino != expected_file_ino {
                bail!("secure remove file identity mismatch");
            }
            if unsafe { libc::unlinkat(dir_fd, name.as_ptr(), 0) } != 0 {
                return Err(io::Error::last_os_error()).map_err(Into::into);
            }
            Ok(json!({"contract": "mdp.secure-install.v1", "status": "removed"}))
        }
        _ => bail!("secure install action must be install or remove"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::fd::AsRawFd;
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
        )
        .unwrap();
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
        )
        .unwrap();
        assert!(!renamed.join("request.json").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
