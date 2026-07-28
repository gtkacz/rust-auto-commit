use anyhow::{bail, Context, Result};
use glob::Pattern;
use regex_lite::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// Stage modifications and deletions to tracked files, matching `git commit -a`
/// without adding untracked paths.
pub fn stage_tracked_changes() -> Result<()> {
    let output = Command::new("git")
        .args(["add", "--update"])
        .output()
        .context("Failed to run git add --update")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git add --update failed: {stderr}");
    }
    Ok(())
}

/// Get the output of `git diff --staged`
pub fn get_staged_diff() -> Result<String> {
    let output = Command::new("git")
        .args([
            "-c",
            "core.quotePath=false",
            "diff",
            "--staged",
            "--no-ext-diff",
        ])
        .output()
        .context("Failed to run git diff --staged")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git diff --staged failed: {stderr}");
    }

    let diff = String::from_utf8_lossy(&output.stdout).to_string();

    if diff.trim().is_empty() {
        bail!(
            "No staged changes found. Stage files with {} first.",
            colored::Colorize::yellow("git add <files>")
        );
    }

    Ok(diff)
}

/// List staged file paths
pub fn list_staged_files() -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["diff", "--staged", "--name-only", "-z"])
        .output()
        .context("Failed to run git diff --staged --name-only")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git diff --staged --name-only failed: {stderr}");
    }

    let files = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect::<Vec<_>>();

    Ok(files)
}

/// Find the git repository root directory
pub fn find_repo_root() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("Failed to run git rev-parse")?;

    if !output.status.success() {
        bail!("Not in a git repository");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Run `git commit -m "<message>" [extra_args...]`
pub fn run_commit(message: &str, extra_args: &[String], suppress_output: bool) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.args(["commit", "-m", message]);
    cmd.args(extra_args);
    configure_stdio(&mut cmd, suppress_output);
    let status = cmd.status().context("Failed to run git commit")?;

    if !status.success() {
        bail!("git commit exited with status {status}");
    }

    Ok(())
}

/// Run `git push`
pub fn run_push(suppress_output: bool) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.arg("push");
    configure_stdio(&mut cmd, suppress_output);

    let status = cmd.status().context("Failed to run git push")?;
    if !status.success() {
        bail!("git push exited with status {status}");
    }

    Ok(())
}

pub fn run_force_push_with_lease(suppress_output: bool) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.args(["push", "--force-with-lease"]);
    configure_stdio(&mut cmd, suppress_output);
    let status = cmd
        .status()
        .context("Failed to run git push --force-with-lease")?;
    if !status.success() {
        bail!("git push --force-with-lease exited with status {status}");
    }
    Ok(())
}

pub fn push_tag(tag_name: &str, suppress_output: bool) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.args(["push", "origin", &format!("refs/tags/{tag_name}")]);
    configure_stdio(&mut cmd, suppress_output);
    let status = cmd.status().context("Failed to push git tag")?;
    if !status.success() {
        bail!("Branch push succeeded, but tag '{tag_name}' remains local (tag push exited with status {status})");
    }
    Ok(())
}

/// Returns the latest tag according to git version sorting.
pub fn get_latest_tag() -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["tag", "--sort=-version:refname"])
        .output()
        .context("Failed to run git tag --sort=-version:refname")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git tag --sort=-version:refname failed: {stderr}");
    }

    let latest = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string);

    Ok(latest)
}

/// Compute the next minor semver tag from latest tag.
pub fn compute_next_minor_tag(latest: Option<&str>) -> Result<String> {
    let Some(latest_tag) = latest else {
        return Ok("0.1.0".to_string());
    };

    let (major, minor, _patch) = parse_semver_tag(latest_tag)?;
    Ok(format!("{major}.{}.0", minor + 1))
}

/// Create a git lightweight tag.
pub fn create_tag(tag_name: &str, suppress_output: bool) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.args(["tag", tag_name]);
    configure_stdio(&mut cmd, suppress_output);
    let status = cmd.status().context("Failed to run git tag")?;

    if !status.success() {
        bail!("git tag exited with status {status}");
    }

    Ok(())
}

