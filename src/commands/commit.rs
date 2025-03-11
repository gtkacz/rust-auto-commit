use crate::error::{Error, Result};
use crate::engine::get_engine;
use crate::prompts::get_main_commit_prompt;
use crate::utils::git::{assert_git_repo, get_staged_files, get_changed_files, git_add, get_diff};
use crate::commands::config::Config;

use std::process::{Command, Stdio};
use colored::Colorize;
use inquire::{Confirm, Select, MultiSelect};
use indicatif::{ProgressBar, ProgressStyle};
use log::{info, error, debug};
use tokio::time::sleep;
use tokio::time::Duration;

// Check message template for placeholder
fn check_message_template(extra_args: &[String], config: &Config) -> Option<String> {
    for arg in extra_args {
        if arg.contains(&config.message_template_placeholder) {
            return Some(arg.clone());
        }
    }
    None
}

// Main function to execute the commit command
pub async fn execute_commit(
    extra_args: Vec<String>,
    context: Option<String>,
    is_stage_all: bool,
    full_gitmoji_spec: bool,
    skip_confirmation: bool,
) -> Result<()> {
    println!("{}", "OpenCommit".bright_blue().bold());
    
    // Ensure we're in a git repository
    let repo = assert_git_repo()?;
    
    // Handle staging files if requested
    let mut staged_files = Vec::new();
    
    if is_stage_all {
        let changed_files = get_changed_files(&repo)?;
        if changed_files.is_empty() {
            println!("{}", "No changes detected, write some code and run `oco` again".yellow());
            return Err(Error::NoStagedFiles);
        }
        
        git_add(&repo, &changed_files)?;
        staged_files = changed_files;
    } else {
        staged_files = get_staged_files(&repo)?;
    }
    
    // If no files are staged, offer to stage some
    if staged_files.is_empty() {
        let changed_files = get_changed_files(&repo)?;
        
        if changed_files.is_empty() {
            println!("{}", "No changes detected".red());
            return Err(Error::NoStagedFiles);
        }
        
        println!("{}", "No files are staged".yellow());
        
        let stage_all = Confirm::new("Do you want to stage all files and generate commit message?")
            .with_default(false)
            .prompt();
        
        match stage_all {
            Ok(true) => {
                return execute_commit(extra_args, context, true, full_gitmoji_spec, skip_confirmation).await;
            }
            Ok(false) => {
                // Let user select files to stage
                let selected_files = MultiSelect::new(
                    "Select the files you want to add to the commit:",
                    changed_files.clone()
                )
                .prompt();
                
                match selected_files {
                    Ok(files) => {
                        if files.is_empty() {
                            println!("{}", "No files selected".yellow());
                            return Err(Error::UserCancelled);
                        }
                        
                        git_add(&repo, &files)?;
                        staged_files = files;
                    }
                    Err(_) => {
                        return Err(Error::UserCancelled);
                    }
                }
            }
            Err(_) => {
                return Err(Error::UserCancelled);
            }
        }
    }
    
    // Print staged files
    println!("{} staged files:", staged_files.len());
    for file in &staged_files {
        println!("  {}", file);
    }
    
    // Get diff of staged files
    let diff = get_diff(&repo, &staged_files)?;
    
    // Load config
    let config = Config::load()?;
    
    // Check if API key is configured
    if config.api_key.is_none() && config.ai_provider != "ollama" && config.ai_provider != "test" {
        return Err(Error::NoApiKey);
    }
    
    // Generate commit message
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner} {msg}")
            .unwrap(),
    );
    spinner.set_message("Generating the commit message");
    spinner.enable_steady_tick(Duration::from_millis(100));
    
    let messages = get_main_commit_prompt(
        full_gitmoji_spec,
        context.unwrap_or_default(),
    ).await?;
    
    let engine = get_engine(&config)?;
    let mut commit_message = engine.generate_commit_message(messages, &diff).await?;
    
    // Check for message template
    if let Some(template) = check_message_template(&extra_args, &config) {
        let mut new_extra_args = extra_args.clone();
        let template_index = new_extra_args.iter().position(|arg| arg == &template).unwrap();
        new_extra_args.remove(template_index);
        
        commit_message = template.replace(&config.message_template_placeholder, &commit_message);
    }
    
    spinner.finish_and_clear();
    
    // Display generated message
    println!("\n{}", "Generated commit message:".green());
    println!("{}", "——————————————————".bright_black());
    println!("{}", commit_message);
    println!("{}", "——————————————————".bright_black());
    
    // Get confirmation
    let confirmed = if skip_confirmation {
        true
    } else {
        match Confirm::new("Confirm the commit message?")
            .with_default(true)
            .prompt() 
        {
            Ok(confirmed) => confirmed,
            Err(_) => return Err(Error::UserCancelled),
        }
    };
    
    if confirmed {
        // Execute git commit
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("{spinner} {msg}")
                .unwrap(),
        );
        spinner.set_message("Committing the changes");
        spinner.enable_steady_tick(Duration::from_millis(100));
        
        let mut commit_args = vec!["commit", "-m", &commit_message];
        for arg in extra_args {
            commit_args.push(&arg);
        }
        
        let output = Command::new("git")
            .args(&commit_args)
            .output()?;
            
        spinner.finish_with_message(format!("{} Successfully committed", "✓".green()));
        
        if !output.stdout.is_empty() {
            println!("{}", String::from_utf8_lossy(&output.stdout));
        }
        
        // Handle push if needed
        if config.gitpush {
            // Get remotes
            let remotes_output = Command::new("git")
                .args(&["remote"])
                .output()?;
                
            let remotes_str = String::from_utf8_lossy(&remotes_output.stdout);
            let remotes: Vec<&str> = remotes_str.lines().collect();
            
            if remotes.is_empty() {
                // No remotes, nothing to push
                return Ok(());
            }
            
            if remotes.len() == 1 {
                // Single remote, ask if user wants to push
                let push_confirmed = match Confirm::new(&format!("Do you want to run `git push {}`?", remotes[0]))
                    .with_default(true)
                    .prompt() 
                {
                    Ok(confirmed) => confirmed,
                    Err(_) => return Err(Error::UserCancelled),
                };
                
                if push_confirmed {
                    let spinner = ProgressBar::new_spinner();
                    spinner.set_style(
                        ProgressStyle::default_spinner()
                            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                            .template("{spinner} {msg}")
                            .unwrap(),
                    );
                    spinner.set_message(format!("Running 'git push {}'", remotes[0]));
                    spinner.enable_steady_tick(Duration::from_millis(100));
                    
                    let output = Command::new("git")
                        .args(&["push", "--verbose", remotes[0]])
                        .output()?;
                        
                    spinner.finish_with_message(format!("{} Successfully pushed all commits to {}", "✓".green(), remotes[0]));
                    
                    if !output.stdout.is_empty() {
                        println!("{}", String::from_utf8_lossy(&output.stdout));
                    }
                } else {
                    println!("{}", "`git push` aborted".yellow());
                }
            } else {
                // Multiple remotes, let user choose
                let mut options = remotes.to_vec();
                options.push("don't push");
                
                let selected = Select::new("Choose a remote to push to:", options)
                    .prompt();
                    
                match selected {
                    Ok(remote) => {
                        if remote != "don't push" {
                            let spinner = ProgressBar::new_spinner();
                            spinner.set_style(
                                ProgressStyle::default_spinner()
                                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                                    .template("{spinner} {msg}")
                                    .unwrap(),
                            );
                            spinner.set_message(format!("Running 'git push {}'", remote));
                            spinner.enable_steady_tick(Duration::from_millis(100));
                            
                            let output = Command::new("git")
                                .args(&["push", remote])
                                .output()?;
                                
                            spinner.finish_with_message(format!("{} Successfully pushed all commits to {}", "✓".green(), remote));
                            
                            if !output.stdout.is_empty() {
                                println!("{}", String::from_utf8_lossy(&output.stdout));
                            }
                        }
                    }
                    Err(_) => {
                        return Err(Error::UserCancelled);
                    }
                }
            }
        }
    } else {
        // Ask if user wants to regenerate the message
        let regenerate = match Confirm::new("Do you want to regenerate the message?")
            .with_default(false)
            .prompt() 
        {
            Ok(regenerate) => regenerate,
            Err(_) => return Err(Error::UserCancelled),
        };
        
        if regenerate {
            return execute_commit(extra_args, context, false, full_gitmoji_spec, skip_confirmation).await;
        }
    }
    
    Ok(())
}