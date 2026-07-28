use auto_commit_rs::config::AppConfig;
use auto_commit_rs::{generation, provider};
use mockito::Matcher;

fn config(url: String) -> AppConfig {
    AppConfig {
        provider: "openai".into(),
        model: "test-model".into(),
        api_key: "test-key".into(),
        api_url: url,
        api_headers: "Authorization: Bearer $ACR_API_KEY".into(),
        fallback_enabled: false,
        one_liner: true,
        ..AppConfig::default()
    }
}

#[test]
fn generates_candidates_sequentially_with_alternative_context() {
    let mut server = mockito::Server::new();
    let first = server
        .mock("POST", "/generate")
        .match_body(Matcher::Regex(
            r#""content":"<diff>\\ndiff\\n</diff>\\n\\nWrite"#.into(),
        ))
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"feat: first"}}]}"#)
        .create();
    let second = server
        .mock("POST", "/generate")
        .match_body(Matcher::Regex("recent_candidates".into()))
        .match_body(Matcher::Regex("feat: first".into()))
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"feat: second"}}]}"#)
        .create();
    let cfg = config(format!("{}/generate", server.url()));

    let candidates = generation::generate_candidates(
        &cfg,
        "diff",
        2,
        Some("focus on behavior"),
        provider::OutputMode::Quiet,
    )
    .unwrap();

    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].message, "feat: first");
    assert_eq!(candidates[1].message, "feat: second");
    first.assert();
    second.assert();
}

#[test]
fn rejects_zero_candidates() {
    let cfg = config("http://127.0.0.1/unused".into());
    assert!(
        generation::generate_candidates(&cfg, "diff", 0, None, provider::OutputMode::Quiet)
            .is_err()
    );
}
