use anyhow::{Context, Result};
use auto_commit_rs::{
    cache, cli, config, editor, generation, git, preset, prompt, provider, ui, update, workflow,
};
use colored::Colorize;
use inquire::Select;
use std::io::IsTerminal;
use std::time::Instant;

fn main() {
    if let Err(e) = run() {
        eprintln!("{} {:#}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = cli::parse();
    validate_invocation(&cli)?;
    let cfg = match &cli.command {
        Some(
            cli::Command::Config
            | cli::Command::Update
            | cli::Command::History
            | cli::Command::Preset
            | cli::Command::Fallback
            | cli::Command::Model,
        ) => None,
        Some(cli::Command::Hook {
            action: cli::HookAction::Install | cli::HookAction::Uninstall | cli::HookAction::Status,
        }) => None,
        _ => {
            let mut c = config::AppConfig::load()?;
            c.apply_overrides(&cli.set)?;
            Some(c)
        }
    };

    // On first run, ask about auto-update preference
    if let Some(c) = cfg.as_ref().filter(|_| !cli.stdout && !is_hook_run(&cli)) {
        if c.auto_update.is_none() {
            prompt_auto_update();
        }
    }

    // Check for updates, except for commands that are local-only or manage
    // updates/config themselves
    let update_warning = match &cli.command {
        Some(
            cli::Command::Config
            | cli::Command::Update
            | cli::Command::History
            | cli::Command::Preset
            | cli::Command::Fallback
            | cli::Command::Model
            | cli::Command::Hook { .. }
            | cli::Command::Prompt
            | cli::Command::Undo,
        ) => None,
        _ if cli.stdout => None,
        _ => check_for_updates(cfg.as_ref()),
    };

    match &cli.command {
        Some(cli::Command::Config) => {
            run_config_command()?;
        }
        Some(cli::Command::Update) => {
            run_update_command()?;
        }
        Some(cli::Command::History) => {
            cache::interactive_history()?;
        }
        Some(cli::Command::Preset) => {
            preset::interactive_presets()?;
        }
        Some(cli::Command::Fallback) => {
            preset::interactive_fallback_order()?;
        }
        Some(cli::Command::Model) => {
            auto_commit_rs::model::run_model_command()?;
        }
        Some(cli::Command::Hook { action }) => {
            run_hook_command(action, cfg.as_ref())?;
        }
        Some(cli::Command::Undo) => {
            run_undo(cfg.as_ref().expect("config should be loaded"))?;
        }
        Some(cli::Command::Alter { commits }) => {
            run_alter(
                cfg.as_ref().expect("config should be loaded"),
                &cli,
                commits,
            )?;
        }
        Some(cli::Command::Prompt) => {
            let c = cfg.as_ref().expect("config should be loaded");
            let system_prompt = prompt::build_system_prompt(c);
            println!("\n{}", "LLM system prompt:".cyan().bold());
            println!("{system_prompt}");
        }
        None => {
            run_standard_commit(cfg.as_ref().expect("config should be loaded"), &cli)?;
        }
    }

    // Show update warning at the end so it doesn't get buried
    if let Some(latest) = update_warning {
        update::print_update_warning(&latest);
    }

    Ok(())
}

fn is_hook_run(cli: &cli::Cli) -> bool {
    matches!(
        cli.command,
        Some(cli::Command::Hook {
            action: cli::HookAction::Run { .. }
        })
    )
}

fn forwarded_all(cli: &cli::Cli) -> bool {
    cli.extra_args
        .iter()
        .any(|arg| arg == "-a" || arg == "--all")
}

fn commit_args(cli: &cli::Cli) -> Vec<String> {
    cli.extra_args
        .iter()
        .filter(|arg| *arg != "-a" && *arg != "--all")
        .cloned()
        .collect()
}

fn validate_invocation(cli: &cli::Cli) -> Result<()> {
    let generates = matches!(cli.command, None | Some(cli::Command::Alter { .. }));
    if (cli.all || forwarded_all(cli)) && cli.command.is_some() {
        anyhow::bail!("--all can only be used when generating from the current index");
    }
    if cli.stdout && !generates {
        anyhow::bail!("--stdout is only supported for ordinary generation and `alter`");
    }
    if cli.stdout && cli.generate != 1 {
        anyhow::bail!("--stdout requires --generate 1");
    }
    if cli.stdout && !commit_args(cli).is_empty() {
        anyhow::bail!("--stdout cannot be used with arguments forwarded to git commit");
    }
    if cli.generate != 1 && !generates {
        anyhow::bail!("--generate can only be used with commit-message generation");
    }
    if cli.prompt.is_some() && !generates {
        anyhow::bail!("--prompt can only be used with ordinary or alter generation");
    }
    Ok(())
}

fn run_hook_command(action: &cli::HookAction, cfg: Option<&config::AppConfig>) -> Result<()> {
    match action {
        cli::HookAction::Install => {
            let path = auto_commit_rs::hook::install()?;
            println!("{} {}", "Installed hook:".green().bold(), path.display());
        }
        cli::HookAction::Uninstall => {
            let path = auto_commit_rs::hook::uninstall()?;
            println!("{} {}", "Uninstalled hook:".green().bold(), path.display());
        }
        cli::HookAction::Status => match auto_commit_rs::hook::status()? {
            auto_commit_rs::hook::HookStatus::Installed { path } => {
                println!("installed: {}", path.display());
            }
            auto_commit_rs::hook::HookStatus::NotInstalled => println!("not installed"),
            auto_commit_rs::hook::HookStatus::Unmanaged { path } => {
                println!(
                    "not installed (unmanaged hook exists at {})",
                    path.display()
                );
            }
        },
        cli::HookAction::Run {
            message_file,
            source,
            ..
        } => {
            auto_commit_rs::hook::run(
                message_file,
                source.as_deref(),
                cfg.expect("config should be loaded for hook run"),
            )?;
        }
    }
    Ok(())
}

fn run_standard_commit(cfg: &config::AppConfig, cli: &cli::Cli) -> Result<()> {
    ensure_api_key(cfg)?;

    if cli.all || forwarded_all(cli) {
        git::stage_tracked_changes()?;
    }
    let staged_files = git::list_staged_files().context("Failed to list staged files")?;

    let excludes: Vec<String> = cfg
        .diff_exclude_globs
        .iter()
        .chain(cli.diff_exclude.iter())
        .cloned()
        .collect();
    let diff = git::get_staged_diff_filtered(&cli.diff_include, &excludes)
        .context("Failed to get staged diff")?;
    let report = workflow::enforce_diff_safety(
        cfg,
        &diff,
        &staged_files,
        cli.allow_large_diff,
        cli.allow_sensitive,
    )?;
    if !cli.stdout {
        print_staged_files(&staged_files, &report.included_files);
    }

    if !cli.stdout {
        if let Some(prompt) = workflow::staged_files_warning(cfg, staged_files.len(), &report) {
            if !ui::confirm(&prompt, false) {
                println!("{}", "Commit cancelled.".dimmed());
                return Ok(());
            }
        }
    }

    let gen_start = Instant::now();
    let Some((final_msg, time_to_ready)) = generate_final_message(cfg, &diff, cli, gen_start)?
    else {
        return Ok(());
    };
    if cli.stdout {
        println!("{final_msg}");
        return Ok(());
    }
    if cli.verbose {
        if let Some(elapsed) = time_to_ready {
            println!(
                "  {} {}",
                "Generated in".dimmed(),
                format!("{:.2}s", elapsed.as_secs_f64()).dimmed()
            );
        }
    }

    if cli.dry_run {
        println!(
            "\n{}",
            "Dry run enabled. Commit not created.".yellow().bold()
        );
        return Ok(());
    }

    git::run_commit(&final_msg, &commit_args(cli), cfg.suppress_tool_output)
        .context("git commit failed")?;

    if cfg.track_generated_commits {
        if let Ok(repo_root) = git::find_repo_root() {
            if let Ok(hash) = cache::get_head_hash() {
                let preview: String = final_msg.chars().take(80).collect();
                let _ = cache::record_commit(&repo_root, &hash, &preview);
            }
        }
    }

    let created_tag = if cli.tag {
        create_semver_tag(cfg)?
    } else {
        None
    };

    handle_post_commit_push(cfg, "Commit created. Push now?", created_tag.as_deref())?;
    Ok(())
}

fn run_alter(cfg: &config::AppConfig, cli: &cli::Cli, commits: &[String]) -> Result<()> {
    ensure_api_key(cfg)?;

    let (target, raw_diff) = match commits {
        [single] => (
            single.to_string(),
            git::get_commit_diff(single).context("Failed to get commit diff")?,
        ),
        [older, newer] => (
            newer.to_string(),
            git::get_range_diff(older, newer).context("Failed to get range diff")?,
        ),
        _ => anyhow::bail!("Expected one or two commit hashes."),
    };
    let all_files = git::diff_paths(&raw_diff)?;
    let excludes: Vec<String> = cfg
        .diff_exclude_globs
        .iter()
        .chain(cli.diff_exclude.iter())
        .cloned()
        .collect();
    let diff = git::filter_diff_by_globs(&raw_diff, &cli.diff_include, &excludes)
        .context("Failed to filter commit diff")?;
    workflow::enforce_diff_safety(
        cfg,
        &diff,
        &all_files,
        cli.allow_large_diff,
        cli.allow_sensitive,
    )?;

    let target_is_pushed = if cli.stdout {
        false
    } else {
        git::commit_is_pushed(&target)?
    };
    if !cli.stdout && target_is_pushed {
        let proceed = ui::confirm(
            "Target commit appears to be pushed already. Rewriting history may require a force push. Continue?",
            false,
        );
        if !proceed {
            println!("{}", "Alter cancelled.".dimmed());
            return Ok(());
        }
    }

    let gen_start = Instant::now();
    let Some((final_msg, time_to_ready)) = generate_final_message(cfg, &diff, cli, gen_start)?
    else {
        return Ok(());
    };
    if cli.stdout {
        println!("{final_msg}");
        return Ok(());
    }
    if cli.verbose {
        if let Some(elapsed) = time_to_ready {
            println!(
                "  {} {}",
                "Generated in".dimmed(),
                format!("{:.2}s", elapsed.as_secs_f64()).dimmed()
            );
        }
    }

    if cli.dry_run {
        println!(
            "\n{}",
            "Dry run enabled. Commit message was generated but history was not rewritten."
                .yellow()
                .bold()
        );
        return Ok(());
    }

    let rewritten_hash = git::rewrite_commit_message(&target, &final_msg, cfg.suppress_tool_output)
        .context("Failed to rewrite commit message")?;

    if cfg.track_generated_commits {
        if let Ok(repo_root) = git::find_repo_root() {
            let preview: String = final_msg.chars().take(80).collect();
            let _ = cache::record_commit(&repo_root, &rewritten_hash, &preview);
        }
    }

    if target_is_pushed {
        let should_push = ui::confirm(
            "History was rewritten on a pushed commit. Run `git push --force-with-lease` now?",
            false,
        );
        if should_push {
            git::run_force_push_with_lease(cfg.suppress_tool_output)
                .context("force-with-lease push failed; rewritten history remains local")?;
        } else {
            println!(
                "{}",
                "Skipped push after history rewrite. Push manually when ready.".dimmed()
            );
        }
    } else {
        handle_post_commit_push(cfg, "Commit message altered. Push now?", None)?;
    }

    Ok(())
}

fn ensure_api_key(cfg: &config::AppConfig) -> Result<()> {
    if provider::provider_requires_api_key(cfg) && cfg.api_key.is_empty() {
        anyhow::bail!(
            "No API key configured. Run {} or set {}",
            "cgen config".yellow(),
            "ACR_API_KEY".yellow()
        );
    }
    Ok(())
}

fn generate_final_message(
    cfg: &config::AppConfig,
    diff: &str,
    cli: &cli::Cli,
    gen_start: Instant,
) -> Result<Option<(String, Option<std::time::Duration>)>> {
    if cli.generate > 1 && !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        anyhow::bail!(
            "Selecting from multiple generated candidates requires an interactive terminal"
        );
    }
    if cli.generate > 5
        && !ui::confirm(
            &format!(
                "Generate {} candidates? This will make at least {} provider calls.",
                cli.generate, cli.generate
            ),
            false,
        )
    {
        println!("{}", "Generation cancelled.".dimmed());
        return Ok(None);
    }

    let system_prompt = prompt::build_system_prompt_with_guidance(cfg, cli.prompt.as_deref());
    if cli.verbose {
        println!("\n{}", "LLM system prompt:".cyan().bold());
        println!("{system_prompt}\n");
    }
    let output_mode = if cli.stdout {
        provider::OutputMode::Quiet
    } else {
        provider::OutputMode::Interactive
    };

    let mut time_to_ready: Option<std::time::Duration> = None;
    let final_msg = loop {
        let candidates = generation::generate_candidates(
            cfg,
            diff,
            cli.generate,
            cli.prompt.as_deref(),
            output_mode,
        )?;
        if !cli.stdout {
            for candidate in &candidates {
                if let Some(name) = &candidate.fallback_preset {
                    println!(
                        "  {} Used fallback preset: {}",
                        "note:".yellow().bold(),
                        name
                    );
                }
            }
        }
        let Some(message) = select_candidate(cfg, candidates, cli.stdout)? else {
            return Ok(None);
        };
        if cli.stdout || !cfg.review_commit {
            break generation::apply_template(cfg, &message)?;
        }
        let reviewed = loop {
            let candidate = generation::apply_template(cfg, &message)?;

            if time_to_ready.is_none() {
                time_to_ready = Some(gen_start.elapsed());
            }
            println!("\n{}", "Commit message:".green().bold());
            println!("  {}\n", candidate);

            match review_message()? {
                ReviewAction::Accept => break Some(candidate),
                ReviewAction::Regenerate => break None,
                ReviewAction::Edit => {
                    let edited = editor::edit(&candidate)?;
                    let edited = edited.trim().to_string();
                    match prompt::validate_final_message(&edited) {
                        Ok(()) => break Some(edited),
                        Err(error) => {
                            println!("  {} {}", "invalid message:".red().bold(), error);
                            continue;
                        }
                    }
                }
                ReviewAction::Cancel => {
                    println!("{}", "Commit cancelled.".dimmed());
                    return Ok(None);
                }
            }
        };
        if let Some(reviewed) = reviewed {
            break reviewed;
        }
    };

    if time_to_ready.is_none() {
        time_to_ready = Some(gen_start.elapsed());
    }
    if !cli.stdout && !cfg.review_commit {
        println!("\n{} {}", "Commit message:".green().bold(), final_msg);
    }

    Ok(Some((final_msg, time_to_ready)))
}

fn select_candidate(
    cfg: &config::AppConfig,
    candidates: Vec<generation::GeneratedCandidate>,
    quiet: bool,
) -> Result<Option<String>> {
    if candidates.len() == 1 {
        return Ok(candidates
            .into_iter()
            .next()
            .map(|candidate| candidate.message));
    }
    if quiet {
        anyhow::bail!("Multiple candidates cannot be selected in quiet output mode");
    }
    let labels = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let preview = generation::apply_template(cfg, &candidate.message)
                .unwrap_or_else(|_| candidate.message.clone())
                .lines()
                .next()
                .unwrap_or_default()
                .to_string();
            format!("Candidate {}: {}", index + 1, preview)
        })
        .collect::<Vec<_>>();
    match Select::new("Choose a commit message:", labels.clone()).prompt() {
        Ok(selected) => {
            let index = labels
                .iter()
                .position(|label| label == &selected)
                .expect("selected candidate must exist");
            Ok(Some(candidates[index].message.clone()))
        }
        Err(_) => {
            println!("{}", "Commit cancelled.".dimmed());
            Ok(None)
        }
    }
}

