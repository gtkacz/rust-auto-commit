mod common;

use auto_commit_rs::config::AppConfig;
use auto_commit_rs::hook::{self, HookStatus};
use common::{git_ok, init_git_repo, write_file, DirGuard};
use serial_test::serial;
use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).unwrap().permissions().mode() & 0o111 != 0
}

#[test]
#[serial]
fn install_chains_idempotently_and_uninstall_restores_existing_hook() {
    let repo = init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let hook_path = repo.path().join(".git/hooks/prepare-commit-msg");
    let original = b"#!/bin/sh\nprintf original >&2\n";
    fs::write(&hook_path, original).unwrap();

    let installed = hook::install().unwrap();
    assert_eq!(installed, hook_path);
    assert!(matches!(
        hook::status().unwrap(),
        HookStatus::Installed { .. }
    ));
    let wrapper = fs::read_to_string(&hook_path).unwrap();
    assert!(wrapper.contains("cgen-managed"));
    assert!(wrapper.contains("prepare-commit-msg.cgen-backup"));
    #[cfg(unix)]
    assert!(executable(&hook_path));
    let once = fs::read(&hook_path).unwrap();

    hook::install().unwrap();
    assert_eq!(fs::read(&hook_path).unwrap(), once);
    assert!(!repo
        .path()
        .join(".git/hooks/prepare-commit-msg.cgen-backup.1")
        .exists());

    hook::uninstall().unwrap();
    assert_eq!(fs::read(&hook_path).unwrap(), original);
    assert!(matches!(
        hook::status().unwrap(),
        HookStatus::Unmanaged { .. }
    ));
}

#[test]
#[serial]
fn install_uses_collision_checked_backup_and_custom_hooks_path() {
    let repo = init_git_repo();
    git_ok(repo.path(), ["config", "core.hooksPath", ".githooks"]);
    let _cwd = DirGuard::enter(repo.path());
    let hook_path = repo.path().join(".githooks/prepare-commit-msg");
    fs::create_dir_all(hook_path.parent().unwrap()).unwrap();
    fs::write(&hook_path, "#!/bin/sh\nexit 0\n").unwrap();
    fs::write(
        repo.path().join(".githooks/prepare-commit-msg.cgen-backup"),
        "collision",
    )
    .unwrap();

    hook::install().unwrap();
    let wrapper = fs::read_to_string(&hook_path).unwrap();
    assert!(wrapper.contains("prepare-commit-msg.cgen-backup.1"));
    hook::uninstall().unwrap();
    assert_eq!(
        fs::read_to_string(&hook_path).unwrap(),
        "#!/bin/sh\nexit 0\n"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join(".githooks/prepare-commit-msg.cgen-backup")).unwrap(),
        "collision"
    );
}

#[test]
#[serial]
fn hook_run_skips_sources_and_existing_messages() {
    let repo = init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let message = repo.path().join("COMMIT_EDITMSG");
    let cfg = AppConfig::default();

    write_file(&message, "# status\n");
    hook::run(&message, Some("merge"), &cfg).unwrap();
    assert_eq!(fs::read_to_string(&message).unwrap(), "# status\n");

    write_file(&message, "feat: supplied by earlier hook\n# status\n");
    hook::run(&message, None, &cfg).unwrap();
    assert_eq!(
        fs::read_to_string(&message).unwrap(),
        "feat: supplied by earlier hook\n# status\n"
    );
}

#[test]
#[serial]
fn hook_run_prepends_generated_message_and_preserves_comments() {
    let repo = init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    write_file(&repo.path().join("file.txt"), "content\n");
    git_ok(repo.path(), ["add", "file.txt"]);
    let message = repo.path().join("COMMIT_EDITMSG");
    let comments = "# Changes to be committed:\n#\tnew file: file.txt\n";
    write_file(&message, comments);

    let mut server = mockito::Server::new();
    let generation = server
        .mock("POST", "/generate")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"feat: add file"}}]}"#)
        .create();
    let cfg = AppConfig {
        provider: "openai".into(),
        model: "test".into(),
        api_key: "key".into(),
        api_url: format!("{}/generate", server.url()),
        api_headers: "Authorization: Bearer $ACR_API_KEY".into(),
        fallback_enabled: false,
        ..AppConfig::default()
    };

    hook::run(&message, None, &cfg).unwrap();
    assert_eq!(
        fs::read_to_string(&message).unwrap(),
        format!("feat: add file\n\n{comments}")
    );
    generation.assert();
}

#[test]
#[serial]
#[cfg(unix)]
fn wrapper_propagates_original_failure_but_fails_open_for_cgen() {
    use std::os::unix::fs::PermissionsExt;

    let repo = init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let hook_path = repo.path().join(".git/hooks/prepare-commit-msg");
    fs::write(&hook_path, "#!/bin/sh\nexit 17\n").unwrap();
    let mut permissions = fs::metadata(&hook_path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&hook_path, permissions).unwrap();
    hook::install().unwrap();
    let message = repo.path().join("COMMIT_EDITMSG");
    write_file(&message, "# status\n");

    let chained = Command::new(&hook_path).arg(&message).output().unwrap();
    assert_eq!(chained.status.code(), Some(17));

    hook::uninstall().unwrap();
    fs::remove_file(&hook_path).unwrap();
    hook::install().unwrap();
    let wrapper = fs::read_to_string(&hook_path)
        .unwrap()
        .lines()
        .map(|line| {
            if line.starts_with("if ! ") && line.contains(" hook run ") {
                "if ! /bin/false; then"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&hook_path, format!("{wrapper}\n")).unwrap();
    let cgen_failed = Command::new(&hook_path).arg(&message).output().unwrap();
    assert!(cgen_failed.status.success());
    assert!(String::from_utf8_lossy(&cgen_failed.stderr)
        .contains("cgen could not generate a commit message"));
}

#[test]
#[serial]
fn uninstall_refuses_marker_only_unmanaged_hook() {
    let repo = init_git_repo();
    let _cwd = DirGuard::enter(repo.path());
    let hook_path = repo.path().join(".git/hooks/prepare-commit-msg");
    fs::write(
        &hook_path,
        "#!/bin/sh\n# cgen-managed prepare-commit-msg hook v1\nexit 0\n",
    )
    .unwrap();
    assert!(hook::uninstall().is_err());
    assert!(hook_path.exists());
}
