use auto_commit_rs::config::AppConfig;
use auto_commit_rs::prompt::{
    build_correction_prompt, build_system_prompt, build_user_prompt, clean_commit_message,
    validate_commit_message,
};

#[test]
fn prompt_includes_core_sections_by_default() {
    let cfg = AppConfig::default();
    let prompt = build_system_prompt(&cfg);

    assert!(prompt.contains("inside a <diff> block"));
    assert!(prompt.contains("following the Conventional Commits specification"));
    assert!(prompt.contains("Output exactly one line"));
    assert!(prompt.contains("Output only the raw commit message"));
    assert!(prompt.contains("imperative mood"));
    assert!(prompt.contains("never fabricate issue numbers"));
    assert!(!prompt.contains("Use Gitmoji"));
    assert!(!prompt.contains("' language"));
}

#[test]
fn prompt_includes_unicode_gitmoji_when_enabled() {
    let cfg = AppConfig {
        use_gitmoji: true,
        gitmoji_format: "unicode".into(),
        ..Default::default()
    };

    let prompt = build_system_prompt(&cfg);
    assert!(prompt.contains("relevant emoji in unicode format"));
    assert!(prompt.contains("⚡"));
}

#[test]
fn prompt_includes_shortcode_gitmoji_and_locale_when_configured() {
    let cfg = AppConfig {
        use_gitmoji: true,
        gitmoji_format: "shortcode".into(),
        locale: "pl".into(),
        one_liner: false,
        ..Default::default()
    };

    let prompt = build_system_prompt(&cfg);
    assert!(prompt.contains("relevant emoji in :shortcode: format"));
    assert!(prompt.contains("in the 'pl' language"));
    assert!(prompt.contains("standard English form"));
    assert!(!prompt.contains("Output exactly one line"));
}

#[test]
fn prompt_gitmoji_does_not_override_conventional_commits() {
    let cfg = AppConfig {
        use_gitmoji: true,
        gitmoji_format: "unicode".into(),
        ..Default::default()
    };

    let prompt = build_system_prompt(&cfg);
    assert!(prompt.contains("following the Conventional Commits specification"));
    assert!(prompt.contains("Conventional Commits specification above"));
    assert!(prompt.contains("type(scope):"));
    assert!(prompt.contains("feat(api):"));
    assert!(prompt.contains("fix(auth):"));
}

#[test]
fn prompt_uses_custom_base_prompt() {
    let cfg = AppConfig {
        llm_system_prompt: "custom base prompt".into(),
        ..Default::default()
    };

    let prompt = build_system_prompt(&cfg);
    assert!(prompt.starts_with("custom base prompt"));
}

#[test]
fn prompt_examples_match_output_mode() {
    let one_liner = build_system_prompt(&AppConfig::default());
    assert!(one_liner.contains("<examples>"));
    assert!(one_liner.contains("refactor!: replace callback API with async traits"));
    assert!(!one_liner.contains("prevent redirect loop after login"));

    let full = build_system_prompt(&AppConfig {
        one_liner: false,
        ..Default::default()
    });
    assert!(full.contains("fix(auth): prevent redirect loop after login"));
    assert!(!full.contains("TTL-based eviction"));

    let gitmoji = build_system_prompt(&AppConfig {
        use_gitmoji: true,
        ..Default::default()
    });
    assert!(!gitmoji.contains("TTL-based eviction"));
    assert!(gitmoji.contains("<examples>"));
}

#[test]
fn blank_base_prompt_resolves_to_built_in_default() {
    let cfg = AppConfig {
        llm_system_prompt: "  ".into(),
        ..Default::default()
    };

    let prompt = build_system_prompt(&cfg);
    assert!(prompt.starts_with("You are an expert software engineer"));
}

