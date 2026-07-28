use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::AppConfig;
use crate::{generation, git, provider, workflow};

const MARKER: &str = "# cgen-managed prepare-commit-msg hook v1";
const BACKUP_MARKER: &str = "# cgen-backup-name: ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookStatus {
    NotInstalled,
    Installed { path: PathBuf },
    Unmanaged { path: PathBuf },
}

pub fn effective_hook_path() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-path", "hooks/prepare-commit-msg"])
        .output()
        .context("Failed to resolve Git hooks directory")?;
    if !output.status.success() {
        anyhow::bail!(
            "Failed to resolve Git hooks directory: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

pub fn status() -> Result<HookStatus> {
    let path = effective_hook_path()?;
    match fs::read_to_string(&path) {
        Ok(content) if is_managed(&content) => Ok(HookStatus::Installed { path }),
        Ok(_) => Ok(HookStatus::Unmanaged { path }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(HookStatus::NotInstalled),
        Err(error) => Err(error).with_context(|| format!("Failed to read {}", path.display())),
    }
}

pub fn install() -> Result<PathBuf> {
    let path = effective_hook_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let existing = fs::read_to_string(&path).ok();
    let backup_name = if let Some(content) = existing.as_deref().filter(|text| is_managed(text)) {
        backup_name_from_wrapper(content)
    } else if path.exists() {
        let backup = collision_free_backup(&path);
        fs::rename(&path, &backup)
            .with_context(|| format!("Failed to preserve existing hook as {}", backup.display()))?;
        backup
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    } else {
        None
    };

    let executable = std::env::current_exe()
        .context("Failed to locate the current cgen executable")?
        .canonicalize()
        .context("Failed to resolve the current cgen executable")?;
    let wrapper = build_wrapper(&executable, backup_name.as_deref());
    if existing.as_deref() != Some(wrapper.as_str()) {
        crate::persistence::atomic_write(&path, wrapper.as_bytes())
            .with_context(|| format!("Failed to install {}", path.display()))?;
    }
    make_executable(&path)?;
    Ok(path)
}

pub fn uninstall() -> Result<PathBuf> {
    let path = effective_hook_path()?;
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!("cgen hook is not installed")
        }
        Err(error) => {
            return Err(error).with_context(|| format!("Failed to read {}", path.display()))
        }
    };
    if !is_managed(&content) {
        anyhow::bail!("Refusing to uninstall unmanaged hook at {}", path.display());
    }
    let backup = backup_name_from_wrapper(&content)
        .and_then(|name| path.parent().map(|parent| parent.join(name)));
    fs::remove_file(&path)
        .with_context(|| format!("Failed to remove cgen hook at {}", path.display()))?;
    if let Some(backup) = backup {
        fs::rename(&backup, &path).with_context(|| {
            format!(
                "cgen hook was removed, but restoring {} failed",
                backup.display()
            )
        })?;
    }
    Ok(path)
}

/// Generate a message for a normal commit. Git source arguments indicate
/// amend/merge/squash/template flows and are intentionally left untouched.
pub fn run(message_file: &Path, source: Option<&str>, cfg: &AppConfig) -> Result<()> {
    if source.is_some() {
        return Ok(());
    }
    let original = fs::read_to_string(message_file)
        .with_context(|| format!("Failed to read {}", message_file.display()))?;
    if contains_real_message(&original, git_comment_char()) {
        return Ok(());
    }
    if provider::provider_requires_api_key(cfg) && cfg.api_key.is_empty() {
        anyhow::bail!("No API key configured");
    }

    let staged_files = git::list_staged_files()?;
    let diff = git::get_staged_diff_filtered(&[], &cfg.diff_exclude_globs)?;
    workflow::enforce_diff_safety(cfg, &diff, &staged_files, false, false)?;
    let candidates =
        generation::generate_candidates(cfg, &diff, 1, None, provider::OutputMode::Quiet)?;
    let message = generation::apply_template(
        cfg,
        &candidates
            .into_iter()
            .next()
            .expect("one candidate was requested")
            .message,
    )?;
    let replacement = if original.is_empty() {
        format!("{message}\n")
    } else {
        format!("{message}\n\n{original}")
    };
    crate::persistence::atomic_write(message_file, replacement.as_bytes())
        .with_context(|| format!("Failed to update {}", message_file.display()))
}

fn is_managed(content: &str) -> bool {
    content.starts_with(&format!("#!/bin/sh\n{MARKER}\n{BACKUP_MARKER}"))
        && content.contains("\nhook_dir=")
        && content.contains(" hook run \"$@\"")
        && content.contains("warning: cgen could not generate a commit message")
}

fn backup_name_from_wrapper(content: &str) -> Option<String> {
    content
        .lines()
        .find_map(|line| line.strip_prefix(BACKUP_MARKER))
        .filter(|name| {
            name.starts_with("prepare-commit-msg.cgen-backup")
                && !name.contains('/')
                && !name.contains('\\')
        })
        .map(ToOwned::to_owned)
}

fn collision_free_backup(hook: &Path) -> PathBuf {
    let base = hook.with_file_name("prepare-commit-msg.cgen-backup");
    if !base.exists() {
        return base;
    }
    for index in 1usize.. {
        let candidate = hook.with_file_name(format!("prepare-commit-msg.cgen-backup.{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn build_wrapper(executable: &Path, backup_name: Option<&str>) -> String {
    let backup_marker = backup_name.unwrap_or("");
    let chained_hook = backup_name.map_or_else(String::new, |name| {
        format!(
            "\"$hook_dir/{name}\" \"$@\"\noriginal_status=$?\nif [ \"$original_status\" -ne 0 ]; then\n  exit \"$original_status\"\nfi\n"
        )
    });
    format!(
        "#!/bin/sh\n{MARKER}\n{BACKUP_MARKER}{backup_marker}\nhook_dir=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\n{chained_hook}if ! {} hook run \"$@\"; then\n  printf '%s\\n' 'warning: cgen could not generate a commit message; continuing' >&2\nfi\nexit 0\n",
        shell_quote(&executable.to_string_lossy())
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn contains_real_message(content: &str, comment_char: char) -> bool {
    content.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && !trimmed.starts_with(comment_char)
    })
}

fn git_comment_char() -> char {
    Command::new("git")
        .args(["config", "--get", "core.commentChar"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|value| {
            let value = value.trim();
            if value.eq_ignore_ascii_case("auto") {
                None
            } else {
                value.chars().next()
            }
        })
        .unwrap_or('#')
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(permissions.mode() | 0o111);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_message_detection_ignores_comments_and_blank_lines() {
        assert!(!contains_real_message("\n# status\n# more\n", '#'));
        assert!(contains_real_message("\nfeat: supplied\n# status\n", '#'));
        assert!(!contains_real_message("\n; status\n", ';'));
    }

    #[test]
    fn wrapper_contains_chain_before_cgen() {
        let wrapper = build_wrapper(Path::new("/tmp/cgen"), Some("prepare-commit-msg.old"));
        let old = wrapper.find("prepare-commit-msg.old").unwrap();
        let cgen = wrapper.find("hook run").unwrap();
        assert!(old < cgen);
        assert!(is_managed(&wrapper));
    }
}
