//! Locking down the vault directory (L01 goal 7, SEC-S).
//!
//! * The folder gets an ACL that names only the current user (no inheritance, so no
//!   Administrators, SYSTEM or Users entries): `icacls <dir> /inheritance:r /grant:r
//!   "<DOMAIN>\<user>:(OI)(CI)F"`.
//! * It is marked "not content indexed" (`attrib +I`), so Windows Search does not index the
//!   vault files; new files created in it inherit the flag.
//!
//! Both are the system's own tools, started by absolute path under `%SystemRoot%\System32`
//! (never found through `PATH`), which keeps this crate free of `unsafe` Win32 ACL code.
//! This is hardening of the *files*; the vault content is encrypted regardless (docs/04).
//!
//! Not done here: File History exclusion (it is configured per user, not per folder; `%LOCALAPPDATA%`
//! is not part of the default File History libraries) and OneDrive known-folder redirection
//! (it moves Desktop, Documents and Pictures only, never `%LOCALAPPDATA%`).

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

fn system_tool(name: &str) -> io::Result<PathBuf> {
    let root = std::env::var_os("SystemRoot")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "SystemRoot is not set"))?;
    Ok(PathBuf::from(root).join("System32").join(name))
}

fn run(tool: &str, args: &[&std::ffi::OsStr]) -> io::Result<()> {
    let out = Command::new(system_tool(tool)?).args(args).output()?;
    if out.status.success() {
        Ok(())
    } else {
        // The tools' own output can contain paths; keep it out of the error.
        Err(io::Error::other(format!("{tool} failed")))
    }
}

/// `DOMAIN\user` of the current user, from the environment of this logon session.
fn current_user() -> io::Result<String> {
    let user = std::env::var("USERNAME")
        .map_err(|_| io::Error::new(io::ErrorKind::NotFound, "USERNAME is not set"))?;
    // On a machine that is not on a domain USERDOMAIN is the computer name, which icacls accepts.
    match std::env::var("USERDOMAIN") {
        Ok(domain) if !domain.is_empty() => Ok(format!("{domain}\\{user}")),
        _ => Ok(user),
    }
}

/// Creates `dir` if needed, restricts it to the current user and excludes it from content
/// indexing.
///
/// # Errors
/// An I/O error if the directory cannot be created or one of the tools fails.
pub fn secure_directory(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let grant = format!("{}:(OI)(CI)F", current_user()?);
    run(
        "icacls.exe",
        &[
            dir.as_os_str(),
            "/inheritance:r".as_ref(),
            "/grant:r".as_ref(),
            grant.as_ref(),
        ],
    )?;
    run("attrib.exe", &["+I".as_ref(), dir.as_os_str()])
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "aryavault-dirsec-{tag}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn tool_output(tool: &str, arg: &Path) -> String {
        let out = Command::new(system_tool(tool).unwrap())
            .arg(arg)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    #[test]
    fn only_the_current_user_keeps_access_and_indexing_is_off() {
        let dir = temp_dir("acl").join("vault");
        secure_directory(&dir).unwrap();

        let acl = tool_output("icacls.exe", &dir).to_lowercase();
        let user = std::env::var("USERNAME").unwrap().to_lowercase();
        assert!(
            acl.contains(&user),
            "the current user must keep access: {acl}"
        );
        for other in [
            "everyone",
            "builtin\\users",
            "builtin\\administrators",
            "nt authority\\system",
            "authenticated users",
        ] {
            assert!(
                !acl.contains(other),
                "{other} must not be in the ACL: {acl}"
            );
        }

        let attrs = tool_output("attrib.exe", &dir);
        let flags = attrs
            .split(dir.to_string_lossy().as_ref())
            .next()
            .unwrap_or("");
        assert!(
            flags.contains('I'),
            "not-content-indexed flag missing: {attrs}"
        );

        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn securing_twice_is_fine_and_a_file_inside_stays_readable() {
        let dir = temp_dir("again").join("vault");
        secure_directory(&dir).unwrap();
        secure_directory(&dir).unwrap();
        let file = dir.join("probe.bin");
        std::fs::write(&file, b"fake canary").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"fake canary");
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }
}