fn create_semver_tag(cfg: &config::AppConfig) -> Result<Option<String>> {
    let latest = git::get_latest_tag().context("Failed to inspect existing tags")?;
    let next_tag = git::compute_next_minor_tag(latest.as_deref())?;

    let should_create = if cfg.confirm_new_version {
        let prompt = match latest.as_deref() {
            Some(tag) => format!("Create new tag {next_tag} (latest: {tag})?"),
            None => format!("Create initial tag {next_tag}?"),
        };
        ui::confirm(&prompt, true)
    } else {
        true
    };

    if !should_create {
        println!("{}", "Tag creation skipped.".dimmed());
        return Ok(None);
    }

    git::create_tag(&next_tag, cfg.suppress_tool_output).context("Failed to create git tag")?;
    println!("{} {}", "Created tag:".green().bold(), next_tag);
    Ok(Some(next_tag))
}

enum ReviewAction {
    Accept,
    Regenerate,
    Edit,
    Cancel,
}

fn review_message() -> Result<ReviewAction> {
    let choices = vec!["Accept", "Regenerate", "Edit", "Cancel"];

    let answer = Select::new("", choices).without_help_message().prompt();

    match answer {
        Ok("Accept") => Ok(ReviewAction::Accept),
        Ok("Regenerate") => Ok(ReviewAction::Regenerate),
        Ok("Edit") => Ok(ReviewAction::Edit),
        _ => Ok(ReviewAction::Cancel),
    }
}

