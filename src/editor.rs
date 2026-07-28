use anyhow::{Context, Result};
use std::io::Write;
use std::process::Command;

/// Edit text using Git's editor precedence without shell evaluation.
pub fn edit(initial: &str) -> Result<String> {
    let mut file = tempfile::Builder::new()
        .prefix("cgen-message-")
        .suffix(".txt")
        .tempfile()
        .context("Failed to create commit message editor file")?;
    file.write_all(initial.as_bytes())
        .context("Failed to prepare commit message editor file")?;
    file.flush()
        .context("Failed to flush commit message editor file")?;

    let editor = ["GIT_EDITOR", "VISUAL", "EDITOR"]
        .into_iter()
        .find_map(|name| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(default_editor);
    let mut parts =
        shlex::split(&editor).with_context(|| format!("Invalid editor command: {editor}"))?;
    if parts.is_empty() {
        anyhow::bail!("Editor command cannot be empty");
    }
    let program = parts.remove(0);
    let status = Command::new(&program)
        .args(parts)
        .arg(file.path())
        .status()
        .with_context(|| format!("Failed to launch editor '{program}'"))?;
    if !status.success() {
        anyhow::bail!("Editor '{program}' exited with status {status}");
    }

    std::fs::read_to_string(file.path()).context("Failed to read edited commit message")
}

#[cfg(windows)]
fn default_editor() -> String {
    "notepad".into()
}

#[cfg(not(windows))]
fn default_editor() -> String {
    "vi".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_editor_is_not_empty() {
        assert!(!default_editor().is_empty());
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial]
    fn edit_returns_editor_output() {
        let _guard = GitEditorGuard::set(r#"sh -c 'printf "edited message\n" > "$1"' cgen-editor"#);

        assert_eq!(edit("initial message\n").unwrap(), "edited message\n");
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial]
    fn edit_reports_editor_failure() {
        let _guard = GitEditorGuard::set("sh -c 'exit 7' cgen-editor");

        let error = edit("initial message\n").unwrap_err().to_string();
        assert!(error.contains("exited with status"));
    }

    #[test]
    #[serial_test::serial]
    fn edit_rejects_invalid_editor_command() {
        let _guard = GitEditorGuard::set("'");

        let error = edit("initial message\n").unwrap_err().to_string();
        assert!(error.contains("Invalid editor command"));
    }

    struct GitEditorGuard(Option<std::ffi::OsString>);

    impl GitEditorGuard {
        fn set(value: &str) -> Self {
            let original = std::env::var_os("GIT_EDITOR");
            std::env::set_var("GIT_EDITOR", value);
            Self(original)
        }
    }

    impl Drop for GitEditorGuard {
        fn drop(&mut self) {
            if let Some(value) = self.0.take() {
                std::env::set_var("GIT_EDITOR", value);
            } else {
                std::env::remove_var("GIT_EDITOR");
            }
        }
    }
}
