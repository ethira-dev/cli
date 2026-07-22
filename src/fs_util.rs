//! Private directory/file helpers: symlink refusal, Unix modes, Windows ACLs.

use anyhow::{Context, Result, bail};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

/// Ensure `dir` exists as a real directory (not a symlink) with owner-only access.
pub fn ensure_private_dir(dir: &Path) -> Result<()> {
    if dir.exists() {
        let meta = fs::symlink_metadata(dir).with_context(|| format!("stat {}", dir.display()))?;
        if meta.file_type().is_symlink() {
            bail!(
                "refusing to use symlinked path {} — remove the symlink or choose another home",
                dir.display()
            );
        }
        if !meta.is_dir() {
            bail!("{} exists and is not a directory", dir.display());
        }
    } else {
        fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let meta = fs::symlink_metadata(dir).with_context(|| format!("stat {}", dir.display()))?;
        if meta.file_type().is_symlink() {
            bail!(
                "refusing to use symlinked path {} after create",
                dir.display()
            );
        }
    }

    #[cfg(unix)]
    {
        let mut perms = fs::metadata(dir)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(dir, perms).with_context(|| format!("chmod 0700 {}", dir.display()))?;
    }
    #[cfg(windows)]
    {
        restrict_to_current_user(dir)?;
    }
    Ok(())
}

/// Refuse to use `path` if it (or, when missing, we still check parents later) is a symlink.
pub fn refuse_symlink(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let meta = fs::symlink_metadata(path).with_context(|| format!("stat {}", path.display()))?;
    if meta.file_type().is_symlink() {
        bail!(
            "refusing to use symlinked path {} — remove the symlink first",
            path.display()
        );
    }
    Ok(())
}

/// Create an empty file with owner-only permissions if it does not exist.
pub fn ensure_private_file(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_private_dir(parent)?;
    }
    refuse_symlink(path)?;
    if path.exists() {
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(path)?.permissions();
            perms.set_mode(0o600);
            fs::set_permissions(path, perms)?;
        }
        #[cfg(windows)]
        {
            restrict_to_current_user(path)?;
        }
        return Ok(());
    }
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        opts.mode(0o600);
    }
    opts.open(path)
        .with_context(|| format!("create private file {}", path.display()))?;
    #[cfg(windows)]
    {
        restrict_to_current_user(path)?;
    }
    Ok(())
}

/// Atomically write `bytes` to `path` with owner-only permissions.
pub fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    ensure_private_dir(&parent)?;
    refuse_symlink(path)?;

    let tmp = parent.join(format!(".ethira-write.{}.tmp", std::process::id()));
    {
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            opts.mode(0o600);
        }
        let mut file = opts
            .open(&tmp)
            .with_context(|| format!("failed to create {}", tmp.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("failed to write {}", tmp.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync {}", tmp.display()))?;
    }
    fs::rename(&tmp, path).with_context(|| {
        format!(
            "failed to replace {} with {}",
            path.display(),
            tmp.display()
        )
    })?;
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms)?;
    }
    #[cfg(windows)]
    {
        restrict_to_current_user(path)?;
    }
    Ok(())
}

/// True when plaintext token fallback is explicitly opted in.
pub fn plaintext_tokens_allowed() -> bool {
    match std::env::var("ETHIRA_ALLOW_PLAINTEXT_TOKENS") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

#[cfg(windows)]
fn restrict_to_current_user(path: &Path) -> Result<()> {
    use std::process::Command;

    let user = std::env::var("USERNAME").unwrap_or_else(|_| "%USERNAME%".into());
    // Best-effort: strip inherited ACEs and grant full control to the current user only.
    let status = Command::new("icacls")
        .arg(path)
        .arg("/inheritance:r")
        .arg("/grant:r")
        .arg(format!("{user}:(F)"))
        .status()
        .with_context(|| format!("failed to run icacls on {}", path.display()))?;
    if !status.success() {
        bail!(
            "failed to restrict ACL on {} (icacls exit {:?})",
            path.display(),
            status.code()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ensure_private_dir_creates() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("ethira-test");
        ensure_private_dir(&nested).unwrap();
        assert!(nested.is_dir());
    }

    #[test]
    fn refuse_symlink_file() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target");
        fs::write(&target, b"x").unwrap();
        let link = dir.path().join("link");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target, &link).unwrap();
            let err = refuse_symlink(&link).unwrap_err();
            assert!(format!("{err:#}").contains("symlink"));
        }
        #[cfg(windows)]
        {
            // Symlink creation may require privilege; skip if unavailable.
            if std::os::windows::fs::symlink_file(&target, &link).is_ok() {
                let err = refuse_symlink(&link).unwrap_err();
                assert!(format!("{err:#}").contains("symlink"));
            }
        }
    }

    #[test]
    fn plaintext_env_gate() {
        // SAFETY: test process only; serial unit tests.
        unsafe {
            std::env::remove_var("ETHIRA_ALLOW_PLAINTEXT_TOKENS");
        }
        assert!(!plaintext_tokens_allowed());
        unsafe {
            std::env::set_var("ETHIRA_ALLOW_PLAINTEXT_TOKENS", "1");
        }
        assert!(plaintext_tokens_allowed());
        unsafe {
            std::env::remove_var("ETHIRA_ALLOW_PLAINTEXT_TOKENS");
        }
    }
}