fn print_staged_files(staged_files: &[String], llm_files: &[String]) {
    println!(
        "\n{} {}",
        "Staged files:".green().bold(),
        staged_files.len()
    );
    if staged_files.is_empty() {
        println!("  {}", "(none)".dimmed());
        return;
    }

    let llm: std::collections::HashSet<&str> = llm_files.iter().map(String::as_str).collect();
    let last = staged_files.len() - 1;
    for (i, file) in staged_files.iter().enumerate() {
        let connector = if i == last {
            "\u{2514}\u{2500}\u{2500}"
        } else {
            "\u{251C}\u{2500}\u{2500}"
        };
        if llm.contains(file.as_str()) {
            println!("  {} {}", connector, file);
        } else {
            println!("  {} {} {}", connector, file, "(not sent to LLM)".dimmed());
        }
    }
}

fn handle_post_commit_push(
    cfg: &config::AppConfig,
    ask_prompt: &str,
    created_tag: Option<&str>,
) -> Result<()> {
    let should_push = match cfg.post_commit_push.as_str() {
        "never" => false,
        "always" => true,
        _ => ui::confirm(ask_prompt, true),
    };
    if should_push {
        git::run_push(cfg.suppress_tool_output).context("git push failed")?;
        if let Some(tag) = created_tag {
            git::push_tag(tag, cfg.suppress_tool_output)?;
        }
    } else if let Some(tag) = created_tag {
        println!(
            "{}",
            format!("Tag '{tag}' remains local until it is pushed.").dimmed()
        );
    }
    Ok(())
}

