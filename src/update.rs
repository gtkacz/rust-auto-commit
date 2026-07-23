use anyhow::{Context, Result};
use colored::Colorize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const GITHUB_REPO: &str = "gtkacz/smart-commit-rs";
const CRATE_NAME: &str = "auto-commit-rs";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_DOWNLOAD_BYTES: usize = 64 * 1024 * 1024;

pub struct VersionCheck {
    pub latest: String,
    pub current: String,
    pub update_available: bool,
}

/// Fetch the latest release tag from GitHub API with a short timeout.
pub fn fetch_latest_version() -> Result<String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .build();
    let response: serde_json::Value = agent
        .get(&url)
        .set("User-Agent", "cgen")
        .set("Accept", "application/vnd.github.v3+json")
        .call()
        .context("Failed to reach GitHub API")?
        .into_json()
        .context("Failed to parse GitHub API response")?;

    response["tag_name"]
        .as_str()
        .map(str::to_string)
        .context("No tag_name in GitHub release response")
}

/// Parse a strict release version (with an optional leading `v`).
pub fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
    let value = version.strip_prefix('v').unwrap_or(version);
    let mut parts = value.split('.');
    let parsed = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    if parts.next().is_some() {
        return None;
    }
    Some(parsed)
}

pub fn check_version() -> Result<VersionCheck> {
    let latest = fetch_latest_version()?;
    let current = CURRENT_VERSION.to_string();
    let update_available = match (parse_semver(&latest), parse_semver(&current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    };
    Ok(VersionCheck {
        latest,
        current,
        update_available,
    })
}

/// Update the exact installation that launched this process.
pub fn run_update(version: &str) -> Result<()> {
    let version_number = version.strip_prefix('v').unwrap_or(version);
    parse_semver(version).context("Release tag is not a strict semantic version")?;
    let executable = std::env::current_exe()
        .context("Could not locate the running cgen executable")?
        .canonicalize()
        .context("Could not resolve the running cgen executable")?;

    if !cfg!(windows) && installed_by_cargo(&executable) && cargo_available() {
        println!("{}", "Updating the Cargo installation...".cyan().bold());
        let status = Command::new("cargo")
            .args([
                "install",
                CRATE_NAME,
                "--version",
                version_number,
                "--locked",
                "--force",
            ])
            .status()
            .context("Failed to run cargo install")?;
        if !status.success() {
            anyhow::bail!("cargo install {CRATE_NAME} failed with status {status}");
        }
    } else {
        println!("{}", "Updating the release binary...".cyan().bold());
        update_release_binary(version, &executable)?;
    }

    println!("{}", "Update complete!".green().bold());
    Ok(())
}

fn installed_by_cargo(executable: &Path) -> bool {
    cargo_bin_dirs().into_iter().any(|directory| {
        directory
            .canonicalize()
            .map(|directory| executable.parent() == Some(directory.as_path()))
            .unwrap_or(false)
    })
}

fn cargo_bin_dirs() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(cargo_home) = std::env::var_os("CARGO_HOME") {
        directories.push(PathBuf::from(cargo_home).join("bin"));
    }
    if let Some(home) = dirs::home_dir() {
        let default = home.join(".cargo").join("bin");
        if !directories.contains(&default) {
            directories.push(default);
        }
    }
    directories
}

fn cargo_available() -> bool {
    Command::new("cargo")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn update_release_binary(version: &str, executable: &Path) -> Result<()> {
    let artifact = platform_artifact()?;
    let base = format!("https://github.com/{GITHUB_REPO}/releases/download/{version}");
    let binary = download(&format!("{base}/{artifact}"), MAX_DOWNLOAD_BYTES)
        .with_context(|| format!("Failed to download {artifact}"))?;
    let checksums = download(&format!("{base}/checksums.sha256"), 1_048_576)
        .context("Failed to download release checksums")?;
    let checksums =
        String::from_utf8(checksums).context("Release checksums were not valid UTF-8")?;
    verify_checksum(artifact, &binary, &checksums)?;
    replace_executable(executable, &binary)
}

fn download(url: &str, limit: usize) -> Result<Vec<u8>> {
    let response = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(60))
        .build()
        .get(url)
        .set("User-Agent", "cgen")
        .call()
        .with_context(|| format!("GET {url} failed"))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .context("Failed to read download")?;
    if bytes.len() > limit {
        anyhow::bail!("Download exceeded the {limit}-byte safety limit");
    }
    Ok(bytes)
}

