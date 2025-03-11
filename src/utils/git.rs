use std::path::Path;
use git2::{Repository, Status, StatusOptions};
use crate::error::{Error, Result};
use std::fs;
use std::process::Command;
use ignore::gitignore::{GitignoreBuilder, Gitignore};

// Assert we're in a git repository
pub fn assert_git_repo() -> Result<Repository> {
    match Repository::open_from_env() {
        Ok(repo) => Ok(repo),
        Err(_) => Err(Error::NotGitRepository),
    }
}

// Get OpenCommit ignore rules
pub fn get_opencommit_ignore() -> Result<Gitignore> {
    let mut builder = GitignoreBuilder::new(".");
    
    // Add default ignore patterns
    builder.add_line(None, "*-lock.*")?;
    builder.add_line(None, "*.lock")?;
    
    // Try to load custom ignore file
    if Path::new(".opencommitignore").exists() {
        let content = fs::read_to_string(".opencommitignore")?;
        for line in content.lines() {
            if !line.trim().is_empty() && !line.trim().starts_with('#') {
                builder.add_line(None, line)?;
            }
        }
    }
    
    builder.build().map_err(|e| Error::Git(git2::Error::from_str(&e.to_string())))
}

// Get staged files
pub fn get_staged_files(repo: &Repository) -> Result<Vec<String>> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(false)
        .recurse_untracked_dirs(false)
        .include_ignored(false);
    
    let statuses = repo.statuses(Some(&mut opts))?;
    
    // Get ignore rules
    let ignore = get_opencommit_ignore()?;
    
    let mut files = Vec::new();
    for entry in statuses.iter() {
        if entry.status().contains(Status::INDEX_NEW) || 
           entry.status().contains(Status::INDEX_MODIFIED) || 
           entry.status().contains(Status::INDEX_RENAMED) || 
           entry.status().contains(Status::INDEX_TYPECHANGE) {
            if let Some(path) = entry.path() {
                // Check if file is ignored
                if !ignore.matched(path, false).is_ignore() {
                    files.push(path.to_string());
                }
            }
        }
    }
    
    files.sort();
    Ok(files)
}

// Get changed but unstaged files
pub fn get_changed_files(repo: &Repository) -> Result<Vec<String>> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false);
    
    let statuses = repo.statuses(Some(&mut opts))?;
    
    // Get ignore rules
    let ignore = get_opencommit_ignore()?;
    
    let mut files = Vec::new();
    for entry in statuses.iter() {
        if entry.status().contains(Status::WT_MODIFIED) || 
           entry.status().contains(Status::WT_NEW) {
            if let Some(path) = entry.path() {
                // Check if file is ignored
                if !ignore.matched(path, false).is_ignore() {
                    files.push(path.to_string());
                }
            }
        }
    }
    
    files.sort();
    Ok(files)
}

// Add files to git index
pub fn git_add(repo: &Repository, files: &[String]) -> Result<()> {
    let mut index = repo.index()?;
    
    for file in files {
        index.add_path(Path::new(file))?;
    }
    
    index.write()?;
    
    Ok(())
}

// Get diff of staged files
pub fn get_diff(repo: &Repository, files: &[String]) -> Result<String> {
    // Use git command for diff to match original behavior
    let mut args = vec!["diff", "--staged"];
    
    // Filter out lock files and binary files
    let filtered_files: Vec<_> = files.iter()
        .filter(|file| {
            !file.contains(".lock") && 
            !file.contains("-lock.") &&
            !file.contains(".svg") &&
            !file.contains(".png") &&
            !file.contains(".jpg") &&
            !file.contains(".jpeg") &&
            !file.contains(".webp") &&
            !file.contains(".gif")
        })
        .map(|s| s.as_str())
        .collect();
    
    // Add files to command
    args.extend(filtered_files);
    
    let output = Command::new("git")
        .args(&args)
        .output()?;
        
    if !output.status.success() {
        return Err(Error::Git(git2::Error::from_str(&String::from_utf8_lossy(&output.stderr))));
    }
    
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}