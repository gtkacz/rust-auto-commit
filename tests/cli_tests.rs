use auto_commit_rs::cli::{Cli, Command};
use clap::Parser;

#[test]
fn parses_prompt_subcommand() {
    let cli = Cli::try_parse_from(["cgen", "prompt"]).expect("prompt should parse");
    assert!(matches!(cli.command, Some(Command::Prompt)));
}

#[test]
fn parses_config_subcommand_without_scope_flag() {
    let cli = Cli::try_parse_from(["cgen", "config"]).expect("config should parse");
    assert!(matches!(cli.command, Some(Command::Config)));
}

#[test]
fn rejects_removed_config_global_flag() {
    let err = Cli::try_parse_from(["cgen", "config", "--global"]).expect_err("should fail");
    let rendered = err.to_string();
    assert!(
        rendered.contains("--global"),
        "expected clap to mention removed --global flag, got: {rendered}"
    );
}

#[test]
fn parses_set_override() {
    let cli = Cli::try_parse_from(["cgen", "--set", "model=gpt-4o"]).expect("should parse");
    assert_eq!(cli.set, vec!["model=gpt-4o".to_string()]);
}

#[test]
fn parses_repeated_set_overrides() {
    let cli = Cli::try_parse_from(["cgen", "--set", "model=gpt-4o", "--set", "one_liner=false"])
        .expect("should parse");
    assert_eq!(
        cli.set,
        vec!["model=gpt-4o".to_string(), "one_liner=false".to_string()]
    );
}

#[test]
fn parses_diff_include_and_exclude() {
    let cli = Cli::try_parse_from(["cgen", "--diff-include", "*.xml", "--diff-exclude", "*.sql"])
        .expect("should parse");
    assert_eq!(cli.diff_include, vec!["*.xml".to_string()]);
    assert_eq!(cli.diff_exclude, vec!["*.sql".to_string()]);
}

#[test]
fn set_before_double_dash_forwards_rest_to_git() {
    let cli = Cli::try_parse_from(["cgen", "--set", "model=x", "--", "--no-verify"])
        .expect("should parse");
    assert_eq!(cli.set, vec!["model=x".to_string()]);
    assert!(cli.extra_args.contains(&"--no-verify".to_string()));
}

#[test]
fn set_with_alter_subcommand_dispatches() {
    let cli =
        Cli::try_parse_from(["cgen", "--set", "model=x", "alter", "abc123"]).expect("should parse");
    assert_eq!(cli.set, vec!["model=x".to_string()]);
    match cli.command {
        Some(Command::Alter { commits }) => assert_eq!(commits, vec!["abc123".to_string()]),
        other => panic!("expected Alter subcommand, got {other:?}"),
    }
}
