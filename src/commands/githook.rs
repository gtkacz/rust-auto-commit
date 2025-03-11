use crate::error::{Error, Result};
use crate::cli::HookAction;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use colored::Colorize;
use log::{info, error};

const HOOK_NAME: &str = "prepare-commit-msg";

// Get the path to git hooks directory
fn get_hooks_path() -> Result<PathBuf> {
    // Try to get hooks path from git config
    let output = Command::new("git")
        .args(&["config", "core.hooksPath"])
        .output()?;
        
    if output.status.success() {
        let hooks_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Ok(Path::new(&hooks_path).join(HOOK_NAME));
    }
    
    // Fallback to default hooks path
    let output = Command::new("git")
        .args(&["rev-parse", "--git-dir"])
        .output()?;
        
    if !output.status.success() {
        return Err(Error::NotGitRepository);
    }
    
    let git_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(Path::new(&git_dir).join("hooks").join(HOOK_NAME))
}

// Check if current process is being called as a hook
pub fn is_hook_called() -> bool {
    match std::env::current_exe() {
        Ok(exe_path) => {
            if let Ok(hooks_path) = get_hooks_path() {
                return exe_path == hooks_path;
            }
        }
        Err(_) => {}
    }
    false
}

// Check if hook exists
fn is_hook_exists() -> Result<bool> {
    let hook_path = get_hooks_path()?;
    Ok(hook_path.exists())
}

// Handler for hook commands
pub async fn handle_hook_command(action: HookAction) -> Result<()> {
    // Get current executable path
    let exe_path = std::env::current_exe()?;
    let hook_path = get_hooks_path()?;
    
    println!("{}", "OpenCommit Hook".bright_blue());
    
    match action {
        HookAction::Set => {
            println!("Setting opencommit as '{}' hook at {}", HOOK_NAME, hook_path.display());
            
            if is_hook_exists()? {
                // Check if it's our hook already
                let target = fs::read_link(&hook_path).unwrap_or_default();
                if target == exe_path {
                    println!("OpenCommit is already set as '{}'", HOOK_NAME);
                    return Ok(());
                }
                
                return Err(Error::HookError(format!(
                    "Different {} is already set. Remove it before setting opencommit as '{}' hook.",
                    HOOK_NAME, HOOK_NAME
                )));
            }
            
            // Create parent directory if it doesn't exist
            if let Some(parent) = hook_path.parent() {
                fs::create_dir_all(parent)?;
            }
            
            // Create symlink
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&exe_path, &hook_path)?;
            }
            
            #[cfg(windows)]
            {
                // On Windows, we can't use symlinks easily, so we create a batch file
                let mut hook_content = format!(
                    "@echo off\r\n\"{}\" hook %*\r\n",
                    exe_path.display().to_string().replace("\\", "\\\\")
                );
                fs::write(&hook_path, hook_content)?;
            }
            
            // Make hook executable
            #[cfg(unix)]
            {
                let metadata = fs::metadata(&hook_path)?;
                let mut permissions = metadata.permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(&hook_path, permissions)?;
            }
            
            println!("{} Hook set", "✓".green());
            
            Ok(())
        }
        
        HookAction::Unset => {
            println!("Unsetting opencommit as '{}' hook from {}", HOOK_NAME, hook_path.display());
            
            if !is_hook_exists()? {
                println!("OpenCommit wasn't previously set as '{}' hook, nothing to remove", HOOK_NAME);
                return Ok(());
            }
            
            // Check if it's our hook
            let is_our_hook = match fs::read_link(&hook_path) {
                Ok(target) => target == exe_path,
                Err(_) => false,
            };
            
            if !is_our_hook {
                println!(
                    "OpenCommit wasn't previously set as '{}' hook, but different hook was, if you want to remove it — do it manually",
                    HOOK_NAME
                );
                return Ok(());
            }
            
            // Remove hook
            fs::remove_file(&hook_path)?;
            
            println!("{} Hook is removed", "✓".green());
            
            Ok(())
        }
    }
}

// Function to handle prepare-commit-msg hook
pub async fn prepare_commit_msg_hook(commit_msg_file: &str) -> Result<()> {
    println!("{}", "OpenCommit Hook".bright_blue());
    
    // Check if commit message file exists
    if !Path::new(commit_msg_file).exists() {
        return Err(Error::HookError(
            "Commit message file path is missing. This file should be called from the \"prepare-commit-msg\" git hook".to_string()
        ));
    }
    
    // Get staged files
    let repo = crate::utils::git::assert_git_repo()?;
    let staged_files = crate::utils::git::get_staged_files(&repo)?;
    
    if staged_files.is_empty() {
        return Ok(());
    }
    
    // Load config
    let config = crate::commands::config::Config::load()?;
    
    if config.api_key.is_none() && config.ai_provider != "ollama" && config.ai_provider != "test" {
        println!("No OCO_API_KEY is set. Set your key via `oco config set OCO_API_KEY=<value>. For more info see https://github.com/yourusername/opencommit-rs");
        return Ok(());
    }
    
    // Show spinner
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_style(
        indicatif::ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner} {msg}")
            .unwrap(),
    );
    spinner.set_message("Generating commit message");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    
    // Get diff
    let diff = crate::utils::git::get_diff(&repo, &staged_files)?;
    
    // Generate commit message
    let messages = crate::prompts::get_main_commit_prompt(false, String::new()).await?;
    let engine = crate::engine::get_engine(&config)?;
    let commit_message = engine.generate_commit_message(messages, &diff).await?;
    
    spinner.finish_with_message("Done");
    
    // Read existing file content
    let file_content = fs::read_to_string(commit_msg_file)?;
    
    // Write new content
    fs::write(commit_msg_file, format!("{}\n{}", commit_message, file_content))?;
    
    Ok(())
}