use std::process::Command;
use crate::error::Result;
use colored::Colorize;
use semver::Version;
use log::warn;

// Get latest version from crates.io
pub async fn get_opencommit_latest_version() -> Result<Option<String>> {
    let output = Command::new("cargo")
        .args(&["search", "opencommit", "--limit", "1"])
        .output()?;
        
    if !output.status.success() {
        return Ok(None);
    }
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    
    // Parse output to get version
    // Output format: "opencommit = "0.1.0" # Auto-generate meaningful commits in a second"
    if let Some(version_start) = output_str.find('"') {
        if let Some(version_end) = output_str[version_start + 1..].find('"') {
            let version = &output_str[version_start + 1..version_start + 1 + version_end];
            return Ok(Some(version.to_string()));
        }
    }
    
    Ok(None)
}

// Check if current version is latest
pub async fn check_latest_version() -> Result<()> {
    // Get current version from Cargo.toml
    let current_version = env!("CARGO_PKG_VERSION");
    
    // Get latest version from crates.io
    if let Some(latest_version) = get_opencommit_latest_version().await? {
        // Parse versions
        let current = Version::parse(current_version)?;
        let latest = Version::parse(&latest_version)?;
        
        // Compare versions
        if current < latest {
            println!("{}", format!(r#"
You are not using the latest stable version of OpenCommit with new features and bug fixes.
Current version: {}. Latest version: {}.
🚀 To update run: cargo install opencommit --force
            "#, current_version, latest_version).yellow());
        }
    }
    
    Ok(())
}