fn prompt_auto_update() {
    println!(
        "  {}",
        "You can change this later with `cgen config`".dimmed()
    );
    let yes = ui::confirm("Would you like to enable automatic updates for cgen?", true);
    if let Err(e) = config::save_auto_update_preference(yes) {
        eprintln!(
            "{} Failed to save auto-update preference: {}",
            "warning:".yellow().bold(),
            e
        );
    } else {
        let status = if yes { "enabled" } else { "disabled" };
        println!("{} Auto-updates {}.\n", "done!".green().bold(), status);
    }
}

/// Check for updates and either auto-update or return the latest version for a warning.
/// Returns Some(latest_version) if a warning should be shown, None otherwise.
fn check_for_updates(cfg: Option<&config::AppConfig>) -> Option<String> {
    let version_check = match update::check_version() {
        Ok(v) => v,
        Err(_) => return None, // silently ignore network errors
    };

    if !version_check.update_available {
        return None;
    }

    let auto_update = cfg.and_then(|c| c.auto_update).unwrap_or(false);

    if auto_update {
        println!(
            "{} {} → {}",
            "Auto-updating cgen...".cyan().bold(),
            version_check.current.dimmed(),
            version_check.latest.green(),
        );
        if let Err(e) = update::run_update(&version_check.latest) {
            eprintln!("{} Auto-update failed: {}", "warning:".yellow().bold(), e);
            return Some(version_check.latest);
        }
        println!(
            "{} Restart cgen to use the new version.\n",
            "note:".yellow().bold()
        );
        return None;
    }

    Some(version_check.latest)
}