fn verify_checksum(artifact: &str, binary: &[u8], checksums: &str) -> Result<()> {
    let expected = checksum_for(artifact, checksums)
        .with_context(|| format!("No checksum published for {artifact}"))?;
    let actual = format!("{:x}", Sha256::digest(binary));
    if !actual.eq_ignore_ascii_case(expected) {
        anyhow::bail!(
            "Checksum mismatch for {artifact}; expected {expected}, received {actual}. Existing binary was not changed."
        );
    }
    Ok(())
}

fn checksum_for<'a>(artifact: &str, checksums: &'a str) -> Option<&'a str> {
    checksums.lines().find_map(|line| {
        let (checksum, filename) = line.split_once(char::is_whitespace)?;
        let filename = filename.trim_start().trim_start_matches('*');
        (filename == artifact
            && checksum.len() == 64
            && checksum.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then_some(checksum)
    })
}

#[cfg(not(windows))]
fn replace_executable(executable: &Path, binary: &[u8]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let parent = executable
        .parent()
        .context("Executable must have a parent directory")?;
    let mut temp = tempfile::Builder::new()
        .prefix(".cgen-update-")
        .tempfile_in(parent)
        .with_context(|| format!("Cannot write updates in {}", parent.display()))?;
    temp.write_all(binary).context("Failed to write update")?;
    temp.flush().context("Failed to flush update")?;
    temp.as_file().sync_all().context("Failed to sync update")?;
    temp.as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o755))
        .context("Failed to make update executable")?;
    temp.persist(executable)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to replace {}", executable.display()))?;
    Ok(())
}

#[cfg(windows)]
fn replace_executable(executable: &Path, binary: &[u8]) -> Result<()> {
    let parent = executable
        .parent()
        .context("Executable must have a parent directory")?;
    let pending = parent.join(format!(".cgen-update-{}.exe", std::process::id()));
    let script = parent.join(format!(".cgen-update-{}.ps1", std::process::id()));
    std::fs::write(&pending, binary).context("Failed to stage Windows update")?;
    let script_body = format!(
        "$ErrorActionPreference='Stop'\nWait-Process -Id {} -ErrorAction SilentlyContinue\nMove-Item -LiteralPath '{}' -Destination '{}' -Force\nRemove-Item -LiteralPath $MyInvocation.MyCommand.Path -Force\n",
        std::process::id(),
        pending.display().to_string().replace('\'', "''"),
        executable.display().to_string().replace('\'', "''"),
    );
    std::fs::write(&script, script_body).context("Failed to stage Windows update helper")?;
    Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script.to_string_lossy(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to launch Windows update helper")?;
    Ok(())
}

fn platform_artifact() -> Result<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => {
            #[cfg(target_env = "musl")]
            {
                Ok("cgen-linux-amd64-musl")
            }
            #[cfg(not(target_env = "musl"))]
            {
                Ok("cgen-linux-amd64")
            }
        }
        ("linux", "aarch64") => Ok("cgen-linux-arm64"),
        ("macos", "x86_64") => Ok("cgen-macos-amd64"),
        ("macos", "aarch64") => Ok("cgen-macos-arm64"),
        ("windows", "x86_64") => Ok("cgen-windows-amd64.exe"),
        (os, arch) => anyhow::bail!("No release artifact is available for {os}/{arch}"),
    }
}

pub fn print_update_warning(latest: &str) {
    eprintln!(
        "\n{}  {} → {}  (run {} to update)",
        "Update available!".yellow().bold(),
        CURRENT_VERSION.dimmed(),
        latest.green(),
        "cgen update".cyan(),
    );
}

pub fn current_version() -> &'static str {
    CURRENT_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_and_comparison_are_strict() {
        assert_eq!(parse_semver("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("1.2"), None);
        assert_eq!(parse_semver("1.2.3.4"), None);
        assert_eq!(parse_semver("1.2.x"), None);
    }

    #[test]
    fn checksum_lookup_and_verification_are_exact() {
        let binary = b"verified binary";
        let digest = format!("{:x}", Sha256::digest(binary));
        let checksums = format!("{digest}  cgen-linux-amd64\n");
        verify_checksum("cgen-linux-amd64", binary, &checksums).unwrap();
        assert!(verify_checksum("other", binary, &checksums).is_err());
        assert!(verify_checksum("cgen-linux-amd64", b"tampered", &checksums).is_err());
    }

    #[test]
    fn crate_name_matches_manifest_package() {
        assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
    }

    #[test]
    fn current_version_is_semver() {
        assert!(parse_semver(current_version()).is_some());
    }
}
