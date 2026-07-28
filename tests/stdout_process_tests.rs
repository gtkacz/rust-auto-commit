mod common;

use common::{commit_file, git_stdout, init_git_repo, write_file};
use mockito::Matcher;
use std::path::Path;
use std::process::{Command, Output};

fn run_cgen(repo: &Path, config_home: &Path, api_url: &str, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cgen"));
    command
        .args(args)
        .current_dir(repo)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("NO_COLOR", "1")
        .env("ACR_CONFIG_HOME", config_home)
        .env("ACR_PROVIDER", "openai")
        .env("ACR_MODEL", "test-model")
        .env("ACR_API_KEY", "test-key")
        .env("ACR_API_URL", api_url)
        .env("ACR_API_HEADERS", "Authorization: Bearer $ACR_API_KEY")
        .env("ACR_AUTO_UPDATE", "false")
        .env("ACR_REVIEW_COMMIT", "true")
        .env("ACR_POST_COMMIT_PUSH", "never");
    if let Some(profile_file) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile_file);
    }
    command.output().expect("failed to run cgen")
}

#[test]
fn stdout_emits_exact_message_and_does_not_commit() {
    let repo = init_git_repo();
    write_file(&repo.path().join("new.txt"), "new\n");
    common::git_ok(repo.path(), ["add", "new.txt"]);
    let before = git_stdout(repo.path(), ["status", "--porcelain"]);

    let mut server = mockito::Server::new();
    let generation = server
        .mock("POST", "/generate")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"feat: add fixture"}}]}"#)
        .create();
    let config_home = tempfile::tempdir().unwrap();
    let output = run_cgen(
        repo.path(),
        config_home.path(),
        &format!("{}/generate", server.url()),
        &["--stdout"],
    );

    assert!(output.status.success(), "{:?}", output);
    assert_eq!(output.stdout, b"feat: add fixture\n");
    assert!(output.stderr.is_empty(), "{:?}", output);
    assert_eq!(git_stdout(repo.path(), ["status", "--porcelain"]), before);
    generation.assert();
}

#[test]
fn stdout_alter_does_not_rewrite_history() {
    let repo = init_git_repo();
    let hash = commit_file(repo.path(), "tracked.txt", "before\n", "chore: initial");
    let original_subject = git_stdout(repo.path(), ["show", "-s", "--format=%s", "HEAD"]);

    let mut server = mockito::Server::new();
    let generation = server
        .mock("POST", "/generate")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"feat: describe history"}}]}"#)
        .create();
    let config_home = tempfile::tempdir().unwrap();
    let output = run_cgen(
        repo.path(),
        config_home.path(),
        &format!("{}/generate", server.url()),
        &["--stdout", "alter", &hash],
    );

    assert!(output.status.success(), "{:?}", output);
    assert_eq!(output.stdout, b"feat: describe history\n");
    assert!(output.stderr.is_empty(), "{:?}", output);
    assert_eq!(
        git_stdout(repo.path(), ["show", "-s", "--format=%s", "HEAD"]),
        original_subject
    );
    assert_eq!(git_stdout(repo.path(), ["rev-parse", "HEAD"]), hash);
    generation.assert();
}

#[test]
fn forwarded_all_stages_only_tracked_changes_before_stdout_generation() {
    let repo = init_git_repo();
    commit_file(repo.path(), "modified.txt", "before\n", "chore: initial");
    commit_file(
        repo.path(),
        "deleted.txt",
        "remove me\n",
        "chore: add deletion",
    );
    write_file(&repo.path().join("modified.txt"), "after\n");
    std::fs::remove_file(repo.path().join("deleted.txt")).unwrap();
    write_file(&repo.path().join("untracked.txt"), "do not stage\n");

    let mut server = mockito::Server::new();
    let generation = server
        .mock("POST", "/generate")
        .match_body(Matcher::Regex(
            "(?s)modified\\.txt.*deleted\\.txt|deleted\\.txt.*modified\\.txt".into(),
        ))
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"fix: sync tracked files"}}]}"#)
        .create();
    let config_home = tempfile::tempdir().unwrap();
    let output = run_cgen(
        repo.path(),
        config_home.path(),
        &format!("{}/generate", server.url()),
        &["--stdout", "--", "-a"],
    );

    assert!(output.status.success(), "{:?}", output);
    assert_eq!(output.stdout, b"fix: sync tracked files\n");
    let staged = git_stdout(repo.path(), ["diff", "--cached", "--name-only"]);
    assert_eq!(
        staged.lines().collect::<Vec<_>>(),
        ["deleted.txt", "modified.txt"]
    );
    assert_eq!(git_stdout(repo.path(), ["ls-files", "untracked.txt"]), "");
    generation.assert();
}

#[test]
fn stdout_safety_diagnostics_use_stderr_only() {
    let repo = init_git_repo();
    write_file(
        &repo.path().join("id_rsa"),
        "-----BEGIN PRIVATE KEY-----\nsecret\n",
    );
    common::git_ok(repo.path(), ["add", "id_rsa"]);
    let config_home = tempfile::tempdir().unwrap();
    let output = run_cgen(
        repo.path(),
        config_home.path(),
        "http://127.0.0.1:9/generate",
        &["--stdout"],
    );

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("sensitive"));
    assert!(!stderr.contains('\u{1b}'));
}
