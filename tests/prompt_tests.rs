use auto_commit_rs::config::AppConfig;
use auto_commit_rs::prompt::{build_system_prompt, clean_commit_message, validate_commit_message};

#[test]
fn prompt_includes_core_sections_by_default() {
    let cfg = AppConfig::default();
    let prompt = build_system_prompt(&cfg);

    assert!(prompt.contains("following the Conventional Commits specification"));
    assert!(prompt.contains("Output ONLY a single-line commit message"));
    assert!(prompt.contains("Output only the raw commit message"));
    assert!(!prompt.contains("Use Gitmoji"));
    assert!(!prompt.contains("locale."));
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
    assert!(prompt.contains("Write the commit message in the 'pl' locale."));
    assert!(!prompt.contains("Output ONLY a single-line commit message"));
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
