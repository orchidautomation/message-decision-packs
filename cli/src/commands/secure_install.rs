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
fn rename_no_replace(dir_fd: RawFd, from: &std::ffi::CStr, to: &std::ffi::CStr) -> Result<()> {
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
fn rename_no_replace(dir_fd: RawFd, from: &std::ffi::CStr, to: &std::ffi::CStr) -> Result<()> {
    const RENAME_EXCL: u32 = 0x00000004;
    let status =
        unsafe { libc::renameatx_np(dir_fd, from.as_ptr(), dir_fd, to.as_ptr(), RENAME_EXCL) };
    if status != 0 {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn rename_no_replace(_dir_fd: RawFd, _from: &std::ffi::CStr, _to: &std::ffi::CStr) -> Result<()> {
    bail!("secure identity-conditional removal is unsupported on this platform")
}

struct SigtermMaskGuard {
    previous: libc::sigset_t,
}

impl SigtermMaskGuard {
    fn block() -> Result<Self> {
        let mut blocked = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
        let mut previous = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
        unsafe {
            libc::sigemptyset(blocked.as_mut_ptr());
            libc::sigaddset(blocked.as_mut_ptr(), libc::SIGTERM);
            let status =
                libc::pthread_sigmask(libc::SIG_BLOCK, blocked.as_ptr(), previous.as_mut_ptr());
            if status != 0 {
                return Err(io::Error::from_raw_os_error(status)).map_err(Into::into);
            }
            Ok(Self {
                previous: previous.assume_init(),
            })
        }
    }
}

impl Drop for SigtermMaskGuard {
    fn drop(&mut self) {
        unsafe {
            libc::pthread_sigmask(libc::SIG_SETMASK, &self.previous, std::ptr::null_mut());
        }
    }
}

fn remove_if_identity_with_hook<F: FnOnce()>(
    dir_fd: RawFd,
    name: &CString,
    expected_file_dev: u64,
    expected_file_ino: u64,
    after_quarantine: F,
) -> Result<()> {
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
    let target = unsafe { File::from_raw_fd(target_fd) };
    let opened = opened_identity(target_fd)?;
    if opened != (expected_file_dev, expected_file_ino) || named_identity(dir_fd, name)? != opened {
        bail!("secure remove file identity mismatch");
    }
    let quarantine = quarantine_name()?;
    // SIGTERM may arrive from the bounded supervisor, but must not interrupt
    // the finite rename -> identity check -> unlink/restore transaction. The
    // caller reserves a termination grace window before any SIGKILL.
    let _sigterm_guard = SigtermMaskGuard::block()?;
    if named_identity(dir_fd, name)? != opened {
        bail!("secure remove file identity changed before quarantine");
    }
    rename_no_replace(dir_fd, name, &quarantine)?;
    drop(target);
    after_quarantine();
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

fn remove_if_identity(
    dir_fd: RawFd,
    name: &CString,
    expected_file_dev: u64,
    expected_file_ino: u64,
) -> Result<()> {
    remove_if_identity_with_hook(dir_fd, name, expected_file_dev, expected_file_ino, || {})
}

fn move_directory_no_replace(
    dir_fd: RawFd,
    from: &CString,
    to: &CString,
    expected_dev: u64,
    expected_ino: u64,
) -> Result<()> {
    let target_fd = unsafe {
        libc::openat(
            dir_fd,
            from.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if target_fd < 0 {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    let target = unsafe { File::from_raw_fd(target_fd) };
    let opened = opened_identity(target_fd)?;
    if opened != (expected_dev, expected_ino) || named_identity(dir_fd, from)? != opened {
        bail!("secure move directory identity mismatch");
    }
    let _sigterm_guard = SigtermMaskGuard::block()?;
    if named_identity(dir_fd, from)? != opened {
        bail!("secure move directory identity changed before rename");
    }
    rename_no_replace(dir_fd, from, to)?;
    if named_identity(dir_fd, to)? != opened {
        let _ = rename_no_replace(dir_fd, to, from);
        bail!("secure move directory identity changed during rename");
    }
    drop(target);
    Ok(())
}

fn remove_empty_directory_if_identity(
    dir_fd: RawFd,
    name: &CString,
    expected_dev: u64,
    expected_ino: u64,
) -> Result<()> {
    let target_fd = unsafe {
        libc::openat(
            dir_fd,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if target_fd < 0 {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    let target = unsafe { File::from_raw_fd(target_fd) };
    let opened = opened_identity(target_fd)?;
    if opened != (expected_dev, expected_ino) || named_identity(dir_fd, name)? != opened {
        bail!("secure remove directory identity mismatch");
    }
    let _sigterm_guard = SigtermMaskGuard::block()?;
    if named_identity(dir_fd, name)? != opened {
        bail!("secure remove directory identity changed before removal");
    }
    if unsafe { libc::unlinkat(dir_fd, name.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    drop(target);
    Ok(())
}

fn remove_directory_contents_with_hook<F: FnMut(RawFd, &std::ffi::CStr, bool)>(
    dir_fd: RawFd,
    hook: &mut F,
) -> Result<()> {
    let duplicate = unsafe { libc::dup(dir_fd) };
    if duplicate < 0 {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    let stream = unsafe { libc::fdopendir(duplicate) };
    if stream.is_null() {
        unsafe { libc::close(duplicate) };
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    let mut names = Vec::new();
    loop {
        let entry = unsafe { libc::readdir(stream) };
        if entry.is_null() {
            break;
        }
        let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) };
        if name.to_bytes() == b"." || name.to_bytes() == b".." {
            continue;
        }
        names.push(name.to_owned());
    }
    if unsafe { libc::closedir(stream) } != 0 {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    // Snapshot names before introducing quarantine leaves so the iterator can
    // never consume names created by this transaction.
    for name in &names {
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
        let expected = (stat.st_dev as u64, stat.st_ino as u64);
        let quarantine = quarantine_name()?;
        let is_directory = stat.st_mode & libc::S_IFMT == libc::S_IFDIR;
        if is_directory {
            let child_fd = unsafe {
                libc::openat(
                    dir_fd,
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if child_fd < 0 {
                return Err(io::Error::last_os_error()).map_err(Into::into);
            }
            let child = unsafe { File::from_raw_fd(child_fd) };
            if opened_identity(child_fd)? != expected {
                bail!("secure directory entry identity changed before quarantine");
            }
            hook(dir_fd, name, true);
            rename_no_replace(dir_fd, name, &quarantine)?;
            if named_identity(dir_fd, &quarantine)? != expected {
                rename_no_replace(dir_fd, &quarantine, name).map_err(|error| {
                    anyhow!("secure directory entry mismatch restore failed: {error}")
                })?;
                bail!("secure directory entry identity changed during quarantine");
            }
            remove_directory_contents_with_hook(child_fd, hook)?;
            drop(child);
            if named_identity(dir_fd, &quarantine)? != expected {
                rename_no_replace(dir_fd, &quarantine, name).map_err(|error| {
                    anyhow!("secure directory entry pre-remove restore failed: {error}")
                })?;
                bail!("secure directory entry identity changed before removal");
            }
            if unsafe { libc::unlinkat(dir_fd, quarantine.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
                return Err(io::Error::last_os_error()).map_err(Into::into);
            }
        } else {
            hook(dir_fd, name, false);
            rename_no_replace(dir_fd, name, &quarantine)?;
            if named_identity(dir_fd, &quarantine)? != expected {
                rename_no_replace(dir_fd, &quarantine, name).map_err(|error| {
                    anyhow!("secure file entry mismatch restore failed: {error}")
                })?;
                bail!("secure file entry identity changed during quarantine");
            }
            if unsafe { libc::unlinkat(dir_fd, quarantine.as_ptr(), 0) } != 0 {
                return Err(io::Error::last_os_error()).map_err(Into::into);
            }
        }
    }
    Ok(())
}

fn remove_directory_tree_if_identity_with_hook<F: FnMut(RawFd, &std::ffi::CStr, bool)>(
    dir_fd: RawFd,
    name: &CString,
    expected_dev: u64,
    expected_ino: u64,
    hook: &mut F,
) -> Result<()> {
    let target_fd = unsafe {
        libc::openat(
            dir_fd,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if target_fd < 0 {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    let target = unsafe { File::from_raw_fd(target_fd) };
    let opened = opened_identity(target_fd)?;
    if opened != (expected_dev, expected_ino) || named_identity(dir_fd, name)? != opened {
        bail!("secure remove directory tree identity mismatch");
    }
    let _sigterm_guard = SigtermMaskGuard::block()?;
    if named_identity(dir_fd, name)? != opened {
        bail!("secure remove directory tree identity changed before removal");
    }
    let quarantine = quarantine_name()?;
    rename_no_replace(dir_fd, name, &quarantine)?;
    if named_identity(dir_fd, &quarantine)? != opened {
        rename_no_replace(dir_fd, &quarantine, name).map_err(|error| {
            anyhow!("secure remove directory tree mismatch restore failed: {error}")
        })?;
        bail!("secure remove directory tree identity changed during quarantine");
    }
    let removal = (|| -> Result<()> {
        remove_directory_contents_with_hook(target_fd, hook)?;
        if named_identity(dir_fd, &quarantine)? != opened {
            bail!("secure remove directory tree identity changed during removal");
        }
        if unsafe { libc::unlinkat(dir_fd, quarantine.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
            return Err(io::Error::last_os_error()).map_err(Into::into);
        }
        Ok(())
    })();
    if let Err(removal_error) = removal {
        match named_identity(dir_fd, &quarantine) {
            Ok(identity) if identity == opened => {
                rename_no_replace(dir_fd, &quarantine, name).map_err(|restore_error| {
                    anyhow!(
                        "secure remove directory tree failed ({removal_error}); restore failed: {restore_error}"
                    )
                })?;
            }
            Ok(_) => bail!(
                "secure remove directory tree failed ({removal_error}); quarantine identity changed and was preserved"
            ),
            Err(status_error) => bail!(
                "secure remove directory tree failed ({removal_error}); quarantine status failed: {status_error}"
            ),
        }
        return Err(removal_error);
    }
    drop(target);
    Ok(())
}

fn remove_directory_tree_if_identity(
    dir_fd: RawFd,
    name: &CString,
    expected_dev: u64,
    expected_ino: u64,
) -> Result<()> {
    remove_directory_tree_if_identity_with_hook(
        dir_fd,
        name,
        expected_dev,
        expected_ino,
        &mut |_, _, _| {},
    )
}

fn directory_identity_status(
    dir_fd: RawFd,
    name: &CString,
    expected_dev: u64,
    expected_ino: u64,
) -> Result<&'static str> {
    match named_identity(dir_fd, name) {
        Ok(identity) if identity == (expected_dev, expected_ino) => Ok("match"),
        Ok(_) => Ok("mismatch"),
        Err(error)
            if error
                .downcast_ref::<io::Error>()
                .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
        {
            Ok("absent")
        }
        Err(error) => Err(error),
    }
}

fn inspect_owned_marker_with_hook<F: FnOnce()>(
    dir_fd: RawFd,
    name: &CString,
    hook: F,
) -> Result<Value> {
    let marker_fd = unsafe {
        libc::openat(
            dir_fd,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if marker_fd < 0 {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    let marker = unsafe { File::from_raw_fd(marker_fd) };
    let mut root_stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    let mut marker_stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(dir_fd, root_stat.as_mut_ptr()) } != 0
        || unsafe { libc::fstat(marker_fd, marker_stat.as_mut_ptr()) } != 0
    {
        return Err(io::Error::last_os_error()).map_err(Into::into);
    }
    let root_stat = unsafe { root_stat.assume_init() };
    let marker_stat = unsafe { marker_stat.assume_init() };
    if marker_stat.st_mode & libc::S_IFMT != libc::S_IFREG
        || marker_stat.st_mode & 0o777 != 0o600
        || marker_stat.st_uid != root_stat.st_uid
        || marker_stat.st_size < 0
        || marker_stat.st_size > 4_096
    {
        bail!("owned workspace marker metadata is invalid");
    }
    hook();
    let mut bytes = Vec::new();
    marker.take(4_097).read_to_end(&mut bytes)?;
    if bytes.len() > 4_096 {
        bail!("owned workspace marker exceeds size limit");
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow!("owned workspace marker JSON is invalid"))?;
    #[cfg(target_os = "linux")]
    let mtime_ms = marker_stat.st_mtime * 1_000 + marker_stat.st_mtime_nsec / 1_000_000;
    #[cfg(target_os = "macos")]
    let mtime_ms =
        marker_stat.st_mtimespec.tv_sec * 1_000 + marker_stat.st_mtimespec.tv_nsec / 1_000_000;
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let mtime_ms = marker_stat.st_mtime * 1_000;
    Ok(json!({"value": value, "mtime_ms": mtime_ms}))
}

fn inspect_owned_marker(dir_fd: RawFd, name: &CString) -> Result<Value> {
    inspect_owned_marker_with_hook(dir_fd, name, || {})
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
        receipt.set_len(0)?;
        receipt.seek(SeekFrom::Start(0))?;
        serde_json::to_writer(
            &mut receipt,
            &json!({
                "contract": "mdp.secure-install-receipt.v1",
                "dev": opened.0.to_string(),
                "ino": opened.1.to_string(),
                "staging_leaf": staging_name.to_str()?
            }),
        )?;
        receipt.write_all(b"\n")?;
        receipt.sync_all()?;
        std::io::copy(&mut input, &mut target)?;
        target.sync_all()?;
        let identity = opened;
        let source_sha256 = file_sha256(&mut input)?;
        let target_sha256 = file_sha256(&mut target)?;
        if source_sha256 != target_sha256 {
            bail!("secure install content mismatch");
        }
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
    to_name: Option<&str>,
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
        "move-directory" => {
            let to_name = checked_name(
                to_name.ok_or_else(|| anyhow!("secure move destination name is required"))?,
            )?;
            let expected_file_dev =
                expected_file_dev.ok_or_else(|| anyhow!("expected directory dev is required"))?;
            let expected_file_ino =
                expected_file_ino.ok_or_else(|| anyhow!("expected directory ino is required"))?;
            move_directory_no_replace(
                dir_fd,
                &name,
                &to_name,
                expected_file_dev,
                expected_file_ino,
            )?;
            Ok(json!({"contract": "mdp.secure-install.v1", "status": "moved"}))
        }
        "remove-directory" => {
            let expected_file_dev =
                expected_file_dev.ok_or_else(|| anyhow!("expected directory dev is required"))?;
            let expected_file_ino =
                expected_file_ino.ok_or_else(|| anyhow!("expected directory ino is required"))?;
            remove_empty_directory_if_identity(
                dir_fd,
                &name,
                expected_file_dev,
                expected_file_ino,
            )?;
            Ok(json!({"contract": "mdp.secure-install.v1", "status": "removed"}))
        }
        "remove-directory-tree" => {
            let expected_file_dev =
                expected_file_dev.ok_or_else(|| anyhow!("expected directory dev is required"))?;
            let expected_file_ino =
                expected_file_ino.ok_or_else(|| anyhow!("expected directory ino is required"))?;
            remove_directory_tree_if_identity(dir_fd, &name, expected_file_dev, expected_file_ino)?;
            Ok(json!({"contract": "mdp.secure-install.v1", "status": "removed"}))
        }
        "verify-directory" => {
            let expected_file_dev =
                expected_file_dev.ok_or_else(|| anyhow!("expected directory dev is required"))?;
            let expected_file_ino =
                expected_file_ino.ok_or_else(|| anyhow!("expected directory ino is required"))?;
            let status =
                directory_identity_status(dir_fd, &name, expected_file_dev, expected_file_ino)?;
            Ok(json!({"contract": "mdp.secure-install.v1", "status": status}))
        }
        "inspect-owned-workspace" => {
            let inspected = inspect_owned_marker(dir_fd, &name)?;
            Ok(
                json!({"contract": "mdp.secure-install.v1", "status": "inspected", "marker": inspected["value"], "marker_mtime_ms": inspected["mtime_ms"]}),
            )
        }
        _ => bail!(
            "secure install action must be install, verify, remove, move-directory, remove-directory, remove-directory-tree, verify-directory, or inspect-owned-workspace"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io::Write;
    use std::os::fd::{AsRawFd, IntoRawFd};
    use std::os::unix::fs::{MetadataExt, symlink};

    #[test]
    fn directory_move_is_no_replace_and_identity_bound() {
        let root = std::env::temp_dir().join(format!(
            "mdp-secure-directory-move-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = root.join("source");
        let destination = root.join("destination");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(source.join("owned"), b"owned bytes").unwrap();
        std::fs::write(destination.join("keep"), b"unrelated bytes").unwrap();
        let parent = File::open(&root).unwrap();
        let source_identity = std::fs::metadata(&source).unwrap();
        let source_name = CString::new("source").unwrap();
        let destination_name = CString::new("destination").unwrap();

        assert!(
            move_directory_no_replace(
                parent.as_raw_fd(),
                &source_name,
                &destination_name,
                source_identity.dev(),
                source_identity.ino(),
            )
            .is_err()
        );
        assert_eq!(std::fs::read(source.join("owned")).unwrap(), b"owned bytes");
        assert_eq!(
            std::fs::read(destination.join("keep")).unwrap(),
            b"unrelated bytes"
        );

        std::fs::remove_dir_all(&destination).unwrap();
        move_directory_no_replace(
            parent.as_raw_fd(),
            &source_name,
            &destination_name,
            source_identity.dev(),
            source_identity.ino(),
        )
        .unwrap();
        assert!(!source.exists());
        assert_eq!(
            std::fs::read(destination.join("owned")).unwrap(),
            b"owned bytes"
        );
        std::fs::remove_file(destination.join("owned")).unwrap();
        assert!(
            remove_empty_directory_if_identity(
                parent.as_raw_fd(),
                &destination_name,
                source_identity.dev(),
                source_identity.ino().wrapping_add(1),
            )
            .is_err()
        );
        remove_empty_directory_if_identity(
            parent.as_raw_fd(),
            &destination_name,
            source_identity.dev(),
            source_identity.ino(),
        )
        .unwrap();
        assert!(!destination.exists());

        let tree = root.join("tree");
        let outside = root.join("outside");
        std::fs::create_dir_all(tree.join("nested")).unwrap();
        std::fs::write(tree.join("nested/file"), b"owned").unwrap();
        std::fs::write(&outside, b"outside").unwrap();
        symlink(&outside, tree.join("link")).unwrap();
        let tree_identity = std::fs::metadata(&tree).unwrap();
        let tree_name = CString::new("tree").unwrap();
        assert_eq!(
            directory_identity_status(
                parent.as_raw_fd(),
                &tree_name,
                tree_identity.dev(),
                tree_identity.ino(),
            )
            .unwrap(),
            "match"
        );
        remove_directory_tree_if_identity(
            parent.as_raw_fd(),
            &tree_name,
            tree_identity.dev(),
            tree_identity.ino(),
        )
        .unwrap();
        assert!(!tree.exists());
        assert_eq!(std::fs::read(&outside).unwrap(), b"outside");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn recursive_cleanup_preserves_entries_swapped_after_classification() {
        let root = std::env::temp_dir().join(format!(
            "mdp-secure-tree-race-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();

        let file_tree = root.join("files");
        std::fs::create_dir(&file_tree).unwrap();
        std::fs::write(file_tree.join("victim"), b"owned").unwrap();
        let replacement_file = root.join("replacement-file");
        std::fs::write(&replacement_file, b"replacement").unwrap();
        let file_dir = File::open(&file_tree).unwrap();
        let file_result = remove_directory_contents_with_hook(
            file_dir.as_raw_fd(),
            &mut |_, name, is_directory| {
                if name.to_bytes() == b"victim" && !is_directory {
                    std::fs::rename(file_tree.join("victim"), file_tree.join("saved")).unwrap();
                    std::fs::rename(&replacement_file, file_tree.join("victim")).unwrap();
                }
            },
        );
        assert!(file_result.is_err());
        assert_eq!(std::fs::read(file_tree.join("saved")).unwrap(), b"owned");
        assert_eq!(
            std::fs::read(file_tree.join("victim")).unwrap(),
            b"replacement"
        );

        let dir_tree = root.join("dirs");
        std::fs::create_dir_all(dir_tree.join("victim")).unwrap();
        std::fs::write(dir_tree.join("victim/owned"), b"owned").unwrap();
        let replacement_dir = root.join("replacement-dir");
        std::fs::create_dir(&replacement_dir).unwrap();
        std::fs::write(replacement_dir.join("keep"), b"replacement").unwrap();
        let dir_fd = File::open(&dir_tree).unwrap();
        let dir_result = remove_directory_contents_with_hook(
            dir_fd.as_raw_fd(),
            &mut |_, name, is_directory| {
                if name.to_bytes() == b"victim" && is_directory {
                    std::fs::rename(dir_tree.join("victim"), dir_tree.join("saved")).unwrap();
                    std::fs::rename(&replacement_dir, dir_tree.join("victim")).unwrap();
                }
            },
        );
        assert!(dir_result.is_err());
        assert_eq!(
            std::fs::read(dir_tree.join("saved/owned")).unwrap(),
            b"owned"
        );
        assert_eq!(
            std::fs::read(dir_tree.join("victim/keep")).unwrap(),
            b"replacement"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn owned_marker_inspection_stays_bound_during_root_path_swap() {
        let root = std::env::temp_dir().join(format!(
            "mdp-secure-marker-race-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let victim = root.join("victim");
        let replacement = root.join("replacement");
        std::fs::create_dir_all(&victim).unwrap();
        std::fs::create_dir(&replacement).unwrap();
        let marker_name = CString::new("marker.json").unwrap();
        std::fs::write(victim.join("marker.json"), br#"{"owner":"victim"}"#).unwrap();
        std::fs::write(
            replacement.join("marker.json"),
            br#"{"owner":"replacement"}"#,
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            victim.join("marker.json"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        std::fs::set_permissions(
            replacement.join("marker.json"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let directory = File::open(&victim).unwrap();

        let inspected = inspect_owned_marker_with_hook(directory.as_raw_fd(), &marker_name, || {
            std::fs::rename(&victim, root.join("saved")).unwrap();
            std::fs::rename(&replacement, &victim).unwrap();
        })
        .unwrap();
        assert_eq!(inspected["value"]["owner"], "victim");
        assert_eq!(
            std::fs::read_to_string(victim.join("marker.json")).unwrap(),
            r#"{"owner":"replacement"}"#
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn failed_tree_removal_restores_discoverable_outer_name() {
        let root = std::env::temp_dir().join(format!(
            "mdp-secure-tree-restore-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let tree = root.join("tree");
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::write(tree.join("seed"), b"seed").unwrap();
        let parent = File::open(&root).unwrap();
        let identity = std::fs::metadata(&tree).unwrap();
        let name = CString::new("tree").unwrap();
        let mut added = false;
        let result = remove_directory_tree_if_identity_with_hook(
            parent.as_raw_fd(),
            &name,
            identity.dev(),
            identity.ino(),
            &mut |directory_fd, _, _| {
                if !added {
                    let late = CString::new("late").unwrap();
                    let fd = unsafe {
                        libc::openat(
                            directory_fd,
                            late.as_ptr(),
                            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC,
                            0o600,
                        )
                    };
                    assert!(fd >= 0);
                    let mut file = unsafe { File::from_raw_fd(fd) };
                    file.write_all(b"concurrent").unwrap();
                    added = true;
                }
            },
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read(tree.join("late")).unwrap(), b"concurrent");
        assert!(std::fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".mdp-quarantine-")
        }));
        std::fs::remove_dir_all(&root).unwrap();
    }

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
            None,
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
    fn remove_rejects_a_preexisting_identical_replacement_before_quarantine() {
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
        std::fs::write(&target, b"owned").unwrap();
        let replacement = target.metadata().unwrap();
        let directory = File::open(&root).unwrap();
        let hook_called = Cell::new(false);

        let result = remove_if_identity_with_hook(
            directory.as_raw_fd(),
            &CString::new("request.json").unwrap(),
            owned.dev(),
            owned.ino(),
            || hook_called.set(true),
        );

        assert!(result.is_err());
        assert!(
            !hook_called.get(),
            "mismatch must be rejected before rename"
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"owned");
        let after = target.metadata().unwrap();
        assert_eq!(
            (after.dev(), after.ino()),
            (replacement.dev(), replacement.ino())
        );
        assert!(std::fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("mdp-quarantine")
        }));
        std::fs::remove_dir_all(&root).unwrap();
    }

    fn sigterm_is_blocked() -> bool {
        let mut current = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
        let status = unsafe {
            libc::pthread_sigmask(libc::SIG_BLOCK, std::ptr::null(), current.as_mut_ptr())
        };
        assert_eq!(status, 0);
        unsafe { libc::sigismember(current.as_ptr(), libc::SIGTERM) == 1 }
    }

    #[test]
    fn remove_masks_sigterm_only_during_quarantine_transaction() {
        let root = std::env::temp_dir().join(format!(
            "mdp-secure-remove-signal-mask-{}-{}",
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
        let directory = File::open(&root).unwrap();
        let before = sigterm_is_blocked();
        let observed = Cell::new(false);

        remove_if_identity_with_hook(
            directory.as_raw_fd(),
            &CString::new("request.json").unwrap(),
            owned.dev(),
            owned.ino(),
            || observed.set(sigterm_is_blocked()),
        )
        .unwrap();

        assert!(
            observed.get(),
            "SIGTERM must be masked after quarantine rename"
        );
        assert_eq!(
            sigterm_is_blocked(),
            before,
            "prior signal mask must be restored"
        );
        assert!(!target.exists());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
