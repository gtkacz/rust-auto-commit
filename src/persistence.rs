use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Run a complete read/modify/write transaction while holding an advisory lock.
///
/// Lock files intentionally remain beside their targets so every process uses a
/// stable inode. They never contain application data.
pub fn with_file_lock<T>(path: &Path, action: impl FnOnce() -> Result<T>) -> Result<T> {
    ensure_parent(path)?;
    let lock_path = lock_path(path);
    let lock = open_private(&lock_path)?;
    lock.lock_exclusive()
        .with_context(|| format!("Failed to lock {}", path.display()))?;
    let result = action();
    let unlock_result =
        FileExt::unlock(&lock).with_context(|| format!("Failed to unlock {}", path.display()));

    match (result, unlock_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

/// Atomically replace a file with owner-only permissions.
///
/// Callers performing a read/modify/write transaction should hold
/// [`with_file_lock`] across both the read and this write.
pub fn atomic_write_unlocked(path: &Path, contents: &[u8]) -> Result<()> {
    ensure_parent(path)?;
    let parent = path
        .parent()
        .context("Persistence target must have a parent directory")?;
    let mut temp = tempfile::Builder::new()
        .prefix(".cgen-")
        .tempfile_in(parent)
        .with_context(|| format!("Failed to create a temporary file in {}", parent.display()))?;

    set_owner_only(temp.as_file(), temp.path())?;
    temp.write_all(contents)
        .with_context(|| format!("Failed to write temporary file for {}", path.display()))?;
    temp.flush()
        .with_context(|| format!("Failed to flush temporary file for {}", path.display()))?;
    temp.as_file()
        .sync_all()
        .with_context(|| format!("Failed to sync temporary file for {}", path.display()))?;

    temp.persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to atomically replace {}", path.display()))?;
    sync_parent(parent)?;
    Ok(())
}

pub fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    with_file_lock(path, || atomic_write_unlocked(path, contents))
}

fn ensure_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .context("Persistence target must have a parent directory")?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create {}", parent.display()))?;
    Ok(())
}

fn lock_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".lock");
    path.with_file_name(name)
}

fn open_private(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    set_owner_only(&file, path)?;
    Ok(file)
}

#[cfg(unix)]
fn set_owner_only(file: &File, path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("Failed to secure {}", path.display()))
}

#[cfg(not(unix))]
fn set_owner_only(_file: &File, _path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn sync_parent(parent: &Path) -> Result<()> {
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .with_context(|| format!("Failed to sync {}", parent.display()))
}

#[cfg(not(unix))]
fn sync_parent(_parent: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_uses_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.toml");
        atomic_write(&path, b"secret").unwrap();
        let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