#[test]
fn legacy_default_base_prompts_resolve_to_built_in_default() {
    // Every default shipped before blank-means-default existed, verbatim as it
    // was persisted into user config files by those versions
    let legacy_defaults = [
        "You are to act as an author of a commit message in git. \
         I'll send you an output of 'git diff --staged' command, and you are to convert \
         it into a commit message. Follow the Conventional Commits specification.",
        "You are to act as an author of a commit message in git.\n\
         Your mission is to create clean and comprehensive commit messages as per\n\
         the Conventional Commit specification and explain WHAT were the changes and mainly WHY the changes were done.\n\
         I'll send you an output of 'git diff --staged' command, and you are to convert\n\
         it into a commit message. Use the present tense.",
        "You are to act as an author of a commit message in git.\n\
         Your mission is to create clean and comprehensive commit messages as per\n\
         the Conventional Commit specification and explain WHAT were the changes and mainly WHY the changes were done.\n\
         I'll send you an output of 'git diff --staged' command, and you are to convert\n\
         it into a commit message. Use the present tense.\n\
         Lines must not be longer than 80 characters. Use english for the commit message.",
        "You are to act as an author of a commit message in git.\n\
         Your mission is to create clean and comprehensive commit messages as per\n\
         the Conventional Commit specification and explain WHAT were the changes and mainly WHY the changes were done.\n\
         I'll send you an output of 'git diff --staged' command, and you are to convert\n\
         it into a commit message. Use the present tense. Use english for the commit message.",
    ];

    for legacy in legacy_defaults {
        let cfg = AppConfig {
            llm_system_prompt: legacy.into(),
            ..Default::default()
        };

        let prompt = build_system_prompt(&cfg);
        assert!(
            prompt.starts_with("You are an expert software engineer"),
            "legacy default was not upgraded: {legacy}"
        );
        assert!(!prompt.contains("act as an author"));
    }
}

#[test]
fn correction_prompt_shows_attempt_and_error_then_restates_task() {
    let prompt = build_correction_prompt(
        "diff --git a/x b/x",
        "a bad message",
        "Commit message must start with a Conventional Commit header",
    );

    assert!(prompt.starts_with("<diff>\ndiff --git a/x b/x\n</diff>"));
    assert!(prompt.contains("<previous_attempt>\na bad message\n</previous_attempt>"));
    assert!(prompt.contains(
        "<error>\nCommit message must start with a Conventional Commit header\n</error>"
    ));
    let task = prompt.rfind("Output only the raw commit message").unwrap();
    assert!(task > prompt.rfind("</error>").unwrap());
}

#[test]
fn user_prompt_wraps_diff_and_restates_task_after_it() {
    let user_prompt = build_user_prompt("diff --git a/x b/x\n+line");

    assert!(user_prompt.starts_with("<diff>\ndiff --git a/x b/x\n+line\n</diff>"));
    let diff_end = user_prompt.find("</diff>").unwrap();
    let instruction = user_prompt
        .find("Output only the raw commit message")
        .unwrap();
    assert!(instruction > diff_end);
}

#[test]
fn clean_message_strips_markdown_code_fence() {
    let raw = "```\nfeat: add login\n```";
    assert_eq!(clean_commit_message(raw), "feat: add login");
}

#[test]
fn clean_message_strips_code_fence_with_language_tag() {
    let raw = "```commit\nfix(auth): correct redirect\n```";
    assert_eq!(clean_commit_message(raw), "fix(auth): correct redirect");
}

#[test]
fn clean_message_strips_label_prefix() {
    let raw = "Here's your commit message:\nfeat: implement dark mode";
    assert_eq!(clean_commit_message(raw), "feat: implement dark mode");
}

#[test]
fn clean_message_strips_surrounding_quotes() {
    let raw = "\"feat: add user authentication\"";
    assert_eq!(clean_commit_message(raw), "feat: add user authentication");
}

#[test]
fn clean_message_passes_through_clean_input() {
    let raw = "feat(api): improve response time";
    assert_eq!(clean_commit_message(raw), raw);
}

#[test]
fn clean_message_handles_multiline_with_fence() {
    let raw = "```\nfeat: add search\n\nAdds full-text search support.\n```";
    assert_eq!(
        clean_commit_message(raw),
        "feat: add search\n\nAdds full-text search support."
    );
}

#[test]
fn validates_conventional_one_liner() {
    let cfg = AppConfig::default();
    validate_commit_message("feat(parser): add strict validation", &cfg).unwrap();
    assert!(validate_commit_message("", &cfg).is_err());
    assert!(validate_commit_message("not a conventional header", &cfg).is_err());
    assert!(validate_commit_message("feat: first\n\nbody", &cfg).is_err());
}

#[test]
fn validates_configured_gitmoji_format() {
    let mut cfg = AppConfig {
        use_gitmoji: true,
        ..Default::default()
    };
    validate_commit_message("✨ feat: add presets", &cfg).unwrap();
    assert!(validate_commit_message("feat: add presets", &cfg).is_err());

    cfg.gitmoji_format = "shortcode".into();
    validate_commit_message(":sparkles: feat: add presets", &cfg).unwrap();
    assert!(validate_commit_message("✨ feat: add presets", &cfg).is_err());
}