/// Returns true when HEAD exists on upstream branch
pub fn is_head_pushed() -> Result<bool> {
    if !has_upstream_branch()? {
        return Ok(false);
    }

    let output = Command::new("git")
        .args(["branch", "-r", "--contains", "HEAD"])
        .output()
        .context("Failed to determine whether HEAD is pushed")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git branch -r --contains HEAD failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pushed = stdout
        .lines()
        .map(str::trim)
        .any(|line| !line.is_empty() && !line.contains("->"));
    Ok(pushed)
}

/// Returns true if HEAD has multiple parents
pub fn head_is_merge_commit() -> Result<bool> {
    ensure_head_exists()?;

    let output = Command::new("git")
        .args(["rev-list", "--parents", "-n", "1", "HEAD"])
        .output()
        .context("Failed to inspect latest commit parents")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git rev-list --parents -n 1 HEAD failed: {stderr}");
    }

    let parent_count = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .count()
        .saturating_sub(1);
    Ok(parent_count > 1)
}

/// Undo latest commit, keep all changes staged
pub fn undo_last_commit_soft(suppress_output: bool) -> Result<()> {
    ensure_head_exists()?;

    let mut cmd = Command::new("git");
    let has_parent = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD^"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to inspect HEAD parent")?
        .success();
    if has_parent {
        cmd.args(["reset", "--soft", "HEAD~1"]);
    } else {
        cmd.args(["update-ref", "-d", "HEAD"]);
    }
    configure_stdio(&mut cmd, suppress_output);

    let status = cmd.status().context("Failed to undo latest commit")?;
    if !status.success() {
        bail!("Undoing the latest commit exited with status {status}");
    }
    Ok(())
}

pub fn has_upstream_branch() -> Result<bool> {
    let status = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to detect upstream branch")?;
    Ok(status.success())
}

pub fn ensure_head_exists() -> Result<()> {
    let status = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to run git rev-parse --verify HEAD")?;

    if !status.success() {
        bail!("No commits found in this repository.");
    }
    Ok(())
}

