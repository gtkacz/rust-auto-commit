use crate::{config::AppConfig, git};
use anyhow::Result;

/// Validate the exact filtered payload before any provider is called.
pub fn enforce_diff_safety(
    config: &AppConfig,
    diff: &str,
    all_files: &[String],
    allow_large_diff: bool,
    allow_sensitive: bool,
) -> Result<()> {
    let report = git::assess_diff_safety(diff, all_files, &config.sensitive_file_globs)?;
    if report.bytes > config.max_diff_bytes && !allow_large_diff {
        anyhow::bail!(
            "Filtered diff is {} bytes across {} file(s), exceeding max_diff_bytes={}. {} file(s) were omitted by filters. Review the file list and rerun with --allow-large-diff to send it.",
            report.bytes,
            report.included_files.len(),
            config.max_diff_bytes,
            report.omitted_files.len(),
        );
    }
    if (!report.sensitive_files.is_empty() || !report.secret_findings.is_empty())
        && !allow_sensitive
    {
        let files = if report.sensitive_files.is_empty() {
            "none".to_string()
        } else {
            report.sensitive_files.join(", ")
        };
        let findings = if report.secret_findings.is_empty() {
            "none".to_string()
        } else {
            report.secret_findings.join(", ")
        };
        anyhow::bail!(
            "Sensitive content blocked before the LLM request. Files: {files}. Findings: {findings}. Review the diff and rerun with --allow-sensitive only if disclosure is intended."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_diff_requires_explicit_override() {
        let config = AppConfig {
            max_diff_bytes: 10,
            ..Default::default()
        };
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n+long content\n";
        let files = vec!["src/lib.rs".to_string()];
        assert!(enforce_diff_safety(&config, diff, &files, false, false).is_err());
        enforce_diff_safety(&config, diff, &files, true, false).unwrap();
    }

    #[test]
    fn sensitive_diff_requires_explicit_override() {
        let config = AppConfig::default();
        let diff = "diff --git a/.env b/.env\n+TOKEN=abcdefghijklmnop\n";
        let files = vec![".env".to_string()];
        assert!(enforce_diff_safety(&config, diff, &files, false, false).is_err());
        enforce_diff_safety(&config, diff, &files, false, true).unwrap();
    }
}