fn run_config_command() -> Result<()> {
    match git::find_repo_root() {
        Ok(_) => {
            let choices = vec!["Local (.env in repo)", "Global (TOML config)"];
            let answer = Select::new("Configure global or local settings?", choices).prompt();
            match answer {
                Ok(choice) => {
                    let global = choice.contains("Global");
                    cli::interactive_config(global)?;
                }
                Err(_) => {
                    println!("{}", "Cancelled.".dimmed());
                }
            }
        }
        Err(_) => {
            cli::interactive_config(true)?;
        }
    }
    Ok(())
}

fn run_update_command() -> Result<()> {
    println!("{}", "Checking for updates...".cyan().bold());

    match update::check_version() {
        Ok(v) if v.update_available => {
            println!(
                "{} {} → {}",
                "New version available!".green().bold(),
                v.current.dimmed(),
                v.latest.green(),
            );
            update::run_update(&v.latest)?;
        }
        Ok(v) => {
            println!(
                "{} You are already on the latest version ({}).",
                "Up to date!".green().bold(),
                v.current,
            );
        }
        Err(e) => {
            anyhow::bail!("Failed to check for updates: {}", e);
        }
    }
    Ok(())
}

fn run_undo(cfg: &config::AppConfig) -> Result<()> {
    git::ensure_head_exists()?;

    if git::head_is_merge_commit()? {
        let proceed_merge = ui::confirm(
            "Latest commit is a merge commit. Undo it with git reset --soft HEAD~1?",
            false,
        );
        if !proceed_merge {
            println!("{}", "Undo cancelled.".dimmed());
            return Ok(());
        }
    }

    if !git::has_upstream_branch()? {
        println!(
            "{}",
            "No upstream branch detected. Assuming latest commit is not pushed."
                .yellow()
                .bold()
        );
    } else if git::is_head_pushed()? {
        let proceed_pushed = ui::confirm(
            "Latest commit appears to be pushed already. Undo locally anyway?",
            false,
        );
        if !proceed_pushed {
            println!("{}", "Undo cancelled.".dimmed());
            return Ok(());
        }
    }

    git::undo_last_commit_soft(cfg.suppress_tool_output).context("Failed to undo latest commit")?;
    println!("{}", "Latest commit undone (soft reset).".green().bold());
    Ok(())
}