fn configure_stdio(cmd: &mut Command, suppress_output: bool) {
    if suppress_output {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
}

pub fn ensure_commit_exists(commit: &str) -> Result<()> {
    let status = Command::new("git")
        .args(["rev-parse", "--verify", &format!("{commit}^{{commit}}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("Failed to verify commit reference {commit}"))?;

    if !status.success() {
        bail!("Commit reference not found: {commit}");
    }
    Ok(())
}

pub fn get_commit_diff(commit: &str) -> Result<String> {
    ensure_commit_exists(commit)?;
    let output = Command::new("git")
        .args([
            "-c",
            "core.quotePath=false",
            "show",
            "--format=",
            "--no-color",
            "--no-ext-diff",
            commit,
        ])
        .output()
        .with_context(|| format!("Failed to run git show for {commit}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git show failed for {commit}: {stderr}");
    }

    let diff = String::from_utf8_lossy(&output.stdout).to_string();
    if diff.trim().is_empty() {
        bail!("Selected commit has no diff to analyze: {commit}");
    }
    Ok(diff)
}

pub fn get_range_diff(older: &str, newer: &str) -> Result<String> {
    ensure_commit_exists(older)?;
    ensure_commit_exists(newer)?;

    let output = Command::new("git")
        .args([
            "-c",
            "core.quotePath=false",
            "diff",
            "--no-color",
            "--no-ext-diff",
            older,
            newer,
        ])
        .output()
        .with_context(|| format!("Failed to run git diff {older} {newer}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git diff failed for {older}..{newer}: {stderr}");
    }

    let diff = String::from_utf8_lossy(&output.stdout).to_string();
    if diff.trim().is_empty() {
        bail!("No diff found for range {older}..{newer}");
    }
    Ok(diff)
}

pub fn is_head_commit(commit: &str) -> Result<bool> {
    ensure_commit_exists(commit)?;
    Ok(resolve_commit("HEAD")? == resolve_commit(commit)?)
}

pub fn commit_is_merge(commit: &str) -> Result<bool> {
    ensure_commit_exists(commit)?;
    let output = Command::new("git")
        .args(["rev-list", "--parents", "-n", "1", commit])
        .output()
        .with_context(|| format!("Failed to inspect parents for {commit}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git rev-list failed for {commit}: {stderr}");
    }

    let parent_count = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .count()
        .saturating_sub(1);
    Ok(parent_count > 1)
}

pub fn commit_is_pushed(commit: &str) -> Result<bool> {
    ensure_commit_exists(commit)?;
    if !has_upstream_branch()? {
        return Ok(false);
    }

    let output = Command::new("git")
        .args(["branch", "-r", "--contains", commit])
        .output()
        .with_context(|| format!("Failed to determine whether {commit} is pushed"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git branch -r --contains {commit} failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .map(str::trim)
        .any(|line| !line.is_empty() && !line.contains("->")))
}

pub fn rewrite_commit_message(
    target: &str,
    message: &str,
    suppress_output: bool,
) -> Result<String> {
    ensure_commit_exists(target)?;
    let distance = commits_after(target)?;

    if is_head_commit(target)? {
        let mut cmd = Command::new("git");
        cmd.args(["commit", "--amend", "-m", message]);
        configure_stdio(&mut cmd, suppress_output);
        let status = cmd.status().context("Failed to run git commit --amend")?;
        if !status.success() {
            bail!("git commit --amend exited with status {status}");
        }
        return resolve_commit("HEAD");
    }

    if commit_is_merge(target)? {
        bail!("Altering non-HEAD merge commits is not supported.");
    }

    ensure_ancestor_of_head(target)?;
    reword_non_head_commit(target, message, suppress_output)?;
    resolve_commit(&format!("HEAD~{distance}"))
}

fn commits_after(target: &str) -> Result<usize> {
    let output = Command::new("git")
        .args(["rev-list", "--count", &format!("{target}..HEAD")])
        .output()
        .context("Failed to determine rewritten commit position")?;
    if !output.status.success() {
        bail!(
            "Failed to determine rewritten commit position: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .context("Git returned an invalid commit distance")
}

fn resolve_commit(commit: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", &format!("{commit}^{{commit}}")])
        .output()
        .with_context(|| format!("Failed to resolve commit {commit}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to resolve commit {commit}: {stderr}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn ensure_ancestor_of_head(commit: &str) -> Result<()> {
    let status = Command::new("git")
        .args(["merge-base", "--is-ancestor", commit, "HEAD"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("Failed to check whether {commit} is an ancestor of HEAD"))?;

    if !status.success() {
        bail!("Target commit must be on the current branch and reachable from HEAD.");
    }
    Ok(())
}

fn reword_non_head_commit(target: &str, message: &str, suppress_output: bool) -> Result<()> {
    let temp = tempfile::tempdir().context("Failed to create rebase helper directory")?;
    let sequence_editor = temp.path().join("sequence-editor.sh");
    let message_editor = temp.path().join("message-editor.sh");

    write_sequence_editor_script(&sequence_editor)?;
    write_message_editor_script(&message_editor)?;

    let mut cmd = Command::new("git");
    let has_parent = Command::new("git")
        .args(["rev-parse", "--verify", &format!("{target}^")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to inspect target parent")?
        .success();
    if has_parent {
        cmd.args(["rebase", "-i", &format!("{target}^")]);
    } else {
        cmd.args(["rebase", "-i", "--root"]);
    }
    cmd.env("GIT_SEQUENCE_EDITOR", script_command(&sequence_editor));
    cmd.env("GIT_EDITOR", script_command(&message_editor));
    cmd.env("CGEN_NEW_MESSAGE", message);
    configure_stdio(&mut cmd, suppress_output);

    let status = cmd.status().context("Failed to run git rebase -i")?;

    if !status.success() {
        bail!(
            "Rewriting commit message failed during rebase. Resolve conflicts and run `git rebase --abort` if needed."
        );
    }
    Ok(())
}

fn write_sequence_editor_script(path: &Path) -> Result<()> {
    let script = r#"#!/bin/sh
set -e
todo="$1"
tmp="${todo}.cgen"
first=1

while IFS= read -r line; do
  if [ "$first" -eq 1 ] && printf '%s\n' "$line" | grep -q '^pick '; then
    printf '%s\n' "$line" | sed 's/^pick /reword /' >> "$tmp"
    first=0
  else
    printf '%s\n' "$line" >> "$tmp"
  fi
done < "$todo"

mv "$tmp" "$todo"
"#;
    fs::write(path, script).with_context(|| format!("Failed to write {:?}", path))?;
    make_executable(path)?;
    Ok(())
}

fn write_message_editor_script(path: &Path) -> Result<()> {
    let script = r#"#!/bin/sh
set -e
msg_file="$1"
printf '%s\n' "$CGEN_NEW_MESSAGE" > "$msg_file"
"#;
    fs::write(path, script).with_context(|| format!("Failed to write {:?}", path))?;
    make_executable(path)?;
    Ok(())
}

fn make_executable(path: &Path) -> Result<()> {
    #[cfg(not(unix))]
    let _ = path;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

fn script_command(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn parse_semver_tag(tag: &str) -> Result<(u64, u64, u64)> {
    let parts: Vec<&str> = tag.trim().split('.').collect();
    if parts.len() != 3 {
        bail!("Latest tag '{tag}' is not valid semantic versioning (expected MAJOR.MINOR.PATCH).");
    }

    let major = parts[0]
        .parse::<u64>()
        .with_context(|| format!("Latest tag '{tag}' is not valid semantic versioning."))?;
    let minor = parts[1]
        .parse::<u64>()
        .with_context(|| format!("Latest tag '{tag}' is not valid semantic versioning."))?;
    let patch = parts[2]
        .parse::<u64>()
        .with_context(|| format!("Latest tag '{tag}' is not valid semantic versioning."))?;

    Ok((major, minor, patch))
}

/// Filter a unified diff. A file's hunk is kept when it matches an include glob
/// (allow-over-deny) OR does not match any exclude glob. Patterns containing a
/// slash match repository-relative paths; basename patterns retain the
/// historical `*.ext` behavior. Excluded files are still committed.
pub fn filter_diff_by_globs(
    diff: &str,
    include_patterns: &[String],
    exclude_patterns: &[String],
) -> Result<String> {
    let includes = compile_globs(include_patterns)?;
    let excludes = compile_globs(exclude_patterns)?;

    if includes.is_empty() && excludes.is_empty() {
        if diff.trim().is_empty() {
            bail!("Diff is empty");
        }
        return Ok(diff.to_string());
    }

    let mut result = String::new();
    let mut include_current = true;
    let mut saw_header = false;

    for line in diff.split_inclusive('\n') {
        if line.starts_with("diff --git ") {
            saw_header = true;
            let file_path = diff_header_path(line)?;
            // Include wins over exclude (gitignore-`!` semantics).
            let allowed = includes
                .iter()
                .any(|pattern| path_matches(pattern, &file_path));
            let denied = excludes
                .iter()
                .any(|pattern| path_matches(pattern, &file_path));
            include_current = allowed || !denied;
        }

        if include_current {
            result.push_str(line);
        }
    }

    if !saw_header {
        bail!("Could not parse diff: no 'diff --git' file headers found");
    }
    if result.trim().is_empty() {
        bail!(
            "All changed files were excluded from LLM analysis. Adjust diff globs or use --diff-include."
        );
    }
    Ok(result)
}

fn compile_globs(patterns: &[String]) -> Result<Vec<Pattern>> {
    patterns
        .iter()
        .map(|pattern| {
            Pattern::new(pattern).with_context(|| format!("Invalid diff glob '{pattern}'"))
        })
        .collect()
}

fn diff_header_path(line: &str) -> Result<String> {
    let rest = line
        .trim_end_matches(['\r', '\n'])
        .strip_prefix("diff --git ")
        .context("Invalid diff header")?;
    if rest.starts_with('"') {
        let tokens = shlex::split(rest).context("Invalid quoted path in diff header")?;
        let path = tokens
            .get(1)
            .context("Diff header is missing its destination path")?;
        return path
            .strip_prefix("b/")
            .map(str::to_string)
            .context("Diff destination path is missing the 'b/' prefix");
    }
    let (_, destination) = rest
        .rsplit_once(" b/")
        .context("Diff header is missing its destination path")?;
    Ok(destination.to_string())
}

fn path_matches(pattern: &Pattern, path: &str) -> bool {
    if pattern.matches_path(Path::new(path)) {
        return true;
    }
    !pattern.as_str().contains('/')
        && Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| pattern.matches(name))
            .unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSafetyReport {
    pub bytes: usize,
    pub included_files: Vec<String>,
    pub omitted_files: Vec<String>,
    pub sensitive_files: Vec<String>,
    pub secret_findings: Vec<String>,
}

pub fn assess_diff_safety(
    diff: &str,
    all_files: &[String],
    sensitive_patterns: &[String],
) -> Result<DiffSafetyReport> {
    let patterns = compile_globs(sensitive_patterns)?;
    let included_files = diff_paths(diff)?;
    let included: HashSet<&str> = included_files.iter().map(String::as_str).collect();
    let omitted_files = all_files
        .iter()
        .filter(|path| !included.contains(path.as_str()))
        .cloned()
        .collect();
    let sensitive_files = included_files
        .iter()
        .filter(|path| patterns.iter().any(|pattern| path_matches(pattern, path)))
        .cloned()
        .collect();
    let secret_findings = detect_secret_findings(diff);
    Ok(DiffSafetyReport {
        bytes: diff.len(),
        included_files,
        omitted_files,
        sensitive_files,
        secret_findings,
    })
}

pub fn diff_paths(diff: &str) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    for line in diff.lines().filter(|line| line.starts_with("diff --git ")) {
        let path = diff_header_path(line)?;
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        bail!("Could not determine changed files from diff");
    }
    Ok(paths)
}

fn detect_secret_findings(diff: &str) -> Vec<String> {
    static HIGH_CONFIDENCE: OnceLock<Regex> = OnceLock::new();
    static ASSIGNMENT: OnceLock<Regex> = OnceLock::new();
    let high_confidence = HIGH_CONFIDENCE.get_or_init(|| {
        Regex::new(
            r"(?i)(-----BEGIN [A-Z ]*PRIVATE KEY-----|AKIA[A-Z0-9]{16}|gh[pousr]_[A-Za-z0-9]{20,}|sk-[A-Za-z0-9_-]{20,})",
        )
        .unwrap()
    });
    let assignment = ASSIGNMENT.get_or_init(|| {
        Regex::new(
            r#"(?i)(api[_-]?key|access[_-]?token|client[_-]?secret|password)\s*[:=]\s*["']?[A-Za-z0-9_./+=-]{12,}"#,
        )
        .unwrap()
    });
    let mut findings: Vec<String> = Vec::new();
    let mut path = None;
    let mut new_line = None;
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            path = diff_header_path(line).ok();
            new_line = None;
            continue;
        }
        if let Some(hunk) = line.strip_prefix("@@ ") {
            new_line = hunk
                .split_whitespace()
                .find_map(|part| part.strip_prefix('+'))
                .and_then(|part| {
                    part.split_once(',')
                        .map_or(Some(part), |(start, _)| Some(start))
                })
                .and_then(|start| start.parse::<usize>().ok());
            continue;
        }
        if line.starts_with(' ') {
            if let Some(line_number) = &mut new_line {
                *line_number += 1;
            }
            continue;
        }
        if !line.starts_with('+') || line.starts_with("+++") {
            continue;
        }

        let lower = line.to_ascii_lowercase();
        if lower.contains("$acr_")
            || lower.contains("example")
            || lower.contains("placeholder")
            || lower.contains("[redacted]")
            || lower.contains("your-key-here")
        {
            if let Some(line_number) = &mut new_line {
                *line_number += 1;
            }
            continue;
        }
        let location = match (&path, new_line) {
            (Some(path), Some(line_number)) => format!("{path}:{line_number}"),
            (Some(path), None) => path.clone(),
            (None, _) => "unknown location".to_string(),
        };
        if high_confidence.is_match(line) {
            if !findings
                .iter()
                .any(|finding| finding.starts_with("credential-like token or private key"))
            {
                findings.push(format!("credential-like token or private key ({location})"));
            }
        } else if assignment.is_match(line)
            && !findings
                .iter()
                .any(|finding| finding.starts_with("secret-like assignment"))
        {
            findings.push(format!("secret-like assignment ({location})"));
        }
        if let Some(line_number) = &mut new_line {
            *line_number += 1;
        }
    }
    findings
}

/// Get staged diff with files filtered by include/exclude globs.
/// Excluded files are still committed, just not sent to the LLM for analysis.
pub fn get_staged_diff_filtered(
    include_patterns: &[String],
    exclude_patterns: &[String],
) -> Result<String> {
    let diff = get_staged_diff()?;
    filter_diff_by_globs(&diff, include_patterns, exclude_patterns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_semver_tag_valid() {
        let (major, minor, patch) = parse_semver_tag("1.2.3").unwrap();
        assert_eq!((major, minor, patch), (1, 2, 3));
    }

    #[test]
    fn test_parse_semver_tag_zeros() {
        let (major, minor, patch) = parse_semver_tag("0.0.0").unwrap();
        assert_eq!((major, minor, patch), (0, 0, 0));
    }

    #[test]
    fn test_parse_semver_tag_large_numbers() {
        let (major, minor, patch) = parse_semver_tag("100.200.300").unwrap();
        assert_eq!((major, minor, patch), (100, 200, 300));
    }

    #[test]
    fn test_parse_semver_tag_invalid_format_two_parts() {
        let result = parse_semver_tag("1.2");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not valid semantic"));
    }

    #[test]
    fn test_parse_semver_tag_invalid_format_four_parts() {
        let result = parse_semver_tag("1.2.3.4");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_semver_tag_non_numeric_major() {
        let result = parse_semver_tag("a.2.3");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_semver_tag_non_numeric_minor() {
        let result = parse_semver_tag("1.b.3");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_semver_tag_non_numeric_patch() {
        let result = parse_semver_tag("1.2.c");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_semver_tag_with_v_prefix() {
        // v prefix is not supported - this should fail
        let result = parse_semver_tag("v1.2.3");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_semver_tag_with_whitespace() {
        // Whitespace is trimmed
        let (major, minor, patch) = parse_semver_tag("  1.2.3  ").unwrap();
        assert_eq!((major, minor, patch), (1, 2, 3));
    }

    #[test]
    fn test_script_command_unix_path() {
        let path = std::path::Path::new("/tmp/script.sh");
        let result = script_command(path);
        assert_eq!(result, "/tmp/script.sh");
    }

    #[test]
    fn test_script_command_windows_path() {
        let path = std::path::Path::new("C:\\Users\\test\\script.sh");
        let result = script_command(path);
        // Backslashes should be converted to forward slashes
        assert!(result.contains('/'));
        assert!(!result.contains('\\'));
    }

    #[test]
    fn test_configure_stdio_suppress() {
        let mut cmd = Command::new("echo");
        configure_stdio(&mut cmd, true);
        // Can't easily verify the result, but ensure it doesn't panic
    }

    #[test]
    fn test_configure_stdio_no_suppress() {
        let mut cmd = Command::new("echo");
        configure_stdio(&mut cmd, false);
        // Can't easily verify the result, but ensure it doesn't panic
    }

    #[test]
    fn test_filter_diff_all_invalid_patterns_are_rejected() {
        let diff = "diff --git a/test.rs b/test.rs\n+code\n";
        let patterns = vec!["[invalid".to_string(), "[also[bad".to_string()];
        assert!(filter_diff_by_globs(diff, &[], &patterns).is_err());
    }

    #[test]
    fn test_filter_diff_empty_diff() {
        let patterns = vec!["*.json".to_string()];
        assert!(filter_diff_by_globs("", &[], &patterns).is_err());
    }

    #[test]
    fn test_filter_diff_no_diff_header() {
        // Content without proper diff header
        let content = "just some random text\nwithout diff headers";
        let patterns = vec!["*.json".to_string()];
        assert!(filter_diff_by_globs(content, &[], &patterns).is_err());
    }

    #[test]
    fn test_filter_diff_malformed_header() {
        // Diff header without proper format
        let diff = "diff --git \n+something\n";
        let patterns = vec!["*.json".to_string()];
        assert!(filter_diff_by_globs(diff, &[], &patterns).is_err());
    }

    #[test]
    fn test_compute_next_minor_tag_increment() {
        let result = compute_next_minor_tag(Some("2.5.9")).unwrap();
        assert_eq!(result, "2.6.0");
    }

    #[test]
    fn test_compute_next_minor_tag_none() {
        let result = compute_next_minor_tag(None).unwrap();
        assert_eq!(result, "0.1.0");
    }

    #[test]
    fn test_make_executable_windows() {
        // On Windows, make_executable is a no-op
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_make_executable.sh");
        std::fs::write(&test_file, "#!/bin/sh\necho test").unwrap();
        let result = make_executable(&test_file);
        assert!(result.is_ok());
        let _ = std::fs::remove_file(&test_file);
    }

    #[test]
    fn test_write_sequence_editor_script() {
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join("test_seq_editor.sh");
        let result = write_sequence_editor_script(&script_path);
        assert!(result.is_ok());
        let content = std::fs::read_to_string(&script_path).unwrap();
        assert!(content.contains("#!/bin/sh"));
        assert!(content.contains("reword"));
        let _ = std::fs::remove_file(&script_path);
    }

    #[test]
    fn test_write_message_editor_script() {
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join("test_msg_editor.sh");
        let result = write_message_editor_script(&script_path);
        assert!(result.is_ok());
        let content = std::fs::read_to_string(&script_path).unwrap();
        assert!(content.contains("#!/bin/sh"));
        assert!(content.contains("CGEN_NEW_MESSAGE"));
        let _ = std::fs::remove_file(&script_path);
    }

    #[test]
    fn test_filter_diff_single_file_excluded() {
        let diff = "diff --git a/config.json b/config.json\n+{}\n";
        let patterns = vec!["*.json".to_string()];
        let error = filter_diff_by_globs(diff, &[], &patterns).unwrap_err();
        assert!(error.to_string().contains("All changed files"));
    }

    #[test]
    fn test_filter_diff_preserves_context_lines() {
        let diff = r#"diff --git a/main.rs b/main.rs
--- a/main.rs
+++ b/main.rs
@@ -1,5 +1,6 @@
 fn main() {
+    println!("new");
     old_code();
 }
"#;
        let patterns = vec!["*.json".to_string()]; // Won't match .rs
        let filtered = filter_diff_by_globs(diff, &[], &patterns).unwrap();
        assert!(filtered.contains("fn main()"));
        assert!(filtered.contains("println!"));
        assert!(filtered.contains("old_code"));
    }

    #[test]
    fn test_filter_diff_consecutive_files() {
        let diff = r#"diff --git a/a.json b/a.json
+first
diff --git a/b.json b/b.json
+second
diff --git a/c.rs b/c.rs
+third
"#;
        let patterns = vec!["*.json".to_string()];
        let filtered = filter_diff_by_globs(diff, &[], &patterns).unwrap();
        assert!(!filtered.contains("first"));
        assert!(!filtered.contains("second"));
        assert!(filtered.contains("third"));
    }

    #[test]
    fn test_parse_semver_tag_empty() {
        let result = parse_semver_tag("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_semver_tag_single_number() {
        let result = parse_semver_tag("1");
        assert!(result.is_err());
    }

    #[test]
    fn test_script_command_empty_path() {
        let path = std::path::Path::new("");
        let result = script_command(path);
        assert_eq!(result, "");
    }

    #[test]
    fn test_script_command_relative_path() {
        let path = std::path::Path::new("scripts/test.sh");
        let result = script_command(path);
        assert_eq!(result, "scripts/test.sh");
    }

    #[test]
    fn test_filter_diff_mixed_content_types() {
        let diff = r#"diff --git a/readme.md b/readme.md
--- a/readme.md
+++ b/readme.md
@@ -1 +1,2 @@
+Documentation
diff --git a/package-lock.json b/package-lock.json
--- a/package-lock.json
+++ b/package-lock.json
@@ -1 +1,2 @@
+{"deps": true}
diff --git a/src/app.ts b/src/app.ts
--- a/src/app.ts
+++ b/src/app.ts
@@ -1 +1,2 @@
+// TypeScript code
"#;
        let patterns = vec!["*.json".to_string()];
        let filtered = filter_diff_by_globs(diff, &[], &patterns).unwrap();

        assert!(filtered.contains("readme.md"));
        assert!(filtered.contains("Documentation"));
        assert!(!filtered.contains("package-lock.json"));
        assert!(filtered.contains("src/app.ts"));
        assert!(filtered.contains("TypeScript code"));
    }

    #[test]
    fn test_filter_diff_special_characters_in_path() {
        let diff = "diff --git a/path with spaces/file.rs b/path with spaces/file.rs\n+code\n";
        let patterns = vec!["*.json".to_string()];
        let filtered = filter_diff_by_globs(diff, &[], &patterns).unwrap();
        assert!(filtered.contains("code"));
    }

    #[test]
    fn test_compute_next_minor_tag_large_version() {
        let result = compute_next_minor_tag(Some("99.999.0")).unwrap();
        assert_eq!(result, "99.1000.0");
    }

    #[test]
    fn test_compute_next_minor_tag_error_propagation() {
        // Invalid semver should return error
        let result = compute_next_minor_tag(Some("not-semver"));
        assert!(result.is_err());
    }

    #[test]
    fn test_filter_diff_include_overrides_exclude() {
        let diff = "diff --git a/data.xml b/data.xml\n+<root/>\n";
        let include = vec!["*.xml".to_string()];
        let exclude = vec!["*.xml".to_string()];
        let filtered = filter_diff_by_globs(diff, &include, &exclude).unwrap();
        assert!(filtered.contains("data.xml"));
        assert!(filtered.contains("<root/>"));
    }

    #[test]
    fn test_filter_diff_extra_exclude_drops_file() {
        let diff = "diff --git a/schema.sql b/schema.sql\n+CREATE TABLE x;\n";
        let exclude = vec!["*.sql".to_string()];
        let error = filter_diff_by_globs(diff, &[], &exclude).unwrap_err();
        assert!(error.to_string().contains("All changed files"));
    }

    #[test]
    fn test_filter_diff_empty_include_matches_legacy_behavior() {
        let diff = "diff --git a/a.json b/a.json\n+{}\ndiff --git a/b.rs b/b.rs\n+code\n";
        let exclude = vec!["*.json".to_string()];
        let filtered = filter_diff_by_globs(diff, &[], &exclude).unwrap();
        assert!(!filtered.contains("a.json"));
        assert!(filtered.contains("b.rs"));
    }

    #[test]
    fn test_filter_diff_include_and_exclude_both_match_includes() {
        let diff = "diff --git a/keep.xml b/keep.xml\n+<a/>\n";
        let include = vec!["keep.xml".to_string()];
        let exclude = vec!["*.xml".to_string()];
        let filtered = filter_diff_by_globs(diff, &include, &exclude).unwrap();
        assert!(filtered.contains("keep.xml"));
    }

    #[test]
    fn test_filter_diff_matches_repository_relative_directory_glob() {
        let diff = "diff --git a/generated/private/data.txt b/generated/private/data.txt\n+secret\ndiff --git a/src/data.txt b/src/data.txt\n+safe\n";
        let excluded = vec!["generated/**".to_string()];
        let filtered = filter_diff_by_globs(diff, &[], &excluded).unwrap();
        assert!(!filtered.contains("generated/private"));
        assert!(filtered.contains("src/data.txt"));
    }

    #[test]
    fn test_filter_diff_parses_quoted_paths() {
        let diff =
            "diff --git \"a/path with spaces/data.json\" \"b/path with spaces/data.json\"\n+{}\n";
        let excluded = vec!["path with spaces/**".to_string()];
        assert!(filter_diff_by_globs(diff, &[], &excluded).is_err());
    }

    #[test]
    fn test_diff_safety_detects_sensitive_paths_and_secret_additions() {
        let diff = "diff --git a/.env b/.env\n@@ -0,0 +1 @@\n+API_KEY=abcdefghijklmnop1234\n";
        let files = vec![".env".to_string(), "src/lib.rs".to_string()];
        let patterns = vec![".env".to_string()];
        let report = assess_diff_safety(diff, &files, &patterns).unwrap();
        assert_eq!(report.sensitive_files, vec![".env"]);
        assert_eq!(report.omitted_files, vec!["src/lib.rs"]);
        assert!(report
            .secret_findings
            .iter()
            .any(|finding| finding == "secret-like assignment (.env:1)"));
    }

    #[test]
    fn test_diff_safety_ignores_documented_key_placeholder() {
        let diff =
            "diff --git a/README.md b/README.md\n@@ -1 +1 @@\n+export ACR_API_KEY=your-key-here\n";
        let report = assess_diff_safety(diff, &["README.md".to_string()], &[]).unwrap();
        assert!(report.secret_findings.is_empty());
    }
}
