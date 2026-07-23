use crate::{config::AppConfig, git};
use anyhow::Result;

/// Validate the exact filtered payload before any provider is called and
/// return the assessment for downstream warnings.
pub fn enforce_diff_safety(
    config: &AppConfig,
    diff: &str,
    all_files: &[String],
    allow_large_diff: bool,
    allow_sensitive: bool,
) -> Result<git::DiffSafetyReport> {
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
    Ok(report)
}

/// Build the merged staged/LLM file-count warning, if any enabled threshold is
/// exceeded. LLM-payload files are the filtered subset actually sent to the
/// provider, so this is the token-relevant count.
pub fn staged_files_warning(
    config: &AppConfig,
    staged_count: usize,
    report: &git::DiffSafetyReport,
) -> Option<String> {
    let staged_exceeded =
        config.warn_staged_files_enabled && staged_count > config.warn_staged_files_threshold;
    let llm_exceeded = config.warn_llm_files_enabled
        && report.included_files.len() > config.warn_llm_files_threshold;
    if !staged_exceeded && !llm_exceeded {
        return None;
    }

    let mut reasons = Vec::new();
    if staged_exceeded {
        reasons.push(format!(
            "{} staged files (threshold {})",
            staged_count, config.warn_staged_files_threshold
        ));
    }
    if llm_exceeded {
        reasons.push(format!(
            "{} files in the LLM payload, ~{} KB (threshold {})",
            report.included_files.len(),
            report.bytes.div_ceil(1024),
            config.warn_llm_files_threshold
        ));
    }
    Some(format!(
        "You have {}. Continue with commit generation?",
        reasons.join(" and ")
    ))
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

    fn report_with(files: usize, bytes: usize) -> git::DiffSafetyReport {
        git::DiffSafetyReport {
            bytes,
            included_files: (0..files).map(|i| format!("src/file{i}.rs")).collect(),
            omitted_files: Vec::new(),
            sensitive_files: Vec::new(),
            secret_findings: Vec::new(),
        }
    }

    #[test]
    fn no_warning_when_thresholds_not_exceeded() {
        let config = AppConfig::default();
        assert!(staged_files_warning(&config, 20, &report_with(20, 1_000)).is_none());
    }

    #[test]
    fn warns_on_staged_count_alone() {
        let config = AppConfig {
            warn_llm_files_enabled: false,
            ..Default::default()
        };
        let message = staged_files_warning(&config, 25, &report_with(2, 1_000)).unwrap();
        assert!(message.contains("25 staged files (threshold 20)"));
        assert!(!message.contains("LLM payload"));
    }

    #[test]
    fn warns_on_llm_count_alone_with_size() {
        let config = AppConfig {
            warn_staged_files_enabled: false,
            warn_llm_files_threshold: 5,
            ..Default::default()
        };
        let message = staged_files_warning(&config, 6, &report_with(6, 3_000)).unwrap();
        assert!(message.contains("6 files in the LLM payload, ~3 KB (threshold 5)"));
        assert!(!message.contains("staged files (threshold"));
    }

    #[test]
    fn merges_both_reasons_into_one_prompt() {
        let config = AppConfig {
            warn_llm_files_threshold: 10,
            ..Default::default()
        };
        let message = staged_files_warning(&config, 30, &report_with(12, 10_240)).unwrap();
        assert!(message.contains(
            "30 staged files (threshold 20) and 12 files in the LLM payload, ~10 KB (threshold 10)"
        ));
    }

    #[test]
    fn disabled_warnings_never_fire() {
        let config = AppConfig {
            warn_staged_files_enabled: false,
            warn_llm_files_enabled: false,
            ..Default::default()
        };
        assert!(staged_files_warning(&config, 1_000, &report_with(1_000, 500_000)).is_none());
    }

    #[test]
    fn enforce_diff_safety_returns_report() {
        let config = AppConfig::default();
        let diff = "diff --git a/src/lib.rs b/src/lib.rs\n+content\n";
        let files = vec!["src/lib.rs".to_string(), "Cargo.lock".to_string()];
        let report = enforce_diff_safety(&config, diff, &files, false, false).unwrap();
        assert_eq!(report.included_files, vec!["src/lib.rs".to_string()]);
        assert_eq!(report.omitted_files, vec!["Cargo.lock".to_string()]);
        assert_eq!(report.bytes, diff.len());
    }
}
